//! Port of `src/SnowSoilTempModule.f90` @ 0ff055e.
//!
//! `TSNOSOI` solves the snow/soil heat diffusion implicitly (`HRT` builds the tridiagonal
//! system, `HSTEP` solves it through `ROSR12`); `PHASECHANGE` then melts and freezes snow and
//! soil water against the resulting temperatures.

#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::layers::Shifted;
use crate::levels::Levels;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// `TSNOSOI` -- snow (up to 3L) and soil (4L) temperature.
///
/// Snow temperatures during the melting season may exceed the melting point; `PHASECHANGE`
/// resets them afterwards.
///
/// The Fortran computes `EFLXB2` and a balance-check `TBEG` and then `return`s before using
/// either ("skip the energy balance check for now"). Neither reaches any state, so neither
/// is reproduced. `ICE`, `SAG` and `TG` are likewise only read by the skipped check.
pub fn tsnosoi(
    parameters: &Parameters,
    levels: &Levels,
    domain: &Domain,
    options: &Options,
    forcing: &Forcing,
    ISNOW: i32,
    SSOIL: f32,
    DF: &Shifted<f32>,
    HCPCT: &Shifted<f32>,
    SNOWH: f32,
    STC: &mut Shifted<f32>,
) {
    let _ = forcing;
    let nsnow = levels.nsnow;
    let nsoil = levels.nsoil;

    // compute solar penetration through water, needs more work
    let PHI: Shifted<f32> = Shifted::snow_soil(nsnow, nsoil, 0.);

    // adjust ZBOT from soil surface to ZBOTSNO from snow surface
    let ZBOTSNO = parameters.ZBOT - SNOWH; // from snow surface

    let mut AI = Shifted::snow_soil(nsnow, nsoil, 0.0f32);
    let mut BI = Shifted::snow_soil(nsnow, nsoil, 0.0f32);
    let mut CI = Shifted::snow_soil(nsnow, nsoil, 0.0f32);
    let mut RHSTS = Shifted::snow_soil(nsnow, nsoil, 0.0f32);

    // compute soil temperatures
    let _EFLXB = hrt(
        parameters, domain, options, nsoil, ISNOW, STC, ZBOTSNO, DF, HCPCT, SSOIL, &PHI,
        &mut AI, &mut BI, &mut CI, &mut RHSTS,
    );

    hstep(nsoil, ISNOW, domain.dt, &mut AI, &mut BI, &mut CI, &mut RHSTS, STC);
}

