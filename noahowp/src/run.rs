//! Port of `src/RunModule.f90` @ 0242a96.
//!
//! `initialize_from_file` builds the model and then overwrites a long list of state that the
//! `*Type` modules had just set to `huge(1.0)`. That second pass is not decoration: it is where
//! the run actually starts from, and a field it misses keeps the `huge` sentinel.
//!
//! Four pieces of the module are deliberately absent, all of them behind `#ifndef` guards that
//! the BMI path compiles out: `open_forcing_file` and `read_forcing_text`, because forcing
//! arrives through `set_value`, and `initialize_output` / `add_to_output`, the only NetCDF
//! users in the codebase.

#![allow(non_snake_case)]

use crate::date_time_utils::{get_utime_list, DateError};
use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::levels::Levels;
use crate::namelist_read::{ConfigError, NamelistConfig};
use crate::options::Options;
use crate::parameters::Parameters;
use crate::parameters_read::Tables;
use crate::physics::atm_processing::PrecipInput;
use crate::physics::energy_main::{energy_main, EmittedLongwaveNotPositive};
use crate::physics::forcing_main::forcing_main;
use crate::physics::interception::interception_main;
use crate::physics::water_main::water_main;
use crate::utilities::utilities_main;
use crate::water::Water;

/// Why a timestep could not be taken. Upstream `stop`s.
#[derive(Debug)]
pub enum StepError {
    Energy(EmittedLongwaveNotPositive),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepError::Energy(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StepError {}

impl From<EmittedLongwaveNotPositive> for StepError {
    fn from(e: EmittedLongwaveNotPositive) -> Self {
        StepError::Energy(e)
    }
}

/// `noahowp_type` -- one model column.
///
/// Holds every piece of its own state, with nothing in a global. That is what makes the model
/// `Send`, and it is the property a threaded calibration driver depends on.
#[derive(Debug, Clone)]
pub struct NoahOwp {
    pub namelist: NamelistConfig,
    pub levels: Levels,
    pub domain: Domain,
    pub options: Options,
    pub parameters: Parameters,
    pub water: Water,
    pub forcing: Forcing,
    pub energy: Energy,
}

impl NoahOwp {
    /// `initialize_from_file`.
    pub fn new(namelist: NamelistConfig, tables: &Tables) -> Result<Self, ConfigError> {
        let levels = Levels::new(&namelist);
        let domain = Domain::new(&namelist)?;
        let options = Options::new(&namelist);
        let parameters = Parameters::new(&namelist, tables)?;
        let forcing = Forcing::new(&namelist);
        let energy = Energy::new(&namelist);
        let water = Water::new(&namelist);

        let mut this = Self {
            namelist,
            levels,
            domain,
            options,
            parameters,
            water,
            forcing,
            energy,
        };
        this.initialize_state()?;
        Ok(this)
    }

    /// `advance_in_time`.
    ///
    /// `precip_input` says which precipitation forcing the driver wrote: the BMI path sets
    /// `PRCPNONC` ([`PrecipInput::NonConvective`]), the reference build's ASCII reader `PRCP`.
    pub fn advance_in_time(&mut self, precip_input: PrecipInput) -> Result<(), StepError> {
        self.solve_noahowp(precip_input)?;

        self.domain.itime += 1; // increment the integer time by 1
        // `dble(time_dbl + dt)`: real*8 + real promotes dt, so the sum is already f64.
        self.domain.time_dbl += self.domain.dt as f64; // increment model time in seconds by DT
        Ok(())
    }

    /// `solve_noahowp` -- one timestep: the five physics calls, in order.
    pub fn solve_noahowp(&mut self, precip_input: PrecipInput) -> Result<(), StepError> {
        let domain = &mut self.domain;

        // Compute the current UNIX datetime -- end-of-timestep datetimes.
        //
        // Upstream indexes `sim_datetimes(itime)` unchecked, which reads past the end of the
        // list once a driver steps beyond `end_time` (bmi-driver's `allow_exceed_end_time`).
        // Nothing in the timestep reads the value, so past the end `curr_datetime` is
        // extrapolated by dt rather than read out of bounds.
        let i = (domain.itime - 1) as usize;
        domain.curr_datetime = match domain.sim_datetimes.get(i) {
            Some(&t) => t,
            None => match domain.sim_datetimes.last() {
                Some(&last) => {
                    let beyond = (i + 1 - domain.sim_datetimes.len()) as f64;
                    last + beyond * domain.dt as f64
                }
                None => domain.curr_datetime,
            },
        };

        utilities_main(domain.itime, domain, &mut self.forcing, &mut self.energy);

        forcing_main(
            &self.options,
            &self.parameters,
            &mut self.forcing,
            &mut self.energy,
            &mut self.water,
            precip_input,
        );

        interception_main(
            &self.domain,
            &self.options,
            &mut self.parameters,
            &self.forcing,
            &mut self.energy,
            &mut self.water,
        );

        energy_main(
            &mut self.domain,
            &self.levels,
            &self.options,
            &mut self.parameters,
            &self.forcing,
            &mut self.energy,
            &mut self.water,
        )?;

        water_main(
            &mut self.domain,
            &self.levels,
            &self.options,
            &self.parameters,
            &self.forcing,
            &mut self.energy,
            &mut self.water,
        );

        Ok(())
    }

    /// The assignments `initialize_from_file` makes after the types are built.
    ///
    /// Transcribed in source order, including the comments that flag which option each value
    /// is really for -- several are described upstream as only needed for a `runoff_option`
    /// this configuration does not use, and are set anyway.
    fn initialize_state(&mut self) -> Result<(), DateError> {
        let namelist = &self.namelist;
        let water = &mut self.water;

        // for soil water
        // water%zwt is left as InitTransfer set it -- the upstream line is commented out.
        water.smcwtd = 0.0; // should only be needed for run=5
        water.deeprech = 0.0; // should only be needed for run=5
        water.qinsur = 0.0;
        water.runsrf = 0.0;
        water.runsub = 0.0;
        water.qdrain = 0.0;
        water.wcnd.fill(0.0);
        water.fcrmax = 0.0;
        water.snoflow = 0.0; // glacier outflow for all RUNSUB options, [mm/s]
        water.qseva = 0.0; // soil evaporation [mm/s]
        water.etrani.fill(0.0); // transpiration from each level [mm/s]
        water.BTRANI.fill(0.0); // soil water transpiration factor by layer
        water.BTRAN = 0.0;

        // for canopy water
        water.rain = 0.0; // rainfall mm/s
        water.snow = 0.0; // snowfall mm/s
        water.bdfall = 0.0; // bulk density of snowfall (kg/m3)
        water.FB_snow = 0.0; // canopy fraction buried by snow
        water.FP = 1.0; // fraction of the gridcell that receives precipitation
        water.canliq = 0.0; // canopy liquid water [mm]
        water.canice = 0.0; // canopy frozen water [mm]
        water.FWET = 0.0; // canopy fraction wet or snow
        water.CMC = 0.0; // intercepted water per ground area (mm)
        water.QINTR = 0.0;
        water.QDRIPR = 0.0;
        water.QTHROR = 0.0;
        water.QINTS = 0.0;
        water.QDRIPS = 0.0;
        water.QTHROS = 0.0;
        water.QRAIN = 0.0; // rain at ground srf (mm/s) [+]
        water.QSNOW = 0.0; // snow at ground srf (mm/s) [+]
        water.SNOWHIN = 0.0; // snow depth increasing rate (m/s)
        water.ECAN = 0.0; // evap of intercepted water (mm/s) [+]
        water.ETRAN = 0.0; // transpiration rate (mm/s) [+]

        // for snow water
        water.QVAP = 0.0; // evaporation/sublimation rate mm/s
        water.ISNOW = 0;
        water.SNOWH = 0.0;
        water.SNEQV = 0.0;
        water.SNEQVO = 0.0;
        water.BDSNO = 0.0;
        water.PONDING = 0.0;
        water.PONDING1 = 0.0;
        water.PONDING2 = 0.0;
        water.QSNBOT = 0.0;
        water.QSNFRO = 0.0;
        water.QSNSUB = 0.0;
        water.QDEW = 0.0;
        water.QSDEW = 0.0;
        water.SNICE.fill(0.0);
        water.SNLIQ.fill(0.0);
        water.FICEOLD.fill(0.0);
        water.FSNO = 0.0;

        // for energy-related variable
        let energy = &mut self.energy;
        energy.TV = 298.0; // leaf temperature [K]
        energy.TG = 298.0; // ground temperature [K]
        energy.CM = 0.0; // momentum drag coefficient
        energy.CH = 0.0; // heat drag coefficient
        energy.FCEV = 5.0; // constant canopy evaporation (w/m2) [+ to atm]
        energy.FCTR = 5.0; // constant transpiration (w/m2) [+ to atm]
        energy.IMELT.fill(1); // freeze
        energy.STC.fill(298.0);
        energy.COSZ = 0.7; // cosine of solar zenith angle
        energy.ICE = 0; // 1 sea ice, -1 glacier, 0 no land ice
        energy.ALB = 0.6; // initialize snow albedo in CLASS routine
        energy.ALBOLD = 0.6;
        energy.FROZEN_CANOPY = false; // used to define latent heat pathway
        energy.FROZEN_GROUND = false;

        // forcing-related variables.
        //
        // The eight the ASCII reader supplies -- UU, VV, SFCPRS, SFCTMP, Q2, PRCP, SOLDN,
        // LWDN -- are commented out upstream and keep their `huge(1.0)`. On the BMI path they
        // arrive through `set_value` before the first `update`.
        let forcing = &mut self.forcing;
        forcing.PRCPCONV = 0.0; // convective precipitation entering [mm/s]
        forcing.PRCPNONC = 0.0; // non-convective precipitation entering [mm/s]
        forcing.PRCPSHCV = 0.0; // shallow convective precip entering [mm/s]
        forcing.PRCPSNOW = 0.0; // snow entering land model [mm/s]
        forcing.PRCPGRPL = 0.0; // graupel entering land model [mm/s]
        forcing.PRCPHAIL = 0.0; // hail entering land model [mm/s]
        forcing.THAIR = 0.0; // potential temperature (k)
        forcing.QAIR = 0.0; // specific humidity (kg/kg)
        forcing.EAIR = 0.0; // vapor pressure air (pa)
        forcing.RHOAIR = 0.0; // density air (kg/m3)
        forcing.SWDOWN = 0.0; // downward solar filtered by sun angle [w/m2]
        forcing.FPICE = 0.0; // fraction of ice
        forcing.JULIAN = 0.0; // setting arbitrary julian day
        forcing.YEARLEN = 365; // setting year to be normal (i.e. not a leap year)
        forcing.FOLN = 1.0; // foliage nitrogen concentration (%)
        forcing.TBOT = 285.0; // bottom condition for soil temperature [K]

        // domain variables
        let domain = &mut self.domain;
        for i in -namelist.nsnow + 1..=0 {
            domain.zsnso[i] = 0.0;
        }
        for i in 1..=namelist.nsoil {
            domain.zsnso[i] = namelist.zsoil[i];
        }

        // time variables
        domain.itime = 1; // initialize the time loop counter at 1
        domain.time_dbl = 0.0; // start model run at t = 0

        domain.sim_datetimes = get_utime_list(
            domain.start_datetime,
            domain.end_datetime,
            domain.dt,
        )?;
        domain.ntime = domain.sim_datetimes.len() as i32;

        Ok(())
    }
}
