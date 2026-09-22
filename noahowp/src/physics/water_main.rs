//! Port of `src/WaterModule.f90` @ 0ff055e.
//!
//! The last of the five physics calls: canopy water, the snowpack, and the soil column.
//!
//! `WaterMain` also keeps a soil water budget -- `tw0`, `smcold`, `acsrf`, `acsub`, `acpcp`,
//! `dtheta_max`, `totalwat`, `errwat` -- in locals that nothing reads and nothing returns.
//! Several of them carry initialisers, which makes them implicitly `SAVE`d, but a saved value
//! is still only visible to `WaterMain` itself, and `WaterMain` overwrites each one before
//! reading it. None of it is reproduced.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::levels::Levels;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::canopy_water::canopy_hydrology;
use crate::physics::snow_water::snow_water;
use crate::physics::soil_water::soil_water;
use crate::water::Water;

/// `WaterMain`.
// The frozen flags negate a `>` on purpose; see where they are set.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn water_main(
    domain: &mut Domain,
    levels: &Levels,
    options: &Options,
    parameters: &Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &mut Water,
) {
    let nsoil = levels.nsoil;

    // Below 4 computations moved from main level of noahmp_sflx in old model to WaterModule here

    // Compute layer ice content and previous time step's snow water equivalent
    for K in 1..=nsoil {
        water.sice[K] = fi::max(0.0, water.smc[K] - water.sh2o[K]);
    }
    water.SNEQVO = water.SNEQV;

    // Convert energy flux FGEV (w/m2) to evaporation/dew rate (mm/s)
    water.QVAP = fi::max(energy.FGEV / energy.LATHEAG, 0.); // positive part of fgev; Barlage change to ground v3.6
    water.QDEW = fi::min(energy.FGEV / energy.LATHEAG, 0.).abs(); // negative part of fgev

    // determine frozen canopy and/or ground. Kept as upstream spells it rather than as `<=`:
    // the two differ on a NaN temperature, which this branch calls frozen.
    energy.FROZEN_CANOPY = !(energy.TV > parameters.TFRZ);
    energy.FROZEN_GROUND = !(energy.TG > parameters.TFRZ);

    //---------------------------------------------------------------------
    // call the canopy water routines
    //---------------------------------------------------------------------

    canopy_hydrology(domain, parameters, energy, water); // original CANWATER

    //---------------------------------------------------------------------
    // call the snow water routines
    //---------------------------------------------------------------------

    // sublimation, frost, evaporation, and dew
    water.QSNSUB = 0.0;
    if water.SNEQV > 0. {
        water.QSNSUB = fi::min(water.QVAP, water.SNEQV / domain.dt);
    }
    water.qseva = water.QVAP - water.QSNSUB;
    water.QSNFRO = 0.0;
    if water.SNEQV > 0. {
        water.QSNFRO = water.QDEW;
    }
    water.QSDEW = water.QDEW - water.QSNFRO;

    snow_water(domain, levels, parameters, energy, water, forcing);

    if energy.FROZEN_GROUND {
        water.sice[1] += (water.QSDEW - water.qseva) * domain.dt / (domain.dzsnso[1] * 1000.);
        water.QSDEW = 0.0;
        water.qseva = 0.0;
        if water.sice[1] < 0. {
            water.sh2o[1] += water.sice[1];
            water.sice[1] = 0.;
        }
    }

    // convert units (mm/s -> m/s)
    // PONDING: melting water from snow when there is no layer
    water.qinsur = (water.PONDING + water.PONDING1 + water.PONDING2) / domain.dt * 0.001;
    water.ACSNOM = water.PONDING + water.PONDING1 + water.PONDING2 + (water.QSNBOT * domain.dt);

    //    QINSUR = PONDING/DT * 0.001
    if water.ISNOW == 0 {
        water.qinsur += (water.QSNBOT + water.QSDEW + water.QRAIN) * 0.001;
    } else {
        water.qinsur += (water.QSNBOT + water.QSDEW) * 0.001;
    }
    water.qseva *= 0.001;

    // For vegetation root
    for IZ in 1..=parameters.NROOT {
        water.etrani[IZ] = water.ETRAN * water.BTRANI[IZ] * 0.001;
    }

    // For lake points
    if domain.ist == 2 {
        // lake
        water.runsrf = 0.;
        if water.WSLAKE >= parameters.WSLMAX {
            water.runsrf = water.qinsur * 1000.; // mm/s
        }
        water.WSLAKE = water.WSLAKE + (water.qinsur - water.qseva) * 1000. * domain.dt
            - water.runsrf * domain.dt; // mm
    } else {
        // soil
        soil_water(domain, levels, options, parameters, water);
        // did not include groundwater part
    }

    // Compute total soil moisture content
    for K in 1..=nsoil {
        water.smc[K] = water.sh2o[K] + water.sice[K];
    }

    // Compute evapotranspiration
    water.EVAPOTRANS = water.qseva + (water.ETRAN * 0.001);

    //---------------------------------------------------------------------
    // accumulate some fields and error checks when opt_sub == 1
    //---------------------------------------------------------------------
    if options.opt_sub == 1 {
        water.runsub += water.snoflow; // add glacier outflow to subsurface runoff [mm/s]
    }
}
