//! Port of `bmi/bmi_noahowp.f90` @ 0ff055e.
//!
//! Scope is what `bmi-driver` actually calls -- see `docs/design.md` section 1.
//! `get_value_ptr`, `*_at_indices` and the extended grid functions are deliberately absent.
//! [`c_abi`] exposes this as a C BMI through `register_bmi`.

pub mod c_abi;
pub mod vars;

use noahowp::parameters_read::Tables;
use noahowp::physics::atm_processing::PrecipInput;
use noahowp::run::{NoahOwp, StepError};
use noahowp::{ConfigError, NamelistConfig, Shifted};
use std::path::Path;

pub use vars::{VarInfo, VarType, COMPONENT_NAME, INPUT_ITEMS, OUTPUT_ITEMS, VARS};

#[derive(Debug)]
pub enum BmiError {
    Config(ConfigError),
    Date(noahowp::date_time_utils::DateError),
    /// A timestep failed where upstream would `stop`.
    Step(StepError),
    /// The model has not been initialized.
    NotInitialized,
    /// No such exchange item.
    UnknownVar(String),
    /// The name exists, but not for this operation -- e.g. `set_value` on an output only
    /// `get_value` accepts, or the wrong numeric type.
    Unsupported { op: &'static str, name: String },
    /// No such grid id.
    UnknownGrid(i32),
    /// `update_until` was given a time before the current one.
    TimeInThePast { requested: f64, current: f64 },
}

impl std::fmt::Display for BmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BmiError::Config(e) => write!(f, "{e}"),
            BmiError::Date(e) => write!(f, "{e}"),
            BmiError::Step(e) => write!(f, "{e}"),
            BmiError::NotInitialized => write!(f, "model is not initialized"),
            BmiError::UnknownVar(n) => write!(f, "unknown variable {n:?}"),
            BmiError::Unsupported { op, name } => write!(f, "{op} does not accept {name:?}"),
            BmiError::UnknownGrid(g) => write!(f, "unknown grid {g}"),
            BmiError::TimeInThePast { requested, current } => {
                write!(f, "update_until({requested}) is before the current time {current}")
            }
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

impl From<StepError> for BmiError {
    fn from(e: StepError) -> Self {
        BmiError::Step(e)
    }
}

pub type BmiResult<T> = Result<T, BmiError>;

/// `bmi_noahowp`.
///
/// Holds no globals, so instances are independent and `Send` -- which is what lets a driver run
/// many catchments as threads rather than as processes. See `docs/design.md`, section 7.
#[derive(Debug, Default)]
pub struct BmiNoahOwp {
    model: Option<Box<NoahOwp>>,
}

fn assert_send<T: Send>() {}

/// `dest = [x]` for a scalar.
fn put(dest: &mut [f32], x: f32) {
    if let Some(d) = dest.first_mut() {
        *d = x;
    }
}

/// `dest = [array]`.
fn put_all(dest: &mut [f32], src: &Shifted<f32>) {
    for (d, s) in dest.iter_mut().zip(src.iter()) {
        *d = *s;
    }
}

/// `array(:) = src(:)`.
fn take_all(dst: &mut Shifted<f32>, src: &[f32]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = *s;
    }
}

impl BmiNoahOwp {
    pub fn new() -> Self {
        Self::default()
    }

    fn model(&self) -> BmiResult<&NoahOwp> {
        self.model.as_deref().ok_or(BmiError::NotInitialized)
    }

    fn model_mut(&mut self) -> BmiResult<&mut NoahOwp> {
        self.model.as_deref_mut().ok_or(BmiError::NotInitialized)
    }

    pub fn is_initialized(&self) -> bool {
        self.model.is_some()
    }

    /// The wrapped model, for callers that want the state directly.
    pub fn noahowp(&self) -> Option<&NoahOwp> {
        self.model.as_deref()
    }

    // ------------------------------------------------------------------ lifecycle

    /// `noahowp_initialize` -> `initialize_from_file`.
    pub fn initialize(&mut self, config_file: &str) -> BmiResult<()> {
        let namelist = NamelistConfig::read(Path::new(config_file))?;
        let tables = Tables::read(&namelist)?;
        self.model = Some(Box::new(NoahOwp::new(namelist, &tables)?));
        Ok(())
    }

