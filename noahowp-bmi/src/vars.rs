//! The BMI exchange-item registry, from `bmi/bmi_noahowp.f90` @ 0ff055e.
//!
//! Upstream spreads this across four `select case` blocks (`noahowp_var_type`,
//! `noahowp_var_units`, `noahowp_var_grid`, `noahowp_var_itemsize`) that must agree with each
//! other. One table keeps them consistent by construction, and a test asserts the table covers
//! exactly the names the Fortran accepts.

/// `noahowp_var_type` -- the two types Noah-OWP exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Real,
    Integer,
}

impl VarType {
    /// The BMI type string.
    pub fn as_str(self) -> &'static str {
        match self {
            VarType::Real => "real",
            VarType::Integer => "integer",
        }
    }

    /// `noahowp_var_itemsize` -- `sizeof` of the underlying Fortran scalar.
    ///
    /// Both are 4 bytes: the build does not set `-fdefault-real-8`, so `real` is binary32.
    pub fn itemsize(self) -> i32 {
        match self {
            VarType::Real => 4,
            VarType::Integer => 4,
        }
    }
}

/// One exchange item.
#[derive(Debug, Clone, Copy)]
pub struct VarInfo {
    pub name: &'static str,
    pub var_type: VarType,
    pub units: &'static str,
    /// 0 = scalar, 1 = nsnow vector, 2 = nsoil vector.
    pub grid: i32,
}

use VarType::{Integer, Real};

/// Every name `get_value` / `set_value` accepts, in the Fortran's alphabetical order.
///
/// This is a superset of the declared exchange items: the extra names are the calibration
/// parameters that `set_value` exposes without listing them in the input/output item counts.
pub const VARS: &[VarInfo] = &[
    VarInfo { name: "ACSNOM",     var_type: Real,    units: "mm",              grid: 0 },
    VarInfo { name: "AXAJ",       var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "BEXP",       var_type: Real,    units: "unitless",        grid: 2 },
    VarInfo { name: "BXAJ",       var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "CMC",        var_type: Real,    units: "mm",              grid: 0 },
    VarInfo { name: "CWP",        var_type: Real,    units: "1/m",             grid: 0 },
    VarInfo { name: "DKSAT",      var_type: Real,    units: "m/s",             grid: 2 },
    VarInfo { name: "ECAN",       var_type: Real,    units: "mm",              grid: 0 },
    VarInfo { name: "ETRAN",      var_type: Real,    units: "mm",              grid: 0 },
    VarInfo { name: "EVAPOTRANS", var_type: Real,    units: "m/s",             grid: 0 },
    VarInfo { name: "FIRA",       var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "FRZX",       var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "FSA",        var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "FSH",        var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "FSNO",       var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "GH",         var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "HVT",        var_type: Real,    units: "m",               grid: 0 },
    VarInfo { name: "ISNOW",      var_type: Integer, units: "unitless",        grid: 0 },
    VarInfo { name: "KDT",        var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "LH",         var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "LWDN",       var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "MFSNO",      var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "MP",         var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "PRCPNONC",   var_type: Real,    units: "mm/s",            grid: 0 },
    VarInfo { name: "Q2",         var_type: Real,    units: "kg/kg",           grid: 0 },
    VarInfo { name: "QINSUR",     var_type: Real,    units: "m/s",             grid: 0 },
    VarInfo { name: "QRAIN",      var_type: Real,    units: "mm/s",            grid: 0 },
    VarInfo { name: "QSEVA",      var_type: Real,    units: "mm/s",            grid: 0 },
    VarInfo { name: "QSNOW",      var_type: Real,    units: "mm/s",            grid: 0 },
    VarInfo { name: "REFKDT",     var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "RSURF_EXP",  var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "RSURF_SNOW", var_type: Real,    units: "s/m",             grid: 0 },
    VarInfo { name: "SCAMAX",     var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "SFCPRS",     var_type: Real,    units: "Pa",              grid: 0 },
    VarInfo { name: "SFCTMP",     var_type: Real,    units: "K",               grid: 0 },
    VarInfo { name: "SLOPE",      var_type: Real,    units: "unitless",        grid: 0 },
    VarInfo { name: "SMCMAX",     var_type: Real,    units: "volumetric",      grid: 2 },
    VarInfo { name: "SNEQV",      var_type: Real,    units: "mm",              grid: 0 },
    VarInfo { name: "SNLIQ",      var_type: Real,    units: "mm",              grid: 1 },
    VarInfo { name: "SNOWH",      var_type: Real,    units: "m",               grid: 0 },
    VarInfo { name: "SNOWT_AVG",  var_type: Real,    units: "K",               grid: 0 },
    VarInfo { name: "SOLDN",      var_type: Real,    units: "W/m2",            grid: 0 },
    VarInfo { name: "TG",         var_type: Real,    units: "K",               grid: 0 },
    VarInfo { name: "TGS",        var_type: Real,    units: "K",               grid: 0 },
    VarInfo { name: "TRAD",       var_type: Real,    units: "K",               grid: 0 },
    VarInfo { name: "UU",         var_type: Real,    units: "m/s",             grid: 0 },
    VarInfo { name: "VCMX25",     var_type: Real,    units: "umol co2/m**2/s", grid: 0 },
    VarInfo { name: "VV",         var_type: Real,    units: "m/s",             grid: 0 },
    VarInfo { name: "XXAJ",       var_type: Real,    units: "unitless",        grid: 0 },
];

