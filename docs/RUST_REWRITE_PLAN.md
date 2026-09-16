# Noah-OWP-Modular → Rust rewrite plan

Target: a Rust reimplementation of the **BMI-facing** subset of Noah-OWP-Modular, driven by
[`bmi-driver`](https://github.com/JoshCu/bmi-driver), with a file layout that mirrors the Fortran
so upstream physics changes can be tracked and ported mechanically.

Goals, in priority order:

1. **Bit-identical output** against the current Fortran build, for the same config + forcing.
2. **Calibration throughput** — cheap re-initialisation, in-process parallelism, no `dlopen`/IPC.
3. **Modifiability** — readable, testable, `unsafe`-free physics code.

---

## 1. Scope

### 1.1 What the BMI path actually exercises

`bmi-driver` only ever calls the 29 functions listed in
`bmi-driver/src/adapters/bmi_functions.md`. Against Noah-OWP that reduces to:

| Area | Needed | Notes |
|---|---|---|
| Lifecycle | `initialize`, `update`, `update_until`, `finalize` | |
| Metadata | `get_component_name`, input/output item counts + var names | 8 inputs, 23 outputs |
| Var info | `get_var_grid/type/units/itemsize/nbytes/location` | driver caches these at init |
| Time | `get_current/start/end_time`, `get_time_units`, `get_time_step` | |
| Values | `get_value_{int,float,double}`, `set_value_{int,float,double}` | typed entry points |
| Grid | `get_grid_rank`, `get_grid_size`, `get_grid_type` | grids 0/1/2 only |

Noah-OWP's grids are `0` = scalar, `1` = `nsnow` vector, `2` = `nsoil` vector
(`bmi/bmi_noahowp.f90:326-423`). The 2-D / uniform-rectilinear cases are commented out upstream
and unreachable. **Confirmed: no gridded functionality is needed.**
`get_grid_shape/spacing/origin/x/y/z`, node/edge/face counts, `get_value_ptr`, and
`*_at_indices` are never resolved by the driver and can be omitted entirely.

### 1.2 In scope (port these)

Everything reachable from `solve_noahowp` with `NGEN_FORCING_ACTIVE` and `NGEN_OUTPUT_ACTIVE`
defined — i.e. the physics column plus config/parameter loading:

```
src/ConstantsModule.f90            src/EnergyModule.f90
src/LevelsType.f90                 src/EtFluxModule.f90
src/DomainType.f90                 src/AlbedoModule.f90
src/OptionsType.f90                src/ShortwaveRadiationModule.f90
src/ParametersType.f90             src/ThermalPropertiesModule.f90
src/ForcingType.f90                src/SnowSoilTempModule.f90
src/EnergyType.f90                 src/PrecipHeatModule.f90
src/WaterType.f90                  src/WaterModule.f90
src/NamelistRead.f90               src/CanopyWaterModule.f90
src/ParametersRead.f90             src/SnowWaterModule.f90
src/ErrorCheckModule.f90           src/SnowWaterRenew.f90
src/UtilitiesModule.f90            src/SnowLayerChange.f90
src/DateTimeUtilsModule.f90        src/SoilWaterModule.f90
src/ForcingModule.f90              src/SoilWaterMovement.f90
src/AtmProcessing.f90              src/SoilWaterRetentionCoeff.f90
src/InterceptionModule.f90         src/SurfaceRunoffModule.f90
src/RunModule.f90                  src/SurfaceRunoffInfiltration.f90
bmi/bmi_noahowp.f90                src/SubsurfaceRunoffModule.f90
```

≈ 13.5k lines of Fortran, of which ~9k is physics and ~2.5k is table/namelist parsing.

### 1.3 Out of scope (do not port)

| File | Why |
|---|---|
| `driver/NoahModularDriver.f90` | standalone mode — not needed |
| `driver/AsciiReadModule.f90` | `#ifndef NGEN_FORCING_ACTIVE` only; forcing arrives via `set_value` |
| `driver/OutputModule.f90` | `#ifndef NGEN_OUTPUT_ACTIVE` only; **the sole NetCDF dependency** |
| `bmi/bmi.f90` | abstract type; Rust uses a trait |
| `test/`, `run/`, `config/`, `configure` | build scaffolding |

Dropping `OutputModule` means **the Rust model crate has zero external dependencies** — no
NetCDF, no C toolchain. That alone removes most of the build pain.

### 1.4 Also in the BMI surface: calibration parameters

`get_value`/`set_value` expose more than the 8+23 declared exchange items. The extra settable
names are precisely the calibration knobs (`bmi/bmi_noahowp.f90:680-840`):

```
BEXP  DKSAT  SMCMAX  REFKDT  KDT  FRZX  SLOPE  MFSNO  SCAMAX
AXAJ  BXAJ   XXAJ    CWP     VCMX25  MP  HVT   RSURF_EXP  RSURF_SNOW
```

`BEXP`, `DKSAT`, `SMCMAX` are `nsoil` vectors (grid 2); the rest are scalars. Note that several
are *secondary* parameters derived at init (`KDT` from `REFKDT`, `FRZX`, …) — the Rust version
must reproduce the same recompute-on-set semantics that `ParametersType`/`paramRead` implement,
or calibration results will silently diverge. See §5.3.

---

## 2. Crate layout — mirror the Fortran

Two crates, in a new `rust/` subtree of this repo (keeps physics next to its Fortran source of
truth, so a `git log -p src/EnergyModule.f90` diff maps to one Rust file):

```
rust/
  noahowp/                     # pure model, no_std-friendly, zero deps
    Cargo.toml
    src/
      lib.rs                   # re-exports; the NoahOwp aggregate type
      constants.rs             # ConstantsModule.f90
      levels.rs                # LevelsType.f90
      domain.rs                # DomainType.f90
      options.rs               # OptionsType.f90
      parameters.rs            # ParametersType.f90
      forcing.rs               # ForcingType.f90
      energy.rs                # EnergyType.f90          (state)
      water.rs                 # WaterType.f90           (state)
      namelist_read.rs         # NamelistRead.f90
      parameters_read.rs       # ParametersRead.f90
      error_check.rs           # ErrorCheckModule.f90
      utilities.rs             # UtilitiesModule.f90
      date_time_utils.rs       # DateTimeUtilsModule.f90
      run.rs                   # RunModule.f90
      layers.rs                # NEW: -nsnow+1..nsoil indexing helper
      fortran/                 # NEW: Fortran runtime shims (namelist parser, intrinsics)
        namelist.rs
        intrinsics.rs
      physics/
        forcing_main.rs        # ForcingModule.f90
        atm_processing.rs      # AtmProcessing.f90
        interception.rs        # InterceptionModule.f90
        energy_main.rs         # EnergyModule.f90
        et_flux.rs             # EtFluxModule.f90
        albedo.rs              # AlbedoModule.f90
        shortwave_radiation.rs # ShortwaveRadiationModule.f90
        thermal_properties.rs  # ThermalPropertiesModule.f90
        snow_soil_temp.rs      # SnowSoilTempModule.f90
        precip_heat.rs         # PrecipHeatModule.f90
        water_main.rs          # WaterModule.f90
        canopy_water.rs        # CanopyWaterModule.f90
        snow_water.rs          # SnowWaterModule.f90
        snow_water_renew.rs    # SnowWaterRenew.f90
        snow_layer_change.rs   # SnowLayerChange.f90
        soil_water.rs          # SoilWaterModule.f90
        soil_water_movement.rs # SoilWaterMovement.f90
        soil_water_retention_coeff.rs
        surface_runoff.rs      # SurfaceRunoffModule.f90
        surface_runoff_infiltration.rs
        subsurface_runoff.rs   # SubsurfaceRunoffModule.f90
  noahowp-bmi/                 # BMI surface
    src/
      lib.rs                   # bmi_noahowp.f90 — the Bmi impl
      c_abi.rs                 # register_bmi + extern "C" shims (cdylib)
```

**Mapping discipline** (this is what makes upstream tracking work):

- One Fortran module → one Rust file, same order of items within the file.
- Subroutine names kept verbatim, lowercased: `SUBROUTINE ENERGY(...)` → `fn energy(...)`.
- Local variable names kept verbatim, lowercased.
- Every ported function carries a header comment with the source file and the upstream commit
  it was ported from: `// Port of src/EtFluxModule.f90::SFCDIF1 @ eaa8282`.
- A `rust/PORTING.md` table of file → upstream-commit, so `git log eaa8282..upstream/main -- src/`
  tells you exactly which Rust files need attention.

Upstream churn is low — over the last three years only `SurfaceRunoffInfiltration.f90`,
`EnergyModule.f90`, `EtFluxModule.f90`, `AtmProcessing.f90`, `SnowWaterModule.f90`,
`WaterModule.f90` and the type modules changed at all (1–2 commits each). Tracking is tractable.

### 2.1 Functional style, not OOP

The Fortran derived types are plain data with `Init/InitDefault/InitTransfer` methods; physics is
free subroutines taking those types as arguments. That maps directly onto:

```rust
pub struct NoahOwp {
    pub namelist:   Namelist,
    pub levels:     Levels,
    pub domain:     Domain,
    pub options:    Options,
    pub parameters: Parameters,
    pub water:      Water,
    pub forcing:    Forcing,
    pub energy:     Energy,
}

// physics/energy_main.rs — mirrors EnergyMain(domain, levels, options, parameters, forcing, energy, water)
pub fn energy_main(
    domain: &Domain, levels: &Levels, options: &Options,
    parameters: &mut Parameters, forcing: &mut Forcing,
    energy: &mut Energy, water: &mut Water,
) { ... }
```

Free functions over borrowed structs — no traits, no methods on the physics side. Borrow-checker
friction is real where the Fortran passes the same type as both `intent(in)` and `intent(inout)`
through a call chain; resolve it by splitting structs at the argument list (as above), not by
introducing `RefCell`.

---

## 3. Bit-identical output

This is the hard constraint and it drives most of the coding rules below.

### 3.1 Types

`config/user_build_options.gfortran.linux` has `-fdefault-real-8` **commented out**, so
Fortran `real` is IEEE binary32.

| Fortran | Rust |
|---|---|
| `real` | `f32` |
| `real*8`, `double precision` | `f64` |
| `integer` | `i32` |
| `logical` | `bool` |
| `character(len=N)` | `[u8; N]` for BMI buffers; `String` internally where length is not observable |

Only `DateTimeUtilsModule` and `DomainType`'s datetime fields are `f64`. **Everything in the
physics column is `f32`.** Do not "improve" this — widening to `f64` breaks bit-identity and
changes calibration behaviour.

### 3.2 Rules that preserve the bits

1. **Preserve expression structure exactly.** `a*b + c*d` stays `a*b + c*d`; never factor,
   reassociate, or hoist a common subexpression that changes rounding. Where Fortran writes
   `X = A / B / C`, write `a / b / c`, not `a / (b * c)`.
2. **No FMA contraction.** Rust does not auto-contract `a*b+c` into `fma`, and gfortran at `-O2`
   without `-ffp-contract=fast` does not either — but pin it: build the reference Fortran with
   `-ffp-contract=off` and never call `f32::mul_add` in ported code. Add a CI grep for `mul_add`
   in `noahowp/src/physics/`.
3. **No fast-math.** Rust has none by default; just don't reach for `core::intrinsics::fadd_fast`
   or nightly float intrinsics.
4. **Literal precision.** A Fortran `real` literal `0.7` is `0.7_f32`. Write `0.7f32`, never
   `0.7` promoted from `f64`. Mixed-precision expressions in Fortran promote per the standard —
   translate the promotion explicitly with `as f64` / `as f32` at exactly the point Fortran does it.
5. **Intrinsics.** `EXP`, `LOG`, `SQRT`, `SIN`, `COS`, `TANH`, `**` for `real` arguments are
   **not** guaranteed bit-identical between glibc's libm (what gfortran calls) and Rust's std
   (which also calls libm on Linux — usually the same symbols, but `f32::powf` vs gfortran's
   `__powisf2`/`powf` differ for integer exponents). Mitigation in `fortran/intrinsics.rs`:
   - `sqrt` → hardware instruction, exact by IEEE. Safe.
   - `exp/log/sin/cos/tanh` on `f32` → gfortran computes these as `expf` etc. Call the same libm
     symbols via `extern "C"` rather than Rust's `f32::exp` (which is `expf` too on Linux glibc,
     but pin it explicitly so it can't be swapped for a Rust-native impl).
   - `x ** n` with integer `n` → gfortran expands to repeated multiplication for small literal
     `n`. Implement `powi_fortran(x, n)` matching gfortran's binary-exponentiation expansion,
     **not** `f32::powi` (which uses a different reduction order).
   - `x ** y` with real `y` → `powf`.
   This is the single largest bit-identity risk. Budget a spike (§7, Phase 0) to measure it.
6. **Integer division and `NINT`/`INT`.** Fortran `INT()` truncates toward zero — same as Rust
   `as i32` for in-range values, but Rust's `as` saturates on overflow where Fortran is UB.
   `NINT()` is round-half-away-from-zero; Rust's `f32::round()` matches. Fortran integer division
   truncates toward zero — Rust `/` on integers does too. Note the recent upstream fix
   (`eaa8282 Fix integer division rounding to 0`) — a reminder these paths are live.
7. **`huge(1.0)` sentinels.** `InitDefault` fills fields with `huge(1.0)` = `f32::MAX`. Keep
   this exactly; some code paths compare against it.
8. **No auto-vectorisation reordering.** Reductions in the Fortran are written as explicit loops
   and LLVM will not reassociate floats without fast-math, so plain `for` loops are safe. Do not
   replace a loop with `.iter().sum()` — same result today, but it invites a future rewrite to a
   reassociating form. Keep the loops.

### 3.3 Array indexing — the `-nsnow+1 .. nsoil` problem

`domain%zsnso`, `dzsnso`, `energy%stc`, `water%snice`, `water%snliq`, `water%ficeold` are declared
`(-nsnow+1 : nsoil)`. Every physics loop uses those indices directly (`DO IZ = ISNOW+1, NSOIL`).

Introduce one helper in `layers.rs` rather than doing arithmetic at every call site:

```rust
/// Fortran array with lower bound `lo` (typically -nsnow+1).
pub struct Shifted<T> { lo: i32, data: Vec<T> }

impl<T> core::ops::Index<i32> for Shifted<T> {
    type Output = T;
    fn index(&self, i: i32) -> &T { &self.data[(i - self.lo) as usize] }
}
// + IndexMut, iter_from(lo)..=hi helpers
```

This keeps ported loops reading `for iz in (isnow + 1)..=nsoil { stc[iz] = ... }` — visually
identical to the Fortran, with a bounds check in debug and a single subtraction in release.
Soil-only arrays (`zsoil`, `sh2o`, `sice`, `smc`, `bexp`, `dksat`, `smcmax`) are `1:nsoil`; use
the same `Shifted` with `lo = 1` so indexing conventions never have to be mentally translated.

### 3.4 `GOTO`

15 `GOTO`s total: `SurfaceRunoffInfiltration.f90` (11), `SnowSoilTempModule.f90` (2),
`AlbedoModule.f90` (1), `EtFluxModule.f90` (1). Inspect each — most are loop-exit or
early-return patterns that map to `break 'label` / `return`. Only rewrite to structured control
flow when the equivalence is obvious; otherwise use a `loop { ... }` + state variable to preserve
the exact path.

---

## 4. Integration with `bmi-driver`

Two tiers. Build tier A first (it is the compatibility oracle), then tier B (the performance win).

### Tier A — `cdylib` exposing the **C** BMI ABI

Ship `libnoahowp_bmi.so` exporting:

```c
Bmi* register_bmi(Bmi* model);
```

filling the struct in `bmi-driver/src/adapters/ffi.rs`. Use the existing `bmi_c` adapter — **not**
`bmi_fortran`. Reasons:

- No Fortran middleware `.so` needed; `BmiC` treats absent optional function pointers as absent,
  whereas `BmiFortran` requires all 29 symbols to resolve at load.
- The `Bmi` struct is `#[repr(C)]` and already defined in the driver — the model crate can
  duplicate that definition and the ABI is checked by construction.
- Realization config changes from `"name": "bmi_fortran"` to `"name": "bmi_c"`, plus
  `"library_file"` pointing at the Rust `.so`. Nothing else in the config moves.

Note `BmiC` calls `update_until` with `c_double` **by value** while `BmiFortran` passes a
pointer — one more reason to sit on the C side.

Left unset (never called by the driver): `get_value_ptr`, `*_at_indices`, all extended grid
functions.

### Tier B — native in-process adapter

Add `BmiNoahOwp` to `bmi-driver/src/adapters/`, alongside `BmiSloth`, implementing the driver's
own `Bmi` trait directly against the `noahowp` crate. `sloth.rs` is the template. Selected by
`"name": "bmi_rust"` + `"model_type_name": "NoahOWP"` in the realization config, with no
`library_file`.

This is where calibration performance actually comes from:

| Cost today | Tier B |
|---|---|
| `dlopen` + 29 `dlsym` per model instance | none |
| One `extern "C"` call per variable per timestep, with a `CString` alloc and a heap buffer sized from cached `nbytes` | direct field access, no allocation |
| Fortran `character(len=*)` marshalling on every name lookup | `&str` match, or a pre-resolved variable index |
| Re-reading `MPTABLE.TBL`/`SOILPARM.TBL`/`GENPARM.TBL` from disk on every `initialize` | tables parsed once, shared `Arc` (§5.2) |
| Worker **processes** + `ipc-channel` (`src/main.rs:355-400`) because the Fortran has module-level `save` state | worker **threads**; `NoahOwp` is `Send` |

That last row is the structural one. `ParametersRead.f90` declares `save` at module scope with
~200 public `*_TABLE` globals; `bmi_noahowp.f90` keeps `input_items`/`output_items` as saved
module arrays. Two model instances in one process share that state, which is why the driver
forks workers and pays IPC for every result. A Rust model with all state owned by the instance
lets the driver drop `ipc-channel` and the `--ipc-server` worker protocol for Rust-only
realizations — less memory, no serialisation, and a far shorter path for a calibration loop that
wants to re-run one catchment thousands of times.

**Calibration-specific API** (beyond BMI, on the `noahowp` crate — not exposed through the C ABI):

```rust
impl NoahOwp {
    /// Parse config + tables once.
    pub fn from_config(path: &Path) -> Result<Self>;
    /// Reset state to t0 without re-reading anything from disk.
    pub fn reset(&mut self);
    /// Apply calibration params, re-deriving secondary parameters (see §5.3).
    pub fn set_calibration(&mut self, params: &[(CalParam, f32)]);
}
```

`reset` + `set_calibration` turns a calibration iteration from "open 4 files, parse 1300 lines of
namelist tables, allocate" into a memcpy of a few hundred bytes.

---

## 5. Configuration and parameter tables

### 5.1 Two Fortran input readers

**Correction to an earlier draft of this plan:** the four inputs are *not* all namelists. Only
two are. `SOILPARM.TBL` and `GENPARM.TBL` are read with list-directed input
(`READ (21,*) ITMP, BEXP_TABLE(LC), ...`, `src/ParametersRead.f90`), which is a different
grammar with different rules. Both readers are needed.

**Namelist** (`&group key = values /`):

| File | Groups |
|---|---|
| `namelist.input` (the BMI `init_config`) | `timing`, `parameters`, `location`, `forcing`, `model_options`, `structure`, `initial_values` |
| `MPTABLE.TBL` | `usgs_veg_categories`, `usgs_veg_parameters`, `modis_veg_categories`, `modis_veg_parameters`, `rad_parameters`, `global_parameters`, `crop_parameters`, `irrigation_parameters`, `tiledrain_parameters`, `optional_parameters` |

**List-directed** (`READ (unit,*) a, b, c`):

| File | Shape |
|---|---|
| `SOILPARM.TBL` | a class label (`STAS` / `STAS-RUC`), a category count, then one row of 18 values per soil class |
| `GENPARM.TBL` | `SLOPE_DATA` + count + values, then label/value pairs |

The list-directed semantics that matter: each `READ` statement begins at a **new record**, so
values left on the previous line are discarded; within a statement the reader continues onto
following records until the io-list is satisfied; `n*value` repeats; a null value (`,,`) leaves
its target **unchanged**; and `/` terminates a statement early. Getting the record boundary
wrong silently shifts a whole table by one column.

Note also that `SAIM_TABLE(MVT,12)` and `LAIM_TABLE` are not single namelist keys --
`ParametersRead.f90:535-546` assembles them from twelve monthly vectors (`SAI_JAN`..`SAI_DEC`).

Write a small `fortran/namelist.rs` reader — group headers `&name` / `/`, `!` comments,
`key = v1, v2, ...` with repeat syntax `n*value`, quoted strings, `.true.`/`.false.`. A few
hundred lines, no dependency. **Match gfortran's float parsing exactly** (`strtof` semantics,
correctly-rounded nearest) — Rust's `str::parse::<f32>()` is correctly rounded, so this matches,
but note that Fortran accepts `1.0d0`/`1.0e0` exponent markers that Rust's parser rejects;
normalise `d`/`D` → `e` before parsing.

