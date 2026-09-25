//! Port of `src/SurfaceRunoffInfiltration.f90` @ 0ff055e.
//!
//! The infiltration and surface runoff schemes behind `OPT_RUN` 3, 6, 7 and 8: free drainage
//! (`INFIL`), VIC, Xinanjiang, and dynamic VIC with its three infiltration equations.
//!
//! # `DYNAMIC_VIC` reads `YD` before assigning it
//!
//! Two branches of `DYNAMIC_VIC` -- `DP + I_0 > I_MAX` or not, each with `FMAX*DT < DP` --
//! call `RR1(..., YD, TEMPR1)` while `YD` is still unassigned. `YD` is an ordinary local, not
//! `SAVE`d, so the Fortran reads whatever its storage held. See [`YD_UNASSIGNED`] for what the
//! port supplies and why.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::soil_water_retention_coeff::wdfcnd2;
use crate::water::Water;

/// The value `DYNAMIC_VIC` sees for `YD` on the two paths that read it unassigned.
///
/// Upstream behaviour here is undefined: `YD` is passed by reference, so gfortran gives it a
/// stack slot and these calls see whatever that slot last held. Zero is what `YD` is set to on
/// every other path into the same `RR1` call. The water sweep enters both branches and the
/// reference build agrees with any value from `0.0` to `1e-3` and not with `0.5` -- a
/// measurement of this build, not a guarantee; see `docs/porting.md`.
///
/// `RR1` of a zero depth is exactly `+0.0`, which makes the `IITERATION30` / `IITERATION3`
/// branches below each of these calls unreachable. Only a non-zero `YD` could enter them.
pub const YD_UNASSIGNED: f32 = 0.0;

/// `INFIL` -- infiltration rate at soil surface and surface runoff, for free drainage.
pub fn infil(parameters: &Parameters, domain: &Domain, nsoil: i32, water: &mut Water) {
    const CVFRZ: i32 = 3;
    let zsoil = &domain.zsoil;
    let dt = water.runsrf_dt;

    if water.qinsur > 0.0 {
        let DT1 = dt / 86400.;
        let SMCAV = parameters.smcmax[1] - parameters.smcwlt[1];

        // maximum infiltration rate
        let mut DMAX = crate::layers::Shifted::ones(nsoil, 0.0f32);
        DMAX[1] = -zsoil[1] * SMCAV;
        let mut DICE = -zsoil[1] * water.sice[1];
        DMAX[1] *= 1.0 - (water.sh2o[1] + water.sice[1] - parameters.smcwlt[1]) / SMCAV;

        let mut DD = DMAX[1];

        for K in 2..=nsoil {
            DICE += (zsoil[K - 1] - zsoil[K]) * water.sice[K];
            DMAX[K] = (zsoil[K - 1] - zsoil[K]) * SMCAV;
            DMAX[K] *= 1.0 - (water.sh2o[K] + water.sice[K] - parameters.smcwlt[K]) / SMCAV;
            DD += DMAX[K];
        }

        let VAL = 1. - fi::exp(-parameters.kdt * DT1);
        let DDT = DD * VAL;
        let PX = fi::max(0., water.qinsur * dt);
        let mut INFMAX = (PX * (DDT / (PX + DDT))) / dt;

        // impermeable fraction due to frozen soil
        let mut FCR1 = 1.0f32;
        if DICE > 1.0E-2 {
            let ACRT = CVFRZ as f32 * parameters.frzx / DICE;
            let mut SUM = 1.0f32;
            let IALP1 = CVFRZ - 1;
            for J in 1..=IALP1 {
                let mut K = 1;
                for JJ in J + 1..=IALP1 {
                    K *= JJ;
                }
                SUM += fi::powi(ACRT, CVFRZ - J) / K as f32;
            }
            FCR1 = 1. - fi::exp(-ACRT) * SUM;
        }

        // correction of infiltration limitation
        INFMAX *= FCR1;

        // jref for urban areas
        //       IF ( parameters%urban_flag ) INFMAX == INFMAX * 0.05

        let (_WDF, WCND1) = wdfcnd2(parameters, water.sh2o[1], water.sicemax, 1);
        INFMAX = fi::max(INFMAX, WCND1);
        INFMAX = fi::min(INFMAX, PX);

        water.runsrf = fi::max(0., water.qinsur - INFMAX);
        water.pddum = water.qinsur - water.runsrf;
    }
}

