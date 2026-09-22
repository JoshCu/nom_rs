# The reference Fortran, and the differential fixtures

The port's first goal is bit-identical output. That is only checkable against the Fortran
itself, so this directory builds a reference copy of `noah-owp-modular` and records what it
computes, one physics call at a time.

```sh
./build.sh              # build the reference
./build.sh --fixtures   # build, run the Bondville year, write the fixtures
```

Roughly a minute for the build, a fraction of a second for the year. The output lands in
`noahowp/tests/fixtures/difftest/bondville.difftest` and is committed, so **nothing in the Rust
test suite needs `gfortran`** -- only regenerating the fixtures does.

## What the fixtures contain

`solve_noahowp` calls exactly five subroutines:

| tag | subroutine |
|---|---|
| 1 | `UtilitiesMain` |
| 2 | `ForcingMain` |
| 3 | `InterceptionMain` |
| 4 | `EnergyMain` |
| 5 | `WaterMain` |

For a sample of timesteps, the complete state (`domain`, `forcing`, `energy`, `water`) is
written immediately before and immediately after each of them, plus the run configuration
(`levels`, `options`, `parameters`) once at the start.

That turns the physics port into a checklist: each ported Rust function is handed the recorded
pre-state and has to reproduce the post-state bit for bit. `noahowp::difftest` reads the
fixtures, and `State::diff` returns the names of the fields that disagree.

Sampling covers the first 24 timesteps consecutively -- spin-up, where the state moves fastest
-- and then spreads evenly across the year. The spread is not decoration: Bondville's winter is
a small slice of 17,522 timesteps, and a fixture set drawn from a single window would never
reach the snow physics at all.

## The parameter sweep

`paramsweep` writes a second fixture, `param_sweep.difftest`. Bondville exercises exactly one
vegetation type, one soil texture and one soil colour, which leaves most of every table unread
-- a column misread in the port would not show up. The sweep walks each class index in turn,
under both vegetation classifications.

One dimension at a time rather than the full cross product: the indices select independent
table rows, and the only expressions that mix them (`kdt`, `frzx`) are functions of the soil row
alone. 65 cases per dataset instead of 6,480, for the same coverage of every table entry.

Indices deliberately run past the number of rows the file supplies, up to the declared table
size, because that is in bounds for the Fortran and returns the `-1.E36` sentinel. Reproducing
the sentinel -- and the infinities the derived parameters then take -- is part of the contract.

It earns its keep: a 1-ULP error from writing `kdt` as `refkdt * (dksat(1) / refdk)` instead of
`refkdt * dksat(1) / refdk` shows up in 16 sweep cases and in none of the Bondville ones.

## The date sweep

`datesweep` writes `newdate_sweep.bin` and `declin_sweep.bin`. Replaying Bondville covers one
year at one location on flat ground, which exercises neither leap-year handling nor the
slope/aspect correction in `calc_declin` -- both live code the port has to get right.

The sweep walks them directly: eight start dates chosen around leap years, the century and
400-year exceptions and year boundaries, times fourteen offsets in both directions; and a solar
geometry grid over month, hour, latitude, longitude, slope and azimuth (20,160 cases).

Both files are fixed-width records with no framing, since the layouts do not change and the
reader knows them:

```text
newdate: odate(12) idt(i4) ndate(12)
declin:  nowdate(19) lat lon slope azimuth cosz cosz_horiz julian (7 x r4) yearlen(i4)
```

## The ATM sweep

`atmsweep` writes `atm_sweep.difftest`. Bondville runs `precip_phase_option = 1` for the whole
year, so six of the seven precipitation-phase schemes in `AtmProcessing` are never entered --
and neither is the wet-bulb calculation, the Jennings logistic regression, the weather-model
phase path, or the direct-irradiance clamp in the shortwave split. The port could have every
one of them wrong and the year-long recording would still agree.

574 cases: each of the seven options over 34 hand-placed inputs and 48 pseudo-random ones. The
hand-placed cases are the ones a random sweep will not find --

- each Jordan (1991) temperature boundary and the rain-snow threshold hit **exactly**, and
  again at the next representable value up, since `<=` and `<` differ only there;
- eight points inside the Jordan ramp, the one band where `FPICE` is neither 0, 0.6 nor 1;
- both cosines at and below zero, and a sunrise case where they disagree enough to push the
  direct irradiance over the solar constant;
- frozen precipitation of exactly zero under `OPT_SNF == 4`, where upstream divides 0 by 0 and
  `bdfall` goes NaN. That is live code, so reproducing the NaN is part of the contract;
