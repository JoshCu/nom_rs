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
