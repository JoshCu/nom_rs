//! Port of `src/AlbedoModule.f90` @ 0ff055e.
//!
//! Surface albedo for the two wavebands: snow ageing, the BATS or CLASS snow albedo, the ground
//! albedo, and the two-stream canopy radiative transfer that turns them into the absorbed,
//! reflected and transmitted fractions `NetSolarRadiation` uses.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::water::Water;

/// `ALBEDO`.
///
/// `levels` and `forcing` are in the Fortran signature and unused; they are dropped here.
pub fn albedo(
    domain: &Domain,
    options: &Options,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &Water,
) {
    // initialize output because solar radiation only done if COSZ > 0
    energy.BGAP = 0.0;
    energy.WGAP = 0.0;

    for IB in 1..=parameters.NBAND {
        energy.ALBD[IB] = 0.0;
        energy.ALBI[IB] = 0.0;
        energy.ALBGRD[IB] = 0.0;
        energy.ALBGRI[IB] = 0.0;
        energy.ALBSND[IB] = 0.0;
        energy.ALBSNI[IB] = 0.0;
        energy.FABD[IB] = 0.0;
        energy.FABI[IB] = 0.0;
        energy.FTDD[IB] = 0.0;
        energy.FTID[IB] = 0.0;
        energy.FTII[IB] = 0.0;
        if IB == 1 {
            energy.FSUN = 0.0;
        }
    }
    // FREVD, FREVI, FREGD, FREGI and RHO, TAU are not reset: when the sun is down they keep
    // whatever the last daylit call left in them, as upstream does.

    // When COSZ is less then zero (ie sun is down), skip the albedo calculations
    if energy.COSZ <= 0. {
        return;
    }

    // weight reflectance/transmittance by LAI and SAI
    for IB in 1..=parameters.NBAND {
        let WL = parameters.ELAI / fi::max(parameters.VAI, parameters.MPE);
        let WS = parameters.ESAI / fi::max(parameters.VAI, parameters.MPE);
        energy.RHO[IB] = fi::max(
            parameters.RHOL[IB] * WL + parameters.RHOS[IB] * WS,
            parameters.MPE,
        );
        energy.TAU[IB] = fi::max(
            parameters.TAUL[IB] * WL + parameters.TAUS[IB] * WS,
            parameters.MPE,
        );
    }

    // Age the snow surface to calculate snow albedo
    snow_age(domain, parameters, energy, water);

    // snow albedos: only if COSZ > 0 and FSNO > 0
    if options.opt_alb == 1 {
        snowalb_bats(parameters, energy);
    }
    if options.opt_alb == 2 {
        snowalb_class(domain, parameters, energy, water);
        energy.ALBOLD = energy.ALB;
    }

    // ground surface albedo
    groundalb(domain, parameters, energy, water);

    // loop over NBAND wavebands to calculate surface albedos and solar
    // fluxes for unit incoming direct (IC=0) and diffuse flux (IC=1)
    //
    // GDIR and EXT are `intent(out)` of every TWOSTREAM call and only the last call's value
    // survives; they depend on neither IB nor IC, so that is the same value every time.
    let mut GDIR = 0.0;
    for IB in 1..=parameters.NBAND {
        // direct shortwave
        twostream(parameters, energy, water, options, IB, 0);
        // diffuse shortwave
        GDIR = twostream(parameters, energy, water, options, IB, 1).0;
    }

    // sunlit fraction of canopy. set FSUN = 0 if FSUN < 0.01.
    let EXT = GDIR / energy.COSZ * fi::sqrt(1. - energy.RHO[1] - energy.TAU[1]);
    energy.FSUN =
        (1. - fi::exp(-EXT * parameters.VAI)) / fi::max(EXT * parameters.VAI, parameters.MPE);
    let EXT = energy.FSUN;

    let WL = if EXT < 0.01 { 0. } else { EXT };
    energy.FSUN = WL;
}

/// `SNOW_AGE` -- see Yang et al. (1997) J. of Climate for detail.
fn snow_age(domain: &Domain, parameters: &Parameters, energy: &mut Energy, water: &Water) {
    // Age the snow surface when snow present
    if water.SNEQV <= 0.0 {
        energy.TAUSS = 0.0;
    } else {
        let DELA0 = domain.dt / parameters.TAU0;
        let ARG = parameters.GRAIN_GROWTH * (1. / parameters.TFRZ - 1. / energy.TG);
        let AGE1 = fi::exp(ARG);
        let AGE2 = fi::exp(fi::min(0., parameters.EXTRA_GROWTH * ARG));
        let AGE3 = parameters.DIRT_SOOT;
        let TAGE = AGE1 + AGE2 + AGE3;
        let DELA = DELA0 * TAGE;
        let DELS = fi::max(0.0, water.SNEQV - water.SNEQVO) / parameters.SWEMX;
        let SGE = (energy.TAUSS + DELA) * (1.0 - DELS);
        energy.TAUSS = fi::max(0., SGE);
    }

    // Compute snow age
    energy.FAGE = energy.TAUSS / (energy.TAUSS + 1.0);
}

