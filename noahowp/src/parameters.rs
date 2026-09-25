//! Port of `src/ParametersType.f90` @ 0ff055e.
//!
//! `Init` allocates and defaults, `paramRead` reads the tables and assembles the parameter set
//! for one `(vegtyp, isltyp, soilcolor)`. Here that is a single constructor, because there is
//! no way to observe the half-built state in between.
//!
//! Fields keep the Fortran's names and case so `parameters%SMCMAX` and `parameters.SMCMAX`
//! read the same in a side-by-side diff. Literal values are copied digit for digit -- see
//! `constants.rs` for what happens otherwise.

#![allow(non_snake_case)]

use crate::layers::Shifted;
use crate::namelist_read::{ConfigError, NamelistConfig};
use crate::parameters_read::Tables;

/// `parameters_type`.
#[derive(Debug, Clone)]
pub struct Parameters {
    /// b parameter
    pub bexp: Shifted<f32>,
    /// porosity (volumetric)
    pub smcmax: Shifted<f32>,
    /// wilting point
    pub smcwlt: Shifted<f32>,
    /// field capacity
    pub smcref: Shifted<f32>,
    /// saturated conductivity
    pub dksat: Shifted<f32>,
    /// saturated diffusivity
    pub dwsat: Shifted<f32>,
    /// saturated matric potential
    pub psisat: Shifted<f32>,
    /// VIC or DVIC model infiltration parameter
    pub bvic: f32,
    /// Xinanjiang: Tension water distribution inflection parameter [-]
    pub AXAJ: f32,
    /// Xinanjiang: Tension water distribution shape parameter [-]
    pub BXAJ: f32,
    /// Xinanjiang: Free water distribution shape parameter [-]
    pub XXAJ: f32,
    /// DVIC heterogeniety parameter for infiltration
    pub BBVIC: f32,
    /// Mean Capillary Drive (m) for infiltration models
    pub G: f32,
    /// fraction of soil comprised of quartz [-] (equal to pctsand/100)
    pub QUARTZ: f32,
    pub kdt: f32,
    pub refkdt: f32,
    pub refdk: f32,
    /// volumetric soil heat capacity [j/m3/K]
    pub csoil: f32,
    /// bare soil roughness length (m)
    pub Z0: f32,
    /// Parameter used in the calculation of the roughness length for heat, originally in GENPARM.TBL
    pub CZIL: f32,
    /// Depth (m) of lower boundary soil temperature, originally in GENPARM.TBL
    pub ZBOT: f32,
    pub frzx: f32,
    /// drainage parameter
    pub slope: f32,
    pub timean: f32,
    pub fsatmx: f32,
    /// initial water table depth below surface [m]
    pub ZWT_INIT: f32,
    pub urban_flag: bool,
    /// monthly LAI
    pub LAIM: Shifted<f32>,
    /// monthly SAI
    pub SAIM: Shifted<f32>,
    pub LAI: f32,
    pub SAI: f32,
    /// maximum intercepted h2o per unit lai+sai (mm)
    pub CH2OP: f32,
    /// vegetation root level
    pub NROOT: i32,
    /// canopy top height (m)
    pub HVT: f32,
    /// canopy bottom height (m)
    pub HVB: f32,
    /// minimum temperature for photosynthesis (k)
    pub TMIN: f32,
    /// fraction of surface covered by vegetation (dimensionless, 0.0 to 1.0)
    pub SHDFAC: f32,
    /// annual maximum fraction of surface covered by vegetation (dimensionless, 0.0 to 1.0)
    pub SHDMAX: f32,
    /// momentum roughness length (m)
    pub Z0MVT: f32,
    /// tree crown radius (m)
    pub RC: f32,
    /// leaf/stem orientation index
    pub XL: f32,
    /// minimum leaf conductance (umol/m**2/s)
    pub BP: f32,
    /// foliage nitrogen concentration when f(n)=1 (%)
    pub FOLNMX: f32,
    /// quantum efficiency at 25c (umol co2 / umol photon)
    pub QE25: f32,
    /// maximum rate of carboxylation at 25c (umol co2/m**2/s)
    pub VCMX25: f32,
    /// slope of conductance-to-photosynthesis relationship
    pub MP: f32,
    /// Parameter used in radiation stress function
    pub RGL: f32,
    /// Minimum stomatal resistance [s m-1]
    pub RSMIN: f32,
    /// Parameter used in vapor pressure deficit function
    pub HS: f32,
    /// q10 for kc25
    pub AKC: f32,
    /// q10 for ko25
    pub AKO: f32,
    /// q10 for vcmx25
    pub AVCMX: f32,
    /// Maximal stomatal resistance [s m-1]
    pub RSMAX: f32,
    /// canopy wind absorption coefficient (formerly CWPVT)
    pub CWP: f32,
    /// photosynth. pathway: 0. = c4, 1. = c3 [by vegtype]
    pub C3PSN: f32,
    /// characteristic leaf dimension (m)
    pub DLEAF: f32,
    /// co2 michaelis-menten constant at 25c (pa)
    pub KC25: f32,
    /// o2 michaelis-menten constant at 25c (pa)
    pub KO25: f32,
    pub ELAI: f32,
    pub ESAI: f32,
    /// sum of ELAI + ESAI
    pub VAI: f32,
    /// grid cell is vegetated (true) or not (false)
    pub VEG: bool,
    /// vegetation fraction
    pub FVEG: f32,
    /// leaf reflectance (1 = vis, 2 = NIR)
    pub RHOL: Shifted<f32>,
    /// stem reflectance (1 = vis, 2 = NIR)
    pub RHOS: Shifted<f32>,
    /// leaf transmittance (1 = vis, 2 = NIR)
    pub TAUL: Shifted<f32>,
    /// stem transmittance (1 = vis, 2 = NIR)
    pub TAUS: Shifted<f32>,
    /// vegtype code for urban land cover
    pub ISURBAN: i32,
    /// vegtype code for water
    pub ISWATER: i32,
    /// vegtype code for barren land cover
    pub ISBARREN: i32,
    /// vegtype code for ice/snow land cover
    pub ISICE: i32,
    /// vegtype code for crop land cover
    pub ISCROP: i32,
    /// vegtype code for evergreen broadleaf forest
    pub EBLFOREST: i32,
    /// vegtype code for cropland/grassland mosaic
    pub NATURAL: i32,
    /// vegtype code for low density residential
    pub LOW_DENSITY_RESIDENTIAL: i32,
    /// vegtype code for high density residential
    pub HIGH_DENSITY_RESIDENTIAL: i32,
    /// vegtype code for high density industrial
    pub HIGH_INTENSITY_INDUSTRIAL: i32,
    /// Stefan-Boltzmann constant (w/m2/k4)
    pub SB: f32,
    /// von Karman constant
    pub VKC: f32,
    /// freezing/melting point (k)
    pub TFRZ: f32,
    /// latent heat of sublimation (j/kg)
    pub HSUB: f32,
    /// latent heat of vaporization (j/kg)
    pub HVAP: f32,
    /// latent heat of fusion (j/kg)
    pub HFUS: f32,
    /// specific heat capacity of water (j/m3/k)
    pub CWAT: f32,
    /// specific heat capacity of ice (j/m3/k)
    pub CICE: f32,
    /// heat capacity dry air at const pres (j/kg/k)
    pub CPAIR: f32,
    /// thermal conductivity of water (w/m/k)
    pub TKWAT: f32,
    /// thermal conductivity of ice (w/m/k)
    pub TKICE: f32,
    /// thermal conductivity of air (w/m/k) (not used MB: 20140718)
    pub TKAIR: f32,
    /// gas constant for dry air (j/kg/k)
    pub RAIR: f32,
    /// gas constant for  water vapor (j/kg/k)
    pub RW: f32,
    /// density of water (kg/m3)
    pub DENH2O: f32,
    /// density of ice (kg/m3)
    pub DENICE: f32,
    /// thermal conductivity of water in soil module (W/m/K)
    pub THKW: f32,
    /// thermal conductivity of for other soil components in soil module (W/m/K)
    pub THKO: f32,
    /// thermal conductivity of quartz in soil module (W/m/K)
    pub THKQTZ: f32,
    /// liquid water holding capacity for snowpack (m3/m3)
    pub SSI: f32,
    /// fractional snow covered area (FSNO) curve parameter
    pub MFSNO: f32,
    /// snow surface roughness length (m)
    pub Z0SNO: f32,
    /// new SWE required (QSNOW * dt) to fully cover old snow (mm)
    pub SWEMX: f32,
    /// tau0 from Yang97 eqn. 10a
    pub TAU0: f32,
    /// growth from vapor diffusion Yang97 eqn. 10b
    pub GRAIN_GROWTH: f32,
    /// extra growth near freezing Yang97 eqn. 10c
    pub EXTRA_GROWTH: f32,
    /// dirt and soot term Yang97 eqn. 10d
    pub DIRT_SOOT: f32,
    /// zenith angle snow albedo adjustment; b in Yang97 eqn. 15
    pub BATS_COSZ: f32,
    /// new snow visible albedo
    pub BATS_VIS_NEW: f32,
    /// new snow NIR albedo
    pub BATS_NIR_NEW: f32,
    /// age factor for diffuse visible snow albedo Yang97 eqn. 17
    pub BATS_VIS_AGE: f32,
    /// age factor for diffuse NIR snow albedo Yang97 eqn. 18
    pub BATS_NIR_AGE: f32,
    /// cosz factor for direct visible snow albedo Yang97 eqn. 15
    pub BATS_VIS_DIR: f32,
    /// cosz factor for direct NIR snow albedo Yang97 eqn. 16
    pub BATS_NIR_DIR: f32,
    /// surface resistence for snow [s/m]
    pub RSURF_SNOW: f32,
    /// exponent in the shape parameter for soil resistance option 1
    pub RSURF_EXP: f32,
    /// saturated soil albedo (1=vis, 2=nir)
    pub ALBSAT: Shifted<f32>,
    /// dry soil albedo (1=vis, 2=nir)
    pub ALBDRY: Shifted<f32>,
    /// Land ice albedo (1=vis, 2=nir)
    pub ALBICE: Shifted<f32>,
    /// Lake ice albedo (1=vis, 2=nir)
    pub ALBLAK: Shifted<f32>,
    /// two-stream parameter omega for snow (1=vis, 2=nir)
    pub OMEGAS: Shifted<f32>,
    /// two-stream parameter betad for snow
    pub BETADS: f32,
    /// two-stream parameter betaI for snow
    pub BETAIS: f32,
    /// emissivity of land surface (1=soil,2=lake)
    pub EG: Shifted<f32>,
    /// maximum lake water storage (mm)
    pub WSLMAX: f32,
    /// For snow water retention
    pub max_liq_mass_fraction: f32,
    /// snowpack water release timescale factor (1/s)
    pub SNOW_RET_FAC: f32,
    /// Number of shortwave bands (2, visible and NIR)
    pub NBAND: i32,
    /// MPE is nominally small to prevent dividing by zero error
    pub MPE: f32,
    /// Optimum transpiration air temperature [K]
    pub TOPT: f32,
    /// o2 partial pressure, from MPTABLE.TBL
    pub O2: f32,
    /// co2 partial pressure, from MPTABLE.TBL
    pub CO2: f32,
    /// matric potential for wilting point (m)  (orig a fixed param.)
    pub PSIWLT: f32,
    /// bottom condition for soil temp. (k)
    pub TBOT: f32,
    /// acceleration due to gravity (m/s2)
    pub GRAV: f32,
    /// user-defined rain-snow temperature threshold (°C)
    pub rain_snow_thresh: f32,
    /// maximum fractional snow-covered area
    pub SCAMAX: f32,
}

