//! Port of `src/EtFluxModule.f90` @ 0ff055e.
//!
//! The surface energy balance: Newton-Raphson iteration for the vegetation and ground
//! temperatures (`VegeFluxMain`) and for the bare-ground temperature (`BareFluxMain`), with the
//! resistance, stomatal and stability routines they call.
//!
//! This is where a ULP of drift is amplified: `VegeFluxMain` runs up to `NITERC = 20`
//! iterations on `TV`, and every transcendental in the loop goes through
//! [`crate::fortran::intrinsics`] for that reason.
//!
//! `opt_crop == 2` (GECROS) is unsupported upstream and its one branch reads `FRSU` and `GLAIE`
//! before anything assigns them. The namelist check rejects `crop_model_option /= 0`, so the
//! branch is unreachable; it is transcribed with both undefined values as zero.

#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// `VegeFluxMain` -- solve for vegetation (TV) and ground (TG) temperatures that balance the
/// vegetated surface energy budgets.
///
/// `levels` is in the Fortran signature and unused; it is dropped here.
pub fn vege_flux_main(
    domain: &Domain,
    options: &Options,
    parameters: &Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &mut Water,
) {
    // jref - NITERC test from 5 to 20
    const NITERC: i32 = 20; // number of iterations for surface temperature
    // jref - NITERG test from 3-5
    const NITERG: i32 = 5; // number of iterations for ground temperature

    let SFCTMP = forcing.SFCTMP;
    let RHOAIR = forcing.RHOAIR;
    let PSFC = forcing.SFCPRS;
    let Z0M = energy.Z0M;
    let ZLVL = energy.ZLVL;
    let EMV = energy.EMV;
    let EMG = energy.EMG;
    let LWDN = forcing.LWDN;
    let RHSUR = energy.RHSUR;
    let FVEG = parameters.FVEG;
    let CPAIR = parameters.CPAIR;
    let UR = forcing.UR;

    let mut LITER = 0; // last iteration
    let mut FV = 0.1;

    // initialization variables that do not depend on stability iteration
    let mut DTV;
    let mut DTG;
    let mut MOZ = 0.;
    let mut MOZSGN = 0;
    let mut FH2 = 0.;
    let mut HG = 0.;
    let mut H = 0.;
    // The remaining locals SFCDIF1/SFCDIF2/RAGRB write before reading, or read as
    // `intent(inout)` from a first-iteration value they set themselves.
    let mut FM = 0.;
    let mut FH = 0.;
    let mut FM2 = 0.;
    let mut CH2 = 0.;
    let mut WSTAR = 0.;
    let mut MOZG = 0.;
    let mut FHG = 0.;
    let mut Z0H = 0.;
    let mut RAHC;
    let mut RAHG = 0.;
    let mut RAWG = 0.;
    let mut RB;
    let mut CAH = 0.;
    let mut CVH = 0.;
    let mut CSH;
    let mut CEV;
    let mut CGH;
    let mut ESTG;
    let mut DESTG;
    let mut A;
    let mut B;
    let mut AIR;
    let mut CIR;
    let FRSU = 0.0f32;
    let GLAIE = 0.0f32;

    // limit LAI
    let VAIE = fi::min(6., parameters.VAI);
    let LAISUNE = fi::min(6., energy.LAISUN);
    let LAISHAE = fi::min(6., energy.LAISHA);

    // saturation vapor pressure at ground temperature
    let mut T = tdc(energy.TG, parameters.TFRZ);
    let (ESATW, ESATI, _, _) = esat(T);
    if T > 0. {
        ESTG = ESATW;
    } else {
        ESTG = ESATI;
    }

    // jref - consistent surface specific humidity for sfcdif3 and sfcdif4
    energy.QSFC = 0.622 * forcing.EAIR / (PSFC - 0.378 * forcing.EAIR);

    // canopy height
    let HCAN = parameters.HVT;
    // The first UC is dead upstream too -- overwritten on the next line (MB: add ZPD v3.7).
    let UC = UR * fi::log((HCAN - energy.ZPD + Z0M) / Z0M) / fi::log(ZLVL / Z0M);
    // Upstream writes "CRITICAL PROBLEM: HCAN <= ZPD" to stdout here and carries on. The
    // port carries on too; there is nothing to reproduce but the message.

    // prepare for longwave rad.
    AIR = -EMV * (1. + (1. - EMV) * (1. - EMG)) * LWDN
        - EMV * EMG * parameters.SB * fi::powi(energy.TG, 4);
    CIR = (2. - EMV * (1. - EMG)) * EMV * parameters.SB;

    // ---------------------------------------------------------------------------------------------
    // begin stability iteration
    for ITER in 1..=NITERC {
        let Z0HG;
        if ITER == 1 {
            Z0H = Z0M;
            Z0HG = energy.Z0MG;
        } else {
            Z0H = Z0M; //* EXP(-CZIL*0.4*258.2*SQRT(FV*Z0M))
            Z0HG = energy.Z0MG; //* EXP(-CZIL*0.4*258.2*SQRT(FV*Z0MG))
        }

        // aerodyn resistances between heights zlvl and d+z0v
        if options.opt_sfc == 1 {
            sfcdif1(
                parameters,
                ITER,
                SFCTMP,
                RHOAIR,
                H,
                forcing.QAIR,
                ZLVL,
                energy.ZPD,
                Z0M,
                Z0H,
                UR,
                &mut MOZ,
                &mut MOZSGN,
                &mut FM,
                &mut FH,
                &mut FM2,
                &mut FH2,
                &mut energy.CM,
                &mut energy.CH,
                &mut FV,
                &mut CH2,
            );
        }

        if options.opt_sfc == 2 {
            sfcdif2(
                parameters,
                ITER,
                Z0M,
                energy.TAH,
                forcing.THAIR,
                UR,
                ZLVL,
                &mut energy.CM,
                &mut energy.CH,
                &mut MOZ,
                &mut WSTAR,
                &mut FV,
            );
            // Undo the multiplication by windspeed that SFCDIF2
            // applies to exchange coefficients CH and CM:
            energy.CH /= UR;
            energy.CM /= UR;
        }

        let _RAMC = fi::max(1., 1. / (energy.CM * UR));
        RAHC = fi::max(1., 1. / (energy.CH * UR));
        let RAWC = RAHC;

        // calculate aerodynamic resistance between heights z0g and d+z0v, RAG, and leaf
        // boundary layer resistance, RB
        {
            let (_RAMG, rahg, rawg, rb) = ragrb(
                parameters,
                ITER,
                VAIE,
                RHOAIR,
                HG,
                energy.TAH,
                energy.ZPD,
                energy.Z0MG,
                Z0HG,
                HCAN,
                UC,
                Z0H,
                FV,
                &mut MOZG,
                &mut FHG,
            );
            RAHG = rahg;
            RAWG = rawg;
            RB = rb;
        }

        // es and d(es)/dt evaluated at tv
        T = tdc(energy.TV, parameters.TFRZ);
        let (ESATW, ESATI, DSATW, DSATI) = esat(T);
        let (ESTV, DESTV);
        if T > 0. {
            ESTV = ESATW;
            DESTV = DSATW;
        } else {
            ESTV = ESATI;
            DESTV = DSATI;
        }

        // calculate stomatal resistance and photosynthesis (two options available)
        if ITER == 1 {
            if options.opt_crs == 1 {
                // Ball-Berry
                // sun
                (energy.RSSUN, energy.PSNSUN) = stomata(
                    parameters,
                    energy.PARSUN,
                    forcing.FOLN,
                    energy.TV,
                    ESTV,
                    energy.EAH,
                    SFCTMP,
                    forcing.SFCPRS,
                    forcing.O2PP,
                    forcing.CO2PP,
                    energy.IGS,
                    water.BTRAN,
                    RB,
                );
                // shade
                (energy.RSSHA, energy.PSNSHA) = stomata(
                    parameters,
                    energy.PARSHA,
                    forcing.FOLN,
                    energy.TV,
                    ESTV,
                    energy.EAH,
                    SFCTMP,
                    forcing.SFCPRS,
                    forcing.O2PP,
                    forcing.CO2PP,
                    energy.IGS,
                    water.BTRAN,
                    RB,
                );
            }

            // calculate sunlit and shaded resistances and leaf photosynthesis
            if options.opt_crs == 2 {
                // Jarvis
                // sun
                (energy.RSSUN, energy.PSNSUN) = canres(
                    parameters,
                    energy.PARSUN,
                    energy.TV,
                    water.BTRAN,
                    energy.EAH,
                    forcing.SFCPRS,
                );
                // shade
                (energy.RSSHA, energy.PSNSHA) = canres(
                    parameters,
                    energy.PARSHA,
                    energy.TV,
                    water.BTRAN,
                    energy.EAH,
                    forcing.SFCPRS,
                );
            }

            // Maximum ETRAN: Force minimum stomatal resistance when OPT_BTR=4
            if options.opt_btr == 4 {
                energy.RSSUN = parameters.RSMIN;
                energy.RSSHA = parameters.RSMIN;
            }

            // Call GECROS
            // Note:  GECROS option (opt_crop == 2) is not currently supported.
        }

        // prepare for sensible heat flux above veg.
        CAH = 1. / RAHC;
        CVH = 2. * VAIE / RB;
        CGH = 1. / RAHG;
        let mut COND = CAH + CVH + CGH;
        let ATA = (SFCTMP * CAH + energy.TG * CGH) / COND;
        let BTA = CVH / COND;
        CSH = (1. - BTA) * RHOAIR * CPAIR * CVH;

        // prepare for latent heat flux above veg.
        let CAW = 1. / RAWC;
        let CEW = water.FWET * VAIE / RB;

        let CTW = if options.opt_crop != 2 {
            (1. - water.FWET)
                * (LAISUNE / (RB + energy.RSSUN) + LAISHAE / (RB + energy.RSSHA))
        } else {
            // RSSUN and RSSHA are in resistance per unit LAI in the Jarvis and Ball-Berry!.
            // RSSUN and RSSHA of Gecros are in s/m
            (1. - water.FWET)
                * (1. / (RB / (FRSU * GLAIE) + energy.RSSUN)
                    + 1. / (RB / ((1. - FRSU) * GLAIE) + energy.RSSHA))
        };

        // comment needed
        let CGW = 1. / (RAWG + energy.RSURF);
        COND = CAW + CEW + CTW + CGW;
        let AEA = (forcing.EAIR * CAW + ESTG * CGW) / COND;
        let BEA = (CEW + CTW) / COND;
        CEV = (1. - BEA) * CEW * RHOAIR * CPAIR / energy.GAMMAV; // Barlage: change to vegetation v3.6
        let CTR = (1. - BEA) * CTW * RHOAIR * CPAIR / energy.GAMMAV;

        // evaluate surface fluxes with current temperature and solve for dts
        energy.TAH = ATA + BTA * energy.TV; // canopy air T.
        energy.EAH = AEA + BEA * ESTV; // canopy air e

        energy.IRC = FVEG * (AIR + CIR * fi::powi(energy.TV, 4));
        energy.SHC = FVEG * RHOAIR * CPAIR * CVH * (energy.TV - energy.TAH);
        energy.EVC = FVEG * RHOAIR * CPAIR * CEW * (ESTV - energy.EAH) / energy.GAMMAV; // Barlage: change to v in v3.6
        energy.TR = FVEG * RHOAIR * CPAIR * CTW * (ESTV - energy.EAH) / energy.GAMMAV;

        // `MIN(CANLIQ*LATHEAV/DT, EVC)` upstream. The arguments are swapped here because the
        // reference build resolves a tie between +0.0 and -0.0 in favour of the *first* --
        // 6 of 200 sampled Bondville timesteps, a dry canopy with EVC at -0.0 -- and
        // `fi::min` gives the tie to its second argument. See `intrinsics::min`.
        if energy.TV > parameters.TFRZ {
            energy.EVC = fi::min(energy.EVC, water.canliq * energy.LATHEAV / domain.dt); // Barlage: add if block for canice in v3.6
        } else {
            energy.EVC = fi::min(energy.EVC, water.canice * energy.LATHEAV / domain.dt);
        }

        B = energy.SAV - energy.IRC - energy.SHC - energy.EVC - energy.TR + energy.PAHV; // additional w/m2
        A = FVEG * (4. * CIR * fi::powi(energy.TV, 3) + CSH + (CEV + CTR) * DESTV); // volumetric heat capacity
        DTV = B / A;
        energy.IRC += FVEG * 4. * CIR * fi::powi(energy.TV, 3) * DTV;
        energy.SHC += FVEG * CSH * DTV;
        energy.EVC += FVEG * CEV * DESTV * DTV;
        energy.TR += FVEG * CTR * DESTV * DTV;

        // update vegetation surface temperature
        energy.TV += DTV;
        //TAH = ATA + BTA*TV               ! canopy air T; update here for consistency

        // for computing M-O length in the next iteration
        H = RHOAIR * CPAIR * (energy.TAH - SFCTMP) / RAHC;
        HG = RHOAIR * CPAIR * (energy.TG - energy.TAH) / RAHG;

        // consistent specific humidity from canopy air vapor pressure
        energy.QSFC = (0.622 * energy.EAH) / (forcing.SFCPRS - 0.378 * energy.EAH);

        if LITER == 1 {
            break;
        }
        if ITER >= 5 && DTV.abs() <= 0.01 && LITER == 0 {
            LITER = 1;
        }
    } // end stability iteration
    // ---------------------------------------------------------------------------

    // under-canopy fluxes and tg
    AIR = -EMG * (1. - EMV) * LWDN - EMG * EMV * parameters.SB * fi::powi(energy.TV, 4);
    CIR = EMG * parameters.SB;
    CSH = RHOAIR * CPAIR / RAHG;
    CEV = RHOAIR * CPAIR / (energy.GAMMAG * (RAWG + energy.RSURF)); // Barlage: change to ground v3.6
    CGH = 2. * energy.DF[water.ISNOW + 1] / domain.dzsnso[water.ISNOW + 1];

    // ========= LOOP 2 ========================
    for _ITER in 1..=NITERG {
        T = tdc(energy.TG, parameters.TFRZ);
        let (ESATW, ESATI, DSATW, DSATI) = esat(T);
        if T > 0. {
            ESTG = ESATW;
            DESTG = DSATW;
        } else {
            ESTG = ESATI;
            DESTG = DSATI;
        }

        energy.IRG = CIR * fi::powi(energy.TG, 4) + AIR;
        energy.SHG = CSH * (energy.TG - energy.TAH);
        energy.EVG = CEV * (ESTG * energy.RHSUR - energy.EAH);
        energy.GHV = CGH * (energy.TG - energy.STC[water.ISNOW + 1]);

        B = energy.SAG - energy.IRG - energy.SHG - energy.EVG - energy.GHV + energy.PAHG;
        A = 4. * CIR * fi::powi(energy.TG, 3) + CSH + CEV * DESTG + CGH;
        DTG = B / A;

        energy.IRG += 4. * CIR * fi::powi(energy.TG, 3) * DTG;
        energy.SHG += CSH * DTG;
        energy.EVG += CEV * DESTG * DTG;
        energy.GHV += CGH * DTG;
        energy.TG += DTG;
    }
    // ---------------------------------------------------------------------------

    //TAH = (CAH*SFCTMP + CVH*TV + CGH*TG)/(CAH + CVH + CGH)

    // if snow on ground and TG > TFRZ: reset TG = TFRZ. reevaluate ground fluxes.
    if (options.opt_stc == 1 || options.opt_stc == 3)
        && water.SNOWH > 0.05
        && energy.TG > parameters.TFRZ
    {
        if options.opt_stc == 1 {
            energy.TG = parameters.TFRZ;
        }
        if options.opt_stc == 3 {
            energy.TG = (1. - water.FSNO) * energy.TG + water.FSNO * parameters.TFRZ; // MB: allow TG>0C during melt v3.7
        }
        energy.IRG = CIR * fi::powi(energy.TG, 4)
            - EMG * (1. - EMV) * LWDN
            - EMG * EMV * parameters.SB * fi::powi(energy.TV, 4);
        energy.SHG = CSH * (energy.TG - energy.TAH);
        energy.EVG = CEV * (ESTG * RHSUR - energy.EAH);
        energy.GHV = energy.SAG + energy.PAHG - (energy.IRG + energy.SHG + energy.EVG);
    }

    // wind stresses
    energy.TAUXV = -RHOAIR * energy.CM * UR * forcing.UU;
    energy.TAUYV = -RHOAIR * energy.CM * UR * forcing.VV;

    // 2m temperature over vegetation ( corrected for low CQ2V values )
    if options.opt_sfc == 1 || options.opt_sfc == 2 {
        // The first CAH2 is dead upstream too -- overwritten on the next line.
        energy.CAH2 = FV * parameters.VKC / fi::log((2. + Z0H) / Z0H);
        energy.CAH2 = FV * parameters.VKC / (fi::log((2. + Z0H) / Z0H) - FH2);
        let CQ2V = energy.CAH2;
        if energy.CAH2 < 1.0E-5 {
            energy.T2MV = energy.TAH;
            energy.Q2V = energy.QSFC;
        } else {
            energy.T2MV = energy.TAH
                - (energy.SHG + energy.SHC / FVEG) / (RHOAIR * CPAIR) * 1. / energy.CAH2;
            energy.Q2V = energy.QSFC
                - ((energy.EVC + energy.TR) / FVEG + energy.EVG) / (energy.LATHEAV * RHOAIR) * 1.
                    / CQ2V;
        }
    }

    // update CH for output
    energy.CH = CAH;
    energy.CHLEAF = CVH;
    energy.CHUC = 1. / RAHG;
}

