//! Port of `src/EnergyModule.f90` @ 0ff055e.
//!
//! The fourth of the five physics calls: the surface energy balance, from snow cover and
//! roughness through radiation, turbulent fluxes, the snow/soil temperature solve and phase
//! change.
//!
//! # `parameters` is not all parameters
//!
//! `EnergyMain` writes `parameters.VAI` and `parameters.VEG` every timestep, so this module
//! takes `&mut Parameters` for the same reason `InterceptionModule` does.

#![allow(non_snake_case)]

use std::fmt;

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::levels::Levels;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::et_flux::{bare_flux_main, vege_flux_main};
use crate::physics::precip_heat::precip_heat;
use crate::physics::shortwave_radiation::shortwave_radiation_main;
use crate::physics::snow_soil_temp::{phasechange, tsnosoi};
use crate::physics::thermal_properties::thermoprop;
use crate::water::Water;

/// The one `STOP` in the energy column: emitted longwave came out non-positive, which upstream
/// attributes to an inconsistent `SHDFAC` / LAI input.
///
/// Carries what the Fortran prints before stopping.
#[derive(Debug, Clone, PartialEq)]
pub struct EmittedLongwaveNotPositive {
    pub iloc: i32,
    pub jloc: i32,
    pub fveg: f32,
    pub vai: f32,
    pub tv: f32,
    pub tg: f32,
    pub lwdn: f32,
    pub fira: f32,
    pub snowh: f32,
}

impl fmt::Display for EmittedLongwaveNotPositive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "emitted longwave <0; skin T may be wrong due to inconsistent input of SHDFAC with \
             LAI ({} {} SHDFAC={} VAI={} TV={} TG={} LWDN={} FIRA={} SNOWH={})",
            self.iloc, self.jloc, self.fveg, self.vai, self.tv, self.tg, self.lwdn, self.fira,
            self.snowh
        )
    }
}

impl std::error::Error for EmittedLongwaveNotPositive {}

