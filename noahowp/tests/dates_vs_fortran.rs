//! `geth_newdate` and `calc_declin` against a sweep of the Fortran.
//!
//! Replaying Bondville (`utilities_vs_fortran.rs`) covers one year at one location on flat
//! ground, which leaves leap-year handling and the slope/aspect correction untested -- both
//! live code. `reference/datesweep_driver.f90` walks those directly: eight start dates chosen
//! around leap years, century and 400-year exceptions and year boundaries, times fourteen
//! offsets in both directions; and a solar-geometry grid over month, hour, latitude, longitude,
//! slope and azimuth.
//!
//! Record layouts are fixed-width with no framing, so a truncated sweep is caught by the length
//! check rather than read as a short one.

use noahowp::utilities::{calc_declin, geth_newdate};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/difftest/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!("reading {path}: {e}\nregenerate with reference/build.sh --fixtures")
    })
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// `odate(12) idt(i4) ndate(12)`
const NEWDATE_RECORD: usize = 28;

#[test]
fn geth_newdate_matches_the_fortran() {
    let data = fixture("newdate_sweep.bin");
    assert_eq!(data.len() % NEWDATE_RECORD, 0, "truncated newdate sweep");
    let mut failures = Vec::new();
    let mut cases = 0;

    for rec in data.as_chunks::<NEWDATE_RECORD>().0 {
        let odate = std::str::from_utf8(&rec[0..12]).expect("ascii");
        let idt = i32_at(rec, 12);
        let want = std::str::from_utf8(&rec[16..28]).expect("ascii");

        match geth_newdate(odate, idt) {
            Ok(got) if got == want => {}
            Ok(got) => failures.push(format!("{odate} + {idt}: fortran {want:?}, rust {got:?}")),
            Err(e) => failures.push(format!("{odate} + {idt}: fortran {want:?}, rust error {e}")),
        }
        cases += 1;
    }

    assert!(cases >= 100, "sweep covers only {cases} cases");
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// `nowdate(19) lat lon slope azimuth cosz cosz_horiz julian (7 x r4) yearlen(i4)`
const DECLIN_RECORD: usize = 19 + 7 * 4 + 4;

#[test]
fn calc_declin_matches_the_fortran() {
    let data = fixture("declin_sweep.bin");
    assert_eq!(data.len() % DECLIN_RECORD, 0, "truncated declin sweep");
    let mut failures = Vec::new();
    let mut cases = 0;

    for rec in data.as_chunks::<DECLIN_RECORD>().0 {
        let nowdate = std::str::from_utf8(&rec[0..19]).expect("ascii");
        let (lat, lon) = (f32_at(rec, 19), f32_at(rec, 23));
        let (slope, azimuth) = (f32_at(rec, 27), f32_at(rec, 31));
        let want_cosz = f32_at(rec, 35);
        let want_horiz = f32_at(rec, 39);
        let want_julian = f32_at(rec, 43);
        let want_yearlen = i32_at(rec, 47);

        let got = calc_declin(nowdate, lat, lon, slope, azimuth)
            .unwrap_or_else(|e| panic!("{nowdate}: {e}"));

        let mut bad = |what: &str, g: f32, w: f32| {
            if g.to_bits() != w.to_bits() {
                failures.push(format!(
                    "{nowdate} lat={lat} lon={lon} slope={slope} az={azimuth}: \
                     {what} fortran {w:e} ({:08X}), rust {g:e} ({:08X})",
                    w.to_bits(),
                    g.to_bits()
                ));
            }
        };
        bad("cosz", got.cosz, want_cosz);
        bad("cosz_horiz", got.cosz_horiz, want_horiz);
        bad("julian", got.julian, want_julian);
        if got.yearlen != want_yearlen {
            failures.push(format!("{nowdate}: yearlen {want_yearlen} vs {}", got.yearlen));
        }
        cases += 1;
    }

    assert!(cases >= 1000, "sweep covers only {cases} cases");
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n  {}",
        failures.len(),
        failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")
    );
}

/// Leap-year handling has to be exercised for its own sake: the sweep's start dates include
/// 2000 (leap), 1999 (common), 2100 (the century exception) and 2016.
#[test]
fn the_sweep_actually_crosses_leap_days() {
    let data = fixture("newdate_sweep.bin");
    let crossings: Vec<String> = data
        .as_chunks::<NEWDATE_RECORD>()
        .0
        .iter()
        .map(|r| std::str::from_utf8(&r[16..28]).unwrap().to_string())
        .filter(|d| d.starts_with("20000229") || d.starts_with("20160229"))
        .collect();
    assert!(
        !crossings.is_empty(),
        "no case lands on a leap day -- the sweep is not testing what it claims to"
    );
}
