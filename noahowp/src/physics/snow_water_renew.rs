//! Port of `src/SnowWaterRenew.f90` @ 0ff055e.
//!
//! `SnowFall` adds this timestep's snowfall to the pack, creating the first layer once the
//! pack is deep enough; `SnowRenew` (originally `SNOWH2O`) applies sublimation, frost and rain
//! to the surface layer and drains liquid water down through the layers.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::parameters::Parameters;
use crate::physics::snow_layer_change::combine;
use crate::water::Water;

/// `SnowFall` -- snow depth and density to account for the new snowfall.
pub fn snow_fall(domain: &mut Domain, energy: &mut Energy, water: &mut Water, forcing: &Forcing) {
    let DT = domain.dt;
    let mut NEWNODE = 0; // 0-no new layers, 1-creating new layers

    // shallow snow / no layer
    if water.ISNOW == 0 && water.QSNOW > 0. {
        water.SNOWH += water.SNOWHIN * DT;
        water.SNEQV += water.QSNOW * DT;
    }

    // creating a new layer
    if water.ISNOW == 0 && water.QSNOW > 0. && water.SNOWH >= 0.025 {
        // MB: change limit
        water.ISNOW = -1;
        NEWNODE = 1;
        domain.dzsnso[0] = water.SNOWH;
        water.SNOWH = 0.;
        energy.STC[0] = fi::min(273.16, forcing.SFCTMP); // temporary setup
        water.SNICE[0] = water.SNEQV;
        water.SNLIQ[0] = 0.;
    }

    // snow with layers
    if water.ISNOW < 0 && NEWNODE == 0 && water.QSNOW > 0. {
        let top = water.ISNOW + 1;
        water.SNICE[top] += water.QSNOW * DT;
        domain.dzsnso[top] += water.SNOWHIN * DT;
    }
}

/// `SnowRenew` -- renew the mass of ice lens (`SNICE`) and liquid (`SNLIQ`) of the surface
/// snow layer resulting from sublimation (frost) / evaporation (dew).
pub fn snow_renew(
    domain: &mut Domain,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
) {
    let DT = domain.dt;

    // for the case when SNEQV becomes '0' after 'COMBINE'
    if water.SNEQV == 0. {
        water.sice[1] += (water.QSNFRO - water.QSNSUB) * DT / (domain.dzsnso[1] * 1000.); // Barlage: SH2O->SICE v3.6
        if water.sice[1] < 0. {
            water.sh2o[1] += water.sice[1];
            water.sice[1] = 0.;
        }
    }

    // for shallow snow without a layer
    // snow surface sublimation may be larger than existing snow mass. To conserve water,
    // excessive sublimation is used to reduce soil water. Smaller time steps would tend
    // to aviod this problem.
    if water.ISNOW == 0 && water.SNEQV > 0. {
        let TEMP = water.SNEQV;
        water.SNEQV = water.SNEQV - water.QSNSUB * DT + water.QSNFRO * DT;
        let PROPOR = water.SNEQV / TEMP;
        water.SNOWH = fi::max(0., PROPOR * water.SNOWH);
        water.SNOWH = fi::min(
            fi::max(water.SNOWH, water.SNEQV / 500.0),
            water.SNEQV / 50.0,
        ); // limit adjustment to a reasonable density
        if water.SNEQV < 0. {
            water.sice[1] += water.SNEQV / (domain.dzsnso[1] * 1000.);
            water.SNEQV = 0.;
            water.SNOWH = 0.;
        }
        if water.sice[1] < 0. {
            water.sh2o[1] += water.sice[1];
            water.sice[1] = 0.;
        }
    }

    if water.SNOWH <= 1.0E-8 || water.SNEQV <= 1.0E-6 {
        water.SNOWH = 0.0;
        water.SNEQV = 0.0;
    }

    // for deep snow
    if water.ISNOW < 0 {
        // KWM added this IF statement to prevent out-of-bounds array references
        let top = water.ISNOW + 1;
        let WGDIF = water.SNICE[top] - water.QSNSUB * DT + water.QSNFRO * DT;
        water.SNICE[top] = WGDIF;
        if WGDIF < 1.0e-6 && water.ISNOW < 0 {
            combine(domain, parameters, energy, water);
        }
        // KWM:  Subroutine COMBINE can change ISNOW to make it 0 again?
        if water.ISNOW < 0 {
            // KWM added this IF statement to prevent out-of-bounds array references
            let top = water.ISNOW + 1;
            water.SNLIQ[top] += water.QRAIN * DT;
            water.SNLIQ[top] = fi::max(0., water.SNLIQ[top]);
        }
    } // KWM  -- Can the ENDIF be moved toward the end of the subroutine (Just set QSNBOT=0)?

    let dz = &mut domain.dzsnso;

    // Porosity and partial volume. The Fortran's `EPORE` here is a local, not `water%EPORE`.
    let mut EPORE = crate::layers::Shifted::new(dz.lo(), 0, 0.0f32);
    for J in water.ISNOW + 1..=0 {
        let VOL_ICE = fi::min(1., water.SNICE[J] / (dz[J] * parameters.DENICE));
        EPORE[J] = 1. - VOL_ICE;
    }

    let mut QIN = 0.0f32;
    let mut QOUT = 0.0f32;
    for J in water.ISNOW + 1..=0 {
        water.SNLIQ[J] += QIN;
        let VOL_LIQ = water.SNLIQ[J] / (dz[J] * parameters.DENH2O);
        QOUT = fi::max(0., (VOL_LIQ - parameters.SSI * EPORE[J]) * dz[J]);
        // New snow water retention code
        if J == 0 {
            QOUT = fi::max(
                (VOL_LIQ - EPORE[J]) * dz[J],
                parameters.SNOW_RET_FAC * DT * QOUT,
            );
        }
        QOUT *= parameters.DENH2O;
        water.SNLIQ[J] -= QOUT;
        // New snow water retention code
        if (water.SNLIQ[J] / (water.SNICE[J] + water.SNLIQ[J])) > parameters.max_liq_mass_fraction {
            QOUT += water.SNLIQ[J]
                - parameters.max_liq_mass_fraction / (1.0 - parameters.max_liq_mass_fraction)
                    * water.SNICE[J];
            water.SNLIQ[J] = parameters.max_liq_mass_fraction
                / (1.0 - parameters.max_liq_mass_fraction)
                * water.SNICE[J];
        }
        QIN = QOUT;
    }
    // New snow water retention code
    for J in water.ISNOW + 1..=0 {
        dz[J] = fi::max(
            dz[J],
            water.SNLIQ[J] / parameters.DENH2O + water.SNICE[J] / parameters.DENICE,
        );
    }

    // Liquid water from snow bottom to soil
    water.QSNBOT = QOUT / DT; // mm/s
}