/// `HRT` -- the right-hand side of the soil thermal diffusion equation and the tridiagonal
/// matrix coefficients of the implicit scheme. Returns `BOTFLX`.
fn hrt(
    parameters: &Parameters,
    domain: &Domain,
    options: &Options,
    NSOIL: i32,
    ISNOW: i32,
    STC: &Shifted<f32>,
    ZBOT: f32,
    DF: &Shifted<f32>,
    HCPCT: &Shifted<f32>,
    SSOIL: f32,
    PHI: &Shifted<f32>,
    AI: &mut Shifted<f32>,
    BI: &mut Shifted<f32>,
    CI: &mut Shifted<f32>,
    RHSTS: &mut Shifted<f32>,
) -> f32 {
    let ZSNSO = &domain.zsnso;
    let lo = STC.lo();
    let mut DDZ = Shifted::new(lo, NSOIL, 0.0f32);
    let mut DENOM = Shifted::new(lo, NSOIL, 0.0f32);
    let mut DTSDZ = Shifted::new(lo, NSOIL, 0.0f32);
    let mut EFLUX = Shifted::new(lo, NSOIL, 0.0f32);
    // `intent(out)`, assigned only under OPT_TBOT 1 or 2; the namelist check allows nothing
    // else.
    let mut BOTFLX = 0.0f32;

    for K in ISNOW + 1..=NSOIL {
        if K == ISNOW + 1 {
            DENOM[K] = -ZSNSO[K] * HCPCT[K];
            let TEMP1 = -ZSNSO[K + 1];
            DDZ[K] = 2.0 / TEMP1;
            DTSDZ[K] = 2.0 * (STC[K] - STC[K + 1]) / TEMP1;
            EFLUX[K] = DF[K] * DTSDZ[K] - SSOIL - PHI[K];
        } else if K < NSOIL {
            DENOM[K] = (ZSNSO[K - 1] - ZSNSO[K]) * HCPCT[K];
            let TEMP1 = ZSNSO[K - 1] - ZSNSO[K + 1];
            DDZ[K] = 2.0 / TEMP1;
            DTSDZ[K] = 2.0 * (STC[K] - STC[K + 1]) / TEMP1;
            EFLUX[K] = (DF[K] * DTSDZ[K] - DF[K - 1] * DTSDZ[K - 1]) - PHI[K];
        } else if K == NSOIL {
            DENOM[K] = (ZSNSO[K - 1] - ZSNSO[K]) * HCPCT[K];
            let _TEMP1 = ZSNSO[K - 1] - ZSNSO[K];
            if options.opt_tbot == 1 {
                BOTFLX = 0.;
            }
            if options.opt_tbot == 2 {
                DTSDZ[K] =
                    (STC[K] - parameters.TBOT) / (0.5 * (ZSNSO[K - 1] + ZSNSO[K]) - ZBOT);
                BOTFLX = -DF[K] * DTSDZ[K];
            }
            EFLUX[K] = (-BOTFLX - DF[K - 1] * DTSDZ[K - 1]) - PHI[K];
        }
    }

    for K in ISNOW + 1..=NSOIL {
        if K == ISNOW + 1 {
            AI[K] = 0.0;
            CI[K] = -DF[K] * DDZ[K] / DENOM[K];
            if options.opt_stc == 1 || options.opt_stc == 3 {
                BI[K] = -CI[K];
            }
            if options.opt_stc == 2 {
                BI[K] = -CI[K] + DF[K] / (0.5 * ZSNSO[K] * ZSNSO[K] * HCPCT[K]);
            }
        } else if K < NSOIL {
            AI[K] = -DF[K - 1] * DDZ[K - 1] / DENOM[K];
            CI[K] = -DF[K] * DDZ[K] / DENOM[K];
            BI[K] = -(AI[K] + CI[K]);
        } else if K == NSOIL {
            AI[K] = -DF[K - 1] * DDZ[K - 1] / DENOM[K];
            CI[K] = 0.0;
            BI[K] = -(AI[K] + CI[K]);
        }
        RHSTS[K] = EFLUX[K] / (-DENOM[K]);
    }

    BOTFLX
}

/// `HSTEP` -- calculate/update the soil temperature field.
fn hstep(
    NSOIL: i32,
    ISNOW: i32,
    DT: f32,
    AI: &mut Shifted<f32>,
    BI: &mut Shifted<f32>,
    CI: &mut Shifted<f32>,
    RHSTS: &mut Shifted<f32>,
    STC: &mut Shifted<f32>,
) {
    for K in ISNOW + 1..=NSOIL {
        RHSTS[K] *= DT;
        AI[K] *= DT;
        BI[K] = 1. + BI[K] * DT;
        CI[K] *= DT;
    }

    // copy values for input variables before call to rosr12
    let RHSTSIN = RHSTS.clone();
    let mut CIIN = CI.clone();

    // solve the tri-diagonal matrix equation
    rosr12(AI, BI, &RHSTSIN, ISNOW + 1, NSOIL, &mut CIIN, CI, RHSTS);

    // update snow & soil temperature
    for K in ISNOW + 1..=NSOIL {
        STC[K] += CI[K];
    }
}