/// `COMPUTE_VIC_SURFRUNOFF` -- saturated area and runoff from the VIC scheme.
pub fn compute_vic_surfrunoff(
    parameters: &Parameters,
    domain: &Domain,
    nsoil: i32,
    water: &mut Water,
) {
    let zsoil = &domain.zsoil;

    // Initialize Variables
    let DT = water.runsrf_dt;
    let mut TOP_MOIST = 0.0f32;
    let mut TOP_MAX_MOIST = 0.0f32;
    water.runsrf = 0.0;
    water.ASAT = 0.0;

    for IZ in 1..=nsoil - 2 {
        TOP_MOIST += water.smc[IZ] * (-1.) * zsoil[IZ]; // m
        TOP_MAX_MOIST += parameters.smcmax[IZ] * (-1.) * zsoil[IZ]; // m
    }

    // Saturated area from soil moisture
    let EX = parameters.bvic / (1. + parameters.bvic);
    water.ASAT = 1.0 - fi::powf(1.0 - (TOP_MOIST / TOP_MAX_MOIST), EX);
    water.ASAT = fi::max(0.0, water.ASAT);
    water.ASAT = fi::min(1.0, water.ASAT);

    // Infiltration for the previous time-step soil moisture based on ASAT
    let I_MAX = (1.0 + parameters.bvic) * TOP_MAX_MOIST; // m
    let I_0 = I_MAX * (1.0 - fi::powf(1.0 - water.ASAT, 1.0 / parameters.bvic)); // m

    // Solve for surface runoff
    if water.qinsur == 0.0 {
        water.runsrf = 0.0;
    } else if I_MAX == 0.0 {
        water.runsrf = water.qinsur * DT;
    } else if (I_0 + (water.qinsur * DT)) > I_MAX {
        water.runsrf = (water.qinsur * DT) - TOP_MAX_MOIST + TOP_MOIST;
    } else {
        let BASIS = 1.0 - ((I_0 + (water.qinsur * DT)) / I_MAX);
        water.runsrf = (water.qinsur * DT) - TOP_MAX_MOIST
            + TOP_MOIST
            + TOP_MAX_MOIST * fi::powf(BASIS, 1.0 + parameters.bvic);
    }

    water.runsrf /= DT; // m/s
    if water.runsrf < 0.0 {
        water.runsrf = 0.0;
    }
    if water.runsrf > water.qinsur {
        water.runsrf = water.qinsur;
    }
    water.pddum = water.qinsur - water.runsrf; // m/s
}