- four wind speeds taken from the intrinsic spike's own report, where `UU ** 2.` and a `powf`
  call disagree. Those two spellings differ on 0.04% of inputs, which a sweep this size would
  otherwise reach about once by luck -- and a port that used `powf` there passed both this
  sweep and the Bondville year until these four cases were added.

Inputs are generated in the driver and never agreed with the Rust side: the port reads them
back out of the recorded pre-state. The one thing both sides must agree on is which option a
case ran under, and that travels in the record's `itime` slot as `opt_snf * 100000 + sample`.

Records go through the same `dt_record` the instrumented `RunModule` uses, so this fixture
parses with the same reader and picks up new fields when upstream adds them. The static block
at the head of the file is the namelist's own option set, *not* the option any given case ran
under -- the sweep rebuilds `options` and `parameters` per `OPT_SNF`, because `paramRead`
resolves `rain_snow_thresh` from the option and poking `options%opt_snf` alone would leave the
threshold wrong.

## The interception sweep

`interceptsweep` writes `intercept_sweep.difftest`. Bondville runs
`dynamic_veg_option = 1, vegtyp = 1, croptype = 0` for the whole year, which leaves eight of the
nine `dveg` branches in `PHENOLOGY` unreached, along with the southern-hemisphere day-of-year
shift, the short-canopy snow-burial formula, the water/barren/ice/urban zeroing and the crop
path -- and, because LAI and SAI never reach zero there, the entire buried-canopy branch of
`CanopyWaterIntercept`.

Three phases:

1. **Branch coverage** -- all nine `dveg` options crossed with seven vegetation types (Bondville's,
   the four that zero LAI and SAI outright, and two short canopies that take the `SNOWHC` branch)
   and both crop settings, two cases each.
2. **Numerical depth** -- one tall canopy and one short one, with the canopy water balance swept
   over snow depth, rainfall, snowfall, canopy storage and timestep length.
3. **The 0.05 cutoffs** -- run under `dveg = 2`, because 1, 3 and 4 overwrite LAI and SAI from
   the monthly tables on entry and a hand-set value never survives to be tested.

That third phase is the fiddly one. `ESAI` and `ELAI` are never assigned directly -- they are
`SAI` and `LAI` scaled by the unburied fraction -- and `SAI` below 0.05 is zeroed *before* they
are computed, so the only way to land `ESAI` just under its own cutoff is to start above it and
let the snow take the difference. `ELAI` is worse: with LAI and SAI equal, `ESAI` is zeroed
first and `ESAI == 0.0` then forces `ELAI` to zero whichever way `ELAI`'s own comparison falls,
masking it entirely. It needs a second pass with the stem area well clear of its cutoff. Both
passes sweep snow depth rather than solving for one value, since the burial fraction depends on
the canopy heights of whichever vegetation type the case selected.

The configuration travels in the record id as
`((dveg * 32 + vegtyp) * 2 + croptype) * 100 + sample`.

## The water sweep

`watersweep` writes `water_sweep.difftest`. Bondville runs one option of each water scheme for
the whole year -- `runoff_option = 8`, `drainage_option = 8`, `frozen_soil_option = 1`,
`dynamic_vic_option = 1`, `subsurface_option = 1` -- which leaves seven of the eight surface
runoff schemes, both TOPMODEL water tables, MMF drainage, the second diffusivity form and
one-way coupling unentered. Its winter is thin too: over 200 sampled timesteps `WaterMain` never
starts with more than one snow layer, so `DIVIDE` and `COMBO` are never reached at all.

Unlike the other sweeps, most cases here are not synthetic. Snow layers, soil moisture and the
water table are state that builds up over weeks, and a hand-drawn pre-state is a poor stand-in
for one the model grew itself. So the driver **replays the Bondville year** under 56
configurations, making the five physics calls itself in `solve_noahowp`'s order so that only
`WaterMain` is recorded:

- every runoff scheme with the drainage scheme of the same number, under both diffusivity forms;
- the other two dynamic VIC infiltration equations;
- one-way coupling (`subsurface_option = 2`);
- six cross pairings where one scheme reads what the other writes -- `ZWT` from `ZWTEQ`,
  `FCRMAX`, the MMF water table;

each in the recorded climate and in a colder, wetter one (`SFCTMP - 8 K`, `PRCP x 2`) where the
pack builds up in layers and melts out again. A replay records against per-category budgets
(layered snow, shallow snow, rain, frozen ground, anything) so a long snow season does not
spend them all in December.

Then three sets of hand-pushed cases, each derived from a state a replay reached and restored
afterwards so the replay continues on its own trajectory:

