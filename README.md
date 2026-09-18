# nom_rs

A Rust reimplementation of the BMI-facing subset of
[Noah-OWP-Modular](https://github.com/NOAA-OWP/noah-owp-modular), driven by
[`bmi-driver`](https://github.com/JoshCu/bmi-driver).

**Status: skeleton and BMI metadata surface.** The physics column is not ported yet.

| Area | State |
|---|---|
| Input readers (namelist, list-directed), Fortran-index arrays | done, tested against the real upstream `.TBL` files |
| Config chain (namelist → levels/options/domain), constants, date utils | done |
| BMI metadata, grids, time, `initialize`, `finalize` | done |
| BMI `get_value` / `set_value` | stub -- awaits the state types |
| BMI `update` (the physics) | stub |

**Picking this up?** Start with [`HANDOFF.md`](HANDOFF.md) -- current state, what to do first
with a Fortran toolchain, and the traps found in the Fortran so far. Then
[`PORTING.md`](PORTING.md) for the per-file table and how to track upstream changes.

```sh
cargo test      # 131 tests; also passes with --release
cargo clippy --all-targets
cargo build --release   # target/release/libnoahowp_bmi.so
```

**Bit-identity is confirmed achievable.** The Phase 0 intrinsic spike returned GO against
gfortran 15.2.0: every intrinsic the model uses has a Rust spelling that matches bit for bit.
Ported physics must call through
[`noahowp/src/fortran/intrinsics.rs`](noahowp/src/fortran/intrinsics.rs) rather than reaching
for `f32::exp` or `x * x * x * x` -- several of the obvious spellings are wrong. See
[`spike/intrinsics/README.md`](spike/intrinsics/README.md).

## Why

- **Calibration throughput.** The Fortran keeps parameter tables in module-level `save` globals,
  so instances cannot share a process — `bmi-driver` forks workers and pays IPC for every result.
  Instance-owned state in Rust makes the model `Send`, enabling thread-based workers and cheap
  re-initialisation between calibration iterations.
- **Modifiability.** Readable, testable, `unsafe`-free physics with per-subroutine differential
  tests against the Fortran.

## Goals

1. **Bit-identical output** vs. the Fortran for the same config + forcing.
2. Calibration throughput.
3. Modifiability.

## Scope

BMI path only — the 29 functions `bmi-driver` actually calls, grids 0/1/2. No standalone driver,
no ASCII forcing reader, no NetCDF output, no gridded code. See
[`docs/RUST_REWRITE_PLAN.md`](docs/RUST_REWRITE_PLAN.md) for the full plan: file-for-file layout
mirroring the Fortran `src/` and `bmi/`, bit-identity rules, verification harness, and phasing.

## Upstream

Pinned to `NOAA-OWP/noah-owp-modular` @ `eaa8282`. See [`PORTING.md`](PORTING.md).

## Building the reference Fortran

Bit-identity is verified differentially against a reference build of the Fortran:

```sh
cd reference && ./build.sh --fixtures
```

That records the complete model state before and after each of the five physics calls, over a
sampled Bondville year, and writes it to `noahowp/tests/fixtures/difftest/`. Each ported Rust
function is then held to reproducing the post-state bit for bit from the pre-state. See
[`reference/README.md`](reference/README.md).

The fixtures are committed, so **`gfortran` is not needed to build or test this repo** -- only
to regenerate them. The reference build needs no ngen, no CMake and no NetCDF.
