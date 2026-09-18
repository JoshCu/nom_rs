//! Port of `src/LevelsType.f90` @ 0ff055e.

use crate::namelist_read::NamelistConfig;

/// `levels_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levels {
    /// number of soil layers
    pub nsoil: i32,
    /// number of snow layers
    pub nsnow: i32,
    /// number of vegetation types in the chosen table
    pub nveg: i32,
}

impl Default for Levels {
    /// `InitDefault`.
    fn default() -> Self {
        Self { nsoil: i32::MAX, nsnow: i32::MAX, nveg: i32::MAX }
    }
}

impl Levels {
    /// `Init` + `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Self {
        let mut this = Self::default();
        this.init_transfer(namelist);
        this
    }

    /// `InitTransfer`.
    pub fn init_transfer(&mut self, namelist: &NamelistConfig) {
        self.nsoil = namelist.nsoil;
        self.nsnow = namelist.nsnow;
        self.nveg = namelist.nveg;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_huge() {
        let l = Levels::default();
        assert_eq!(l.nsoil, i32::MAX);
    }
}
