# Handoff

Written 2026-09-18, from a Claude Code web session with **no `gfortran`** available. That
constraint shaped what got built: everything here is verifiable without a Fortran toolchain, and
the work that needs one is set up ready to run rather than left as prose.

Read in this order: this file, then `PORTING.md` (per-file status), then
`docs/RUST_REWRITE_PLAN.md` (the full design).

---

## 1. What exists

Five commits on `main`. **125 tests, clippy clean, release cdylib builds.**

```sh
cargo test                  # 125
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

### Step 1 -- run the Phase 0 spike (one command, ~1 minute)

```sh
cd spike/intrinsics && ./run.sh
```

This is the **go/no-go on strict bit-identity** and it is the single highest-value thing you can
do first, because it is the only open question that no amount of careful translation can fix
later. Read `spike/intrinsics/README.md` for what it does and how to read the result.

It answers "which Rust spelling of each intrinsic matches gfortran bit for bit", by having the
Rust probe emit every candidate and reporting which agrees. Exit 0 means all clear and the
output names the winners.

Two things are already known without gfortran, from candidate-vs-candidate spread:

- **Widening to f64 is not a safe substitute** for f32 libm: 1 ULP apart on ~0.1-1% of inputs
  for `exp`/`log`/`sin`/`cos`/`powf`, 2 ULP on ~6% for `tanh`. `sqrt` is exact either way.
- **Integer exponents are the real hazard.** For `x ** 7` the candidates disagree with each
  other by up to 4 ULP; `powi` disagrees with repeated multiplication on 59% of inputs. For
  `x ** 3`, `powf` differs from `powi`/`mul` on 25%. Whichever gfortran uses, the others are
  wrong by well above the noise floor.

Whatever the verdict, **record the winning candidate per intrinsic in a new
`noahowp/src/fortran/intrinsics.rs`** and have ported physics call through it. A bare
`f32::exp()` in `noahowp/src/physics/` should then be a CI-greppable bug.

If some intrinsic has no exact match: don't abandon the goal, take
`docs/RUST_REWRITE_PLAN.md` §6.4 -- pin the divergence to a named list, measure the drift over a
full Bondville year, and decide whether it is acceptable for calibration. It probably is. What
matters is measuring rather than assuming.

### Step 2 -- build the reference Fortran

Pin the compiler version and record it in `PORTING.md`. Flags that matter:

```
-O2 -ffp-contract=off       # no fused multiply-add; never -ffast-math
-cpp -DNGEN_FORCING_ACTIVE -DNGEN_OUTPUT_ACTIVE
```

Those two defines are what put the model on the BMI path: they compile out the ASCII forcing
reader and the NetCDF output module, which is the whole reason this port has no external
dependencies.

**A trap worth knowing before you start:** `noah-owp-modular` has no CMake of its own. Its
`Makefile` produces `.o` files, and `libsurfacebmi.so` is built by a CMake wrapper that lives in
**ngen's** `extern/noah-owp-modular/` (see `.github/workflows/ngen_integration.yaml`), together
with the `iso_c_fortran_bmi` middleware. So getting a loadable Fortran `.so` means pulling in
ngen, or writing a small standalone `CMakeLists.txt`.

**You probably don't need the `.so` at all.** For per-module differential testing (§6.2 of the
plan), skip BMI entirely: write a short Fortran driver that calls the physics subroutines
directly and dumps the derived types as raw bytes. That avoids ngen, the middleware and the
whole C ABI question, and it is what actually catches translation bugs. Build the `.so` later,
only when you want the end-to-end `bmi-driver` comparison.

### Step 3 -- the differential fixture generator

This is the tool the whole physics port depends on, so build it before porting physics.

1. Add a `#ifdef DIFFTEST` block to the Fortran that, on each call to a `*Main` subroutine,
   writes the raw bytes of the derived types before and after.
2. Run a Bondville year, sampling ~200 states per module.
3. Commit the fixtures. Each ported Rust function then has to reproduce the post-state
   bit-for-bit from the pre-state.

This turns the port into a checklist and gives you a permanent regression suite for every future
upstream port. Do it once, properly.

### Step 4 -- then the port, in this order

`PORTING.md` has the full table. The order that keeps things verifiable:

1. `ParametersRead` + `ParametersType` -- the readers are done, so this is table assembly plus
   secondary-parameter derivation. Test against the Fortran over the full cross product of
   `(veg_class_name, veg type 1..27, soil class 1..30)`; §6.3 of the plan.
2. `ForcingType`, `EnergyType`, `WaterType` -- plain data, mechanical.
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
