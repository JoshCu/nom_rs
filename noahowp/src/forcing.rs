//! Port of `src/ForcingType.f90` @ 0ff055e.
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

/// `forcing_type` -- the atmospheric state for the current timestep.
#[derive(Debug, Clone)]
pub struct Forcing {
    /// surface pressure (pa)
    pub SFCPRS: f32,
    /// surface air temperature [K]
    pub SFCTMP: f32,
    /// specific humidity (note: in some Noah-MP versions Q2 is mixing ratio)
    pub Q2: f32,
    /// total input precipitation[mm/s]
    pub PRCP: f32,
    /// convective precipitation entering  [mm/s]
    pub PRCPCONV: f32,
    /// non-convective precipitation entering [mm/s]
    pub PRCPNONC: f32,
    /// shallow convective precip entering  [mm/s]
    pub PRCPSHCV: f32,
    /// snow entering land model [mm/s]
    pub PRCPSNOW: f32,
    /// graupel entering land model [mm/s]
    pub PRCPGRPL: f32,
    /// hail entering land model [mm/s]
    pub PRCPHAIL: f32,
    /// downward shortwave radiation (w/m2)
    pub SOLDN: f32,
    /// atmospheric longwave radiation (w/m2)
    pub LWDN: f32,
    /// foliage nitrogen concentration (%)
    pub FOLN: f32,
    /// atmospheric co2 concentration partial pressure (pa)
    pub O2PP: f32,
    /// atmospheric o2 concentration partial pressure (pa)
    pub CO2PP: f32,
    /// wind speed in eastward dir (m/s)
    pub UU: f32,
    /// wind speed in northward dir (m/s)
    pub VV: f32,
    /// bottom condition for soil temperature [K]
    pub TBOT: f32,
    /// wind speed at reference height (m/s)
    pub UR: f32,
    /// potential temperature (k)
    pub THAIR: f32,
    /// specific humidity (kg/kg) (q2/(1+q2))
    pub QAIR: f32,
    /// vapor pressure air (pa)
    pub EAIR: f32,
    /// density air (kg/m3)
    pub RHOAIR: f32,
    /// fraction of ice in precipitation (-)
    pub FPICE: f32,
    /// downward solar filtered by sun angle [w/m2]
    pub SWDOWN: f32,
    /// julian day of year
    pub JULIAN: f32,
    /// year length (days)
    pub YEARLEN: i32,
    /// incoming direct solar radiation (w/m2)
    pub SOLAD: Shifted<f32>,
    /// incoming diffuse solar radiation (w/m2)
    pub SOLAI: Shifted<f32>,
}

impl Forcing {
    /// `Init` (`InitAllocate` + `InitDefault`) followed by `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Self {
        let mut this = Self {
            SFCPRS: HUGE,
            SFCTMP: HUGE,
            Q2: HUGE,
            PRCP: HUGE,
            PRCPCONV: HUGE,
            PRCPNONC: HUGE,
            PRCPSHCV: HUGE,
            PRCPSNOW: HUGE,
            PRCPGRPL: HUGE,
            PRCPHAIL: HUGE,
            SOLDN: HUGE,
            LWDN: HUGE,
            FOLN: HUGE,
            O2PP: HUGE,
            CO2PP: HUGE,
            UU: HUGE,
            VV: HUGE,
            TBOT: HUGE,
            UR: HUGE,
            THAIR: HUGE,
            QAIR: HUGE,
            EAIR: HUGE,
            RHOAIR: HUGE,
            FPICE: HUGE,
            // `InitDefault` skips SWDOWN; `RunModule`'s initialisation sets it to 0.0
            // before anything reads it, so the value here is never observed.
            SWDOWN: HUGE,
            JULIAN: HUGE,
            YEARLEN: HUGE_INT,
            SOLAD: Shifted::ones(2, HUGE),
            SOLAI: Shifted::ones(2, HUGE),
        };
        this.init_transfer(namelist);
        this
    }

    /// `InitTransfer`, which upstream leaves empty. Kept so the three state types
    /// present the same surface and a future upstream body has somewhere to land.
    pub fn init_transfer(&mut self, _namelist: &NamelistConfig) {}
}
