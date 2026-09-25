# Design

How the port is put together, and the rules that keep it bit-identical to the Fortran. For the
per-file status and the upstream quirks that are deliberately reproduced, see
[`porting.md`](porting.md).

Goals, in priority order:

1. **Bit-identical output** against the Fortran, for the same config and forcing.
2. **Calibration throughput** -- instance-owned state, so no globals, no worker processes.
3. **Modifiability** -- readable, `unsafe`-free physics with a differential test per call.

## 1. Scope

The BMI path only: everything reachable from `solve_noahowp` with `NGEN_FORCING_ACTIVE` and
`NGEN_OUTPUT_ACTIVE` defined, plus config and parameter-table loading.

| Area | Functions |
|---|---|
| Lifecycle | `initialize`, `update`, `update_until`, `finalize` |
| Metadata | component name, input/output item counts and names (8 inputs, 23 outputs) |
| Var info | `get_var_grid/type/units/itemsize/nbytes/location` |
| Time | `get_current/start/end_time`, `get_time_units`, `get_time_step` |
| Values | `get_value`, `set_value` |
| Grid | `get_grid_rank/size/type` for grids 0 (scalar), 1 (`nsnow`), 2 (`nsoil`) |

Out of scope, all of it unreachable from the BMI path:

- `driver/` -- the standalone driver, the ASCII forcing reader and `OutputModule`, the only
  NetCDF user. Dropping it is why the model crate has **zero dependencies**.
- Gridded code. The 2-D cases are commented out upstream, so `get_grid_shape/spacing/origin/
  x/y/z`, the node/edge/face counts, `get_value_ptr` and `*_at_indices` are left null in the
  function table.
- The crop, irrigation, tiledrain and optional parameter readers, which `paramRead` never calls.

`get_value` / `set_value` accept more than the declared exchange items: the extra names are the
calibration parameters (`BEXP DKSAT SMCMAX REFKDT KDT FRZX SLOPE MFSNO SCAMAX AXAJ BXAJ XXAJ CWP
VCMX25 MP HVT RSURF_EXP RSURF_SNOW`). `BEXP`, `DKSAT` and `SMCMAX` are `nsoil` vectors.

## 2. Layout

```
noahowp/          the model: zero dependencies
  src/*.rs          one file per Fortran type/support module (domain.rs <- DomainType.f90, ...)
  src/physics/      one file per physics module (et_flux.rs <- EtFluxModule.f90, ...)
  src/fortran/      shims: namelist and list-directed readers, literal parsing, intrinsics
  src/difftest/     reader for the Fortran-recorded fixtures
noahowp-bmi/      the BMI surface: rlib + cdylib
  src/lib.rs        BmiNoahOwp <- bmi_noahowp.f90
  src/vars.rs       the exchange-item registry
  src/c_abi.rs      register_bmi and the C function table
reference/        builds the Fortran and records the fixtures (needs gfortran)
```

**Mapping discipline** -- what makes upstream changes portable:

- One Fortran module, one Rust file, items in the same order.
- Subroutine and local names kept verbatim, including their case where it helps a side-by-side
  read (`fn energy_main`, `let QPRECC`).
- Every ported function carries `// Port of <file>::<SUBROUTINE> @ <commit>`.
- The Fortran's derived types are plain structs; physics is free functions over borrowed
  structs, split at the argument list where the Fortran passes one type as both `in` and
  `inout`. No traits, no `RefCell`, no methods on the physics side.
- `Init` / `InitDefault` / `InitTransfer` become `new` / `Default::default` / `init_transfer`.
- Every Fortran `write(*,*) ... stop` is a typed error (`ConfigError`, `StepError`, `BmiError`)
  so a calibration driver survives a bad config.

## 3. Bit-identity rules

### 3.1 Types

The upstream build does not set `-fdefault-real-8`, so `real` is binary32.

| Fortran | Rust |
|---|---|
| `real` | `f32` |
| `real*8`, `double precision` | `f64` |
| `integer` | `i32` |
| `logical` | `bool` |

Only the datetime fields are `f64`; the whole physics column is `f32`. Widening to `f64` is
measurably wrong: 1 ULP apart on 0.07% of `exp` inputs and 1.4% of `sin`.

### 3.2 Expressions

1. **Keep the expression structure.** No factoring, reassociating or hoisting. `A / B / C` is
   `a / b / c`, never `a / (b * c)`; `refkdt * dksat(1) / refdk` is not
   `refkdt * (dksat(1) / refdk)` (1 ULP out in 16 sweep cases).
2. **No FMA, no fast-math.** The reference builds with `-ffp-contract=off`; Rust never
   contracts. `mul_add` does not appear in ported code.
3. **Copy literals digit for digit.** `0.703972477` is not `0.7039725`. Clippy's
   `excessive_precision` and `approx_constant` lints are off crate-wide because both suggest
   exactly this mistake.
4. **Promote where Fortran promotes**, with an explicit `as f64` / `as f32` at that point.
5. **Keep reductions as loops**, not `.iter().sum()`, so nothing invites a reassociating
   rewrite.
6. **Keep `MIN` / `MAX` argument order.** They are not commutative on signed zeros.
7. **Reproduce the sentinels.** `huge(1.0)` is `f32::MAX`; `huge(1)` assigned to a `real*8` is
   `2147483647.0`; unset table entries are `-1.E36`.

