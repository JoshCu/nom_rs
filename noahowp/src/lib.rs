//! Noah-OWP-Modular column model, ported from Fortran.
//!
//! Layout mirrors the Fortran source one file per module -- see `docs/RUST_REWRITE_PLAN.md`
//! and `PORTING.md`. Scope is the BMI path only: no standalone driver, no ASCII forcing
//! reader, no NetCDF output, no gridded code.

pub mod fortran;
pub mod layers;

pub use layers::Shifted;
