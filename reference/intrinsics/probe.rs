//! Phase 0 intrinsic-drift probe -- Rust side.
//!
//! Reads f32 bit patterns as 8 hex digits, one per line, on stdin. Applies every *candidate*
//! implementation of the intrinsic named by argv[1] and writes
//! `<input hex> <candidate> <output hex>` per line.
//!
//! Emitting several candidates is the point: the question is not "does Rust match gfortran"
//! but "which spelling of this operation matches gfortran". The comparison script then reports,
//! per intrinsic, which candidate agrees bit for bit.
//!
//! Build:  rustc -O -o probe_rs probe.rs

use std::io::{self, BufRead, Write};

extern "C" {
    fn expf(x: f32) -> f32;
    fn logf(x: f32) -> f32;
    fn sqrtf(x: f32) -> f32;
    fn sinf(x: f32) -> f32;
    fn cosf(x: f32) -> f32;
    fn tanhf(x: f32) -> f32;
    fn tanf(x: f32) -> f32;
    fn asinf(x: f32) -> f32;
    fn acosf(x: f32) -> f32;
    fn atanf(x: f32) -> f32;
    fn log10f(x: f32) -> f32;
    fn powf(x: f32, y: f32) -> f32;
}

/// Reinterpret an integer result as an f32 so it can travel through the same channel as the
/// floating-point candidates. Only the bits are ever compared.
fn int_bits(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The exponent gfortran's `x ** 0.6666667` is given.
const REAL_EXP: f32 = 0.6666667;

/// Candidate implementations per intrinsic.
///
/// - `std`   -- what a port would write by default (`x.exp()`).
/// - `libm`  -- the C symbol gfortran itself calls, via FFI.
/// - `f64`   -- widen, compute in double, narrow. Sometimes the only way to match.
/// - `mul`   -- repeated multiplication, for integer exponents.
/// - `powi`  -- `f32::powi`, which uses a different reduction order than gfortran's expansion.
///
/// `n` is the runtime exponent for the `pown*` cases and is ignored by every other intrinsic.
fn candidates(name: &str, x: f32, n: i32) -> Vec<(&'static str, f32)> {
    match name {
        "exp" => vec![
            ("std", x.exp()),
            ("libm", unsafe { expf(x) }),
            ("f64", (x as f64).exp() as f32),
        ],
        "log" => vec![
            ("std", x.ln()),
            ("libm", unsafe { logf(x) }),
            ("f64", (x as f64).ln() as f32),
        ],
        "sqrt" => vec![
            ("std", x.sqrt()),
            ("libm", unsafe { sqrtf(x) }),
            ("f64", (x as f64).sqrt() as f32),
        ],
        "sin" => vec![
            ("std", x.sin()),
            ("libm", unsafe { sinf(x) }),
            ("f64", (x as f64).sin() as f32),
        ],
        "cos" => vec![
            ("std", x.cos()),
            ("libm", unsafe { cosf(x) }),
            ("f64", (x as f64).cos() as f32),
        ],
        "tanh" => vec![
            ("std", x.tanh()),
            ("libm", unsafe { tanhf(x) }),
            ("f64", (x as f64).tanh() as f32),
        ],
        // x**0 and x**1 are trivial, but cheap to pin: a wrong answer here would be a
        // silent identity error rather than a ULP, and Fortran defines 0.0**0 as 1.0.
        "tan" => vec![
            ("std", x.tan()),
            ("libm", unsafe { tanf(x) }),
            ("f64", (x as f64).tan() as f32),
        ],
        "asin" => vec![
            ("std", x.asin()),
            ("libm", unsafe { asinf(x) }),
            ("f64", (x as f64).asin() as f32),
        ],
        "acos" => vec![
            ("std", x.acos()),
            ("libm", unsafe { acosf(x) }),
            ("f64", (x as f64).acos() as f32),
        ],
        "atan" => vec![
            ("std", x.atan()),
            ("libm", unsafe { atanf(x) }),
            ("f64", (x as f64).atan() as f32),
        ],
        "log10" => vec![
            ("std", x.log10()),
            ("libm", unsafe { log10f(x) }),
            ("f64", (x as f64).log10() as f32),
            // The identity a compiler is tempted to apply.
            ("ln_ratio", x.ln() / std::f32::consts::LN_10),
        ],
        // SIGN(1.0, x): the Fortran standard says |a| when b >= 0, and -0.0 >= 0 is true --
        // so SIGN(1.0, -0.0) is +1.0 where copysign gives -1.0.
        "sign1" => vec![
            ("copysign", 1.0f32.copysign(x)),
            ("ge_zero", if x >= 0.0 { 1.0 } else { -1.0 }),
        ],
        // NINT and INT yield integers; the candidates emit the integer's own bits so the
        // comparison is not confused by the sign of zero. See probe.f90.
        "nint" => vec![
            ("round", int_bits(x.round() as i32)),
            ("round_ties_even", int_bits(x.round_ties_even() as i32)),
        ],
        "int" => vec![
            ("trunc", int_bits(x as i32)),
            ("floor", int_bits(x.floor() as i32)),
        ],
        // Fortran MOD is the truncated remainder; MODULO is the Euclidean one.
        "mod24" => vec![
            ("rem", x % 24.0),
            ("rem_euclid", x.rem_euclid(24.0)),
        ],
        // `f32::min` is IEEE minNum: it returns the non-NaN operand, and prefers -0.0. The
        // alternatives are what a hand translation of Fortran's MIN would produce, and they
        // differ from it on exactly the NaN and signed-zero inputs the sweep forces in.
        "min24" => vec![
            ("min", x.min(24.0)),
            ("lt", if x < 24.0 { x } else { 24.0 }),
            ("le", if x <= 24.0 { x } else { 24.0 }),
        ],
        "max24" => vec![
            ("max", x.max(24.0)),
            ("gt", if x > 24.0 { x } else { 24.0 }),
            ("ge", if x >= 24.0 { x } else { 24.0 }),
        ],
        "min0" => vec![
            ("min", x.min(0.0)),
            ("lt", if x < 0.0 { x } else { 0.0 }),
            ("le", if x <= 0.0 { x } else { 0.0 }),
        ],
        "max0" => vec![
            ("max", x.max(0.0)),
            ("gt", if x > 0.0 { x } else { 0.0 }),
            ("ge", if x >= 0.0 { x } else { 0.0 }),
        ],
        "pow0" => vec![
            ("one", 1.0),
            ("powi", x.powi(0)),
            ("powf", x.powf(0.0)),
        ],
        "pow1" => vec![
            ("ident", x),
            ("powi", x.powi(1)),
            ("powf", x.powf(1.0)),
        ],
        "pow2" => vec![
            ("mul", x * x),
            ("powi", x.powi(2)),
            ("powf", x.powf(2.0)),
            ("libm", unsafe { powf(x, 2.0) }),
        ],
        "pow3" => vec![
            ("mul", x * x * x),
            ("mul_sq", (x * x) * x),
            ("powi", x.powi(3)),
            ("powf", x.powf(3.0)),
        ],
        "pow4" => vec![
            ("mul", x * x * x * x),
            // Squaring the square -- what exponentiation by squaring produces.
            ("sq_sq", {
                let x2 = x * x;
                x2 * x2
            }),
            ("powi", x.powi(4)),
            ("powf", x.powf(4.0)),
        ],
        "powm1" => vec![
            ("recip", 1.0 / x),
            ("powi", x.powi(-1)),
            ("powf", x.powf(-1.0)),
        ],
        // Runtime exponent: `n` comes from argv, so neither compiler can fold these into an
        // inline expansion. `powi` here lowers to a libcall, not to multiplications.
        "pown2" | "pown3" | "pown4" => vec![
            ("powi", x.powi(n)),
            ("powf", x.powf(n as f32)),
            ("loop", {
                let mut acc = 1.0f32;
                for _ in 0..n {
                    acc *= x;
                }
                acc
            }),
        ],
        "pow7" => vec![
            // Left-to-right, as a naive expansion would.
            ("mul", x * x * x * x * x * x * x),
            // Binary exponentiation: x^7 = x^4 * x^2 * x.
            ("binary", {
                let x2 = x * x;
                let x4 = x2 * x2;
                x4 * x2 * x
            }),
            ("powi", x.powi(7)),
            ("powf", x.powf(7.0)),
        ],
        // Real *literal* exponents, as the model spells them. gfortran does not necessarily
        // call powf for these: a whole or half number exponent gets rewritten into
        // multiplications and square roots, which is a different computation. Each case
        // offers powf alongside the rewrite a compiler would apply, so the report says which
        // one gfortran actually chose rather than assuming.
        "powr2" => vec![
            ("powf", x.powf(2.0)),
            ("mul", x * x),
            ("powi", x.powi(2)),
        ],
        "powr3" => vec![
            ("powf", x.powf(3.0)),
            ("mul", x * x * x),
            ("powi", x.powi(3)),
        ],
        "powr4" => vec![
            ("powf", x.powf(4.0)),
            ("sq_sq", {
                let x2 = x * x;
                x2 * x2
            }),
            ("powi", x.powi(4)),
        ],
        // `x ** 0.5` is the one exponent where LLVM rewrites `powf` on its own: at -O it turns
        // `x.powf(0.5)` into a square root, which is not what gfortran's powf returns. Hiding
        // the exponent behind `black_box` makes it opaque to that transform, so the call
        // survives optimisation -- the `_opaque` candidate exists to prove it does.
        "powrh" => vec![
            ("powf", x.powf(0.5)),
            ("sqrt", x.sqrt()),
            ("powf_opaque", x.powf(std::hint::black_box(0.5))),
            ("libm_opaque", unsafe { powf(x, std::hint::black_box(0.5)) }),
        ],
        "powrq" => vec![
            ("powf", x.powf(0.25)),
            ("sqrt2", x.sqrt().sqrt()),
        ],
        "powrmq" => vec![
            ("powf", x.powf(-0.25)),
            ("recip", 1.0 / x.sqrt().sqrt()),
            ("rsqrt2", (1.0f32 / x).sqrt().sqrt()),
        ],
        "powrmh" => vec![
            ("powf", x.powf(-1.0 / 2.0)),
            ("recip", 1.0 / x.sqrt()),
            ("rsqrt", (1.0f32 / x).sqrt()),
        ],
        "powr15" => vec![
            ("powf", x.powf(1.5)),
            ("x_sqrt", x * x.sqrt()),
            ("sqrt_cube", (x * x * x).sqrt()),
        ],
        "powr17" => vec![
            ("powf", x.powf(1.7)),
            ("libm", unsafe { powf(x, 1.7) }),
        ],
        "powr667" => vec![
            ("powf", x.powf(0.667)),
            ("libm", unsafe { powf(x, 0.667) }),
        ],
        // The Fortran writes the exponent as `(2./3.)`; both compilers fold it to the same
        // f32, so the Rust side may write the division out the same way.
        "powr23" => vec![
            ("powf", x.powf(2.0 / 3.0)),
            ("libm", unsafe { powf(x, 2.0 / 3.0) }),
        ],
        "powr" => vec![
            ("std", x.powf(REAL_EXP)),
            ("libm", unsafe { powf(x, REAL_EXP) }),
            ("f64", (x as f64).powf(REAL_EXP as f64) as f32),
            ("exp_log", unsafe { expf(REAL_EXP * logf(x)) }),
        ],
        other => {
            eprintln!("unknown function: {other}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: probe_rs <intrinsic>");
        std::process::exit(2);
    });

    // Runtime exponent for the pown* cases, parsed from argv so it is opaque to the
    // optimiser -- the same condition the Fortran side is held to.
    let n: i32 = name
        .strip_prefix("pown")
        .and_then(|d| d.parse().ok())
        .unwrap_or(0);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line.expect("read stdin");
        let hex = line.trim();
        if hex.is_empty() {
            continue;
        }
        let bits = u32::from_str_radix(hex, 16).unwrap_or_else(|_| {
            eprintln!("not 8 hex digits: {hex:?}");
            std::process::exit(1);
        });
        let x = f32::from_bits(bits);

        for (label, y) in candidates(&name, x, n) {
            // A closed stdout (piping into `head`) is not an error worth a panic.
            if writeln!(out, "{:08X} {} {:08X}", bits, label, y.to_bits()).is_err() {
                return;
            }
        }
    }
}