/// `COMPUTE_XAJ_SURFRUNOFF` -- saturated area and runoff from the Xinanjiang scheme.
pub fn compute_xaj_surfrunoff(
    parameters: &Parameters,
    domain: &Domain,
    nsoil: i32,
    water: &mut Water,
) {
    let zsoil = &domain.zsoil;

    // initialize
    let mut WM = 0.0f32;
    let mut WM_MAX = 0.0f32;
    let mut SM = 0.0f32;
    let mut SM_MAX = 0.0f32;
    let DT = water.runsrf_dt;
    water.runsrf = 0.0;

    for IZ in 1..=nsoil - 2 {
        if (water.smc[IZ] - parameters.smcref[IZ]) > 0. {
            // soil moisture greater than field capacity
            SM += (water.smc[IZ] - parameters.smcref[IZ]) * (-1.) * zsoil[IZ]; // m
            WM += parameters.smcref[IZ] * (-1.) * zsoil[IZ]; // m
        } else {
            WM += water.smc[IZ] * (-1.) * zsoil[IZ];
        }
        WM_MAX += parameters.smcref[IZ] * (-1.) * zsoil[IZ];
        SM_MAX += (parameters.smcmax[IZ] - parameters.smcref[IZ]) * (-1.) * zsoil[IZ];
    }
    WM = fi::min(WM, WM_MAX); // tension water (m)
    SM = fi::min(SM, SM_MAX); // free water (m)

    // impervious surface runoff R_IMP
    let IRUNOFF = water.fcr[1] * water.qinsur * DT;
    // solve pervious surface runoff (m) based on Eq. (310)
    let PRUNOFF = if (WM / WM_MAX) <= (0.5 - parameters.AXAJ) {
        (1. - water.fcr[1])
            * water.qinsur
            * DT
            * fi::powf(0.5 - parameters.AXAJ, 1. - parameters.BXAJ)
            * fi::powf(WM / WM_MAX, parameters.BXAJ)
    } else {
        (1. - water.fcr[1])
            * water.qinsur
            * DT
            * (1.
                - (fi::powf(0.5 + parameters.AXAJ, 1. - parameters.BXAJ)
                    * fi::powf(1. - (WM / WM_MAX), parameters.BXAJ)))
    };
    // estimate surface runoff based on Eq. (313)
    if water.qinsur == 0.0 {
        water.runsrf = 0.0;
    } else {
        water.runsrf = PRUNOFF * (1. - fi::powf(1. - (SM / SM_MAX), parameters.XXAJ)) + IRUNOFF;
    }
    water.runsrf /= DT; // m/s
    water.runsrf = fi::max(0.0, water.runsrf);
    water.runsrf = fi::min(water.qinsur, water.runsrf);
    water.pddum = water.qinsur - water.runsrf;
}

