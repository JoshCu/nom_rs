//! `advance_in_time`, run continuously from initialisation, against the Fortran.
//!
//! The per-call tests each load a recorded pre-state, so none of them can see a mistake in how
//! the calls are strung together -- the order, what `solve_noahowp` sets before them, or the
//! clock `advance_in_time` advances afterwards. This one loads nothing but forcing: the model
//! starts from `NoahOwp::new`, and after every step its complete state must equal the Fortran's
//! as `WaterMain` left it. The fixtures record the first 24 timesteps consecutively, so that is
//! the stretch the chain can be followed for.
//!
//! Forcing comes from the recorded `UtilitiesMain` pre-state, which is where the reference
//! build's `read_forcing_text` has just written it. That reader sets `PRCP`, so the steps take
//! [`PrecipInput::Total`].

use noahowp::difftest::bind;
use noahowp::difftest::{Call, Fixture};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::physics::atm_processing::PrecipInput;
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
fn consecutive_timesteps_reproduce_the_fortran() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let forcings = recording.pairs(Call::Utilities);
    let results = recording.pairs(Call::Water);

    let mut m = model();
    let mut failures = Vec::new();
    let mut steps = 0;

    for ((forced, _), (_, after)) in forcings.iter().zip(&results) {
        assert_eq!(forced.itime, after.itime, "fixture pairs out of step");
        if forced.itime != m.domain.itime {
            break; // the end of the consecutive run
        }

        let f = forced.state("forcing");
        m.forcing.UU = f.f32("UU");
        m.forcing.VV = f.f32("VV");
        m.forcing.SFCTMP = f.f32("SFCTMP");
        m.forcing.Q2 = f.f32("Q2");
        m.forcing.SFCPRS = f.f32("SFCPRS");
        m.forcing.SOLDN = f.f32("SOLDN");
        m.forcing.LWDN = f.f32("LWDN");
        m.forcing.PRCP = f.f32("PRCP");

        let itime = m.domain.itime;
        m.advance_in_time(PrecipInput::Total).expect("step");
        steps += 1;

        // The record is taken inside solve_noahowp, before advance_in_time moves the clock.
        assert_eq!(m.domain.itime, itime + 1);
        assert_eq!(m.domain.time_dbl, itime as f64 * m.domain.dt as f64);
        for d in bind::compare(&mut m, after, &["domain%itime", "domain%time_dbl"]) {
            failures.push(format!("itime {itime}: {d}"));
        }
    }

    assert!(steps >= 24, "only {steps} consecutive timesteps in the fixture");
    assert!(
        failures.is_empty(),
        "{} disagreements over {steps} consecutive timesteps:\n  {}",
        failures.len(),
        failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
    );
}
