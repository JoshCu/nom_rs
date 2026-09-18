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
| `src/ParametersRead.f90` | `noahowp/src/parameters_read.rs` | todo |
| `src/ParametersType.f90` | `noahowp/src/parameters.rs` | todo |
| `src/ForcingType.f90` | `noahowp/src/forcing.rs` | todo |
| `src/EnergyType.f90` | `noahowp/src/energy.rs` | todo |
| `src/WaterType.f90` | `noahowp/src/water.rs` | todo |
| `src/UtilitiesModule.f90` | `noahowp/src/utilities.rs` | todo |
| `src/RunModule.f90` | `noahowp/src/run.rs` | todo |

`DateTimeUtilsModule.f90` also vendors a general string-utility library (`parse`, `compact`,
`removesp`, `shiftstr`, `insertstr`, `delsubstr`, `uppercase`, `readline`, `match`, `write_*`,
`writeq_*`, `trimzero`, `split`, `removebksl`). None of it is reachable from the BMI path;
only `parse_date`, `julian_date`, `calendar_date`, `date_to_unix`, `unix_to_date` and
`get_utime_list` are ported.

### Physics

| Upstream | Rust | Status |
|---|---|---|
| `src/ForcingModule.f90` | `physics/forcing_main.rs` | todo |
| `src/AtmProcessing.f90` | `physics/atm_processing.rs` | todo |
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

8. **Integer powers go through `powi`, never through repeated multiplication.** gfortran
   expands `x ** n` by squaring, so `x * x * x * x` is a different computation from `x ** 4`
   and disagrees on 34% of inputs. `powf` is also not a substitute, and looks correct in
   release builds only because LLVM folds it into a multiply. (`fortran/intrinsics.rs`)
