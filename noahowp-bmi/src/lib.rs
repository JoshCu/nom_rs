//! Port of `bmi/bmi_noahowp.f90` @ eaa8282.
//!
//! Scope is what `bmi-driver` actually calls -- see `docs/RUST_REWRITE_PLAN.md` section 1.1.
//! `get_value_ptr`, `*_at_indices` and the extended grid functions are deliberately absent.
//!
//! Physics is not ported yet: [`BmiNoahOwp::update`] returns [`BmiError::NotImplemented`].
//! Everything that does not need the physics column -- metadata, grids, time, the variable
//! registry, and `initialize` reading a config -- is complete and tested.

pub mod vars;

use noahowp::domain::Domain;
use noahowp::levels::Levels;
use noahowp::options::Options;
use noahowp::{ConfigError, NamelistConfig};
use std::path::Path;

pub use vars::{VarInfo, VarType, COMPONENT_NAME, INPUT_ITEMS, OUTPUT_ITEMS, VARS};

#[derive(Debug)]
pub enum BmiError {
    Config(ConfigError),
    Date(noahowp::date_time_utils::DateError),
    /// The model has not been initialized.
    NotInitialized,
    /// No such exchange item.
    UnknownVar(String),
    /// No such grid id.
    UnknownGrid(i32),
    /// `update_until` was given a time before the current one.
    TimeInThePast { requested: f64, current: f64 },
    /// Reached only for the parts of the port that are still outstanding.
    NotImplemented(&'static str),
}

impl std::fmt::Display for BmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BmiError::Config(e) => write!(f, "{e}"),
            BmiError::Date(e) => write!(f, "{e}"),
            BmiError::NotInitialized => write!(f, "model is not initialized"),
            BmiError::UnknownVar(n) => write!(f, "unknown variable {n:?}"),
            BmiError::UnknownGrid(g) => write!(f, "unknown grid {g}"),
            BmiError::TimeInThePast { requested, current } => {
                write!(f, "update_until({requested}) is before the current time {current}")
            }
            BmiError::NotImplemented(what) => write!(f, "{what} is not ported yet"),
        }
    }
}

impl std::error::Error for BmiError {}

impl From<ConfigError> for BmiError {
    fn from(e: ConfigError) -> Self {
        BmiError::Config(e)
    }
}

impl From<noahowp::date_time_utils::DateError> for BmiError {
    fn from(e: noahowp::date_time_utils::DateError) -> Self {
        BmiError::Date(e)
    }
}

pub type BmiResult<T> = Result<T, BmiError>;

/// The model state a `bmi_noahowp` instance wraps.
///
/// Holds no globals, so instances are independent and `Send` -- which is what lets a driver run
/// many catchments as threads rather than as processes. See the plan, section 4.
#[derive(Debug)]
struct Model {
    // namelist and options are read by the physics column, which is not ported yet.
    #[allow(dead_code)]
    namelist: NamelistConfig,
    levels: Levels,
    domain: Domain,
    #[allow(dead_code)]
    options: Options,
}

/// `bmi_noahowp`.
#[derive(Debug, Default)]
pub struct BmiNoahOwp {
    model: Option<Model>,
}

fn assert_send<T: Send>() {}

impl BmiNoahOwp {
    pub fn new() -> Self {
        Self::default()
    }

    fn model(&self) -> BmiResult<&Model> {
        self.model.as_ref().ok_or(BmiError::NotInitialized)
    }

    fn model_mut(&mut self) -> BmiResult<&mut Model> {
        self.model.as_mut().ok_or(BmiError::NotInitialized)
    }

    pub fn is_initialized(&self) -> bool {
        self.model.is_some()
    }

    // ------------------------------------------------------------------ lifecycle

    /// `noahowp_initialize` -> `initialize_from_file`.
    pub fn initialize(&mut self, config_file: &str) -> BmiResult<()> {
        let namelist = NamelistConfig::read(Path::new(config_file))?;

        let levels = Levels::new(&namelist);
        let options = Options::new(&namelist);
        let mut domain = Domain::new(&namelist)?;

        // RunModule: the model starts at nowdate = startdate, itime = 1, t = 0.
        domain.nowdate = domain.startdate.clone();
        domain.itime = 1;
        domain.time_dbl = 0.0;

        // domain%zsnso(-nsnow+1:0) = 0.0; domain%zsnso(1:nsoil) = namelist%zsoil
        for iz in (-namelist.nsnow + 1)..=0 {
            domain.zsnso[iz] = 0.0;
        }
        for iz in 1..=namelist.nsoil {
            domain.zsnso[iz] = namelist.zsoil[iz];
        }

        // The simulation timeline, from which ntime (and so end_time) follows.
        let sim_datetimes = noahowp::date_time_utils::get_utime_list(
            domain.start_datetime,
            domain.end_datetime,
            domain.dt,
        )?;
        domain.ntime = sim_datetimes.len() as i32;
        domain.sim_datetimes = sim_datetimes;

        self.model = Some(Model { namelist, levels, domain, options });
        Ok(())
    }