/// `ROSR12` -- invert (solve) the tridiagonal matrix problem.
///
/// The same routine appears twice upstream -- here and in `SoilWaterMovement.f90`, where it
/// runs over `1:nsoil` -- with identical bodies; both call this one. `A`, `B`, `D` are inputs,
/// `C`, `P`, `DELTA` are outputs (`C(NSOIL)` is zeroed on entry, which is why `C` is `&mut`).
pub(crate) fn rosr12(
    A: &Shifted<f32>,
    B: &Shifted<f32>,
    D: &Shifted<f32>,
    NTOP: i32,
    NSOIL: i32,
    C: &mut Shifted<f32>,
    P: &mut Shifted<f32>,
    DELTA: &mut Shifted<f32>,
) {
    // INITIALIZE EQN COEF C FOR THE LOWEST SOIL LAYER
    C[NSOIL] = 0.0;
    P[NTOP] = -C[NTOP] / B[NTOP];
    // SOLVE THE COEFS FOR THE 1ST SOIL LAYER
    DELTA[NTOP] = D[NTOP] / B[NTOP];
    // SOLVE THE COEFS FOR SOIL LAYERS 2 THRU NSOIL
    for K in NTOP + 1..=NSOIL {
        P[K] = -C[K] * (1.0 / (B[K] + A[K] * P[K - 1]));
        DELTA[K] = (D[K] - A[K] * DELTA[K - 1]) * (1.0 / (B[K] + A[K] * P[K - 1]));
    }
    // SET P TO DELTA FOR LOWEST SOIL LAYER
    P[NSOIL] = DELTA[NSOIL];
    // ADJUST P FOR SOIL LAYERS 2 THRU NSOIL
    for K in NTOP + 1..=NSOIL {
        let KK = NSOIL - K + (NTOP - 1) + 1;
        P[KK] = P[KK] * P[KK + 1] + DELTA[KK];
    }
}

