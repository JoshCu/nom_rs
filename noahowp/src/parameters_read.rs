//! Port of `src/ParametersRead.f90` @ 0ff055e -- the four readers `paramRead` calls.
//!
//! The Fortran keeps every `*_TABLE` in module-level `save` globals, which is why two Fortran
//! instances cannot share a process and why `bmi-driver` forks workers and pays IPC for every
//! result. Here the tables are an ordinary value: [`Tables`] is owned by the caller, so the
//! model is `Send` and a calibration driver can run instances on threads.
//!
//! Only the four readers `ParametersType::paramRead` actually calls are ported --
//! `read_veg_parameters`, `read_soil_parameters`, `read_rad_parameters` and
//! `read_global_parameters`. `read_crop_parameters`, `read_irrigation_parameters`,
//! `read_tiledrain_parameters` and `read_optional_parameters` are never called from the BMI
//! path (`crop_model_option = 0` is the only supported value), and porting ~450 lines of
//! tables that nothing can read would be dead weight. See `docs/porting.md`.
//!
//! # Sentinels
//!
//! Every table is filled with [`UNSET`] (`-1.E36`) before reading, and only entries `1..=nveg`
//! or `1..=slcats` are overwritten. Entries past that keep the sentinel, and some option paths
//! test for it -- so the fill is load-bearing, not defensive. Integer tables use
//! [`UNSET_INT`] (`-99999`) for the same reason.

use std::path::Path;

use crate::fortran::list_directed::ListDirectedReader;
use crate::fortran::namelist::{Group, Namelist};
use crate::layers::Shifted;
use crate::namelist_read::{ConfigError, NamelistConfig};

/// Number of vegetation types a table can hold (`MVT`).
pub const MVT: i32 = 27;
/// Number of shortwave bands (`MBAND`): visible and near-infrared.
pub const MBAND: i32 = 2;
/// Number of soil colour classes (`MSC`).
pub const MSC: i32 = 8;
/// Number of soil texture classes a table can hold (`MAX_SOILTYP`).
pub const MAX_SOILTYP: i32 = 30;
/// Number of slope categories `GENPARM.TBL` can supply (`SLOPE_TABLE(9)`).
pub const MAX_SLOPE: i32 = 9;

/// `-1.E36`: an entry no table row supplied.
///
/// Written as the Fortran writes it. This is a real value the physics can and does see, not a
/// "missing" marker the reader invents.
pub const UNSET: f32 = -1.0e36;

/// `-99999`: the integer equivalent of [`UNSET`].
pub const UNSET_INT: i32 = -99999;

