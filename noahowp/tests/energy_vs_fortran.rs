//! `EnergyMain`, replayed against the Fortran's own recording.
//!
//! Same contract as `interception_vs_fortran.rs`, with one difference in method: the complete
//! recorded pre-state -- every field of `domain`, `forcing`, `energy`, `water` and the
//! per-timestep parameter components -- is loaded, the Rust runs, and the complete recorded
//! post-state must match bit for bit. Nothing is listed by hand, so a field the Fortran
//! changes that the port does not (or the reverse) is a failure rather than an oversight.

use noahowp::difftest::bind;
use noahowp::difftest::{Call, Fixture};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::physics::energy_main::energy_main;
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

#[test]
fn energy_main_reproduces_the_fortran() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut m = model();
    let mut failures = Vec::new();
    let pairs = recording.pairs(Call::Energy);
    assert!(!pairs.is_empty(), "no EnergyMain records");

    for (before, after) in &pairs {
        bind::load(&mut m, before);
        let (levels, options) = (m.levels, m.options.clone());
        energy_main(
            &mut m.domain,
            &levels,
            &options,
            &mut m.parameters,
            &m.forcing,
            &mut m.energy,
            &mut m.water,
        )
        .expect("EnergyMain stopped");
        for f in bind::compare(&mut m, after, &[]) {
            failures.push(format!("itime {}: {f}", before.itime));
        }
    }

    assert!(
        failures.is_empty(),
        "{} disagreements over {} sampled timesteps:\n  {}",
        failures.len(),
        pairs.len(),
        failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
    );
}