/// `noahowp_input_var_names` -- order is significant, the driver maps forcings by it.
pub const INPUT_ITEMS: &[&str] = &[
    "SFCPRS",   // surface pressure (Pa)
    "SFCTMP",   // surface air temperature (K)
    "SOLDN",    // incoming shortwave radiation (W/m2)
    "LWDN",     // incoming longwave radiation (W/m2)
    "UU",       // wind speed in eastward direction (m/s)
    "VV",       // wind speed in northward direction (m/s)
    "Q2",       // mixing ratio (kg/kg)
    "PRCPNONC", // precipitation rate (mm/s)
];

/// `noahowp_output_var_names`.
pub const OUTPUT_ITEMS: &[&str] = &[
    "QINSUR",     // total liquid water input to surface rate (m/s)
    "ETRAN",      // transpiration rate (mm)
    "QSEVA",      // evaporation rate (mm/s)
    "EVAPOTRANS", // evapotranspiration rate (m/s)
    "TG",         // surface/ground temperature (K)
    "SNEQV",      // snow water equivalent (mm)
    "TGS",        // ground temperature (K)
    "ACSNOM",     // accumulated meltwater from bottom snow layer (mm)
    "SNOWT_AVG",  // average snow temperature (K)
    "ISNOW",      // number of snow layers
    "QRAIN",      // rainfall rate on the ground (mm/s)
    "FSNO",       // snow-cover fraction on the ground
    "SNOWH",      // snow depth (m)
    "SNLIQ",      // snow layer liquid water (mm)
    "QSNOW",      // snowfall rate on the ground (mm/s)
    "ECAN",       // evaporation of intercepted water (mm)
    "GH",         // heat flux into the soil (W/m2)
    "TRAD",       // surface radiative temperature (K)
    "FSA",        // total absorbed SW radiation (W/m2)
    "CMC",        // total canopy water (mm)
    "LH",         // total latent heat to the atmosphere (W/m2)
    "FIRA",       // total net LW radiation to atmosphere (W/m2)
    "FSH",        // total sensible heat to the atmosphere (W/m2)
];

/// `component_name`.
pub const COMPONENT_NAME: &str = "Noah-OWP-Modular Surface Module";

/// Look up an exchange item by name. Case-sensitive, as the Fortran `select case` is.
pub fn lookup(name: &str) -> Option<&'static VarInfo> {
    VARS.iter().find(|v| v.name == name)
}

/// `noahowp_var_location`. Upstream returns "node" for *every* name, including unknown ones,
/// and always reports success.
pub fn var_location(_name: &str) -> &'static str {
    "node"
}

/// `noahowp_grid_type`.
pub fn grid_type(grid: i32) -> Option<&'static str> {
    match grid {
        0 => Some("scalar"),
        1 | 2 => Some("vector"),
        _ => None,
    }
}

/// `noahowp_grid_rank`.
pub fn grid_rank(grid: i32) -> Option<i32> {
    match grid {
        0 => Some(0),
        1 | 2 => Some(1),
        _ => None,
    }
}

