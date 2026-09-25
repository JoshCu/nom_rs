# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Rust port of the BMI path of [Noah-OWP-Modular](https://github.com/NOAA-OWP/noah-owp-modular)
(Fortran), pinned at upstream `0ff055e`. **The overriding goal is output that is bit-identical to
the Fortran.** Almost every rule below exists to protect that.

## Commands

```sh
cargo build --release                      # target/release/libnoahowp_bmi.so (the C BMI library)
cargo test                                 # all tests; also run with --release, both must pass
cargo test -p noahowp --test water_vs_fortran        # one integration test file
cargo test -p noahowp-bmi qseva                      # tests whose name matches
cargo clippy --all-targets                 # must stay at 0 warnings
```

Tests need no gfortran: the Fortran-recorded fixtures are committed under
`noahowp/tests/fixtures/difftest/`. gfortran is needed only to regenerate them:

```sh
cd reference && ./build.sh --fixtures      # needs a noah-owp-modular checkout (NOM_SRC)
cd reference/intrinsics && ./run.sh        # re-verify intrinsics after a gfortran/rustc change
```

End-to-end check against the Fortran library, if the NGIAB test data is present: run
`bmi-driver <data dir>` with a realization whose NoahOWP module is `bmi_c` pointing at the
`.so`, and diff `outputs/ngen/*.csv` against a run using `libsurfacebmi.so`.

## Architecture

- **`noahowp/`**: the model, with **zero dependencies** (keep it that way). One Rust file per
  Fortran module, in the same item order (`physics/et_flux.rs` ← `EtFluxModule.f90`). Every
  ported function has a `// Port of <file>::<SUBROUTINE> @ <commit>` header. Bump the commit
  there and in `docs/porting.md` when porting an upstream change.
- **`run.rs`**: `NoahOwp` is the aggregate of the Fortran derived types (namelist, levels,
  domain, options, parameters, water, forcing, energy). `solve_noahowp` makes exactly five calls
  in order: `utilities_main`, `forcing_main`, `interception_main`, `energy_main`, `water_main`.
  Physics are free functions over borrowed structs, never methods. There is no `RefCell`.
- **`PrecipInput`**: which precipitation field the driver wrote. On the BMI path it is
  `NonConvective` (`PRCPNONC` via `set_value`). The fixtures were recorded with the ASCII reader,
  so tests pass `Total` (`PRCP`).
- **`noahowp-bmi/`**: `lib.rs` is `BmiNoahOwp`, a port of `bmi/bmi_noahowp.f90`. `vars.rs` is
  the single registry of exchange items (type, units, grid). `c_abi.rs` exports `register_bmi`,
  filling a CSDMS `bmi.h` function table. Grids are 0 (scalar), 1 (`nsnow`) and 2 (`nsoil`).
- **`difftest/`**: reads the fixtures. `bind::load` and `bind::compare` move a whole recorded
  state in or out of a `NoahOwp`. **`difftest/manifest.rs` is generated** by
  `reference/gen_serializer.py` from the Fortran types. Do not hand-edit it.
- **Profile arrays** use `Shifted<T>` (`layers.rs`), indexed with the Fortran's own `i32`
  bounds: `(-nsnow+1 : nsoil)` for snow/soil, and `1:nsoil` with `lo = 1` for soil-only arrays.
  Write loops with the Fortran indices, not 0-based translations.

## Bit-identity rules (violating these silently shifts results)

- `real` is `f32`; never widen the physics to `f64`.
- Copy expression structure and float literals from the Fortran exactly. Don't reassociate,
  factor, hoist, use `mul_add`, or replace loops with `.sum()`. The `excessive_precision` and
  `approx_constant` clippy lints are disabled on purpose; don't "fix" literals they would flag.
- All `EXP`, `LOG`, `**` and so on go through `noahowp/src/fortran/intrinsics.rs`. Use `powi`
  for integer exponents, never `x*x*x*x`. Use `powf` for real exponents (`x ** 3.` is not
  `x ** 3`). A bare `f32::exp` in `physics/` is a bug.
- Keep `MIN`/`MAX` argument order; they are not commutative on signed zeros.
- Many things that look like upstream bugs are reproduced deliberately and pinned by tests: see
  "Upstream behaviours that are deliberately preserved" in `docs/porting.md` before changing one.
- A new or changed physics path needs a check against a Fortran recording (the `*_vs_fortran`
  tests load a recorded pre-state and require the post-state to match bit for bit). Tolerances
  are not used.

## Docs

`docs/design.md` covers design and verification. `docs/porting.md` covers per-file status, the
toolchain pin (GNU Fortran 15.2.0 and rustc 1.98.1) and how to track upstream.
`reference/README.md` covers fixtures and sweeps.
