//! Port of `src/WaterType.f90` @ 0ff055e.
//!
//! Plain state: `Init` allocates the profile arrays and fills everything with
//! `huge(1.0)`, so a field read before the physics writes it is loud rather than
//! plausibly zero. Field names and case follow the Fortran.

#![allow(non_snake_case)]

use crate::layers::Shifted;
use crate::namelist_read::NamelistConfig;

/// `huge(1.0)`.
const HUGE: f32 = f32::MAX;
/// `huge(1)`.
const HUGE_INT: i32 = i32::MAX;

/// `water_type` -- the water-balance state.
#[derive(Debug, Clone)]
pub struct Water {
    /// water input on soil surface [m/s]
    pub qinsur: f32,
    /// soil surface evap rate [m/s]
    pub qseva: f32,
    /// evapotranspiration, sum of QSEVA + ETRAN [m/s]
    pub EVAPOTRANS: f32,
    /// surface runoff [mm/s]
    pub runsrf: f32,
    /// baseflow (sturation excess) [mm/s]
    pub runsub: f32,
    /// soil-bottom free drainage [mm/s]
    pub qdrain: f32,
    /// the depth to water table [m]
    pub zwt: f32,
    /// soil water content between bottom of the soil and water table [m3/m3]
    pub smcwtd: f32,
    /// recharge to or from the water table when deep [m]
    pub deeprech: f32,
    /// maximum of fcr (-)
    pub fcrmax: f32,
    /// glacier outflow, added to RUNSUB
    pub snoflow: f32,
    /// infiltration rate at surface (m/s)
    pub pddum: f32,
    /// accumulated infiltration rate (m/s) in dynamic vic option
    pub FACC: f32,
    /// maximum soil ice content (m3/m3)
    pub sicemax: f32,
    /// canopy fraction buried by snow
    pub FB_snow: f32,
    /// rainfall (mm/s)
    pub rain: f32,
    /// snowfall (mm/s)
    pub snow: f32,
    /// bulk density of new snowfall (kg/m3)
    pub bdfall: f32,
    /// fraction of the gridcell that receives precipitation
    pub FP: f32,
    /// intercepted liquid water (mm)
    pub canliq: f32,
    /// intercepted ice mass (mm)
    pub canice: f32,
    /// wetted or snowed fraction of the canopy (-)
    pub FWET: f32,
    /// total canopy moisture content (CANLIQ + CANICE) (mm)
    pub CMC: f32,
    /// interception rate for rain (mm/s)
    pub QINTR: f32,
    /// drip rate for rain (mm/s)
    pub QDRIPR: f32,
    /// throughfall for rain (mm/s)
    pub QTHROR: f32,
    /// interception (loading) rate for snowfall (mm/s)
    pub QINTS: f32,
    /// drip (unloading) rate for intercepted snow (mm/s)
    pub QDRIPS: f32,
    /// throughfall of snowfall (mm/s)
    pub QTHROS: f32,
    /// rain at ground surface (mm/s) [+]
    pub QRAIN: f32,
    /// snow at ground surface (mm/s) [+]
    pub QSNOW: f32,
    /// snow depth increasing rate (m/s)
    pub SNOWHIN: f32,
    /// evaporation of intercepted water (mm/s) [+]
    pub ECAN: f32,
    /// transpiration rate (mm/s) [+]
    pub ETRAN: f32,
    /// snow surface frost rate[mm/s]
    pub QSNFRO: f32,
    /// snow surface sublimation rate[mm/s]
    pub QSNSUB: f32,
    /// snow height [m]
    pub SNOWH: f32,
    /// snow water eqv. [mm]
    pub SNEQV: f32,
    /// snow water eqv. of previous time step [mm]
    pub SNEQVO: f32,
    /// bulk density of snowpack (kg/m3)
    pub BDSNO: f32,
    /// melting water out of snow bottom [mm/s]
    pub QSNBOT: f32,
    pub PONDING: f32,
    pub PONDING1: f32,
    pub PONDING2: f32,
    /// ground surface evaporation/sublimation rate mm/s
    pub QVAP: f32,
    /// ground surface dew rate [mm/s]
    pub QDEW: f32,
    /// soil surface dew rate [mm/s]
    pub QSDEW: f32,
    /// water storage in lake (can be -) (mm)
    pub WSLAKE: f32,
    /// temporal time step for surface runoff calculations
    pub runsrf_dt: f32,
    /// accumulated saturation in VIC runoff scheme
    pub ASAT: f32,
    /// Accumulated meltwater from bottom snow layer [mm] (NWM 3.0)
    pub ACSNOM: f32,
    /// actual no. of snow layers
    pub ISNOW: i32,
    /// total soil water content [m3/m3]
    pub smc: Shifted<f32>,
    /// initial total soil water content [m3/m3]
    pub smc_init: Shifted<f32>,
    /// total soil ice content [m3/m3]
    pub sice: Shifted<f32>,
    /// total soil liquid content [m3/m3]
    pub sh2o: Shifted<f32>,
    /// transpiration rate (mm/s) [+]
    pub etrani: Shifted<f32>,
    /// Soil water transpiration factor (0 - 1)
    pub BTRANI: Shifted<f32>,
    /// hydraulic conductivity (m/s)
    pub wcnd: Shifted<f32>,
    /// impermeable fraction due to frozen soil
    pub fcr: Shifted<f32>,
    /// ice fraction at last timestep
    pub FICEOLD: Shifted<f32>,
    /// snow layer ice [mm]
    pub SNICE: Shifted<f32>,
    /// snow layer liquid water [mm]
    pub SNLIQ: Shifted<f32>,
    /// snow layer partial volume of ice [m3/m3]
    pub SNICEV: Shifted<f32>,
    /// snow layer partial volume of liquid water [m3/m3]
    pub SNLIQV: Shifted<f32>,
    /// fraction of ice at current time step
    pub FICE: Shifted<f32>,
    /// snow layer effective porosity [m3/m3]
    pub EPORE: Shifted<f32>,
    /// fraction of grid cell with snow cover
    pub FSNO: f32,
    /// soil water transpiration factor (0 to 1)
    pub BTRAN: f32,
}