/// `BareFluxMain` -- solve the ground temperature that balances the bare-soil energy budget.
///
/// `levels` is in the Fortran signature and unused; it is dropped here.
pub fn bare_flux_main(
    domain: &Domain,
    options: &Options,
    parameters: &Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &Water,
) {
    const NITERB: i32 = 5; // number of iterations for surface temperature

    let SFCTMP = forcing.SFCTMP;
    let RHOAIR = forcing.RHOAIR;
    let PSFC = forcing.SFCPRS;
    let Z0M = energy.Z0M;
    let EMG = energy.EMG;
    let LWDN = forcing.LWDN;
    let CPAIR = parameters.CPAIR;
    let UR = forcing.UR;

    // initialization variables that do not depend on stability iteration
    let mut DTG;
    let mut MOZ = 0.;
    let mut MOZSGN = 0;
    let mut FH2 = 0.;
    let mut H = 0.;
    let mut FV = 0.1;
    let mut FM = 0.;
    let mut FH = 0.;
    let mut FM2 = 0.;
    let mut CH2 = 0.;
    let mut WSTAR = 0.;
    let mut Z0H = 0.;
    let mut EHB = 0.;
    let mut CSH = 0.;
    let mut CEV = 0.;
    let mut ESTG = 0.;
    let mut DESTG;

    let CIR = EMG * parameters.SB;
    let CGH = 2. * energy.DF[water.ISNOW + 1] / domain.dzsnso[water.ISNOW + 1];

    // -----------------------------------------------------------------
    // begin stability iteration
    for ITER in 1..=NITERB {
        if ITER == 1 {
            Z0H = Z0M;
        } else {
            Z0H = Z0M; //* EXP(-CZIL*0.4*258.2*SQRT(FV*Z0M))
        }

        // aerodyn resistances
        if options.opt_sfc == 1 {
            sfcdif1(
                parameters,
                ITER,
                SFCTMP,
                RHOAIR,
                H,
                forcing.QAIR,
                energy.ZLVL,
                energy.ZPD,
                energy.Z0M,
                Z0H,
                UR,
                &mut MOZ,
                &mut MOZSGN,
                &mut FM,
                &mut FH,
                &mut FM2,
                &mut FH2,
                &mut energy.CM,
                &mut energy.CH,
                &mut FV,
                &mut CH2,
            );
        }

        if options.opt_sfc == 2 {
            sfcdif2(
                parameters,
                ITER,
                energy.Z0M,
                energy.TGB,
                forcing.THAIR,
                UR,
                energy.ZLVL,
                &mut energy.CM,
                &mut energy.CH,
                &mut MOZ,
                &mut WSTAR,
                &mut FV,
            );
            // Undo the multiplication by windspeed that SFCDIF2
            // applies to exchange coefficients CH and CM:
            energy.CH /= UR;
            energy.CM /= UR;
            if water.SNOWH > 0. {
                energy.CM = fi::min(0.01, energy.CM); // CM & CH are too large, causing
                energy.CH = fi::min(0.01, energy.CH); // computational instability
            }
        }

        let RAMB = fi::max(1., 1. / (energy.CM * UR));
        let RAHB = fi::max(1., 1. / (energy.CH * UR));
        let RAWB = RAHB;

        // variables for diagnostics
        let _EMB = 1. / RAMB;
        EHB = 1. / RAHB;

        // es and d(es)/dt evaluated at tg
        let mut T = tdc(energy.TGB, parameters.TFRZ);
        let (mut ESATW, mut ESATI, DSATW, DSATI) = esat(T);
        if T > 0. {
            ESTG = ESATW;
            DESTG = DSATW;
        } else {
            ESTG = ESATI;
            DESTG = DSATI;
        }

        CSH = RHOAIR * CPAIR / RAHB;
        CEV = RHOAIR * CPAIR / energy.GAMMAG / (energy.RSURF + RAWB);

        // surface fluxes and dtg
        energy.IRB = CIR * fi::powi(energy.TGB, 4) - EMG * LWDN;
        energy.SHB = CSH * (energy.TGB - SFCTMP);
        energy.EVB = CEV * (ESTG * energy.RHSUR - forcing.EAIR);
        energy.GHB = CGH * (energy.TGB - energy.STC[water.ISNOW + 1]);

        let B = energy.SAG - energy.IRB - energy.SHB - energy.EVB - energy.GHB + energy.PAHB;
        let A = 4. * CIR * fi::powi(energy.TGB, 3) + CSH + CEV * DESTG + CGH;
        DTG = B / A;

        energy.IRB += 4. * CIR * fi::powi(energy.TGB, 3) * DTG;
        energy.SHB += CSH * DTG;
        energy.EVB += CEV * DESTG * DTG;
        energy.GHB += CGH * DTG;

        // update ground surface temperature
        energy.TGB += DTG;

        // for M-O length
        H = CSH * (energy.TGB - SFCTMP);

        T = tdc(energy.TGB, parameters.TFRZ);
        (ESATW, ESATI, _, _) = esat(T);
        if T > 0. {
            ESTG = ESATW;
        } else {
            ESTG = ESATI;
        }
        energy.QSFC =
            0.622 * (ESTG * energy.RHSUR) / (PSFC - 0.378 * (ESTG * energy.RHSUR));
        let _QFX = (energy.QSFC - forcing.QAIR) * CEV * energy.GAMMAG / CPAIR;
    } // end stability iteration
    // -----------------------------------------------------------------

    // if snow on ground and TG > TFRZ: reset TG = TFRZ. reevaluate ground fluxes.
    if (options.opt_stc == 1 || options.opt_stc == 3)
        && water.SNOWH > 0.05
        && energy.TGB > parameters.TFRZ
    {
        if options.opt_stc == 1 {
            energy.TGB = parameters.TFRZ;
        }
        if options.opt_stc == 3 {
            energy.TGB = (1. - water.FSNO) * energy.TGB + water.FSNO * parameters.TFRZ; // MB: allow TG>0C during melt v3.7
        }
        energy.IRB = CIR * fi::powi(energy.TGB, 4) - EMG * LWDN;
        energy.SHB = CSH * (energy.TGB - SFCTMP);
        energy.EVB = CEV * (ESTG * energy.RHSUR - forcing.EAIR); // ESTG reevaluate ?
        energy.GHB = energy.SAG + energy.PAHB - (energy.IRB + energy.SHB + energy.EVB);
    }

    // wind stresses
    energy.TAUXB = -RHOAIR * energy.CM * UR * forcing.UU;
    energy.TAUYB = -RHOAIR * energy.CM * UR * forcing.VV;

    // 2m air temperature
    if options.opt_sfc == 1 || options.opt_sfc == 2 {
        // The first EHB2 is dead upstream too -- overwritten on the next line.
        energy.EHB2 = FV * parameters.VKC / fi::log((2. + Z0H) / Z0H);
        energy.EHB2 = FV * parameters.VKC / (fi::log((2. + Z0H) / Z0H) - FH2);
        let CQ2B = energy.EHB2;
        if energy.EHB2 < 1.0E-5 {
            energy.T2MB = energy.TGB;
            energy.Q2B = energy.QSFC;
        } else {
            energy.T2MB = energy.TGB - energy.SHB / (RHOAIR * CPAIR) * 1. / energy.EHB2;
            energy.Q2B = energy.QSFC
                - energy.EVB / (energy.LATHEAG * RHOAIR) * (1. / CQ2B + energy.RSURF);
        }
        if parameters.urban_flag {
            energy.Q2B = energy.QSFC;
        }
    }

    // update CH
    energy.CH = EHB;
}

