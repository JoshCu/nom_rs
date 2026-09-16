//! Port of `src/ConstantsModule.f90` @ eaa8282.
//!
//! Derived constants keep their defining expressions (`CP = 7.*R_D/2.`) rather than a
//! pre-computed literal: Rust evaluates `const` float arithmetic at compile time under the same
//! IEEE rules gfortran folds with, so the expression is both exact and reviewable against the
//! Fortran.
//!
//! Upstream has no `implicit none` here and carries a number of constants the BMI path never
//! reads (WRF/NMM-core leftovers). They are ported anyway so a future physics module can use
//! one without a detour.

#![allow(dead_code)]

// See the crate-level note in lib.rs: literals are copied verbatim from the Fortran.
#![allow(clippy::excessive_precision)]
// The Fortran writes 3.1415926 for DEGRAD rather than calling on a pi constant. Substituting
// core::f32::consts::PI would be a different literal (one ULP away before division).
#![allow(clippy::approx_constant)]

// Bounds on real numbers.
/// A really small number.
pub const EPSILON: f32 = 1.0E-15;
pub const TINY: f64 = 0.000_000_000_000_000_000_000_000_000_000_01;

// Physical constants.
/// acceleration due to gravity (m s^-2)
pub const G: f32 = 9.81;
pub const R_D: f32 = 287.0;
pub const CP: f32 = 7.0 * R_D / 2.0;
pub const R_V: f32 = 461.6;
pub const CV: f32 = CP - R_D;
pub const CPV: f32 = 4.0 * R_V;
pub const CVV: f32 = CPV - R_V;
pub const CVPM: f32 = -CV / CP;
pub const CLIQ: f32 = 4190.0;
pub const CICE: f32 = 2106.0;
pub const PSAT: f32 = 610.78;
pub const RCV: f32 = R_D / CV;
pub const RCP: f32 = R_D / CP;
pub const ROVG: f32 = R_D / G;
pub const C2: f32 = CP * RCV;
/// molecular weight of dry air (g/mole)
pub const MWDRY: f32 = 28.966;

pub const P1000MB: f32 = 100000.0;
pub const T0: f32 = 300.0;
pub const P0: f32 = P1000MB;
pub const CPOVCV: f32 = CP / (CP - R_D);
pub const CVOVCP: f32 = 1.0 / CPOVCV;
pub const RVOVRD: f32 = R_V / R_D;

pub const RERADIUS: f32 = 1.0 / 6370.0e03;

pub const ASSELIN: f32 = 0.025;
pub const CB: f32 = 25.0;

pub const XLV0: f32 = 3.15E6;
pub const XLV1: f32 = 2370.0;
pub const XLS0: f32 = 2.905E6;
pub const XLS1: f32 = 259.532;

pub const XLS: f32 = 2.85E6;
pub const XLV: f32 = 2.5E6;
pub const XLF: f32 = 3.50E5;

pub const RHOWATER: f32 = 1000.0;
pub const RHOSNOW: f32 = 100.0;
pub const RHOAIR0: f32 = 1.28;

pub const N_CCN0: f32 = 1.0E8;

pub const DEGRAD: f32 = 3.1415926 / 180.0;
pub const DPD: f32 = 360.0 / 365.0;

pub const SVP1: f32 = 0.6112;
pub const SVP2: f32 = 17.67;
pub const SVP3: f32 = 29.65;
pub const SVPT0: f32 = 273.15;
pub const EP_1: f32 = R_V / R_D - 1.0;
pub const EP_2: f32 = R_D / R_V;
pub const KARMAN: f32 = 0.4;
pub const EOMEG: f32 = 7.2921E-5;
pub const STBOLT: f32 = 5.67051E-8;

pub const PRANDTL: f32 = 1.0 / 3.0;

// Constants for the w-damping option.
/// strength m/s/s
pub const W_ALPHA: f32 = 0.3;
/// activation cfl number
pub const W_BETA: f32 = 1.0;

pub const PQ0: f32 = 379.90516;
pub const EPSQ2: f32 = 0.2;
pub const A2: f32 = 17.2693882;
pub const A3: f32 = 273.16;
pub const A4: f32 = 35.86;
pub const EPSQ: f32 = 1.0e-12;
pub const P608: f32 = RVOVRD - 1.0;