/// `MPTABLE.TBL` vegetation parameters, from `read_veg_parameters`.
///
/// Per-vegetation-type tables are indexed by the Fortran's own 1-based vegetation type.
#[derive(Debug, Clone)]
pub struct VegTables {
    /// Vegetation types the chosen dataset supplies. Tables are only filled to here.
    pub nveg: i32,
    /// `ISURBAN_TABLE`
    pub isurban: i32,
    /// `ISWATER_TABLE`
    pub iswater: i32,
    /// `ISBARREN_TABLE`
    pub isbarren: i32,
    /// `ISICE_TABLE`
    pub isice: i32,
    /// `ISCROP_TABLE`
    pub iscrop: i32,
    /// `EBLFOREST_TABLE`
    pub eblforest: i32,
    /// `NATURAL_TABLE`
    pub natural: i32,
    /// `LCZ_1_TABLE`
    pub lcz_1: i32,
    /// `LCZ_2_TABLE`
    pub lcz_2: i32,
    /// `LCZ_3_TABLE`
    pub lcz_3: i32,
    /// `LCZ_4_TABLE`
    pub lcz_4: i32,
    /// `LCZ_5_TABLE`
    pub lcz_5: i32,
    /// `LCZ_6_TABLE`
    pub lcz_6: i32,
    /// `LCZ_7_TABLE`
    pub lcz_7: i32,
    /// `LCZ_8_TABLE`
    pub lcz_8: i32,
    /// `LCZ_9_TABLE`
    pub lcz_9: i32,
    /// `LCZ_10_TABLE`
    pub lcz_10: i32,
    /// `LCZ_11_TABLE`
    pub lcz_11: i32,
    /// `CH2OP_TABLE(MVT)`
    pub ch2op: Shifted<f32>,
    /// `DLEAF_TABLE(MVT)`
    pub dleaf: Shifted<f32>,
    /// `Z0MVT_TABLE(MVT)`
    pub z0mvt: Shifted<f32>,
    /// `HVT_TABLE(MVT)`
    pub hvt: Shifted<f32>,
    /// `HVB_TABLE(MVT)`
    pub hvb: Shifted<f32>,
    /// `DEN_TABLE(MVT)`
    pub den: Shifted<f32>,
    /// `RC_TABLE(MVT)`
    pub rc: Shifted<f32>,
    /// `MFSNO_TABLE(MVT)`
    pub mfsno: Shifted<f32>,
    /// `SCFFAC_TABLE(MVT)`
    pub scffac: Shifted<f32>,
    /// `XL_TABLE(MVT)`
    pub xl: Shifted<f32>,
    /// `CWPVT_TABLE(MVT)`
    pub cwpvt: Shifted<f32>,
    /// `C3PSN_TABLE(MVT)`
    pub c3psn: Shifted<f32>,
    /// `KC25_TABLE(MVT)`
    pub kc25: Shifted<f32>,
    /// `AKC_TABLE(MVT)`
    pub akc: Shifted<f32>,
    /// `KO25_TABLE(MVT)`
    pub ko25: Shifted<f32>,
    /// `AKO_TABLE(MVT)`
    pub ako: Shifted<f32>,
    /// `AVCMX_TABLE(MVT)`
    pub avcmx: Shifted<f32>,
    /// `AQE_TABLE(MVT)`
    pub aqe: Shifted<f32>,
    /// `LTOVRC_TABLE(MVT)`
    pub ltovrc: Shifted<f32>,
    /// `DILEFC_TABLE(MVT)`
    pub dilefc: Shifted<f32>,
    /// `DILEFW_TABLE(MVT)`
    pub dilefw: Shifted<f32>,
    /// `RMF25_TABLE(MVT)`
    pub rmf25: Shifted<f32>,
    /// `SLA_TABLE(MVT)`
    pub sla: Shifted<f32>,
    /// `FRAGR_TABLE(MVT)`
    pub fragr: Shifted<f32>,
    /// `TMIN_TABLE(MVT)`
    pub tmin: Shifted<f32>,
    /// `VCMX25_TABLE(MVT)`
    pub vcmx25: Shifted<f32>,
    /// `TDLEF_TABLE(MVT)`
    pub tdlef: Shifted<f32>,
    /// `BP_TABLE(MVT)`
    pub bp: Shifted<f32>,
    /// `MP_TABLE(MVT)`
    pub mp: Shifted<f32>,
    /// `QE25_TABLE(MVT)`
    pub qe25: Shifted<f32>,
    /// `RMS25_TABLE(MVT)`
    pub rms25: Shifted<f32>,
    /// `RMR25_TABLE(MVT)`
    pub rmr25: Shifted<f32>,
    /// `ARM_TABLE(MVT)`
    pub arm: Shifted<f32>,
    /// `FOLNMX_TABLE(MVT)`
    pub folnmx: Shifted<f32>,
    /// `WDPOOL_TABLE(MVT)`
    pub wdpool: Shifted<f32>,
    /// `WRRAT_TABLE(MVT)`
    pub wrrat: Shifted<f32>,
    /// `MRP_TABLE(MVT)`
    pub mrp: Shifted<f32>,
    /// `SHDFAC_TABLE(MVT)`
    pub shdfac: Shifted<f32>,
    /// `NROOT_TABLE(MVT)`
    pub nroot: Shifted<f32>,
    /// `RGL_TABLE(MVT)`
    pub rgl: Shifted<f32>,
    /// `RS_TABLE(MVT)`
    pub rs: Shifted<f32>,
    /// `HS_TABLE(MVT)`
    pub hs: Shifted<f32>,
    /// `TOPT_TABLE(MVT)`
    pub topt: Shifted<f32>,
    /// `RSMAX_TABLE(MVT)`
    pub rsmax: Shifted<f32>,
    /// `SAIM_TABLE(MVT,12)` -- monthly stem area index, assembled from `SAI_JAN`..`SAI_DEC`.
    pub saim: Shifted<Shifted<f32>>,
    /// `LAIM_TABLE(MVT,12)` -- monthly leaf area index, assembled from `LAI_JAN`..`LAI_DEC`.
    pub laim: Shifted<Shifted<f32>>,
    /// `RHOL_TABLE(MVT,MBAND)` -- leaf reflectance; band 1 = visible, 2 = NIR.
    pub rhol: Shifted<Shifted<f32>>,
    /// `RHOS_TABLE(MVT,MBAND)` -- stem reflectance; band 1 = visible, 2 = NIR.
    pub rhos: Shifted<Shifted<f32>>,
    /// `TAUL_TABLE(MVT,MBAND)` -- leaf transmittance; band 1 = visible, 2 = NIR.
    pub taul: Shifted<Shifted<f32>>,
    /// `TAUS_TABLE(MVT,MBAND)` -- stem transmittance; band 1 = visible, 2 = NIR.
    pub taus: Shifted<Shifted<f32>>,
}

/// `SOILPARM.TBL` soil texture parameters, from `read_soil_parameters`.
///
/// Indexed by the Fortran's own 1-based soil texture class (`isltyp`).
#[derive(Debug, Clone)]
pub struct SoilTables {
    /// Soil classes the chosen table supplies (`SLCATS`).
    pub slcats: i32,
    /// `BEXP_TABLE(MAX_SOILTYP)`
    pub bexp: Shifted<f32>,
    /// `SMCDRY_TABLE(MAX_SOILTYP)`
    pub smcdry: Shifted<f32>,
    /// `F1_TABLE(MAX_SOILTYP)`
    pub f1: Shifted<f32>,
    /// `SMCMAX_TABLE(MAX_SOILTYP)`
    pub smcmax: Shifted<f32>,
    /// `SMCREF_TABLE(MAX_SOILTYP)`
    pub smcref: Shifted<f32>,
    /// `PSISAT_TABLE(MAX_SOILTYP)`
    pub psisat: Shifted<f32>,
    /// `DKSAT_TABLE(MAX_SOILTYP)`
    pub dksat: Shifted<f32>,
    /// `DWSAT_TABLE(MAX_SOILTYP)`
    pub dwsat: Shifted<f32>,
    /// `SMCWLT_TABLE(MAX_SOILTYP)`
    pub smcwlt: Shifted<f32>,
    /// `QUARTZ_TABLE(MAX_SOILTYP)`
    pub quartz: Shifted<f32>,
    /// `BVIC_TABLE(MAX_SOILTYP)`
    pub bvic: Shifted<f32>,
    /// `AXAJ_TABLE(MAX_SOILTYP)`
    pub axaj: Shifted<f32>,
    /// `BXAJ_TABLE(MAX_SOILTYP)`
    pub bxaj: Shifted<f32>,
    /// `XXAJ_TABLE(MAX_SOILTYP)`
    pub xxaj: Shifted<f32>,
    /// `BDVIC_TABLE(MAX_SOILTYP)`
    pub bdvic: Shifted<f32>,
    /// `BBVIC_TABLE(MAX_SOILTYP)`
    pub bbvic: Shifted<f32>,
    /// `GDVIC_TABLE(MAX_SOILTYP)`
    pub gdvic: Shifted<f32>,
}