    /// `noahowp_update` -> `advance_in_time`.
    ///
    /// The shipping library is built with `NGEN_FORCING_ACTIVE`, so precipitation arrives as
    /// `PRCPNONC` through `set_value`.
    pub fn update(&mut self) -> BmiResult<()> {
        self.model_mut()?.advance_in_time(PrecipInput::NonConvective)?;
        Ok(())
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

    /// `noahowp_get_float`.
    ///
    /// Three names are not a plain copy: `ECAN` and `ETRAN` are rates scaled to a depth per
    /// timestep, and `QSEVA` is m/s scaled to mm/s. The factors and operation order are the
    /// Fortran's -- see `set_value_f32` for why that matters.
    pub fn get_value_f32(&self, name: &str, dest: &mut [f32]) -> BmiResult<()> {
        self.var(name)?;
        let m = self.model()?;
        let (f, w, e, d, p) = (&m.forcing, &m.water, &m.energy, &m.domain, &m.parameters);
        let m2mm: f32 = 1000.; // unit conversion m to mm
        match name {
            "ACSNOM" => put(dest, w.ACSNOM),
            "AXAJ" => put(dest, p.AXAJ),
            "BEXP" => put_all(dest, &p.bexp),
            "BXAJ" => put(dest, p.BXAJ),
            "CMC" => put(dest, w.CMC),
            "CWP" => put(dest, p.CWP),
            "DKSAT" => put_all(dest, &p.dksat),
            "ECAN" => put(dest, w.ECAN * d.dt),
            "ETRAN" => put(dest, w.ETRAN * d.dt),
            "EVAPOTRANS" => put(dest, w.EVAPOTRANS),
            "FIRA" => put(dest, e.FIRA),
            "FRZX" => put(dest, p.frzx),
            "FSA" => put(dest, e.FSA),
            "FSH" => put(dest, e.FSH),
            "FSNO" => put(dest, w.FSNO),
            "GH" => put(dest, e.GH),
            "HVT" => put(dest, p.HVT),
            "KDT" => put(dest, p.kdt),
            "LH" => put(dest, e.LH),
            "LWDN" => put(dest, f.LWDN),
            "MFSNO" => put(dest, p.MFSNO),
            "MP" => put(dest, p.MP),
            "PRCPNONC" => put(dest, f.PRCPNONC),
            "Q2" => put(dest, f.Q2),
            "QINSUR" => put(dest, w.qinsur),
            "QRAIN" => put(dest, w.QRAIN),
            "QSEVA" => put(dest, w.qseva * m2mm),
            "QSNOW" => put(dest, w.QSNOW),
            "REFKDT" => put(dest, p.refkdt),
            "RSURF_EXP" => put(dest, p.RSURF_EXP),
            "RSURF_SNOW" => put(dest, p.RSURF_SNOW),
            "SCAMAX" => put(dest, p.SCAMAX),
            "SFCPRS" => put(dest, f.SFCPRS),
            "SFCTMP" => put(dest, f.SFCTMP),
            "SLOPE" => put(dest, p.slope),
            "SMCMAX" => put_all(dest, &p.smcmax),
            "SNEQV" => put(dest, w.SNEQV),
            "SNLIQ" => put_all(dest, &w.SNLIQ),
            "SNOWH" => put(dest, w.SNOWH),
            "SNOWT_AVG" => put(dest, e.SNOWT_AVG),
            "SOLDN" => put(dest, f.SOLDN),
            "TG" => put(dest, e.TG),
            "TGS" => put(dest, e.TGS),
            "TRAD" => put(dest, e.TRAD),
            "UU" => put(dest, f.UU),
            "VCMX25" => put(dest, p.VCMX25),
            "VV" => put(dest, f.VV),
            "XXAJ" => put(dest, p.XXAJ),
            _ => {
                // ISNOW: an integer, so not reachable through get_float upstream either.
                dest.fill(-1.0);
                return Err(BmiError::Unsupported { op: "get_value (real)", name: name.into() });
            }
        }
        Ok(())
    }

    /// `noahowp_get_int` -- only `ISNOW`.
    pub fn get_value_i32(&self, name: &str, dest: &mut [i32]) -> BmiResult<()> {
        self.var(name)?;
        let m = self.model()?;
        match name {
            "ISNOW" => dest.fill(m.water.ISNOW),
            _ => {
                dest.fill(-1);
                return Err(BmiError::Unsupported { op: "get_value (integer)", name: name.into() });
            }
        }
        Ok(())
    }

    /// `noahowp_set_float`.
    ///
    /// Not symmetric with `get_value_f32`: fewer names are settable, and the unit conversions
    /// are written as the Fortran writes them -- `QSEVA` comes in as `src * 0.001`, which in
    /// binary32 is not `src / 1000`.
    ///
    /// Three names recompute a secondary parameter inline, exactly as upstream does:
    /// `DKSAT` and `REFKDT` recompute `kdt`, `SMCMAX` recomputes `frzx`. The rest do not --
    /// in particular setting `KDT` or `FRZX` directly is overwritten by a later `DKSAT`,
    /// `REFKDT` or `SMCMAX`, and setting `REFDK` is not possible at all.
    pub fn set_value_f32(&mut self, name: &str, src: &[f32]) -> BmiResult<()> {
        self.var(name)?;
        let m = self.model_mut()?;
        let (f, w, e, d, p) =
            (&mut m.forcing, &mut m.water, &mut m.energy, &m.domain, &mut m.parameters);
        let mm2m: f32 = 0.001; // unit conversion mm to m
        let Some(&s) = src.first() else {
            return Err(BmiError::Unsupported { op: "set_value (empty)", name: name.into() });
        };
        match name {
            "AXAJ" => p.AXAJ = s,
            "BEXP" => take_all(&mut p.bexp, src),
            "BXAJ" => p.BXAJ = s,
            "CWP" => p.CWP = s,
            "DKSAT" => {
                take_all(&mut p.dksat, src);
                p.kdt = p.refkdt * p.dksat[1] / p.refdk;
            }
            "ETRAN" => w.ETRAN = s / d.dt,
            "EVAPOTRANS" => w.EVAPOTRANS = s,
            "FRZX" => p.frzx = s,
            "HVT" => p.HVT = s,
            "KDT" => p.kdt = s,
            "LWDN" => f.LWDN = s,
            "MFSNO" => p.MFSNO = s,
            "MP" => p.MP = s,
            "PRCPNONC" => f.PRCPNONC = s,
            "Q2" => f.Q2 = s,
            "QINSUR" => w.qinsur = s,
            "QSEVA" => w.qseva = s * mm2m,
            "REFKDT" => {
                p.refkdt = s;
                p.kdt = p.refkdt * p.dksat[1] / p.refdk;
            }
            "RSURF_EXP" => p.RSURF_EXP = s,
            "RSURF_SNOW" => p.RSURF_SNOW = s,
            "SCAMAX" => p.SCAMAX = s,
            "SFCPRS" => f.SFCPRS = s,
            "SFCTMP" => f.SFCTMP = s,
            "SLOPE" => p.slope = s,
            "SMCMAX" => {
                take_all(&mut p.smcmax, src);
                p.frzx = 0.15 * (p.smcmax[1] / p.smcref[1]) * (0.412 / 0.468);
            }
            "SNEQV" => w.SNEQV = s,
            "SOLDN" => f.SOLDN = s,
            "TG" => e.TG = s,
            "TGS" => e.TGS = s,
            "UU" => f.UU = s,
            "VCMX25" => p.VCMX25 = s,
            "VV" => f.VV = s,
            "XXAJ" => p.XXAJ = s,
            _ => return Err(BmiError::Unsupported { op: "set_value (real)", name: name.into() }),
        }
        Ok(())
    }

    /// `noahowp_set_int` -- accepts nothing upstream.
    pub fn set_value_i32(&mut self, name: &str, _src: &[i32]) -> BmiResult<()> {
        self.var(name)?;
        self.model()?;
        Err(BmiError::Unsupported { op: "set_value (integer)", name: name.into() })
    }
}

/// A model instance carries no global state, so a driver can run catchments on threads.
/// See `docs/design.md`, section 7 -- this is the property that lets `bmi-driver` drop its
/// subprocess + IPC worker protocol for Rust-only realizations.
#[allow(dead_code)]
fn _assert_model_is_send() {
    assert_send::<BmiNoahOwp>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const NAMELIST: &str = include_str!("../../noahowp/tests/fixtures/namelist.input");

    /// The Bondville namelist, pointed at the verbatim upstream tables the model crate tests
    /// against.
    fn namelist() -> String {
        let tables = concat!(env!("CARGO_MANIFEST_DIR"), "/../noahowp/tests/fixtures/");
        NAMELIST.replace("\"../parameters/\"", &format!("\"{tables}\""))
    }

    fn initialized() -> (BmiNoahOwp, tempdir::TempPath) {
        let path = tempdir::write_temp(&namelist());
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

    /// Bondville-ish forcing, so a step has something physical to work with.
    fn force(b: &mut BmiNoahOwp) {
        for (name, v) in [
            ("SFCPRS", 101_325.0),
            ("SFCTMP", 285.0),
            ("SOLDN", 400.0),
            ("LWDN", 300.0),
            ("UU", 2.0),
            ("VV", 1.0),
            ("Q2", 0.006),
            ("PRCPNONC", 0.0005),
        ] {
            b.set_value_f32(name, &[v]).unwrap();
        }
    }

    #[test]
    fn update_advances_time_and_produces_output() {
        let (mut b, _p) = initialized();
        force(&mut b);
        b.update().unwrap();
        assert_eq!(b.get_current_time().unwrap(), 1800.0);
        b.update_until(1800.0 * 5.0).unwrap();
        assert_eq!(b.get_current_time().unwrap(), 1800.0 * 5.0);

        for name in OUTPUT_ITEMS.iter().filter(|n| **n != "ISNOW") {
            let n = (b.get_var_nbytes(name).unwrap() / 4) as usize;
            let mut buf = vec![f32::NAN; n];
            b.get_value_f32(name, &mut buf).unwrap();
            assert!(buf.iter().all(|x| x.is_finite()), "{name} = {buf:?}");
        }
        // It rained on a warm column: water reached the surface.
        let mut q = [0.0f32];
        b.get_value_f32("QINSUR", &mut q).unwrap();
        assert!(q[0] > 0.0, "QINSUR = {}", q[0]);
        let mut isnow = [99];
        b.get_value_i32("ISNOW", &mut isnow).unwrap();
        assert_eq!(isnow, [0]);
    }

    #[test]
    fn set_then_get_round_trips_forcings() {
        let (mut b, _p) = initialized();
        force(&mut b);
        let mut buf = [0.0f32];
        b.get_value_f32("SFCTMP", &mut buf).unwrap();
        assert_eq!(buf, [285.0]);
    }

    #[test]
    fn qseva_is_scaled_both_ways_as_upstream_writes_it() {
        // In by `* 0.001`, out by `* 1000.` -- not the same as dividing.
        let (mut b, _p) = initialized();
        b.set_value_f32("QSEVA", &[3.0]).unwrap();
        assert_eq!(b.noahowp().unwrap().water.qseva, 3.0f32 * 0.001);
        let mut buf = [0.0f32];
        b.get_value_f32("QSEVA", &mut buf).unwrap();
        assert_eq!(buf, [3.0f32 * 0.001 * 1000.0]);
    }

    #[test]
    fn refkdt_and_dksat_recompute_kdt_and_smcmax_recomputes_frzx() {
        let (mut b, _p) = initialized();
        b.set_value_f32("REFKDT", &[2.0]).unwrap();
        let p = &b.noahowp().unwrap().parameters;
        assert_eq!(p.kdt, 2.0 * p.dksat[1] / p.refdk);

        b.set_value_f32("DKSAT", &[1e-5, 2e-5, 3e-5, 4e-5]).unwrap();
        let p = &b.noahowp().unwrap().parameters;
        assert_eq!(p.dksat.as_slice(), &[1e-5, 2e-5, 3e-5, 4e-5]);
        assert_eq!(p.kdt, 2.0 * 1e-5f32 / p.refdk);

        b.set_value_f32("SMCMAX", &[0.4; 4]).unwrap();
        let p = &b.noahowp().unwrap().parameters;
        assert_eq!(p.frzx, 0.15 * (0.4 / p.smcref[1]) * (0.412 / 0.468));

        // KDT itself is settable, and does not recompute anything.
        b.set_value_f32("KDT", &[7.0]).unwrap();
        assert_eq!(b.noahowp().unwrap().parameters.kdt, 7.0);
    }

    #[test]
    fn etran_and_ecan_are_depths_per_timestep() {
        let (mut b, _p) = initialized();
        b.set_value_f32("ETRAN", &[1.0]).unwrap();
        assert_eq!(b.noahowp().unwrap().water.ETRAN, 1.0 / 1800.0);
        let mut buf = [0.0f32];
        b.get_value_f32("ETRAN", &mut buf).unwrap();
        assert_eq!(buf, [(1.0f32 / 1800.0) * 1800.0]);
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
        let (mut b, _p) = initialized();
        assert!(b.update_until(0.0).is_ok());
        assert_eq!(b.get_current_time().unwrap(), 0.0);
    }

    #[test]
    fn value_access_rejects_what_upstream_rejects() {
        let (mut b, _p) = initialized();
        let mut buf = [0.0f32; 1];
        assert!(matches!(b.get_value_f32("NOPE", &mut buf), Err(BmiError::UnknownVar(_))));
        // Outputs are readable but not writable...
        assert!(matches!(b.set_value_f32("FSH", &[1.0]), Err(BmiError::Unsupported { .. })));
        // ...ISNOW is an integer, and no integer is settable.
        assert!(matches!(b.get_value_f32("ISNOW", &mut buf), Err(BmiError::Unsupported { .. })));
        assert!(matches!(b.set_value_i32("ISNOW", &[1]), Err(BmiError::Unsupported { .. })));
    }

    #[test]
    fn value_access_needs_initialization() {
        let mut b = BmiNoahOwp::new();
        let mut buf = [0.0f32; 1];
        assert!(matches!(b.get_value_f32("SFCTMP", &mut buf), Err(BmiError::NotInitialized)));
        assert!(matches!(b.set_value_f32("SFCTMP", &[1.0]), Err(BmiError::NotInitialized)));
        assert!(matches!(b.update(), Err(BmiError::NotInitialized)));
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
