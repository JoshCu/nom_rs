//! Port of `src/SoilWaterMovement.f90` @ 0ff055e.
//!
//! `SRT` builds the tridiagonal system for the implicit soil water diffusion step and `SSTEP`
//! solves it and moves any excess above saturation between layers, bucket-style.
//!
//! This module's `ROSR12` is a character-for-character copy of the one in
//! `SnowSoilTempModule.f90`, so the port calls
//! [`crate::physics::snow_soil_temp::rosr12`] rather than keeping a second copy. If upstream
//! ever changes one and not the other, this is where they diverge.

#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use crate::domain::Domain;
use crate::fortran::intrinsics as fi;
use crate::layers::Shifted;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::snow_soil_temp::rosr12;
use crate::physics::soil_water_retention_coeff::{wdfcnd1, wdfcnd2};
use crate::water::Water;

/// `SRT` -- the right-hand side of the time tendency term of the soil water diffusion
/// equation, and the matrix coefficients of the implicit scheme.
pub fn srt(
    options: &Options,
    parameters: &Parameters,
    domain: &Domain,
    nsoil: i32,
    water: &mut Water,
    RHSTT: &mut Shifted<f32>,
    AI: &mut Shifted<f32>,
    BI: &mut Shifted<f32>,
    CI: &mut Shifted<f32>,
) {
    let zsoil = &domain.zsoil;
    let mut DDZ = Shifted::ones(nsoil, 0.0f32);
    let mut DENOM = Shifted::ones(nsoil, 0.0f32);
    let mut DSMDZ = Shifted::ones(nsoil, 0.0f32);
    let mut WFLUX = Shifted::ones(nsoil, 0.0f32);
    let mut WDF = Shifted::ones(nsoil, 0.0f32);
    let mut SMX = Shifted::ones(nsoil, 0.0f32);
    // Assigned only under OPT_DRN == 5, and read only there.
    let mut SMXWTD = 0.0f32;

    // Niu and Yang (2006), J. of Hydrometeorology
    if options.opt_inf == 1 {
        for K in 1..=nsoil {
            (WDF[K], water.wcnd[K]) = wdfcnd1(parameters, water.smc[K], water.fcr[K], K);
            SMX[K] = water.smc[K];
        }
        if options.opt_drn == 5 {
            SMXWTD = water.smcwtd;
        }
    }

    if options.opt_inf == 2 {
        for K in 1..=nsoil {
            (WDF[K], water.wcnd[K]) = wdfcnd2(parameters, water.sh2o[K], water.sicemax, K);
            SMX[K] = water.sh2o[K];
        }
        if options.opt_drn == 5 {
            SMXWTD = water.smcwtd * water.sh2o[nsoil] / water.smc[nsoil]; // same liquid fraction as in the bottom layer
        }
    }

    for K in 1..=nsoil {
        if K == 1 {
            DENOM[K] = -zsoil[K];
            let TEMP1 = -zsoil[K + 1];
            DDZ[K] = 2.0 / TEMP1;
            DSMDZ[K] = 2.0 * (SMX[K] - SMX[K + 1]) / TEMP1;
            WFLUX[K] =
                WDF[K] * DSMDZ[K] + water.wcnd[K] - water.pddum + water.etrani[K] + water.qseva;
        } else if K < nsoil {
            DENOM[K] = zsoil[K - 1] - zsoil[K];
            let TEMP1 = zsoil[K - 1] - zsoil[K + 1];
            DDZ[K] = 2.0 / TEMP1;
            DSMDZ[K] = 2.0 * (SMX[K] - SMX[K + 1]) / TEMP1;
            WFLUX[K] =
                WDF[K] * DSMDZ[K] + water.wcnd[K] - WDF[K - 1] * DSMDZ[K - 1] - water.wcnd[K - 1]
                    + water.etrani[K];
        } else {
            DENOM[K] = zsoil[K - 1] - zsoil[K];
            if options.opt_drn == 1 || options.opt_drn == 2 {
                water.qdrain = 0.;
            }
            if options.opt_drn == 3
                || options.opt_drn == 6
                || options.opt_drn == 7
                || options.opt_drn == 8
            {
                water.qdrain = parameters.slope * water.wcnd[K];
            }
            if options.opt_drn == 4 {
                water.qdrain = (1.0 - water.fcrmax) * water.wcnd[K];
            }
            if options.opt_drn == 5 {
                // gmm new m-m&f water table dynamics formulation
                let TEMP1 = 2.0 * DENOM[K];
                let SMXBOT = if water.zwt < zsoil[nsoil] - DENOM[nsoil] {
                    // gmm interpolate from below, midway to the water table, to the middle of the auxiliary layer below the soil bottom
                    SMX[K] - (SMX[K] - SMXWTD) * DENOM[K] * 2. / (DENOM[K] + zsoil[K] - water.zwt)
                } else {
                    SMXWTD
                };
                DSMDZ[K] = 2.0 * (SMX[K] - SMXBOT) / TEMP1;
                water.qdrain = WDF[K] * DSMDZ[K] + water.wcnd[K];
            }
            WFLUX[K] =
                -(WDF[K - 1] * DSMDZ[K - 1]) - water.wcnd[K - 1] + water.etrani[K] + water.qdrain;
        }
    }

    for K in 1..=nsoil {
        if K == 1 {
            AI[K] = 0.0;
            BI[K] = WDF[K] * DDZ[K] / DENOM[K];
            CI[K] = -BI[K];
        } else if K < nsoil {
            AI[K] = -WDF[K - 1] * DDZ[K - 1] / DENOM[K];
            CI[K] = -WDF[K] * DDZ[K] / DENOM[K];
            BI[K] = -(AI[K] + CI[K]);
        } else {
            AI[K] = -WDF[K - 1] * DDZ[K - 1] / DENOM[K];
            CI[K] = 0.0;
            BI[K] = -(AI[K] + CI[K]);
        }
        RHSTT[K] = WFLUX[K] / (-DENOM[K]);
    }
}

