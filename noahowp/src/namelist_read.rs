//! Port of `src/NamelistRead.f90` @ 0ff055e.
//!
//! Reads the model configuration (the BMI `init_config` file). The Fortran signals every
//! problem with `write(*,*)` + `stop`; here each becomes a `ConfigError` so a calibration
//! driver can report it rather than take the process down.

use crate::error_check::is_within_bound;
use crate::fortran::namelist::{Namelist, NamelistError};
use crate::layers::Shifted;
use core::fmt;
use std::path::Path;

/// Sentinels the Fortran assigns before the read so absent entries can be detected afterwards.
pub const INTEGER_MISSING: i32 = -999999;
pub const REAL_MISSING: f32 = -999999.0;
pub const STRING_MISSING: &str = "MISSING";

#[derive(Debug)]
pub enum ConfigError {
    Io { path: String, source: std::io::Error },
    Parse(NamelistError),
    /// A required namelist entry was absent.
    MissingEntry(&'static str),
    /// A model option fell outside its documented range.
    OptionOutOfRange { name: &'static str, value: i32, lo: i32, hi: i32 },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, source } => write!(f, "cannot read namelist {path}: {source}"),
            ConfigError::Parse(e) => write!(f, "{e}"),
            ConfigError::MissingEntry(name) => {
                write!(f, "required entry {name} not found in namelist")
            }
            ConfigError::OptionOutOfRange { name, value, lo, hi } => {
                write!(f, "model options: {name} should be {lo}-{hi}, got {value}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<NamelistError> for ConfigError {
    fn from(e: NamelistError) -> Self {
        ConfigError::Parse(e)
    }
}

/// `namelist_type`.
#[derive(Debug, Clone)]
pub struct NamelistConfig {
    pub dt: f32,
    pub startdate: String,
    pub enddate: String,
    pub forcing_filename: String,
    pub output_filename: String,
    pub parameter_dir: String,
    pub noahowp_table: String,
    pub soil_table: String,
    pub general_table: String,
    pub soil_class_name: String,
    pub veg_class_name: String,
    pub lat: f32,
    pub lon: f32,
    pub terrain_slope: f32,
    pub azimuth: f32,
    pub zref: f32,
    pub rain_snow_thresh: f32,

    pub isltyp: i32,
    pub nsoil: i32,
    pub nsnow: i32,
    pub nveg: i32,
    pub soil_depth: f32,
    pub vegtyp: i32,
    pub croptype: i32,
    pub sfctyp: i32,
    pub soilcolor: i32,

    pub zsoil: Shifted<f32>,
    pub dzsnso: Shifted<f32>,
    pub sice: Shifted<f32>,
    pub sh2o: Shifted<f32>,
    pub zwt: f32,

    pub precip_phase_option: i32,
    pub runoff_option: i32,
    pub drainage_option: i32,
    pub frozen_soil_option: i32,
    pub dynamic_vic_option: i32,
    pub dynamic_veg_option: i32,
    pub snow_albedo_option: i32,
    pub radiative_transfer_option: i32,
    pub sfc_drag_coeff_option: i32,
    pub canopy_stom_resist_option: i32,
    pub crop_model_option: i32,
    pub snowsoil_temp_time_option: i32,
    pub soil_temp_boundary_option: i32,
    pub supercooled_water_option: i32,
    pub stomatal_resistance_option: i32,
    pub evap_srfc_resistance_option: i32,
    pub subsurface_option: i32,
}

/// Require that a value was supplied, mirroring the Fortran's
/// `if (x /= missing) then ... else; write(...); stop; end if`.
fn require_real(v: f32, name: &'static str) -> Result<f32, ConfigError> {
    if v != REAL_MISSING {
        Ok(v)
    } else {
        Err(ConfigError::MissingEntry(name))
    }
}

fn require_int(v: i32, name: &'static str) -> Result<i32, ConfigError> {
    if v != INTEGER_MISSING {
        Ok(v)
    } else {
        Err(ConfigError::MissingEntry(name))
    }
}

fn require_string(v: String, name: &'static str) -> Result<String, ConfigError> {
    if v != STRING_MISSING {
        Ok(v)
    } else {
        Err(ConfigError::MissingEntry(name))
    }
}

fn check_option(name: &'static str, value: i32, lo: i32, hi: i32) -> Result<(), ConfigError> {
    if is_within_bound(value, lo, hi) {
        Ok(())
    } else {
        Err(ConfigError::OptionOutOfRange { name, value, lo, hi })
    }
}

impl NamelistConfig {
    /// `ReadNamelist`.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::parse(&contents)
    }

    /// `ReadNamelist` on already-loaded text, so callers can configure from memory.
    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        let nml = Namelist::parse(contents)?;

        // Every local starts at its missing sentinel, so an absent key is detectable below.
        let mut dt = REAL_MISSING;
        let mut startdate = STRING_MISSING.to_string();
        let mut enddate = STRING_MISSING.to_string();
        let mut forcing_filename = STRING_MISSING.to_string();
        let mut output_filename = STRING_MISSING.to_string();
        let mut parameter_dir = STRING_MISSING.to_string();
        let mut soil_table = STRING_MISSING.to_string();
        let mut veg_class_name = STRING_MISSING.to_string();
        let mut general_table = STRING_MISSING.to_string();
        let mut noahowp_table = STRING_MISSING.to_string();
        let mut soil_class_name = STRING_MISSING.to_string();
        let mut lat = REAL_MISSING;
        let mut lon = REAL_MISSING;
        let mut terrain_slope = REAL_MISSING;
        let mut azimuth = REAL_MISSING;
        let mut zref = REAL_MISSING;
        let mut rain_snow_thresh = REAL_MISSING;

        let mut isltyp = INTEGER_MISSING;
        let mut nsoil = INTEGER_MISSING;
        let mut nsnow = INTEGER_MISSING;
        let mut nveg = INTEGER_MISSING;
        let mut vegtyp = INTEGER_MISSING;
        let mut croptype = INTEGER_MISSING;
        let mut sfctyp = INTEGER_MISSING;
        let mut soilcolor = INTEGER_MISSING;
        let mut zwt = REAL_MISSING;

        let mut precip_phase_option = INTEGER_MISSING;
        let mut runoff_option = INTEGER_MISSING;
        let mut drainage_option = INTEGER_MISSING;
        let mut frozen_soil_option = INTEGER_MISSING;
        let mut dynamic_vic_option = INTEGER_MISSING;
        let mut dynamic_veg_option = INTEGER_MISSING;
        let mut snow_albedo_option = INTEGER_MISSING;
        let mut radiative_transfer_option = INTEGER_MISSING;
        let mut sfc_drag_coeff_option = INTEGER_MISSING;
        let mut canopy_stom_resist_option = INTEGER_MISSING;
        let mut crop_model_option = INTEGER_MISSING;
        let mut snowsoil_temp_time_option = INTEGER_MISSING;
        let mut soil_temp_boundary_option = INTEGER_MISSING;
        let mut supercooled_water_option = INTEGER_MISSING;
        let mut stomatal_resistance_option = INTEGER_MISSING;
        let mut evap_srfc_resistance_option = INTEGER_MISSING;
        let mut subsurface_option = INTEGER_MISSING;

        // &timing
        let g = nml.group_or_empty("timing");
        g.get_real("dt", &mut dt)?;
        g.get_string("startdate", &mut startdate)?;
        g.get_string("enddate", &mut enddate)?;
        g.get_string("forcing_filename", &mut forcing_filename)?;
        g.get_string("output_filename", &mut output_filename)?;

        // &parameters
        let g = nml.group_or_empty("parameters");
        g.get_string("parameter_dir", &mut parameter_dir)?;
        g.get_string("soil_table", &mut soil_table)?;
        g.get_string("general_table", &mut general_table)?;
        g.get_string("noahowp_table", &mut noahowp_table)?;
        g.get_string("soil_class_name", &mut soil_class_name)?;
        g.get_string("veg_class_name", &mut veg_class_name)?;

        // &location
        let g = nml.group_or_empty("location");
        g.get_real("lat", &mut lat)?;
        g.get_real("lon", &mut lon)?;
        g.get_real("terrain_slope", &mut terrain_slope)?;
        g.get_real("azimuth", &mut azimuth)?;

        // &forcing
        let g = nml.group_or_empty("forcing");
        g.get_real("ZREF", &mut zref)?;
        g.get_real("rain_snow_thresh", &mut rain_snow_thresh)?;

        // &model_options
        let g = nml.group_or_empty("model_options");
        g.get_int("precip_phase_option", &mut precip_phase_option)?;
        g.get_int("runoff_option", &mut runoff_option)?;
        g.get_int("drainage_option", &mut drainage_option)?;
        g.get_int("frozen_soil_option", &mut frozen_soil_option)?;
        g.get_int("dynamic_vic_option", &mut dynamic_vic_option)?;
        g.get_int("dynamic_veg_option", &mut dynamic_veg_option)?;
        g.get_int("snow_albedo_option", &mut snow_albedo_option)?;
        g.get_int("radiative_transfer_option", &mut radiative_transfer_option)?;
        g.get_int("sfc_drag_coeff_option", &mut sfc_drag_coeff_option)?;
        g.get_int("canopy_stom_resist_option", &mut canopy_stom_resist_option)?;
        g.get_int("crop_model_option", &mut crop_model_option)?;
        g.get_int("snowsoil_temp_time_option", &mut snowsoil_temp_time_option)?;
        g.get_int("soil_temp_boundary_option", &mut soil_temp_boundary_option)?;
        g.get_int("supercooled_water_option", &mut supercooled_water_option)?;
        g.get_int("stomatal_resistance_option", &mut stomatal_resistance_option)?;
        g.get_int("evap_srfc_resistance_option", &mut evap_srfc_resistance_option)?;
        g.get_int("subsurface_option", &mut subsurface_option)?;

        // &structure
        let g = nml.group_or_empty("structure");
        g.get_int("isltyp", &mut isltyp)?;
        g.get_int("nsoil", &mut nsoil)?;
        g.get_int("nsnow", &mut nsnow)?;
        g.get_int("nveg", &mut nveg)?;
        g.get_int("vegtyp", &mut vegtyp)?;
        g.get_int("croptype", &mut croptype)?;
        g.get_int("sfctyp", &mut sfctyp)?;
        g.get_int("soilcolor", &mut soilcolor)?;

        // Model option validity, checked before the arrays are sized -- same order as the Fortran.
        check_option("precip_phase_option", precip_phase_option, 1, 7)?;
        check_option("runoff_option", runoff_option, 1, 8)?;
        check_option("drainage_option", drainage_option, 1, 8)?;
        check_option("frozen_soil_option", frozen_soil_option, 1, 2)?;
        check_option("dynamic_vic_option", dynamic_vic_option, 1, 3)?;
        check_option("dynamic_veg_option", dynamic_veg_option, 1, 9)?;
        check_option("snow_albedo_option", snow_albedo_option, 1, 2)?;
        check_option("radiative_transfer_option", radiative_transfer_option, 1, 3)?;
        check_option("sfc_drag_coeff_option", sfc_drag_coeff_option, 1, 2)?;
        check_option("canopy_stom_resist_option", canopy_stom_resist_option, 1, 2)?;
        check_option("snowsoil_temp_time_option", snowsoil_temp_time_option, 1, 3)?;
        check_option("soil_temp_boundary_option", soil_temp_boundary_option, 1, 2)?;
        check_option("supercooled_water_option", supercooled_water_option, 1, 2)?;
        check_option("stomatal_resistance_option", stomatal_resistance_option, 1, 4)?;
        check_option("evap_srfc_resistance_option", evap_srfc_resistance_option, 1, 5)?;
        check_option("subsurface_option", subsurface_option, 1, 3)?;

        // Arrays can only be sized once nsoil/nsnow are known.
        let nsoil_v = require_int(nsoil, "nsoil")?;
        let nsnow_v = require_int(nsnow, "nsnow")?;

        let mut zsoil = Shifted::ones(nsoil_v, 0.0f32);
        let mut dzsnso = Shifted::snow_soil(nsnow_v, nsoil_v, 0.0f32);
        let mut sice = Shifted::ones(nsoil_v, 0.0f32);
        let mut sh2o = Shifted::ones(nsoil_v, 0.0f32);

        // Pre-assign the sentinels the Fortran uses to detect absent entries.
        sice[1] = REAL_MISSING;
        dzsnso[1] = REAL_MISSING;
        sh2o[1] = REAL_MISSING;

        // &initial_values
        let g = nml.group_or_empty("initial_values");
        g.get_real_array("dzsnso", dzsnso.as_mut_slice())?;
        g.get_real_array("sice", sice.as_mut_slice())?;
        g.get_real_array("sh2o", sh2o.as_mut_slice())?;
        g.get_real("zwt", &mut zwt)?;

        // Total soil depth and depth of layer-bottom from the surface.
        if dzsnso[1] == REAL_MISSING {
            return Err(ConfigError::MissingEntry("dzsnso"));
        }
        // soil_depth = sum(dzsnso(1:nsoil))
        let mut soil_depth = 0.0f32;
        for iz in 1..=nsoil_v {
            soil_depth += dzsnso[iz];
        }
        // zsoil(iz) = -1. * sum(dzsnso(1:iz))
        for iz in 1..=nsoil_v {
            let mut partial = 0.0f32;
            for jz in 1..=iz {
                partial += dzsnso[jz];
            }
            zsoil[iz] = -1.0f32 * partial;
        }

        Ok(Self {
            dt: require_real(dt, "dt")?,
            startdate: require_string(startdate, "startdate")?,
            enddate: require_string(enddate, "enddate")?,
            forcing_filename: require_string(forcing_filename, "forcing_filename")?,
            output_filename: require_string(output_filename, "output_filename")?,
            parameter_dir: require_string(parameter_dir, "parameter_dir")?,
            soil_table: require_string(soil_table, "soil_table")?,
            general_table: require_string(general_table, "general_table")?,
            noahowp_table: require_string(noahowp_table, "noahowp_table")?,
            soil_class_name: require_string(soil_class_name, "soil_class_name")?,
            veg_class_name: require_string(veg_class_name, "veg_class_name")?,

            lat: require_real(lat, "lat")?,
            lon: require_real(lon, "lon")?,
            terrain_slope: require_real(terrain_slope, "terrain_slope")?,
            azimuth: require_real(azimuth, "azimuth")?,
            zref: require_real(zref, "ZREF")?,
            rain_snow_thresh: require_real(rain_snow_thresh, "rain_snow_thresh")?,

            isltyp: require_int(isltyp, "isltyp")?,
            nsoil: nsoil_v,
            nsnow: nsnow_v,
            nveg: require_int(nveg, "nveg")?,
            soil_depth,
            vegtyp: require_int(vegtyp, "vegtyp")?,
            croptype: require_int(croptype, "croptype")?,
            sfctyp: require_int(sfctyp, "sfctyp")?,
            soilcolor: require_int(soilcolor, "soilcolor")?,

            zsoil,
            sice: {
                if sice[1] == REAL_MISSING {
                    return Err(ConfigError::MissingEntry("sice"));
                }
                sice
            },
            sh2o: {
                if sh2o[1] == REAL_MISSING {
                    return Err(ConfigError::MissingEntry("sh2o"));
                }
                sh2o
            },
            dzsnso,
            zwt: require_real(zwt, "zwt")?,

            precip_phase_option,
            runoff_option,
            drainage_option,
            frozen_soil_option,
            dynamic_vic_option,
            dynamic_veg_option,
            snow_albedo_option,
            radiative_transfer_option,
            sfc_drag_coeff_option,
            canopy_stom_resist_option,
            crop_model_option: require_int(crop_model_option, "crop_model_option")?,
            snowsoil_temp_time_option,
            soil_temp_boundary_option,
            supercooled_water_option,
            stomatal_resistance_option,
            evap_srfc_resistance_option,
            subsurface_option,
        })
    }
}

/// The configuration shipped as `run/namelist.input`, for tests across the crate.
#[cfg(test)]
pub(crate) mod tests_support {
    /// Verbatim content of the upstream `run/namelist.input` @ 0ff055e, comments removed.
    pub const SHIPPED: &str = r#"
&timing
  dt = 1800.0
  startdate = "199801010630"
  enddate = "199901010630"
  forcing_filename = "../data/bondville.dat"
  output_filename = "../data/output.nc"
/
&parameters
  parameter_dir = "../parameters/"
  general_table = "GENPARM.TBL"
  soil_table = "SOILPARM.TBL"
  noahowp_table = "MPTABLE.TBL"
  soil_class_name = "STAS"
  veg_class_name = "MODIFIED_IGBP_MODIS_NOAH"
/
&location
  lat = 40.01
  lon = -88.37
  terrain_slope = 0.0
  azimuth = 0.0
/
&forcing
  ZREF = 10.0
  rain_snow_thresh = 1.0
/
&model_options
  precip_phase_option = 1
  snow_albedo_option = 1
  dynamic_veg_option = 1
  runoff_option = 8
  drainage_option = 8
  frozen_soil_option = 1
  dynamic_vic_option = 1
  radiative_transfer_option = 3
  sfc_drag_coeff_option = 1
  canopy_stom_resist_option = 1
  crop_model_option = 0
  snowsoil_temp_time_option = 3
  soil_temp_boundary_option = 2
  supercooled_water_option = 1
  stomatal_resistance_option = 1
  evap_srfc_resistance_option = 1
  subsurface_option = 1
/
&structure
 isltyp = 1
 nsoil = 4
 nsnow = 3
 nveg = 20
 vegtyp = 1
 croptype = 0
 sfctyp = 1
 soilcolor = 4
/
&initial_values
 dzsnso = 0.0, 0.0, 0.0, 0.1, 0.3, 0.6, 1.0
 sice = 0.0, 0.0, 0.0, 0.0
 sh2o = 0.3, 0.3, 0.3, 0.3
 zwt = -2.0
/
"#;
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::tests_support::SHIPPED as GOOD;

    #[test]
    fn reads_the_shipped_configuration() {
        let c = NamelistConfig::parse(GOOD).unwrap();
        assert_eq!(c.dt, 1800.0);
        assert_eq!(c.startdate, "199801010630");
        assert_eq!(c.nsoil, 4);
        assert_eq!(c.nsnow, 3);
        assert_eq!(c.vegtyp, 1);
        assert_eq!(c.runoff_option, 8);
        assert_eq!(c.soil_class_name, "STAS");
    }

    #[test]
    fn dzsnso_lands_on_the_right_indices() {
        // Allocated (-nsnow+1 : nsoil) = (-2 : 4); values map in order.
        let c = NamelistConfig::parse(GOOD).unwrap();
        assert_eq!(c.dzsnso.lo(), -2);
        assert_eq!(c.dzsnso.hi(), 4);
        assert_eq!(c.dzsnso[-2], 0.0);
        assert_eq!(c.dzsnso[0], 0.0);
        assert_eq!(c.dzsnso[1], 0.1);
        assert_eq!(c.dzsnso[4], 1.0);
    }

    #[test]
    fn derives_soil_depth_and_zsoil() {
        let c = NamelistConfig::parse(GOOD).unwrap();
        // sum(dzsnso(1:nsoil)) = 0.1 + 0.3 + 0.6 + 1.0
        let expect_depth = 0.1f32 + 0.3f32 + 0.6f32 + 1.0f32;
        assert_eq!(c.soil_depth.to_bits(), expect_depth.to_bits());

        // zsoil(iz) = -1. * sum(dzsnso(1:iz)), computed in the same order and precision.
        assert_eq!(c.zsoil[1].to_bits(), (-1.0f32 * 0.1f32).to_bits());
        assert_eq!(c.zsoil[2].to_bits(), (-1.0f32 * (0.1f32 + 0.3f32)).to_bits());
        assert_eq!(
            c.zsoil[3].to_bits(),
            (-1.0f32 * (0.1f32 + 0.3f32 + 0.6f32)).to_bits()
        );
        assert_eq!(
            c.zsoil[4].to_bits(),
            (-1.0f32 * (0.1f32 + 0.3f32 + 0.6f32 + 1.0f32)).to_bits()
        );
    }

    #[test]
    fn missing_required_entry_is_reported_by_name() {
        let src = GOOD.replace("  dt = 1800.0\n", "");
        match NamelistConfig::parse(&src) {
            Err(ConfigError::MissingEntry("dt")) => {}
            other => panic!("expected MissingEntry(dt), got {other:?}"),
        }
    }

    #[test]
    fn missing_dzsnso_is_reported() {
        let src = GOOD.replace(" dzsnso = 0.0, 0.0, 0.0, 0.1, 0.3, 0.6, 1.0\n", "");
        match NamelistConfig::parse(&src) {
            Err(ConfigError::MissingEntry("dzsnso")) => {}
            other => panic!("expected MissingEntry(dzsnso), got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_option_is_rejected() {
        let src = GOOD.replace("  runoff_option = 8", "  runoff_option = 9");
        match NamelistConfig::parse(&src) {
            Err(ConfigError::OptionOutOfRange { name: "runoff_option", value: 9, .. }) => {}
            other => panic!("expected OptionOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn every_option_range_is_enforced() {
        // One probe per option: a value just past the documented upper bound.
        for (line, bad) in [
            ("  precip_phase_option = 1", "  precip_phase_option = 8"),
            ("  snow_albedo_option = 1", "  snow_albedo_option = 3"),
            ("  dynamic_veg_option = 1", "  dynamic_veg_option = 10"),
            ("  drainage_option = 8", "  drainage_option = 9"),
            ("  frozen_soil_option = 1", "  frozen_soil_option = 3"),
            ("  dynamic_vic_option = 1", "  dynamic_vic_option = 4"),
            ("  radiative_transfer_option = 3", "  radiative_transfer_option = 4"),
            ("  sfc_drag_coeff_option = 1", "  sfc_drag_coeff_option = 3"),
            ("  canopy_stom_resist_option = 1", "  canopy_stom_resist_option = 3"),
            ("  snowsoil_temp_time_option = 3", "  snowsoil_temp_time_option = 4"),
            ("  soil_temp_boundary_option = 2", "  soil_temp_boundary_option = 3"),
            ("  supercooled_water_option = 1", "  supercooled_water_option = 3"),
            ("  stomatal_resistance_option = 1", "  stomatal_resistance_option = 5"),
            ("  evap_srfc_resistance_option = 1", "  evap_srfc_resistance_option = 6"),
            ("  subsurface_option = 1", "  subsurface_option = 4"),
        ] {
            let src = GOOD.replace(line, bad);
            assert!(
                matches!(
                    NamelistConfig::parse(&src),
                    Err(ConfigError::OptionOutOfRange { .. })
                ),
                "{bad} should have been rejected"
            );
        }
    }
}
