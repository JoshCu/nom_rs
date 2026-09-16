//! Port of `src/ErrorCheckModule.f90` @ eaa8282.
//!
//! `sys_abort` has no Rust counterpart: callers return a typed error instead of stopping the
//! process, which a calibration driver needs.

/// `is_within_bound`. Generic here, where the Fortran overloads on int and real.
pub fn is_within_bound<T: PartialOrd>(var: T, lower_bound: T, upper_bound: T) -> bool {
    !(var < lower_bound || var > upper_bound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_on_both_ends() {
        assert!(is_within_bound(1, 1, 8));
        assert!(is_within_bound(8, 1, 8));
        assert!(is_within_bound(4, 1, 8));
    }

    #[test]
    fn rejects_outside() {
        assert!(!is_within_bound(0, 1, 8));
        assert!(!is_within_bound(9, 1, 8));
    }

    #[test]
    fn works_for_reals_too() {
        assert!(is_within_bound(0.5f32, 0.0, 1.0));
        assert!(!is_within_bound(1.5f32, 0.0, 1.0));
    }
}
