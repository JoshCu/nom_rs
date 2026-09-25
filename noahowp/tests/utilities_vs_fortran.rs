//! `UtilitiesMain`, replayed against the Fortran's own recording.
//!
//! This is the differential contract the whole physics port is built on, exercised end to end
//! for the first time: take the state the reference Fortran had immediately before a physics
//! call, run the Rust, and require the result to match the state it had immediately after --
//! bit for bit, on every sampled timestep of a Bondville year.

use noahowp::difftest::{Call, Fixture};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::run::NoahOwp;
use noahowp::utilities::utilities_main;

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
fn utilities_main_reproduces_the_fortran() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut m = model();
    let mut failures = Vec::new();
    let pairs = recording.pairs(Call::Utilities);
    assert!(!pairs.is_empty(), "no UtilitiesMain records");

    for (before, after) in &pairs {
        // Load the inputs the call reads from the recorded pre-state, so the Rust starts from
        // exactly what the Fortran started from.
        let d = before.state("domain");
        m.domain.dt = d.f32("DT");
        m.domain.start_year = d.i32("start_year");
        m.domain.start_month = d.i32("start_month");
        m.domain.start_day = d.i32("start_day");
        m.domain.start_hour = d.i32("start_hour");
        m.domain.start_minute = d.i32("start_minute");
        m.domain.lat = d.f32("lat");
        m.domain.lon = d.f32("lon");
        m.domain.terrain_slope = d.f32("terrain_slope");
        m.domain.azimuth = d.f32("azimuth");

        utilities_main(before.itime, &m.domain, &mut m.forcing, &mut m.energy);

        let ea = after.state("energy");
        let fa = after.state("forcing");
        let at = before.itime;

        for (name, got, want) in [
            ("energy%COSZ", m.energy.COSZ, ea.f32("COSZ")),
            ("energy%COSZ_HORIZ", m.energy.COSZ_HORIZ, ea.f32("COSZ_HORIZ")),
            ("forcing%JULIAN", m.forcing.JULIAN, fa.f32("JULIAN")),
        ] {
            if got.to_bits() != want.to_bits() {
                failures.push(format!(
                    "itime {at}: {name} fortran {want:e} ({:08X}), rust {got:e} ({:08X})",
                    want.to_bits(),
                    got.to_bits()
                ));
            }
        }
        if m.forcing.YEARLEN != fa.i32("YEARLEN") {
            failures.push(format!(
                "itime {at}: forcing%YEARLEN fortran {}, rust {}",
                fa.i32("YEARLEN"),
                m.forcing.YEARLEN
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} sampled timesteps differ:\n  {}",
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

/// The call is supposed to touch four fields and nothing else -- none of them in `domain`,
/// since the current date is a local. Checking what the *Fortran* changed -- rather than what
/// the Rust changed -- is what catches an output the port never knew about, which would
/// otherwise look like agreement.
#[test]
fn utilities_main_touches_only_what_the_port_reproduces() {
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let mut unexpected: Vec<String> = Vec::new();

    for (before, after) in recording.pairs(Call::Utilities) {
        for (ty, allowed) in [
            ("domain", &[][..]),
            ("energy", &["COSZ", "COSZ_HORIZ"][..]),
            ("forcing", &["YEARLEN", "JULIAN"][..]),
            ("water", &[][..]),
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
        "UtilitiesMain also changes {unexpected:?}, which the port does not reproduce"
    );
}