    /// `noahowp_update` -> `advance_in_time`.
    pub fn update(&mut self) -> BmiResult<()> {
        let _ = self.model_mut()?;
        Err(BmiError::NotImplemented("solve_noahowp (the physics column)"))
    }

    /// `noahowp_update_until`.
    ///
    /// The fractional remainder is discarded, as upstream does -- `update_frac` is commented
    /// out there.
    pub fn update_until(&mut self, time: f64) -> BmiResult<()> {
        let (current, dt) = {
            let m = self.model()?;
            (m.domain.time_dbl, m.domain.dt as f64)
        };
        if time < current {
            return Err(BmiError::TimeInThePast { requested: time, current });
        }
        let n_steps = ((time - current) / dt).floor() as i64;
        for _ in 0..n_steps {
            self.update()?;
        }
        Ok(())
    }

    /// `noahowp_finalize` -> `cleanup`.
    ///
    /// Upstream closes the NetCDF output file here; with `NGEN_OUTPUT_ACTIVE` that body is
    /// empty, and this port has no output file at all. Dropping the state is the whole job.
    pub fn finalize(&mut self) -> BmiResult<()> {
        self.model = None;
        Ok(())
    }

    // ------------------------------------------------------------------ metadata

    pub fn get_component_name(&self) -> &'static str {
        COMPONENT_NAME
    }

    pub fn get_input_item_count(&self) -> i32 {
        INPUT_ITEMS.len() as i32
    }

    pub fn get_output_item_count(&self) -> i32 {
        OUTPUT_ITEMS.len() as i32
    }

    pub fn get_input_var_names(&self) -> &'static [&'static str] {
        INPUT_ITEMS
    }

    pub fn get_output_var_names(&self) -> &'static [&'static str] {
        OUTPUT_ITEMS
    }

    fn var(&self, name: &str) -> BmiResult<&'static VarInfo> {
        vars::lookup(name).ok_or_else(|| BmiError::UnknownVar(name.to_string()))
    }

    pub fn get_var_grid(&self, name: &str) -> BmiResult<i32> {
        Ok(self.var(name)?.grid)
    }

    pub fn get_var_type(&self, name: &str) -> BmiResult<&'static str> {
        Ok(self.var(name)?.var_type.as_str())
    }

    pub fn get_var_units(&self, name: &str) -> BmiResult<&'static str> {
        Ok(self.var(name)?.units)
    }

    pub fn get_var_itemsize(&self, name: &str) -> BmiResult<i32> {
        Ok(self.var(name)?.var_type.itemsize())
    }

    /// `noahowp_var_nbytes` -- itemsize for a scalar, itemsize * grid size otherwise.
    pub fn get_var_nbytes(&self, name: &str) -> BmiResult<i32> {
        let v = self.var(name)?;
        if v.grid == 0 {
            return Ok(v.var_type.itemsize());
        }
        Ok(v.var_type.itemsize() * self.get_grid_size(v.grid)?)
    }

    pub fn get_var_location(&self, name: &str) -> &'static str {
        vars::var_location(name)
    }

    // ------------------------------------------------------------------ time

    /// `noahowp_time_units`.
    pub fn get_time_units(&self) -> &'static str {
        "s"
    }

    /// `noahowp_start_time` -- always 0.
    pub fn get_start_time(&self) -> f64 {
        0.0
    }

    /// `noahowp_end_time` = ntime * dt.
    pub fn get_end_time(&self) -> BmiResult<f64> {
        let d = &self.model()?.domain;
        Ok((d.ntime as f32 * d.dt) as f64)
    }

    pub fn get_current_time(&self) -> BmiResult<f64> {
        Ok(self.model()?.domain.time_dbl)
    }

    pub fn get_time_step(&self) -> BmiResult<f64> {
        Ok(self.model()?.domain.dt as f64)
    }

    // ------------------------------------------------------------------ grids

    pub fn get_grid_type(&self, grid: i32) -> BmiResult<&'static str> {
        vars::grid_type(grid).ok_or(BmiError::UnknownGrid(grid))
    }

    pub fn get_grid_rank(&self, grid: i32) -> BmiResult<i32> {
        vars::grid_rank(grid).ok_or(BmiError::UnknownGrid(grid))
    }

    pub fn get_grid_size(&self, grid: i32) -> BmiResult<i32> {
        let l = &self.model()?.levels;
        vars::grid_size(grid, l.nsnow, l.nsoil).ok_or(BmiError::UnknownGrid(grid))
    }

    // ------------------------------------------------------------------ values

    /// `noahowp_get_float` / `_int`. Awaits the state types.
    pub fn get_value_f32(&self, name: &str, _dest: &mut [f32]) -> BmiResult<()> {
        self.var(name)?;
        Err(BmiError::NotImplemented("get_value"))
    }

    pub fn get_value_i32(&self, name: &str, _dest: &mut [i32]) -> BmiResult<()> {
        self.var(name)?;
        Err(BmiError::NotImplemented("get_value"))
    }

    pub fn set_value_f32(&mut self, name: &str, _src: &[f32]) -> BmiResult<()> {
        self.var(name)?;
        Err(BmiError::NotImplemented("set_value"))
    }

    pub fn set_value_i32(&mut self, name: &str, _src: &[i32]) -> BmiResult<()> {
        self.var(name)?;
        Err(BmiError::NotImplemented("set_value"))
    }
}

