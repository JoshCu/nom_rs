//! Port of `src/DomainType.f90` @ eaa8282.

use crate::date_time_utils::{date_to_unix, DateError};
use crate::layers::Shifted;
use crate::namelist_read::NamelistConfig;

/// `domain_type`.
#[derive(Debug, Clone)]
pub struct Domain {
    /// i index in grid
    pub iloc: i32,
    /// j index in grid
    pub jloc: i32,
    /// run timestep (s)
    pub dt: f32,
    /// start date of the model run (YYYYMMDDHHmm)
    pub startdate: String,
    /// end date of the model run (YYYYMMDDHHmm)
    pub enddate: String,
    /// current date of the model run (YYYYMMDDHHmm)
    pub nowdate: String,
    /// unix start datetime (s since 1970-01-01)
    pub start_datetime: f64,
    /// unix end datetime
    pub end_datetime: f64,
    /// unix current datetime
    pub curr_datetime: f64,
    /// unix sim times given start/end dates and dt
    pub sim_datetimes: Vec<f64>,
    /// current integer time step of the model run
    pub itime: i32,
    /// total number of integer time steps
    pub ntime: i32,
    /// current time of the run in seconds from the beginning
    pub time_dbl: f64,
    /// latitude (deg)
    pub lat: f32,
    /// longitude (deg)
    pub lon: f32,
    /// measurement height of wind speed (m)
    pub zref: f32,
    /// terrain slope (deg)
    pub terrain_slope: f32,
    /// terrain azimuth or aspect (deg clockwise from north)
    pub azimuth: f32,
    /// land cover type
    pub vegtyp: i32,
    /// crop type
    pub croptype: i32,
    /// soil type
    pub isltyp: i32,
    /// surface type 1-soil; 2-lake
    pub ist: i32,
    /// depth of layer-bottom from soil surface, `1:nsoil`
    pub zsoil: Shifted<f32>,
    /// snow/soil layer thickness (m), `-nsnow+1:nsoil`
    pub dzsnso: Shifted<f32>,
    /// depth of snow/soil layer-bottom, `-nsnow+1:nsoil`
    pub zsnso: Shifted<f32>,
}

/// `huge(1)` -- the *integer* huge, which `InitDefault` assigns to the `real*8` datetime
/// fields. Widening it gives 2147483647.0, not `f64::MAX`.
const HUGE_INT_AS_REAL: f64 = i32::MAX as f64;

impl Domain {
    /// `Init` + `InitTransfer`.
    pub fn new(namelist: &NamelistConfig) -> Result<Self, DateError> {
        let mut this = Self::init(namelist);
        this.init_transfer(namelist)?;
        Ok(this)
    }

    /// `Init` = `InitAllocate` + `InitDefault`.
    pub fn init(namelist: &NamelistConfig) -> Self {
        Self {
            // InitDefault
            iloc: i32::MAX,
            jloc: i32::MAX,
            dt: f32::MAX,
            startdate: "EMPTYDATE999".to_string(),
            enddate: "EMPTYDATE999".to_string(),
            nowdate: "EMPTYDATE999".to_string(),
            start_datetime: HUGE_INT_AS_REAL,
            end_datetime: HUGE_INT_AS_REAL,
            curr_datetime: HUGE_INT_AS_REAL,
            sim_datetimes: Vec::new(),
            itime: i32::MAX,
            ntime: i32::MAX,
            time_dbl: f64::MAX,
            lat: f32::MAX,
            lon: f32::MAX,
            zref: f32::MAX,
            terrain_slope: f32::MAX,
            azimuth: f32::MAX,
            vegtyp: i32::MAX,
            croptype: i32::MAX,
            isltyp: i32::MAX,
            ist: i32::MAX,

            // InitAllocate -- every element huge(1.0)
            zsoil: Shifted::ones(namelist.nsoil, f32::MAX),
            dzsnso: Shifted::snow_soil(namelist.nsnow, namelist.nsoil, f32::MAX),
            zsnso: Shifted::snow_soil(namelist.nsnow, namelist.nsoil, f32::MAX),
        }
    }

    /// `InitTransfer`.
    pub fn init_transfer(&mut self, namelist: &NamelistConfig) -> Result<(), DateError> {
        self.dt = namelist.dt;
        self.startdate = namelist.startdate.clone();
        self.enddate = namelist.enddate.clone();
        self.lat = namelist.lat;
        self.lon = namelist.lon;
        self.terrain_slope = namelist.terrain_slope;
        self.azimuth = namelist.azimuth;
        self.zref = namelist.zref;
        self.zsoil = namelist.zsoil.clone();
        self.dzsnso = namelist.dzsnso.clone();
        self.vegtyp = namelist.vegtyp;
        self.croptype = namelist.croptype;
        self.isltyp = namelist.isltyp;
        self.ist = namelist.sfctyp;
        self.start_datetime = date_to_unix(&namelist.startdate)?;
        self.end_datetime = date_to_unix(&namelist.enddate)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namelist() -> NamelistConfig {
        NamelistConfig::parse(crate::namelist_read::tests_support::SHIPPED).unwrap()
    }

    #[test]
    fn allocates_with_fortran_bounds() {
        let d = Domain::init(&namelist());
        assert_eq!((d.zsoil.lo(), d.zsoil.hi()), (1, 4));
        assert_eq!((d.dzsnso.lo(), d.dzsnso.hi()), (-2, 4));
        assert_eq!((d.zsnso.lo(), d.zsnso.hi()), (-2, 4));
    }

    #[test]
    fn defaults_are_huge() {
        let d = Domain::init(&namelist());
        assert_eq!(d.dt, f32::MAX);
        assert_eq!(d.startdate, "EMPTYDATE999");
        assert_eq!(d.time_dbl, f64::MAX);
        // huge(1) is the integer huge even though the field is real*8.
        assert_eq!(d.start_datetime, 2147483647.0);
        assert_ne!(d.start_datetime, f64::MAX);
        // Allocated arrays start at huge(1.0).
        assert!(d.zsnso.iter().all(|&v| v == f32::MAX));
    }

    #[test]
    fn transfers_from_namelist() {
        let d = Domain::new(&namelist()).unwrap();
        assert_eq!(d.dt, 1800.0);
        assert_eq!(d.lat, 40.01f32);
        assert_eq!(d.lon, -88.37f32);
        assert_eq!(d.isltyp, 1);
        assert_eq!(d.ist, 1); // from sfctyp
        assert_eq!(d.start_datetime, 883_636_200.0);
        assert_eq!(d.end_datetime, 915_172_200.0);
        // dzsnso comes across with its bounds intact.
        assert_eq!(d.dzsnso[1], 0.1);
        assert_eq!(d.zsoil[4].to_bits(), (-1.0f32 * 2.0f32).to_bits());
    }
}
