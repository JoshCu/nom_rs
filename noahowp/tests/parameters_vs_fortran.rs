//! Every field of `parameters_type`, against the Fortran's own assembled values.
//!
//! Two fixtures, both produced by `reference/build.sh --fixtures`:
//!
//! - the Bondville run's static block, which holds `parameters` exactly as the reference
//!   Fortran built it for the shipped configuration;
//! - a sweep over every vegetation type, soil texture and soil colour the tables can hold,
//!   under both vegetation classifications, because Bondville exercises exactly one of each
//!   and would leave a misread column invisible.
//!
//! All 134 fields are compared, on raw bit patterns. `assert_eq!` on `f32` would accept a
//! value that differs in the last bit, which is the failure this port exists to prevent.

use noahowp::difftest::{parse_param_sweep, Fixture, State};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters::Parameters;
use noahowp::parameters_read::Tables;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!("reading {path}: {e}\nregenerate with reference/build.sh --fixtures")
    })
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture_bytes(name)).expect("utf-8 fixture")
}

fn namelist() -> NamelistConfig {
    NamelistConfig::parse(&fixture_text("namelist.input")).expect("namelist")
}

fn tables(namelist: &NamelistConfig) -> Tables {
    Tables::parse(
        &fixture_text("MPTABLE.TBL"),
        &fixture_text("SOILPARM.TBL"),
        &fixture_text("GENPARM.TBL"),
        namelist,
    )
    .expect("parameter tables")
}

/// Collects every mismatch instead of stopping at the first, so one run names everything that
/// is wrong. A single failing field is usually a typo; forty is a shifted column.
#[derive(Default)]
struct Report(Vec<String>);

impl Report {
    fn f32(&mut self, state: &State, name: &str, got: f32) {
        let want = state.f32(name);
        if got.to_bits() != want.to_bits() {
            self.0.push(format!(
                "{name}: fortran {want:e} ({:08X}), rust {got:e} ({:08X})",
                want.to_bits(),
                got.to_bits()
            ));
        }
    }

    fn i32(&mut self, state: &State, name: &str, got: i32) {
        let want = state.i32(name);
        if got != want {
            self.0.push(format!("{name}: fortran {want}, rust {got}"));
        }
    }

    fn bool(&mut self, state: &State, name: &str, got: bool) {
        let want = state.bool(name);
        if got != want {
            self.0.push(format!("{name}: fortran {want}, rust {got}"));
        }
    }

    fn f32s(&mut self, state: &State, name: &str, got: &[f32]) {
        let want = state.f32s(name);
        if got.len() != want.len() {
            self.0.push(format!(
                "{name}: fortran has {} elements, rust {}",
                want.len(),
                got.len()
            ));
            return;
        }
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            if g.to_bits() != w.to_bits() {
                self.0.push(format!(
                    "{name}({}): fortran {w:e} ({:08X}), rust {g:e} ({:08X})",
                    i + 1,
                    w.to_bits(),
                    g.to_bits()
                ));
            }
        }
    }

    fn finish(self, what: &str) {
        assert!(
            self.0.is_empty(),
            "{} field(s) of {what} differ from the Fortran:\n  {}",
            self.0.len(),
            self.0.join("\n  ")
        );
    }
}