/// `PHASECHANGE` -- melting/freezing of snow water and soil water.
pub fn phasechange(
    parameters: &Parameters,
    domain: &Domain,
    energy: &mut Energy,
    water: &mut Water,
    options: &Options,
    NSNOW: i32,
    NSOIL: i32,
) {
    let ISNOW = water.ISNOW;

    let mut HM = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut XM = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut WMASS0 = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut WICE0 = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut WLIQ0 = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut MICE = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut MLIQ = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);
    let mut SUPERCOOL = Shifted::snow_soil(NSNOW, NSOIL, 0.0f32);

    // Initialization
    energy.QMELT = 0.;
    water.PONDING = 0.;
    let mut XMF = 0.;

    for J in -NSNOW + 1..=NSOIL {
        SUPERCOOL[J] = 0.0;
    }

    for J in ISNOW + 1..=0 {
        // all layers
        MICE[J] = water.SNICE[J];
        MLIQ[J] = water.SNLIQ[J];
    }

    for J in 1..=NSOIL {
        // soil
        MLIQ[J] = water.sh2o[J] * domain.dzsnso[J] * 1000.;
        MICE[J] = (water.smc[J] - water.sh2o[J]) * domain.dzsnso[J] * 1000.;
    }

    for J in ISNOW + 1..=NSOIL {
        // all layers
        energy.IMELT[J] = 0;
        HM[J] = 0.;
        XM[J] = 0.;
        WICE0[J] = MICE[J];
        WLIQ0[J] = MLIQ[J];
        WMASS0[J] = MICE[J] + MLIQ[J];
    }

    if domain.ist == 1 {
        for J in 1..=NSOIL {
            if options.opt_frz == 1 && energy.STC[J] < parameters.TFRZ {
                let SMP = parameters.HFUS * (parameters.TFRZ - energy.STC[J])
                    / (parameters.GRAV * energy.STC[J]); // (m)
                SUPERCOOL[J] = parameters.smcmax[J]
                    * fi::powf(SMP / parameters.psisat[J], -1. / parameters.bexp[J]);
                SUPERCOOL[J] = SUPERCOOL[J] * domain.dzsnso[J] * 1000.; // (mm)
            }
            if options.opt_frz == 2 {
                SUPERCOOL[J] = frh2o(parameters, J, energy.STC[J], water.smc[J], water.sh2o[J]);
                SUPERCOOL[J] = SUPERCOOL[J] * domain.dzsnso[J] * 1000.; // (mm)
            }
        }
    }

    for J in ISNOW + 1..=NSOIL {
        if MICE[J] > 0. && energy.STC[J] >= parameters.TFRZ {
            // melting
            energy.IMELT[J] = 1;
        }
        if MLIQ[J] > SUPERCOOL[J] && energy.STC[J] < parameters.TFRZ {
            energy.IMELT[J] = 2;
        }

        // If snow exists, but its thickness is not enough to create a layer
        if ISNOW == 0 && water.SNEQV > 0. && J == 1 && energy.STC[J] >= parameters.TFRZ {
            energy.IMELT[J] = 1;
        }
    }

    // Calculate the energy surplus and loss for melting and freezing
    for J in ISNOW + 1..=NSOIL {
        if energy.IMELT[J] > 0 {
            HM[J] = (energy.STC[J] - parameters.TFRZ) / energy.FACT[J];
            energy.STC[J] = parameters.TFRZ;
        }
        if energy.IMELT[J] == 1 && HM[J] < 0. {
            HM[J] = 0.;
            energy.IMELT[J] = 0;
        }
        if energy.IMELT[J] == 2 && HM[J] > 0. {
            HM[J] = 0.;
            energy.IMELT[J] = 0;
        }
        XM[J] = HM[J] * domain.dt / parameters.HFUS;
    }

    // The rate of melting and freezing for snow without a layer, needs more work.
    if ISNOW == 0 && water.SNEQV > 0. && XM[1] > 0. {
        let TEMP1 = water.SNEQV;
        water.SNEQV = fi::max(0., TEMP1 - XM[1]);
        let PROPOR = water.SNEQV / TEMP1;
        water.SNOWH = fi::max(0., PROPOR * water.SNOWH);
        let HEATR = HM[1] - parameters.HFUS * (TEMP1 - water.SNEQV) / domain.dt;
        if HEATR > 0. {
            XM[1] = HEATR * domain.dt / parameters.HFUS;
            HM[1] = HEATR;
        } else {
            XM[1] = 0.;
            HM[1] = 0.;
        }
        energy.QMELT = fi::max(0., TEMP1 - water.SNEQV) / domain.dt;
        XMF = parameters.HFUS * energy.QMELT;
        water.PONDING = TEMP1 - water.SNEQV;
    }

    // The rate of melting and freezing for snow and soil
    for J in ISNOW + 1..=NSOIL {
        if energy.IMELT[J] > 0 && HM[J].abs() > 0. {
            let mut HEATR = 0.;
            if XM[J] > 0. {
                MICE[J] = fi::max(0., WICE0[J] - XM[J]);
                HEATR = HM[J] - parameters.HFUS * (WICE0[J] - MICE[J]) / domain.dt;
            } else if XM[J] < 0. {
                if J <= 0 {
                    // snow
                    MICE[J] = fi::min(WMASS0[J], WICE0[J] - XM[J]);
                } else {
                    // soil
                    if WMASS0[J] < SUPERCOOL[J] {
                        MICE[J] = 0.;
                    } else {
                        MICE[J] = fi::min(WMASS0[J] - SUPERCOOL[J], WICE0[J] - XM[J]);
                        MICE[J] = fi::max(MICE[J], 0.0);
                    }
                }
                HEATR = HM[J] - parameters.HFUS * (WICE0[J] - MICE[J]) / domain.dt;
            }

            MLIQ[J] = fi::max(0., WMASS0[J] - MICE[J]);

            if HEATR.abs() > 0. {
                energy.STC[J] += energy.FACT[J] * HEATR;
                if J <= 0 {
                    // snow
                    if MLIQ[J] * MICE[J] > 0. {
                        energy.STC[J] = parameters.TFRZ;
                    }
                    if MICE[J] == 0. {
                        // BARLAGE
                        energy.STC[J] = parameters.TFRZ;
                        HM[J + 1] += HEATR;
                        XM[J + 1] = HM[J + 1] * domain.dt / parameters.HFUS;
                    }
                }
            }

            XMF += parameters.HFUS * (WICE0[J] - MICE[J]) / domain.dt;

            if J < 1 {
                energy.QMELT += fi::max(0., WICE0[J] - MICE[J]) / domain.dt;
            }
        }
    }
    let _ = XMF;

    for J in ISNOW + 1..=0 {
        // snow
        water.SNLIQ[J] = MLIQ[J];
        water.SNICE[J] = MICE[J];
    }

    for J in 1..=NSOIL {
        // soil
        water.sh2o[J] = MLIQ[J] / (1000. * domain.dzsnso[J]);
        water.smc[J] = (MLIQ[J] + MICE[J]) / (1000. * domain.dzsnso[J]);
    }
    let _ = WLIQ0;
}

