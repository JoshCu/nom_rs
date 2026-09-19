# Handoff

Written 2026-09-18, from a Claude Code web session with **no `gfortran`** available. That
constraint shaped what got built: everything here is verifiable without a Fortran toolchain, and
the work that needs one is set up ready to run rather than left as prose.

Read in this order: this file, then `PORTING.md` (per-file status), then
`docs/RUST_REWRITE_PLAN.md` (the full design).

> **Update, same day, with gfortran 15.2.0 in hand: steps 1 to 3 below are done.**
> The intrinsic spike says **GO** -- every intrinsic the model uses has a bit-identical Rust
> spelling, recorded in `noahowp/src/fortran/intrinsics.rs`. The reference Fortran builds with
> no ngen, no CMake and no NetCDF, and the differential fixture generator exists: a sampled
> Bondville year is recorded before and after each of the five physics calls, committed, and
> validated by `noahowp/tests/difftest_fixtures.rs`. `gfortran` is no longer needed to build or
> test this repo, only to regenerate fixtures. **The next action is step 4** -- the port itself,
> starting with `ParametersRead` + `ParametersType`.

---

## 1. What exists

Nine commits on `main`. **157 tests, clippy clean in debug and release, release cdylib builds.**

```sh
cargo test                  # 157, and again with --release
cargo clippy --all-targets  # 0 warnings
cargo build --release       # target/release/libnoahowp_bmi.so
```

### Done and tested

| Area | Files | Notes |
|---|---|---|
| Fortran-index arrays | `noahowp/src/layers.rs` | `Shifted<T>` -- ported loops index `(-nsnow+1 : nsoil)` with the Fortran's own `i32` indices |
| Fortran value parsing | `noahowp/src/fortran/value.rs` | the `d`/`D` exponent marker and sign-only exponent form Rust's parser rejects |
| List-directed input | `noahowp/src/fortran/list_directed.rs` | `READ(unit,*)` for `SOILPARM.TBL`, `GENPARM.TBL` |
| Namelist input | `noahowp/src/fortran/namelist.rs` | `&group ... /` for `namelist.input`, `MPTABLE.TBL` |
| Config chain | `namelist_read.rs`, `levels.rs`, `options.rs`, `domain.rs` | `NamelistRead` + the three `*Type` modules it feeds |
| Constants | `constants.rs` | derived constants keep their defining expressions |
| Date handling | `date_time_utils.rs` | `parse_date`, `julian_date`, `calendar_date`, `date_to_unix`, `unix_to_date`, `get_utime_list` |
| BMI metadata surface | `noahowp-bmi/src/lib.rs`, `vars.rs` | names, counts, types, units, grids, itemsize, nbytes, location, all time functions, `initialize`, `finalize` |
| Bit-identical intrinsics | `noahowp/src/fortran/intrinsics.rs` | the spellings of `EXP`/`LOG`/`**`/... gfortran matches, pinned by gfortran's own answers as test vectors |
| Reference Fortran + fixtures | `reference/` | builds upstream at the pin and records the state around all five physics calls over a Bondville year |
| Fixture reader | `noahowp/src/difftest/` | `manifest.rs` generated from the Fortran derived types; `State::diff` names the fields that disagree |

Both input readers are tested against **verbatim copies** of the upstream `MPTABLE.TBL`,
`SOILPARM.TBL`, `GENPARM.TBL` and `namelist.input` in `noahowp/tests/fixtures/`, not against
hand-written approximations. `noahowp/tests/real_inputs.rs` is the file to look at first if you
want to see what is actually proven.

### Stubs -- surface exists, body outstanding

- `BmiNoahOwp::update` -> `BmiError::NotImplemented` (the physics column)
- `get_value_*` / `set_value_*` -> `NotImplemented`, but they **do** still reject unknown
  variable names as unknown, so the metadata contract is honest
- `c_abi.rs` (`register_bmi`, the C ABI) -- not started

### Not started

`ParametersRead`, `ParametersType`, `ForcingType`, `EnergyType`, `WaterType`,
`UtilitiesModule`, `RunModule`, and all 21 physics modules. See `PORTING.md`.

---

## 2. Start here, with gfortran in hand

### Step 1 -- the Phase 0 spike: done, verdict GO

```sh
cd spike/intrinsics && ./run.sh     # ~1 minute; COUNT=200000 ./run.sh for the wide sweep
```

Run against GNU Fortran 15.2.0 and rustc 1.98.1 at 20k and 200k inputs per intrinsic: **every
intrinsic the model uses has a bit-identical Rust candidate**, at `-O` and at `-O0`. Strict
bit-identity is achievable and the plan's §6.4 fallback is not needed. The winners are in
`noahowp/src/fortran/intrinsics.rs`; ported physics calls through it, and a bare `f32::exp()`
in `noahowp/src/physics/` is a CI-greppable bug.