/// `SNOWALB_BATS`.
fn snowalb_bats(parameters: &Parameters, energy: &mut Energy) {
    // zero albedos for all points
    for IB in 1..=parameters.NBAND {
        energy.ALBSND[IB] = 0.;
        energy.ALBSNI[IB] = 0.;
    }

    // Compute zenith angle correction
    let SL1 = 1.0 / parameters.BATS_COSZ;
    let SL2 = 2.0 * parameters.BATS_COSZ;
    let CF1 = (1.0 + SL1) / (1.0 + SL2 * energy.COSZ) - SL1;
    let FZEN = fi::max(CF1, 0.0);

    // Compute snow albedo for diffuse solar radiation (1 = vis, 2 = NIR)
    energy.ALBSNI[1] = parameters.BATS_VIS_NEW * (1. - parameters.BATS_VIS_AGE * energy.FAGE);
    energy.ALBSNI[2] = parameters.BATS_NIR_NEW * (1. - parameters.BATS_NIR_AGE * energy.FAGE);

    // Compute snow albedo for direct solar radiation (1 = vis, 2 = NIR)
    energy.ALBSND[1] =
        energy.ALBSNI[1] + parameters.BATS_VIS_DIR * FZEN * (1. - energy.ALBSNI[1]);
    energy.ALBSND[2] =
        energy.ALBSNI[2] + parameters.BATS_NIR_DIR * FZEN * (1. - energy.ALBSNI[2]);
}

/// `SNOWALB_CLASS`.
fn snowalb_class(domain: &Domain, parameters: &Parameters, energy: &mut Energy, water: &Water) {
    // zero albedos for all points
    for IB in 1..=parameters.NBAND {
        energy.ALBSND[IB] = 0.;
        energy.ALBSNI[IB] = 0.;
    }

    // Compute albedo as function of time and albedo at previous time step
    energy.ALB = 0.55 + (energy.ALBOLD - 0.55) * fi::exp(-0.01 * domain.dt / 3600.);

    // Refresh the snow surface albedo when sufficient snow
    if water.QSNOW > 0. {
        energy.ALB += fi::min(water.QSNOW, parameters.SWEMX / domain.dt) * (0.84 - energy.ALB)
            / (parameters.SWEMX / domain.dt);
    }

    // Set all direct & diffuse (vis & nir) albedos to ALB
    energy.ALBSNI[1] = energy.ALB; // vis diffuse
    energy.ALBSNI[2] = energy.ALB; // nir diffuse
    energy.ALBSND[1] = energy.ALB; // vis direct
    energy.ALBSND[2] = energy.ALB; // nir direct
}

/// `GROUNDALB`.
fn groundalb(domain: &Domain, parameters: &Parameters, energy: &mut Energy, water: &Water) {
    for IB in 1..=parameters.NBAND {
        let INC = fi::max(0.11 - 0.40 * water.smc[1], 0.);
        let (ALBSOD, ALBSOI);
        if domain.ist == 1 {
            // soil
            ALBSOD = fi::min(parameters.ALBSAT[IB] + INC, parameters.ALBDRY[IB]);
            ALBSOI = ALBSOD;
        } else if energy.TG > parameters.TFRZ {
            // unfrozen lake, wetland
            ALBSOD = 0.06 / (fi::powf(fi::max(0.01, energy.COSZ), 1.7) + 0.15);
            ALBSOI = 0.06;
        } else {
            // frozen lake, wetland
            ALBSOD = parameters.ALBLAK[IB];
            ALBSOI = ALBSOD;
        }

        // Compute surface as function of bare ground and snow albedo, weighted by FSNO
        energy.ALBGRD[IB] = ALBSOD * (1. - water.FSNO) + energy.ALBSND[IB] * water.FSNO;
        energy.ALBGRI[IB] = ALBSOI * (1. - water.FSNO) + energy.ALBSNI[IB] * water.FSNO;
    }
}

