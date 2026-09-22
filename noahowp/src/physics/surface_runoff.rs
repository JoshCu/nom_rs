//! Port of `src/SurfaceRunoffModule.f90` @ 0ff055e.
//!
//! `SurfaceRunoff` dispatches on `OPT_RUN`. Options 1, 2, 4 and 5 are computed here; 3, 6, 7
//! and 8 live in [`crate::physics::surface_runoff_infiltration`].

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::surface_runoff_infiltration::{
    compute_vic_surfrunoff, compute_xaj_surfrunoff, dynamic_vic, infil,
};
use crate::water::Water;

/// `SurfaceRunoff`. An `opt_run` outside 1..=8 matches no `case` and does nothing, as upstream.
pub fn surface_runoff(
    domain: &Domain,
    nsoil: i32,
    options: &Options,
    parameters: &Parameters,
    water: &mut Water,
) {
    match options.opt_run {
        1 => surface_runoff_topmodel_groundwater(parameters, water),
        2 => surface_runoff_topmodel_equiwatertable(parameters, water),
        3 => infil(parameters, domain, nsoil, water),
        4 => surface_runoff_bats(domain, nsoil, parameters, water),
        5 => surface_runoff_mmfan07(parameters, water),
        6 => compute_vic_surfrunoff(parameters, domain, nsoil, water),
        7 => compute_xaj_surfrunoff(parameters, domain, nsoil, water),
        8 => dynamic_vic(parameters, options, domain, nsoil, water),
        _ => {}
    }
}

/// `surface_runoff_TOPMODEL_groundwater` -- TOPMODEL with groundwater (Niu et al. 2007 JGR).
fn surface_runoff_topmodel_groundwater(parameters: &Parameters, water: &mut Water) {
    let FFF = 6.0f32;
    let FSAT = parameters.fsatmx * fi::exp(-0.5 * FFF * (water.zwt - 2.0));
    if water.qinsur > 0. {
        water.runsrf = water.qinsur * ((1.0 - water.fcr[1]) * FSAT + water.fcr[1]);
        water.pddum = water.qinsur - water.runsrf; // m/s
    }
}

/// `surface_runoff_TOPMODEL_equiwatertable` -- TOPMODEL with an equilibrium water table
/// (Niu et al. 2005 JGR).
fn surface_runoff_topmodel_equiwatertable(parameters: &Parameters, water: &mut Water) {
    let FFF = 2.0f32;
    let FSAT = parameters.fsatmx * fi::exp(-0.5 * FFF * water.zwt);
    if water.qinsur > 0. {
        water.runsrf = water.qinsur * ((1.0 - water.fcr[1]) * FSAT + water.fcr[1]);
        water.pddum = water.qinsur - water.runsrf; // m/s
    }
}

/// `surface_runoff_BATS`.
fn surface_runoff_bats(domain: &Domain, nsoil: i32, parameters: &Parameters, water: &mut Water) {
    let mut SMCTOT = 0.0f32; // 2-m averaged soil moisture (m3/m3)
    let mut DZTOT = 0.0f32; // 2-m soil depth (m)
    for K in 1..=nsoil {
        DZTOT += domain.dzsnso[K];
        SMCTOT += water.smc[K] / parameters.smcmax[K] * domain.dzsnso[K];
        if DZTOT >= 2.0 {
            break;
        }
    }
    SMCTOT /= DZTOT;
    let FSAT = fi::powf(fi::max(0.01, SMCTOT), 4.); // BATS

    if water.qinsur > 0. {
        water.runsrf = water.qinsur * ((1.0 - water.fcr[1]) * FSAT + water.fcr[1]);
        water.pddum = water.qinsur - water.runsrf; // m/s
    }
}

/// `surface_runoff_MMFan07`.
fn surface_runoff_mmfan07(parameters: &Parameters, water: &mut Water) {
    let FFF = 6.0f32;
    let FSAT = parameters.fsatmx * fi::exp(-0.5 * FFF * fi::max(-2.0 - water.zwt, 0.));
    if water.qinsur > 0. {
        water.runsrf = water.qinsur * ((1.0 - water.fcr[1]) * FSAT + water.fcr[1]);
        water.pddum = water.qinsur - water.runsrf; // m/s
    }
}