/// `RAGRB` -- under-canopy aerodynamic resistance and leaf boundary layer resistance.
///
/// Returns `(RAMG, RAHG, RAWG, RB)`. `VEGTYP` and `TV` are in the Fortran signature and
/// unused; they are dropped here.
fn ragrb(
    parameters: &Parameters,
    ITER: i32,
    VAI: f32,
    RHOAIR: f32,
    HG: f32,
    TAH: f32,
    ZPD: f32,
    Z0MG: f32,
    Z0HG: f32,
    HCAN: f32,
    UC: f32,
    Z0H: f32,
    FV: f32,
    MOZG: &mut f32,
    FHG: &mut f32,
) -> (f32, f32, f32, f32) {
    // stability correction to below canopy resistance
    *MOZG = 0.;
    let mut MOLG = 0.;

    if ITER > 1 {
        let mut TMP1 = parameters.VKC * (parameters.GRAV / TAH) * HG / (RHOAIR * parameters.CPAIR);
        if TMP1.abs() <= parameters.MPE {
            TMP1 = parameters.MPE;
        }
        MOLG = -1. * fi::powi(FV, 3) / TMP1;
        *MOZG = fi::min((ZPD - Z0MG) / MOLG, 1.);
    }
    let _ = MOLG;

    let FHGNEW = if *MOZG < 0. {
        fi::powf(1. - 15. * *MOZG, -0.25)
    } else {
        1. + 4.7 * *MOZG
    };

    if ITER == 1 {
        *FHG = FHGNEW;
    } else {
        *FHG = 0.5 * (*FHG + FHGNEW);
    }

    let CWPC = fi::powf(parameters.CWP * VAI * HCAN * *FHG, 0.5);
    //CWPC = (CWP*FHG)**0.5

    let TMP1 = fi::exp(-CWPC * Z0HG / HCAN);
    let TMP2 = fi::exp(-CWPC * (Z0H + ZPD) / HCAN);
    let TMPRAH2 = HCAN * fi::exp(CWPC) / CWPC * (TMP1 - TMP2);

    // aerodynamic resistances raw and rah between heights zpd+z0h and z0hg.
    let KH = fi::max(parameters.VKC * FV * (HCAN - ZPD), parameters.MPE);
    let RAMG = 0.;
    let RAHG = TMPRAH2 / KH;
    let RAWG = RAHG;

    // leaf boundary layer resistance
    let TMPRB = CWPC * 50. / (1. - fi::exp(-CWPC / 2.));
    let mut RB = TMPRB * fi::sqrt(parameters.DLEAF / UC);
    RB = fi::max(RB, 100.0);
    //RB = 200

    (RAMG, RAHG, RAWG, RB)
}