/// `huge(1.0)`, the value `InitAllocate` and `InitDefault` use for "not set yet".
const HUGE: f32 = f32::MAX;

impl Parameters {
    /// `Init` (`InitAllocate` + `InitDefault`) followed by `paramRead`.
    ///
    /// The Fortran splits these because `parameters` is a member of a type that must exist
    /// before the tables are read. Nothing observes the intermediate state, so they are one
    /// constructor here.
    pub fn new(namelist: &NamelistConfig, tables: &Tables) -> Result<Self, ConfigError> {
        let nsoil = namelist.nsoil;
        let isltyp = namelist.isltyp;
        let vegtyp = namelist.vegtyp;
        let soilcolor = namelist.soilcolor;

        // Bounded by the table's declared size, not by how many rows the file supplied.
        // A class between `nveg` and `MVT` is in bounds for the Fortran and reads back the
        // `-1.E36` sentinel, which some option paths test for -- so rejecting it here would
        // diverge. Past `MVT` the Fortran reads off the end of the array, which is where this
        // stops rather than following it.
        let in_range = |what: &'static str, value: i32, max: i32| {
            if (1..=max).contains(&value) {
                Ok(())
            } else {
                Err(ConfigError::TableTooLarge { what, value, max })
            }
        };
        in_range("isltyp", isltyp, crate::parameters_read::MAX_SOILTYP)?;
        in_range("vegtyp", vegtyp, crate::parameters_read::MVT)?;
        in_range("soilcolor", soilcolor, crate::parameters_read::MSC)?;

