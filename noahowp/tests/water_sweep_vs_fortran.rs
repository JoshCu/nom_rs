//! `WaterMain` over every runoff, drainage, infiltration and subsurface option, and through a
//! layered snowpack, against `reference/watersweep_driver.f90`.
//!
//! Bondville runs one option of each for a whole year, and its winter never starts
//! `WaterMain` with more than one snow layer -- so seven of the eight surface runoff schemes,
//! both TOPMODEL water tables, MMF drainage, the second diffusivity form, one-way coupling,
//! `DIVIDE` and `COMBO` are all sweep-only.
//!
//! Same method as `water_vs_fortran.rs`: the complete recorded pre-state is loaded, the
//! options come from the record id, and the complete recorded post-state must match bit for
//! bit.

use std::collections::HashSet;

use noahowp::difftest::bind;
use noahowp::difftest::{Call, Fixture};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::physics::water_main::water_main;
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

/// `((((run * 10 + drn) * 10 + inf) * 10 + infdv) * 10 + sub) * 10 + climate) * 100 + sample`,
/// as `watersweep_driver.f90` packs it. Climate 9 marks the hand-pushed edge cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Config {
    run: i32,
    drn: i32,
    inf: i32,
    infdv: i32,
    sub: i32,
    climate: i32,
}

impl Config {
    fn unpack(id: i32) -> (Self, i32) {
        let digit = |k: u32| (id / 10i32.pow(k)) % 10;
        (
            Config {
                run: digit(7),
                drn: digit(6),
                inf: digit(5),
                infdv: digit(4),
                sub: digit(3),
                climate: digit(2),
            },
            id % 100,
        )
    }
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

fn recording() -> Fixture {
    Fixture::parse(&fixture_bytes("difftest/water_sweep.difftest")).expect("fixture")
}

#[test]
fn every_water_option_matches_the_fortran() {
    let recording = recording();
    let pairs = recording.pairs(Call::Water);
    assert!(!pairs.is_empty(), "no sweep records");

    let mut m = model();
    let mut failures = Vec::new();
    let mut configs = HashSet::new();

    for (before, after) in &pairs {
        let (cfg, sample) = Config::unpack(before.itime);
        configs.insert(cfg);

        bind::load(&mut m, before);
        m.options.opt_run = cfg.run;
        m.options.opt_drn = cfg.drn;
        m.options.opt_inf = cfg.inf;
        m.options.opt_infdv = cfg.infdv;
        m.options.opt_sub = cfg.sub;
        let (levels, options) = (m.levels, m.options.clone());
        water_main(
            &mut m.domain,
            &levels,
            &options,
            &m.parameters,
            &m.forcing,
            &mut m.energy,
            &mut m.water,
        );
        for f in bind::compare(&mut m, after, &[]) {
            failures.push(format!("{cfg:?} case {sample}: {f}"));
        }
    }

    for run in 1..=8 {
        assert!(
            configs.iter().any(|c| c.run == run),
            "no cases for opt_run {run}"
        );
        assert!(
            configs.iter().any(|c| c.drn == run),
            "no cases for opt_drn {run}"
        );
    }
    for infdv in 1..=3 {
        assert!(
            configs.iter().any(|c| c.run == 8 && c.infdv == infdv),
            "no dynamic VIC cases for opt_infdv {infdv}"
        );
    }
    assert!(configs.iter().any(|c| c.inf == 2), "no cases for opt_inf 2");
    assert!(configs.iter().any(|c| c.sub == 2), "no cases for opt_sub 2");

    assert!(
        failures.is_empty(),
        "{} disagreements over {} sweep cases:\n  {}",
        failures.len(),
        pairs.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// The paths Bondville never takes, asserted reached rather than assumed -- each identified
/// by the signature it leaves in the pre- and post-state.
#[test]
fn the_sweep_reaches_the_paths_bondville_misses() {
    let recording = recording();
    let mut seen: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut hit = |what| *seen.entry(what).or_default() += 1;

    for (before, after) in recording.pairs(Call::Water) {
        let (bw, aw) = (before.state("water"), after.state("water"));
        let bd = before.state("domain");
        let (isnow0, isnow1) = (bw.i32("ISNOW"), aw.i32("ISNOW"));

        if isnow0 < -1 {
            hit("more than one snow layer on entry");
        }
        if isnow0 == -1 && isnow1 < -1 {
            hit("DIVIDE splitting a single layer");
        }
        if isnow0 == -2 && isnow1 == -3 {
            hit("DIVIDE splitting two layers into three");
        }
        // COMBINE's work is often undone by DIVIDE in the same call, so it is identified by
        // what forces it -- a top layer with too little ice, or thinner than DZMIN, in a
        // multi-layer pack -- rather than by the layer count it leaves.
        if isnow0 < -1 {
            let (ice, dz) = (bw.f32s("SNICE"), bd.f32s("dzsnso"));
            let top = (isnow0 + 3) as usize; // SNICE is (-2:0), dzsnso (-2:nsoil)
            if ice[top] <= 0.1 {
                hit("COMBINE merging a top layer that has lost its ice");
            } else if dz[top] < 0.025 {
                hit("COMBINE merging a top layer thinner than DZMIN");
            }
        }
        if isnow0 < 0 && isnow1 == 0 {
            hit("the pack melting or sublimating out of layers");
        }
        if isnow0 == 0 && isnow1 < 0 {
            hit("a new layer from shallow snow");
        }
        if aw.f32("snoflow") > 0.0 {
            hit("the 5000 mm glacier cap");
        }
        if bd.i32("IST") == 2 {
            hit("a lake point");
        }
        if isnow0 < 0 && isnow1 == 0 && aw.f32("PONDING2") > 0.0 {
            hit("the whole pack folded back into shallow snow");
        }
        if before.state("energy").f32("TG") <= 273.16 && bw.f32("SNEQV") == 0.0 {
            hit("frozen ground under a snow-free surface");
        }
    }

    for what in [
        "more than one snow layer on entry",
        "DIVIDE splitting a single layer",
        "DIVIDE splitting two layers into three",
        "COMBINE merging a top layer that has lost its ice",
        "COMBINE merging a top layer thinner than DZMIN",
        "the pack melting or sublimating out of layers",
        "a new layer from shallow snow",
        "the 5000 mm glacier cap",
        "a lake point",
        "the whole pack folded back into shallow snow",
        "frozen ground under a snow-free surface",
    ] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "the sweep never reaches: {what}\n{seen:#?}"
        );
    }
}