/// `STOMATA` -- Ball-Berry stomatal resistance and photosynthesis. Returns `(RS, PSN)`.
///
/// `VEGTYP` is in the Fortran signature and unused; it is dropped here.
fn stomata(
    parameters: &Parameters,
    APAR: f32,
    FOLN: f32,
    TV: f32,
    EI: f32,
    EA: f32,
    SFCTMP: f32,
    SFCPRS: f32,
    O2: f32,
    CO2: f32,
    IGS: f32,
    BTRAN: f32,
    RB: f32,
) -> (f32, f32) {
    const NITER: i32 = 3;

    // initialize RS=RSMAX and PSN=0 because will only do calculations
    // for APAR > 0, in which case RS <= RSMAX and PSN >= 0
    let CF = SFCPRS / (8.314 * SFCTMP) * 1.0e06;
    let mut RS = 1. / parameters.BP * CF;
    let mut PSN = 0.;

    if APAR <= 0. {
        return (RS, PSN);
    }

    let FNF = fi::min(FOLN / fi::max(parameters.MPE, parameters.FOLNMX), 1.0);
    let TC = TV - parameters.TFRZ;
    let PPF = 4.6 * APAR;
    let J = PPF * parameters.QE25;
    let KC = parameters.KC25 * f1(parameters.AKC, TC);
    let KO = parameters.KO25 * f1(parameters.AKO, TC);
    let AWC = KC * (1. + O2 / KO);
    let CP = 0.5 * KC / KO * O2 * 0.21;
    let VCMX = parameters.VCMX25 / f2(TC) * FNF * BTRAN * f1(parameters.AVCMX, TC);

    // first guess ci
    let mut CI = 0.7 * CO2 * parameters.C3PSN + 0.4 * CO2 * (1. - parameters.C3PSN);

    // rb: s/m -> s m**2 / umol
    let RLB = RB / CF;

    // constrain ea
    let CEA = fi::max(
        0.25 * EI * parameters.C3PSN + 0.40 * EI * (1. - parameters.C3PSN),
        fi::min(EA, EI),
    );

    // ci iteration
    // jref: C3PSN is equal to 1 for all veg types.
    for _ITER in 1..=NITER {
        let WJ = fi::max(CI - CP, 0.) * J / (CI + 2. * CP) * parameters.C3PSN
            + J * (1. - parameters.C3PSN);
        let WC = fi::max(CI - CP, 0.) * VCMX / (CI + AWC) * parameters.C3PSN
            + VCMX * (1. - parameters.C3PSN);
        let WE = 0.5 * VCMX * parameters.C3PSN + 4000. * VCMX * CI / SFCPRS * (1. - parameters.C3PSN);
        // MIN(WJ, WC, WE): gfortran evaluates a three-argument MIN pairwise, left to right.
        PSN = fi::min(fi::min(WJ, WC), WE) * IGS;

        let CS = fi::max(CO2 - 1.37 * RLB * SFCPRS * PSN, parameters.MPE);
        let A = parameters.MP * PSN * SFCPRS * CEA / (CS * EI) + parameters.BP;
        let B = (parameters.MP * PSN * SFCPRS / CS + parameters.BP) * RLB - 1.;
        let C = -RLB;
        let Q = if B >= 0. {
            -0.5 * (B + fi::sqrt(B * B - 4. * A * C))
        } else {
            -0.5 * (B - fi::sqrt(B * B - 4. * A * C))
        };
        let R1 = Q / A;
        let R2 = C / Q;
        RS = fi::max(R1, R2);
        CI = fi::max(CS - PSN * SFCPRS * 1.65 * RS, 0.);
    }

    // rs, rb:  s m**2 / umol -> s/m
    RS *= CF;

    (RS, PSN)
}

