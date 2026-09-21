//! Port of `src/ThermalPropertiesModule.f90` @ 0ff055e.
//!
//! Snow and soil thermal conductivity and heat capacity, layer by layer, plus the
//! `FACT` phase-change factor `THERMOPROP` derives from them.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::fortran::intrinsics as fi;
use crate::levels::Levels;
use crate::parameters::Parameters;
use crate::water::Water;

/// `THERMOPROP`.
///
/// `options` and `forcing` are in the Fortran signature and unused; they are dropped here.
pub fn thermoprop(
    domain: &mut Domain,
    levels: &Levels,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
) {
    // snow/soil layer thickness (m)
    for IZ in water.ISNOW + 1..=levels.nsoil {
        if IZ == water.ISNOW + 1 {
            domain.dzsnso[IZ] = -domain.zsnso[IZ];
        } else {
            domain.dzsnso[IZ] = domain.zsnso[IZ - 1] - domain.zsnso[IZ];
        }
    }

    // compute snow thermal conductivity and heat capacity
    csnow(domain, parameters, energy, water);

    // compute soil thermal properties
    tdfcnd(levels, parameters, energy, water);

    // Change for urban case
    if parameters.urban_flag {
        for IZ in 1..=levels.nsoil {
            energy.DF[IZ] = 3.24; // where does this number come from? KSJ 2021-04-08
        }
    }

    // compute lake thermal properties
    // (no consideration of turbulent mixing for this version)
    if domain.ist == 2 {
        for IZ in 1..=levels.nsoil {
            if energy.STC[IZ] > parameters.TFRZ {
                energy.HCPCT[IZ] = parameters.CWAT;
                energy.DF[IZ] = parameters.TKWAT; //+ KEDDY * CWAT
            } else {
                energy.HCPCT[IZ] = parameters.CICE;
                energy.DF[IZ] = parameters.TKICE;
            }
        }
    }

    // combine a temporary variable used for melting/freezing of snow and frozen soil
    for IZ in water.ISNOW + 1..=levels.nsoil {
        energy.FACT[IZ] = domain.dt / (energy.HCPCT[IZ] * domain.dzsnso[IZ]);
    }

    // snow/soil interface
    if water.ISNOW == 0 {
        energy.DF[1] = (energy.DF[1] * domain.dzsnso[1] + 0.35 * water.SNOWH)
            / (water.SNOWH + domain.dzsnso[1]);
    } else {
        energy.DF[1] = (energy.DF[1] * domain.dzsnso[1] + energy.DF[0] * domain.dzsnso[0])
            / (domain.dzsnso[0] + domain.dzsnso[1]);
    }
}

/// `CSNOW` -- snowpack properties, volumetric heat capacity and thermal conductivity.
fn csnow(domain: &Domain, parameters: &Parameters, energy: &mut Energy, water: &mut Water) {
    for IZ in water.ISNOW + 1..=0 {
        // Compute snowpack properties
        water.SNICEV[IZ] = fi::min(
            1.0,
            water.SNICE[IZ] / (domain.dzsnso[IZ] * parameters.DENICE),
        );
        water.EPORE[IZ] = 1.0 - water.SNICEV[IZ];
        water.SNLIQV[IZ] = fi::min(
            water.EPORE[IZ],
            water.SNLIQ[IZ] / (domain.dzsnso[IZ] * parameters.DENH2O),
        );
        let BDSNOI = (water.SNICE[IZ] + water.SNLIQ[IZ]) / domain.dzsnso[IZ];

        // volumetric specific heat
        let CVSNO = (parameters.CICE * water.SNICEV[IZ]) + (parameters.CWAT * water.SNLIQV[IZ]);

        // thermal conductivity of snow -- Stieglitz (Yen, 1965). `BDSNOI ** 2.0`, which
        // gfortran rewrites into a multiply.
        let TKSNO = 3.2217E-6 * fi::pow2_real(BDSNOI);

        // Assign DF and HCPCT to each snow layer
        energy.DF[IZ] = TKSNO;
        energy.HCPCT[IZ] = CVSNO;
    }
}

/// `TDFCND` -- soil thermal conductivity after Peters-Lidard (1998) / Johansen (1975).
fn tdfcnd(levels: &Levels, parameters: &Parameters, energy: &mut Energy, water: &mut Water) {
    for IZ in 1..=levels.nsoil {
        water.sice[IZ] = water.smc[IZ] - water.sh2o[IZ];
        energy.HCPCT[IZ] = water.sh2o[IZ] * parameters.CWAT
            + (1.0 - parameters.smcmax[IZ]) * parameters.csoil
            + (parameters.smcmax[IZ] - water.smc[IZ]) * parameters.CPAIR
            + water.sice[IZ] * parameters.CICE;

        let SATRATIO = water.smc[IZ] / parameters.smcmax[IZ];

        let THKS = fi::powf(parameters.THKQTZ, parameters.QUARTZ)
            * fi::powf(parameters.THKO, 1. - parameters.QUARTZ);

        // UNFROZEN VOLUME FOR SATURATION (POROSITY*XUNFROZ)
        let mut XUNFROZ = 1.0; // Prevent divide by zero (suggested by D. Mocko)
        if water.smc[IZ] > 0.0 {
            XUNFROZ = water.sh2o[IZ] / water.smc[IZ];
        }

        // SATURATED THERMAL CONDUCTIVITY
        let XU = XUNFROZ * parameters.smcmax[IZ];

        // DRY DENSITY IN KG/M3
        let THKSAT = fi::powf(THKS, 1. - parameters.smcmax[IZ])
            * fi::powf(parameters.TKICE, parameters.smcmax[IZ] - XU)
            * fi::powf(parameters.THKW, XU);

        // DRY THERMAL CONDUCTIVITY IN W.M-1.K-1
        let GAMMD = (1. - parameters.smcmax[IZ]) * 2700.;
        let THKDRY = (0.135 * GAMMD + 64.7) / (2700. - 0.947 * GAMMD);

        let AKE = if (water.sh2o[IZ] + 0.0005) < water.smc[IZ] {
            SATRATIO
        } else if SATRATIO > 0.1 {
            fi::log10(SATRATIO) + 1.0
        } else {
            0.0
        };

        energy.DF[IZ] = AKE * (THKSAT - THKDRY) + THKDRY;
    }
}
