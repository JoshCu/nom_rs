//! Port of `src/SnowLayerChange.f90` @ 0ff055e.
//!
//! The snow layer bookkeeping: `COMBINE` merges layers that have thinned or lost their ice,
//! `DIVIDE` splits layers that have grown thick, `COMBO` merges two layers' mass and enthalpy,
//! and `COMPACT` settles the pack.
//!
//! The snow layers occupy `water.ISNOW + 1 ..= 0`, so every loop here runs over a range that
//! the loop itself changes. The Fortran `DO` loops fix their trip count on entry and then read
//! `ISNOW` afresh inside the body; the port does the same, which is why several loops take a
//! copy of `ISNOW` for their bounds and keep reading `water.ISNOW` in the body.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::fortran::intrinsics as fi;
use crate::parameters::Parameters;
use crate::water::Water;

/// `COMBINE`.
pub fn combine(
    domain: &mut Domain,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
) {
    // DATA DZMIN /0.045, 0.05, 0.2/
    const DZMIN: [f32; 3] = [0.025, 0.025, 0.1]; // MB: change limit

    let dz = &mut domain.dzsnso;
    let mut ISNOW_OLD = water.ISNOW;
    for J in ISNOW_OLD + 1..=0 {
        if water.SNICE[J] <= 0.1 {
            if J != 0 {
                water.SNLIQ[J + 1] += water.SNLIQ[J];
                water.SNICE[J + 1] += water.SNICE[J];
                dz[J + 1] += dz[J];
            } else if ISNOW_OLD < -1 {
                // MB/KM: change to ISNOW
                water.SNLIQ[J - 1] += water.SNLIQ[J];
                water.SNICE[J - 1] += water.SNICE[J];
                dz[J - 1] += dz[J];
            } else {
                if water.SNICE[J] >= 0. {
                    water.PONDING1 = water.SNLIQ[J]; // ISNOW WILL GET SET TO ZERO BELOW; PONDING1 WILL GET
                    water.SNEQV = water.SNICE[J]; // ADDED TO PONDING FROM PHASECHANGE PONDING SHOULD BE
                    water.SNOWH = dz[J]; // ZERO HERE BECAUSE IT WAS CALCULATED FOR THIN SNOW
                } else {
                    // SNICE OVER-SUBLIMATED EARLIER
                    water.PONDING1 = water.SNLIQ[J] + water.SNICE[J];
                    if water.PONDING1 < 0. {
                        // IF SNICE AND SNLIQ SUBLIMATES REMOVE FROM SOIL
                        water.sice[1] =
                            fi::max(0.0, water.sice[1] + water.PONDING1 / (dz[1] * 1000.));
                        water.PONDING1 = 0.0;
                    }
                    water.SNEQV = 0.0;
                    water.SNOWH = 0.0;
                }
                water.SNLIQ[J] = 0.0;
                water.SNICE[J] = 0.0;
                dz[J] = 0.0;
            }

            // shift all elements above this down by one.
            if J > water.ISNOW + 1 && water.ISNOW < -1 {
                for I in (water.ISNOW + 2..=J).rev() {
                    energy.STC[I] = energy.STC[I - 1];
                    water.SNLIQ[I] = water.SNLIQ[I - 1];
                    water.SNICE[I] = water.SNICE[I - 1];
                    dz[I] = dz[I - 1];
                }
            }
            water.ISNOW += 1;
        }
    }

    // to conserve water in case of too large surface sublimation
    if water.sice[1] < 0. {
        water.sh2o[1] += water.sice[1];
        water.sice[1] = 0.;
    }

    if water.ISNOW == 0 {
        return; // MB: get out if no longer multi-layer
    }
    water.SNEQV = 0.;
    water.SNOWH = 0.;
    let mut ZWICE = 0.0f32;
    let mut ZWLIQ = 0.0f32;
    for J in water.ISNOW + 1..=0 {
        water.SNEQV = water.SNEQV + water.SNICE[J] + water.SNLIQ[J];
        water.SNOWH += dz[J];
        ZWICE += water.SNICE[J];
        ZWLIQ += water.SNLIQ[J];
    }

    // check the snow depth - all snow gone
    // the liquid water assumes ponding on soil surface.
    if water.SNOWH < 0.025 && water.ISNOW < 0 {
        // MB: change limit
        water.ISNOW = 0;
        water.SNEQV = ZWICE;
        water.PONDING2 = ZWLIQ; // LIMIT OF ISNOW < 0 MEANS INPUT PONDING
        if water.SNEQV <= 0. {
            water.SNOWH = 0.; // SHOULD BE ZERO; SEE ABOVE
        }
    }

    // check the snow depth - snow layers combined
    if water.ISNOW < -1 {
        ISNOW_OLD = water.ISNOW;
        let mut MSSI = 1usize;
        for I in ISNOW_OLD + 1..=0 {
            if dz[I] < DZMIN[MSSI - 1] {
                let NEIBOR = if I == water.ISNOW + 1 {
                    I + 1
                } else if I == 0 {
                    I - 1
                } else {
                    let mut n = I + 1;
                    if (dz[I - 1] + dz[I]) < (dz[I + 1] + dz[I]) {
                        n = I - 1;
                    }
                    n
                };
                // Node l and j are combined and stored as node j.
                let (J, L) = if NEIBOR > I { (NEIBOR, I) } else { (I, NEIBOR) };
                (dz[J], water.SNLIQ[J], water.SNICE[J], energy.STC[J]) = combo(
                    parameters,
                    dz[J],
                    water.SNLIQ[J],
                    water.SNICE[J],
                    energy.STC[J],
                    dz[L],
                    water.SNLIQ[L],
                    water.SNICE[L],
                    energy.STC[L],
                );
                // Now shift all elements above this down one.
                if J - 1 > water.ISNOW + 1 {
                    for K in (water.ISNOW + 2..=J - 1).rev() {
                        energy.STC[K] = energy.STC[K - 1];
                        water.SNICE[K] = water.SNICE[K - 1];
                        water.SNLIQ[K] = water.SNLIQ[K - 1];
                        dz[K] = dz[K - 1];
                    }
                }
                // Decrease the number of snow layers
                water.ISNOW += 1;
                if water.ISNOW >= -1 {
                    break;
                }
            } else {
                // The layer thickness is greater than the prescribed minimum value
                MSSI += 1;
            }
        }
    }
}