/// `CANRES` -- Jarvis canopy resistance. Returns `(RS, PSN)`; `PSN` is always `-999.99`.
fn canres(parameters: &Parameters, PAR: f32, TV: f32, BTRAN: f32, EAH: f32, SFCPRS: f32) -> (f32, f32) {
    // compute Q2 and Q2SAT
    let mut Q2 = 0.622 * EAH / (SFCPRS - 0.378 * EAH); // specific humidity [kg/kg]
    Q2 /= 1.0 + Q2; // mixing ratio [kg/kg]

    let (Q2SAT, _DQSDT2) = calhum(TV, SFCPRS);

    // contribution due to incoming solar radiation
    let FF = 2.0 * PAR / parameters.RGL;
    let mut RCS = (FF + parameters.RSMIN / parameters.RSMAX) / (1.0 + FF);
    RCS = fi::max(RCS, 0.0001);

    // contribution due to air temperature -- `(TOPT - TV)**2.0`, a multiply in gfortran
    let mut RCT = 1.0 - 0.0016 * fi::pow2_real(parameters.TOPT - TV);
    RCT = fi::max(RCT, 0.0001);

    // contribution due to vapor pressure deficit
    let mut RCQ = 1.0 / (1.0 + parameters.HS * fi::max(0., Q2SAT - Q2));
    RCQ = fi::max(RCQ, 0.01);

    // determine canopy resistance due to all factors
    let RS = parameters.RSMIN / (RCS * RCT * RCQ * BTRAN); // note BTRAN was originally RCSOIL
    let PSN = -999.99; // PSN not applied for dynamic carbon

    (RS, PSN)
}