/// `DYNAMIC_VIC` -- infiltration rate at soil surface and surface runoff from Liang & Xie
/// (2001).
pub fn dynamic_vic(
    parameters: &Parameters,
    options: &Options,
    domain: &Domain,
    nsoil: i32,
    water: &mut Water,
) {
    let zsoil = &domain.zsoil;

    let mut TOP_MOIST = 0.0f32;
    let mut TOP_MAX_MOIST = 0.0f32;
    let DT = water.runsrf_dt;
    water.runsrf = 0.0;
    const IZMAX: i32 = 20;
    let ERROR = 1.388889E-07 * DT; // 0.5 mm per hour time step
    let BB = parameters.BBVIC;

    for IZ in 1..=nsoil - 2 {
        TOP_MOIST += water.smc[IZ] * (-1.) * zsoil[IZ]; // actual moisture in top layers, [m]
        TOP_MAX_MOIST += parameters.smcmax[IZ] * (-1.) * zsoil[IZ]; // maximum moisture in top layers, [m]
    }
    if TOP_MOIST > TOP_MAX_MOIST {
        TOP_MOIST = TOP_MAX_MOIST;
    }
    let DP = water.qinsur * DT; // precipitation depth, [m]
    let I_MAX = TOP_MAX_MOIST * (parameters.bvic + 1.0); // maximum infiltration capacity, im, [m], Eq. 14
    let i_0 = |top_moist: f32| {
        I_MAX * (1. - (1. - fi::powf(top_moist / TOP_MAX_MOIST, 1.0 / (1.0 + parameters.bvic))))
    };
    let mut I_0 = i_0(TOP_MOIST); // infiltration capacity, i [m] in the Eq. 1
                                  // I_MAX = CAP_minf ; I_0 = A
    let INFLMAX = 0;
    // `FSUR` is `intent(out)` of all three, and the namelist allows no other opt_infdv.
    let mut FSUR = 0.0f32;
    if options.opt_infdv == 1 {
        FSUR = philip_infil(parameters, domain, water, DT, INFLMAX);
    } else if options.opt_infdv == 2 {
        FSUR = green_ampt_infil(parameters, domain, water, INFLMAX);
    } else if options.opt_infdv == 3 {
        FSUR = smith_parlange_infil(parameters, domain, water, INFLMAX);
    }

    // I_MM = FSUR; I_M = FMAX
    let FMAX = (BB + 1.0) * FSUR;
    let rr1 = |I_0: f32, YD: f32| rr1(parameters, I_0, I_MAX, YD);
    // Labels 1001 and 1002: clamp YD to [0, DP] and split it into the two runoffs.
    let settle = |I_0: f32, mut YD: f32| {
        if YD <= 0.0 {
            YD = 0.0;
        }
        if YD >= DP {
            YD = DP;
        }
        let R1 = rr1(I_0, YD);
        (R1, DP - YD)
    };

    // Label 2001 is the end of this block: every `GOTO 2001` is a `break 'done`.
    let (RUNOFFSAT, RUNOFFINF) = 'done: {
        if DP <= 0.0 {
            break 'done (0.0, 0.0);
        }
        if (TOP_MOIST >= TOP_MAX_MOIST) && (I_0 >= I_MAX) {
            break 'done (DP, 0.0);
        }
        I_0 = i_0(TOP_MOIST);
        let mut YD;
        if (DP + I_0) > I_MAX {
            if (FMAX * DT) >= DP {
                YD = I_MAX - I_0;
                let TEMPR1 = rr1(I_0, YD);
                let TEMP1 = I_MAX
                    - I_0
                    - TEMPR1
                    - ((FSUR * DT) * (1. - (1. - fi::powf((DP - TEMPR1) / (FMAX * DT), BB + 1.0))));
                if TEMP1 <= 0.0 {
                    let INFILTRTN = TOP_MAX_MOIST - TOP_MOIST;
                    break 'done (DP - INFILTRTN, 0.0);
                }
                YD = 0.0;
                for IZ in 1..=IZMAX {
                    // loop : IITERATION1
                    let YD_OLD = YD;
                    let TEMPR1 = rr1(I_0, YD);
                    YD = TEMPR1
                        + ((FSUR * DT)
                            * (1. - (1. - fi::powf((DP - TEMPR1) / (FMAX * DT), BB + 1.0))));
                    if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                        break;
                    }
                }
            } else {
                let TEMPR1 = rr1(I_0, YD_UNASSIGNED);
                if (TEMPR1 + (FMAX * DT)) <= DP {
                    if (I_MAX - I_0 - TEMPR1 - (FMAX * DT)) <= 0.0 {
                        let INFILTRTN = TOP_MAX_MOIST - TOP_MOIST;
                        break 'done (DP - INFILTRTN, 0.0);
                    }
                    YD = 0.0;
                    for IZ in 1..=IZMAX {
                        // loop : IITERATION2
                        let YD_OLD = YD;
                        let TEMPR1 = rr1(I_0, YD);
                        YD = TEMPR1 + (FSUR * DT);
                        if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                            break;
                        }
                    }
                } else {
                    YD = DP / 2.0;
                    let mut YD0 = 0.0f32;
                    for IZ in 1..=IZMAX {
                        // loop : IITERATION30
                        let YD_OLD = YD;
                        let TEMPR1 = rr1(I_0, YD);
                        YD = YD - TEMPR1 - (FSUR * DT) + DP;
                        if YD <= 0.0 {
                            YD = 0.0;
                        }
                        if YD >= DP {
                            YD = DP;
                        }
                        if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                            YD0 = YD;
                            break;
                        }
                    }
                    for IZ in 1..=IZMAX {
                        // loop : IITERATION3
                        let YD_OLD = YD;
                        let TEMPR1 = rr1(I_0, YD);
                        let TEMPR2 = rr2(YD, YD0, TEMPR1, FMAX, FSUR, DT, DP, BB);
                        YD = DP - TEMPR2;
                        if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                            break;
                        }
                    }
                }
            }
            // 1001. The Fortran then updates TOP_MOIST and I_0, which nothing reads again.
            break 'done settle(I_0, YD);
        } else {
            if (FMAX * DT) >= DP {
                YD = DP / 2.0;
                for IZ in 1..=IZMAX {
                    // ITERATION1
                    let YD_OLD = YD;
                    let TEMPR1 = rr1(I_0, YD);
                    YD = TEMPR1
                        + ((FSUR * DT)
                            * (1. - (1. - fi::powf((DP - TEMPR1) / (FMAX * DT), BB + 1.0))));
                    if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                        break;
                    }
                }
            } else {
                let TEMPR1 = rr1(I_0, YD_UNASSIGNED);
                if (TEMPR1 + (FMAX * DT)) <= DP {
                    YD = DP / 2.0;
                    for IZ in 1..=IZMAX {
                        // ITERATION2
                        let YD_OLD = YD;
                        let TEMPR1 = rr1(I_0, YD);
                        YD = TEMPR1 + (FSUR * DT);
                        if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                            break;
                        }
                    }
                } else {
                    YD = 0.0;
                    let mut YD0 = 0.0f32;
                    for IZ in 1..=IZMAX {
                        // ITERATION30
                        let TEMPR1 = rr1(I_0, YD);
                        YD = (DP - (FMAX * DT)) + YD - TEMPR1;
                        if YD <= 0.0 {
                            YD = 0.0;
                        }
                        if YD >= DP {
                            YD = DP;
                        }
                        let TEMPR1 = rr1(I_0, YD);
                        if ((TEMPR1 + (FMAX * DT) - DP).abs() <= ERROR) || (IZ == IZMAX) {
                            YD0 = YD;
                            break;
                        }
                    }
                    for IZ in 1..=IZMAX {
                        // ITERATION3
                        let YD_OLD = YD;
                        let TEMPR1 = rr1(I_0, YD);
                        let TEMPR2 = rr2(YD, YD0, TEMPR1, FMAX, FSUR, DT, DP, BB);
                        YD = DP - TEMPR2;
                        if ((YD - YD_OLD).abs() <= ERROR) || (IZ == IZMAX) {
                            break;
                        }
                    }
                }
            }
            // 1002. As at 1001, the TOP_MOIST and I_0 updates that follow are dead.
            break 'done settle(I_0, YD);
        }
    };

    // 2001
    water.runsrf = (RUNOFFSAT + RUNOFFINF) / DT;
    water.runsrf = fi::min(water.runsrf, water.qinsur);
    water.runsrf = fi::max(water.runsrf, 0.0);
    water.pddum = water.qinsur - water.runsrf;
}

