//! Port of `src/EnergyType.f90` @ 0ff055e.
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

/// `energy_type` -- the energy-balance state.
#[derive(Debug, Clone)]
pub struct Energy {
    /// vegetation temperature (k)
    pub TV: f32,
    /// ground temperature (k)
    pub TG: f32,
    /// canopy evaporation energy flux (w/m2)
    pub FCEV: f32,
    /// canopy transpiration energy flux (w/m2)
    pub FCTR: f32,
    /// growing season index (0=off, 1=on)
    pub IGS: f32,
    /// binary frozen canopy status (true when TV <= parameters%TFRZ)
    pub FROZEN_CANOPY: bool,
    /// binary frozen ground status (true when TG <= parameters%TFRZ)
    pub FROZEN_GROUND: bool,
    /// snow layer melting state index [0-no melt;1-melt]
    pub IMELT: Shifted<i32>,
    /// snow/soil layer temperature [k]
    pub STC: Shifted<f32>,
    /// snow and soil layer thermal conductivity [w/m/k]
    pub DF: Shifted<f32>,
    /// snow and soil layer heat capacity [j/m3/k]
    pub HCPCT: Shifted<f32>,
    /// temporary variable used in phase change [s/j/m2/k]
    pub FACT: Shifted<f32>,
    /// average snow temperature [k] (by layer mass) (a NWM 3.0 output variable)
    pub SNOWT_AVG: f32,
    /// precipitation advected heat - vegetation net (W/m2)
    pub PAHV: f32,
    /// precipitation advected heat - under canopy net (W/m2)
    pub PAHG: f32,
    /// precipitation advected heat - bare ground net (W/m2)
    pub PAHB: f32,
    /// precipitation advected heat - total (W/m2)
    pub PAH: f32,
    /// non-dimensional snow age
    pub TAUSS: f32,
    /// snow age (0 = new snow)
    pub FAGE: f32,
    /// broadband albedo in CLASS scheme
    pub ALB: f32,
    /// broadband albedo at previous timestep
    pub ALBOLD: f32,
    /// surface albedo (direct)
    pub ALBD: Shifted<f32>,
    /// surface albedo (diffuse)
    pub ALBI: Shifted<f32>,
    /// ground albedo (direct)
    pub ALBGRD: Shifted<f32>,
    /// ground albedo (diffuse)
    pub ALBGRI: Shifted<f32>,
    /// snow albedo for direct(1=vis, 2=nir)
    pub ALBSND: Shifted<f32>,
    /// snow albedo for diffuse
    pub ALBSNI: Shifted<f32>,
    /// flux abs by veg (per unit direct flux)
    pub FABD: Shifted<f32>,
    /// flux abs by veg (per unit diffuse flux)
    pub FABI: Shifted<f32>,
    /// down direct flux below veg (per unit dir flux)
    pub FTDD: Shifted<f32>,
    /// down diffuse flux below veg (per unit dif flux)
    pub FTDI: Shifted<f32>,
    /// down diffuse flux below veg (per unit dir flux)
    pub FTID: Shifted<f32>,
    /// down diffuse flux below veg (per unit dif flux)
    pub FTII: Shifted<f32>,
    /// direct flux reflected by veg layer (per unit incoming flux)
    pub FREVD: Shifted<f32>,
    /// diffuse flux reflected by veg layer (per unit incoming flux)
    pub FREVI: Shifted<f32>,
    /// direct flux reflected by ground (per unit incoming flux)
    pub FREGD: Shifted<f32>,
    /// direct flux reflected by ground (per unit incoming flux)
    pub FREGI: Shifted<f32>,
    /// leaf/stem reflectance weighted by fraction LAI and SAI
    pub RHO: Shifted<f32>,
    /// leaf/stem transmittance weighted by fraction LAI and SAI
    pub TAU: Shifted<f32>,
    /// cosine solar zenith angle [0-1]
    pub COSZ: f32,
    /// cosine solar zenith angle for flat ground [0-1]
    pub COSZ_HORIZ: f32,
    /// between canopy gap fraction for beam (-)
    pub BGAP: f32,
    /// within canopy gap fraction for beam (-)
    pub WGAP: f32,
    /// sunlit fraction of canopy (-)
    pub FSUN: f32,
    /// shaded fraction of canopy (-)
    pub FSHA: f32,
    /// sunlit leaf area (-)
    pub LAISUN: f32,
    /// shaded leaf area (-)
    pub LAISHA: f32,
    /// average absorbed par for sunlit leaves (w/m2)
    pub PARSUN: f32,
    /// average absorbed par for shaded leaves (w/m2)
    pub PARSHA: f32,
    /// solar radiation absorbed by vegetation (w/m2)
    pub SAV: f32,
    /// solar radiation absorbed by ground (w/m2)
    pub SAG: f32,
    /// total absorbed solar radiation (w/m2)
    pub FSA: f32,
    /// total reflected solar radiation (w/m2)
    pub FSR: f32,
    /// reflected solar radiation by vegetation
    pub FSRV: f32,
    /// reflected solar radiation by ground
    pub FSRG: f32,
    /// canopy air tmeperature (K)
    pub TAH: f32,
    /// canopy water vapor pressure (Pa)
    pub EAH: f32,
    /// zero plane displacement, ground (m)
    pub ZPD: f32,
    /// z0 momentum, ground (m)
    pub Z0MG: f32,
    /// roughness length, momentum (m)
    pub Z0M: f32,
    /// reference height (m)
    pub ZLVL: f32,
    /// momentum drag coefficient (vegetated surface)
    pub CMV: f32,
    /// momentum drag coefficient (bare ground)
    pub CMB: f32,
    /// momentum drag coefficient (weighted version of CMV + CMB by FVEG)
    pub CM: f32,
    /// drag coefficient for heat
    pub CH: f32,
    /// ground temperature (K)
    pub TGB: f32,
    /// mixing ratio at lowest model layer (g/g)
    pub QSFC: f32,
    /// vegetation emissivity (-)
    pub EMV: f32,
    /// ground emissivity (-)
    pub EMG: f32,
    /// psychrometric constant (Pa/K)
    pub GAMMAV: f32,
    /// psychrometric constant (Pa/K)
    pub GAMMAG: f32,
    /// evaporation heat flux (w/m2)  [+= to atm]
    pub EVC: f32,
    /// net longwave radiation (w/m2) [+= to atm]
    pub IRC: f32,
    /// net longwave radiation (w/m2) [+= to atm]
    pub IRG: f32,
    /// sensible heat flux (w/m2)     [+= to atm]
    pub SHC: f32,
    /// sensible heat flux (w/m2)     [+= to atm]
    pub SHG: f32,
    /// sensible heat flux (w/m2) [+ to atm]
    pub SHB: f32,
    /// evaporation heat flux (w/m2)  [+= to atm]
    pub EVG: f32,
    /// latent heat flux (w/m2)   [+ to atm]
    pub EVB: f32,
    /// transpiration heat flux (w/m2)[+= to atm]
    pub TR: f32,
    /// ground heat (w/m2) [+ = to soil]
    pub GH: f32,
    /// ground heat flux (w/m2)  [+ to soil]
    pub GHB: f32,
    /// ground heat flux [w/m2]  [+ to soil]
    pub GHV: f32,
    /// 2 m height air temperature (k)
    pub T2MV: f32,
    /// leaf exchange coefficient
    pub CHLEAF: f32,
    /// under canopy exchange coefficient
    pub CHUC: f32,
    /// sensible heat exch. coeff. over vegetated fraction (m/s)
    pub CHV2: f32,
    /// sensible heat exch. coeff. bare-ground (m/s)
    pub CHB2: f32,
    /// check
    pub Q2V: f32,
    /// latent heat of vaporization/subli (j/kg) (varies if froz)
    pub LATHEAV: f32,
    /// latent heat of vaporization/subli (j/kg) (varies if froz)
    pub LATHEAG: f32,
    /// latent heat of vaporization/subli (j/kg) (bare ground version)
    pub LATHEA: f32,
    /// ground surface resistance (s/m)
    pub RSURF: f32,
    /// relative humidity in surface soil/snow air space (-)
    pub RHSUR: f32,
    /// wind stress: e-w (n/m2) vegetation
    pub TAUXV: f32,
    /// wind stress: n-s (n/m2) vegetation
    pub TAUYV: f32,
    /// wind stress: e-w (n/m2) bare ground
    pub TAUXB: f32,
    /// wind stress: n-s (n/m2) bare ground
    pub TAUYB: f32,
    /// wind stress: e-w (n/m2)
    pub TAUX: f32,
    /// wind stress: n-s (n/m2)
    pub TAUY: f32,
    /// sensible heat conductance for diagnostics
    pub CAH2: f32,
    /// sensible heat conductance for diagnostics (bare ground)
    pub EHB2: f32,
    /// 2 m height air temperature (K)
    pub T2MB: f32,
    /// bare ground heat conductance
    pub Q2B: f32,
    /// ground surface temp. [k]
    pub TGV: f32,
    /// sensible heat exchange coefficient
    pub CHV: f32,
    /// sunlit leaf stomatal resistance (s/m)
    pub RSSUN: f32,
    /// shaded leaf stomatal resistance (s/m)
    pub RSSHA: f32,
    /// leaf boundary layer resistance (s/m)
    pub RB: f32,
    /// total net LW. rad (w/m2)   [+ to atm]
    pub FIRA: f32,
    /// total sensible heat (w/m2) [+ to atm]
    pub FSH: f32,
    /// ground evaporation (w/m2)  [+ to atm]
    pub FGEV: f32,
    /// radiative temperature (k)
    pub TRAD: f32,
    /// net longwave rad. [w/m2] [+ to atm]
    pub IRB: f32,
    /// ground heat flux (w/m2)   [+ to soil]
    pub SSOIL: f32,
    /// 2-meter air temperature (k)
    pub T2M: f32,
    /// surface temperature (k)
    pub TS: f32,
    /// sensible heat exchange coefficient
    pub CHB: f32,
    pub Q1: f32,
    pub Q2E: f32,
    /// combined z0 sent to coupled model
    pub Z0WRF: f32,
    /// net surface emissivity
    pub EMISSI: f32,
    /// total photosyn. (umolco2/m2/s) [+]
    pub PSN: f32,
    /// sunlit photosynthesis (umolco2/m2/s)
    pub PSNSUN: f32,
    /// shaded photosynthesis (umolco2/m2/s)
    pub PSNSHA: f32,
    /// total photosyn. active energy (w/m2)
    pub APAR: f32,
    /// snowmelt [mm/s]
    pub QMELT: f32,
    /// latent heat (total) flux [W m-2]
    pub LH: f32,
    /// ground surface temperature (K, takes value of TG when SNOWH <= 0.05 and STC[0] when SNOWH > 0.05)
    pub TGS: f32,
    /// 1 if sea ice, -1 if glacier, 0 if no land ice (seasonal snow)
    pub ICE: i32,
}

