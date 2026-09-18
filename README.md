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
cargo test      # 125 tests
cargo clippy --all-targets
cargo build --release   # target/release/libnoahowp_bmi.so
```

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

Bit-identity is verified differentially against a reference build of the Fortran, which needs
`gfortran` (built with `-O2 -ffp-contract=off`, no `-ffast-math`, and
`NGEN_FORCING_ACTIVE` / `NGEN_OUTPUT_ACTIVE` defined). That toolchain is not required to build
or test this repo, but it is required for the Phase 0 spike and for the per-module differential
fixtures described in the plan.