/// `RR1` -- saturation excess runoff.
pub fn rr1(parameters: &Parameters, I_0: f32, I_MAX: f32, YD: f32) -> f32 {
    let mut TDEPTH = I_0 + YD;
    if TDEPTH > I_MAX {
        TDEPTH = I_MAX;
    }

    // Saturation excess runoff , Eq 5.
    let mut R1 = YD
        - ((I_MAX / (parameters.bvic + 1.0))
            * (fi::powf(1. - (I_0 / I_MAX), parameters.bvic + 1.0)
                - fi::powf(1. - (TDEPTH / I_MAX), parameters.bvic + 1.0)));

    if R1 < 0.0 {
        R1 = 0.0;
    }
    R1
}

/// `RR2` -- infiltration excess runoff.
#[allow(clippy::too_many_arguments)]
pub fn rr2(YD: f32, Y0: f32, R1: f32, FMAX: f32, FSUR: f32, DT: f32, DP: f32, BB: f32) -> f32 {
    let _ = FSUR;
    let mut R2 = if YD >= Y0 {
        DP - R1 - (FMAX * DT * (1. - fi::powf(1. - (DP - R1) / (FMAX * DT), BB + 1.0)))
    } else {
        DP - R1 - (FMAX * DT)
    };

    if R2 < 0.0 {
        R2 = 0.0;
    }
    R2
}