/// `EnergyMain`.
pub fn energy_main(
    domain: &mut Domain,
    levels: &Levels,
    options: &Options,
    parameters: &mut Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &mut Water,
) -> Result<(), EmittedLongwaveNotPositive> {
    // Initialize the the fluxes from the vegetated fraction
    energy.TAUXV = 0.;
    energy.TAUYV = 0.;
    energy.IRC = 0.;
    energy.SHC = 0.;
    energy.IRG = 0.;
    energy.SHG = 0.;
    energy.EVG = 0.;
    energy.EVC = 0.;
    energy.TR = 0.;
    energy.GHV = 0.;
    energy.PSNSUN = 0.;
    energy.PSNSHA = 0.;
    energy.T2MV = 0.;
    energy.Q2V = 0.;
    energy.CHV = 0.;
    energy.CHLEAF = 0.;
    energy.CHUC = 0.;
    energy.CHV2 = 0.;
    energy.RB = 0.;

    // Determine whether grid cell is vegetated or not
    parameters.VAI = parameters.ELAI + parameters.ESAI;
    parameters.VEG = parameters.VAI > 0.0;

    // Compute fraction of grid cell with snow cover [Niu and Yang, 2007, JGR]
    // TO DO: MFSNO (m in Niu and Yang) is set to 2.5 for all vegtypes
    // Reference paper indicates MFSNO varies in space (values of 1.0, 1.6, 1.8)
    // KSJ 2021-04-06
    water.FSNO = 0.0;
    if water.SNOWH > 0.0 {
        water.BDSNO = water.SNEQV / water.SNOWH;
        let FMELT = fi::powf(water.BDSNO / 100., parameters.MFSNO);
        water.FSNO = parameters.SCAMAX * fi::tanh(water.SNOWH / (2.5 * parameters.Z0 * FMELT)); // eq. 4 from Niu and Yang (2007)
    }

    // Compute ground roughness length
    if domain.ist == 2 {
        if energy.TG <= parameters.TFRZ {
            energy.Z0MG = (0.01 * (1.0 - water.FSNO)) + (water.FSNO * parameters.Z0SNO);
        } else {
            energy.Z0MG = 0.01;
        }
    } else {
        energy.Z0MG = (parameters.Z0 * (1.0 - water.FSNO)) + (water.FSNO * parameters.Z0SNO);
    }

    // Compute roughness length and displacement height
    let mut ZPDG = water.SNOWH;
    if parameters.VEG {
        energy.Z0M = parameters.Z0MVT;
        energy.ZPD = 0.65 * parameters.HVT;
        if water.SNOWH > energy.ZPD {
            energy.ZPD = water.SNOWH;
        }
    } else {
        energy.Z0M = energy.Z0MG;
        energy.ZPD = ZPDG;
    }

    // special case for urban
    if parameters.urban_flag {
        energy.Z0MG = parameters.Z0MVT;
        ZPDG = 0.65 * parameters.HVT;
        energy.Z0M = energy.Z0MG;
        energy.ZPD = ZPDG;
    }

    energy.ZLVL = fi::max(energy.ZPD, parameters.HVT) + domain.zref;
    if ZPDG >= energy.ZLVL {
        energy.ZLVL = ZPDG + domain.zref;
    }

    // Compute snow and soil thermodynamic properties
    thermoprop(domain, levels, parameters, energy, water);

    // Compute the heat advected by precipitation
    precip_heat(parameters, forcing, energy, water);

    // Compute net solar radiation
    // Subroutine was formerly called RADIATION
    // Changed name because it only computes shortwave
    // KSJ 2021-04-20
    shortwave_radiation_main(domain, options, parameters, forcing, energy, water);

    // vegetation and ground emissivity
    energy.EMV = 1. - fi::exp(-(parameters.ELAI + parameters.ESAI) / 1.0);
    if energy.ICE == 1 {
        energy.EMG = 0.98 * (1. - water.FSNO) + 1.0 * water.FSNO;
    } else {
        energy.EMG = parameters.EG[domain.ist] * (1. - water.FSNO) + 1.0 * water.FSNO;
    }

    // calculate soil moisture stress factor controlling stomatal resistance
    water.BTRAN = 0.;

    if domain.ist == 1 {
        for IZ in 1..=parameters.NROOT {
            // GX and PSI are assigned under every OPT_BTR / OPT_SUB the namelist check allows;
            // the zero initialisers stand in for the Fortran's undefined value otherwise.
            let mut GX = 0.0;
            let mut PSI = 0.0;
            if options.opt_btr == 1 {
                // Noah
                GX = (water.sh2o[IZ] - parameters.smcwlt[IZ])
                    / (parameters.smcref[IZ] - parameters.smcwlt[IZ]);
            }
            if options.opt_btr == 2 {
                // CLM
                if options.opt_sub == 1 {
                    // Noah-MP style subsurface
                    PSI = fi::max(
                        parameters.PSIWLT,
                        -parameters.psisat[IZ]
                            * fi::powf(
                                fi::max(0.01, water.sh2o[IZ]) / parameters.smcmax[IZ],
                                -parameters.bexp[IZ],
                            ),
                    );
                }
                if options.opt_sub == 2 {
                    // one-way coupled subsurface
                    PSI = water.zwt - (domain.zsoil[IZ] + (domain.dzsnso[IZ] / 2.)); // set PSI to be midpoint of layer above water table
                }
                GX = (1. - PSI / parameters.PSIWLT) / (1. + parameters.psisat[IZ] / parameters.PSIWLT);
            }
            if options.opt_btr == 3 {
                // SSiB
                if options.opt_sub == 1 {
                    // Noah-MP style subsurface
                    PSI = fi::max(
                        parameters.PSIWLT,
                        -parameters.psisat[IZ]
                            * fi::powf(
                                fi::max(0.01, water.sh2o[IZ]) / parameters.smcmax[IZ],
                                -parameters.bexp[IZ],
                            ),
                    );
                }
                if options.opt_sub == 2 {
                    // one-way coupled subsurface
                    PSI = water.zwt - (domain.zsoil[IZ] + (domain.dzsnso[IZ] / 2.)); // set PSI to be midpoint of layer above water table
                }
                GX = 1. - fi::exp(-5.8 * fi::log(parameters.PSIWLT / PSI));
            }
            if options.opt_btr == 4 {
                // Maximum ETRAN (no soil moisture stress)
                GX = 1.0;
            }
            GX = fi::min(1., fi::max(0., GX));
            water.BTRANI[IZ] = fi::max(
                parameters.MPE,
                domain.dzsnso[IZ] / (-domain.zsoil[parameters.NROOT]) * GX,
            );
            water.BTRAN += water.BTRANI[IZ];
        }
        water.BTRAN = fi::max(parameters.MPE, water.BTRAN);
        for IZ in 1..=parameters.NROOT {
            water.BTRANI[IZ] /= water.BTRAN;
        }
    }

    // calculate soil surface resistance for ground evap.
    let BEVAP = fi::max(0.0, water.sh2o[1] / parameters.smcmax[1]);
    if domain.ist == 2 {
        energy.RSURF = 1.0; // avoid being divided by 0
        energy.RHSUR = 1.0;
    } else {
        if options.opt_rsf == 1 || options.opt_rsf == 4 || options.opt_rsf == 5 {
            // RSURF based on Sakaguchi and Zeng, 2009
            // taking the "residual water content" to be the wilting point,
            // and correcting the exponent on the D term (typo in SZ09 ?)
            let L_RSURF = (-domain.zsoil[1])
                * (fi::exp(fi::powf(
                    1.0 - fi::min(1.0, water.sh2o[1] / parameters.smcmax[1]),
                    parameters.RSURF_EXP,
                )) - 1.0)
                / (2.71828 - 1.0);
            let D_RSURF = 2.2E-5
                * parameters.smcmax[1]
                * parameters.smcmax[1]
                * fi::powf(
                    1.0 - parameters.smcwlt[1] / parameters.smcmax[1],
                    2.0 + 3.0 / parameters.bexp[1],
                );
            energy.RSURF = L_RSURF / D_RSURF;
        } else if options.opt_rsf == 2 {
            energy.RSURF = water.FSNO * 1. + (1. - water.FSNO) * fi::exp(8.25 - 4.225 * BEVAP); // Sellers (1992) ! Older RSURF computations
        } else if options.opt_rsf == 3 {
            energy.RSURF = water.FSNO * 1. + (1. - water.FSNO) * fi::exp(8.25 - 6.0 * BEVAP); // adjusted to decrease RSURF for wet soil
        }
        if options.opt_rsf == 4 {
            // AD: FSNO weighted; snow RSURF set in MPTABLE v3.8
            energy.RSURF = 1.
                / (water.FSNO * (1. / parameters.RSURF_SNOW)
                    + (1. - water.FSNO) * (1. / fi::max(energy.RSURF, 0.001)));
        }
        if options.opt_rsf == 5 {
            // Minimize RSURF for soil evap; preserve sublimation when snow present
            // Weighted mean: RSURF_SNOW for snow fraction, 0.001 for non-snow fraction
            energy.RSURF =
                1. / (water.FSNO * (1. / parameters.RSURF_SNOW) + (1. - water.FSNO) * (1. / 0.001));
        }
        if water.sh2o[1] < 0.01 && water.SNOWH == 0. {
            energy.RSURF = 1.0E6;
        }
        let PSI = -parameters.psisat[1]
            * fi::powf(
                fi::max(0.01, water.sh2o[1]) / parameters.smcmax[1],
                -parameters.bexp[1],
            );
        energy.RHSUR = water.FSNO
            + (1. - water.FSNO) * fi::exp(PSI * parameters.GRAV / (parameters.RW * energy.TG));
    }

    // urban - jref
    if parameters.urban_flag && water.SNOWH == 0. {
        energy.RSURF = 1.0E6;
    }

    // set psychrometric constant
    if energy.TV > parameters.TFRZ {
        // Barlage: add distinction between ground and vegetation in v3.6
        energy.LATHEAV = parameters.HVAP;
        energy.FROZEN_CANOPY = false;
    } else {
        energy.LATHEAV = parameters.HSUB;
        energy.FROZEN_CANOPY = true;
    }
    energy.GAMMAV = parameters.CPAIR * forcing.SFCPRS / (0.622 * energy.LATHEAV);

    if energy.TG > parameters.TFRZ {
        energy.LATHEAG = parameters.HVAP;
        energy.FROZEN_GROUND = false;
    } else {
        energy.LATHEAG = parameters.HSUB;
        energy.FROZEN_GROUND = true;
    }
    energy.GAMMAG = parameters.CPAIR * forcing.SFCPRS / (0.622 * energy.LATHEAG);

    // Calculate surface temperatures of the ground and canopy and energy fluxes
    if parameters.VEG && parameters.FVEG > 0. {
        energy.TGV = energy.TG;
        energy.CMV = energy.CM;
        energy.CHV = energy.CH;

        // Calculate canopy energy fluxes
        vege_flux_main(domain, options, parameters, forcing, energy, water);
    }

    energy.TGB = energy.TG;
    energy.CMB = energy.CM;
    energy.CHB = energy.CH;

    bare_flux_main(domain, options, parameters, forcing, energy, water);

    // energy balance at vege canopy: SAV          =(IRC+SHC+EVC+TR)     *FVEG  at   FVEG
    // energy balance at vege ground: SAG*    FVEG =(IRG+SHG+EVG+GHV)    *FVEG  at   FVEG
    // energy balance at bare ground: SAG*(1.-FVEG)=(IRB+SHB+EVB+GHB)*(1.-FVEG) at 1-FVEG

    if parameters.VEG && parameters.FVEG > 0. {
        let FVEG = parameters.FVEG;
        energy.TAUX = FVEG * energy.TAUXV + (1.0 - FVEG) * energy.TAUXB;
        energy.TAUY = FVEG * energy.TAUYV + (1.0 - FVEG) * energy.TAUYB;
        energy.FIRA = FVEG * energy.IRG + (1.0 - FVEG) * energy.IRB + energy.IRC;
        energy.FSH = FVEG * energy.SHG + (1.0 - FVEG) * energy.SHB + energy.SHC;
        energy.FGEV = FVEG * energy.EVG + (1.0 - FVEG) * energy.EVB;
        energy.SSOIL = FVEG * energy.GHV + (1.0 - FVEG) * energy.GHB;
        energy.GH = FVEG * energy.GHV + (1.0 - FVEG) * energy.GHB; // FVEG = 1. Hence, this is a weighted average
        energy.FCEV = energy.EVC;
        energy.FCTR = energy.TR;
        energy.PAH = FVEG * energy.PAHG + (1.0 - FVEG) * energy.PAHB + energy.PAHV;
        energy.TG = FVEG * energy.TGV + (1.0 - FVEG) * energy.TGB;
        energy.T2M = FVEG * energy.T2MV + (1.0 - FVEG) * energy.T2MB;
        energy.TS = FVEG * energy.TV + (1.0 - FVEG) * energy.TGB;
        energy.CM = FVEG * energy.CMV + (1.0 - FVEG) * energy.CMB; // better way to average?
        energy.CH = FVEG * energy.CHV + (1.0 - FVEG) * energy.CHB;
        energy.Q1 = FVEG * (energy.EAH * 0.622 / (forcing.SFCPRS - 0.378 * energy.EAH))
            + (1.0 - FVEG) * energy.QSFC;
        energy.Q2E = FVEG * energy.Q2V + (1.0 - FVEG) * energy.Q2B;
        energy.Z0WRF = energy.Z0M;
    } else {
        energy.TAUX = energy.TAUXB;
        energy.TAUY = energy.TAUYB;
        energy.FIRA = energy.IRB;
        energy.FSH = energy.SHB;
        energy.FGEV = energy.EVB;
        energy.SSOIL = energy.GHB;
        energy.TG = energy.TGB; // could use more associated variables to unclutter the code
        energy.T2M = energy.T2MB;
        energy.FCEV = 0.;
        energy.FCTR = 0.;
        energy.PAH = energy.PAHB;
        energy.TS = energy.TG;
        energy.CM = energy.CMB;
        energy.CH = energy.CHB;
        energy.Q1 = energy.QSFC;
        energy.Q2E = energy.Q2B;
        energy.RSSUN = 0.0;
        energy.RSSHA = 0.0;
        energy.TGV = energy.TGB;
        energy.CHV = energy.CHB;
        energy.Z0WRF = energy.Z0MG;
    }

    let FIRE = forcing.LWDN + energy.FIRA;
    if FIRE <= 0. {
        return Err(EmittedLongwaveNotPositive {
            iloc: domain.iloc,
            jloc: domain.jloc,
            fveg: parameters.FVEG,
            vai: parameters.VAI,
            tv: energy.TV,
            tg: energy.TG,
            lwdn: forcing.LWDN,
            fira: energy.FIRA,
            snowh: water.SNOWH,
        });
    }

    // Compute a net emissivity
    energy.EMISSI = parameters.FVEG
        * (energy.EMG * (1. - energy.EMV)
            + energy.EMV
            + energy.EMV * (1. - energy.EMV) * (1. - energy.EMG))
        + (1. - parameters.FVEG) * energy.EMG;

    // When we're computing a TRAD, subtract from the emitted IR the
    // reflected portion of the incoming LWDN, so we're just
    // considering the IR originating in the canopy/ground system.
    energy.TRAD = fi::powf(
        (FIRE - (1. - energy.EMISSI) * forcing.LWDN) / (energy.EMISSI * parameters.SB),
        0.25,
    );

    energy.APAR = energy.PARSUN * energy.LAISUN + energy.PARSHA * energy.LAISHA;
    energy.PSN = energy.PSNSUN * energy.LAISUN + energy.PSNSHA * energy.LAISHA;

    // calculate 3L snow & 4L soil temperatures
    tsnosoi(
        parameters,
        levels,
        domain,
        options,
        forcing,
        water.ISNOW,
        energy.SSOIL,
        &energy.DF,
        &energy.HCPCT,
        water.SNOWH,
        &mut energy.STC,
    );

    // AW:  need to decide what to do with STC if no subsurface will be simulated
    //      ie, should soil layers be 0C if there is snow and TGB if not?

    // adjusting snow surface temperature
    if options.opt_stc == 2 && water.SNOWH > 0.05 && energy.TG > parameters.TFRZ {
        energy.TGV = parameters.TFRZ;
        energy.TGB = parameters.TFRZ;
        if parameters.VEG && parameters.FVEG > 0. {
            energy.TG = parameters.FVEG * energy.TGV + (1.0 - parameters.FVEG) * energy.TGB;
            energy.TS = parameters.FVEG * energy.TV + (1.0 - parameters.FVEG) * energy.TGB;
        } else {
            energy.TG = energy.TGB;
            energy.TS = energy.TGB;
        }
    }

    // Energy released or consumed by snow & frozen soil
    phasechange(parameters, domain, energy, water, options, levels.nsnow, levels.nsoil);

    // derived diagnostic variables
    energy.LH = energy.FCEV + energy.FGEV + energy.FCTR;

    // Compute TGS, or ground surface temperature for passing to a subsurface module
    // TGS is equal to TG when SNOWH <= 0.05 and equal to the temperature of the bottom snow
    // element when SNOWH > 0.05
    if water.SNOWH > 0.05 {
        energy.TGS = energy.STC[0];
    } else {
        energy.TGS = energy.TG;
    }

    Ok(())
}