impl Energy {
    /// `Init` (`InitAllocate` + `InitDefault`) followed by `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Self {
        let nsoil = namelist.nsoil;
        let nsnow = namelist.nsnow;
        let mut this = Self {
            TV: HUGE,
            TG: HUGE,
            FCEV: HUGE,
            FCTR: HUGE,
            IGS: HUGE,
            FROZEN_CANOPY: false,
            FROZEN_GROUND: false,
            IMELT: Shifted::snow_soil(nsnow, nsoil, HUGE_INT),
            STC: Shifted::snow_soil(nsnow, nsoil, HUGE),
            DF: Shifted::snow_soil(nsnow, nsoil, HUGE),
            HCPCT: Shifted::snow_soil(nsnow, nsoil, HUGE),
            FACT: Shifted::snow_soil(nsnow, nsoil, HUGE),
            SNOWT_AVG: HUGE,
            PAHV: HUGE,
            PAHG: HUGE,
            PAHB: HUGE,
            PAH: HUGE,
            TAUSS: HUGE,
            FAGE: HUGE,
            ALB: HUGE,
            ALBOLD: HUGE,
            ALBD: Shifted::ones(2, HUGE),
            ALBI: Shifted::ones(2, HUGE),
            ALBGRD: Shifted::ones(2, HUGE),
            ALBGRI: Shifted::ones(2, HUGE),
            ALBSND: Shifted::ones(2, HUGE),
            ALBSNI: Shifted::ones(2, HUGE),
            FABD: Shifted::ones(2, HUGE),
            FABI: Shifted::ones(2, HUGE),
            FTDD: Shifted::ones(2, HUGE),
            FTDI: Shifted::ones(2, HUGE),
            FTID: Shifted::ones(2, HUGE),
            FTII: Shifted::ones(2, HUGE),
            FREVD: Shifted::ones(2, HUGE),
            FREVI: Shifted::ones(2, HUGE),
            FREGD: Shifted::ones(2, HUGE),
            FREGI: Shifted::ones(2, HUGE),
            RHO: Shifted::ones(2, HUGE),
            TAU: Shifted::ones(2, HUGE),
            COSZ: HUGE,
            COSZ_HORIZ: HUGE,
            BGAP: HUGE,
            WGAP: HUGE,
            FSUN: HUGE,
            FSHA: HUGE,
            LAISUN: HUGE,
            LAISHA: HUGE,
            PARSUN: HUGE,
            PARSHA: HUGE,
            SAV: HUGE,
            SAG: HUGE,
            FSA: HUGE,
            FSR: HUGE,
            FSRV: HUGE,
            FSRG: HUGE,
            TAH: HUGE,
            EAH: HUGE,
            ZPD: HUGE,
            Z0MG: HUGE,
            Z0M: HUGE,
            ZLVL: HUGE,
            CMV: HUGE,
            CMB: HUGE,
            CM: HUGE,
            CH: HUGE,
            TGB: HUGE,
            QSFC: HUGE,
            EMV: HUGE,
            EMG: HUGE,
            GAMMAV: HUGE,
            GAMMAG: HUGE,
            EVC: HUGE,
            IRC: HUGE,
            IRG: HUGE,
            SHC: HUGE,
            SHG: HUGE,
            SHB: HUGE,
            EVG: HUGE,
            EVB: HUGE,
            TR: HUGE,
            GH: HUGE,
            GHB: HUGE,
            GHV: HUGE,
            T2MV: HUGE,
            CHLEAF: HUGE,
            CHUC: HUGE,
            CHV2: HUGE,
            CHB2: HUGE,
            Q2V: HUGE,
            LATHEAV: HUGE,
            LATHEAG: HUGE,
            LATHEA: HUGE,
            RSURF: HUGE,
            RHSUR: HUGE,
            TAUXV: HUGE,
            TAUYV: HUGE,
            TAUXB: HUGE,
            TAUYB: HUGE,
            TAUX: HUGE,
            TAUY: HUGE,
            CAH2: HUGE,
            EHB2: HUGE,
            T2MB: HUGE,
            Q2B: HUGE,
            // TGV is assigned by neither `InitDefault` nor `RunModule`, so in the Fortran it
            // holds whatever the allocation left there until the physics first writes it.
            // The reference build leaves it zero -- fresh pages -- and the fixtures record
            // that, so zero is what keeps the first timestep bit-identical. This is the one
            // value in the port that depends on the compiler rather than on the source.
            TGV: 0.0,
            CHV: HUGE,
            RSSUN: HUGE,
            RSSHA: HUGE,
            RB: HUGE,
            FIRA: HUGE,
            FSH: HUGE,
            FGEV: HUGE,
            TRAD: HUGE,
            IRB: HUGE,
            SSOIL: HUGE,
            T2M: HUGE,
            TS: HUGE,
            CHB: HUGE,
            Q1: HUGE,
            Q2E: HUGE,
            Z0WRF: HUGE,
            EMISSI: HUGE,
            PSN: HUGE,
            PSNSUN: HUGE,
            PSNSHA: HUGE,
            APAR: HUGE,
            QMELT: HUGE,
            LH: HUGE,
            TGS: HUGE,
            ICE: HUGE_INT,
        };
        this.init_transfer(namelist);
        this
    }

    /// `InitTransfer`, which upstream leaves empty. Kept so the three state types
    /// present the same surface and a future upstream body has somewhere to land.
    pub fn init_transfer(&mut self, _namelist: &NamelistConfig) {}
}