The rule turned out to be short: **`f32`'s own method for the transcendentals, `powi` for every
integer exponent, `powf` for real ones.** Four things the run settled that the no-gfortran
guesswork had not:

- **The literal exponents in the model are 0..4, not 7.** `**7` never appears; `**2` (20
  sites), `**3` (13) and `**4` (11) do, plus `x ** ifrc` and `x ** (CVFRZ - J)` where the
  exponent is a runtime variable. The spike now probes exactly that set, with negative bases.
- **`x * x * x * x` is wrong for `x ** 4`** -- gfortran expands by squaring, so left-to-right
  multiplication disagrees on 34% of inputs by up to 2 ULP. This is the one that would have
  been written by instinct and silently shifted results. `powi` matched every exponent probed,
  literal and runtime alike, so no per-exponent judgement is needed.
- **`powf` for an integer exponent looks correct in release and is not.** LLVM rewrites
  `x.powf(2.0)` into `x * x`, so an optimised build hides the 1-ULP disagreement that a debug
  build -- how `cargo test` runs -- would show. `run.sh` now builds the Rust probe at both
  levels and flags any candidate that wins at only one.
- **Constant folding is a trap for tests, not for the port.** Both compilers evaluate a
  compile-time-constant argument in higher precision than the libm they would otherwise call:
  `exp(-29.5739784)` folds to `2A215188` where glibc returns `2A215189`. gfortran folds to the
  same value LLVM does, so ported code agrees either way -- but a test comparing a folded Rust
  value against a probe value computed at runtime fails. `intrinsics.rs` puts its inputs behind
  `black_box`, and pins both values so the reasoning cannot rot.

Widening to f64 is confirmed unsafe, as suspected: 1 ULP apart on 0.07% of `exp` inputs and
1.4% of `sin`. `sqrt` and `tanh` are exact either way, but there is no reason to special-case
them.

### Steps 2 and 3 -- reference build and differential fixtures: done

```sh
cd reference && ./build.sh --fixtures     # ~1 minute; the Bondville year itself takes 0.14 s
```

The advice to skip BMI was right, and it paid better than expected. **No ngen, no CMake, no
NetCDF, and no `.so`.** `-DNGEN_OUTPUT_ACTIVE` compiles out `OutputModule` -- the only NetCDF
user -- while leaving `NGEN_FORCING_ACTIVE` *undefined* keeps the ASCII forcing reader, so a
plain `program` driving `initialize_from_file` + `advance_in_time` runs the full Bondville year
from `data/bondville.dat`. `reference/README.md` has the details.

`solve_noahowp` turned out to call exactly five subroutines -- `UtilitiesMain`, `ForcingMain`,
`InterceptionMain`, `EnergyMain`, `WaterMain` -- which is a small enough surface that the whole
physics port is now a checklist of five entry points.

Three things worth knowing if you touch it:

- **The serializer is generated, not written.** `gen_serializer.py` parses the derived types and
  emits both the Fortran writer and `noahowp/src/difftest/manifest.rs`, so a field added
  upstream appears on both sides at once. Hand-writing ~500 field accesses across seven types
  and keeping them in step with upstream was never going to survive. The parser refuses to skip
  a declaration it cannot understand, which caught a real bug in its own first draft: the
  attribute-list regex rejected `dimension(:)` and silently dropped every allocatable field.
  Silent is the dangerous part -- a missing field shifts every field after it, and the damage
  would have surfaced as unrelated physics appearing to be wrong.
- **The upstream `src/Makefile` OBJS list is a link order, not a compile order.** `DomainType`
  uses `DateTimeUtilsModule` and comes before it. `compile_order.py` derives the order from the
  `use` graph instead.
- **The upstream checkout is never written to.** Sources come out of the pinned commit via
  `git archive`, and the instrumentation is applied to the build copy. `instrument.py` requires
  each of the five call sites to appear exactly once, so an upstream rename stops the build
  rather than quietly mislabelling a tag.

Fixtures are committed (~3 MB, 200 sampled timesteps: the first 24 consecutively for spin-up,
then spread across the year so the snow physics is actually reached).
`noahowp/tests/difftest_fixtures.rs` validates the recording against the Bondville namelist and
checks that a truncated, corrupted or misaligned fixture is rejected rather than read into
plausible nonsense.

### Step 4 -- then the port, in this order

`PORTING.md` has the full table. The order that keeps things verifiable:

1. ~~`ParametersRead` + `ParametersType`~~ -- **done.** All 134 fields agree with the Fortran
   bit for bit, over Bondville and a 130-case sweep of every table row under both vegetation
   classifications (`reference/paramsweep_driver.f90`). Only the four readers `paramRead`
   actually calls are ported; the crop, irrigation, tiledrain and optional readers are
   unreachable from the BMI path. The sweep earns its keep: writing `kdt` as
   `refkdt * (dksat(1) / refdk)` instead of `refkdt * dksat(1) / refdk` is 1 ULP out in 16
   sweep cases and in none of the Bondville ones.
2. ~~`ForcingType`, `EnergyType`, `WaterType`~~ -- **done**, together with
   `RunModule::initialize_from_file`. The whole initial state -- `domain`, `forcing`, `energy`,
   `water`, 220-odd fields -- matches the Fortran bit for bit at the first timestep
   (`tests/initial_state_vs_fortran.rs`). Three groups are excluded and each is scope, not
   oversight: the eight forcings the ASCII reader supplies (they arrive via `set_value`),
   `domain%curr_datetime` (assigned at the top of `solve_noahowp`), and `domain%sim_datetimes`
   (omitted from the fixtures as one f64 per timestep).
3. `get_value` / `set_value` -- now unblocked. Watch the two traps in §4 below.
4. `c_abi.rs` + `register_bmi` -- `bmi-driver` can then load it via its **`bmi_c`** adapter
   (not `bmi_fortran`: no middleware needed, and `BmiFortran` requires all 29 symbols to
   resolve).
5. `UtilitiesModule`, `RunModule`.
6. Physics: water tree, then energy tree. `EtFluxModule` last -- 1503 lines with nested
   iteration loops, where any upstream ULP difference gets amplified.

Phases for water and energy are independent once the fixtures exist, so they can go in parallel.

---

## 3. Decisions already made

- **Two crates.** `noahowp` (pure model, zero dependencies) and `noahowp-bmi` (the BMI surface,
  `rlib` + `cdylib`). Nothing in the model crate needs NetCDF or a C toolchain.
- **Free functions over borrowed structs**, not methods on a god object. `Init` / `InitDefault`
  / `InitTransfer` are preserved as `new` / `Default::default` / `init_transfer` so the mapping
  to the Fortran stays one-to-one.
- **`Shifted<T>` for every profile array**, including 1-based soil arrays, so no ported loop
  ever has to reason about which indexing convention an array follows.
- **Typed errors, never `stop`.** Every Fortran `write(*,*) + stop` became a `ConfigError` or
  `BmiError` variant, because a calibration driver needs to survive a bad config.
- **Three clippy lints disabled crate-wide**, each with the reason in the source:
  `neg_multiply`, `excessive_precision`, `approx_constant`. All three push toward rewriting
  float literals or expressions. See §4.3.
- **`BmiNoahOwp` holds no globals** and is `Send` (asserted in a test). This is the property that
  lets `bmi-driver` eventually drop its subprocess + `ipc-channel` worker protocol for
  Rust-only realizations -- see §5.

---

## 4. Traps found in the Fortran

All seven are catalogued in `PORTING.md` with the source location. Each is pinned by a test that
fails if someone "tidies" it. The four that cost real time to find:

### 4.1 `julian_date` and `calendar_date` are one day apart

`calendar_date(julian_date(d,m,y) + 1) == (d,m,y)`. `julian_date(1,1,1970)` returns **2440587**,
where the astronomical JDN is 2440588. `unix_to_date` is written around the offset (it adds 1 to
`i_day`); `date_to_unix` differences two calls so it cancels. Neither routine should be
"corrected". My first test asserted the astronomical value and failed -- the code was right, the
expectation wasn't.

### 4.2 `huge(1)` assigned to `real*8`

`DomainType::InitDefault` assigns `huge(1)` -- the **integer** huge -- to `start_datetime`,
`end_datetime` and `curr_datetime`, which are `real*8`. They default to `2147483647.0`, not
`f64::MAX`. `time_dbl` uses `huge(1.d0)` and really is `f64::MAX`.

### 4.3 Float literals must be copied digit for digit

I wrote `TLC = 2.0 * 0.7039725` where the Fortran has `0.703972477`, and landed one ULP away.
Caught by my own test within an hour of writing the plan section warning about it. Both the
`excessive_precision` and `approx_constant` clippy lints actively suggest this error, which is
why they are off. `DEGRAD` uses the literal `3.1415926`, not a pi constant -- it happens to
collapse onto `PI/180` in f32, but the literal stays.