/// `noahowp_grid_size`, given the layer counts.
pub fn grid_size(grid: i32, nsnow: i32, nsoil: i32) -> Option<i32> {
    match grid {
        0 => Some(1),
        1 => Some(nsnow),
        2 => Some(nsoil),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_alphabetical_and_unique() {
        for pair in VARS.windows(2) {
            assert!(
                pair[0].name < pair[1].name,
                "{} should sort before {}",
                pair[0].name,
                pair[1].name
            );
        }
    }

    #[test]
    fn registry_matches_the_fortran_name_set() {
        // The union of names accepted by noahowp_var_type: 48 reals plus ISNOW.
        const FORTRAN_NAMES: &[&str] = &[
            "ACSNOM", "AXAJ", "BEXP", "BXAJ", "CMC", "CWP", "DKSAT", "ECAN", "ETRAN",
            "EVAPOTRANS", "FIRA", "FRZX", "FSA", "FSH", "FSNO", "GH", "HVT", "ISNOW", "KDT",
            "LH", "LWDN", "MFSNO", "MP", "PRCPNONC", "Q2", "QINSUR", "QRAIN", "QSEVA", "QSNOW",
            "REFKDT", "RSURF_EXP", "RSURF_SNOW", "SCAMAX", "SFCPRS", "SFCTMP", "SLOPE",
            "SMCMAX", "SNEQV", "SNLIQ", "SNOWH", "SNOWT_AVG", "SOLDN", "TG", "TGS", "TRAD",
            "UU", "VCMX25", "VV", "XXAJ",
        ];
        assert_eq!(VARS.len(), FORTRAN_NAMES.len());
        for name in FORTRAN_NAMES {
            assert!(lookup(name).is_some(), "{name} missing from the registry");
        }
    }

    #[test]
    fn only_isnow_is_an_integer() {
        let ints: Vec<_> = VARS
            .iter()
            .filter(|v| v.var_type == VarType::Integer)
            .map(|v| v.name)
            .collect();
        assert_eq!(ints, vec!["ISNOW"]);
    }

    #[test]
    fn layered_variables_are_on_the_right_grids() {
        // Only SNLIQ is on the snow grid; only the soil-profile parameters are on grid 2.
        let g1: Vec<_> = VARS.iter().filter(|v| v.grid == 1).map(|v| v.name).collect();
        assert_eq!(g1, vec!["SNLIQ"]);

        let g2: Vec<_> = VARS.iter().filter(|v| v.grid == 2).map(|v| v.name).collect();
        assert_eq!(g2, vec!["BEXP", "DKSAT", "SMCMAX"]);

        // Everything else is scalar.
        assert_eq!(VARS.iter().filter(|v| v.grid == 0).count(), VARS.len() - 4);
    }

    #[test]
    fn declared_exchange_items_are_all_in_the_registry() {
        assert_eq!(INPUT_ITEMS.len(), 8);
        assert_eq!(OUTPUT_ITEMS.len(), 23);
        for name in INPUT_ITEMS.iter().chain(OUTPUT_ITEMS) {
            assert!(lookup(name).is_some(), "{name} not in the registry");
        }
    }

    #[test]
    fn calibration_parameters_are_settable_but_undeclared() {
        // These are reachable through set_value yet appear in neither item list -- the
        // calibration surface. Losing one would silently break a calibration run.
        const CAL: &[&str] = &[
            "BEXP", "DKSAT", "SMCMAX", "REFKDT", "KDT", "FRZX", "SLOPE", "MFSNO", "SCAMAX",
            "AXAJ", "BXAJ", "XXAJ", "CWP", "VCMX25", "MP", "HVT", "RSURF_EXP", "RSURF_SNOW",
        ];
        for name in CAL {
            assert!(lookup(name).is_some(), "{name} not in the registry");
            assert!(
                !INPUT_ITEMS.contains(name) && !OUTPUT_ITEMS.contains(name),
                "{name} is a declared exchange item, not a hidden calibration knob"
            );
        }
    }

    #[test]
    fn units_match_the_fortran() {
        for (name, want) in [
            ("SFCPRS", "Pa"),
            ("SFCTMP", "K"),
            ("SOLDN", "W/m2"),
            ("UU", "m/s"),
            ("Q2", "kg/kg"),
            ("QINSUR", "m/s"),
            ("EVAPOTRANS", "m/s"),
            ("DKSAT", "m/s"),
            ("PRCPNONC", "mm/s"),
            ("SNEQV", "mm"),
            ("ISNOW", "unitless"),
            ("SNOWH", "m"),
            ("HVT", "m"),
            ("VCMX25", "umol co2/m**2/s"),
            ("CWP", "1/m"),
            ("RSURF_SNOW", "s/m"),
            ("SMCMAX", "volumetric"),
        ] {
            assert_eq!(lookup(name).unwrap().units, want, "{name}");
        }
    }

    #[test]
    fn itemsize_is_four_bytes_for_both_types() {
        // The build does not pass -fdefault-real-8, so `real` is binary32.
        assert_eq!(VarType::Real.itemsize(), 4);
        assert_eq!(VarType::Integer.itemsize(), 4);
    }

    #[test]
    fn grid_metadata() {
        assert_eq!(grid_type(0), Some("scalar"));
        assert_eq!(grid_type(1), Some("vector"));
        assert_eq!(grid_type(2), Some("vector"));
        assert_eq!(grid_type(3), None);

        assert_eq!(grid_rank(0), Some(0));
        assert_eq!(grid_rank(1), Some(1));
        assert_eq!(grid_rank(3), None);

        assert_eq!(grid_size(0, 3, 4), Some(1));
        assert_eq!(grid_size(1, 3, 4), Some(3));
        assert_eq!(grid_size(2, 3, 4), Some(4));
        assert_eq!(grid_size(9, 3, 4), None);
    }

    #[test]
    fn unknown_names_are_rejected() {
        assert!(lookup("NOT_A_VARIABLE").is_none());
        // Case-sensitive, matching Fortran's select case on an exact string.
        assert!(lookup("sfcprs").is_none());
    }

    #[test]
    fn var_location_is_node_even_for_unknown_names() {
        assert_eq!(var_location("SFCTMP"), "node");
        assert_eq!(var_location("NOT_A_VARIABLE"), "node");
    }
}