/// A model instance carries no global state, so a driver can run catchments on threads.
/// See the plan, section 4 -- this is the property that lets `bmi-driver` drop its
/// subprocess + IPC worker protocol for Rust-only realizations.
#[allow(dead_code)]
fn _assert_model_is_send() {
    assert_send::<BmiNoahOwp>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const NAMELIST: &str = include_str!("../tests/namelist.input");

    fn initialized() -> (BmiNoahOwp, tempdir::TempPath) {
        let path = tempdir::write_temp(NAMELIST);
        let mut b = BmiNoahOwp::new();
        b.initialize(path.as_str()).unwrap();
        (b, path)
    }

    /// Minimal temp-file helper -- the model crate takes a path, and pulling in a dependency
    /// for three lines is not worth it.
    mod tempdir {
        use super::Write;

        pub struct TempPath(String);

        impl TempPath {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Drop for TempPath {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        pub fn write_temp(contents: &str) -> TempPath {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "noahowp-bmi-test-{}-{}.input",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(contents.as_bytes()).unwrap();
            TempPath(path.to_string_lossy().into_owned())
        }
    }

    #[test]
    fn component_name_matches_upstream() {
        let b = BmiNoahOwp::new();
        assert_eq!(b.get_component_name(), "Noah-OWP-Modular Surface Module");
    }

    #[test]
    fn item_counts_and_names() {
        let b = BmiNoahOwp::new();
        assert_eq!(b.get_input_item_count(), 8);
        assert_eq!(b.get_output_item_count(), 23);
        assert_eq!(b.get_input_var_names()[0], "SFCPRS");
        assert_eq!(b.get_input_var_names()[7], "PRCPNONC");
        assert_eq!(b.get_output_var_names()[0], "QINSUR");
        assert_eq!(b.get_output_var_names()[22], "FSH");
    }

    #[test]
    fn metadata_needs_no_initialization() {
        // The driver caches types right after initialize, but none of these touch model state.
        let b = BmiNoahOwp::new();
        assert_eq!(b.get_var_type("SFCTMP").unwrap(), "real");
        assert_eq!(b.get_var_type("ISNOW").unwrap(), "integer");
        assert_eq!(b.get_var_units("SFCPRS").unwrap(), "Pa");
        assert_eq!(b.get_var_grid("SNLIQ").unwrap(), 1);
        assert_eq!(b.get_var_itemsize("SFCTMP").unwrap(), 4);
        assert_eq!(b.get_var_location("SFCTMP"), "node");
    }

    #[test]
    fn unknown_variable_is_an_error() {
        let b = BmiNoahOwp::new();
        assert!(matches!(b.get_var_type("NOPE"), Err(BmiError::UnknownVar(_))));
        assert!(matches!(b.get_var_grid("NOPE"), Err(BmiError::UnknownVar(_))));
    }

    #[test]
    fn uninitialized_model_reports_it() {
        let b = BmiNoahOwp::new();
        assert!(!b.is_initialized());
        assert!(matches!(b.get_current_time(), Err(BmiError::NotInitialized)));
        assert!(matches!(b.get_grid_size(0), Err(BmiError::NotInitialized)));
    }

    #[test]
    fn initialize_reads_the_config() {
        let (b, _p) = initialized();
        assert!(b.is_initialized());
        assert_eq!(b.get_time_step().unwrap(), 1800.0);
        assert_eq!(b.get_start_time(), 0.0);
        assert_eq!(b.get_current_time().unwrap(), 0.0);
        assert_eq!(b.get_time_units(), "s");
    }

    #[test]
    fn end_time_is_ntime_times_dt() {
        let (b, _p) = initialized();
        // 365 days of 30-minute steps, plus the start point.
        let ntime = 365.0f32 * 48.0 + 1.0;
        assert_eq!(b.get_end_time().unwrap(), (ntime * 1800.0f32) as f64);
    }

    #[test]
    fn grid_sizes_follow_the_configured_layers() {
        let (b, _p) = initialized();
        assert_eq!(b.get_grid_size(0).unwrap(), 1); // scalar
        assert_eq!(b.get_grid_size(1).unwrap(), 3); // nsnow
        assert_eq!(b.get_grid_size(2).unwrap(), 4); // nsoil
        assert!(matches!(b.get_grid_size(3), Err(BmiError::UnknownGrid(3))));

        assert_eq!(b.get_grid_type(0).unwrap(), "scalar");
        assert_eq!(b.get_grid_type(1).unwrap(), "vector");
        assert_eq!(b.get_grid_rank(0).unwrap(), 0);
        assert_eq!(b.get_grid_rank(2).unwrap(), 1);
    }

    #[test]
    fn nbytes_accounts_for_layered_variables() {
        let (b, _p) = initialized();
        // Scalars: one item.
        assert_eq!(b.get_var_nbytes("SFCTMP").unwrap(), 4);
        assert_eq!(b.get_var_nbytes("ISNOW").unwrap(), 4);
        // SNLIQ is on the snow grid (nsnow = 3).
        assert_eq!(b.get_var_nbytes("SNLIQ").unwrap(), 4 * 3);
        // Soil-profile parameters (nsoil = 4).
        for name in ["BEXP", "DKSAT", "SMCMAX"] {
            assert_eq!(b.get_var_nbytes(name).unwrap(), 4 * 4, "{name}");
        }
    }

    #[test]
    fn finalize_clears_state() {
        let (mut b, _p) = initialized();
        b.finalize().unwrap();
        assert!(!b.is_initialized());
        assert!(matches!(b.get_current_time(), Err(BmiError::NotInitialized)));
    }

    #[test]
    fn initialize_reports_a_missing_config() {
        let mut b = BmiNoahOwp::new();
        match b.initialize("/nonexistent/namelist.input") {
            Err(BmiError::Config(_)) => {}
            other => panic!("expected a config error, got {other:?}"),
        }
    }

    #[test]
    fn update_is_not_ported_yet() {
        let (mut b, _p) = initialized();
        assert!(matches!(b.update(), Err(BmiError::NotImplemented(_))));
    }

    #[test]
    fn update_until_rejects_the_past() {
        let (mut b, _p) = initialized();
        match b.update_until(-1.0) {
            Err(BmiError::TimeInThePast { .. }) => {}
            other => panic!("expected TimeInThePast, got {other:?}"),
        }
    }

    #[test]
    fn update_until_to_the_current_time_is_a_no_op() {
        // Zero steps, so it must not reach the unported physics.
        let (mut b, _p) = initialized();
        assert!(b.update_until(0.0).is_ok());
    }

    #[test]
    fn value_access_awaits_the_state_types() {
        let (mut b, _p) = initialized();
        let mut buf = [0.0f32; 1];
        assert!(matches!(
            b.get_value_f32("SFCTMP", &mut buf),
            Err(BmiError::NotImplemented(_))
        ));
        // ...but an unknown name still fails as an unknown name.
        assert!(matches!(
            b.get_value_f32("NOPE", &mut buf),
            Err(BmiError::UnknownVar(_))
        ));
        assert!(matches!(
            b.set_value_f32("SFCTMP", &[1.0]),
            Err(BmiError::NotImplemented(_))
        ));
    }

    #[test]
    fn instances_are_independent() {
        // Two models in one process, which the Fortran's module-level `save` tables prevent.
        let (a, _pa) = initialized();
        let (bb, _pb) = initialized();
        assert_eq!(a.get_grid_size(2).unwrap(), bb.get_grid_size(2).unwrap());
        assert!(a.is_initialized() && bb.is_initialized());
    }
}
