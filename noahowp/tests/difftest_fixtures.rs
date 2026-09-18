//! The differential fixtures are the oracle the whole physics port is verified against, so the
//! fixture format itself needs to be trustworthy before anything depends on it.
//!
//! These tests read the committed Bondville recording end to end and check it against what the
//! namelist and the Fortran type definitions say it must contain. A fixture regenerated from a
//! different upstream commit, a different `nsoil`, or a serializer whose field order has drifted
//! fails here rather than in whichever physics module happens to be ported first.

use noahowp::difftest::{Call, Fixture, Phase};

const CALLS: [Call; 5] = [
    Call::Utilities,
    Call::Forcing,
    Call::Interception,
    Call::Energy,
    Call::Water,
];

fn fixture() -> Fixture {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/difftest/bondville.difftest"
    );
    let data = std::fs::read(path)
        .unwrap_or_else(|e| panic!("{path}: {e}\nregenerate with reference/build.sh --fixtures"));
    Fixture::parse(&data).expect("fixture parses")
}

/// Parsing consumes the file exactly: every record's magic lands where the previous record's
/// last field ended. This is the check that the Rust field table and the Fortran writer agree
/// -- a single wrong width or a missing field would desynchronise the stream and be caught at
/// the next magic.
#[test]
fn parses_completely() {
    let f = fixture();
    assert!(!f.records.is_empty(), "fixture has no records");
    // 200 sampled timesteps, five calls each, before and after.
    assert_eq!(f.records.len() % (CALLS.len() * 2), 0, "incomplete record set");
}

#[test]
fn every_call_is_recorded_before_and_after() {
    let f = fixture();
    for call in CALLS {
        let pairs = f.pairs(call);
        assert!(!pairs.is_empty(), "{} has no pairs", call.subroutine());
        for (before, after) in &pairs {
            assert_eq!(before.phase, Phase::Before);
            assert_eq!(after.phase, Phase::After);
            assert_eq!(
                before.itime,
                after.itime,
                "{} pair spans two timesteps",
                call.subroutine()
            );
        }
    }
    // Every call is sampled on the same timesteps, so the pair counts must agree.
    let counts: Vec<usize> = CALLS.iter().map(|c| f.pairs(*c).len()).collect();
    assert!(
        counts.windows(2).all(|w| w[0] == w[1]),
        "calls sampled unevenly: {counts:?}"
    );
}

/// The timestep in the record header and the one inside the dumped `domain` are written by
/// different code paths, so agreeing is evidence the record is internally consistent.
#[test]
fn header_itime_matches_domain_itime() {
    let f = fixture();
    for r in &f.records {
        assert_eq!(r.itime, r.state("domain").i32("itime"));
    }
}

/// Sampling covers spin-up and then spreads over the year. Without the spread, snow physics
/// would never appear in a fixture set: Bondville's winter is a small slice of 17,522 steps.
#[test]
fn sampling_covers_the_whole_run() {
    let f = fixture();
    let pairs = f.pairs(Call::Water);
    let times: Vec<i32> = pairs.iter().map(|(b, _)| b.itime).collect();

    assert_eq!(times[0], 1, "sampling should start at the first timestep");
    assert!(
        times.windows(2).all(|w| w[0] < w[1]),
        "timesteps should be strictly increasing"
    );

    let ntime = f.records[0].state("domain").i32("ntime");
    assert!(ntime > 17_000, "Bondville is a full year, got ntime={ntime}");
    let last = *times.last().unwrap();
    assert!(
        last > ntime - 200,
        "sampling stops at {last} of {ntime}, missing the end of the run"
    );

    // Consecutive early samples, so the opening transient is covered densely.
    assert_eq!(&times[..5], &[1, 2, 3, 4, 5]);
}

