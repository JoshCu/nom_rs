//! Port of `src/CanopyWaterModule.f90` @ 0ff055e.
//!
//! `CanopyHydrology` (originally `CANWATER`): evaporation, dew, sublimation and frost on the
//! intercepted canopy water, and the phase change between canopy liquid and ice.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::fortran::intrinsics as fi;
use crate::parameters::Parameters;
use crate::water::Water;

/// `CanopyHydrology`.
pub fn canopy_hydrology(
    domain: &Domain,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
) {
    let DT = domain.dt;

    // initialization
    water.ECAN = 0.0;

    // --------------------------- liquid water ------------------------------

    // maximum canopy water
    let MAXLIQ = parameters.CH2OP * (parameters.ELAI + parameters.ESAI);

    // evaporation, transpiration, and dew
    let mut QEVAC;
    let QDEWC;
    let mut QSUBC;
    let QFROC;
    if !energy.FROZEN_CANOPY {
        // Barlage: change to frozen_canopy
        water.ETRAN = fi::max(energy.FCTR / parameters.HVAP, 0.);
        QEVAC = fi::max(energy.FCEV / parameters.HVAP, 0.);
        QDEWC = fi::min(energy.FCEV / parameters.HVAP, 0.).abs();
        QSUBC = 0.;
        QFROC = 0.;
    } else {
        water.ETRAN = fi::max(energy.FCTR / parameters.HSUB, 0.);
        QEVAC = 0.;
        QDEWC = 0.;
        QSUBC = fi::max(energy.FCEV / parameters.HSUB, 0.);
        QFROC = fi::min(energy.FCEV / parameters.HSUB, 0.).abs();
    }

    // canopy water balance. for convenience allow dew to bring CANLIQ above
    // maxh2o or else would have to re-adjust drip
    QEVAC = fi::min(water.canliq / DT, QEVAC);
    water.canliq = fi::max(0., water.canliq + (QDEWC - QEVAC) * DT);
    if water.canliq <= 1.0E-06 {
        water.canliq = 0.0;
    }

    // --------------------------- canopy ice ------------------------------

    // for canopy ice
    let MAXSNO = 6.6 * (0.27 + 46. / water.bdfall) * (parameters.ELAI + parameters.ESAI);
    QSUBC = fi::min(water.canice / DT, QSUBC);
    water.canice = fi::max(0., water.canice + (QFROC - QSUBC) * DT);
    if water.canice <= 1.0E-6 {
        water.canice = 0.;
    }

    // wetted fraction of canopy
    if water.canice > 0. {
        water.FWET = fi::max(0., water.canice) / fi::max(MAXSNO, 1.0E-06);
    } else {
        water.FWET = fi::max(0., water.canliq) / fi::max(MAXLIQ, 1.0E-06);
    }
    water.FWET = fi::powf(fi::min(water.FWET, 1.), 0.667);

    // phase change
    if water.canice > 1.0E-6 && energy.TV > parameters.TFRZ {
        let QMELTC = fi::min(
            water.canice / DT,
            (energy.TV - parameters.TFRZ) * parameters.CICE * water.canice
                / parameters.DENICE
                / (DT * parameters.HFUS),
        );
        water.canice = fi::max(0., water.canice - QMELTC * DT);
        water.canliq = fi::max(0., water.canliq + QMELTC * DT);
        energy.TV = water.FWET * parameters.TFRZ + (1. - water.FWET) * energy.TV;
    }

    if water.canliq > 1.0E-6 && energy.TV < parameters.TFRZ {
        let QFRZC = fi::min(
            water.canliq / DT,
            (parameters.TFRZ - energy.TV) * parameters.CWAT * water.canliq
                / parameters.DENH2O
                / (DT * parameters.HFUS),
        );
        water.canliq = fi::max(0., water.canliq - QFRZC * DT);
        water.canice = fi::max(0., water.canice + QFRZC * DT);
        energy.TV = water.FWET * parameters.TFRZ + (1. - water.FWET) * energy.TV;
    }

    // total canopy water
    water.CMC = water.canliq + water.canice;

    // total canopy evaporation
    water.ECAN = QEVAC + QSUBC - QDEWC - QFROC;
}
