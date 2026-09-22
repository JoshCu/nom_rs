//! Port of `src/SoilWaterRetentionCoeff.f90` @ 0ff055e.
//!
//! Soil water diffusivity and hydraulic conductivity, in the two forms `OPT_INF` selects
//! between.

#![allow(non_snake_case)]

use crate::fortran::intrinsics as fi;
use crate::parameters::Parameters;

/// `WDFCND1` -- returns `(WDF, WCND)`; frozen soil reduces both through `FCR`.
pub fn wdfcnd1(parameters: &Parameters, SMC: f32, FCR: f32, ISOIL: i32) -> (f32, f32) {
    // soil water diffusivity
    let FACTR = fi::max(0.01, SMC / parameters.smcmax[ISOIL]);
    let mut EXPON = parameters.bexp[ISOIL] + 2.0;
    let mut WDF = parameters.dwsat[ISOIL] * fi::powf(FACTR, EXPON);
    WDF *= 1.0 - FCR;

    // hydraulic conductivity
    EXPON = 2.0 * parameters.bexp[ISOIL] + 3.0;
    let mut WCND = parameters.dksat[ISOIL] * fi::powf(FACTR, EXPON);
    WCND *= 1.0 - FCR;

    (WDF, WCND)
}

/// `WDFCND2` -- returns `(WDF, WCND)`; frozen soil reduces the diffusivity through `SICE`.
pub fn wdfcnd2(parameters: &Parameters, SMC: f32, SICE: f32, ISOIL: i32) -> (f32, f32) {
    // soil water diffusivity
    let mut FACTR1 = 0.05 / parameters.smcmax[ISOIL];
    let FACTR2 = fi::max(0.01, SMC / parameters.smcmax[ISOIL]);
    FACTR1 = fi::min(FACTR1, FACTR2);
    let mut EXPON = parameters.bexp[ISOIL] + 2.0;
    let mut WDF = parameters.dwsat[ISOIL] * fi::powf(FACTR2, EXPON);

    if SICE > 0.0 {
        let VKWGT = 1. / (1. + fi::powf(500. * SICE, 3.));
        WDF = VKWGT * WDF + (1. - VKWGT) * parameters.dwsat[ISOIL] * fi::powf(FACTR1, EXPON);
    }

    // hydraulic conductivity
    EXPON = 2.0 * parameters.bexp[ISOIL] + 3.0;
    let WCND = parameters.dksat[ISOIL] * fi::powf(FACTR2, EXPON);

    (WDF, WCND)
}