pub const CLIMIT: f32 = 1.0e-20;
pub const CM1: f32 = 2937.4;
pub const CM2: f32 = 4.9283;
pub const CM3: f32 = 23.5518;
pub const DEFC: f32 = 0.0;
pub const DEFM: f32 = 99999.0;
pub const EPSFC: f32 = 1.0 / 1.05;
pub const EPSWET: f32 = 0.0;
pub const FCDIF: f32 = 1.0 / 3.0;
pub const FCM: f32 = 0.00003;
pub const GMA: f32 = -R_D * (1.0 - RCP) * 0.5;
pub const P400: f32 = 40000.0;
pub const PHITP: f32 = 15000.0;
pub const PI: f64 = 3.141_592_653_589_793_2;
pub const PI2: f32 = 2.0 * 3.1415926;
pub const PLBTM: f32 = 105000.0;
pub const PLOMD: f32 = 64200.0;
pub const PMDHI: f32 = 35000.0;
pub const Q2INI: f32 = 0.50;
pub const RFCP: f32 = 0.25 / CP;
pub const RHCRIT_LAND: f32 = 0.75;
pub const RHCRIT_SEA: f32 = 0.80;
pub const RLAG: f32 = 14.8125;
pub const RLX: f32 = 0.90;
pub const SCQ2: f32 = 50.0;
pub const SLOPHT: f32 = 0.001;
pub const TLC: f32 = 2.0 * 0.703972477;
pub const WA: f32 = 0.15;
pub const WGHT: f32 = 0.35;
pub const WPC: f32 = 0.075;
pub const Z0LAND: f32 = 0.10;
pub const Z0MAX: f32 = 0.008;
pub const Z0SEA: f32 = 0.001;

// Orbital constants.
pub const PLANET_YEAR: i32 = 365;
pub const OBLIQUITY: f32 = 23.5;
pub const ECCENTRICITY: f32 = 0.014;
/// In AU
pub const SEMIMAJORAXIS: f32 = 1.0;
/// Time of perihelion passage
pub const ZERO_DATE: f32 = 0.0;
/// Fraction into the year (from perihelion) of the Northern Spring Equinox
pub const EQUINOX_FRACTION: f32 = 0.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_constants_fold_as_the_fortran_does() {
        // CP = 7.*R_D/2. -- evaluated left to right, not as 3.5*R_D.
        assert_eq!(CP.to_bits(), (7.0f32 * 287.0f32 / 2.0f32).to_bits());
        assert_eq!(CP, 1004.5);

        assert_eq!(CV.to_bits(), (CP - R_D).to_bits());
        assert_eq!(RCP.to_bits(), (R_D / CP).to_bits());
        assert_eq!(CVPM.to_bits(), (-CV / CP).to_bits());
        assert_eq!(GMA.to_bits(), (-R_D * (1.0f32 - RCP) * 0.5f32).to_bits());
    }

    #[test]
    fn degrad_uses_the_literal_the_fortran_writes() {
        assert_eq!(DEGRAD.to_bits(), (3.1415926f32 / 180.0f32).to_bits());

        // The literal 3.1415926 is one ULP below f32 pi...
        assert_ne!(3.1415926f32.to_bits(), core::f32::consts::PI.to_bits());
        // ...but the two collapse once divided by 180, so DEGRAD itself is not sensitive to
        // the choice. Keep the literal regardless: any other use of it would be.
        assert_eq!(
            DEGRAD.to_bits(),
            (core::f32::consts::PI / 180.0f32).to_bits()
        );
    }

    #[test]
    fn tlc_matches_the_literal_exactly() {
        // Regression: writing this as 0.7039725 lands one ULP away from the Fortran's
        // 0.703972477. Float literals get copied verbatim, never rounded for readability.
        assert_eq!(TLC.to_bits(), (2.0f32 * 0.703972477f32).to_bits());
        assert_ne!(TLC.to_bits(), (2.0f32 * 0.703_972_5f32).to_bits());
    }

    #[test]
    fn pi_is_double_precision() {
        assert_eq!(PI, core::f64::consts::PI);
    }

    #[test]
    fn spot_check_physical_values() {
        assert_eq!(G, 9.81);
        assert_eq!(RHOWATER, 1000.0);
        assert_eq!(STBOLT, 5.67051E-8);
        assert_eq!(KARMAN, 0.4);
        assert_eq!(SVPT0, 273.15);
    }
}
