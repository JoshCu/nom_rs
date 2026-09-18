//! Port of `src/OptionsType.f90` @ 0ff055e.
//!
//! Field names keep the Fortran's short forms (`opt_snf`, `dveg`, ...) because that is what
//! every physics module branches on; the long namelist names they come from are in
//! `namelist_read.rs`.

use crate::namelist_read::NamelistConfig;

/// `options_type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// precip_phase_option: 1 Jordan91, 2 T>2.2C, 3 T>0C, 4 from weather model,
    /// 5 user T threshold, 6 user wet-bulb threshold, 7 Jennings18 logistic
    pub opt_snf: i32,
    /// runoff_option: 1-8 (8 = dynamic VIC)
    pub opt_run: i32,
    /// drainage_option: 1-8
    pub opt_drn: i32,
    /// frozen_soil_option: 1 linear (more permeable), 2 nonlinear (less permeable)
    pub opt_inf: i32,
    /// dynamic_vic_option: 1 Philip, 2 Green-Ampt, 3 Smith-Parlange
    pub opt_infdv: i32,
    /// dynamic_veg_option: 1-9
    pub dveg: i32,
    /// snow_albedo_option: 1 BATS, 2 CLASS
    pub opt_alb: i32,
    /// radiative_transfer_option: 1 modified two-stream, 2 grid-cell, 3 vegetated fraction
    pub opt_rad: i32,
    /// sfc_drag_coeff_option: 1 Monin-Obukhov, 2 original Noah (Chen97)
    pub opt_sfc: i32,
    /// canopy_stom_resist_option: 1 Ball-Berry, 2 Jarvis
    pub opt_crs: i32,
    /// crop_model_option: 0 only (no crop model supported)
    pub opt_crop: i32,
    /// snowsoil_temp_time_option: 1-3
    pub opt_stc: i32,
    /// soil_temp_boundary_option: 1-2
    pub opt_tbot: i32,
    /// supercooled_water_option: 1-2
    pub opt_frz: i32,
    /// stomatal_resistance_option: 1 Noah, 2 CLM, 3 SSiB, 4 maximum ETRAN (approximates PET)
    pub opt_btr: i32,
    /// evap_srfc_resistance_option: 1 Sakaguchi-Zeng, 2 Sellers92, 3 adjusted Sellers, 4 snow,
    /// 5 minimised for soil evaporation, FSNO weighted (approximates PET)
    pub opt_rsf: i32,
    /// subsurface_option: 1 full Noah-MP, 2 one-way coupled hydrostatic, 3 two-way (unimplemented)
    pub opt_sub: i32,
}

impl Default for Options {
    /// `InitDefault` -- every field `huge(1)`, so an unset option is loud rather than silently 0.
    fn default() -> Self {
        let h = i32::MAX;
        Self {
            opt_snf: h,
            opt_run: h,
            opt_drn: h,
            opt_inf: h,
            opt_infdv: h,
            dveg: h,
            opt_alb: h,
            opt_rad: h,
            opt_sfc: h,
            opt_crs: h,
            opt_crop: h,
            opt_stc: h,
            opt_tbot: h,
            opt_frz: h,
            opt_btr: h,
            opt_rsf: h,
            opt_sub: h,
        }
    }
}

impl Options {
    /// `Init` + `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Self {
        let mut this = Self::default();
        this.init_transfer(namelist);
        this
    }

    /// `InitTransfer`.
    pub fn init_transfer(&mut self, namelist: &NamelistConfig) {
        self.opt_snf = namelist.precip_phase_option;
        self.opt_run = namelist.runoff_option;
        self.opt_drn = namelist.drainage_option;
        self.opt_inf = namelist.frozen_soil_option;
        self.opt_infdv = namelist.dynamic_vic_option;
        self.dveg = namelist.dynamic_veg_option;
        self.opt_alb = namelist.snow_albedo_option;
        self.opt_rad = namelist.radiative_transfer_option;
        self.opt_sfc = namelist.sfc_drag_coeff_option;
        self.opt_crs = namelist.canopy_stom_resist_option;
        self.opt_crop = namelist.crop_model_option;
        self.opt_stc = namelist.snowsoil_temp_time_option;
        self.opt_tbot = namelist.soil_temp_boundary_option;
        self.opt_frz = namelist.supercooled_water_option;
        self.opt_btr = namelist.stomatal_resistance_option;
        self.opt_rsf = namelist.evap_srfc_resistance_option;
        self.opt_sub = namelist.subsurface_option;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_huge() {
        let o = Options::default();
        assert_eq!(o.opt_snf, i32::MAX);
        assert_eq!(o.opt_sub, i32::MAX);
    }
}