/// `CALHUM` -- saturated mixing ratio and its derivative with temperature.
/// Returns `(Q2SAT, DQSDT2)`.
fn calhum(TV: f32, SFCPRS: f32) -> (f32, f32) {
    const A2: f32 = 17.67;
    const A3: f32 = 273.15;
    const A4: f32 = 29.65;
    const ELWV: f32 = 2.501E6;
    const A23M4: f32 = A2 * (A3 - A4);
    const E0: f32 = 0.611;
    const RV: f32 = 461.0;
    const EPSILON: f32 = 0.622;

    // Q2SAT: saturated mixing ratio
    let ES = E0 * fi::exp(ELWV / RV * (1. / A3 - 1. / TV));
    // convert SFCPRS from Pa to KPa
    let SFCPRSX = SFCPRS * 1.0E-3;
    let Q2SAT = EPSILON * ES / (SFCPRSX - ES);

    // DQSDT2 is calculated using Q2SAT converted to specific humidity (g/kg) from mix. ratio (g/g)
    let DQSDT2 = ((Q2SAT * 1.0E3) / (1. + (Q2SAT * 1.0E3))) * A23M4 / fi::powi(TV - A4, 2);

    (Q2SAT, DQSDT2)
}

/// `SFCDIF1` -- surface drag coefficient CM for momentum and CH for heat (Monin-Obukhov).
fn sfcdif1(
    parameters: &Parameters,
    ITER: i32,
    SFCTMP: f32,
    RHOAIR: f32,
    H: f32,
    QAIR: f32,
    ZLVL: f32,
    ZPD: f32,
    Z0M: f32,
    Z0H: f32,
    UR: f32,
    MOZ: &mut f32,
    MOZSGN: &mut i32,
    FM: &mut f32,
    FH: &mut f32,
    FM2: &mut f32,
    FH2: &mut f32,
    CM: &mut f32,
    CH: &mut f32,
    FV: &mut f32,
    CH2: &mut f32,
) {
    // Monin-Obukhov stability parameter moz for next iteration
    let MOZOLD = *MOZ;

    // Upstream prints "WARNING: critical problem: ZLVL <= ZPD; model stops" here and does
    // not, in fact, stop. Nothing to reproduce.

    let TMPCM = fi::log((ZLVL - ZPD) / Z0M);
    let TMPCH = fi::log((ZLVL - ZPD) / Z0H);
    let TMPCM2 = fi::log((2.0 + Z0M) / Z0M);
    let TMPCH2 = fi::log((2.0 + Z0H) / Z0H);

    let mut MOZ2;
    if ITER == 1 {
        *FV = 0.0;
        *MOZ = 0.0;
        MOZ2 = 0.0;
    } else {
        let TVIR = (1. + 0.61 * QAIR) * SFCTMP;
        let mut TMP1 = parameters.VKC * (parameters.GRAV / TVIR) * H / (RHOAIR * parameters.CPAIR);
        if TMP1.abs() <= parameters.MPE {
            TMP1 = parameters.MPE;
        }
        let MOL = -1. * fi::powi(*FV, 3) / TMP1;
        *MOZ = fi::min((ZLVL - ZPD) / MOL, 1.);
        MOZ2 = fi::min((2.0 + Z0H) / MOL, 1.);
    }

    // accumulate number of times moz changes sign.
    if MOZOLD * *MOZ < 0. {
        *MOZSGN += 1;
    }
    if *MOZSGN >= 2 {
        *MOZ = 0.;
        *FM = 0.;
        *FH = 0.;
        MOZ2 = 0.;
        *FM2 = 0.;
        *FH2 = 0.;
    }

    // evaluate stability-dependent variables using moz from prior iteration
    let (FMNEW, FHNEW, FM2NEW, FH2NEW);
    if *MOZ < 0. {
        let TMP1 = fi::powf(1. - 16. * *MOZ, 0.25);
        let TMP2 = fi::log((1. + TMP1 * TMP1) / 2.);
        let TMP3 = fi::log((1. + TMP1) / 2.);
        FMNEW = 2. * TMP3 + TMP2 - 2. * fi::atan(TMP1) + 1.5707963;
        FHNEW = 2. * TMP2;

        // 2-meter
        let TMP12 = fi::powf(1. - 16. * MOZ2, 0.25);
        let TMP22 = fi::log((1. + TMP12 * TMP12) / 2.);
        let TMP32 = fi::log((1. + TMP12) / 2.);
        FM2NEW = 2. * TMP32 + TMP22 - 2. * fi::atan(TMP12) + 1.5707963;
        FH2NEW = 2. * TMP22;
    } else {
        FMNEW = -5. * *MOZ;
        FHNEW = FMNEW;
        FM2NEW = -5. * MOZ2;
        FH2NEW = FM2NEW;
    }

    // except for first iteration, weight stability factors for previous
    // iteration to help avoid flip-flops from one iteration to the next
    if ITER == 1 {
        *FM = FMNEW;
        *FH = FHNEW;
        *FM2 = FM2NEW;
        *FH2 = FH2NEW;
    } else {
        *FM = 0.5 * (*FM + FMNEW);
        *FH = 0.5 * (*FH + FHNEW);
        *FM2 = 0.5 * (*FM2 + FM2NEW);
        *FH2 = 0.5 * (*FH2 + FH2NEW);
    }

    // exchange coefficients
    *FH = fi::min(*FH, 0.9 * TMPCH);
    *FM = fi::min(*FM, 0.9 * TMPCM);
    *FH2 = fi::min(*FH2, 0.9 * TMPCH2);
    *FM2 = fi::min(*FM2, 0.9 * TMPCM2);

    let mut CMFM = TMPCM - *FM;
    let mut CHFH = TMPCH - *FH;
    let mut CM2FM2 = TMPCM2 - *FM2;
    let mut CH2FH2 = TMPCH2 - *FH2;
    if CMFM.abs() <= parameters.MPE {
        CMFM = parameters.MPE;
    }
    if CHFH.abs() <= parameters.MPE {
        CHFH = parameters.MPE;
    }
    if CM2FM2.abs() <= parameters.MPE {
        CM2FM2 = parameters.MPE;
    }
    if CH2FH2.abs() <= parameters.MPE {
        CH2FH2 = parameters.MPE;
    }
    *CM = parameters.VKC * parameters.VKC / (CMFM * CMFM);
    *CH = parameters.VKC * parameters.VKC / (CMFM * CHFH);
    *CH2 = parameters.VKC * parameters.VKC / (CM2FM2 * CH2FH2);

    // friction velocity
    *FV = UR * fi::sqrt(*CM);
    *CH2 = parameters.VKC * *FV / CH2FH2;
}

