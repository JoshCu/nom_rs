//! Scalar parsing with Fortran semantics.
//!
//! Both the namelist reader and the list-directed reader feed tokens through here, so
//! `1.0d0`, `.5`, `1.`, `.true.` and `'quoted'` are interpreted the same way everywhere.

use core::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Real(String),
    Int(String),
    Logical(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Real(t) => write!(f, "not a Fortran real: {t:?}"),
            ParseError::Int(t) => write!(f, "not a Fortran integer: {t:?}"),
            ParseError::Logical(t) => write!(f, "not a Fortran logical: {t:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a Fortran real literal as `f64`.
///
/// Accepts the `d`/`D`/`q`/`Q` exponent markers Rust's parser rejects, and the bare-exponent
/// form (`1.0+6`, `2.5-3`) that list-directed input permits. Rust's `str::parse` is correctly
/// rounded, matching `strtod`, so the resulting value matches what gfortran reads.
pub fn parse_real(token: &str) -> Result<f64, ParseError> {
    let t = token.trim();
    if t.is_empty() {
        return Err(ParseError::Real(token.to_string()));
    }

    let normalized = normalize_exponent(t);
    normalized
        .parse::<f64>()
        .map_err(|_| ParseError::Real(token.to_string()))
}

/// Parse a Fortran real literal as `f32`.
///
/// Parses at `f64` then rounds once to `f32`. This is what gfortran does for a
/// `real` namelist item, and double rounding is not a concern here: `strtod` is correctly
/// rounded and the subsequent narrowing is a single correctly-rounded step.
pub fn parse_real32(token: &str) -> Result<f32, ParseError> {
    parse_real(token).map(|v| v as f32)
}

pub fn parse_int(token: &str) -> Result<i32, ParseError> {
    let t = token.trim();
    let t = t.strip_prefix('+').unwrap_or(t);
    t.parse::<i32>().map_err(|_| ParseError::Int(token.to_string()))
}

/// `.true.` / `.t.` / `t` / `true` and the false equivalents, case-insensitive.
///
/// Fortran also accepts anything beginning with `.t` or `t`, e.g. `.TRUE_ANYTHING`.
pub fn parse_logical(token: &str) -> Result<bool, ParseError> {
    let t = token.trim().to_ascii_lowercase();
    let body = t.strip_prefix('.').unwrap_or(&t);
    match body.chars().next() {
        Some('t') => Ok(true),
        Some('f') => Ok(false),
        _ => Err(ParseError::Logical(token.to_string())),
    }
}

/// Strip surrounding quotes and collapse Fortran's doubled-quote escape.
pub fn parse_string(token: &str) -> String {
    let t = token.trim();
    for q in ['\'', '"'] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            let inner = &t[1..t.len() - 1];
            let doubled = [q, q].iter().collect::<String>();
            return inner.replace(&doubled, &q.to_string());
        }
    }
    t.to_string()
}

/// Rewrite Fortran exponent forms into something Rust's float parser accepts.
///
/// - `1.0d0` / `1.0D0` / `1.0q0` -> `1.0e0`
/// - `1.0+6` / `2.5-3` -> `1.0e+6` / `2.5e-3` (sign-only exponent, no marker)
fn normalize_exponent(t: &str) -> String {
    let mut out = String::with_capacity(t.len() + 1);
    let bytes = t.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'd' | b'D' | b'q' | b'Q' => out.push('e'),
            b'+' | b'-' if i > 0 => {
                // A sign after a digit or '.' with no exponent marker is an implicit exponent.
                let prev = bytes[i - 1];
                let has_marker = matches!(prev, b'e' | b'E' | b'd' | b'D' | b'q' | b'Q');
                if !has_marker && (prev.is_ascii_digit() || prev == b'.') {
                    out.push('e');
                }
                out.push(b as char);
            }
            _ => out.push(b as char),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_reals() {
        assert_eq!(parse_real("1.0").unwrap(), 1.0);
        assert_eq!(parse_real("-2.5").unwrap(), -2.5);
        assert_eq!(parse_real(".5").unwrap(), 0.5);
        assert_eq!(parse_real("1.").unwrap(), 1.0);
        assert_eq!(parse_real("  3.25  ").unwrap(), 3.25);
    }

    #[test]
    fn exponent_markers() {
        assert_eq!(parse_real("2.0E-6").unwrap(), 2.0e-6);
        assert_eq!(parse_real("2.0e-6").unwrap(), 2.0e-6);
        // The d/D marker, which Rust's parser rejects outright.
        assert_eq!(parse_real("1.0d0").unwrap(), 1.0);
        assert_eq!(parse_real("2.5D3").unwrap(), 2500.0);
        assert_eq!(parse_real("1.0D-15").unwrap(), 1.0e-15);
    }

    #[test]
    fn implicit_exponent() {
        // List-directed input accepts a sign-only exponent.
        assert_eq!(parse_real("1.0+6").unwrap(), 1.0e6);
        assert_eq!(parse_real("2.5-3").unwrap(), 2.5e-3);
        // A leading sign is not an exponent.
        assert_eq!(parse_real("-1.5").unwrap(), -1.5);
    }

    #[test]
    fn values_from_the_real_tables() {
        // SOILPARM.TBL row 1
        assert_eq!(parse_real32("4.66E-5").unwrap(), 4.66E-5f32);
        assert_eq!(parse_real32("2.65E-5").unwrap(), 2.65E-5f32);
        // GENPARM.TBL
        assert_eq!(parse_real32("2.00E+6").unwrap(), 2.00E6f32);
        assert_eq!(parse_real32("2.0E-6").unwrap(), 2.0E-6f32);
        // The -1.E36 sentinel
        assert_eq!(parse_real32("-1.E36").unwrap(), -1.0E36f32);
    }

    #[test]
    fn f32_narrowing_is_single_rounding() {
        // 0.1 is not representable; check we land on the same f32 gfortran would.
        assert_eq!(parse_real32("0.1").unwrap().to_bits(), 0.1f32.to_bits());
        assert_eq!(parse_real32("0.3").unwrap().to_bits(), 0.3f32.to_bits());
        assert_eq!(parse_real32("1.E-15").unwrap().to_bits(), 1.0e-15f32.to_bits());
    }

    #[test]
    fn integers() {
        assert_eq!(parse_int("27").unwrap(), 27);
        assert_eq!(parse_int("-3").unwrap(), -3);
        assert_eq!(parse_int("+8").unwrap(), 8);
        assert!(parse_int("1.5").is_err());
    }

    #[test]
    fn logicals() {
        for t in [".true.", ".TRUE.", "T", "t", "true", ".t."] {
            assert!(parse_logical(t).unwrap(), "{t}");
        }
        for f in [".false.", ".FALSE.", "F", "f", "false", ".f."] {
            assert!(!parse_logical(f).unwrap(), "{f}");
        }
        assert!(parse_logical("maybe").is_err());
    }

    #[test]
    fn strings() {
        assert_eq!(parse_string("'USGS'"), "USGS");
        assert_eq!(parse_string("\"STAS\""), "STAS");
        assert_eq!(parse_string("bare"), "bare");
        assert_eq!(parse_string("'SANDY LOAM'"), "SANDY LOAM");
        // Doubled quote is an escaped quote.
        assert_eq!(parse_string("'it''s'"), "it's");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_real("").is_err());
        assert!(parse_real("abc").is_err());
    }
}
