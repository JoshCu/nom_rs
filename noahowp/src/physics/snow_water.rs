//! Port of `src/SnowWaterModule.f90` @ 0ff055e.
//!
//! `SnowWater`: snowfall, compaction, layer combination and division, the surface layer's
//! water renewal, the glacier cap, and the rebuilt layer geometry.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::levels::Levels;
use crate::parameters::Parameters;
use crate::physics::snow_layer_change::{combine, compact, divide};
use crate::physics::snow_water_renew::{snow_fall, snow_renew};
use crate::water::Water;

/// `SnowWater`.
pub fn snow_water(
    domain: &mut Domain,
    levels: &Levels,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
    forcing: &Forcing,
) {
    let nsnow = levels.nsnow;
    let nsoil = levels.nsoil;

    // initialization
    let realMissing = -999999.0f32;
    water.snoflow = 0.0;
    water.PONDING1 = 0.0;
    water.PONDING2 = 0.0;

    snow_fall(domain, energy, water, forcing);

    // MB: do each if block separately
    if water.ISNOW < 0 {
        // when multi-layer
        compact(domain, parameters, energy, water);
    }

    if water.ISNOW < 0 {
        // when multi-layer
        combine(domain, parameters, energy, water);
    }

    if water.ISNOW < 0 {
        // when multi-layer
        divide(domain, nsnow, parameters, energy, water);
    }

    snow_renew(domain, parameters, energy, water); // originally named SNOWH2O

    // set empty snow layers to zero
    for iz in -nsnow + 1..=water.ISNOW {
        water.SNICE[iz] = 0.;
        water.SNLIQ[iz] = 0.;
        energy.STC[iz] = 0.;
        domain.dzsnso[iz] = 0.;
        domain.zsnso[iz] = 0.;
    }

    // to obtain equilibrium state of snow in glacier region
    if water.SNEQV > 5000. {
        // 5000 mm -> maximum water depth
        let BDSNOW = water.SNICE[0] / domain.dzsnso[0];
        water.snoflow = water.SNEQV - 5000.;
        water.SNICE[0] -= water.snoflow;
        domain.dzsnso[0] -= water.snoflow / BDSNOW;
        water.snoflow /= domain.dt;
    }

    // sum up snow mass for layered snow
    if water.ISNOW < 0 {
        // MB: only do for multi-layer
        water.SNEQV = 0.;
        for IZ in water.ISNOW + 1..=0 {
            water.SNEQV = water.SNEQV + water.SNICE[IZ] + water.SNLIQ[IZ];
        }
    }

    // Reset ZSNSO and layer thinkness DZSNSO
    let dz = &mut domain.dzsnso;
    let z = &mut domain.zsnso;
    for IZ in water.ISNOW + 1..=0 {
        dz[IZ] = -dz[IZ];
    }
    dz[1] = domain.zsoil[1];
    for IZ in 2..=nsoil {
        dz[IZ] = domain.zsoil[IZ] - domain.zsoil[IZ - 1];
    }
    z[water.ISNOW + 1] = dz[water.ISNOW + 1];
    for IZ in water.ISNOW + 2..=nsoil {
        z[IZ] = z[IZ - 1] + dz[IZ];
    }
    for IZ in water.ISNOW + 1..=nsoil {
        dz[IZ] = -dz[IZ];
    }

    // NWM3.0 parameter
    let mut mass = 0.0f32;
    for iz in -nsnow + 1..=0 {
        mass += water.SNICE[iz] + water.SNLIQ[iz];
    }
    if mass > 0. {
        let mut weighted = 0.0f32;
        for iz in -nsnow + 1..=0 {
            weighted += energy.STC[iz] * (water.SNICE[iz] + water.SNLIQ[iz]);
        }
        energy.SNOWT_AVG = weighted / mass;
    } else {
        energy.SNOWT_AVG = realMissing;
    }
}
