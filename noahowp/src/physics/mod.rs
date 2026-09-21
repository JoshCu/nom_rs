//! The physics column: the five calls `solve_noahowp` makes, and everything under them.
//!
//! One file per Fortran module, named for it. Every one of them does its arithmetic through
//! [`crate::fortran::intrinsics`] rather than reaching for `f32::exp` or `x * x` directly --
//! see that module for why the obvious spellings are not interchangeable.

pub mod atm_processing;
pub mod forcing_main;