/// Every field, in declaration order. Generated from `ParametersType.f90`, so a field added
/// upstream is a compile error here rather than a silently unchecked value.
fn compare(p: &Parameters, state: &State, r: &mut Report) {
    r.f32s(state, "bexp", p.bexp.as_slice());
    r.f32s(state, "smcmax", p.smcmax.as_slice());
    r.f32s(state, "smcwlt", p.smcwlt.as_slice());
    r.f32s(state, "smcref", p.smcref.as_slice());
    r.f32s(state, "dksat", p.dksat.as_slice());
    r.f32s(state, "dwsat", p.dwsat.as_slice());
    r.f32s(state, "psisat", p.psisat.as_slice());
    r.f32(state, "bvic", p.bvic);
    r.f32(state, "AXAJ", p.AXAJ);
    r.f32(state, "BXAJ", p.BXAJ);
    r.f32(state, "XXAJ", p.XXAJ);
    r.f32(state, "BBVIC", p.BBVIC);
    r.f32(state, "G", p.G);
    r.f32(state, "QUARTZ", p.QUARTZ);
    r.f32(state, "kdt", p.kdt);
    r.f32(state, "refkdt", p.refkdt);
    r.f32(state, "refdk", p.refdk);
    r.f32(state, "csoil", p.csoil);
    r.f32(state, "Z0", p.Z0);
    r.f32(state, "CZIL", p.CZIL);
    r.f32(state, "ZBOT", p.ZBOT);
    r.f32(state, "frzx", p.frzx);
    r.f32(state, "slope", p.slope);
    r.f32(state, "timean", p.timean);
    r.f32(state, "fsatmx", p.fsatmx);
    r.f32(state, "ZWT_INIT", p.ZWT_INIT);
    r.bool(state, "urban_flag", p.urban_flag);
    r.f32s(state, "LAIM", p.LAIM.as_slice());
    r.f32s(state, "SAIM", p.SAIM.as_slice());
    r.f32(state, "LAI", p.LAI);
    r.f32(state, "SAI", p.SAI);
    r.f32(state, "CH2OP", p.CH2OP);
    r.i32(state, "NROOT", p.NROOT);
    r.f32(state, "HVT", p.HVT);
    r.f32(state, "HVB", p.HVB);
    r.f32(state, "TMIN", p.TMIN);
    r.f32(state, "SHDFAC", p.SHDFAC);
    r.f32(state, "SHDMAX", p.SHDMAX);
    r.f32(state, "Z0MVT", p.Z0MVT);
    r.f32(state, "RC", p.RC);
    r.f32(state, "XL", p.XL);
    r.f32(state, "BP", p.BP);
    r.f32(state, "FOLNMX", p.FOLNMX);
    r.f32(state, "QE25", p.QE25);
    r.f32(state, "VCMX25", p.VCMX25);
    r.f32(state, "MP", p.MP);
    r.f32(state, "RGL", p.RGL);
    r.f32(state, "RSMIN", p.RSMIN);
    r.f32(state, "HS", p.HS);
    r.f32(state, "AKC", p.AKC);
    r.f32(state, "AKO", p.AKO);
    r.f32(state, "AVCMX", p.AVCMX);
    r.f32(state, "RSMAX", p.RSMAX);
    r.f32(state, "CWP", p.CWP);
    r.f32(state, "C3PSN", p.C3PSN);
    r.f32(state, "DLEAF", p.DLEAF);
    r.f32(state, "KC25", p.KC25);
    r.f32(state, "KO25", p.KO25);
    r.f32(state, "ELAI", p.ELAI);
    r.f32(state, "ESAI", p.ESAI);
    r.f32(state, "VAI", p.VAI);
    r.bool(state, "VEG", p.VEG);
    r.f32(state, "FVEG", p.FVEG);
    r.f32s(state, "RHOL", p.RHOL.as_slice());
    r.f32s(state, "RHOS", p.RHOS.as_slice());
    r.f32s(state, "TAUL", p.TAUL.as_slice());
    r.f32s(state, "TAUS", p.TAUS.as_slice());
    r.i32(state, "ISURBAN", p.ISURBAN);
    r.i32(state, "ISWATER", p.ISWATER);
    r.i32(state, "ISBARREN", p.ISBARREN);
    r.i32(state, "ISICE", p.ISICE);
    r.i32(state, "ISCROP", p.ISCROP);
    r.i32(state, "EBLFOREST", p.EBLFOREST);
    r.i32(state, "NATURAL", p.NATURAL);
    r.i32(state, "LOW_DENSITY_RESIDENTIAL", p.LOW_DENSITY_RESIDENTIAL);
    r.i32(state, "HIGH_DENSITY_RESIDENTIAL", p.HIGH_DENSITY_RESIDENTIAL);
    r.i32(state, "HIGH_INTENSITY_INDUSTRIAL", p.HIGH_INTENSITY_INDUSTRIAL);
    r.f32(state, "SB", p.SB);
    r.f32(state, "VKC", p.VKC);
    r.f32(state, "TFRZ", p.TFRZ);
    r.f32(state, "HSUB", p.HSUB);
    r.f32(state, "HVAP", p.HVAP);
    r.f32(state, "HFUS", p.HFUS);
    r.f32(state, "CWAT", p.CWAT);
    r.f32(state, "CICE", p.CICE);
    r.f32(state, "CPAIR", p.CPAIR);
    r.f32(state, "TKWAT", p.TKWAT);
    r.f32(state, "TKICE", p.TKICE);
    r.f32(state, "TKAIR", p.TKAIR);
    r.f32(state, "RAIR", p.RAIR);
    r.f32(state, "RW", p.RW);
    r.f32(state, "DENH2O", p.DENH2O);
    r.f32(state, "DENICE", p.DENICE);
    r.f32(state, "THKW", p.THKW);
    r.f32(state, "THKO", p.THKO);
    r.f32(state, "THKQTZ", p.THKQTZ);
    r.f32(state, "SSI", p.SSI);
    r.f32(state, "MFSNO", p.MFSNO);
    r.f32(state, "Z0SNO", p.Z0SNO);
    r.f32(state, "SWEMX", p.SWEMX);
    r.f32(state, "TAU0", p.TAU0);
    r.f32(state, "GRAIN_GROWTH", p.GRAIN_GROWTH);
    r.f32(state, "EXTRA_GROWTH", p.EXTRA_GROWTH);
    r.f32(state, "DIRT_SOOT", p.DIRT_SOOT);
    r.f32(state, "BATS_COSZ", p.BATS_COSZ);
    r.f32(state, "BATS_VIS_NEW", p.BATS_VIS_NEW);
    r.f32(state, "BATS_NIR_NEW", p.BATS_NIR_NEW);
    r.f32(state, "BATS_VIS_AGE", p.BATS_VIS_AGE);
    r.f32(state, "BATS_NIR_AGE", p.BATS_NIR_AGE);
    r.f32(state, "BATS_VIS_DIR", p.BATS_VIS_DIR);
    r.f32(state, "BATS_NIR_DIR", p.BATS_NIR_DIR);
    r.f32(state, "RSURF_SNOW", p.RSURF_SNOW);
    r.f32(state, "RSURF_EXP", p.RSURF_EXP);
    r.f32s(state, "ALBSAT", p.ALBSAT.as_slice());
    r.f32s(state, "ALBDRY", p.ALBDRY.as_slice());
    r.f32s(state, "ALBICE", p.ALBICE.as_slice());
    r.f32s(state, "ALBLAK", p.ALBLAK.as_slice());
    r.f32s(state, "OMEGAS", p.OMEGAS.as_slice());
    r.f32(state, "BETADS", p.BETADS);
    r.f32(state, "BETAIS", p.BETAIS);
    r.f32s(state, "EG", p.EG.as_slice());
    r.f32(state, "WSLMAX", p.WSLMAX);
    r.f32(state, "max_liq_mass_fraction", p.max_liq_mass_fraction);
    r.f32(state, "SNOW_RET_FAC", p.SNOW_RET_FAC);
    r.i32(state, "NBAND", p.NBAND);
    r.f32(state, "MPE", p.MPE);
    r.f32(state, "TOPT", p.TOPT);
    r.f32(state, "O2", p.O2);
    r.f32(state, "CO2", p.CO2);
    r.f32(state, "PSIWLT", p.PSIWLT);
    r.f32(state, "TBOT", p.TBOT);
    r.f32(state, "GRAV", p.GRAV);
    r.f32(state, "rain_snow_thresh", p.rain_snow_thresh);
    r.f32(state, "SCAMAX", p.SCAMAX);
}

