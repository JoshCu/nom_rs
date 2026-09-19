//! The Rust spellings of Fortran's floating-point intrinsics that gfortran matches bit for bit.
//!
//! Ported physics must call through here rather than reaching for `f32::exp` or `x * x * x * x`
//! directly. The spellings are not interchangeable: several of the obvious ones are wrong, and
//! wrong by more than a rounding hair. `EtFluxModule` runs up to `NITERC = 20` surface
//! temperature iterations, so a 1-ULP difference in `exp` does not stay a 1-ULP difference.
//!
//! # Where these came from
//!
//! `spike/intrinsics/run.sh` puts every plausible Rust spelling of each operation against
//! gfortran over 20k inputs per intrinsic, comparing raw bit patterns. Verdict as run on
//! 2026-09-18, GNU Fortran 15.2.0 (Ubuntu 15.2.0-16ubuntu1) / rustc 1.98.1, `-O2
//! -ffp-contract=off`: **every intrinsic the model uses has a bit-identical Rust candidate**,
//! at both `-O` and `-O0`. Strict bit-identity is achievable; see `PORTING.md`.
//!
//! # The rules, and why the obvious spellings lose
//!
//! **Transcendentals: call `f32`'s own method.** gfortran emits `expf`/`logf`/`sinf` and so on,
//! and so does Rust on glibc targets. What loses is widening: `(x as f64).exp() as f32` differs
//! on ~0.07% of inputs for `exp` and ~1.4% for `sin`. Computing in double and narrowing is
//! *more* accurate and therefore wrong. `sqrt` and `tanh` happen to agree either way.
//!
//! **Integer exponents: always [`powi`], never repeated multiplication.** gfortran expands
//! `x ** n` by squaring, which for `n >= 4` is not left-to-right multiplication:
//! `x * x * x * x` differs from `x ** 4` on **34%** of inputs by up to 2 ULP. `x ** 7` (not
//! used by the model, but the sharpest case) differs on 58% by up to 4 ULP. `powi` matched on
//! every exponent probed -- 0, 1, 2, 3, 4, 7, -1, and the same values supplied at runtime,
//! which is a libcall on both sides rather than an inline expansion.
//!
//! **Real exponents: [`powf`].** `exp(y * log x)` is off by up to 12 ULP.
//!
//! **Never `powf` for an integer exponent.** It is a different function, off by 1 ULP on 26% of
//! inputs for `x ** 3`. It looks exact at `-O` only because LLVM rewrites `powf(x, 2.0)` into
//! `x * x`; unoptimised -- which is how `cargo test` runs -- the real call disagrees with
//! gfortran. `run.sh` checks both optimisation levels for exactly this reason.
//!
//! The vectors in this module's tests are gfortran's own answers, so a future rustc that
//! changes how `llvm.powi` expands will fail CI here rather than silently in the physics.

/// Fortran `EXP(x)`.
#[inline]
pub fn exp(x: f32) -> f32 {
    x.exp()
}

/// Fortran `LOG(x)`.
#[inline]
pub fn log(x: f32) -> f32 {
    x.ln()
}

/// Fortran `SQRT(x)`.
#[inline]
pub fn sqrt(x: f32) -> f32 {
    x.sqrt()
}

/// Fortran `SIN(x)`.
#[inline]
pub fn sin(x: f32) -> f32 {
    x.sin()
}

/// Fortran `COS(x)`.
#[inline]
pub fn cos(x: f32) -> f32 {
    x.cos()
}

/// Fortran `TANH(x)`.
#[inline]
pub fn tanh(x: f32) -> f32 {
    x.tanh()
}

/// Fortran `LOG10(x)`.
#[inline]
pub fn log10(x: f32) -> f32 {
    x.log10()
}

/// Fortran `TAN(x)`.
#[inline]
pub fn tan(x: f32) -> f32 {
    x.tan()
}