/// `SMITH_PARLANGE_INFIL` -- returns `FSUR`, the surface infiltration rate (m/s).
pub fn smith_parlange_infil(
    parameters: &Parameters,
    domain: &Domain,
    water: &mut Water,
    INFLMAX: i32,
) -> f32 {
    // smith-parlang weighing parameter, GAMMA
    let GAM = 0.82f32;
    let ISOIL = 1;
    let mut FSUR;

    // check whether we are estimating infiltration for current SMC or SMCWLT
    if INFLMAX == 1 {
        // not active for now as the maximum infiltration is estimated based on table values
        // estimate initial soil hydraulic conductivty (Ki in the equation), WCND (m/s)
        let (_WDF, WCND) = wdfcnd2(parameters, parameters.smcwlt[ISOIL], 0.0, ISOIL);
        // Maximum infiltrability based on the Eq. 6.25. (m/s)
        let JJ = parameters.G
            * (parameters.smcmax[ISOIL] - parameters.smcwlt[ISOIL])
            * (-1.)
            * domain.zsoil[ISOIL];
        FSUR = parameters.dksat[ISOIL]
            + (GAM * (parameters.dksat[ISOIL] - WCND) / (fi::exp(GAM * 1E-05 / JJ) - 1.));
        // infiltration rate at surface
        if parameters.dksat[ISOIL] < water.qinsur {
            FSUR = fi::min(water.qinsur, FSUR);
        } else {
            FSUR = water.qinsur;
        }
        if FSUR < 0.0 {
            FSUR = 0.0;
        }
    } else {
        // estimate initial soil hydraulic conductivty (Ki in the equation), WCND (m/s)
        let (_WDF, WCND) = wdfcnd2(parameters, water.smc[ISOIL], water.sice[ISOIL], ISOIL);
        // Maximum infiltrability based on the Eq. 6.25. (m/s)
        let JJ = parameters.G
            * (parameters.smcmax[ISOIL] - water.smc[ISOIL])
            * (-1.)
            * domain.zsoil[ISOIL];
        FSUR = parameters.dksat[ISOIL]
            + (GAM * (parameters.dksat[ISOIL] - WCND) / (fi::exp(GAM * water.FACC / JJ) - 1.));
        // infiltration rate at surface
        if parameters.dksat[ISOIL] < water.qinsur {
            FSUR = fi::min(water.qinsur, FSUR);
        } else {
            FSUR = water.qinsur;
        }

        // accumulated infiltration function
        water.FACC += FSUR;
    }
    FSUR
}

/// `GREEN_AMPT_INFIL` -- returns `FSUR`, the surface infiltration rate (m/s).
pub fn green_ampt_infil(
    parameters: &Parameters,
    domain: &Domain,
    water: &mut Water,
    INFLMAX: i32,
) -> f32 {
    let ISOIL = 1;
    let mut FSUR;

    if INFLMAX == 1 {
        // estimate initial soil hydraulic conductivty (Ki in the equation), WCND (m/s)
        let (_WDF, WCND) = wdfcnd2(parameters, parameters.smcwlt[ISOIL], 0.0, ISOIL);
        // Maximum infiltrability based on the Eq. 6.25. (m/s)
        let JJ = parameters.G
            * (parameters.smcmax[ISOIL] - parameters.smcwlt[ISOIL])
            * (-1.)
            * domain.zsoil[ISOIL];
        FSUR = parameters.dksat[ISOIL] + ((JJ / 1E-05) * (parameters.dksat[ISOIL] - WCND));
        // maximum infiltration rate at surface
        if FSUR < 0.0 {
            FSUR = 0.0;
        }
    } else {
        // estimate initial soil hydraulic conductivty (Ki in the equation), WCND (m/s)
        let (_WDF, WCND) = wdfcnd2(parameters, water.smc[ISOIL], water.sice[ISOIL], ISOIL);
        // Maximum infiltrability based on the Eq. 6.25. (m/s)
        let JJ = parameters.G
            * (parameters.smcmax[ISOIL] - water.smc[ISOIL])
            * (-1.)
            * domain.zsoil[ISOIL];
        FSUR = parameters.dksat[ISOIL] + ((JJ / water.FACC) * (parameters.dksat[ISOIL] - WCND));
        // infiltration rate at surface
        if parameters.dksat[ISOIL] < water.qinsur {
            FSUR = fi::min(water.qinsur, FSUR);
        } else {
            FSUR = water.qinsur;
        }
        // accumulated infiltration function
        water.FACC += FSUR;
    }
    FSUR
}