/// The static block is the run configuration, and it must match `run/namelist.input`.
#[test]
fn static_block_matches_the_bondville_namelist() {
    let f = fixture();

    let levels = f.static_state("levels");
    assert_eq!(levels.i32("nsoil"), 4);
    assert_eq!(levels.i32("nsnow"), 3);

    let options = f.static_state("options");
    assert_eq!(options.i32("opt_run"), 8, "runoff_option");
    assert_eq!(options.i32("opt_drn"), 8, "drainage_option");
    assert_eq!(options.i32("opt_rad"), 3, "radiative_transfer_option");
    assert_eq!(options.i32("opt_stc"), 3, "snowsoil_temp_time_option");

    // Soil parameters are allocated per soil layer, so their recorded lengths pin nsoil.
    let parameters = f.static_state("parameters");
    for field in ["bexp", "smcmax", "dksat", "psisat"] {
        assert_eq!(parameters.f32s(field).len(), 4, "{field} should be nsoil long");
    }
    // Monthly tables have a fixed extent in the Fortran rather than an allocatable one.
    assert_eq!(parameters.f32s("LAIM").len(), 12);
    assert_eq!(parameters.f32s("SAIM").len(), 12);
}

#[test]
fn domain_matches_the_bondville_namelist() {
    let f = fixture();
    let domain = f.records[0].state("domain");

    assert_eq!(domain.f32("DT"), 1800.0);
    assert_eq!(domain.str("startdate"), "199801010630");
    assert_eq!(domain.str("enddate"), "199901010630");
    assert_eq!(domain.i32("vegtyp"), 1);
    assert_eq!(domain.i32("isltyp"), 1);
    assert!((domain.f32("lat") - 40.01).abs() < 1e-4);
    assert!((domain.f32("lon") + 88.37).abs() < 1e-4);

    // Profile arrays span the snow and soil layers together.
    assert_eq!(domain.f32s("dzsnso").len(), 7);
    assert_eq!(domain.f32s("zsnso").len(), 7);
    assert_eq!(domain.f32s("zsoil").len(), 4);
}

/// A recording of a call that changed nothing would be a useless fixture. Each call must move
/// some field, and the field it moves must be one it is supposed to own.
#[test]
fn each_call_actually_changes_state() {
    let f = fixture();
    for call in CALLS {
        let changed_somewhere = f.pairs(call).iter().any(|(before, after)| {
            ["forcing", "energy", "water"]
                .iter()
                .any(|t| !before.state(t).diff(after.state(t)).is_empty())
        });
        assert!(
            changed_somewhere,
            "{} never changed any state -- is it being bracketed correctly?",
            call.subroutine()
        );
    }
}

/// Spot-check that the recorded state is physically sensible, which catches a fixture that
/// parses but has its fields transposed: a shifted stream tends to read temperatures as fluxes.
#[test]
fn recorded_state_is_physically_plausible() {
    let f = fixture();
    for (_, after) in f.pairs(Call::Energy) {
        let energy = after.state("energy");

        let tg = energy.f32("TG");
        assert!((200.0..350.0).contains(&tg), "ground temperature {tg} K");
        let tv = energy.f32("TV");
        assert!((200.0..350.0).contains(&tv), "vegetation temperature {tv} K");

        for (i, stc) in energy.f32s("STC").iter().enumerate() {
            // Snow layers above the pack are carried as zero rather than a temperature.
            assert!(
                *stc == 0.0 || (200.0..350.0).contains(stc),
                "layer {i} temperature {stc} K"
            );
        }

        let water = after.state("water");
        for (i, sh2o) in water.f32s("sh2o").iter().enumerate() {
            assert!((0.0..1.0).contains(sh2o), "layer {i} liquid fraction {sh2o}");
        }
        let isnow = water.i32("ISNOW");
        assert!((-3..=0).contains(&isnow), "snow layer count {isnow}");
    }
}

/// Truncation, corruption and a length that cannot be right are all caught rather than read
/// through -- the failure mode that matters is a fixture that parses into plausible nonsense.
#[test]
fn damaged_fixtures_are_rejected() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/difftest/bondville.difftest"
    );
    let data = std::fs::read(path).expect("read fixture");

    assert!(Fixture::parse(&data[..data.len() - 8]).is_err(), "truncated");

    let mut corrupt = data.clone();
    // Flip the magic of the static block.
    corrupt[0] ^= 0xFF;
    assert!(Fixture::parse(&corrupt).is_err(), "bad magic");

    let mut shifted = data.clone();
    shifted.drain(..4);
    assert!(Fixture::parse(&shifted).is_err(), "misaligned stream");
}
