//! Port of `src/ShortwaveRadiationModule.f90` @ 0ff055e.
//!
//! Net shortwave: albedo first, then the absorbed and reflected fluxes per band.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::albedo::albedo;
use crate::water::Water;

/// `ShortwaveRadiationMain`, formerly `RADIATION`.
pub fn shortwave_radiation_main(
    domain: &Domain,
    options: &Options,
    parameters: &Parameters,
    forcing: &Forcing,
    energy: &mut Energy,
    water: &Water,
) {
    // Compute albedo for all surfaces (e.g., snow, bare ground, veg)
    albedo(domain, options, parameters, energy, water);

    // Compute FSHA, LAISUN, and LAISHA terms
    energy.FSHA = 1.0 - energy.FSUN;
    energy.LAISUN = parameters.ELAI * energy.FSUN;
    energy.LAISHA = parameters.ELAI * energy.FSHA;

    // Compute net solar radiation
    net_solar_radiation(parameters, forcing, energy);
}

/// `NetSolarRadiation`, formerly `SURRAD`.
fn net_solar_radiation(parameters: &Parameters, forcing: &Forcing, energy: &mut Energy) {
    // Zero out the solar fluxes
    energy.SAG = 0.;
    energy.SAV = 0.;
    energy.FSA = 0.;

    // direct beam / diffuse radiation absorbed by canopy (w/m2), `DIMENSION(1:2)`
    let mut CAD = [0.0f32; 3];
    let mut CAI = [0.0f32; 3];

    // loop over the vis and nir bands
    for IB in 1..=parameters.NBAND {
        let ib = IB as usize;
        // absorbed by canopy
        CAD[ib] = forcing.SOLAD[IB] * energy.FABD[IB];
        CAI[ib] = forcing.SOLAI[IB] * energy.FABI[IB];
        energy.SAV = energy.SAV + CAD[ib] + CAI[ib];
        energy.FSA = energy.FSA + CAD[ib] + CAI[ib];

        // transmitted solar fluxes incident on ground
        let TRD = forcing.SOLAD[IB] * energy.FTDD[IB];
        let TRI = forcing.SOLAD[IB] * energy.FTID[IB] + forcing.SOLAI[IB] * energy.FTII[IB];

        // solar radiation absorbed by ground surface
        let ABS = TRD * (1. - energy.ALBGRD[IB]) + TRI * (1. - energy.ALBGRI[IB]);
        energy.SAG += ABS;
        energy.FSA += ABS;
    }

    // partition visible canopy absorption to sunlit and shaded fractions
    // to get average absorbed par for sunlit and shaded leaves
    let LAIFRA = parameters.ELAI / fi::max(parameters.VAI, parameters.MPE);
    if energy.FSUN > 0.0 {
        energy.PARSUN =
            (CAD[1] + energy.FSUN * CAI[1]) * LAIFRA / fi::max(energy.LAISUN, parameters.MPE);
        energy.PARSHA = (energy.FSHA * CAI[1]) * LAIFRA / fi::max(energy.LAISHA, parameters.MPE);
    } else {
        energy.PARSUN = 0.;
        energy.PARSHA = (CAD[1] + CAI[1]) * LAIFRA / fi::max(energy.LAISHA, parameters.MPE);
    }

    // reflected solar radiation
    let RVIS = energy.ALBD[1] * forcing.SOLAD[1] + energy.ALBI[1] * forcing.SOLAI[1];
    let RNIR = energy.ALBD[2] * forcing.SOLAD[2] + energy.ALBI[2] * forcing.SOLAI[2];
    energy.FSR = RVIS + RNIR;

    // reflected solar radiation of veg. and ground (combined ground)
    energy.FSRV = energy.FREVD[1] * forcing.SOLAD[1]
        + energy.FREVI[1] * forcing.SOLAI[1]
        + energy.FREVD[2] * forcing.SOLAD[2]
        + energy.FREVI[2] * forcing.SOLAI[2];
    energy.FSRG = energy.FREGD[1] * forcing.SOLAD[1]
        + energy.FREGI[1] * forcing.SOLAI[1]
        + energy.FREGD[2] * forcing.SOLAD[2]
        + energy.FREGI[2] * forcing.SOLAI[2];
}