**Unset-variable semantics matter.** `ParametersRead` initialises table arrays to a sentinel
before reading and the namelist read leaves absent keys untouched. Reproduce the sentinel exactly
(`-1.E36`, `src/ParametersRead.f90:377+`) — some option paths test for it.

### 5.2 Table caching

Tables depend only on `(parameter_dir, soil_class_name, veg_class_name)`. Parse once into an
`Arc<Tables>` behind a process-global cache keyed on that triple. For calibration this removes
~1300 lines of parsing per model init.

Both readers are implemented and tested against verbatim copies of the upstream files; see
`noahowp/tests/real_inputs.rs`.

Optional later step: a `build.rs` that bakes the stock `MPTABLE/SOILPARM/GENPARM` into the binary
as a fallback, so a run needs only `namelist.input`. Keep file loading as the primary path —
baking in tables would break users who tune them.

### 5.3 Secondary parameter derivation

`ParametersType::paramRead` computes derived parameters from table values plus `namelist` (e.g.
`kdt` from `refkdt`, `frzx` from `frzk`, `smcref`/`smcwlt` from `smcmax`/`bexp`/`psisat`). When a
calibration run does `set_value("REFKDT", ...)`, the Fortran BMI re-derives the dependents — but
**inline, per case**, not through a shared helper. `bmi/bmi_noahowp.f90:1286-1288`:

```fortran
case("REFKDT")
  parameters%refkdt = src(1)
  parameters%kdt    = parameters%refkdt * parameters%dksat(1) / parameters%refdk
```

duplicating `src/ParametersType.f90:281`. The same derivation is repeated again in the `int`
variant at line 1248. Port this as one `recompute_secondary(&mut Parameters)` called from exactly
the set of `set_value` names the Fortran recomputes for — **and audit which names those are**, as
the inline duplication makes it easy for upstream to update one copy and not the other. Then
differential-test it (§6.3): a mismatch here is invisible until a calibration produces different
optima.

**`set_value`/`get_value` are not unit-symmetric.** `QSEVA` is stored in m/s but read out as
`water%qseva * 1000.0` (line 1000) and written in as `src(1) * 0.001` (line 1284). Reproduce the
conversion factors *and the order of operations* exactly — `x * 0.001f32` is not `x / 1000.0f32`
in binary32. Audit every `get_value`/`set_value` case for a hidden factor; there are only two
today, but they are exactly the kind of thing a "cleanup" would silently change.

---

## 6. Verification

Bit-identity has to be mechanised or it will not hold.

### 6.1 Golden-run harness

1. Build the Fortran with `NGEN_FORCING_ACTIVE`/`NGEN_OUTPUT_ACTIVE`, `-O2 -ffp-contract=off`,
   no `-ffast-math`, pinned gfortran version (record it in `rust/PORTING.md`).
