//! Fortran-style arrays with a non-unit lower bound.
//!
//! Noah-OWP declares its snow/soil profile arrays as `(-nsnow+1 : nsoil)` and indexes them
//! directly throughout the physics (`DO IZ = ISNOW+1, NSOIL`). Rather than translate every
//! index expression -- which is where porting bugs hide -- ported code indexes `Shifted<T>`
//! with the same `i32` indices the Fortran uses.
//!
//! Soil-only arrays (`1:nsoil`) use the same type with `lo = 1`, so no ported loop ever has to
//! reason about which convention an array follows.

use core::ops::{Index, IndexMut};

/// A Fortran array with lower bound `lo`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shifted<T> {
    lo: i32,
    data: Vec<T>,
}

impl<T: Clone> Shifted<T> {
    /// Array spanning `lo..=hi` inclusive, every element `fill`.
    ///
    /// Panics if `hi < lo - 1`. `hi == lo - 1` yields an empty array, which Fortran permits.
    pub fn new(lo: i32, hi: i32, fill: T) -> Self {
        let len = (hi - lo + 1)
            .try_into()
            .unwrap_or_else(|_| panic!("Shifted::new: invalid bounds {lo}..={hi}"));
        Self { lo, data: vec![fill; len] }
    }

    /// `1:n`, the Fortran default.
    pub fn ones(n: i32, fill: T) -> Self {
        Self::new(1, n, fill)
    }

    /// `-nsnow+1 : nsoil`, the snow/soil profile shape.
    pub fn snow_soil(nsnow: i32, nsoil: i32, fill: T) -> Self {
        Self::new(-nsnow + 1, nsoil, fill)
    }

    /// Set every element, mirroring Fortran's `arr(:) = x`.
    pub fn fill(&mut self, value: T) {
        self.data.fill(value);
    }
}

impl<T> Shifted<T> {
    pub fn lo(&self) -> i32 {
        self.lo
    }

    pub fn hi(&self) -> i32 {
        self.lo + self.data.len() as i32 - 1
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Storage order, low index first -- the layout `get_value` must hand to BMI.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    pub fn get(&self, i: i32) -> Option<&T> {
        usize::try_from(i - self.lo).ok().and_then(|k| self.data.get(k))
    }

    pub fn get_mut(&mut self, i: i32) -> Option<&mut T> {
        usize::try_from(i - self.lo).ok().and_then(|k| self.data.get_mut(k))
    }

    #[inline]
    fn offset(&self, i: i32) -> usize {
        match usize::try_from(i - self.lo) {
            Ok(k) if k < self.data.len() => k,
            _ => panic!(
                "Shifted index {i} out of bounds for array ({}..={})",
                self.lo,
                self.hi()
            ),
        }
    }
}

impl<T> Index<i32> for Shifted<T> {
    type Output = T;

    #[inline]
    fn index(&self, i: i32) -> &T {
        &self.data[self.offset(i)]
    }
}

impl<T> IndexMut<i32> for Shifted<T> {
    #[inline]
    fn index_mut(&mut self, i: i32) -> &mut T {
        let k = self.offset(i);
        &mut self.data[k]
    }
}

/// Build from values already in storage order (low index first).
impl<T> Shifted<T> {
    pub fn from_vec(lo: i32, data: Vec<T>) -> Self {
        Self { lo, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_soil_bounds_match_fortran() {
        // domain%zsnso(-nsnow+1 : nsoil) with nsnow = 3, nsoil = 4 => (-2..=4), 7 elements.
        let a = Shifted::snow_soil(3, 4, 0.0f32);
        assert_eq!(a.lo(), -2);
        assert_eq!(a.hi(), 4);
        assert_eq!(a.len(), 7);
    }

    #[test]
    fn indexes_by_fortran_index() {
        let mut a = Shifted::snow_soil(3, 4, 0.0f32);
        a[-2] = 1.0;
        a[0] = 2.0;
        a[4] = 3.0;
        assert_eq!(a[-2], 1.0);
        assert_eq!(a[0], 2.0);
        assert_eq!(a[4], 3.0);
        // Storage order is low-index-first.
        assert_eq!(a.as_slice(), &[1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0]);
    }

    #[test]
    fn soil_arrays_are_one_based() {
        let a = Shifted::ones(4, 0.3f32);
        assert_eq!(a.lo(), 1);
        assert_eq!(a.hi(), 4);
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn typical_physics_loop() {
        // DO IZ = ISNOW+1, NSOIL
        let (nsnow, nsoil, isnow) = (3, 4, -2);
        let mut stc = Shifted::snow_soil(nsnow, nsoil, 0.0f32);
        for iz in (isnow + 1)..=nsoil {
            stc[iz] = iz as f32;
        }
        assert_eq!(stc[-1], -1.0);
        assert_eq!(stc[4], 4.0);
        // Layers above ISNOW+1 stay untouched, as in the Fortran.
        assert_eq!(stc[-2], 0.0);
    }

    #[test]
    fn fill_mirrors_whole_array_assignment() {
        let mut a = Shifted::snow_soil(3, 4, 0.0f32);
        a.fill(f32::MAX); // huge(1.0)
        assert!(a.iter().all(|&v| v == f32::MAX));
    }

    #[test]
    fn empty_array_is_allowed() {
        let a: Shifted<f32> = Shifted::new(1, 0, 0.0);
        assert!(a.is_empty());
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn below_lower_bound_panics() {
        let a = Shifted::snow_soil(3, 4, 0.0f32);
        let _ = a[-3];
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn above_upper_bound_panics() {
        let a = Shifted::snow_soil(3, 4, 0.0f32);
        let _ = a[5];
    }

    #[test]
    fn get_returns_none_out_of_bounds() {
        let a = Shifted::snow_soil(3, 4, 0.0f32);
        assert!(a.get(-3).is_none());
        assert!(a.get(5).is_none());
        assert!(a.get(0).is_some());
    }
}