/// `SFCDIF2` -- surface layer exchange coefficients via iterative process, Chen et al (1997).
///
/// `AKMS`/`AKHS` are `energy%CM`/`energy%CH` (the caller divides the wind speed back out),
/// `RLMO` is the caller's `MOZ`, `WSTAR2` its `WSTAR` and `USTAR` its `FV`. `USTAR` is
/// `intent(out)` upstream but is read before it is written on every iteration after the
/// first, so it is `&mut` here.
fn sfcdif2(
    parameters: &Parameters,
    ITER: i32,
    Z0: f32,
    THZ0: f32,
    THLM: f32,
    SFCSPD: f32,
    ZLM: f32,
    AKMS: &mut f32,
    AKHS: &mut f32,
    RLMO: &mut f32,
    WSTAR2: &mut f32,
    USTAR: &mut f32,
) {
    const WWST: f32 = 1.2;
    const WWST2: f32 = WWST * WWST;
    const VKRM: f32 = 0.40;
    const EXCM: f32 = 0.001;
    const BETA: f32 = 1.0 / 270.0;
    const WOLD: f32 = 0.15;
    const WNEW: f32 = 1.0 - WOLD;
    const PIHF: f32 = 3.14159265 / 2.;
    const EPSU2: f32 = 1.0E-4;
    const EPSUST: f32 = 0.07;
    const ZTMIN: f32 = -5.0;
    const ZTMAX: f32 = 1.0;
    const HPBL: f32 = 1000.0;
    const SQVISC: f32 = 258.2;
    const RIC: f32 = 0.183;
    const RRIC: f32 = 1.0 / RIC;
    const FHNEU: f32 = 0.8;
    const RFC: f32 = 0.191;
    const RFAC: f32 = RIC / (FHNEU * RFC * RFC);

    let BTG = BETA * parameters.GRAV; // BTG orig. a const. parameter
    let ELFC = VKRM * BTG; // ELFC orig. a const. parameter

    // THIS ROUTINE SFCDIF CAN HANDLE BOTH OVER OPEN WATER (SEA, OCEAN) AND
    // OVER SOLID SURFACE (LAND, SEA-ICE).
    //     ZTFC: RATIO OF ZOH/ZOM  LESS OR EQUAL THAN 1
    //     CZIL: CONSTANT C IN Zilitinkevich, S. S.1995,:NOTE ABOUT ZT
    let ILECH = 0;

    let ZILFC = -parameters.CZIL * VKRM * SQVISC;
    let ZU = Z0;
    let RDZ = 1. / ZLM;
    let CXCH = EXCM * RDZ;
    let DTHV = THLM - THZ0;

    // BELJARS CORRECTION OF USTAR
    let DU2 = fi::max(SFCSPD * SFCSPD, EPSU2);
    let BTGH = BTG * HPBL;

    if ITER == 1 {
        if BTGH * *AKHS * DTHV != 0.0 {
            *WSTAR2 = WWST2 * fi::powf((BTGH * *AKHS * DTHV).abs(), 2. / 3.);
        } else {
            *WSTAR2 = 0.0;
        }
        *USTAR = fi::max(fi::sqrt(*AKMS * fi::sqrt(DU2 + *WSTAR2)), EPSUST);
        *RLMO = ELFC * *AKHS * DTHV / fi::powi(*USTAR, 3);
    }

    // ZILITINKEVITCH APPROACH FOR ZT
    let mut ZT = fi::max(1.0E-6, fi::exp(ZILFC * fi::sqrt(*USTAR * Z0)) * Z0);
    let ZSLU = ZLM + ZU;
    let mut ZSLT = ZLM + ZT;
    let RLOGU = fi::log(ZSLU / ZU);
    let mut RLOGT = fi::log(ZSLT / ZT);

    // 1./MONIN-OBUKKHOV LENGTH-SCALE
    let mut ZETALT = fi::max(ZSLT * *RLMO, ZTMIN);
    *RLMO = ZETALT / ZSLT;
    let mut ZETALU = ZSLU * *RLMO;
    let mut ZETAU = ZU * *RLMO;
    let mut ZETAT = ZT * *RLMO;

    let (mut SIMM, mut SIMH);
    if ILECH == 0 {
        if *RLMO < 0. {
            let XLU4 = 1. - 16. * ZETALU;
            let XLT4 = 1. - 16. * ZETALT;
            let XU4 = 1. - 16. * ZETAU;
            let XT4 = 1. - 16. * ZETAT;
            let XLU = fi::sqrt(fi::sqrt(XLU4));
            let XLT = fi::sqrt(fi::sqrt(XLT4));
            let XU = fi::sqrt(fi::sqrt(XU4));

            let XT = fi::sqrt(fi::sqrt(XT4));
            let PSMZ = pspmu(XU, PIHF);
            SIMM = pspmu(XLU, PIHF) - PSMZ + RLOGU;
            let PSHZ = psphu(XT);
            SIMH = psphu(XLT) - PSHZ + RLOGT;
        } else {
            ZETALU = fi::min(ZETALU, ZTMAX);
            ZETALT = fi::min(ZETALT, ZTMAX);
            ZETAU = fi::min(ZETAU, ZTMAX / (ZSLU / ZU)); // Barlage: add limit on ZETAU/ZETAT
            ZETAT = fi::min(ZETAT, ZTMAX / (ZSLT / ZT)); // Barlage: prevent SIMM/SIMH < 0
            let PSMZ = pspms(ZETAU);
            SIMM = pspms(ZETALU) - PSMZ + RLOGU;
            let PSHZ = psphs(ZETAT);
            SIMH = psphs(ZETALT) - PSHZ + RLOGT;
        }
    } else {
        // LECH'S FUNCTIONS
        if *RLMO < 0. {
            let PSMZ = pslmu(ZETAU);
            SIMM = pslmu(ZETALU) - PSMZ + RLOGU;
            let PSHZ = pslhu(ZETAT);
            SIMH = pslhu(ZETALT) - PSHZ + RLOGT;
        } else {
            ZETALU = fi::min(ZETALU, ZTMAX);
            ZETALT = fi::min(ZETALT, ZTMAX);
            let PSMZ = pslms(ZETAU, RRIC);
            SIMM = pslms(ZETALU, RRIC) - PSMZ + RLOGU;
            let PSHZ = pslhs(ZETAT, RFAC);
            SIMH = pslhs(ZETALT, RFAC) - PSHZ + RLOGT;
        }
    }

    // BELJAARS CORRECTION FOR USTAR
    *USTAR = fi::max(fi::sqrt(*AKMS * fi::sqrt(DU2 + *WSTAR2)), EPSUST);

    // ZILITINKEVITCH FIX FOR ZT
    ZT = fi::max(1.0E-6, fi::exp(ZILFC * fi::sqrt(*USTAR * Z0)) * Z0);
    ZSLT = ZLM + ZT;
    RLOGT = fi::log(ZSLT / ZT);
    let _ = RLOGT;
    let USTARK = *USTAR * VKRM;
    if SIMM < 1.0e-6 {
        SIMM = 1.0e-6; // Limit stability function
    }
    *AKMS = fi::max(USTARK / SIMM, CXCH);
    // IF STATEMENTS TO AVOID TANGENT LINEAR PROBLEMS NEAR ZERO
    if SIMH < 1.0e-6 {
        SIMH = 1.0e-6; // Limit stability function
    }
    *AKHS = fi::max(USTARK / SIMH, CXCH);

    if BTGH * *AKHS * DTHV != 0.0 {
        *WSTAR2 = WWST2 * fi::powf((BTGH * *AKHS * DTHV).abs(), 2. / 3.);
    } else {
        *WSTAR2 = 0.0;
    }
    let RLMN = ELFC * *AKHS * DTHV / fi::powi(*USTAR, 3);
    //     IF(ABS((RLMN-RLMO)/RLMA).LT.EPSIT)    GO TO 110
    let RLMA = *RLMO * WOLD + RLMN * WNEW;
    *RLMO = RLMA;
}