/// `PHILIP_INFIL` -- returns `FSUR`, the surface infiltration rate (m/s).
pub fn philip_infil(
    parameters: &Parameters,
    domain: &Domain,
    water: &mut Water,
    DT: f32,
    INFLMAX: i32,
) -> f32 {
    let _ = domain;
    let ISOIL = 1;
    let mut FSUR;

    if INFLMAX == 1 {
        // estimate initial soil hydraulic conductivty and diffusivity (Ki, D(theta) in the equation)
        let (WDF, WCND) = wdfcnd2(parameters, parameters.smcwlt[ISOIL], 0.0, ISOIL);
        // Sorptivity based on Eq. 10b from Kutílek, Miroslav, and Jana Valentová (1986)
        // Sorptivity approximations. Transport in Porous Media 1.1, 57-62.
        let SP = fi::sqrt(
            2. * (parameters.smcmax[ISOIL] - parameters.smcwlt[ISOIL])
                * (parameters.dwsat[ISOIL] - WDF),
        );
        // Parameter A in Eq. 9 of Valiantzas (2010) is given by
        let mut AP = fi::min(WCND, (2.0 / 3.) * parameters.dksat[ISOIL]);
        AP = fi::max(AP, (1.0 / 3.) * parameters.dksat[ISOIL]);
        // Maximun infiltration rate, m
        FSUR = (1.0 / 2.) * SP * fi::powf(DT, -1.0 / 2.) + AP; // m/s
        if FSUR < 0.0 {
            FSUR = 0.0;
        }
    } else {
        // estimate initial soil hydraulic conductivty and diffusivity (Ki, D(theta) in the equation)
        let (WDF, WCND) = wdfcnd2(parameters, water.smc[ISOIL], water.sice[ISOIL], ISOIL);
        // Sorptivity based on Eq. 10b from Kutílek, Miroslav, and Jana Valentová (1986)
        // Sorptivity approximations. Transport in Porous Media 1.1, 57-62.
        let SP = fi::sqrt(
            2. * (parameters.smcmax[ISOIL] - water.smc[ISOIL]) * (parameters.dwsat[ISOIL] - WDF),
        );
        // Parameter A in Eq. 9 of Valiantzas (2010) is given by
        let mut AP = fi::min(WCND, (2.0 / 3.) * parameters.dksat[ISOIL]);
        AP = fi::max(AP, (1.0 / 3.) * parameters.dksat[ISOIL]);
        // Maximun infiltration rate, m
        FSUR = (1.0 / 2.) * SP * fi::powf(DT, -1.0 / 2.) + AP; // m/s
                                                               // infiltration rate at surface
        if parameters.dksat[ISOIL] < water.qinsur {
            FSUR = fi::min(water.qinsur, FSUR);
        } else {
            FSUR = water.qinsur;
        }
        // accumulated infiltration function
        water.FACC += FSUR;
    }
    FSUR
}