/// `TWOSTREAM` -- returns `(GDIR, EXT)`, the Fortran's two `intent(out)` scalars.
///
/// `IC == 0` is the direct beam, `IC == 1` diffuse.
fn twostream(
    parameters: &Parameters,
    energy: &mut Energy,
    water: &Water,
    options: &Options,
    IB: i32,
    IC: i32,
) -> (f32, f32) {
    // variables for the modified two-stream scheme, Niu and Yang (2004), JGR
    const PAI: f32 = 3.14159265;

    // compute within and between gaps
    let (GAP, KOPEN);
    if parameters.VAI == 0.0 {
        GAP = 1.0;
        KOPEN = 1.0;
    } else {
        // Upstream leaves GAP and KOPEN unset when OPT_RAD is none of 1..3; the namelist
        // check rules that out, and 0.0 stands in for the undefined value here.
        let mut gap = 0.0;
        let mut kopen = 0.0;
        if options.opt_rad == 1 {
            let DENFVEG =
                -fi::log(fi::max(1.0 - parameters.FVEG, 0.01)) / (PAI * fi::powi(parameters.RC, 2));
            let HD = parameters.HVT - parameters.HVB;
            let BB = 0.5 * HD;
            let THETAP = fi::atan(BB / parameters.RC * fi::tan(fi::acos(fi::max(0.01, energy.COSZ))));
            energy.BGAP = fi::exp(-DENFVEG * PAI * fi::powi(parameters.RC, 2) / fi::cos(THETAP));
            let FA = parameters.VAI
                / (1.33 * PAI * fi::powf(parameters.RC, 3.0) * (BB / parameters.RC) * DENFVEG);
            let NEWVAI = HD * FA;
            energy.WGAP = (1.0 - energy.BGAP) * fi::exp(-0.5 * NEWVAI / energy.COSZ);
            gap = fi::min(1.0 - parameters.FVEG, energy.BGAP + energy.WGAP);
            kopen = 0.05;
        }
        if options.opt_rad == 2 {
            gap = 0.0;
            kopen = 0.0;
        }
        if options.opt_rad == 3 {
            gap = 1.0 - parameters.FVEG;
            kopen = 1.0 - parameters.FVEG;
        }
        GAP = gap;
        KOPEN = kopen;
    }

    // calculate two-stream parameters OMEGA, BETAD, BETAI, AVMU, GDIR, EXT.
    // OMEGA, BETAD, BETAI are adjusted for snow. values for OMEGA*BETAD
    // and OMEGA*BETAI are calculated and then divided by the new OMEGA
    // because the product OMEGA*BETAI, OMEGA*BETAD is used in solution.
    // also, the transmittances and reflectances (TAU, RHO) are linear
    // weights of leaf and stem values.
    let COSZI = fi::max(0.001, energy.COSZ);
    let mut CHIL = fi::min(fi::max(parameters.XL, -0.4), 0.6);
    if CHIL.abs() <= 0.01 {
        CHIL = 0.01;
    }
    let PHI1 = 0.5 - 0.633 * CHIL - 0.330 * CHIL * CHIL;
    let PHI2 = 0.877 * (1. - 2. * PHI1);
    let GDIR = PHI1 + PHI2 * COSZI;
    let EXT = GDIR / COSZI;
    let AVMU = (1. - PHI1 / PHI2 * fi::log((PHI1 + PHI2) / PHI1)) / PHI2;
    let OMEGAL = energy.RHO[IB] + energy.TAU[IB];
    let mut TMP0 = GDIR + PHI2 * COSZI;
    let mut TMP1 = PHI1 * COSZI;
    let ASU = 0.5 * OMEGAL * GDIR / TMP0 * (1. - TMP1 / TMP0 * fi::log((TMP1 + TMP0) / TMP1));
    let BETADL = (1. + AVMU * EXT) / (OMEGAL * AVMU * EXT) * ASU;
    let BETAIL = 0.5
        * (energy.RHO[IB] + energy.TAU[IB]
            + (energy.RHO[IB] - energy.TAU[IB]) * fi::powi((1. + CHIL) / 2., 2))
        / OMEGAL;

    // adjust omega, betad, and betai for intercepted snow
    // TO DO: update this logic as TV > TFRZ doesn't necessarily mean the canopy is snow-free
    // KSJ 2021-04-19
    let TMP2;
    if energy.TV > parameters.TFRZ {
        // no snow
        TMP0 = OMEGAL;
        TMP1 = BETADL;
        TMP2 = BETAIL;
    } else {
        TMP0 = (1. - water.FWET) * OMEGAL + water.FWET * parameters.OMEGAS[IB];
        TMP1 = ((1. - water.FWET) * OMEGAL * BETADL
            + water.FWET * parameters.OMEGAS[IB] * parameters.BETADS)
            / TMP0;
        TMP2 = ((1. - water.FWET) * OMEGAL * BETAIL
            + water.FWET * parameters.OMEGAS[IB] * parameters.BETAIS)
            / TMP0;
    }

    let OMEGA = TMP0;
    let BETAD = TMP1;
    let BETAI = TMP2;

    // absorbed, reflected, transmitted fluxes per unit incoming radiation
    let B = 1. - OMEGA + OMEGA * BETAI;
    let C = OMEGA * BETAI;
    let TMP0 = AVMU * EXT;
    let D = TMP0 * OMEGA * BETAD;
    let F = TMP0 * OMEGA * (1. - BETAD);
    let TMP1 = B * B - C * C;
    let H = fi::sqrt(TMP1) / AVMU;
    let mut SIGMA = TMP0 * TMP0 - TMP1;
    if SIGMA.abs() < 1.0e-6 {
        SIGMA = fi::sign(1.0e-6, SIGMA);
    }
    let P1 = B + AVMU * H;
    let P2 = B - AVMU * H;
    let P3 = B + TMP0;
    let P4 = B - TMP0;
    let S1 = fi::exp(-H * parameters.VAI);
    let S2 = fi::exp(-EXT * parameters.VAI);
    let (U1, U2, U3);
    if IC == 0 {
        // direct
        U1 = B - C / energy.ALBGRD[IB];
        U2 = B - C * energy.ALBGRD[IB];
        U3 = F + C * energy.ALBGRD[IB];
    } else {
        // diffuse
        U1 = B - C / energy.ALBGRI[IB];
        U2 = B - C * energy.ALBGRI[IB];
        U3 = F + C * energy.ALBGRI[IB];
    }
    let TMP2 = U1 - AVMU * H;
    let TMP3 = U1 + AVMU * H;
    let D1 = P1 * TMP2 / S1 - P2 * TMP3 * S1;
    let TMP4 = U2 + AVMU * H;
    let TMP5 = U2 - AVMU * H;
    let D2 = TMP4 / S1 - TMP5 * S1;
    let H1 = -D * P4 - C * F;
    let TMP6 = D - H1 * P3 / SIGMA;
    let TMP7 = (D - C - H1 / SIGMA * (U1 + TMP0)) * S2;
    let H2 = (TMP6 * TMP2 / S1 - P2 * TMP7) / D1;
    let H3 = -(TMP6 * TMP3 * S1 - P1 * TMP7) / D1;
    let H4 = -F * P3 - C * D;
    let TMP8 = H4 / SIGMA;
    let TMP9 = (U3 - TMP8 * (U2 - TMP0)) * S2;
    let H5 = -(TMP8 * TMP4 / S1 + TMP9) / D2;
    let H6 = (TMP8 * TMP5 * S1 + TMP9) / D2;
    let H7 = (C * TMP2) / (D1 * S1);
    let H8 = (-C * TMP3 * S1) / D1;
    let H9 = TMP4 / (D2 * S1);
    let H10 = (-TMP5 * S1) / D2;

    // downward direct and diffuse fluxes below vegetation
    // Niu and Yang (2004), JGR.
    if IC == 0 {
        // direct
        let FTDS = S2 * (1.0 - GAP) + GAP;
        let FTIS = (H4 * S2 / SIGMA + H5 * S1 + H6 / S1) * (1.0 - GAP);
        energy.FTDD[IB] = FTDS;
        energy.FTID[IB] = FTIS;
    } else {
        let FTDS = 0.;
        let FTIS = (H9 * S1 + H10 / S1) * (1.0 - KOPEN) + KOPEN;
        energy.FTDI[IB] = FTDS;
        energy.FTII[IB] = FTIS;
    }

    // flux reflected by the surface (veg. and ground)
    if IC == 0 {
        let FRES = (H1 / SIGMA + H2 + H3) * (1.0 - GAP) + energy.ALBGRD[IB] * GAP;
        let FREVEG = (H1 / SIGMA + H2 + H3) * (1.0 - GAP);
        let FREBAR = energy.ALBGRD[IB] * GAP; // jref - separate veg. and ground reflection
        energy.ALBD[IB] = FRES;
        energy.FREVD[IB] = FREVEG;
        energy.FREGD[IB] = FREBAR;
        // Flux absorbed by radiation
        energy.FABD[IB] = 1.
            - energy.ALBD[IB]
            - (1. - energy.ALBGRD[IB]) * energy.FTDD[IB]
            - (1. - energy.ALBGRI[IB]) * energy.FTID[IB];
    } else {
        let FRES = (H7 + H8) * (1.0 - KOPEN) + energy.ALBGRI[IB] * KOPEN;
        let FREVEG = (H7 + H8) * (1.0 - KOPEN) + energy.ALBGRI[IB] * KOPEN;
        let FREBAR = 0.; // jref - separate veg. and ground reflection
        energy.ALBI[IB] = FRES;
        energy.FREVI[IB] = FREVEG;
        energy.FREGI[IB] = FREBAR;
        // Flux absorbed by radiation
        energy.FABI[IB] = 1.
            - energy.ALBI[IB]
            - (1. - energy.ALBGRD[IB]) * energy.FTDI[IB]
            - (1. - energy.ALBGRI[IB]) * energy.FTII[IB];
    }

    (GDIR, EXT)
}
