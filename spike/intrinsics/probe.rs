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
    fn powf(x: f32, y: f32) -> f32;
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
