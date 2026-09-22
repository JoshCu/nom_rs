//! Port of `src/SubsurfaceRunoffModule.f90` @ 0ff055e.
//!
//! `SubsurfaceRunoff` dispatches on `OPT_DRN`. Only options 1 and 2 compute anything of their
//! own; every other option adds the soil-bottom drainage `SRT` already computed.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// `SubsurfaceRunoff`. An `opt_drn` outside 1..=8 matches no `case` and does nothing, as
/// upstream.
pub fn subsurface_runoff(
    domain: &Domain,
    nsoil: i32,
    options: &Options,
    parameters: &Parameters,
    water: &mut Water,
) {
    match options.opt_drn {
        1 => subsurface_runoff_groundwater(parameters, water),
        2 => subsurface_runoff_equiwatertable(domain, nsoil, parameters, water),
        // 3 free drainage, 4 BATS, 5 MMF, 6 VIC, 7 Xinanjiang, 8 dynamic VIC: the same sum.
        3..=8 => water.runsub += water.qdrain, // mm/s
        _ => {}
    }
}

/// `subsurface_runoff_groundwater` -- the groundwater module of Niu et al. (2007), without
/// the groundwater: `ZWT` is not updated.
fn subsurface_runoff_groundwater(parameters: &Parameters, water: &mut Water) {
    let FFF = 6.0f32; // runoff decay factor (m-1)
    let RSBMX = 5.0f32; // baseflow coefficient [mm/s]
    water.runsub = (1.0 - water.fcrmax)
        * RSBMX
        * fi::exp(-parameters.timean)
        * fi::exp(-FFF * (water.zwt - 2.0));
}

/// `subsurface_runoff_equiwatertable` -- an equilibrium water table (Niu et al. 2005).
fn subsurface_runoff_equiwatertable(
    domain: &Domain,
    nsoil: i32,
    parameters: &Parameters,
    water: &mut Water,
) {
    let FFF = 2.0f32; // runoff decay factor (m-1)
    let RSBMX = 4.0f32; // baseflow coefficient [mm/s]
    zwteq(parameters, domain, nsoil, water);
    water.runsub =
        (1.0 - water.fcrmax) * RSBMX * fi::exp(-parameters.timean) * fi::exp(-FFF * water.zwt);
    // mm/s
}

/// `ZWTEQ` -- equilibrium water table depth (Niu et al., 2005).
pub fn zwteq(parameters: &Parameters, domain: &Domain, nsoil: i32, water: &mut Water) {
    const NFINE: i32 = 100; // no. of fine soil layers of 6m soil
    let zsoil = &domain.zsoil;

    let mut WD1 = 0.0f32;
    for K in 1..=nsoil {
        WD1 += (parameters.smcmax[1] - water.sh2o[K]) * domain.dzsnso[K]; // [m]
    }

    let DZFINE = 3.0 * (-zsoil[nsoil]) / NFINE as f32;

    water.zwt = -3. * zsoil[nsoil] - 0.001; // initial value [m]

    let mut WD2 = 0.0f32;
    for K in 1..=NFINE {
        let ZFINE = K as f32 * DZFINE;
        let TEMP = 1. + (water.zwt - ZFINE) / parameters.psisat[1];
        WD2 += parameters.smcmax[1] * (1. - fi::powf(TEMP, -1. / parameters.bexp[1])) * DZFINE;
        if (WD2 - WD1).abs() <= 0.01 {
            water.zwt = ZFINE;
            break;
        }
    }
}
