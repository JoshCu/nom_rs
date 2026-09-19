//! Noah-OWP-Modular column model, ported from Fortran.
//!
//! Layout mirrors the Fortran source one file per module -- see `docs/RUST_REWRITE_PLAN.md`
//! and `PORTING.md`. Scope is the BMI path only: no standalone driver, no ASCII forcing
//! reader, no NetCDF output, no gridded code.

// Ported code preserves the Fortran's expression structure so the arithmetic stays bit-for-bit
// comparable and a reviewer can read the two side by side. `-1. * x` is written that way
// upstream and stays that way here.
#![allow(clippy::neg_multiply)]
// Fortran passes whole derived types; a ported subroutine takes the same set as separate
// borrows, which routinely exceeds clippy's argument-count threshold.
#![allow(clippy::too_many_arguments)]
// `excessive_precision` would have us shorten float literals to the digits f32 can hold. That
// is exactly how a constant drifts a ULP from the Fortran's (it already happened once, to TLC
// -- see constants.rs). Literals are copied verbatim from the Fortran, full stop.
#![allow(clippy::excessive_precision)]
// `approx_constant` wants `3.14159265` replaced with `PI`. They are not the same number in
// f32, and the Fortran has two different pi literals of its own -- `ConstantsModule`'s DEGRAD
// uses `3.1415926` while `calc_declin`'s uses `3.14159265`. Literals stay as found.
#![allow(clippy::approx_constant)]

pub mod constants;
pub mod date_time_utils;
pub mod difftest;
pub mod domain;
pub mod energy;
pub mod error_check;
pub mod forcing;
pub mod fortran;
pub mod layers;
pub mod levels;
pub mod namelist_read;
pub mod options;
pub mod parameters;
pub mod parameters_read;
pub mod run;
pub mod utilities;
pub mod water;

pub use layers::Shifted;
pub use namelist_read::{ConfigError, NamelistConfig};