/// Fortran `ASIN(x)`.
#[inline]
pub fn asin(x: f32) -> f32 {
    x.asin()
}

/// Fortran `ACOS(x)`.
#[inline]
pub fn acos(x: f32) -> f32 {
    x.acos()
}

/// Fortran `ATAN(x)`.
#[inline]
pub fn atan(x: f32) -> f32 {
    x.atan()
}

/// Fortran `SIGN(a, b)` -- the magnitude of `a` with the sign of `b`.
///
/// `copysign`, not `if b >= 0.0`. Reading the standard suggests the latter, since `-0.0 >= 0`
/// is true and the standard says "`|a|` when `b >= 0`" -- but gfortran returns `-|a|` for
/// `b = -0.0`, following IEEE. The spike probes `-0.0` explicitly for this reason.
#[inline]
pub fn sign(a: f32, b: f32) -> f32 {
    a.abs().copysign(b)
}

/// Fortran `MOD(a, p)` for reals -- the truncated remainder.
///
/// Rust's `%` is the same operation. `rem_euclid` is Fortran's `MODULO`, and disagrees on
/// every negative argument.
#[inline]
pub fn modulo_trunc(a: f32, p: f32) -> f32 {
    a % p
}

/// Fortran `INT(x)` -- truncation toward zero.
#[inline]
pub fn int(x: f32) -> i32 {
    x as i32
}

/// Fortran `NINT(x)` -- round half away from zero.
///
/// Not `round_ties_even`, which differs on every exact `.5`.
#[inline]
pub fn nint(x: f32) -> i32 {
    x.round() as i32
}

/// Fortran `x ** n` where `n` is an integer -- literal or variable, positive or negative.
///
/// Do not hand-expand this into multiplications. See the module docs: for `n >= 4` the
/// left-to-right expansion is not what gfortran computes.
#[inline]
pub fn powi(x: f32, n: i32) -> f32 {
    x.powi(n)
}