/// `GENPARM.TBL` general parameters, read positionally by `read_soil_parameters`.
#[derive(Debug, Clone)]
pub struct GenTables {
    /// `SLOPE_TABLE(9)`; only `1..=num_slope` are supplied.
    pub slope: Shifted<f32>,
    /// Slope categories the file supplies (`NUM_SLOPE`).
    pub num_slope: i32,
    /// `CSOIL_TABLE`
    pub csoil: f32,
    /// `REFDK_TABLE`
    pub refdk: f32,
    /// `REFKDT_TABLE`
    pub refkdt: f32,
    /// `FRZK_TABLE`
    pub frzk: f32,
    /// `ZBOT_TABLE`
    pub zbot: f32,
    /// `CZIL_TABLE`
    pub czil: f32,
    /// `Z0_TABLE`
    pub z0: f32,
}

/// `MPTABLE.TBL` `&rad_parameters`, from `read_rad_parameters`.
#[derive(Debug, Clone)]
pub struct RadTables {
    /// `ALBSAT_TABLE(MSC,MBAND)` -- saturated soil albedo, indexed by soil colour.
    pub albsat: Shifted<Shifted<f32>>,
    /// `ALBDRY_TABLE(MSC,MBAND)` -- dry soil albedo, indexed by soil colour.
    pub albdry: Shifted<Shifted<f32>>,
    /// `ALBICE_TABLE(MBAND)`
    pub albice: Shifted<f32>,
    /// `ALBLAK_TABLE(MBAND)`
    pub alblak: Shifted<f32>,
    /// `OMEGAS_TABLE(MBAND)`
    pub omegas: Shifted<f32>,
    /// `BETADS_TABLE`
    pub betads: f32,
    /// `BETAIS_TABLE`
    pub betais: f32,
    /// `EG_TABLE(2)` -- surface emissivity; 1 = soil, 2 = lake.
    pub eg: Shifted<f32>,
}

/// `MPTABLE.TBL` `&global_parameters`, from `read_global_parameters`.
#[derive(Debug, Clone)]
pub struct GlobalTables {
    /// `CO2_TABLE`
    pub co2: f32,
    /// `O2_TABLE`
    pub o2: f32,
    /// `TIMEAN_TABLE`
    pub timean: f32,
    /// `FSATMX_TABLE`
    pub fsatmx: f32,
    /// `Z0SNO_TABLE`
    pub z0sno: f32,
    /// `SSI_TABLE`
    pub ssi: f32,
    /// `SNOW_RET_FAC_TABLE`
    pub snow_ret_fac: f32,
    /// `SNOW_EMIS_TABLE`
    pub snow_emis: f32,
    /// `SWEMX_TABLE`
    pub swemx: f32,
    /// `TAU0_TABLE`
    pub tau0: f32,
    /// `GRAIN_GROWTH_TABLE`
    pub grain_growth: f32,
    /// `EXTRA_GROWTH_TABLE`
    pub extra_growth: f32,
    /// `DIRT_SOOT_TABLE`
    pub dirt_soot: f32,
    /// `BATS_COSZ_TABLE`
    pub bats_cosz: f32,
    /// `BATS_VIS_NEW_TABLE`
    pub bats_vis_new: f32,
    /// `BATS_NIR_NEW_TABLE`
    pub bats_nir_new: f32,
    /// `BATS_VIS_AGE_TABLE`
    pub bats_vis_age: f32,
    /// `BATS_NIR_AGE_TABLE`
    pub bats_nir_age: f32,
    /// `BATS_VIS_DIR_TABLE`
    pub bats_vis_dir: f32,
    /// `BATS_NIR_DIR_TABLE`
    pub bats_nir_dir: f32,
    /// `RSURF_SNOW_TABLE`
    pub rsurf_snow: f32,
    /// `RSURF_EXP_TABLE`
    pub rsurf_exp: f32,
}

/// Every parameter table the model reads, owned rather than global.
#[derive(Debug, Clone)]
pub struct Tables {
    pub veg: VegTables,
    pub soil: SoilTables,
    pub gen: GenTables,
    pub rad: RadTables,
    pub global: GlobalTables,
}