1. **Snow and surface** -- the 5000 mm glacier cap, a lake point over and under its storage
   cap, frozen ground, sublimation large enough for `COMBINE` to drop a layer, a top layer
   starved of ice and one thinner than `DZMIN`, a pack thin enough to fold back into shallow
   snow, and a single thick layer for `DIVIDE` to split into two and on into three.
2. **Infiltration capacity** -- a near-saturated top layer under heavy ponding, so that more
   water arrives in a sub-step than `DYNAMIC_VIC` can take in (`FMAX*DT < DP`). The two
   branches that follow are the ones where upstream reads `YD` before assigning it; see
   `PORTING.md`. Run under all three infiltration equations.
3. **A deep MMF water table**, where `SSTEP` accumulates recharge instead of updating `SMCWTD`.

Some legal option pairings drive the column out of physical range -- `drainage_option = 1` is
the groundwater scheme with the groundwater model absent, and `SH2O` goes NaN within weeks --
and `EnergyMain` then `STOP`s on negative emitted longwave. The driver ends such a replay when
its state leaves range, and keeps what it had recorded: those pairs are still valid.

The configuration travels in the record id as
`((((run * 10 + drn) * 10 + inf) * 10 + infdv) * 10 + sub) * 10 + climate) * 100 + sample`,
with climate 9 marking the hand-pushed cases. The forcing year is read once up front:
`read_forcing_text` keeps its file position in saved locals that only reset at end of file, so
it cannot be restarted for a second replay.

## Why it is built this way

**No BMI, no ngen, no NetCDF.** `noah-owp-modular` has no CMake of its own; `libsurfacebmi.so`
is produced by a wrapper in ngen's `extern/` together with the `iso_c_fortran_bmi` middleware.
Driving the model directly avoids all of it. Compiling with `-DNGEN_OUTPUT_ACTIVE` removes
`OutputModule`, the only NetCDF user in the codebase, while leaving `NGEN_FORCING_ACTIVE`
undefined keeps the ASCII forcing reader so the run can be driven from `data/bondville.dat`.

**The upstream checkout is read-only.** Sources are extracted from the pinned commit with
`git archive`, so a dirty working tree there cannot leak into the reference and nothing is ever
written back.

**The serializer is generated, not written.** `gen_serializer.py` parses the derived types and
emits both halves of the format -- the Fortran writer and `noahowp/src/difftest/manifest.rs` --
so a field added upstream appears on both sides at once. Writing ~500 field accesses by hand
and keeping them in step was never going to survive contact with an upstream bump. The parser
refuses to skip a declaration it cannot understand, because a quietly dropped field shifts
every field after it and would surface as unrelated physics appearing to be wrong.

**The instrumentation is applied, not committed.** `instrument.py` inserts the dump calls into
the build copy of `RunModule.f90` and requires each of the five call sites to appear exactly
once, so an upstream rename stops the build rather than mislabelling a tag.

## Flags

```
-O2 -ffp-contract=off -cpp -DNGEN_OUTPUT_ACTIVE
```

`-ffp-contract=off` stops the compiler fusing multiply-add, which would change results the Rust
port has to reproduce. Never `-ffast-math`.

## Environment

| | |
|---|---|
| `NOM_SRC` | upstream checkout; searched for if unset |
| `NOM_COMMIT` | commit to build; defaults to the pin in `PORTING.md` |
| `FC` | Fortran compiler, default `gfortran` |
| `NSAMPLES` | timesteps sampled per call, default 200 (~3 MB of fixtures) |

`FC` is worth knowing about. Bit-identity is a claim about a **(compiler, libc) pair**, and the
`libsurfacebmi.so` in use at `/dmod/shared_libs/` was built with GCC 11.5.0 on Red Hat, not the
GCC 15.2.0 these fixtures were generated with. The intrinsic spike found no divergence that
depends on the compiler version, but that was measured on one pair. If the calibration target
is the deployed library, regenerate with a matching `FC` and diff the fixtures.

## Format

```text
magic u32 ('NOMD')                     -- static block
  levels, options, parameters
repeated:
  magic u32, tag i32, phase i32, itime i32
  domain, forcing, energy, water
```

Fields follow manifest order, little-endian. `logical` is normalised to an `integer` 0 or 1;
`character(len=N)` is N bytes, space padded; an `allocatable` field carries an `i32` element
count immediately before its data, so a fixture built for a different `nsoil`/`nsnow` is caught
on read instead of being reinterpreted. `domain%sim_datetimes` is excluded -- one f64 per
timestep of the whole run, 140 KB in every record, and derivable from `(start, end, dt)`, which
are in the record already.
