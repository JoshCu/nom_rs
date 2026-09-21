# Porting status

Upstream: [`NOAA-OWP/noah-owp-modular`](https://github.com/NOAA-OWP/noah-owp-modular)
**pinned at `0ff055e`** ("Add OPT_BTR=4 and OPT_RSF=5 to approximate PET ...", #125).

The pin moved from `eaa8282` once a Fortran toolchain was available, because `0ff055e` is what
the `libsurfacebmi.so` in use at `/dmod/shared_libs/` is built from, and the reference build
should match what is actually being run. The cost was small: the four files that changed are
additive, every new branch is gated behind `OPT_BTR == 4` or `OPT_RSF == 5`, and no existing
option path differs. Only the namelist bounds needed porting (`stomatal_resistance_option` to
1-4, `evap_srfc_resistance_option` to 1-5). The two new option bodies land with
`EnergyModule`/`EtFluxModule`.

## Reference toolchain

Bit-identity is a claim about a specific pair of compilers, so both are pinned here. Changing
either means re-running `spike/intrinsics/run.sh`.

| | Version | Flags |
|---|---|---|
| Reference Fortran | GNU Fortran 15.2.0 (Ubuntu 15.2.0-16ubuntu1) | `-O2 -ffp-contract=off -cpp -DNGEN_OUTPUT_ACTIVE`; never `-ffast-math` |
| Rust | rustc 1.98.1 | stock `dev` and `release` profiles; the intrinsic verdict holds at both |

The intrinsic spike was run against these on 2026-09-18 and returned **GO**: every intrinsic
the model uses has a bit-identical Rust spelling. See `spike/intrinsics/README.md` for the
verdict and `noahowp/src/fortran/intrinsics.rs` for the spellings ported code must use.

`NGEN_FORCING_ACTIVE` is deliberately *not* defined for the reference build: leaving the ASCII
forcing reader in is what lets it run the Bondville year standalone, with no ngen and no NetCDF.
The shipping BMI path still compiles it out.

**Bit-identity is a claim about a (compiler, libc) pair.** The `libsurfacebmi.so` at
`/dmod/shared_libs/` was built with GCC 11.5.0 on Red Hat, not the GCC 15.2.0 above. Nothing the
spike measured varies with compiler version, but that was one pair; if the calibration target is
the deployed library, regenerate the fixtures with `FC=gfortran-11` and diff them.

## How to track upstream

```sh
git -C /path/to/noah-owp-modular fetch origin
git -C /path/to/noah-owp-modular log --oneline eaa8282..origin/main -- src/ bmi/
```

Any file that appears there and is marked **ported** below needs its Rust counterpart
revisited. Churn is low: over the three years to the pin, only `SurfaceRunoffInfiltration.f90`,
`EnergyModule.f90`, `EtFluxModule.f90`, `AtmProcessing.f90`, `SnowWaterModule.f90`,
`WaterModule.f90` and a few type modules changed at all, 1-2 commits each.

Every ported function carries a `// Port of <file>::<SUBROUTINE> @ <commit>` header. When you
port a change, bump the commit in that header and in the table below.

## Status

Legend: **done** -- ported and tested · **stub** -- surface exists, body outstanding ·
**todo** -- not started · **n/a** -- deliberately out of scope.

### Configuration and support

| Upstream | Rust | Status |
|---|---|---|
| `src/ConstantsModule.f90` | `noahowp/src/constants.rs` | done |
| `src/LevelsType.f90` | `noahowp/src/levels.rs` | done |
| `src/DomainType.f90` | `noahowp/src/domain.rs` | done |
| `src/OptionsType.f90` | `noahowp/src/options.rs` | done |
| `src/NamelistRead.f90` | `noahowp/src/namelist_read.rs` | done |
| `src/ErrorCheckModule.f90` | `noahowp/src/error_check.rs` | done |
| `src/DateTimeUtilsModule.f90` | `noahowp/src/date_time_utils.rs` | done (date routines only) |
| `src/ParametersRead.f90` | `noahowp/src/parameters_read.rs` | done (the four readers `paramRead` calls) |
| `src/ParametersType.f90` | `noahowp/src/parameters.rs` | done |
| `src/ForcingType.f90` | `noahowp/src/forcing.rs` | done |
| `src/EnergyType.f90` | `noahowp/src/energy.rs` | done |
| `src/WaterType.f90` | `noahowp/src/water.rs` | done |
| `src/UtilitiesModule.f90` | `noahowp/src/utilities.rs` | done (the shapes `UtilitiesMain` calls) |
| `src/RunModule.f90` | `noahowp/src/run.rs` | partial -- `initialize_from_file`; the timestep loop awaits the physics |

`UtilitiesModule.f90`'s `geth_newdate` and `geth_idts` handle punctuated and unpunctuated
dates at six resolutions with fractional seconds. `UtilitiesMain` calls each exactly one way --
a 12-character `YYYYMMDDHHMM` date with an offset in minutes, and two 10-character `YYYY-MM-DD`
dates -- so only those shapes are ported. Anything else is a typed error; upstream reaches
`call abort()` on most of them. The unported branches have no caller and no way to test them.

`ParametersRead.f90` also contains `read_crop_parameters`, `read_irrigation_parameters`,
`read_tiledrain_parameters` and `read_optional_parameters` -- roughly 450 lines. `paramRead`
calls none of them, and nothing else can reach their tables, so they are out of scope
(`crop_model_option = 0` is the only supported value). They become reachable only if upstream
starts transferring those tables into `parameters_type`.

`DateTimeUtilsModule.f90` also vendors a general string-utility library (`parse`, `compact`,
`removesp`, `shiftstr`, `insertstr`, `delsubstr`, `uppercase`, `readline`, `match`, `write_*`,
`writeq_*`, `trimzero`, `split`, `removebksl`). None of it is reachable from the BMI path;
only `parse_date`, `julian_date`, `calendar_date`, `date_to_unix`, `unix_to_date` and
`get_utime_list` are ported.

### Physics

| Upstream | Rust | Status |
|---|---|---|
| `src/ForcingModule.f90` | `noahowp/src/physics/forcing_main.rs` | done |
| `src/AtmProcessing.f90` | `noahowp/src/physics/atm_processing.rs` | done |
| `src/InterceptionModule.f90` | `physics/interception.rs` | todo |
| `src/EnergyModule.f90` | `physics/energy_main.rs` | todo |
| `src/EtFluxModule.f90` | `physics/et_flux.rs` | todo |
| `src/AlbedoModule.f90` | `physics/albedo.rs` | todo |
| `src/ShortwaveRadiationModule.f90` | `physics/shortwave_radiation.rs` | todo |
| `src/ThermalPropertiesModule.f90` | `physics/thermal_properties.rs` | todo |
| `src/SnowSoilTempModule.f90` | `physics/snow_soil_temp.rs` | todo |
| `src/PrecipHeatModule.f90` | `physics/precip_heat.rs` | todo |
| `src/WaterModule.f90` | `physics/water_main.rs` | todo |
| `src/CanopyWaterModule.f90` | `physics/canopy_water.rs` | todo |
| `src/SnowWaterModule.f90` | `physics/snow_water.rs` | todo |
| `src/SnowWaterRenew.f90` | `physics/snow_water_renew.rs` | todo |
| `src/SnowLayerChange.f90` | `physics/snow_layer_change.rs` | todo |
| `src/SoilWaterModule.f90` | `physics/soil_water.rs` | todo |
| `src/SoilWaterMovement.f90` | `physics/soil_water_movement.rs` | todo |
| `src/SoilWaterRetentionCoeff.f90` | `physics/soil_water_retention_coeff.rs` | todo |
| `src/SurfaceRunoffModule.f90` | `physics/surface_runoff.rs` | todo |
| `src/SurfaceRunoffInfiltration.f90` | `physics/surface_runoff_infiltration.rs` | todo |
| `src/SubsurfaceRunoffModule.f90` | `physics/subsurface_runoff.rs` | todo |

`ForcingModule.f90` and `AtmProcessing.f90` are verified twice over: against the Bondville year
(`tests/forcing_vs_fortran.rs`) and against a 574-case sweep of all seven `OPT_SNF` branches
(`tests/atm_sweep_vs_fortran.rs`). Bondville runs one option, so the sweep is where six of the
seven phase schemes, the wet-bulb calculation and the direct-irradiance clamp are exercised at
all -- see `reference/README.md`.

Two things in `ATM` are deliberately *not* pinned by a test, because the Fortran itself cannot
observe them. The wet-bulb temperature under `OPT_SNF == 6` and the snow probability under
`OPT_SNF == 7` feed nothing but a `>=` comparison whose result is 0 or 1, so a ULP-level
difference in either is invisible unless the value lands exactly on the threshold. They are
still ported through the measured spellings -- but a mutation to them survives both fixtures,
and that is a property of the model rather than a gap in the fixtures.


### BMI

| Upstream | Rust | Status |
|---|---|---|
| `bmi/bmi_noahowp.f90` (metadata, grids, time) | `noahowp-bmi/src/lib.rs`, `vars.rs` | done |
| `bmi/bmi_noahowp.f90` (`get_value`/`set_value`) | `noahowp-bmi/src/lib.rs` | stub -- awaits the state types |
| `bmi/bmi_noahowp.f90` (`update`) | `noahowp-bmi/src/lib.rs` | stub -- awaits the physics column |
| `bmi/bmi_noahowp.f90` (`register_bmi`, C ABI) | `noahowp-bmi/src/c_abi.rs` | todo |
| `bmi/bmi.f90` | -- | n/a (abstract type; Rust uses a trait) |

### Out of scope

| Upstream | Why |
|---|---|
| `driver/NoahModularDriver.f90` | standalone mode; the driver is `bmi-driver` |
| `driver/AsciiReadModule.f90` | `#ifndef NGEN_FORCING_ACTIVE` only -- forcing arrives via `set_value` |
| `driver/OutputModule.f90` | `#ifndef NGEN_OUTPUT_ACTIVE` only -- and the sole NetCDF dependency |
| gridded code paths | commented out upstream; `bmi-driver` never calls the grid functions involved |

## Shims with no upstream counterpart

| Rust | Why it exists |
|---|---|
| `noahowp/src/layers.rs` | `Shifted<T>`, so ported loops index `(-nsnow+1 : nsoil)` arrays with the Fortran's own indices |
| `noahowp/src/fortran/value.rs` | Fortran real literal syntax (`1.0d0`, `1.0+6`) that Rust's parser rejects |
| `noahowp/src/fortran/namelist.rs` | `&group ... /` reader for `namelist.input` and `MPTABLE.TBL` |
| `noahowp/src/fortran/list_directed.rs` | `READ(unit,*)` semantics for `SOILPARM.TBL` and `GENPARM.TBL` |
| `noahowp/src/fortran/intrinsics.rs` | the Rust spellings of `EXP`/`LOG`/`**`/... that gfortran matches bit for bit |
| `noahowp/src/difftest/` | reader for the differential fixtures; `manifest.rs` is generated from the Fortran derived types |
| `reference/` | builds the reference Fortran and records the per-call fixtures the physics port is verified against |

## Upstream behaviours that are deliberately preserved

These look like bugs and are not to be "fixed" -- each is covered by a test that will fail if
someone tidies it up.

1. **`julian_date` and `calendar_date` are one day apart.**
   `calendar_date(julian_date(d,m,y) + 1) == (d,m,y)`. `unix_to_date` is written around the
   offset; `date_to_unix` differences two calls so it cancels. (`date_time_utils.rs`)

2. **`DomainType::InitDefault` assigns `huge(1)` -- the *integer* huge -- to `real*8` fields.**
   They default to `2147483647.0`, not `f64::MAX`. (`domain.rs`)

3. **Float literals are copied digit for digit.** `TLC = 2.*0.703972477`; writing `0.7039725`
   lands one ULP away. The `excessive_precision` and `approx_constant` clippy lints are
   disabled crate-wide for this reason. (`constants.rs`)

4. **`DEGRAD` uses the literal `3.1415926`, not a pi constant.** It happens to collapse onto
   `PI/180` in f32, but the literal stays. (`constants.rs`)

5. **`noahowp_var_location` returns `"node"` for every name, including unknown ones**, and
   always reports success. (`noahowp-bmi/src/vars.rs`)

6. **`set_value`/`get_value` are not unit-symmetric.** `QSEVA` reads out as `qseva * 1000.0`
   and writes in as `src * 0.001`. Reproduce the factors *and* the operation order.
   (outstanding -- lands with `get_value`/`set_value`)

7. **Secondary parameter derivation is duplicated inline** in `set_value`, not shared with
   `ParametersType::paramRead`. Upstream can update one copy and not the other.
   (outstanding -- lands with `parameters.rs`)

8. **`NROOT_TABLE` is `real`; `parameters%NROOT` is `integer`.** The assignment truncates
   toward zero, which `as i32` reproduces. (`parameters.rs`)

9. **`paramRead` overwrites `TOPT` with `1.E-06` after reading `TOPT_TABLE`.** The table value
   is read from `MPTABLE.TBL` and discarded, and `1.E-06` is not a plausible "optimum
   transpiration air temperature [K]". It is reproduced as written. (`parameters.rs`)

10. **`SHDMAX` is assigned from `SHDFAC_TABLE`**, the same table entry as `SHDFAC`, so the
    annual maximum and the current value are always equal. (`parameters.rs`)

11. **A class index between the rows supplied and the declared table size is legal** and reads
    back the `-1.E36` sentinel; `kdt` and `frzx` then go infinite. The port reproduces this
    rather than rejecting the index, and `every_table_row_matches_the_fortran` covers it.
    (`parameters.rs`)

12. **`energy%TGV` is initialised by nothing.** Neither `EnergyType::InitDefault` nor
    `RunModule` assigns it, so the Fortran reads whatever the allocation left there until the
    physics first writes it. The reference build leaves it zero -- fresh pages -- and the
    fixtures record that, so the port uses zero. This is the one value that depends on the
    compiler rather than on the source. (`energy.rs`)

13. **`forcing%SWDOWN` is skipped by `InitDefault` and set by `RunModule` instead.** It keeps
    `huge(1.0)` in between, which nothing observes. (`forcing.rs`)

14. **The eight forcings the ASCII reader supplies keep `huge(1.0)` through initialisation.**
    Their `RunModule` assignments are commented out upstream. On the BMI path they arrive via
    `set_value` before the first `update`. (`run.rs`)

15. **`calc_declin` has its own `DEGRAD`, and it is not the one in `ConstantsModule`.**
    The local parameter is `3.14159265/180.`; the module constant uses the literal `3.1415926`,
    one digit shorter. Two constants, same name, different value. (`utilities.rs`)

16. **`nfeb` has a 3600-year rule**, which is not part of the Gregorian calendar. It changes
    nothing before the year 3600 and is reproduced rather than corrected. `calc_declin`
    open-codes the same rule a second time for `yearlen`. (`utilities.rs`)

17. **`idt = itime * (domain%dt / 60)` divides in real and truncates on assignment.**
    Computing `itime * dt / 60` in integers rounds differently for any `dt` that is not a whole
    number of minutes. (`utilities.rs`)

18. **Integer powers go through `powi`, never through repeated multiplication.** gfortran
   expands `x ** n` by squaring, so `x * x * x * x` is a different computation from `x ** 4`
   and disagrees on 34% of inputs. `powf` is also not a substitute, and looks correct in
   release builds only because LLVM folds it into a multiply. (`fortran/intrinsics.rs`)

19. **A decimal point in an exponent changes the computation.** `x ** 3` is expanded inline by
   gfortran; `x ** 3.` is a `powf` call, and the two disagree on 26% of inputs. Twenty-two
   sites in `src/` use a constant real exponent and each was probed as spelled, because the
   pattern does not generalise: gfortran rewrites `x ** 2.` into a multiply (so `powf` is wrong
   there) but leaves `x ** 3.` and `x ** 4.` as calls (so the multiply is wrong there). Copy
   the exponent from the Fortran exactly, decimal point included.
   (`fortran/intrinsics.rs`, `pow2_real`)

20. **`x ** 0.5` is not a square root.** LLVM rewrites `x.powf(0.5)` into `sqrt` at `-O`, and
   gfortran's `powf` disagrees with `sqrt` on 0.04% of inputs -- so that rewrite makes release
   and debug builds differ. `powf` passes its exponent through `black_box` to keep the call a
   call. This was the only genuine no-go the spike ever turned up, and it was found by probing
   the exponents the physics actually uses rather than a representative sample.
   (`fortran/intrinsics.rs`)

21. **`NGEN_FORCING_ACTIVE` decides which precipitation field is authoritative**, and the two
   branches are mirror images: one copies `PRCP` into `PRCPNONC`, the other copies it back.
   Both leave the pair equal, so the choice only shows when they disagree on entry -- which is
   every timestep, since whichever field the driver did not write still holds the previous
   timestep's value. Upstream settles it with a `#ifdef`; the port takes it as a parameter
   ([`PrecipInput`]), because the shipping configuration (BMI writes `PRCPNONC`) and the one
   the fixtures were recorded under (the ASCII reader writes `PRCP`) both have to work.
   (`physics/atm_processing.rs`)

22. **`ATM` divides by zero under `OPT_SNF == 4` with no frozen precipitation.**
   `bdfall * (PRCPSNOW / PRCP_FROZEN)` is 0/0 and `bdfall` comes out NaN. Nothing guards it
   upstream, the sweep fixture covers the case, and the port reproduces the NaN rather than
   inventing a guard. (`physics/atm_processing.rs`)

23. **The comments on `O2PP` and `CO2PP` are swapped upstream.** `O2PP` is commented "co2
   concentration" and `CO2PP` "o2 concentration". The assignments are right; only the comments
   are crossed. (`physics/atm_processing.rs`)