impl Tables {
    /// Read all four tables from `parameter_dir`, as `paramRead` does.
    ///
    /// The Fortran opens `MPTABLE.TBL` three times -- once each in `read_veg_parameters`,
    /// `read_rad_parameters` and `read_global_parameters` -- and scans forward for a different
    /// group each time. Reading it once and taking three groups out of it is equivalent,
    /// because the group names are distinct.
    pub fn read(namelist: &NamelistConfig) -> Result<Self, ConfigError> {
        let read = |table: &str| -> Result<String, ConfigError> {
            // The Fortran builds this path as `trim(dir)//'/'//trim(table)`, so a directory
            // with a trailing slash yields a doubled separator. Harmless, and kept literal.
            let path = format!("{}/{}", namelist.parameter_dir, table);
            std::fs::read_to_string(Path::new(&path)).map_err(|source| ConfigError::Io {
                path,
                source,
            })
        };
        let mptable = read(&namelist.noahowp_table)?;
        let soilparm = read(&namelist.soil_table)?;
        let genparm = read(&namelist.general_table)?;
        Self::parse(&mptable, &soilparm, &genparm, namelist)
    }

    /// Parse already-loaded table contents.
    pub fn parse(
        mptable: &str,
        soilparm: &str,
        genparm: &str,
        namelist: &NamelistConfig,
    ) -> Result<Self, ConfigError> {
        let nml = Namelist::parse(mptable)?;
        Ok(Self {
            // paramRead calls read_soil_parameters before read_veg_parameters. The order has
            // no effect here -- no table is read twice -- but it is kept for the diff.
            soil: SoilTables::parse(soilparm, &namelist.soil_class_name)?,
            gen: GenTables::parse(genparm)?,
            veg: VegTables::parse(&nml, &namelist.veg_class_name)?,
            rad: RadTables::parse(&nml)?,
            global: GlobalTables::parse(&nml)?,
        })
    }
}

/// One `*_TABLE(MVT)` row: read the whole namelist array, then copy only `1..=nveg`.
///
/// The two-step matters. The Fortran reads into a local of length `MVT` and assigns
/// `TABLE(1:NVEG) = LOCAL(1:NVEG)`, so entries past `nveg` keep the `-1.E36` the table was
/// filled with even when the file supplies more columns than the dataset declares.
fn veg_row(params: &Group, key: &str, nveg: i32) -> Result<Shifted<f32>, ConfigError> {
    let mut local = Shifted::ones(MVT, UNSET);
    params.get_real_array(key, local.as_mut_slice())?;
    let mut table = Shifted::ones(MVT, UNSET);
    for i in 1..=nveg {
        table[i] = local[i];
    }
    Ok(table)
}