/// Fortran `x ** y` where `y` is real.
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    x.powf(y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(argument, gfortran's answer)`, as bit patterns. Produced by
    /// `spike/intrinsics/probe.f90` under GNU Fortran 15.2.0, `-O2 -ffp-contract=off`.
    ///
    /// Each set is a spread across the intrinsic's realistic argument range plus every
    /// argument at which a losing candidate diverged -- those are the discriminating ones, so
    /// a regression to the wrong spelling fails here rather than passing by luck.
    type Vectors = &'static [(u32, u32)];

    fn check(name: &str, vectors: Vectors, f: impl Fn(f32) -> f32) {
        for &(xb, want) in vectors {
            // Opaque, so that a release build makes the actual libm call instead of folding
            // the whole expression at compile time -- see `constant_folding_is_not_a_hazard`.
            let x = f32::from_bits(std::hint::black_box(xb));
            let got = f(x).to_bits();
            assert_eq!(
                got,
                want,
                "{name}({x:e} / {xb:08X}): gfortran {want:08X}, got {got:08X}",
            );
        }
    }

    const EXP: Vectors = &[
        (0xC1F0_0000, 0x29D2_B706),
        (0xC1A0_0106, 0x310D_9215),
        (0xC120_0419, 0x383E_3B0F),
        (0xBAC4_9E2A, 0x3F7F_9DC4),
        (0x411F_F7CF, 0x46AB_BCEC),
        (0x419F_FAE1, 0x4DE6_C45C),
        (0xC1EC_9782, 0x2A21_5189),
    ];

    const LOG: Vectors = &[
        (0x322B_CC77, 0xC193_5D8E),
        (0x369B_B2B4, 0xC144_7DFF),
        (0x3B0D_1B3A, 0xC0C4_81C5),
        (0x3F7F_C3AA, 0xBA71_7475),
        (0x43E7_CB6B, 0x40C4_72AE),
        (0x4852_123F, 0x4144_7673),
        (0x3D91_80D1, 0xC029_3E31),
    ];

    const SQRT: Vectors = &[
        (0x0000_0000, 0x0000_0000),
        (0x4822_C095, 0x43CC_1E79),
        (0x48A2_C095, 0x4410_557E),
        (0x48F4_20E0, 0x4430_C5B4),
        (0x4922_C095, 0x444C_1E79),
        (0x494B_70BB, 0x4464_3648),
    ];

    const SIN: Vectors = &[
        (0xC120_0000, 0x3F0B_44F8),
        (0xC0D5_56B3, 0xBEBF_A504),
        (0xC055_5ACC, 0x3E43_7A0F),
        (0xBA03_141C, 0xBA03_141C),
        (0x4055_4A69, 0xBE42_78AB),
        (0x40D5_4E82, 0x3EBF_2B75),
        (0xC11F_D2F1, 0x3F08_E5E7),
    ];

    const COS: Vectors = &[
        (0xC120_0000, 0xBF56_CD64),
        (0xC0D5_56B3, 0x3F6D_63E1),
        (0xC055_5ACC, 0xBF7B_4AD2),
        (0xBA03_141C, 0x3F7F_FFFE),
        (0x4055_4A69, 0xBF7B_574D),
        (0x40D5_4E82, 0x3F6D_7C61),
        (0xC10A_0C02, 0xBF32_EF67),
    ];

    const TANH: Vectors = &[
        (0xC120_0000, 0xBF80_0000),
        (0xC0D5_56B3, 0xBF7F_FFCA),
        (0xC055_5ACC, 0xBF7F_5984),
        (0xBA03_141C, 0xBA03_141B),
        (0x4055_4A69, 0x3F7F_592F),
        (0x40D5_4E82, 0x3F7F_FFCA),
    ];

    const POW0: Vectors = &[
        (0x3A83_126F, 0x3F80_0000),
        (0xBC23_BA13, 0x3F80_0000),
        (0x3DCC_CCCD, 0x3F80_0000),
        (0xBF7F_D2BE, 0x3F80_0000),
        (0x4120_0000, 0x3F80_0000),
        (0xC2C7_DCA4, 0x3F80_0000),
    ];

    const POW1: Vectors = &[
        (0x3A83_126F, 0x3A83_126F),
        (0xBC23_BA13, 0xBC23_BA13),
        (0x3DCC_CCCD, 0x3DCC_CCCD),
        (0xBF7F_D2BE, 0xBF7F_D2BE),
        (0x4120_0000, 0x4120_0000),
        (0xC2C7_DCA4, 0xC2C7_DCA4),
    ];

    const POW2: Vectors = &[
        (0x3A83_126F, 0x3586_37BE),
        (0xBC23_BA13, 0x38D1_6CF7),
        (0x3DCC_CCCD, 0x3C23_D70B),
        (0xBF7F_D2BE, 0x3F7F_A584),
        (0x4120_0000, 0x42C8_0000),
        (0xC2C7_DCA4, 0x461C_08C5),
        (0x3F5C_AA3F, 0x3F3E_350D),
    ];

    const POW3: Vectors = &[
        (0x3A83_126F, 0x3089_7061),
        (0xBC23_BA13, 0xB585_F09A),
        (0x3DCC_CCCD, 0x3A83_126F),
        (0xBF7F_D2BE, 0xBF7F_7852),
        (0x4120_0000, 0x447A_0000),
        (0xC2C7_DCA4, 0xC973_A299),
        (0x3A83_6F49, 0x308A_9545),
    ];

    const POW4: Vectors = &[
        (0x3A83_126F, 0x2B8C_BCCE),
        (0xBC23_BA13, 0x322B_531A),
        (0x3DCC_CCCD, 0x38D1_B719),
        (0xBF7F_D2BE, 0x3F7F_4B28),
        (0x4120_0000, 0x461C_4000),
        (0xC2C7_DCA4, 0x4CBE_3561),
        (0x3AD1_AAC8, 0x2CE6_5F74),
        (0x3A8E_0219, 0x2BC1_EBA7),
    ];

    /// Not used by the model. Kept because it is where the spellings diverge most, so it is the
    /// test most likely to notice a change in how `powi` expands.
    const POW7: Vectors = &[
        (0x3DCC_CCCD, 0x33D6_BF97),
        (0xBE5C_9035, 0xB7B4_6F9B),
        (0x3EED_A63C, 0x3B98_187A),
        (0xBF7F_F0E9, 0xBF7F_9673),
        (0x4009_E242, 0x4357_7186),
        (0xC094_7F24, 0xC735_0522),
        (0x3DD0_3955, 0x33F1_2D31),
        (0x3E0B_BCD7, 0x356C_8E67),
        (0x3EBD_8D23, 0x3A79_E232),
    ];

    const POWM1: Vectors = &[
        (0x3A83_126F, 0x4479_FFFF),
        (0xBC23_BA13, 0xC2C8_2362),
        (0x3DCC_CCCD, 0x4120_0000),
        (0xBF7F_D2BE, 0xBF80_16A5),
        (0x4120_0000, 0x3DCC_CCCD),
        (0xC2C7_DCA4, 0xBC23_F407),
        (0x3C6E_BF01, 0x4289_401A),
    ];

    /// `LOG10(x)`.
    const LOG10: Vectors = &[
        (0x322B_CC77, 0xC100_0000),
        (0x369B_B2B4, 0xC0AA_ABC2),
        (0x3B0D_1B3A, 0xC02A_AF09),
        (0x3F7F_C3AA, 0xB9D1_B99E),
        (0x43E7_CB6B, 0x402A_A1EE),
        (0x4852_123F, 0x40AA_A534),
        (0x32A2_C7F0, 0xC0F7_1DCA),
    ];
    /// `TAN(x)`.
    const TAN: Vectors = &[
        (0xC120_0000, 0xBF25_FAFA),
        (0xC0D5_56B3, 0xBECE_AB15),
        (0xC055_5ACC, 0xBE47_238E),
        (0xBA03_141C, 0xBA03_141D),
        (0x4055_4A69, 0x3E46_1381),
        (0x40D5_4E82, 0x3ECE_12BA),
    ];
    /// `ASIN(x)`.
    const ASIN: Vectors = &[
        (0xBF80_0000, 0xBFC9_0FDB),
        (0xBF2A_ABC2, 0xBF3A_D0E5),
        (0xBEAA_AF09, 0xBEAE_03BD),
        (0xB851_B9C7, 0xB851_B9C7),
        (0x3EAA_A1EE, 0x3EAD_F5D6),
        (0x3F2A_A534, 0x3F3A_C81A),
    ];
    /// `ACOS(x)`.
    const ACOS: Vectors = &[
        (0xBF80_0000, 0x4049_0FDB),
        (0xBF2A_ABC2, 0x4013_3C27),
        (0xBEAA_AF09, 0x3FF4_90CA),
        (0xB851_B9C7, 0x3FC9_117E),
        (0x3EAA_A1EE, 0x3F9D_9265),
        (0x3F2A_A534, 0x3F57_579B),
    ];
    /// `ATAN(x)`.
    const ATAN: Vectors = &[
        (0xC2C8_0000, 0xBFC7_C82F),
        (0xC285_5630, 0xBFC7_2462),
        (0xC205_58BF, 0xBFC5_3935),
        (0xBBA3_D923, 0xBBA3_D8CA),
        (0x4205_4E82, 0x3FC5_38EA),
        (0x4285_5111, 0x3FC7_244F),
    ];

    /// `SIGN(1.0, x)`. The last entry is `-0.0`, where `b >= 0.0` gives the wrong sign.
    const SIGN1: Vectors = &[
        (0x0000_0000, 0x3F80_0000),
        (0xC0D5_6715, 0xBF80_0000),
        (0xC055_7B91, 0xBF80_0000),
        (0xBB23_D923, 0xBF80_0000),
        (0x4055_29A4, 0x3F80_0000),
        (0x40D5_3E1F, 0x3F80_0000),
        (0x8000_0000, 0xBF80_0000),
    ];

    /// `NINT(x)` and `INT(x)`, as the integer's own bits.
    const NINT: Vectors = &[
        (0xC37A_0000, 0xFFFF_FF06),
        (0xC326_C000, 0xFFFF_FF59),
        (0xC2A7_0000, 0xFFFF_FFAC),
        (0xBE80_0000, 0x0000_0000),
        (0x42A6_0000, 0x0000_0053),
        (0x4326_4000, 0x0000_00A6),
        (0xBF00_0000, 0xFFFF_FFFF),
    ];

    const INT: Vectors = &[
        (0xC47A_0000, 0xFFFF_FC18),
        (0xC426_ABBC, 0xFFFF_FD66),
        (0xC3A6_AEEF, 0xFFFF_FEB3),
        (0xBD4C_CF6C, 0x0000_0000),
        (0x43A6_A222, 0x0000_014D),
        (0x4426_A555, 0x0000_029A),
        (0xBF73_3650, 0x0000_0000),
    ];

    /// `MOD(x, 24.0)`.
    const MOD24: Vectors = &[
        (0xC2C8_0000, 0xC080_0000),
        (0xC285_5630, 0xC195_58C0),
        (0xC205_58BF, 0xC115_62FC),
        (0xBBA3_D923, 0xBBA3_D923),
        (0x4205_4E82, 0x4115_3A08),
        (0x4285_5111, 0x4195_4444),
        (0xC2AF_FFB1, 0xC17F_FD88),
    ];

    /// `x ** 0.6666667`.
    const POWR: Vectors = &[
        (0x3586_37BD, 0x38D1_B714),
        (0x38D1_AABB, 0x3B0D_2BDC),
        (0x3C23_C3BA, 0x3D3E_0FA4),
        (0x3F7F_D2BE, 0x3F7F_E1D3),
        (0x42C7_D0DD, 0x41AC_3FBE),
        (0x461C_11F9, 0x43E7_E6C3),
        (0x367E_382F, 0x397E_CFC1),
        (0x35AB_2A43, 0x38F6_9F74),
    ];

    #[test]
    fn transcendentals_match_gfortran() {
        check("exp", EXP, exp);
        check("log", LOG, log);
        check("sqrt", SQRT, sqrt);
        check("sin", SIN, sin);
        check("cos", COS, cos);
        check("tanh", TANH, tanh);
        check("log10", LOG10, log10);
        check("tan", TAN, tan);
        check("asin", ASIN, asin);
        check("acos", ACOS, acos);
        check("atan", ATAN, atan);
    }

    /// Sign, rounding and remainder: not libm, but the Fortran semantics differ from Rust's
    /// obvious spelling in each case.
    #[test]
    fn conversions_match_gfortran() {
        check("sign(1.0, x)", SIGN1, |x| sign(1.0, x));
        check("mod(x, 24.0)", MOD24, |x| modulo_trunc(x, 24.0));

        // These two produce integers; the vectors hold the integer's own bits, so the
        // comparison is not confused by the sign of zero.
        for &(xb, want) in NINT {
            let x = f32::from_bits(std::hint::black_box(xb));
            assert_eq!(nint(x) as u32, want, "nint({x})");
        }
        for &(xb, want) in INT {
            let x = f32::from_bits(std::hint::black_box(xb));
            assert_eq!(int(x) as u32, want, "int({x})");
        }
    }

    #[test]
    fn integer_powers_match_gfortran() {
        for (n, vectors) in [
            (0, POW0),
            (1, POW1),
            (2, POW2),
            (3, POW3),
            (4, POW4),
            (7, POW7),
            (-1, POWM1),
        ] {
            check(&format!("x ** {n}"), vectors, |x| powi(x, n));
        }
    }

    /// `x ** ifrc` -- a runtime exponent is a libcall on both sides, not an inline expansion,
    /// so it is a separate code path from the literal case and gets its own check.
    #[test]
    fn runtime_integer_powers_match_gfortran() {
        for (n, vectors) in [(2, POW2), (3, POW3), (4, POW4)] {
            let n = std::hint::black_box(n);
            check(&format!("x ** n={n}"), vectors, |x| powi(x, n));
        }
    }

    #[test]
    fn real_power_matches_gfortran() {
        check("x ** 0.6666667", POWR, |x| powf(x, 0.6666667));
    }

    /// Both compilers evaluate a compile-time-constant argument at compile time, in higher
    /// precision than the libm they would otherwise call -- so `exp(-29.5739784)` folds to
    /// `2A215188` while glibc's `expf` returns `2A215189` for the same argument.
    ///
    /// This is a trap for the *tests*, not for the port: gfortran folds to the same value LLVM
    /// does (verified with `gfortran -O2` against a literal argument), so ported code agrees
    /// with the Fortran whichever regime it lands in. What must not happen is comparing a
    /// folded Rust value against a Fortran value that was computed at runtime, which is what
    /// the probe produces -- hence the `black_box` in `check`.
    #[test]
    fn constant_folding_is_not_a_hazard() {
        // Folded: rustc evaluates this at compile time in release.
        let folded = (-29.5739784f32).exp().to_bits();
        // The same argument, opaque, so it reaches glibc.
        let called = std::hint::black_box(-29.5739784f32).exp().to_bits();
        if cfg!(debug_assertions) {
            assert_eq!(folded, called, "debug does not fold");
        } else {
            assert_eq!(folded, 0x2A21_5188, "LLVM's fold, which is gfortran's fold");
            assert_eq!(called, 0x2A21_5189, "glibc expf, which is gfortran at runtime");
        }
    }

    /// The spellings a port would reach for by instinct, pinned as *wrong* so that the module
    /// docs cannot rot into folklore. If one of these ever starts agreeing with gfortran, the
    /// toolchain changed and the whole verdict wants re-running.
    #[test]
    fn the_tempting_spellings_are_wrong() {
        let disagrees = |vectors: Vectors, f: &dyn Fn(f32) -> f32| {
            vectors
                .iter()
                .any(|&(xb, want)| f(f32::from_bits(xb)).to_bits() != want)
        };

        // Left-to-right multiplication is not exponentiation by squaring.
        assert!(disagrees(POW4, &|x| x * x * x * x), "x*x*x*x vs x**4");
        // powf is a different function from an integer power.
        assert!(disagrees(POW3, &|x: f32| x.powf(3.0)), "powf(x,3) vs x**3");
        // Widening to f64 is more accurate, and therefore not what gfortran computed.
        assert!(disagrees(EXP, &|x| (x as f64).exp() as f32), "f64 exp");
        assert!(disagrees(SIN, &|x| (x as f64).sin() as f32), "f64 sin");
        // exp(y * log x) is not powf.
        assert!(
            disagrees(POWR, &|x| (0.6666667f32 * x.ln()).exp()),
            "exp(y*log x) vs x**y"
        );
        // `SIGN(1.0, -0.0)` is -1.0, so the standard-looking `b >= 0.0` is wrong.
        assert!(
            disagrees(SIGN1, &|x| if x >= 0.0 { 1.0 } else { -1.0 }),
            "b >= 0.0 vs SIGN"
        );
        // Fortran MOD is truncated, not Euclidean.
        assert!(disagrees(MOD24, &|x| x.rem_euclid(24.0)), "rem_euclid vs MOD");
        // NINT rounds half away from zero, not to even.
        assert!(
            NINT.iter()
                .any(|&(xb, want)| f32::from_bits(xb).round_ties_even() as i32 as u32 != want),
            "round_ties_even vs NINT"
        );
    }
}