#[test]
fn bondville_parameters_match_the_fortran() {
    let namelist = namelist();
    let p = Parameters::new(&namelist, &tables(&namelist)).expect("parameters");
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");

    let mut r = Report::default();
    compare(&p, recording.static_state("parameters"), &mut r);
    r.finish("the Bondville parameters");
}

/// Every table row, under both vegetation classifications -- including class indices past the
/// number of rows the file supplies, where the Fortran returns `-1.E36` and the derived
/// parameters go infinite. Reproducing that is part of the contract, so those cases are
/// compared like any other rather than skipped.
#[test]
fn every_table_row_matches_the_fortran() {
    let cases = parse_param_sweep(&fixture_bytes("difftest/param_sweep.difftest")).expect("sweep");
    assert!(!cases.is_empty(), "sweep fixture is empty");

    let base = namelist();
    let mut r = Report::default();
    let mut checked = 0;

    for dataset in [
        noahowp::difftest::VegDataset::Usgs,
        noahowp::difftest::VegDataset::ModifiedIgbpModisNoah,
    ] {
        let mut namelist = base.clone();
        namelist.veg_class_name = dataset.name().to_string();
        // The tables depend on veg_class_name, so they are re-read once per dataset rather
        // than once per case.
        let tables = tables(&namelist);

        for case in cases.iter().filter(|c| c.dataset == dataset) {
            namelist.vegtyp = case.vegtyp;
            namelist.isltyp = case.isltyp;
            namelist.soilcolor = case.soilcolor;
            let p = Parameters::new(&namelist, &tables).unwrap_or_else(|e| {
                panic!(
                    "{} vegtyp={} isltyp={} soilcolor={}: {e}",
                    dataset.name(),
                    case.vegtyp,
                    case.isltyp,
                    case.soilcolor
                )
            });

            let before = r.0.len();
            compare(&p, &case.parameters, &mut r);
            // Tag the mismatches this case produced, so the message says which one failed.
            for line in &mut r.0[before..] {
                *line = format!(
                    "[{} vegtyp={} isltyp={} soilcolor={}] {line}",
                    dataset.name(),
                    case.vegtyp,
                    case.isltyp,
                    case.soilcolor
                );
            }
            checked += 1;
        }
    }

    assert_eq!(checked, cases.len(), "not every sweep case was checked");
    // 27 vegetation types + 30 soil textures + 8 soil colours, for each of two datasets.
    assert_eq!(checked, 130, "sweep no longer covers what it used to");
    r.finish("the parameter sweep");
}
