//! Port of `src/InterceptionModule.f90` @ 0ff055e.
//!
//! The third of the five physics calls: update the canopy for the season and the snow burying
//! it, then route this timestep's rain and snow through it.
//!
//! # `parameters` is not all parameters
//!
//! `PHENOLOGY` writes `parameters.LAI`, `SAI`, `ELAI`, `ESAI` and `FVEG`. Those five are
//! per-timestep state that happens to live in the parameter struct -- recomputed from the day
//! of year and the snow depth on every call -- so this module takes `&mut Parameters`. The
//! fixture records them per record for the same reason; see `reference/gen_serializer.py`.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// `InterceptionMain`.
pub fn interception_main(
    domain: &Domain,
    options: &Options,
    parameters: &mut Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &mut Water,
) {
    phenology(domain, options, parameters, forcing, energy, water);
    canopy_water_intercept(domain, parameters, forcing, energy, water);
}

/// `PHENOLOGY` -- adjust LAI and SAI for the season and for the snow burying the canopy.
///
/// `levels` is in the Fortran signature and unused in the body; it is dropped here.
fn phenology(
    domain: &Domain,
    options: &Options,
    parameters: &mut Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &Water,
) {
    // Adjust LAI and SAI depending on the day of year.
    if domain.croptype == 0 {
        if options.dveg == 1 || options.dveg == 3 || options.dveg == 4 {
            let DAY = if domain.lat >= 0. {
                // Northern Hemisphere.
                forcing.JULIAN
            } else {
                // Southern Hemisphere: DAY is shifted by half a year.
                fi::modulo_trunc(
                    forcing.JULIAN + (0.5 * forcing.YEARLEN as f32),
                    forcing.YEARLEN as f32,
                )
            };
            let T = 12. * DAY / forcing.YEARLEN as f32;
            // Real to integer on assignment: truncation toward zero, not rounding. For
            // `T + 0.5` with T non-negative that is the same as rounding to nearest, which is
            // what the expression is reaching for -- but the two part company for negative T,
            // and nothing here rules that out.
            let mut IT1 = fi::int(T + 0.5);
            let mut IT2 = IT1 + 1;
            let WT1 = (IT1 as f32 + 0.5) - T;
            let WT2 = 1. - WT1;
            if IT1 < 1 {
                IT1 = 12;
            }
            if IT2 > 12 {
                IT2 = 1;
            }
            parameters.LAI = (WT1 * parameters.LAIM[IT1]) + (WT2 * parameters.LAIM[IT2]);
            parameters.SAI = (WT1 * parameters.SAIM[IT1]) + (WT2 * parameters.SAIM[IT2]);
        }
        if options.dveg == 7 || options.dveg == 8 || options.dveg == 9 {
            // When LAI is read in, SAI is 10% of it but never below 0.05.
            parameters.SAI = fi::max(0.05, 0.1 * parameters.LAI);
            if parameters.LAI < 0.05 {
                parameters.SAI = 0.0;
            }
        }
        if parameters.SAI < 0.05 {
            parameters.SAI = 0.0;
        }
        if parameters.LAI < 0.05 || parameters.SAI == 0.0 {
            parameters.LAI = 0.0;
        }
        if domain.vegtyp == parameters.ISWATER
            || domain.vegtyp == parameters.ISBARREN
            || domain.vegtyp == parameters.ISICE
            || parameters.urban_flag
        {
            parameters.LAI = 0.;
            parameters.SAI = 0.;
        }
    }

    // How much of the canopy the snow has buried.
    let DB = fi::min(
        fi::max(water.SNOWH - parameters.HVB, 0.),
        parameters.HVT - parameters.HVB,
    );
    let mut FB = DB / fi::max(1.0E-06, parameters.HVT - parameters.HVB);

    if parameters.HVT > 0. && parameters.HVT <= 1.0 {
        let SNOWHC = parameters.HVT * fi::exp(-water.SNOWH / 0.2);
        FB = fi::min(water.SNOWH, SNOWHC) / SNOWHC;
    }

    parameters.ELAI = parameters.LAI * (1. - FB);
    parameters.ESAI = parameters.SAI * (1. - FB);
    if parameters.ESAI < 0.05 && domain.croptype == 0 {
        parameters.ESAI = 0.0;
    }
    if (parameters.ELAI < 0.05 || parameters.ESAI == 0.0) && domain.croptype == 0 {
        parameters.ELAI = 0.0;
    }

    // Growing season flag. Upstream's crop branch -- `PGS > 2 .and. PGS < 7 .and. croptype > 0`
    // -- is commented out there because carbon_crop is not implemented.
    if energy.TV > parameters.TMIN && domain.croptype == 0 {
        energy.IGS = 1.;
    } else {
        energy.IGS = 0.;
    }

    // Vegetated fraction of the cell.
    if options.dveg == 1 || options.dveg == 6 || options.dveg == 7 {
        parameters.FVEG = parameters.SHDFAC;
        if parameters.FVEG <= 0.05 {
            parameters.FVEG = 0.05;
        }
    } else if options.dveg == 2 || options.dveg == 3 || options.dveg == 8 {
        parameters.FVEG = 1. - fi::exp(-0.52 * (parameters.LAI + parameters.SAI));
        if parameters.FVEG <= 0.05 {
            parameters.FVEG = 0.05;
        }
    } else if options.dveg == 4 || options.dveg == 5 || options.dveg == 9 {
        parameters.FVEG = parameters.SHDMAX;
        if parameters.FVEG <= 0.05 {
            parameters.FVEG = 0.05;
        }
    }
    if domain.croptype > 0 {
        parameters.FVEG = parameters.SHDMAX;
        if parameters.FVEG <= 0.05 {
            parameters.FVEG = 0.05;
        }
    }
    if parameters.urban_flag || domain.vegtyp == parameters.ISBARREN {
        parameters.FVEG = 0.0;
    }
    if parameters.ELAI + parameters.ESAI == 0.0 {
        parameters.FVEG = 0.0;
    }
}