2. Write a small Rust harness that loads *both* the Fortran `.so` (via the existing `BmiFortran`
   adapter) and the Rust model, drives them with the same `set_value` forcing sequence from
   `data/bondville.dat`, and after each `update` compares **every** output variable as raw bits
   (`f32::to_bits`), not with a tolerance.
3. On the first differing timestep, dump both states. This is the core debugging loop.

Run it over the full Bondville year (17,520 half-hour steps) and over the `two_cats` test data
already in `bmi-driver/test_data/`.

### 6.2 Per-subroutine differential tests

Whole-model comparison tells you *that* something diverged, not where. For each physics module,
generate a fixture: instrument the Fortran (a `#ifdef DIFFTEST` block writing raw struct bytes
before/after each `*Main` call) over a Bondville run, dump ~200 randomly sampled states per
module, then assert the Rust function reproduces the post-state bit-for-bit from the pre-state.

This makes each ported file independently verifiable and turns the port into a checklist. It also
gives you regression tests for every future upstream port.

### 6.3 Parameter/derivation tests

Golden test over the full cross product of `(veg_class_name, veg type 1..27, soil class 1..30)`:
assert the Rust `Parameters` struct after `paramRead` is bit-identical to the Fortran's. Then the
same after each calibration `set_value`, to lock in §5.3.

