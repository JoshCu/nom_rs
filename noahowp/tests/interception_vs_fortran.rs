//! `InterceptionMain`, replayed against the Fortran's own recording.
//!
//! Same contract as `utilities_vs_fortran.rs` and `forcing_vs_fortran.rs`: load the recorded
//! pre-state, run the Rust, require the post-state to match bit for bit on every sampled
//! timestep of a Bondville year.
//!
//! This is the first call whose inputs include `parameters` fields the physics itself writes --
//! `LAI`, `SAI`, `ELAI`, `ESAI`, `FVEG` -- so they are restored from the record's `paramstate`
//! block rather than from `paramRead`.

#![allow(non_snake_case)]

use noahowp::difftest::{Call, Fixture, Record};
use noahowp::namelist_read::NamelistConfig;
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

fn model() -> NoahOwp {
    let namelist = NamelistConfig::parse(&fixture_text("namelist.input")).expect("namelist");
    let tables = Tables::parse(
        &fixture_text("MPTABLE.TBL"),
        &fixture_text("SOILPARM.TBL"),
        &fixture_text("GENPARM.TBL"),
        &namelist,
    )
    .expect("tables");
    NoahOwp::new(namelist, &tables).expect("model")
}

fn load_inputs(m: &mut NoahOwp, before: &Record) {
    let d = before.state("domain");
    m.domain.dt = d.f32("DT");
    m.domain.lat = d.f32("lat");
    m.domain.vegtyp = d.i32("vegtyp");
    m.domain.croptype = d.i32("croptype");
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

    // The five parameter components PHENOLOGY itself maintains. Feeding them from `paramRead`
    // instead would mean replaying every timestep from a state no timestep after the first
    // ever has.
    let p = before.state("paramstate");
    m.parameters.LAI = p.f32("LAI");
    m.parameters.SAI = p.f32("SAI");
    m.parameters.ELAI = p.f32("ELAI");
    m.parameters.ESAI = p.f32("ESAI");
    m.parameters.FVEG = p.f32("FVEG");
}

fn compare(m: &NoahOwp, after: &Record, itime: i32, failures: &mut Vec<String>) {
    let e = after.state("energy");
    let w = after.state("water");
    let p = after.state("paramstate");

    let fields: Vec<(&str, f32, f32)> = vec![
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
    ];

    for (name, got, want) in fields {
        if got.to_bits() != want.to_bits() {
            failures.push(format!(
                "itime {itime}: {name} fortran {want:e} ({:08X}), rust {got:e} ({:08X})",
                want.to_bits(),
                got.to_bits()
            ));
        }
    }
}

#[test]
fn interception_main_reproduces_the_fortran() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut m = model();
    let mut failures = Vec::new();
    let pairs = recording.pairs(Call::Interception);
    assert!(!pairs.is_empty(), "no InterceptionMain records");

    for (before, after) in &pairs {
        load_inputs(&mut m, before);
        interception_main(
            &m.domain,
            &m.options,
            &mut m.parameters,
            &m.forcing,
            &mut m.energy,
            &mut m.water,
        );
        compare(&m, after, before.itime, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "{} disagreements over {} sampled timesteps:\n  {}",
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

/// What the *Fortran* changed, against what the port reproduces.
#[test]
fn interception_main_touches_only_what_the_port_reproduces() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut unexpected: Vec<String> = Vec::new();

    for (before, after) in recording.pairs(Call::Interception) {
        for (ty, allowed) in [
            ("domain", &[][..]),
            ("forcing", &[][..]),
            ("energy", &["IGS"][..]),
            (
                "water",
                &[
                    "QINTR", "QDRIPR", "QTHROR", "QINTS", "QDRIPS", "QTHROS", "QRAIN", "QSNOW",
                    "SNOWHIN", "canliq", "canice", "FWET", "CMC",
                ][..],
            ),
            ("paramstate", &["LAI", "SAI", "ELAI", "ESAI", "FVEG"][..]),
        ] {
            for field in before.state(ty).diff(after.state(ty)) {
                if !allowed.contains(&field) {
                    unexpected.push(format!("{ty}%{field}"));
                }
            }
        }
    }

    unexpected.sort();
    unexpected.dedup();
    assert!(
        unexpected.is_empty(),
        "InterceptionMain also changes {unexpected:?}, which the port does not reproduce"
    );
}