/// `DIVIDE`.
pub fn divide(
    domain: &mut Domain,
    nsnow: i32,
    parameters: &Parameters,
    energy: &mut Energy,
    water: &mut Water,
) {
    // The Fortran's locals are `DIMENSION(1:levels%nsnow)` and uninitialised past `|ISNOW|`;
    // nothing below reads an element before writing it.
    let n = nsnow as usize;
    let mut DZ = vec![0.0f32; n + 1];
    let mut SWICE = vec![0.0f32; n + 1];
    let mut SWLIQ = vec![0.0f32; n + 1];
    let mut TSNO = vec![0.0f32; n + 1];

    for J in 1..=nsnow {
        if J <= water.ISNOW.abs() {
            let j = J as usize;
            DZ[j] = domain.dzsnso[J + water.ISNOW];
            SWICE[j] = water.SNICE[J + water.ISNOW];
            SWLIQ[j] = water.SNLIQ[J + water.ISNOW];
            TSNO[j] = energy.STC[J + water.ISNOW];
        }
    }

    let mut MSNO = water.ISNOW.abs();
    if MSNO == 1 {
        // Specify a new snow layer
        if DZ[1] > 0.05 {
            MSNO = 2;
            DZ[1] /= 2.;
            SWICE[1] /= 2.;
            SWLIQ[1] /= 2.;
            DZ[2] = DZ[1];
            SWICE[2] = SWICE[1];
            SWLIQ[2] = SWLIQ[1];
            TSNO[2] = TSNO[1];
        }
    }
    if MSNO > 1 && DZ[1] > 0.05 {
        let DRR = DZ[1] - 0.05;
        let mut PROPOR = DRR / DZ[1];
        let ZWICE = PROPOR * SWICE[1];
        let ZWLIQ = PROPOR * SWLIQ[1];
        PROPOR = 0.05 / DZ[1];
        SWICE[1] *= PROPOR;
        SWLIQ[1] *= PROPOR;
        DZ[1] = 0.05;
        (DZ[2], SWLIQ[2], SWICE[2], TSNO[2]) = combo(
            parameters, DZ[2], SWLIQ[2], SWICE[2], TSNO[2], DRR, ZWLIQ, ZWICE, TSNO[1],
        );
        // subdivide a new layer
        if MSNO <= 2 && DZ[2] > 0.20 {
            // MB: change limit
            MSNO = 3;
            let DTDZ = (TSNO[1] - TSNO[2]) / ((DZ[1] + DZ[2]) / 2.);
            DZ[2] /= 2.;
            SWICE[2] /= 2.;
            SWLIQ[2] /= 2.;
            DZ[3] = DZ[2];
            SWICE[3] = SWICE[2];
            SWLIQ[3] = SWLIQ[2];
            TSNO[3] = TSNO[2] - DTDZ * DZ[2] / 2.;
            if TSNO[3] >= parameters.TFRZ {
                TSNO[3] = TSNO[2];
            } else {
                TSNO[2] += DTDZ * DZ[2] / 2.;
            }
        }
    }

    if MSNO > 2 && DZ[2] > 0.2 {
        let DRR = DZ[2] - 0.2;
        let mut PROPOR = DRR / DZ[2];
        let ZWICE = PROPOR * SWICE[2];
        let ZWLIQ = PROPOR * SWLIQ[2];
        PROPOR = 0.2 / DZ[2];
        SWICE[2] *= PROPOR;
        SWLIQ[2] *= PROPOR;
        DZ[2] = 0.2;
        (DZ[3], SWLIQ[3], SWICE[3], TSNO[3]) = combo(
            parameters, DZ[3], SWLIQ[3], SWICE[3], TSNO[3], DRR, ZWLIQ, ZWICE, TSNO[2],
        );
    }
    water.ISNOW = -MSNO;

    for J in water.ISNOW + 1..=0 {
        let j = (J - water.ISNOW) as usize;
        domain.dzsnso[J] = DZ[j];
        water.SNICE[J] = SWICE[j];
        water.SNLIQ[J] = SWLIQ[j];
        energy.STC[J] = TSNO[j];
    }
}