/// `SSTEP` -- update soil moisture content values. Returns `WPLUS`, the saturation excess
/// water (m).
// `WPLUS = 0.0` on entry mirrors the Fortran; every path overwrites it before the return.
#[allow(unused_assignments)]
pub fn sstep(
    options: &Options,
    parameters: &Parameters,
    domain: &Domain,
    nsoil: i32,
    water: &mut Water,
    dt: f32,
    AI: &mut Shifted<f32>,
    BI: &mut Shifted<f32>,
    CI: &mut Shifted<f32>,
    RHSTT: &mut Shifted<f32>,
) -> f32 {
    let dz = &domain.dzsnso;
    let zsoil = &domain.zsoil;
    let mut WPLUS = 0.0f32;

    for K in 1..=nsoil {
        RHSTT[K] *= dt;
        AI[K] *= dt;
        BI[K] = 1. + BI[K] * dt;
        CI[K] *= dt;
    }

    // copy values for input variables before calling rosr12
    let RHSTTIN = RHSTT.clone();
    let mut CIIN = CI.clone();

    // call ROSR12 to solve the tri-diagonal matrix
    rosr12(AI, BI, &RHSTTIN, 1, nsoil, &mut CIIN, CI, RHSTT);

    for K in 1..=nsoil {
        water.sh2o[K] += CI[K];
    }

    // excessive water above saturation in a layer is moved to
    // its unsaturated layer like in a bucket

    // gmmwith opt_drn=5 there is soil moisture below levels%nsoil, to the water table
    if options.opt_drn == 5 {
        // update smcwtd
        if water.zwt < zsoil[nsoil] - dz[nsoil] {
            // accumulate qdrain to update deep water table and soil moisture later
            water.deeprech += dt * water.qdrain;
        } else {
            water.smcwtd += dt * water.qdrain / dz[nsoil];
            WPLUS = fi::max(water.smcwtd - parameters.smcmax[nsoil], 0.0) * dz[nsoil];
            let WMINUS = fi::max(1.0E-4 - water.smcwtd, 0.0) * dz[nsoil];

            water.smcwtd = fi::max(fi::min(water.smcwtd, parameters.smcmax[nsoil]), 1.0E-4);
            water.sh2o[nsoil] += WPLUS / dz[nsoil];

            // reduce fluxes at the bottom boundaries accordingly
            water.qdrain -= WPLUS / dt;
            water.deeprech -= WMINUS;
        }
    }

    for K in (2..=nsoil).rev() {
        let EPORE = fi::max(1.0E-4, parameters.smcmax[K] - water.sice[K]);
        WPLUS = fi::max(water.sh2o[K] - EPORE, 0.0) * dz[K];
        water.sh2o[K] = fi::min(EPORE, water.sh2o[K]);
        water.sh2o[K - 1] += WPLUS / dz[K - 1];
    }

    let EPORE = fi::max(1.0E-4, parameters.smcmax[1] - water.sice[1]);
    WPLUS = fi::max(water.sh2o[1] - EPORE, 0.0) * dz[1];
    water.sh2o[1] = fi::min(EPORE, water.sh2o[1]);

    if WPLUS > 0.0 {
        water.sh2o[2] += WPLUS / dz[2];
        for K in 2..=nsoil - 1 {
            let EPORE = fi::max(1.0E-4, parameters.smcmax[K] - water.sice[K]);
            WPLUS = fi::max(water.sh2o[K] - EPORE, 0.0) * dz[K];
            water.sh2o[K] = fi::min(EPORE, water.sh2o[K]);
            water.sh2o[K + 1] += WPLUS / dz[K + 1];
        }

        let EPORE = fi::max(1.0E-4, parameters.smcmax[nsoil] - water.sice[nsoil]);
        WPLUS = fi::max(water.sh2o[nsoil] - EPORE, 0.0) * dz[nsoil];
        water.sh2o[nsoil] = fi::min(EPORE, water.sh2o[nsoil]);
    }

    for K in 1..=nsoil {
        water.smc[K] = water.sh2o[K] + water.sice[K];
    }

    WPLUS
}
