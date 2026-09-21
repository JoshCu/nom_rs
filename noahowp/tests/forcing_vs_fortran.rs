//! `ForcingMain` / `ATM`, replayed against the Fortran's own recording.
//!
//! Same contract as `utilities_vs_fortran.rs`: load the state the reference Fortran had
//! immediately before the call, run the Rust, require the result to match the state it had
//! immediately after, bit for bit, on every sampled timestep of a Bondville year.
//!
//! Bondville runs `precip_phase_option = 1`, so six of the seven `OPT_SNF` branches are
//! unreachable here. `atm_sweep_vs_fortran.rs` covers those.

#![allow(non_snake_case)]

use noahowp::difftest::{Call, Fixture, State};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::physics::atm_processing::PrecipInput;
use noahowp::physics::forcing_main::forcing_main;
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

/// Every forcing and energy field `ATM` reads, restored from the recorded pre-state so the
/// Rust starts from exactly what the Fortran started from.
fn load_inputs(m: &mut NoahOwp, f: &State, e: &State) {
    m.forcing.SFCPRS = f.f32("SFCPRS");
    m.forcing.SFCTMP = f.f32("SFCTMP");
    m.forcing.Q2 = f.f32("Q2");
    m.forcing.SOLDN = f.f32("SOLDN");
    m.forcing.PRCP = f.f32("PRCP");
    m.forcing.PRCPCONV = f.f32("PRCPCONV");
    m.forcing.PRCPNONC = f.f32("PRCPNONC");
    m.forcing.PRCPSHCV = f.f32("PRCPSHCV");
    m.forcing.PRCPSNOW = f.f32("PRCPSNOW");
    m.forcing.PRCPGRPL = f.f32("PRCPGRPL");
    m.forcing.PRCPHAIL = f.f32("PRCPHAIL");
    m.forcing.UU = f.f32("UU");
    m.forcing.VV = f.f32("VV");
    // FPICE survives from one timestep to the next under `case default`, so it is an input as
    // well as an output.
    m.forcing.FPICE = f.f32("FPICE");
    m.energy.COSZ = e.f32("COSZ");
    m.energy.COSZ_HORIZ = e.f32("COSZ_HORIZ");
}

/// The fields `ATM` writes, paired with what the Fortran left in them.
fn outputs(m: &NoahOwp, f: &State, e: &State, w: &State) -> Vec<(String, f32, f32)> {
    let mut out: Vec<(String, f32, f32)> = [
        ("forcing%THAIR", m.forcing.THAIR, f.f32("THAIR")),
        ("forcing%QAIR", m.forcing.QAIR, f.f32("QAIR")),
        ("forcing%EAIR", m.forcing.EAIR, f.f32("EAIR")),
        ("forcing%RHOAIR", m.forcing.RHOAIR, f.f32("RHOAIR")),
        ("forcing%O2PP", m.forcing.O2PP, f.f32("O2PP")),
        ("forcing%CO2PP", m.forcing.CO2PP, f.f32("CO2PP")),
        ("forcing%SWDOWN", m.forcing.SWDOWN, f.f32("SWDOWN")),
        ("forcing%PRCP", m.forcing.PRCP, f.f32("PRCP")),
        ("forcing%PRCPNONC", m.forcing.PRCPNONC, f.f32("PRCPNONC")),
        ("forcing%FPICE", m.forcing.FPICE, f.f32("FPICE")),
        ("forcing%UR", m.forcing.UR, f.f32("UR")),
        ("energy%TAH", m.energy.TAH, e.f32("TAH")),
        ("energy%EAH", m.energy.EAH, e.f32("EAH")),
        ("water%FP", m.water.FP, w.f32("FP")),
        ("water%rain", m.water.rain, w.f32("rain")),
        ("water%snow", m.water.snow, w.f32("snow")),
        ("water%bdfall", m.water.bdfall, w.f32("bdfall")),
    ]
    .iter()
    .map(|(n, got, want)| (n.to_string(), *got, *want))
    .collect();

    for (name, got, want) in [
        ("SOLAD", &m.forcing.SOLAD, f.f32s("SOLAD")),
        ("SOLAI", &m.forcing.SOLAI, f.f32s("SOLAI")),
    ] {
        for i in 1..=2 {
            out.push((
                format!("forcing%{name}({i})"),
                got[i],
                want[(i - 1) as usize],
            ));
        }
    }
    out
}

#[test]
fn forcing_main_reproduces_the_fortran() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut m = model();
    let mut failures = Vec::new();
    let pairs = recording.pairs(Call::Forcing);
    assert!(!pairs.is_empty(), "no ForcingMain records");

    for (before, after) in &pairs {
        load_inputs(&mut m, before.state("forcing"), before.state("energy"));

        // The fixtures come from the reference build, which leaves `NGEN_FORCING_ACTIVE`
        // undefined so that the ASCII reader stays in and the Bondville year runs standalone.
        forcing_main(
            &m.options,
            &m.parameters,
            &mut m.forcing,
            &mut m.energy,
            &mut m.water,
            PrecipInput::Total,
        );

        for (name, got, want) in outputs(
            &m,
            after.state("forcing"),
            after.state("energy"),
            after.state("water"),
        ) {
            if got.to_bits() != want.to_bits() {
                failures.push(format!(
                    "itime {}: {name} fortran {want:e} ({:08X}), rust {got:e} ({:08X})",
                    before.itime,
                    want.to_bits(),
                    got.to_bits()
                ));
            }
        }
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

/// What the *Fortran* changed, against what the port reproduces. A field `ATM` writes that the
/// port has never heard of would pass the test above -- nothing compares it -- and show up
/// here instead.
#[test]
fn forcing_main_touches_only_what_the_port_reproduces() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut unexpected: Vec<String> = Vec::new();

    for (before, after) in recording.pairs(Call::Forcing) {
        for (ty, allowed) in [
            ("domain", &[][..]),
            (
                "forcing",
                &[
                    "THAIR", "QAIR", "EAIR", "RHOAIR", "O2PP", "CO2PP", "SWDOWN", "SOLAD",
                    "SOLAI", "PRCP", "PRCPNONC", "FPICE", "UR",
                ][..],
            ),
            ("energy", &["TAH", "EAH"][..]),
            ("water", &["FP", "rain", "snow", "bdfall"][..]),
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
        "ATM also changes {unexpected:?}, which the port does not reproduce"
    );
}