impl VegTables {
    /// `read_veg_parameters`.
    fn parse(nml: &Namelist, dataset: &str) -> Result<Self, ConfigError> {
        let (cats_name, params_name) = match dataset.trim() {
            "USGS" => ("usgs_veg_categories", "usgs_veg_parameters"),
            "MODIFIED_IGBP_MODIS_NOAH" => ("modis_veg_categories", "modis_veg_parameters"),
            other => return Err(ConfigError::UnknownVegDataset(other.to_string())),
        };
        let cats = nml
            .group(cats_name)
            .ok_or(ConfigError::MissingGroup { file: "MPTABLE.TBL", group: cats_name })?;
        let params = nml
            .group(params_name)
            .ok_or(ConfigError::MissingGroup { file: "MPTABLE.TBL", group: params_name })?;

        let mut nveg = UNSET_INT;
        cats.get_int("NVEG", &mut nveg)?;
        if !(1..=MVT).contains(&nveg) {
            // The Fortran would index past the end of every table here.
            return Err(ConfigError::TableTooLarge { what: "NVEG", value: nveg, max: MVT });
        }

        let mut isurban = UNSET_INT;
        let mut iswater = UNSET_INT;
        let mut isbarren = UNSET_INT;
        let mut isice = UNSET_INT;
        let mut iscrop = UNSET_INT;
        let mut eblforest = UNSET_INT;
        let mut natural = UNSET_INT;
        let mut lcz_1 = UNSET_INT;
        let mut lcz_2 = UNSET_INT;
        let mut lcz_3 = UNSET_INT;
        let mut lcz_4 = UNSET_INT;
        let mut lcz_5 = UNSET_INT;
        let mut lcz_6 = UNSET_INT;
        let mut lcz_7 = UNSET_INT;
        let mut lcz_8 = UNSET_INT;
        let mut lcz_9 = UNSET_INT;
        let mut lcz_10 = UNSET_INT;
        let mut lcz_11 = UNSET_INT;

        params.get_int("ISURBAN", &mut isurban)?;
        params.get_int("ISWATER", &mut iswater)?;
        params.get_int("ISBARREN", &mut isbarren)?;
        params.get_int("ISICE", &mut isice)?;
        params.get_int("ISCROP", &mut iscrop)?;
        params.get_int("EBLFOREST", &mut eblforest)?;
        params.get_int("NATURAL", &mut natural)?;
        params.get_int("LCZ_1", &mut lcz_1)?;
        params.get_int("LCZ_2", &mut lcz_2)?;
        params.get_int("LCZ_3", &mut lcz_3)?;
        params.get_int("LCZ_4", &mut lcz_4)?;
        params.get_int("LCZ_5", &mut lcz_5)?;
        params.get_int("LCZ_6", &mut lcz_6)?;
        params.get_int("LCZ_7", &mut lcz_7)?;
        params.get_int("LCZ_8", &mut lcz_8)?;
        params.get_int("LCZ_9", &mut lcz_9)?;
        params.get_int("LCZ_10", &mut lcz_10)?;
        params.get_int("LCZ_11", &mut lcz_11)?;

        let mut saim = Shifted::ones(MVT, Shifted::ones(12, UNSET));
        let mut laim = Shifted::ones(MVT, Shifted::ones(12, UNSET));
        // SAIM_TABLE(MVT,12) and LAIM_TABLE are not single namelist keys: the Fortran
        // assembles them from twelve monthly vectors.
        for (month, name) in [
            "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
        ]
        .iter()
        .enumerate()
        {
            let month = month as i32 + 1;
            let sai = veg_row(params, &format!("SAI_{name}"), nveg)?;
            let lai = veg_row(params, &format!("LAI_{name}"), nveg)?;
            for i in 1..=nveg {
                saim[i][month] = sai[i];
                laim[i][month] = lai[i];
            }
        }

        // Canopy radiation properties arrive as separate visible and NIR vectors.
        let banded = |vis: &str, nir: &str| -> Result<Shifted<Shifted<f32>>, ConfigError> {
            let v = veg_row(params, vis, nveg)?;
            let n = veg_row(params, nir, nveg)?;
            let mut out = Shifted::ones(MVT, Shifted::ones(MBAND, UNSET));
            for i in 1..=nveg {
                out[i][1] = v[i];
                out[i][2] = n[i];
            }
            Ok(out)
        };
        let rhol = banded("RHOL_VIS", "RHOL_NIR")?;
        let rhos = banded("RHOS_VIS", "RHOS_NIR")?;
        let taul = banded("TAUL_VIS", "TAUL_NIR")?;
        let taus = banded("TAUS_VIS", "TAUS_NIR")?;

        Ok(Self {
            nveg,
            isurban,
            iswater,
            isbarren,
            isice,
            iscrop,
            eblforest,
            natural,
            lcz_1,
            lcz_2,
            lcz_3,
            lcz_4,
            lcz_5,
            lcz_6,
            lcz_7,
            lcz_8,
            lcz_9,
            lcz_10,
            lcz_11,
            ch2op: veg_row(params, "CH2OP", nveg)?,
            dleaf: veg_row(params, "DLEAF", nveg)?,
            z0mvt: veg_row(params, "Z0MVT", nveg)?,
            hvt: veg_row(params, "HVT", nveg)?,
            hvb: veg_row(params, "HVB", nveg)?,
            den: veg_row(params, "DEN", nveg)?,
            rc: veg_row(params, "RC", nveg)?,
            mfsno: veg_row(params, "MFSNO", nveg)?,
            scffac: veg_row(params, "SCFFAC", nveg)?,
            xl: veg_row(params, "XL", nveg)?,
            cwpvt: veg_row(params, "CWPVT", nveg)?,
            c3psn: veg_row(params, "C3PSN", nveg)?,
            kc25: veg_row(params, "KC25", nveg)?,
            akc: veg_row(params, "AKC", nveg)?,
            ko25: veg_row(params, "KO25", nveg)?,
            ako: veg_row(params, "AKO", nveg)?,
            avcmx: veg_row(params, "AVCMX", nveg)?,
            aqe: veg_row(params, "AQE", nveg)?,
            ltovrc: veg_row(params, "LTOVRC", nveg)?,
            dilefc: veg_row(params, "DILEFC", nveg)?,
            dilefw: veg_row(params, "DILEFW", nveg)?,
            rmf25: veg_row(params, "RMF25", nveg)?,
            sla: veg_row(params, "SLA", nveg)?,
            fragr: veg_row(params, "FRAGR", nveg)?,
            tmin: veg_row(params, "TMIN", nveg)?,
            vcmx25: veg_row(params, "VCMX25", nveg)?,
            tdlef: veg_row(params, "TDLEF", nveg)?,
            bp: veg_row(params, "BP", nveg)?,
            mp: veg_row(params, "MP", nveg)?,
            qe25: veg_row(params, "QE25", nveg)?,
            rms25: veg_row(params, "RMS25", nveg)?,
            rmr25: veg_row(params, "RMR25", nveg)?,
            arm: veg_row(params, "ARM", nveg)?,
            folnmx: veg_row(params, "FOLNMX", nveg)?,
            wdpool: veg_row(params, "WDPOOL", nveg)?,
            wrrat: veg_row(params, "WRRAT", nveg)?,
            mrp: veg_row(params, "MRP", nveg)?,
            shdfac: veg_row(params, "SHDFAC", nveg)?,
            nroot: veg_row(params, "NROOT", nveg)?,
            rgl: veg_row(params, "RGL", nveg)?,
            rs: veg_row(params, "RS", nveg)?,
            hs: veg_row(params, "HS", nveg)?,
            topt: veg_row(params, "TOPT", nveg)?,
            rsmax: veg_row(params, "RSMAX", nveg)?,
            saim,
            laim,
            rhol,
            rhos,
            taul,
            taus,
        })
    }
}

