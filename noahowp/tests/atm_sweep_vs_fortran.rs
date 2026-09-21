//! `ATM` over every `OPT_SNF` branch, against `reference/atmsweep_driver.f90`.
//!
//! Bondville runs `precip_phase_option = 1` for a whole year, so `forcing_vs_fortran.rs` never
//! enters six of the seven precipitation-phase schemes, the wet-bulb calculation, the logistic
//! regression, or the direct-irradiance clamp in the shortwave split. The port could have all
//! of them wrong and the year-long recording would still agree. This is the same replay
//! contract applied to a fixture built to reach them.
//!
//! Each case carries its option in the record's `itime` slot as `opt_snf * 100000 + sample`;
//! every other input is read back out of the recorded pre-state, so the two sides agree on one
//! integer and nothing else.

#![allow(non_snake_case)]

use noahowp::difftest::{Call, Fixture, State};
use noahowp::namelist_read::NamelistConfig;
use noahowp::options::Options;
use noahowp::parameters::Parameters;
use noahowp::parameters_read::Tables;
use noahowp::physics::atm_processing::{atm, PrecipInput};
use noahowp::water::Water;
use noahowp::{energy::Energy, forcing::Forcing};

/// Matches `atmsweep_driver.f90`: neither zero nor the Bondville default, so options 5 and 6
/// produce a threshold distinguishable from option 3's.
const SWEEP_RAIN_SNOW_THRESH: f32 = 1.5;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!("reading {path}: {e}\nregenerate with reference/build.sh --fixtures")
    })
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture_bytes(name)).expect("utf-8 fixture")
}

fn load(m: &mut Forcing, e: &mut Energy, f: &State, es: &State) {
    m.SFCPRS = f.f32("SFCPRS");
    m.SFCTMP = f.f32("SFCTMP");
    m.Q2 = f.f32("Q2");
    m.SOLDN = f.f32("SOLDN");
    m.PRCP = f.f32("PRCP");
    m.PRCPCONV = f.f32("PRCPCONV");
    m.PRCPNONC = f.f32("PRCPNONC");
    m.PRCPSHCV = f.f32("PRCPSHCV");
    m.PRCPSNOW = f.f32("PRCPSNOW");
    m.PRCPGRPL = f.f32("PRCPGRPL");
    m.PRCPHAIL = f.f32("PRCPHAIL");
    m.UU = f.f32("UU");
    m.VV = f.f32("VV");
    m.FPICE = f.f32("FPICE");
    e.COSZ = es.f32("COSZ");
    e.COSZ_HORIZ = es.f32("COSZ_HORIZ");
}

/// Bit equality, with NaN counted as equal to NaN of the same payload. `OPT_SNF == 4` with no
/// frozen precipitation divides 0 by 0 and puts a NaN in `bdfall`; reproducing that is part of
/// the contract, so the comparison has to be able to express it.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits()
}

