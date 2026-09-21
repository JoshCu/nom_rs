//! `InterceptionMain` over every `dveg` branch and every special vegetation type, against
//! `reference/interceptsweep_driver.f90`.
//!
//! Bondville runs `dynamic_veg_option = 1`, `vegtyp = 1`, `croptype = 0` for a whole year,
//! which leaves eight of the nine `dveg` branches in `PHENOLOGY` unreached, together with the
//! southern-hemisphere shift, the short-canopy snow burial, the water/barren/ice/urban
//! zeroing, the crop path, and -- because LAI and SAI never reach zero at Bondville -- the
//! entire buried-canopy branch of `CanopyWaterIntercept`.

#![allow(non_snake_case)]

use noahowp::difftest::{Call, Fixture, Record};
use noahowp::namelist_read::NamelistConfig;
use noahowp::options::Options;
use noahowp::parameters::Parameters;
use noahowp::parameters_read::Tables;
use noahowp::physics::interception::interception_main;
use noahowp::run::NoahOwp;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!("reading {path}: {e}\nregenerate with reference/build.sh --fixtures")
    })
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture_bytes(name)).expect("utf-8 fixture")
}

/// `((dveg * 32 + vegtyp) * 2 + croptype) * 100 + sample`, as `interceptsweep_driver.f90`
/// packs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Config {
    dveg: i32,
    vegtyp: i32,
    croptype: i32,
}

impl Config {
    fn unpack(id: i32) -> (Self, i32) {
        let sample = id % 100;
        let rest = id / 100;
        let croptype = rest % 2;
        let rest = rest / 2;
        (
            Config {
                dveg: rest / 32,
                vegtyp: rest % 32,
                croptype,
            },
            sample,
        )
    }
}

fn load_inputs(m: &mut NoahOwp, before: &Record) {
    let d = before.state("domain");
    m.domain.dt = d.f32("DT");
    m.domain.lat = d.f32("lat");
    m.domain.ist = d.i32("ist");

    let f = before.state("forcing");
    m.forcing.JULIAN = f.f32("JULIAN");
    m.forcing.YEARLEN = f.i32("YEARLEN");
    m.forcing.UU = f.f32("UU");
    m.forcing.VV = f.f32("VV");

    let e = before.state("energy");
    m.energy.TV = e.f32("TV");
    m.energy.TG = e.f32("TG");
    m.energy.IGS = e.f32("IGS");

    let w = before.state("water");
    m.water.SNOWH = w.f32("SNOWH");
    m.water.rain = w.f32("rain");
    m.water.snow = w.f32("snow");
    m.water.bdfall = w.f32("bdfall");
    m.water.FP = w.f32("FP");
    m.water.canliq = w.f32("canliq");
    m.water.canice = w.f32("canice");

    let p = before.state("paramstate");
    m.parameters.LAI = p.f32("LAI");
    m.parameters.SAI = p.f32("SAI");
    m.parameters.ELAI = p.f32("ELAI");
    m.parameters.ESAI = p.f32("ESAI");
    m.parameters.FVEG = p.f32("FVEG");
}