impl Water {
    /// `Init` (`InitAllocate` + `InitDefault`) followed by `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Self {
        let nsoil = namelist.nsoil;
        let nsnow = namelist.nsnow;
        let mut this = Self {
            qinsur: HUGE,
            qseva: HUGE,
            EVAPOTRANS: HUGE,
            runsrf: HUGE,
            runsub: HUGE,
            qdrain: HUGE,
            zwt: HUGE,
            smcwtd: HUGE,
            deeprech: HUGE,
            fcrmax: HUGE,
            snoflow: HUGE,
            pddum: HUGE,
            FACC: HUGE,
            sicemax: HUGE,
            FB_snow: HUGE,
            rain: HUGE,
            snow: HUGE,
            bdfall: HUGE,
            FP: HUGE,
            canliq: HUGE,
            canice: HUGE,
            FWET: HUGE,
            CMC: HUGE,
            QINTR: HUGE,
            QDRIPR: HUGE,
            QTHROR: HUGE,
            QINTS: HUGE,
            QDRIPS: HUGE,
            QTHROS: HUGE,
            QRAIN: HUGE,
            QSNOW: HUGE,
            SNOWHIN: HUGE,
            ECAN: HUGE,
            ETRAN: HUGE,
            QSNFRO: HUGE,
            QSNSUB: HUGE,
            SNOWH: HUGE,
            SNEQV: HUGE,
            SNEQVO: HUGE,
            BDSNO: HUGE,
            QSNBOT: HUGE,
            PONDING: HUGE,
            PONDING1: HUGE,
            PONDING2: HUGE,
            QVAP: HUGE,
            QDEW: HUGE,
            QSDEW: HUGE,
            WSLAKE: HUGE,
            runsrf_dt: HUGE,
            ASAT: HUGE,
            ACSNOM: HUGE,
            ISNOW: HUGE_INT,
            smc: Shifted::ones(nsoil, HUGE),
            smc_init: Shifted::ones(nsoil, HUGE),
            sice: Shifted::ones(nsoil, HUGE),
            sh2o: Shifted::ones(nsoil, HUGE),
            etrani: Shifted::ones(nsoil, HUGE),
            BTRANI: Shifted::ones(nsoil, HUGE),
            wcnd: Shifted::ones(nsoil, HUGE),
            fcr: Shifted::ones(nsoil, HUGE),
            FICEOLD: Shifted::new(-nsnow + 1, 0, HUGE),
            SNICE: Shifted::new(-nsnow + 1, 0, HUGE),
            SNLIQ: Shifted::new(-nsnow + 1, 0, HUGE),
            SNICEV: Shifted::new(-nsnow + 1, 0, HUGE),
            SNLIQV: Shifted::new(-nsnow + 1, 0, HUGE),
            FICE: Shifted::new(-nsnow + 1, 0, HUGE),
            EPORE: Shifted::new(-nsnow + 1, 0, HUGE),
            FSNO: HUGE,
            BTRAN: HUGE,
        };
        this.init_transfer(namelist);
        this
    }

    /// `InitTransfer`.
    pub fn init_transfer(&mut self, namelist: &NamelistConfig) {
        self.sh2o = namelist.sh2o.clone();
        self.sice = namelist.sice.clone();
        // Volumetric soil water, and the copy the water balance is measured against.
        for i in 1..=self.smc.hi() {
            self.smc[i] = self.sh2o[i] + self.sice[i];
        }
        self.smc_init = self.smc.clone();
        self.zwt = namelist.zwt;
    }
}