/// `CanopyWaterIntercept` -- canopy interception, drip and throughfall for snow and liquid.
fn canopy_water_intercept(
    domain: &Domain,
    parameters: &Parameters,
    forcing: &Forcing,
    energy: &Energy,
    water: &mut Water,
) {
    water.QINTR = 0.;
    water.QDRIPR = 0.;
    water.QTHROR = 0.;
    water.QINTR = 0.;
    water.QINTS = 0.;
    water.QDRIPS = 0.;
    water.QTHROS = 0.;
    water.QRAIN = 0.0;
    water.QSNOW = 0.0;
    water.SNOWHIN = 0.0;
    // Upstream also zeroes ICEDRIP here. It is read only inside the branch that assigns it,
    // so the initialiser is dead and the variable is declared where it is used instead.

    // ------------------------- liquid water -------------------------

    // Canopy capacity for rain.
    let MAXLIQ = parameters.CH2OP * (parameters.ELAI + parameters.ESAI);

    if (parameters.ELAI + parameters.ESAI) > 0. {
        // Interception capability.
        water.QINTR = parameters.FVEG * water.rain * water.FP;
        water.QINTR = fi::min(
            water.QINTR,
            (MAXLIQ - water.canliq) / domain.dt
                * (1. - fi::exp(-water.rain * domain.dt / MAXLIQ)),
        );
        water.QINTR = fi::max(water.QINTR, 0.);
        water.QDRIPR = parameters.FVEG * water.rain - water.QINTR;
        water.QTHROR = (1. - parameters.FVEG) * water.rain;
        water.canliq = fi::max(0., water.canliq + water.QINTR * domain.dt);
    } else {
        water.QINTR = 0.;
        water.QDRIPR = 0.;
        water.QTHROR = water.rain;
        // For the case of the canopy getting buried.
        if water.canliq > 0. {
            water.QDRIPR += water.canliq / domain.dt;
            water.canliq = 0.0;
        }
    }

    // --------------------------- canopy ice ---------------------------

    let MAXSNO = 6.6 * (0.27 + 46. / water.bdfall) * (parameters.ELAI + parameters.ESAI);

    if (parameters.ELAI + parameters.ESAI) > 0. {
        water.QINTS = parameters.FVEG * water.snow * water.FP;
        water.QINTS = fi::min(
            water.QINTS,
            (MAXSNO - water.canice) / domain.dt
                * (1. - fi::exp(-water.snow * domain.dt / MAXSNO)),
        );
        water.QINTS = fi::max(water.QINTS, 0.);
        let FT = fi::max(0.0, (energy.TV - 270.15) / 1.87E5);
        // Written out as a multiply upstream, not as `UU ** 2.` the way `ATM` writes the same
        // quantity. The two are different computations, so this stays a multiply.
        let FV = fi::sqrt(forcing.UU * forcing.UU + forcing.VV * forcing.VV) / 1.56E5;
        let ICEDRIP = fi::max(0., water.canice) * (FV + FT);
        water.QDRIPS = (parameters.FVEG * water.snow - water.QINTS) + ICEDRIP;
        water.QTHROS = (1.0 - parameters.FVEG) * water.snow;
        water.canice = fi::max(0., water.canice + (water.QINTS - ICEDRIP) * domain.dt);
    } else {
        water.QINTS = 0.;
        water.QDRIPS = 0.;
        water.QTHROS = water.snow;
        // For the case of the canopy getting buried.
        if water.canice > 0. {
            water.QDRIPS += water.canice / domain.dt;
            water.canice = 0.0;
        }
    }

    // Wetted fraction of the canopy.
    if water.canice > 0. {
        water.FWET = fi::max(0., water.canice) / fi::max(MAXSNO, 1.0E-06);
    } else {
        water.FWET = fi::max(0., water.canliq) / fi::max(MAXLIQ, 1.0E-06);
    }
    water.FWET = fi::powf(fi::min(water.FWET, 1.), 0.667);

    water.CMC = water.canliq + water.canice;

    // Rain or snow reaching the ground.
    water.QRAIN = water.QDRIPR + water.QTHROR;
    water.QSNOW = water.QDRIPS + water.QTHROS;
    water.SNOWHIN = water.QSNOW / water.bdfall;
    if domain.ist == 2 && energy.TG > parameters.TFRZ {
        water.QSNOW = 0.;
        water.SNOWHIN = 0.;
    }
}