impl SoilTables {
    /// The `SOILPARM.TBL` half of `read_soil_parameters`.
    fn parse(contents: &str, soil_class_name: &str) -> Result<Self, ConfigError> {
        let mut r = ListDirectedReader::new(contents);
        let bad = |source| ConfigError::Table { file: "SOILPARM.TBL", source };
        // `do iLine = 1,100 / READ(21,*) SLTYPE / if (trim(SLTYPE) == soil_class_name) exit`.
        r.find_block(soil_class_name.trim(), 100)
            .ok_or_else(|| ConfigError::SoilClassNotFound(soil_class_name.trim().to_string()))?;

        // The record is `19,1  'BB ...'`; the statement takes the first value and the rest
        // of the record is discarded.
        let slcats = r.read_int().map_err(bad)?;
        if !(1..=MAX_SOILTYP).contains(&slcats) {
            return Err(ConfigError::TableTooLarge {
                what: "SLCATS",
                value: slcats,
                max: MAX_SOILTYP,
            });
        }

        let mut bexp = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut smcdry = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut f1 = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut smcmax = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut smcref = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut psisat = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut dksat = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut dwsat = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut smcwlt = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut quartz = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut bvic = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut axaj = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut bxaj = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut xxaj = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut bdvic = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut bbvic = Shifted::ones(MAX_SOILTYP, UNSET);
        let mut gdvic = Shifted::ones(MAX_SOILTYP, UNSET);

        for lc in 1..=slcats {
            let mut itmp = 0;
            let mut row = [UNSET; 17];
            r.read_int_then_reals(&mut itmp, &mut row).map_err(bad)?;
            // The row index is read and discarded: the Fortran stores by loop counter, not
            // by the index in the file.
            bexp[lc] = row[0];
            smcdry[lc] = row[1];
            f1[lc] = row[2];
            smcmax[lc] = row[3];
            smcref[lc] = row[4];
            psisat[lc] = row[5];
            dksat[lc] = row[6];
            dwsat[lc] = row[7];
            smcwlt[lc] = row[8];
            quartz[lc] = row[9];
            bvic[lc] = row[10];
            axaj[lc] = row[11];
            bxaj[lc] = row[12];
            xxaj[lc] = row[13];
            bdvic[lc] = row[14];
            bbvic[lc] = row[15];
            gdvic[lc] = row[16];
        }

        Ok(Self {
            slcats,
            bexp,
            smcdry,
            f1,
            smcmax,
            smcref,
            psisat,
            dksat,
            dwsat,
            smcwlt,
            quartz,
            bvic,
            axaj,
            bxaj,
            xxaj,
            bdvic,
            bbvic,
            gdvic,
        })
    }
}

impl GenTables {
    /// The `GENPARM.TBL` half of `read_soil_parameters`.
    ///
    /// Positional: the file is label/value pairs, and the Fortran walks it with bare
    /// `read (22,*)` statements rather than matching labels. Counting records wrong shifts
    /// every value after it, so the skip counts are the thing to check against the Fortran.
    fn parse(contents: &str) -> Result<Self, ConfigError> {
        let mut r = ListDirectedReader::new(contents);
        let bad = |source| ConfigError::Table { file: "GENPARM.TBL", source };
        r.skip_record(); // "General Parameters"
        r.skip_record(); // "SLOPE_DATA"
        let num_slope = r.read_int().map_err(bad)?;
        if !(0..=MAX_SLOPE).contains(&num_slope) {
            return Err(ConfigError::TableTooLarge {
                what: "NUM_SLOPE",
                value: num_slope,
                max: MAX_SLOPE,
            });
        }
        let mut slope = Shifted::ones(MAX_SLOPE, UNSET);
        for lc in 1..=num_slope {
            slope[lc] = r.read_real().map_err(bad)?;
        }

        for _ in 0..5 {
            r.skip_record();
        }
        let csoil = r.read_real().map_err(bad)?;
        for _ in 0..3 {
            r.skip_record();
        }
        let refdk = r.read_real().map_err(bad)?;
        for _ in 0..1 {
            r.skip_record();
        }
        let refkdt = r.read_real().map_err(bad)?;
        for _ in 0..1 {
            r.skip_record();
        }
        let frzk = r.read_real().map_err(bad)?;
        for _ in 0..1 {
            r.skip_record();
        }
        let zbot = r.read_real().map_err(bad)?;
        for _ in 0..1 {
            r.skip_record();
        }
        let czil = r.read_real().map_err(bad)?;
        for _ in 0..7 {
            r.skip_record();
        }
        let z0 = r.read_real().map_err(bad)?;

        Ok(Self {
            slope,
            num_slope,
            csoil,
            refdk,
            refkdt,
            frzk,
            zbot,
            czil,
            z0,
        })
    }
}

impl RadTables {
    /// `read_rad_parameters`.
    fn parse(nml: &Namelist) -> Result<Self, ConfigError> {
        let g = nml
            .group("rad_parameters")
            .ok_or(ConfigError::MissingGroup { file: "MPTABLE.TBL", group: "rad_parameters" })?;

        let soil_colour = |vis: &str, nir: &str| -> Result<Shifted<Shifted<f32>>, ConfigError> {
            let (mut v, mut n) = (vec![UNSET; MSC as usize], vec![UNSET; MSC as usize]);
            g.get_real_array(vis, &mut v)?;
            g.get_real_array(nir, &mut n)?;
            let mut out = Shifted::ones(MSC, Shifted::ones(MBAND, UNSET));
            for i in 1..=MSC {
                out[i][1] = v[i as usize - 1];
                out[i][2] = n[i as usize - 1];
            }
            Ok(out)
        };
        let albsat = soil_colour("ALBSAT_VIS", "ALBSAT_NIR")?;
        let albdry = soil_colour("ALBDRY_VIS", "ALBDRY_NIR")?;

        let mut albice = Shifted::ones(MBAND, UNSET);
        g.get_real_array("ALBICE", albice.as_mut_slice())?;
        let mut alblak = Shifted::ones(MBAND, UNSET);
        g.get_real_array("ALBLAK", alblak.as_mut_slice())?;
        let mut omegas = Shifted::ones(MBAND, UNSET);
        g.get_real_array("OMEGAS", omegas.as_mut_slice())?;
        let mut eg = Shifted::ones(2, UNSET);
        g.get_real_array("EG", eg.as_mut_slice())?;
        let mut betads = UNSET;
        let mut betais = UNSET;
        g.get_real("BETADS", &mut betads)?;
        g.get_real("BETAIS", &mut betais)?;

        Ok(Self {
            albsat,
            albdry,
            albice,
            alblak,
            omegas,
            betads,
            betais,
            eg,
        })
    }
}