/// `COMBO` -- combine two elements, returning the combined `(DZ, WLIQ, WICE, T)`.
///
/// The Fortran updates its first four arguments in place; returning them lets `COMBINE` pass
/// two elements of the same arrays without aliasing.
#[allow(clippy::too_many_arguments)]
pub fn combo(
    parameters: &Parameters,
    DZ: f32,
    WLIQ: f32,
    WICE: f32,
    T: f32,
    DZ2: f32,
    WLIQ2: f32,
    WICE2: f32,
    T2: f32,
) -> (f32, f32, f32, f32) {
    let p = parameters;
    let DZC = DZ + DZ2;
    let WICEC = WICE + WICE2;
    let WLIQC = WLIQ + WLIQ2;
    let H = (p.CICE * WICE + p.CWAT * WLIQ) * (T - p.TFRZ) + p.HFUS * WLIQ;
    let H2 = (p.CICE * WICE2 + p.CWAT * WLIQ2) * (T2 - p.TFRZ) + p.HFUS * WLIQ2;
    let HC = H + H2;
    let TC = if HC < 0. {
        p.TFRZ + HC / (p.CICE * WICEC + p.CWAT * WLIQC)
    } else if HC <= p.HFUS * WLIQC {
        p.TFRZ
    } else {
        p.TFRZ + (HC - p.HFUS * WLIQC) / (p.CICE * WICEC + p.CWAT * WLIQC)
    };
    (DZC, WLIQC, WICEC, TC)
}

/// `COMPACT`.
pub fn compact(domain: &mut Domain, parameters: &Parameters, energy: &Energy, water: &mut Water) {
    const C2: f32 = 21.0e-3; // [m3/kg] ! default 21.0e-3
    const C3: f32 = 2.5e-6; // [1/s]
    const C4: f32 = 0.04; // [1/k]
    const C5: f32 = 2.0;
    const DM: f32 = 100.0; // upper Limit on destructive metamorphism compaction [kg/m3]
    const ETA0: f32 = 0.8e+6; // viscosity coefficient [kg-s/m2]
                              // according to Anderson, it is between 0.52e6~1.38e6

    let DT = domain.dt;
    let dz = &mut domain.dzsnso;
    let mut BURDEN = 0.0f32;
    for J in water.ISNOW + 1..=0 {
        let WX = water.SNICE[J] + water.SNLIQ[J];
        water.FICE[J] = water.SNICE[J] / WX;
        let VOID =
            1. - (water.SNICE[J] / parameters.DENICE + water.SNLIQ[J] / parameters.DENH2O) / dz[J];
        // Allow compaction only for non-saturated node and higher ice lens node.
        if VOID > 0.001 && water.SNICE[J] > 0.1 {
            let BI = water.SNICE[J] / dz[J];
            let TD = fi::max(0., parameters.TFRZ - energy.STC[J]);
            let DEXPF = fi::exp(-C4 * TD);

            // Settling as a result of destructive metamorphism
            let mut DDZ1 = -C3 * DEXPF;
            if BI > DM {
                DDZ1 *= fi::exp(-46.0E-3 * (BI - DM));
            }
            // Liquid water term
            if water.SNLIQ[J] > 0.01 * dz[J] {
                DDZ1 *= C5;
            }

            // Compaction due to overburden
            let DDZ2 = -(BURDEN + 0.5 * WX) * fi::exp(-0.08 * TD - C2 * BI) / ETA0; // 0.5*WX -> self-burden

            // Compaction occurring during melt
            let DDZ3 = if energy.IMELT[J] == 1 {
                let d = fi::max(
                    0.,
                    (water.FICEOLD[J] - water.FICE[J]) / fi::max(1.0E-6, water.FICEOLD[J]),
                );
                -d / DT // sometimes too large
            } else {
                0.
            };

            // Time rate of fractional change in DZ (units of s-1)
            let mut PDZDTC = (DDZ1 + DDZ2 + DDZ3) * DT;
            PDZDTC = fi::max(-0.5, PDZDTC);
            // The change in DZ due to compaction
            dz[J] *= 1. + PDZDTC;
            dz[J] = fi::max(
                dz[J],
                water.SNICE[J] / parameters.DENICE + water.SNLIQ[J] / parameters.DENH2O,
            );
        }
        // Pressure of overlying snow
        BURDEN += WX;
    }
}