#[test]
fn every_dveg_branch_matches_the_fortran() {
    let base = NamelistConfig::parse(&fixture_text("namelist.input")).expect("namelist");
    let mptable = fixture_text("MPTABLE.TBL");
    let soilparm = fixture_text("SOILPARM.TBL");
    let genparm = fixture_text("GENPARM.TBL");

    let recording =
        Fixture::parse(&fixture_bytes("difftest/intercept_sweep.difftest")).expect("fixture");
    let pairs = recording.pairs(Call::Interception);
    assert!(!pairs.is_empty(), "no sweep records");

    let mut failures = Vec::new();
    let mut current: Option<(Config, NoahOwp)> = None;
    let mut configs = std::collections::HashSet::new();

    for (before, after) in &pairs {
        let (cfg, sample) = Config::unpack(before.itime);
        configs.insert(cfg);

        // Rebuilt whenever the configuration changes rather than per case: `paramRead` reads
        // the vegetation row, so a changed vegtyp means new LAIM, HVT, SHDFAC and urban_flag.
        if current.as_ref().map(|(c, _)| *c) != Some(cfg) {
            let mut nl = base.clone();
            nl.dynamic_veg_option = cfg.dveg;
            nl.vegtyp = cfg.vegtyp;
            nl.croptype = cfg.croptype;
            let tables = Tables::parse(&mptable, &soilparm, &genparm, &nl).expect("tables");
            let mut m = NoahOwp::new(base.clone(), &tables).expect("model");
            m.options = Options::new(&nl);
            m.parameters = Parameters::new(&nl, &tables).expect("parameters");
            m.domain.vegtyp = nl.vegtyp;
            m.domain.croptype = nl.croptype;
            current = Some((cfg, m));
        }
        let m = &mut current.as_mut().unwrap().1;

        load_inputs(m, before);
        interception_main(
            &m.domain,
            &m.options,
            &mut m.parameters,
            &m.forcing,
            &mut m.energy,
            &mut m.water,
        );

        let e = after.state("energy");
        let w = after.state("water");
        let p = after.state("paramstate");
        for (name, got, want) in [
            ("parameters%LAI", m.parameters.LAI, p.f32("LAI")),
            ("parameters%SAI", m.parameters.SAI, p.f32("SAI")),
            ("parameters%ELAI", m.parameters.ELAI, p.f32("ELAI")),
            ("parameters%ESAI", m.parameters.ESAI, p.f32("ESAI")),
            ("parameters%FVEG", m.parameters.FVEG, p.f32("FVEG")),
            ("energy%IGS", m.energy.IGS, e.f32("IGS")),
            ("water%QINTR", m.water.QINTR, w.f32("QINTR")),
            ("water%QDRIPR", m.water.QDRIPR, w.f32("QDRIPR")),
            ("water%QTHROR", m.water.QTHROR, w.f32("QTHROR")),
            ("water%QINTS", m.water.QINTS, w.f32("QINTS")),
            ("water%QDRIPS", m.water.QDRIPS, w.f32("QDRIPS")),
            ("water%QTHROS", m.water.QTHROS, w.f32("QTHROS")),
            ("water%QRAIN", m.water.QRAIN, w.f32("QRAIN")),
            ("water%QSNOW", m.water.QSNOW, w.f32("QSNOW")),
            ("water%SNOWHIN", m.water.SNOWHIN, w.f32("SNOWHIN")),
            ("water%canliq", m.water.canliq, w.f32("canliq")),
            ("water%canice", m.water.canice, w.f32("canice")),
            ("water%FWET", m.water.FWET, w.f32("FWET")),
            ("water%CMC", m.water.CMC, w.f32("CMC")),
        ] {
            if got.to_bits() != want.to_bits() {
                failures.push(format!(
                    "dveg {} vegtyp {} croptype {} case {sample}: {name} \
                     fortran {want:e} ({:08X}), rust {got:e} ({:08X})",
                    cfg.dveg,
                    cfg.vegtyp,
                    cfg.croptype,
                    want.to_bits(),
                    got.to_bits()
                ));
            }
        }
    }

    for dveg in 1..=9 {
        assert!(
            configs.iter().any(|c| c.dveg == dveg),
            "no cases recorded for dveg {dveg}"
        );
    }

    assert!(
        failures.is_empty(),
        "{} disagreements over {} sweep cases:\n  {}",
        failures.len(),
        pairs.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// The branches Bondville never enters, asserted reached rather than assumed.
#[test]
fn the_sweep_reaches_the_branches_bondville_misses() {
    let recording =
        Fixture::parse(&fixture_bytes("difftest/intercept_sweep.difftest")).expect("fixture");

    let (mut southern, mut no_canopy, mut buried_liq, mut buried_ice, mut lake_snow, mut fveg0) =
        (0, 0, 0, 0, 0, 0);
    for (before, after) in recording.pairs(Call::Interception) {
        let d = before.state("domain");
        let bp = before.state("paramstate");
        let ap = after.state("paramstate");
        let bw = before.state("water");
        let aw = after.state("water");

        if d.f32("lat") < 0.0 {
            southern += 1;
        }
        if ap.f32("ELAI") + ap.f32("ESAI") == 0.0 {
            no_canopy += 1;
            if bw.f32("canliq") > 0.0 && aw.f32("canliq") == 0.0 {
                buried_liq += 1;
            }
            if bw.f32("canice") > 0.0 && aw.f32("canice") == 0.0 {
                buried_ice += 1;
            }
        }
        if d.i32("ist") == 2 && aw.f32("QSNOW") == 0.0 && bp.f32("LAI") > 0.0 {
            lake_snow += 1;
        }
        if ap.f32("FVEG") == 0.0 {
            fveg0 += 1;
        }
    }

    // The four 0.05 cutoffs, identified by the signature each leaves in the post-state. ESAI
    // and ELAI are the awkward ones: ESAI is zeroed before ELAI is tested, and `ESAI == 0.0`
    // then forces ELAI to zero whichever way ELAI's own comparison falls -- so ELAI's cutoff
    // is only observable when ESAI survived. It took a dedicated pass over the snow depth with
    // SAI well clear of its own cutoff to reach it.
    let (mut sai_cut, mut lai_cut, mut esai_cut, mut elai_cut) = (0, 0, 0, 0);
    for (before, after) in recording.pairs(Call::Interception) {
        let (b, a) = (before.state("paramstate"), after.state("paramstate"));
        if b.f32("SAI") > 0.0 && b.f32("SAI") < 0.05 && a.f32("SAI") == 0.0 {
            sai_cut += 1;
        }
        if b.f32("LAI") > 0.0 && b.f32("LAI") < 0.05 && a.f32("LAI") == 0.0 {
            lai_cut += 1;
        }
        if a.f32("SAI") > 0.0 && a.f32("ESAI") == 0.0 {
            esai_cut += 1;
        }
        if a.f32("ESAI") > 0.0 && a.f32("ELAI") == 0.0 && a.f32("LAI") > 0.0 {
            elai_cut += 1;
        }
    }

    for (what, n) in [
        ("the SAI < 0.05 cutoff", sai_cut),
        ("the LAI < 0.05 cutoff", lai_cut),
        ("the ESAI < 0.05 cutoff", esai_cut),
        ("the ELAI < 0.05 cutoff, unmasked by ESAI", elai_cut),
        ("southern hemisphere day-of-year shift", southern),
        ("no canopy at all (ELAI + ESAI == 0)", no_canopy),
        ("buried canopy dumping its liquid water", buried_liq),
        ("buried canopy dumping its ice", buried_ice),
        ("lake with unfrozen ground suppressing snowfall", lake_snow),
        ("FVEG zeroed for urban or barren", fveg0),
    ] {
        assert!(n > 0, "the sweep never reaches: {what}");
    }
}