impl GlobalTables {
    /// `read_global_parameters`.
    fn parse(nml: &Namelist) -> Result<Self, ConfigError> {
        let g = nml.group("global_parameters").ok_or(ConfigError::MissingGroup {
            file: "MPTABLE.TBL",
            group: "global_parameters",
        })?;

        let mut co2 = UNSET;
        let mut o2 = UNSET;
        let mut timean = UNSET;
        let mut fsatmx = UNSET;
        let mut z0sno = UNSET;
        let mut ssi = UNSET;
        let mut snow_ret_fac = UNSET;
        let mut snow_emis = UNSET;
        let mut swemx = UNSET;
        let mut tau0 = UNSET;
        let mut grain_growth = UNSET;
        let mut extra_growth = UNSET;
        let mut dirt_soot = UNSET;
        let mut bats_cosz = UNSET;
        let mut bats_vis_new = UNSET;
        let mut bats_nir_new = UNSET;
        let mut bats_vis_age = UNSET;
        let mut bats_nir_age = UNSET;
        let mut bats_vis_dir = UNSET;
        let mut bats_nir_dir = UNSET;
        let mut rsurf_snow = UNSET;
        let mut rsurf_exp = UNSET;

        g.get_real("CO2", &mut co2)?;
        g.get_real("O2", &mut o2)?;
        g.get_real("TIMEAN", &mut timean)?;
        g.get_real("FSATMX", &mut fsatmx)?;
        g.get_real("Z0SNO", &mut z0sno)?;
        g.get_real("SSI", &mut ssi)?;
        g.get_real("SNOW_RET_FAC", &mut snow_ret_fac)?;
        g.get_real("SNOW_EMIS", &mut snow_emis)?;
        g.get_real("SWEMX", &mut swemx)?;
        g.get_real("TAU0", &mut tau0)?;
        g.get_real("GRAIN_GROWTH", &mut grain_growth)?;
        g.get_real("EXTRA_GROWTH", &mut extra_growth)?;
        g.get_real("DIRT_SOOT", &mut dirt_soot)?;
        g.get_real("BATS_COSZ", &mut bats_cosz)?;
        g.get_real("BATS_VIS_NEW", &mut bats_vis_new)?;
        g.get_real("BATS_NIR_NEW", &mut bats_nir_new)?;
        g.get_real("BATS_VIS_AGE", &mut bats_vis_age)?;
        g.get_real("BATS_NIR_AGE", &mut bats_nir_age)?;
        g.get_real("BATS_VIS_DIR", &mut bats_vis_dir)?;
        g.get_real("BATS_NIR_DIR", &mut bats_nir_dir)?;
        g.get_real("RSURF_SNOW", &mut rsurf_snow)?;
        g.get_real("RSURF_EXP", &mut rsurf_exp)?;

        Ok(Self {
            co2,
            o2,
            timean,
            fsatmx,
            z0sno,
            ssi,
            snow_ret_fac,
            snow_emis,
            swemx,
            tau0,
            grain_growth,
            extra_growth,
            dirt_soot,
            bats_cosz,
            bats_vis_new,
            bats_nir_new,
            bats_vis_age,
            bats_nir_age,
            bats_vis_dir,
            bats_nir_dir,
            rsurf_snow,
            rsurf_exp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `SOILPARM.TBL`: two classes, and a second block so the class search has to
    /// pick the right one.
    const SOILPARM: &str = "\
Soil Parameters
STAS
2,1   'BB DRYSMC F11 MAXSMC REFSMC SATPSI SATDK SATDW WLTSMC QTZ BVIC AXAJ BXAJ XXAJ BDVIC BBVIC GDVIC'
1,  2.79, 0.010, -0.472, 0.339, 0.192, 0.069, 4.66E-5, 2.65E-5, 0.010, 0.92, 0.05, 0.009, 0.05, 0.05, 0.05, 1.000, 0.050, 'SAND'
2,  4.26, 0.028, -1.044, 0.421, 0.283, 0.036, 1.41E-5, 5.14E-6, 0.028, 0.82, 0.08, 0.010, 0.08, 0.08, 0.08, 1.010, 0.070, 'LOAMY SAND'
STAS-RUC
1,1   'header'
1,  9.99, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 'OTHER'
";

    fn soil(class: &str) -> Result<SoilTables, ConfigError> {
        SoilTables::parse(SOILPARM, class)
    }

    #[test]
    fn reads_the_named_soil_block() {
        let t = soil("STAS").unwrap();
        assert_eq!(t.slcats, 2);
        assert_eq!(t.bexp[1], 2.79);
        assert_eq!(t.bexp[2], 4.26);
        // The trailing 'SAND' label is on the same record and must not be read as a value.
        assert_eq!(t.gdvic[1], 0.050);
    }

    /// Picking the wrong block is a silent, plausible-looking failure, so it gets its own test.
    #[test]
    fn a_later_block_is_reachable() {
        let t = soil("STAS-RUC").unwrap();
        assert_eq!(t.slcats, 1);
        assert_eq!(t.bexp[1], 9.99);
    }

    #[test]
    fn rows_past_the_last_class_keep_the_sentinel() {
        let t = soil("STAS").unwrap();
        assert_eq!(t.bexp[3], UNSET);
        assert_eq!(t.bexp[MAX_SOILTYP], UNSET);
    }

    #[test]
    fn an_unknown_soil_class_is_an_error() {
        assert!(matches!(
            soil("NOPE"),
            Err(ConfigError::SoilClassNotFound(_))
        ));
    }

    #[test]
    fn more_classes_than_the_table_holds_is_an_error() {
        let src = SOILPARM.replace("2,1   'BB", "99,1   'BB");
        assert!(matches!(
            SoilTables::parse(&src, "STAS"),
            Err(ConfigError::TableTooLarge { what: "SLCATS", value: 99, .. })
        ));
    }

    const GENPARM: &str = "\
General Parameters
SLOPE_DATA
2
0.1
0.6
SBETA_DATA
-2.0
FXEXP_DATA
2.0
CSOIL_DATA
2.00E+6
SALP_DATA
2.6
REFDK_DATA
2.0E-6
REFKDT_DATA
3.0
FRZK_DATA
0.15
ZBOT_DATA
-8.0
CZIL_DATA
0.1
SMLOW_DATA
0.5
SMHIGH_DATA
3.0
LVCOEF_DATA
0.5
Z0_DATA
0.002
";

    /// `GENPARM.TBL` is read positionally, by counting records rather than matching labels, so
    /// an off-by-one in the skip counts shifts every value after it.
    #[test]
    fn genparm_lands_on_the_right_values() {
        let t = GenTables::parse(GENPARM).unwrap();
        assert_eq!(t.num_slope, 2);
        assert_eq!(t.slope[1], 0.1);
        assert_eq!(t.slope[2], 0.6);
        assert_eq!(t.slope[3], UNSET);
        assert_eq!(t.csoil, 2.00e6);
        assert_eq!(t.refdk, 2.0e-6);
        assert_eq!(t.refkdt, 3.0);
        assert_eq!(t.frzk, 0.15);
        assert_eq!(t.zbot, -8.0);
        assert_eq!(t.czil, 0.1);
        assert_eq!(t.z0, 0.002);
    }

    const MPTABLE: &str = "\
&modis_veg_categories
 VEG_DATASET_DESCRIPTION = \"modified igbp modis noah\"
 NVEG = 2
/
&modis_veg_parameters
 CH2OP = 0.1, 0.2, 0.3,
 NROOT = 4.0, 3.0, 2.0,
 SAI_JAN = 0.4, 0.5, 0.6,
 LAI_JUL = 1.4, 1.5, 1.6,
 RHOL_VIS = 0.07, 0.08, 0.09,
 RHOL_NIR = 0.35, 0.36, 0.37,
/
";

    #[test]
    fn veg_tables_stop_at_nveg() {
        let nml = Namelist::parse(MPTABLE).unwrap();
        let t = VegTables::parse(&nml, "MODIFIED_IGBP_MODIS_NOAH").unwrap();
        assert_eq!(t.nveg, 2);
        assert_eq!(t.ch2op[1], 0.1);
        assert_eq!(t.ch2op[2], 0.2);
        // The file supplies a third value; NVEG says there are two, so the third is not
        // copied and the sentinel survives.
        assert_eq!(t.ch2op[3], UNSET);

        // The monthly tables are assembled from twelve separate keys; months the file does not
        // mention keep the sentinel.
        assert_eq!(t.saim[1][1], 0.4);
        assert_eq!(t.saim[1][2], UNSET);
        assert_eq!(t.laim[2][7], 1.5);

        assert_eq!(t.rhol[1][1], 0.07);
        assert_eq!(t.rhol[1][2], 0.35);
        assert_eq!(t.rhol[3][1], UNSET);
    }

    #[test]
    fn an_unknown_veg_dataset_is_an_error() {
        let nml = Namelist::parse(MPTABLE).unwrap();
        assert!(matches!(
            VegTables::parse(&nml, "NOPE"),
            Err(ConfigError::UnknownVegDataset(_))
        ));
    }

    #[test]
    fn a_missing_group_is_an_error() {
        let nml = Namelist::parse("&modis_veg_categories\n NVEG = 2\n/\n").unwrap();
        assert!(matches!(
            VegTables::parse(&nml, "MODIFIED_IGBP_MODIS_NOAH"),
            Err(ConfigError::MissingGroup { group: "modis_veg_parameters", .. })
        ));
    }

    #[test]
    fn more_veg_types_than_the_table_holds_is_an_error() {
        let src = MPTABLE.replace("NVEG = 2", "NVEG = 99");
        let nml = Namelist::parse(&src).unwrap();
        assert!(matches!(
            VegTables::parse(&nml, "MODIFIED_IGBP_MODIS_NOAH"),
            Err(ConfigError::TableTooLarge { what: "NVEG", value: 99, .. })
        ));
    }
}
