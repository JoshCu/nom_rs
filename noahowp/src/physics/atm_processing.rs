//! Port of `src/AtmProcessing.f90` @ 0ff055e.
//!
//! Turns the eight raw forcings into the quantities the column physics actually reads: air
//! density and vapour pressure, the shortwave split, and the rain/snow partition. Everything
//! here is pointwise -- no iteration, no state carried between timesteps -- which is why it
//! goes first among the physics modules.
//!
//! # Precipitation phase
//!
//! `OPT_SNF` selects between seven partitioning schemes and Bondville exercises exactly one of
//! them, so the branches are covered by `tests/atm_sweep_vs_fortran.rs` against a sweep fixture
//! rather than by the year-long recording.

#![allow(non_snake_case)]

use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// Which precipitation field the driver writes -- upstream's `NGEN_FORCING_ACTIVE`.
///
/// The two branches are mirror images: one copies `PRCP` into `PRCPNONC`, the other copies it
/// back. Both leave the pair equal, so the choice only matters for which value is authoritative
/// when they disagree on entry -- which is every timestep, since whichever field the driver did
/// not write still holds the previous timestep's value.
///
/// Upstream settles this with a `#ifdef` at compile time. Here it is a parameter, because the
/// two configurations have to coexist: [`NonConvective`](Self::NonConvective) is what ships,
/// and [`Total`](Self::Total) is what the reference build recorded the fixtures under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecipInput {
    /// `NGEN_FORCING_ACTIVE` defined: the driver writes `PRCPNONC` through BMI `set_value`,
    /// and `PRCP` is derived from it. The shipping configuration.
    NonConvective,
    /// `NGEN_FORCING_ACTIVE` undefined: the driver writes `PRCP` -- upstream's ASCII reader --
    /// and `PRCPNONC` is derived from it. The reference build's configuration.
    Total,
}