### 4.4 `set_value` / `get_value` are not unit-symmetric, and duplicate derivations

Two things to reproduce exactly when you get to §2 step 3:

- `QSEVA` reads out as `water%qseva * 1000.0` (`bmi_noahowp.f90:1000`) and writes in as
  `src(1) * 0.001` (line 1284). Keep the factors **and** the operation order --
  `x * 0.001f32` is not `x / 1000.0f32` in binary32.
- Secondary parameter derivation (`kdt` from `refkdt`) is duplicated **inline in three places**:
  `ParametersType.f90:281`, and again at `bmi_noahowp.f90:1248` and `:1288`. Port it as one
  `recompute_secondary()` and **audit which `set_value` names recompute**, since upstream can
  update one copy and not the others. A mismatch here is invisible until a calibration lands on
  different optima.

### Also worth knowing

- `SOILPARM.TBL` and `GENPARM.TBL` are **list-directed**, not namelists. An earlier draft of the
  plan got this wrong. Each `READ` statement starts at a new record and leftovers on the
  previous line are discarded -- get that boundary wrong and a table silently shifts by a column.
- `SAIM_TABLE(MVT,12)` / `LAIM_TABLE` are not single namelist keys; `ParametersRead.f90:535-546`
  assembles them from twelve monthly vectors `SAI_JAN`..`SAI_DEC`.
- Unset table entries are `-1.E36` (`ParametersRead.f90:377+`). The namelist reader leaves absent
  keys untouched so those sentinels survive; some option paths test for them.
- `var_location` returns `"node"` for **every** name, including unknown ones, and always reports
  success.

---

## 5. Scope, confirmed

Your instinct on gridded code was right. `bmi_noahowp.f90:326-423` defines only grid 0 (scalar),
1 (`nsnow` vector) and 2 (`nsoil` vector); the 2-D / uniform-rectilinear cases are commented out
upstream. And `bmi-driver` never resolves `get_grid_shape`/`spacing`/`origin`/`x`/`y`/`z`, the
node/edge/face counts, `get_value_ptr`, or `*_at_indices`. All droppable, all dropped.

The whole `driver/` directory falls away on the BMI path, including `OutputModule.f90` -- the
only NetCDF user in the codebase.

**Where the calibration speedup actually comes from.** `ParametersRead.f90` keeps ~200
`*_TABLE` arrays in module-level `save` globals, so two Fortran instances in one process share
them. That is why `bmi-driver` forks workers and pays IPC for every result
(`bmi-driver/src/main.rs:355-400`). Instance-owned state in Rust makes the model `Send`, so for
Rust-only realizations the driver can use threads and drop the serialization entirely. That is a
bigger win than removing the FFI overhead, and it is worth protecting in CI before anything
depends on it (there is already an `assert_send` in `noahowp-bmi/src/lib.rs`).

Beyond BMI, the calibration API to add on the `noahowp` crate (plan §4):

```rust
impl NoahOwp {
    pub fn from_config(path: &Path) -> Result<Self>;   // parse config + tables once
    pub fn reset(&mut self);                           // back to t0, no disk I/O
    pub fn set_calibration(&mut self, params: &[(CalParam, f32)]);  // + recompute_secondary
}
```

With a table cache keyed on `(parameter_dir, soil_class_name, veg_class_name)`, a calibration
iteration goes from "open 4 files, parse 1300 lines, allocate" to a memcpy of a few hundred
bytes.

---

## 6. Open questions for you

1. **Is the C ABI tier worth maintaining long-term**, or only as the differential-test oracle?
   Tier A (cdylib + `register_bmi`) is worth building either way as the comparison harness and
   as an ngen escape hatch, but the shipping path for calibration is the native in-process
   adapter.
2. **Should the Rust crates move into `noah-owp-modular` under `rust/`?** The plan assumed that,
   for tighter upstream diff tracking; they ended up in `nom_rs` because pushes to
   `noah-owp-modular` were blocked (Claude's GitHub App isn't installed there). A separate repo
   is a cleaner Cargo story but makes the per-file port table easier to let rot. Your call.
3. **Multi-cell instances.** Nothing in scope needs them, but `NoahOwp` being `Send + 'static`
   with no globals is the enabling property if calibration ever wants many catchments per
   process.

---

## 7. Also unpushed

There is an identical copy of `docs/RUST_REWRITE_PLAN.md` committed on branch
`claude/noah-owp-rust-rewrite-wdv3di` in a local clone of `noah-owp-modular`, which could not be
pushed (403 -- the Claude GitHub App is not installed on that repo). Nothing is lost: this repo
has the same content plus everything since. Ignore it unless you decide on question 6.2 above.