### 6.4 Where identity may legitimately fail

Be prepared to accept a documented exception for transcendental intrinsics if glibc's `expf`/
`powf` cannot be matched. If so: pin the divergence to a named function list, quantify the drift
over a full year run (expect ~1 ULP growing slowly), and gate it behind a `strict-libm` feature
that links the exact libm symbols. Decide this in Phase 0 — do not discover it in Phase 4.

---

## 7. Phasing

| Phase | Work | Rough size |
|---|---|---|
| **0. Spike** | Prove bit-identity is achievable: port `AtmProcessing` + `UtilitiesModule` + `DateTimeUtilsModule`, build the §6.1 harness, measure intrinsic drift, settle the `powi`/`exp` question. **Go/no-go on "byte identical".** | 1–2 weeks |
| **1. Skeleton** | `Shifted`, namelist parser, all 8 type structs with `Init/InitDefault/InitTransfer`, `ParametersRead` + table cache, §6.3 tests green. No physics. | 2–3 weeks |
| **2. BMI surface** | `noahowp-bmi` + `c_abi.rs`, `register_bmi`, all 29 functions. Physics stubbed to `todo!()`. Driver loads it via `bmi_c` and metadata/grid/time calls match the Fortran. | 1 week |
| **3. Physics — water** | `WaterModule` tree: canopy, snow (`SnowWater*`, `SnowLayerChange`), soil (`SoilWater*`), runoff (`SurfaceRunoff*`, `SubsurfaceRunoff`), `Interception`. Per-module differential tests (§6.2). | 4–6 weeks |
| **4. Physics — energy** | `EnergyModule` tree: `EtFlux` (1.5k lines, the big one), `Albedo`, `ShortwaveRadiation`, `ThermalProperties`, `SnowSoilTemp`, `PrecipHeat`. | 5–7 weeks |
| **5. Close the loop** | `RunModule::solve_noahowp`, full-year bit-identity on Bondville + `two_cats`. | 1–2 weeks |
| **6. Tier B** | `BmiNoahOwp` native adapter in `bmi-driver`, `reset`/`set_calibration`, thread-based workers for Rust-only realizations, benchmark against the Fortran path. | 2–3 weeks |
| **7. Tracking** | `PORTING.md`, CI job that diffs `upstream/main` against the recorded commit and fails when a ported file changes upstream. | few days |