### 3.3 Intrinsics

Every transcendental and power goes through `noahowp/src/fortran/intrinsics.rs`, never a bare
`f32::exp` or `x * x`. The rule, measured against gfortran by
[`reference/intrinsics/`](../reference/intrinsics/README.md):

- **`f32`'s own methods for `exp`, `log`, `sqrt`, `sin`, `cos`, `tanh`.**
- **`powi` for every integer exponent.** gfortran expands `x ** 4` by squaring, so
  `x * x * x * x` is a different computation and disagrees on 34% of inputs.
- **`powf` for real exponents**, with the exponent behind `black_box`, because LLVM otherwise
  rewrites `powf(x, 2.0)` into a multiply and `powf(x, 0.5)` into `sqrt` in release builds
  only. A decimal point in the Fortran exponent (`x ** 3.`) means `powf`; copy it exactly.

### 3.4 Profile arrays

The snow/soil arrays are declared `(-nsnow+1 : nsoil)` and every physics loop indexes them
directly. `Shifted<T>` (`layers.rs`) is a vector with a Fortran lower bound, so ported loops use
the Fortran's own indices. Soil-only `1:nsoil` arrays use it too, with `lo = 1`, so no loop ever
has to know which convention an array follows.

## 4. Configuration

Two input grammars, both needed:

- **Namelist** (`&group key = values /`) -- `namelist.input` and `MPTABLE.TBL`. Absent keys
  leave their target untouched, so the `-1.E36` sentinels survive. `SAIM_TABLE` / `LAIM_TABLE`
  are assembled from twelve monthly keys, not read as one.
- **List-directed** (`READ(unit,*)`) -- `SOILPARM.TBL` and `GENPARM.TBL`. Each `READ` starts a
  new record and leftovers on the previous line are discarded; get that boundary wrong and a
  table silently shifts a column.

Fortran real syntax Rust's parser rejects -- the `d` exponent marker, `1.0+6` -- is handled in
`fortran/value.rs`.

`set_value` re-derives secondary parameters **inline**, as upstream does: `DKSAT` and `REFKDT`
recompute `kdt`, `SMCMAX` recomputes `frzx`, and nothing else recomputes anything. Unit
conversions are reproduced with their operation order -- `QSEVA` goes in as `src * 0.001`, which
in binary32 is not `src / 1000`.

## 5. The C ABI

`noahowp-bmi` builds `libnoahowp_bmi.so`, exporting one symbol:

```c
Bmi* register_bmi(Bmi* model);
```

It fills a CSDMS `bmi.h` (BMI 2.0) function table, so the library loads through the plain
**`bmi_c`** adapter in both `bmi-driver` and ngen, with no `iso_c_bmi` middleware. The model
lives behind `data` as a boxed `BmiNoahOwp`; `finalize` frees it. Every entry point catches
panics and returns `BMI_FAILURE`, printing the error to stderr, because a status code is all a C
caller sees. `ISNOW` is the only `int` variable; everything else is `float`.

## 6. Verification

Bit-identity is checked mechanically, never with a tolerance.

1. **Per-call fixtures.** `reference/build.sh --fixtures` instruments the Fortran to record the
   complete state before and after each of the five calls `solve_noahowp` makes, over a sampled
   Bondville year. Each ported call is handed the recorded pre-state and must reproduce the
   post-state bit for bit. The serializer is generated from the Fortran derived types, so a
   field added upstream appears on both sides at once.
2. **Sweeps.** Bondville exercises one vegetation type, one soil and a handful of options, so
   separate sweeps cover every parameter-table row, every `OPT_SNF`, every `dveg` branch and
   ~1,300 water-physics cases. See [`reference/README.md`](../reference/README.md).
3. **The timestep loop.** `tests/timestep_loop_vs_fortran.rs` runs `advance_in_time`
   continuously from initialisation, loading nothing but forcing, and matches the Fortran's
   whole state after each of the 24 consecutive recorded steps.
4. **The C ABI.** `noahowp-bmi/tests/c_abi.rs` drives the library through `register_bmi` and
   raw function pointers, as a C caller does.
5. **End to end.** Under `bmi-driver`, a year over the 53 catchments of gage-10154200 (SLOTH +
   Noah-OWP + CFE) produces the same `Q_OUT` as the Fortran `libsurfacebmi.so`, to every
   printed digit.

The energy options Bondville does not select -- including `OPT_BTR == 4` and `OPT_RSF == 5` --
are ported but have no sweep yet.

Bit-identity is a claim about a (compiler, libc) pair. The pair used is recorded in
[`porting.md`](porting.md); changing either means re-running `reference/intrinsics/run.sh`.

## 7. Future work

- **A native `bmi-driver` adapter.** The Fortran keeps its parameter tables in module-level
  `save` globals, so two instances cannot share a process and `bmi-driver` forks workers and
  pays IPC for every result. `BmiNoahOwp` holds no globals and is `Send` (asserted at compile
  time), so a Rust-only realization could run catchments as threads with no FFI and no
  serialisation.
- **A calibration API** on the model crate -- `from_config`, `reset`, `set_calibration` -- with
  parsed tables cached on `(parameter_dir, soil_class_name, veg_class_name)`, so an iteration is
  a copy of a few hundred bytes rather than four file reads.
- **An energy sweep**, to cover the energy options Bondville does not reach.