/// `ATM`.
pub fn atm(
    options: &Options,
    parameters: &Parameters,
    forcing: &mut Forcing,
    energy: &mut Energy,
    water: &mut Water,
    precip_input: PrecipInput,
) {
    /// graupel bulk density [kg/m3]
    const RHO_GRPL: f32 = 500.0;
    /// hail bulk density [kg/m3]
    const RHO_HAIL: f32 = 917.0;
    /// solar constant for the direct irradiance check (W m-2)
    const SOLAR_CONST: f32 = 1360.0;

    // Derived variables from the forcing data.
    //
    // PAIR is a copy of SFCPRS, so `SFCPRS / PAIR` is 1.0 and THAIR is just SFCTMP. The
    // division and the power are kept because they are what the Fortran computes: the ratio is
    // NaN rather than 1.0 for a zero or infinite SFCPRS, and dropping them would quietly
    // change that.
    let PAIR = forcing.SFCPRS;
    forcing.THAIR = forcing.SFCTMP
        * fi::powf(
            forcing.SFCPRS / PAIR,
            parameters.RAIR / parameters.CPAIR,
        );
    forcing.QAIR = forcing.Q2;
    forcing.EAIR = forcing.QAIR * forcing.SFCPRS / (0.622 + (0.378 * forcing.QAIR));
    forcing.RHOAIR =
        (forcing.SFCPRS - (0.378 * forcing.EAIR)) / (parameters.RAIR * forcing.SFCTMP);

    // O2 and CO2 partial pressures. The comments upstream have these two the wrong way round;
    // the assignments themselves are right.
    forcing.O2PP = parameters.O2 * PAIR;
    forcing.CO2PP = parameters.CO2 * PAIR;

    // Starting canopy temperature and vapour pressure.
    energy.TAH = forcing.SFCTMP;
    let QV_CURR = forcing.Q2 / (1.0 - forcing.Q2);
    energy.EAH = forcing.SFCPRS * QV_CURR / (0.622 + QV_CURR);

    // Zero the incoming shortwave below the horizon, and correct the direct beam for
    // slope/aspect (e.g. Duguay 1993). The guardrails are upstream's, for sunrise/sunset
    // timing against slopes and for measurement time against model time.
    if energy.COSZ <= 0.0 || energy.COSZ_HORIZ <= 0.0 {
        forcing.SWDOWN = 0.0;
    } else {
        let dir_irr = forcing.SOLDN / energy.COSZ_HORIZ;
        if dir_irr > SOLAR_CONST {
            forcing.SWDOWN = SOLAR_CONST * energy.COSZ;
        } else {
            forcing.SWDOWN = forcing.SOLDN * (energy.COSZ / energy.COSZ_HORIZ);
        }
    }

    // Split the incoming solar into direct and diffuse, visible and near infrared.
    forcing.SOLAD[1] = forcing.SWDOWN * 0.7 * 0.5;
    forcing.SOLAD[2] = forcing.SWDOWN * 0.7 * 0.5;
    forcing.SOLAI[1] = forcing.SWDOWN * 0.3 * 0.5;
    forcing.SOLAI[2] = forcing.SWDOWN * 0.3 * 0.5;

    match precip_input {
        PrecipInput::Total => forcing.PRCPNONC = forcing.PRCP,
        PrecipInput::NonConvective => forcing.PRCP = forcing.PRCPNONC,
    }

    // Convective and large-scale fractions, used only to compute FP below.
    let (QPRECC, QPRECL) = if options.opt_snf == 4 {
        (forcing.PRCPCONV + forcing.PRCPSHCV, forcing.PRCPNONC)
    } else {
        // "should be from the atmospheric model"
        (0.10 * forcing.PRCP, 0.90 * forcing.PRCP)
    };

    // Fraction of the grid cell receiving precipitation (Niu et al. 2005).
    water.FP = 0.0;
    if QPRECC + QPRECL > 0. {
        water.FP = (QPRECC + QPRECL) / (10. * QPRECC + QPRECL);
    }

    // Relative humidity, for the two options that need it. Left uninitialised upstream for
    // every other option, and never read there; zero stands in so the value is at least
    // deterministic if that ever stops being true.
    let mut rh = 0.0;
    if options.opt_snf == 6 || options.opt_snf == 7 {
        rh = 0.263
            * forcing.SFCPRS
            * forcing.Q2
            * fi::powi(
                fi::exp(
                    (17.67 * (forcing.SFCTMP - 273.15)) / (forcing.SFCTMP - 29.65),
                ),
                -1,
            );
        // in case the estimate exceeds 100
        rh = fi::min(rh, 100.0);
    }

    // Only assigned under OPT_SNF == 4, and only read there.
    let mut prcp_frozen = 0.0;

    match options.opt_snf {
        // The Jordan (1991) SNTHERM equation.
        1 => {
            if forcing.SFCTMP > parameters.TFRZ + 2.5 {
                forcing.FPICE = 0.;
            } else if forcing.SFCTMP <= parameters.TFRZ + 0.5 {
                forcing.FPICE = 1.0;
            } else if forcing.SFCTMP <= parameters.TFRZ + 2. {
                forcing.FPICE = 1. - (-54.632 + 0.2 * forcing.SFCTMP);
            } else {
                forcing.FPICE = 0.6;
            }
        }

        // A rain-snow temperature threshold, on air temperature (2: 2.2 degC, 3: 0 degC,
        // 5: user-defined) or on wet bulb temperature (6: user-defined). The threshold itself
        // is `parameters%rain_snow_thresh`, resolved from the option in `paramRead`.
        2 | 3 | 5 | 6 => {
            let temp = if options.opt_snf == 6 {
                let tair_C = forcing.SFCTMP - parameters.TFRZ;
                let twet_C = tair_C * fi::atan(0.151977 * fi::powf(rh + 8.313659, 0.5))
                    + fi::atan(tair_C + rh)
                    - fi::atan(rh - 1.676331)
                    + ((0.00391838 * fi::powf(rh, 1.5)) * fi::atan(0.023101 * rh))
                    - 4.86035;
                twet_C + parameters.TFRZ
            } else {
                forcing.SFCTMP
            };

            if temp >= parameters.rain_snow_thresh {
                forcing.FPICE = 0.;
            } else {
                forcing.FPICE = 1.0;
            }
        }

        // Phase straight from the weather model.
        4 => {
            prcp_frozen = forcing.PRCPSNOW + forcing.PRCPGRPL + forcing.PRCPHAIL;
            if forcing.PRCPNONC > 0. && prcp_frozen > 0. {
                forcing.FPICE = fi::min(1.0, prcp_frozen / forcing.PRCPNONC);
                forcing.FPICE = fi::max(0.0, forcing.FPICE);
            } else {
                forcing.FPICE = 0.0;
            }
        }

        // The optimised binary logistic regression of Jennings et al. (2018).
        7 => {
            let tair_C = forcing.SFCTMP - parameters.TFRZ;
            let snow_prob = 1.0 / (1.0 + fi::exp(-10.04 + 1.41 * tair_C + 0.09 * rh));

            if snow_prob >= 0.5 {
                forcing.FPICE = 1.0;
            } else {
                forcing.FPICE = 0.0;
            }
        }

        // `case default` upstream: FPICE keeps whatever it already held. Unreachable from a
        // valid namelist, which bounds `precip_phase_option` to 1..7.
        _ => {}
    }

    water.rain = forcing.PRCP * (1. - forcing.FPICE);
    water.snow = forcing.PRCP * forcing.FPICE;

    // Density of new snow, Hedstrom and Pomeroy (1998), Hydrol. Processes 12, 1611-1625.
    water.bdfall = fi::min(
        120.,
        67.92 + 51.25 * fi::exp((forcing.SFCTMP - parameters.TFRZ) / 2.59),
    );

    // Mix in the other frozen hydrometeors when the weather model supplies them. Divides by
    // PRCP_FROZEN without guarding it: under OPT_SNF == 4 with no frozen precipitation this
    // is 0/0, and bdfall goes NaN. Reproduced as written.
    if options.opt_snf == 4 {
        water.bdfall = water.bdfall * (forcing.PRCPSNOW / prcp_frozen)
            + RHO_GRPL * (forcing.PRCPGRPL / prcp_frozen)
            + RHO_HAIL * (forcing.PRCPHAIL / prcp_frozen);
    }

    // Wind speed at the reference height, floored at 1 m/s. The exponent is `2.` -- a real,
    // not an integer -- which gfortran turns into a multiply; see `pow2_real`.
    forcing.UR = fi::max(
        fi::sqrt(fi::pow2_real(forcing.UU) + fi::pow2_real(forcing.VV)),
        1.,
    );
}