/// `FRH2O` -- supercooled liquid soil water content below `TFRZ`, by Newton iteration on
/// eqn 17 of Koren et al. (1999). Returns `FREE`.
fn frh2o(parameters: &Parameters, ISOIL: i32, TKELV: f32, SMC: f32, SH2O: f32) -> f32 {
    const CK: f32 = 8.0;
    const BLIM: f32 = 5.5;
    const ERROR: f32 = 0.005;

    // LIMITS ON PARAMETER B: B < 5.5  (use parameter BLIM)
    // SIMULATIONS SHOWED IF B > 5.5 UNFROZEN WATER CONTENT IS
    // NON-REALISTICALLY HIGH AT VERY LOW TEMPERATURES.
    let mut BX = parameters.bexp[ISOIL]; // SOIL TYPE "B" PARAMETER

    // INITIALIZING ITERATIONS COUNTER AND ITERATIVE SOLUTION FLAG.
    if parameters.bexp[ISOIL] > BLIM {
        BX = BLIM;
    }
    let mut NLOG = 0;

    // IF TEMPERATURE NOT SIGNIFICANTLY BELOW FREEZING (TFRZ), SH2O = SMC
    let mut KCOUNT = 0;
    let mut FREE;
    if TKELV > (parameters.TFRZ - 1.0E-3) {
        FREE = SMC;
    } else {
        // OPTION 1: ITERATED SOLUTION IN KOREN ET AL, JGR, 1999, EQN 17
        // INITIAL GUESS FOR SWL (frozen content)
        //
        // `IF (CK /= 0.0)` upstream, with CK a parameter of 8.0: always taken.
        let mut SWL = SMC - SH2O;

        // KEEP WITHIN BOUNDS.
        if SWL > (SMC - 0.02) {
            SWL = SMC - 0.02;
        }

        // START OF ITERATIONS
        if SWL < 0. {
            SWL = 0.;
        }

        while NLOG < 10 && KCOUNT == 0 {
            NLOG += 1;
            // `( 1. + CK * SWL )**2.`, a multiply in gfortran; `** BX` is a real power.
            let DF = fi::log(
                (parameters.psisat[ISOIL] * parameters.GRAV / parameters.HFUS)
                    * fi::pow2_real(1. + CK * SWL)
                    * fi::powf(parameters.smcmax[ISOIL] / (SMC - SWL), BX),
            ) - fi::log(-(TKELV - parameters.TFRZ) / TKELV);
            let DENOM = 2. * CK / (1. + CK * SWL) + BX / (SMC - SWL);
            let mut SWLK = SWL - DF / DENOM;

            // BOUNDS USEFUL FOR MATHEMATICAL SOLUTION.
            if SWLK > (SMC - 0.02) {
                SWLK = SMC - 0.02;
            }
            if SWLK < 0. {
                SWLK = 0.;
            }

            // MATHEMATICAL SOLUTION BOUNDS APPLIED.
            let DSWL = (SWLK - SWL).abs();

            // IF MORE THAN 10 ITERATIONS, USE EXPLICIT METHOD (CK=0 APPROX.)
            // WHEN DSWL LESS OR EQ. ERROR, NO MORE ITERATIONS REQUIRED.
            SWL = SWLK;
            if DSWL <= ERROR {
                KCOUNT += 1;
            }
            // END OF ITERATIONS
            // BOUNDS APPLIED WITHIN DO-BLOCK ARE VALID FOR PHYSICAL SOLUTION.
        }

        FREE = SMC - SWL;

        // OPTION 2: EXPLICIT SOLUTION FOR FLERCHINGER EQ. i.e. CK=0
        // IN KOREN ET AL., JGR, 1999, EQN 17
        // APPLY PHYSICAL BOUNDS TO FLERCHINGER SOLUTION
        if KCOUNT == 0 {
            // `( -1/ BX)`: the integer -1 is converted to real before the division.
            let mut FK = fi::powf(
                (parameters.HFUS / (parameters.GRAV * (-parameters.psisat[ISOIL])))
                    * ((TKELV - parameters.TFRZ) / TKELV),
                -1. / BX,
            ) * parameters.smcmax[ISOIL];
            if FK < 0.02 {
                FK = 0.02;
            }
            FREE = fi::min(FK, SMC);
            // END OPTION 2
        }
    } // end IF case for temperature below freezing

    FREE
}
