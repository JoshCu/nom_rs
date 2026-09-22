//! The physics column: the five calls `solve_noahowp` makes, and everything under them.
//!
//! One file per Fortran module, named for it. Every one of them does its arithmetic through
//! [`crate::fortran::intrinsics`] rather than reaching for `f32::exp` or `x * x` directly --
//! see that module for why the obvious spellings are not interchangeable.

pub mod albedo;
pub mod atm_processing;
pub mod canopy_water;
pub mod energy_main;
pub mod et_flux;
pub mod forcing_main;
pub mod interception;
pub mod precip_heat;
pub mod shortwave_radiation;
pub mod snow_layer_change;
pub mod snow_soil_temp;
pub mod snow_water;
pub mod snow_water_renew;
pub mod soil_water;
pub mod soil_water_movement;
pub mod soil_water_retention_coeff;
pub mod subsurface_runoff;
pub mod surface_runoff;
pub mod surface_runoff_infiltration;
pub mod thermal_properties;
pub mod water_main;
