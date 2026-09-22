//! Port of `src/SoilWaterModule.f90` @ 0ff055e.
//!
//! `SOILWATER`: surface runoff and infiltration, the implicit soil moisture solve over
//! `NITER` sub-steps, and subsurface runoff -- or, under `OPT_SUB == 2`, holding the soil
//! column at its initial state.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::fortran::intrinsics as fi;
use crate::layers::Shifted;
use crate::levels::Levels;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::soil_water_movement::{srt, sstep};
use crate::physics::subsurface_runoff::subsurface_runoff;
use crate::physics::surface_runoff::surface_runoff;
use crate::water::Water;

/// `SOILWATER`.
pub fn soil_water(
    domain: &Domain,
    levels: &Levels,
    options: &Options,
    parameters: &Parameters,
    water: &mut Water,
) {
    const A: f32 = 4.0;
    let nsoil = levels.nsoil;
    let dt = domain.dt;
    let dz = &domain.dzsnso;

    // Call the full Noah-MP style subsurface when opt_sub == 1
    if options.opt_sub == 1 {
        water.runsrf = 0.0;
        water.runsub = 0.0;
        water.pddum = 0.0;
        water.runsrf_dt = dt;
        let mut RSAT = 0.0f32;

        // for the case when snowmelt water is too large
        for K in 1..=nsoil {
            let EPORE = fi::max(1.0E-4, parameters.smcmax[K] - water.sice[K]);
            RSAT += fi::max(0., water.sh2o[K] - EPORE) * dz[K];
            water.sh2o[K] = fi::min(EPORE, water.sh2o[K]);
        }

        // impermeable fraction due to frozen soil
        for K in 1..=nsoil {
            let FICE = fi::min(1.0, water.sice[K] / parameters.smcmax[K]);
            water.fcr[K] =
                fi::max(0.0, fi::exp(-A * (1. - FICE)) - fi::exp(-A)) / (1.0 - fi::exp(-A));
        }

        // maximum soil ice content and minimum liquid water of all layers
        water.sicemax = 0.0;
        water.fcrmax = 0.0;
        // SH2OMIN is computed upstream and read by nothing.
        for K in 1..=nsoil {
            if water.sice[K] > water.sicemax {
                water.sicemax = water.sice[K];
            }
            if water.fcr[K] > water.fcrmax {
                water.fcrmax = water.fcr[K];
            }
        }

        // update ZWT for option 2, which are used in both surface and subsurface runoff
        // other subsurface runoff options are done at the end
        if options.opt_drn == 2 {
            subsurface_runoff(domain, nsoil, options, parameters, water);
        }

        // surface runoff and infiltration rate using different schemes
        if parameters.urban_flag {
            water.fcr[1] = 0.95;
        }
        water.FACC = 1E-06;
        surface_runoff(domain, nsoil, options, parameters, water);

        // determine iteration times and finer time step
        //    IF(OPT_INF == 1) THEN    !OPT_INF =2 may cause water imbalance
        let mut NITER = 3;
        if water.pddum * dt > dz[1] * parameters.smcmax[1] {
            NITER *= 2;
        }
        //    END IF

        let DTFINE = dt / NITER as f32;
        water.runsrf_dt = DTFINE;

        // solve soil moisture
        water.FACC = 1E-06;
        let mut QDRAIN_SAVE = 0.0f32;
        let mut RUNSRF_SAVE = 0.0f32;
        let mut RHSTT = Shifted::ones(nsoil, 0.0f32);
        let mut AI = Shifted::ones(nsoil, 0.0f32);
        let mut BI = Shifted::ones(nsoil, 0.0f32);
        let mut CI = Shifted::ones(nsoil, 0.0f32);
        for _ITER in 1..=NITER {
            if water.qinsur > 0.
                && (options.opt_run == 3
                    || options.opt_run == 6
                    || options.opt_run == 7
                    || options.opt_run == 8)
            {
                surface_runoff(domain, nsoil, options, parameters, water);
            }

            srt(
                options, parameters, domain, nsoil, water, &mut RHSTT, &mut AI, &mut BI, &mut CI,
            );

            let WPLUS = sstep(
                options, parameters, domain, nsoil, water, DTFINE, &mut AI, &mut BI, &mut CI,
                &mut RHSTT,
            );

            RSAT += WPLUS;
            QDRAIN_SAVE += water.qdrain;
            RUNSRF_SAVE += water.runsrf;
        }

        water.qdrain = QDRAIN_SAVE / NITER as f32;
        water.runsrf = RUNSRF_SAVE / NITER as f32;

        water.runsrf = water.runsrf * 1000. + RSAT * 1000. / dt; // m/s -> mm/s
        water.qdrain *= 1000.;

        // removal of soil water due to groundwater flow (option 2)
        if options.opt_drn == 2 {
            let mut WTSUB = 0.0f32;
            for K in 1..=nsoil {
                WTSUB += water.wcnd[K] * dz[K];
            }

            for K in 1..=nsoil {
                let MH2O = water.runsub * dt * (water.wcnd[K] * dz[K]) / WTSUB; // mm
                water.sh2o[K] -= MH2O / (dz[K] * 1000.);
            }
        }

        // Limit MLIQ to be greater than or equal to watmin.
        // Get water needed to bring MLIQ equal WATMIN from lower layer.
        if options.opt_drn != 1 {
            let mut MLIQ = Shifted::ones(nsoil, 0.0f32);
            for IZ in 1..=nsoil {
                MLIQ[IZ] = water.sh2o[IZ] * dz[IZ] * 1000.;
            }

            let WATMIN = 0.01f32; // mm
            for IZ in 1..=nsoil - 1 {
                let XS = if MLIQ[IZ] < 0. { WATMIN - MLIQ[IZ] } else { 0. };
                MLIQ[IZ] += XS;
                MLIQ[IZ + 1] -= XS;
            }

            let IZ = nsoil;
            let XS = if MLIQ[IZ] < WATMIN {
                WATMIN - MLIQ[IZ]
            } else {
                0.
            };
            MLIQ[IZ] += XS;
            water.runsub -= XS / dt;
            if options.opt_drn == 5 {
                water.deeprech -= XS * 1.0E-3;
            }

            for IZ in 1..=nsoil {
                water.sh2o[IZ] = MLIQ[IZ] / (dz[IZ] * 1000.);
            }
        }

        // subsurface runoff using different schemes
        if options.opt_drn != 2 {
            subsurface_runoff(domain, nsoil, options, parameters, water);
        }
    }

    // Perform simplified subsurface checks when one-way coupling activated with opt_sub == 2
    if options.opt_sub == 2 {
        // Loop through each soil level and check SMC is constant
        // Preserve the ratios of SICE:SMC and SH2O:SMC
        for K in 1..=nsoil {
            if water.smc[K] != water.smc_init[K] {
                let SICE_SMC_RATIO = water.sice[K] / water.smc[K];
                water.smc[K] = water.smc_init[K];
                water.sice[K] = water.smc[K] * SICE_SMC_RATIO;
                water.sh2o[K] = water.smc[K] - water.sice[K];
            }
        }

        // Check to ensure ZWT is equal to initial water table height
        if water.zwt != parameters.ZWT_INIT {
            water.zwt = parameters.ZWT_INIT;
        }
    }
}