Phases 3 and 4 are independently parallelisable once Phase 2 lands (the differential fixtures
decouple them).

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| **Transcendental intrinsics don't match bit-for-bit** | Phase 0 spike decides this before the bulk of the work. Fallback: documented ULP-level exception, `strict-libm` feature. |
| `EtFluxModule` (1503 lines, nested iteration loops `NITERC=20`, `NITERG=5`, `NITERB=5`) amplifies any `f32` difference | Port it last, with the densest differential fixtures. Iterative solvers are where 1 ULP becomes 1e-3. |
| Uninitialised-variable reads in the Fortran that happen to work | `-fcheck=all -finit-real=snan` on the reference build to flush them out during Phase 0; any found become documented, deliberately-reproduced behaviour. |
| Borrow-checker friction from Fortran aliasing (same derived type as `intent(in)` and `intent(inout)` in one call) | Split at the argument list, as `RunModule` already does. If a genuine alias appears, copy the value — do not reach for `RefCell` or raw pointers. |
| Scope creep into gridded / standalone mode | Explicitly out of scope (§1.3). The 2-D grid code is commented out upstream anyway. |
| Upstream diverges mid-rewrite | Churn is ~1–2 physics commits/year. Pin to `eaa8282`, rebase at the end of Phase 5. |

---

## 9. Open questions

1. **Does `bmi-driver` need the C ABI tier at all long-term**, or is Tier B enough? Tier A is
   worth building regardless as the differential-test oracle and as an escape hatch for `ngen`,
   but it need not be the shipping path.
2. **Where should the Rust crates live** — this repo under `rust/` (best for tracking upstream
   diffs, the assumption above), or a separate `noah-owp-rust` repo (cleaner Cargo story)? A
   separate repo with the Fortran vendored as a submodule is the third option; it makes the
   per-file port table harder to keep honest.
3. **Multi-cell instances.** Nothing in scope needs them, but if calibration wants many
   catchments in one process, `NoahOwp` being `Send + 'static` with no globals is the enabling
   property — worth protecting in CI (`fn assert_send<T: Send>()`) even before it is used.