        // `InitAllocate` allocates these `nsoil` long and fills them with `huge(1.0)`;
        // `paramRead` then assigns a scalar to the whole array, so every layer ends up with
        // the same soil-class value. The fill is still reproduced: if nsoil were zero the
        // arrays would be empty either way, and the intermediate value is what a future
        // partial port would see.
        let profile = |v: f32| {
            let mut a = Shifted::ones(nsoil, HUGE);
            a.fill(v);
            a
        };

        let veg = &tables.veg;
        let soil = &tables.soil;
        let gen = &tables.gen;
        let rad = &tables.rad;
        let global = &tables.global;

        // `do ix = 1,12`: monthly LAI and SAI for this vegetation type.
        let mut LAIM = Shifted::ones(12, HUGE);
        let mut SAIM = Shifted::ones(12, HUGE);
        for ix in 1..=12 {
            LAIM[ix] = veg.laim[vegtyp][ix];
            SAIM[ix] = veg.saim[vegtyp][ix];
        }

        // Band 1 is visible, band 2 NIR.
        let band = |t: &Shifted<Shifted<f32>>, row: i32| {
            let mut a = Shifted::ones(2, HUGE);
            a[1] = t[row][1];
            a[2] = t[row][2];
            a
        };

        let dksat = profile(soil.dksat[isltyp]);
        let smcmax = profile(soil.smcmax[isltyp]);
        let smcref = profile(soil.smcref[isltyp]);

