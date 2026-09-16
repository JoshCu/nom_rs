# nom_rs

A Rust reimplementation of the BMI-facing subset of
[Noah-OWP-Modular](https://github.com/NOAA-OWP/noah-owp-modular), driven by
[`bmi-driver`](https://github.com/JoshCu/bmi-driver).

**Status: planning.** No model code yet.

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

Pinned to `NOAA-OWP/noah-owp-modular` @ `eaa8282`.
