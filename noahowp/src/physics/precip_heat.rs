//! Port of `src/PrecipHeatModule.f90` @ 0ff055e.
//!
//! The heat advected by rain and snow -- air to canopy, canopy to ground, air to ground --
//! split into the vegetated, under-canopy and bare-ground terms `EnergyMain` weights.

#![allow(non_snake_case)]

use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::parameters::Parameters;
use crate::water::Water;

/// `PRECIP_HEAT`.
pub fn precip_heat(parameters: &Parameters, forcing: &Forcing, energy: &mut Energy, water: &Water) {
    energy.PAHV = 0.;
    energy.PAHG = 0.;
    energy.PAHB = 0.;
    energy.PAH = 0.;

    // --------------------------- liquid water ------------------------------
    // heat transported by liquid water
    let mut PAH_AC =
        parameters.FVEG * water.rain * (parameters.CWAT / 1000.0) * (forcing.SFCTMP - energy.TV);
    let mut PAH_CG = water.QDRIPR * (parameters.CWAT / 1000.0) * (energy.TV - energy.TG);
    let mut PAH_AG = water.QTHROR * (parameters.CWAT / 1000.0) * (forcing.SFCTMP - energy.TG);

    // --------------------------- canopy ice ------------------------------
    // heat transported by snow/ice
    PAH_AC +=
        parameters.FVEG * water.snow * (parameters.CICE / 1000.0) * (forcing.SFCTMP - energy.TV);
    PAH_CG += water.QDRIPS * (parameters.CICE / 1000.0) * (energy.TV - energy.TG);
    PAH_AG += water.QTHROS * (parameters.CICE / 1000.0) * (forcing.SFCTMP - energy.TG);

    energy.PAHV = PAH_AC - PAH_CG;
    energy.PAHG = PAH_CG;
    energy.PAHB = PAH_AG;

    if parameters.FVEG > 0.0 && parameters.FVEG < 1.0 {
        energy.PAHG /= parameters.FVEG; // these will be multiplied by fraction later
        energy.PAHB /= 1.0 - parameters.FVEG;
    } else if parameters.FVEG <= 0.0 {
        energy.PAHB += energy.PAHG; // for case of canopy getting buried
        energy.PAHG = 0.0;
        energy.PAHV = 0.0;
    } else if parameters.FVEG >= 1.0 {
        energy.PAHB = 0.0;
    }

    // Put some artificial limits here for stability
    energy.PAHV = fi::max(energy.PAHV, -20.0);
    energy.PAHV = fi::min(energy.PAHV, 20.0);
    energy.PAHG = fi::max(energy.PAHG, -20.0);
    energy.PAHG = fi::min(energy.PAHG, 20.0);
    energy.PAHB = fi::max(energy.PAHB, -20.0);
    energy.PAHB = fi::min(energy.PAHB, 20.0);
}