        let refkdt = gen.refkdt;
        let refdk = gen.refdk;
        // Kept as written: `refkdt * dksat(1) / refdk` associates left to right, and
        // `a * b / c` is not `a * (b / c)` in binary32. This same expression is duplicated
        // inline in two places in `bmi_noahowp.f90`; see docs/porting.md.
        let kdt = refkdt * dksat[1] / refdk;
        let frzx = 0.15 * (smcmax[1] / smcref[1]) * (0.412 / 0.468);

        // TFRZ is assigned a literal further down in paramRead and used here; the value is
        // the same one, not the constants module's.
        let TFRZ: f32 = 273.16;
        let rain_snow_thresh = match namelist.precip_phase_option {
            2 => TFRZ + 2.2,
            3 => TFRZ,
            5 | 6 => TFRZ + namelist.rain_snow_thresh,
            // "set to TFRZ as a backup"
            _ => TFRZ,
        };

        Ok(Self {
            bexp: profile(soil.bexp[isltyp]),
            smcmax,
            smcwlt: profile(soil.smcwlt[isltyp]),
            smcref,
            dksat,
            dwsat: profile(soil.dwsat[isltyp]),
            psisat: profile(soil.psisat[isltyp]),
            bvic: soil.bvic[isltyp],
            AXAJ: soil.axaj[isltyp],
            BXAJ: soil.bxaj[isltyp],
            XXAJ: soil.xxaj[isltyp],
            BBVIC: soil.bbvic[isltyp],
            G: soil.gdvic[isltyp],
            QUARTZ: soil.quartz[isltyp],

            LAIM,
            SAIM,
            CH2OP: veg.ch2op[vegtyp],
            HVT: veg.hvt[vegtyp],
            HVB: veg.hvb[vegtyp],
            TMIN: veg.tmin[vegtyp],
            SHDFAC: veg.shdfac[vegtyp],
            SHDMAX: veg.shdfac[vegtyp],
            Z0MVT: veg.z0mvt[vegtyp],
            RC: veg.rc[vegtyp],
            XL: veg.xl[vegtyp],
            BP: veg.bp[vegtyp],
            FOLNMX: veg.folnmx[vegtyp],
            QE25: veg.qe25[vegtyp],
            VCMX25: veg.vcmx25[vegtyp],
            MP: veg.mp[vegtyp],
            RGL: veg.rgl[vegtyp],
            RSMIN: veg.rs[vegtyp],
            HS: veg.hs[vegtyp],
            AKC: veg.akc[vegtyp],
            AKO: veg.ako[vegtyp],
            AVCMX: veg.avcmx[vegtyp],
            RSMAX: veg.rsmax[vegtyp],
            CWP: veg.cwpvt[vegtyp],
            C3PSN: veg.c3psn[vegtyp],
            DLEAF: veg.dleaf[vegtyp],
            KC25: veg.kc25[vegtyp],
            KO25: veg.ko25[vegtyp],
            MFSNO: veg.mfsno[vegtyp],
            // NROOT_TABLE is declared `real` while the field is `integer`, so the Fortran
            // truncates toward zero on assignment. `as i32` does the same.
            NROOT: veg.nroot[vegtyp] as i32,
            RHOL: band(&veg.rhol, vegtyp),
            RHOS: band(&veg.rhos, vegtyp),
            TAUL: band(&veg.taul, vegtyp),
            TAUS: band(&veg.taus, vegtyp),

            refkdt,
            refdk,
            kdt,
            frzx,
            csoil: gen.csoil,
            Z0: gen.z0,
            CZIL: gen.czil,
            ZBOT: gen.zbot,
            slope: gen.slope[1],

            SSI: global.ssi,
            Z0SNO: global.z0sno,
            SWEMX: global.swemx,
            TAU0: global.tau0,
            GRAIN_GROWTH: global.grain_growth,
            EXTRA_GROWTH: global.extra_growth,
            DIRT_SOOT: global.dirt_soot,
            BATS_COSZ: global.bats_cosz,
            BATS_VIS_NEW: global.bats_vis_new,
            BATS_NIR_NEW: global.bats_nir_new,
            BATS_VIS_AGE: global.bats_vis_age,
            BATS_NIR_AGE: global.bats_nir_age,
            BATS_VIS_DIR: global.bats_vis_dir,
            BATS_NIR_DIR: global.bats_nir_dir,
            RSURF_SNOW: global.rsurf_snow,
            RSURF_EXP: global.rsurf_exp,

            ALBSAT: band(&rad.albsat, soilcolor),
            ALBDRY: band(&rad.albdry, soilcolor),
            ALBICE: rad.albice.clone(),
            ALBLAK: rad.alblak.clone(),
            OMEGAS: rad.omegas.clone(),
            EG: rad.eg.clone(),
            BETADS: rad.betads,
            BETAIS: rad.betais,

            ISURBAN: veg.isurban,
            ISWATER: veg.iswater,
            ISBARREN: veg.isbarren,
            ISICE: veg.isice,
            ISCROP: veg.iscrop,
            EBLFOREST: veg.eblforest,
            NATURAL: veg.natural,
            LOW_DENSITY_RESIDENTIAL: veg.lcz_1,
            HIGH_DENSITY_RESIDENTIAL: veg.lcz_2,
            HIGH_INTENSITY_INDUSTRIAL: veg.lcz_3,

            urban_flag: false,
            timean: 10.5,
            fsatmx: 0.38,
            GRAV: 9.80616,
            SB: 5.67e-08,
            VKC: 0.40,
            TFRZ: 273.16,
            HSUB: 2.8440e06,
            HVAP: 2.5104e06,
            HFUS: 0.3336e06,
            CWAT: 4.188e06,
            CICE: 2.094e06,
            CPAIR: 1004.64,
            TKWAT: 0.6,
            TKICE: 2.2,
            TKAIR: 0.023,
            RAIR: 287.04,
            RW: 461.269,
            DENH2O: 1000.0,
            DENICE: 917.0,
            THKW: 0.57,
            THKO: 2.0,
            THKQTZ: 7.7,
            WSLMAX: 5000.0,
            max_liq_mass_fraction: 0.4,
            SNOW_RET_FAC: 5e-5,
            NBAND: 2,
            MPE: 1e-06,
            TOPT: 1e-06,
            CO2: 395e-06,
            O2: 0.209,
            PSIWLT: -150.0,
            TBOT: 263.0,

            rain_snow_thresh,
            ZWT_INIT: namelist.zwt,

            // `InitDefault`, which paramRead never overwrites.
            LAI: HUGE,
            SAI: HUGE,
            ELAI: HUGE,
            ESAI: HUGE,
            VAI: HUGE,
            FVEG: HUGE,
            VEG: true,
            SCAMAX: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters_read::{MAX_SOILTYP, MVT};

    /// Bit-for-bit agreement with the Fortran is checked in
    /// `tests/parameters_vs_fortran.rs`, over Bondville and a sweep of every table row. These
    /// cover the paths that fixture cannot reach.
    fn bondville() -> (NamelistConfig, Tables) {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
        let read = |n: &str| std::fs::read_to_string(format!("{dir}{n}")).expect("fixture");
        let namelist = NamelistConfig::parse(&read("namelist.input")).expect("namelist");
        let tables = Tables::parse(
            &read("MPTABLE.TBL"),
            &read("SOILPARM.TBL"),
            &read("GENPARM.TBL"),
            &namelist,
        )
        .expect("tables");
        (namelist, tables)
    }

    #[test]
    fn a_class_index_past_the_table_is_an_error() {
        let (namelist, tables) = bondville();
        for (what, mut n) in [
            ("vegtyp", namelist.clone()),
            ("isltyp", namelist.clone()),
            ("soilcolor", namelist.clone()),
        ] {
            match what {
                "vegtyp" => n.vegtyp = MVT + 1,
                "isltyp" => n.isltyp = MAX_SOILTYP + 1,
                _ => n.soilcolor = 9,
            }
            let err = Parameters::new(&n, &tables).unwrap_err();
            assert!(
                matches!(err, ConfigError::TableTooLarge { what: w, .. } if w == what),
                "{what}: {err}"
            );
        }
        for mut n in [namelist.clone(), namelist.clone()] {
            n.vegtyp = 0;
            assert!(Parameters::new(&n, &tables).is_err(), "zero is out of range");
        }
    }

    /// Past the rows the file supplies the tables hold `-1.E36`, which is in bounds for the
    /// Fortran. The derived parameters then go infinite, and that is the correct answer.
    #[test]
    fn unsupplied_rows_propagate_the_sentinel() {
        let (mut namelist, tables) = bondville();
        namelist.isltyp = MAX_SOILTYP; // well past SLCATS
        let p = Parameters::new(&namelist, &tables).expect("still constructs");

        assert_eq!(p.bexp[1], -1.0e36);
        assert!(p.kdt.is_infinite(), "kdt = refkdt * -1e36 / refdk");
        // frzx divides one sentinel by another, which is exactly 1.
        assert_eq!(p.frzx, 0.15 * 1.0 * (0.412 / 0.468));
    }

    /// `NROOT_TABLE` is `real` and the field is `integer`, so the Fortran truncates toward
    /// zero on assignment.
    #[test]
    fn nroot_truncates_rather_than_rounds() {
        let (namelist, tables) = bondville();
        let p = Parameters::new(&namelist, &tables).unwrap();
        let table = tables.veg.nroot[namelist.vegtyp];
        assert_eq!(p.NROOT, table.trunc() as i32);
    }

    /// Soil properties are a scalar table lookup assigned to the whole profile, so every layer
    /// carries the same value.
    #[test]
    fn soil_properties_fill_the_whole_profile() {
        let (namelist, tables) = bondville();
        let p = Parameters::new(&namelist, &tables).unwrap();
        assert_eq!(p.bexp.len(), namelist.nsoil as usize);
        assert!(p.bexp.iter().all(|&v| v == p.bexp[1]));
    }
}