#[test]
fn every_precip_phase_option_matches_the_fortran() {
    let namelist = NamelistConfig::parse(&fixture_text("namelist.input")).expect("namelist");
    let tables = Tables::parse(
        &fixture_text("MPTABLE.TBL"),
        &fixture_text("SOILPARM.TBL"),
        &fixture_text("GENPARM.TBL"),
        &namelist,
    )
    .expect("tables");

    let recording = Fixture::parse(&fixture_bytes("difftest/atm_sweep.difftest")).expect("fixture");
    let pairs = recording.pairs(Call::Forcing);
    assert!(!pairs.is_empty(), "no sweep records");

    // Built once per option rather than per case: `paramRead` resolves `rain_snow_thresh` from
    // the option, so poking `options.opt_snf` alone would leave the threshold wrong.
    let mut built: Vec<Option<(Options, Parameters)>> = (0..8).map(|_| None).collect();
    for isnf in 1..=7 {
        let mut nl = namelist.clone();
        nl.precip_phase_option = isnf;
        nl.rain_snow_thresh = SWEEP_RAIN_SNOW_THRESH;
        let parameters = Parameters::new(&nl, &tables).expect("parameters");
        built[isnf as usize] = Some((Options::new(&nl), parameters));
    }

    let mut failures = Vec::new();
    let mut seen = [0usize; 8];

    for (before, after) in &pairs {
        let opt_snf = before.itime / 100000;
        assert!(
            (1..=7).contains(&opt_snf),
            "case id {} does not encode an option",
            before.itime
        );
        seen[opt_snf as usize] += 1;

        let (options, parameters) = built[opt_snf as usize].as_ref().unwrap();
        let mut forcing = Forcing::new(&namelist);
        let mut energy = Energy::new(&namelist);
        let mut water = Water::new(&namelist);
        load(
            &mut forcing,
            &mut energy,
            before.state("forcing"),
            before.state("energy"),
        );

        atm(
            options,
            parameters,
            &mut forcing,
            &mut energy,
            &mut water,
            PrecipInput::Total,
        );

        let f = after.state("forcing");
        let e = after.state("energy");
        let w = after.state("water");
        let mut check = |name: &str, got: f32, want: f32| {
            if !same(got, want) {
                failures.push(format!(
                    "opt_snf {opt_snf} case {}: {name} fortran {want:e} ({:08X}), \
                     rust {got:e} ({:08X})",
                    before.itime % 100000,
                    want.to_bits(),
                    got.to_bits()
                ));
            }
        };

        check("forcing%THAIR", forcing.THAIR, f.f32("THAIR"));
        check("forcing%QAIR", forcing.QAIR, f.f32("QAIR"));
        check("forcing%EAIR", forcing.EAIR, f.f32("EAIR"));
        check("forcing%RHOAIR", forcing.RHOAIR, f.f32("RHOAIR"));
        check("forcing%O2PP", forcing.O2PP, f.f32("O2PP"));
        check("forcing%CO2PP", forcing.CO2PP, f.f32("CO2PP"));
        check("forcing%SWDOWN", forcing.SWDOWN, f.f32("SWDOWN"));
        check("forcing%PRCP", forcing.PRCP, f.f32("PRCP"));
        check("forcing%PRCPNONC", forcing.PRCPNONC, f.f32("PRCPNONC"));
        check("forcing%FPICE", forcing.FPICE, f.f32("FPICE"));
        check("forcing%UR", forcing.UR, f.f32("UR"));
        check("energy%TAH", energy.TAH, e.f32("TAH"));
        check("energy%EAH", energy.EAH, e.f32("EAH"));
        check("water%FP", water.FP, w.f32("FP"));
        check("water%rain", water.rain, w.f32("rain"));
        check("water%snow", water.snow, w.f32("snow"));
        check("water%bdfall", water.bdfall, w.f32("bdfall"));

        let solad = f.f32s("SOLAD");
        let solai = f.f32s("SOLAI");
        for i in 1..=2usize {
            check(&format!("forcing%SOLAD({i})"), forcing.SOLAD[i as i32], solad[i - 1]);
            check(&format!("forcing%SOLAI({i})"), forcing.SOLAI[i as i32], solai[i - 1]);
        }
    }

    for (isnf, n) in seen.iter().enumerate().skip(1) {
        assert!(*n > 0, "no cases recorded for opt_snf {isnf}");
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

/// The sweep is only worth its size if it reaches what Bondville does not. Each of these is a
/// branch the year-long recording never enters.
#[test]
fn the_sweep_reaches_the_branches_bondville_misses() {
    let recording = Fixture::parse(&fixture_bytes("difftest/atm_sweep.difftest")).expect("fixture");
    let pairs = recording.pairs(Call::Forcing);

    let (mut nan_bdfall, mut clamped_sw, mut zero_sw, mut sloped_sw, mut snow, mut rain) =
        (0, 0, 0, 0, 0, 0);
    for (before, after) in &pairs {
        let f = after.state("forcing");
        let w = after.state("water");
        if w.f32("bdfall").is_nan() {
            nan_bdfall += 1;
        }
        if f.f32("SWDOWN") == 0.0 {
            zero_sw += 1;
        }
        let e = before.state("energy");
        if e.f32("COSZ") > 0.0 && e.f32("COSZ_HORIZ") > 0.0 {
            if before.state("forcing").f32("SOLDN") / e.f32("COSZ_HORIZ") > 1360.0 {
                clamped_sw += 1;
            } else if e.f32("COSZ") != e.f32("COSZ_HORIZ") {
                sloped_sw += 1;
            }
        }
        if f.f32("FPICE") >= 1.0 {
            snow += 1;
        }
        if f.f32("FPICE") <= 0.0 {
            rain += 1;
        }
    }

    for (what, n) in [
        ("bdfall NaN from 0/0 frozen precipitation", nan_bdfall),
        ("shortwave zeroed below the horizon", zero_sw),
        ("direct irradiance over the solar constant", clamped_sw),
        ("slope/aspect correction with COSZ /= COSZ_HORIZ", sloped_sw),
        ("all-snow FPICE", snow),
        ("all-rain FPICE", rain),
    ] {
        assert!(n > 0, "the sweep never reaches: {what}");
    }
}