/// `ESAT` -- saturation vapor pressure and its derivative, over water and over ice.
/// Returns `(ESW, ESI, DESW, DESI)`.
fn esat(T: f32) -> (f32, f32, f32, f32) {
    const A0: f32 = 6.107799961;
    const A1: f32 = 4.436518521E-01;
    const A2: f32 = 1.428945805E-02;
    const A3: f32 = 2.650648471E-04;
    const A4: f32 = 3.031240396E-06;
    const A5: f32 = 2.034080948E-08;
    const A6: f32 = 6.136820929E-11;

    const B0: f32 = 6.109177956;
    const B1: f32 = 5.034698970E-01;
    const B2: f32 = 1.886013408E-02;
    const B3: f32 = 4.176223716E-04;
    const B4: f32 = 5.824720280E-06;
    const B5: f32 = 4.838803174E-08;
    const B6: f32 = 1.838826904E-10;

    const C0: f32 = 4.438099984E-01;
    const C1: f32 = 2.857002636E-02;
    const C2: f32 = 7.938054040E-04;
    const C3: f32 = 1.215215065E-05;
    const C4: f32 = 1.036561403E-07;
    const C5: f32 = 3.532421810e-10;
    const C6: f32 = -7.090244804E-13;

    const D0: f32 = 5.030305237E-01;
    const D1: f32 = 3.773255020E-02;
    const D2: f32 = 1.267995369E-03;
    const D3: f32 = 2.477563108E-05;
    const D4: f32 = 3.005693132E-07;
    const D5: f32 = 2.158542548E-09;
    const D6: f32 = 7.131097725E-12;

    let ESW = 100. * (A0 + T * (A1 + T * (A2 + T * (A3 + T * (A4 + T * (A5 + T * A6))))));
    let ESI = 100. * (B0 + T * (B1 + T * (B2 + T * (B3 + T * (B4 + T * (B5 + T * B6))))));
    let DESW = 100. * (C0 + T * (C1 + T * (C2 + T * (C3 + T * (C4 + T * (C5 + T * C6))))));
    let DESI = 100. * (D0 + T * (D1 + T * (D2 + T * (D3 + T * (D4 + T * (D5 + T * D6))))));

    (ESW, ESI, DESW, DESI)
}

/// `TDC` -- Kelvin to degree Celsius with limit -50 to +50.
fn tdc(T: f32, TFRZ: f32) -> f32 {
    fi::min(50., fi::max(-50., T - TFRZ))
}

/// `F1` -- generic temperature response.
fn f1(AB: f32, BC: f32) -> f32 {
    fi::powf(AB, (BC - 25.) / 10.)
}

/// `F2` -- generic temperature inhibition.
fn f2(AB: f32) -> f32 {
    1. + fi::exp((-2.2E05 + 710. * (AB + 273.16)) / (8.314 * (AB + 273.16)))
}

// ----------------------------------------------------------------------
// DEFINE FUNCTIONS FOR SFCDIF2
// ----------------------------------------------------------------------

/// LECH'S SURFACE FUNCTIONS
fn pslmu(ZZ: f32) -> f32 {
    -0.96 * fi::log(1.0 - 4.5 * ZZ)
}

fn pslms(ZZ: f32, RRIC: f32) -> f32 {
    ZZ * RRIC - 2.076 * (1. - 1. / (ZZ + 1.))
}

fn pslhu(ZZ: f32) -> f32 {
    -0.96 * fi::log(1.0 - 4.5 * ZZ)
}

fn pslhs(ZZ: f32, RFAC: f32) -> f32 {
    ZZ * RFAC - 2.076 * (1. - 1. / (ZZ + 1.))
}

/// PAULSON'S SURFACE FUNCTIONS
fn pspmu(XX: f32, PIHF: f32) -> f32 {
    -2. * fi::log((XX + 1.) * 0.5) - fi::log((XX * XX + 1.) * 0.5) + 2. * fi::atan(XX) - PIHF
}

fn pspms(YY: f32) -> f32 {
    5. * YY
}

fn psphu(XX: f32) -> f32 {
    -2. * fi::log((XX * XX + 1.) * 0.5)
}

fn psphs(YY: f32) -> f32 {
    5. * YY
}
