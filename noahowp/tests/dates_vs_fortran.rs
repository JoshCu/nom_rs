//! `advance_datetime`, `day_of_year` and `calc_declin_components` against a sweep of the
//! Fortran.
//!
//! Replaying Bondville (`utilities_vs_fortran.rs`) covers one year at one location on flat
//! ground, which leaves leap-year handling and the slope/aspect correction untested -- both
//! live code. `reference/datesweep_driver.f90` walks those directly: twelve start dates chosen
//! around leap years, the century, 400- and 3600-year exceptions and year boundaries, times
//! twenty offsets in both directions; and a solar-geometry grid over year, day, hour, minute,
//! second, latitude, longitude, slope and azimuth.
//!
//! Record layouts are fixed-width with no framing, so a truncated sweep is caught by the length
//! check rather than read as a short one.

use noahowp::utilities::{advance_datetime, calc_declin_components, day_of_year};

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

/// `yr mo dy hr mi dminutes yr2 mo2 dy2 hr2 mi2 doy2 (12 x i4)`
const ADVANCE_RECORD: usize = 12 * 4;

fn advance_records() -> Vec<[i32; 12]> {
    let data = fixture("advance_sweep.bin");
    assert_eq!(data.len() % ADVANCE_RECORD, 0, "truncated advance sweep");
    data.as_chunks::<ADVANCE_RECORD>()
        .0
        .iter()
        .map(|r| std::array::from_fn(|i| i32_at(r, 4 * i)))
        .collect()
}

#[test]
fn advance_datetime_matches_the_fortran() {
    let records = advance_records();
    let mut failures = Vec::new();

    for r in &records {
        let [yr, mo, dy, hr, mi, dmin, ..] = *r;
        let n = advance_datetime(yr, mo, dy, hr, mi, dmin);
        let got = [n.year, n.month, n.day, n.hour, n.minute, day_of_year(n.year, n.month, n.day)];
        if got[..] != r[6..] {
            failures.push(format!("{:?} + {dmin}: fortran {:?}, rust {got:?}", &r[..5], &r[6..]));
        }
    }

    assert!(records.len() >= 200, "sweep covers only {} cases", records.len());
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n  {}",
        failures.len(),
        records.len(),
        failures.join("\n  ")
    );
}

/// Leap-year handling has to be exercised for its own sake: the sweep's start dates include
/// 2000 (leap), 1999 (common), 2100 (the century exception), 3600 (the 3600-year exception)
/// and 2016.
#[test]
fn the_sweep_actually_crosses_leap_days() {
    let records = advance_records();
    let lands_on = |yr, mo, dy| records.iter().any(|r| r[6..9] == [yr, mo, dy]);
    assert!(
        lands_on(2000, 2, 29) || lands_on(2016, 2, 29),
        "no case lands on a leap day -- the sweep is not testing what it claims to"
    );
    assert!(lands_on(3600, 3, 1), "no case crosses the year-3600 February");
}

/// `yr iday hr mi sc (5 x i4) lat lon slope azimuth cosz cosz_horiz julian (7 x r4)
/// yearlen(i4)`
const DECLIN_RECORD: usize = 5 * 4 + 7 * 4 + 4;

#[test]
fn calc_declin_components_matches_the_fortran() {
    let data = fixture("declin_sweep.bin");
    assert_eq!(data.len() % DECLIN_RECORD, 0, "truncated declin sweep");
    let mut failures = Vec::new();
    let mut cases = 0;

    for rec in data.as_chunks::<DECLIN_RECORD>().0 {
        let [yr, iday, hr, mi, sc]: [i32; 5] = std::array::from_fn(|i| i32_at(rec, 4 * i));
        let (lat, lon) = (f32_at(rec, 20), f32_at(rec, 24));
        let (slope, azimuth) = (f32_at(rec, 28), f32_at(rec, 32));
        let want_cosz = f32_at(rec, 36);
        let want_horiz = f32_at(rec, 40);
        let want_julian = f32_at(rec, 44);
        let want_yearlen = i32_at(rec, 48);

        let got = calc_declin_components(yr, iday, hr, mi, sc, lat, lon, slope, azimuth);
        let at = format!("{yr} day {iday} {hr:02}:{mi:02}:{sc:02}");

        let mut bad = |what: &str, g: f32, w: f32| {
            if g.to_bits() != w.to_bits() {
                failures.push(format!(
                    "{at} lat={lat} lon={lon} slope={slope} az={azimuth}: \
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
            failures.push(format!("{at}: yearlen {want_yearlen} vs {}", got.yearlen));
        }
        cases += 1;
    }

    assert!(cases >= 10_000, "sweep covers only {cases} cases");
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n  {}",
        failures.len(),
        failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")
    );
}
