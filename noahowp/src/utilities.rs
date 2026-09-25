//! Port of `src/UtilitiesModule.f90` @ 0242a96.
//!
//! The first of the five physics calls: advance the calendar to this timestep, then work out
//! where the sun is. Nothing here is a column process, which is why upstream keeps it separate.
//!
//! # Scope
//!
//! Dates are carried as integer components (year, month, day, hour, minute), parsed once from
//! `startdate` at init. Everything the model calls is ported. `minutes_between` is not: its
//! only caller is the ASCII forcing reader, which the BMI path compiles out.

#![allow(non_snake_case)]

use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;

/// `UtilitiesMain`.
pub fn utilities_main(itime: i32, domain: &Domain, forcing: &mut Forcing, energy: &mut Energy) {
    // `idt = itime * (domain%dt / 60)`. The division is real and the product is real; the
    // assignment to an integer then truncates toward zero. Computing `itime * dt as i32 / 60`
    // instead would round differently for any dt that is not a whole number of minutes.
    let idt = fi::int(itime as f32 * (domain.dt / 60.0));

    // calculate current date components from the start date components
    // plus the integer length of run to current time
    let now = advance_datetime(
        domain.start_year,
        domain.start_month,
        domain.start_day,
        domain.start_hour,
        domain.start_minute,
        idt,
    );

    // calculate current declination of direct solar radiation input
    let declin = calc_declin_components(
        now.year,
        day_of_year(now.year, now.month, now.day),
        now.hour,
        now.minute,
        0,
        domain.lat,
        domain.lon,
        domain.terrain_slope,
        domain.azimuth,
    );
    energy.COSZ = declin.cosz;
    energy.COSZ_HORIZ = declin.cosz_horiz;
    forcing.YEARLEN = declin.yearlen;
    forcing.JULIAN = declin.julian;
}

/// Days in February, `nfeb`.
///
/// Note the 3600-year rule, which is not part of the Gregorian calendar. It makes no difference
/// before the year 3600, and it is reproduced rather than corrected.
pub fn nfeb(year: i32) -> i32 {
    let mut n = 28;
    if year % 4 == 0 {
        n = 29;
        if year % 100 == 0 {
            n = 28;
            if year % 400 == 0 {
                n = 29;
                if year % 3600 == 0 {
                    n = 28;
                }
            }
        }
    }
    n
}

// The routines below implement the same calendar nfeb() defines: the proleptic Gregorian
// leap-year rules with the additional exception that years divisible by 3600 are NOT leap
// years. Day numbers count from 0001-01-01 = day 0. Integer `/` truncates toward zero in both
// languages, so the arithmetic carries over as written.

/// A date/time as integer components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
}

/// `year_start_day`: day number of Jan 1 of the given year, from the count of leap years
/// before it.
pub fn year_start_day(yr: i32) -> i32 {
    365 * (yr - 1) + (yr - 1) / 4 - (yr - 1) / 100 + (yr - 1) / 400 - (yr - 1) / 3600
}

/// `days_from_civil`: day number of a civil date (valid for years >= 1).
pub fn days_from_civil(yr: i32, mo: i32, dy: i32) -> i32 {
    year_start_day(yr) + day_of_year(yr, mo, dy)
}

const MDAY: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// `civil_from_days`: the inverse of [`days_from_civil`], as `(yr, mo, dy)`.
pub fn civil_from_days(days: i32) -> (i32, i32, i32) {
    // estimate the year from the mean Gregorian year length, then step to
    // the year whose [start, start + length) interval contains the day.
    // `int()` of a real*8 truncates toward zero, as `as i32` does.
    let mut yr = ((days as f64 + 0.5) / 365.2425) as i32 + 1;
    while year_start_day(yr + 1) <= days {
        yr += 1;
    }
    while year_start_day(yr) > days {
        yr -= 1;
    }

    let mut doy = days - year_start_day(yr);
    let mut mo = 1;
    loop {
        let mut mlen = MDAY[mo as usize - 1];
        if mo == 2 {
            mlen = nfeb(yr);
        }
        if doy < mlen {
            break;
        }
        doy -= mlen;
        mo += 1;
    }
    (yr, mo, doy + 1)
}

/// `advance_datetime`: advance a date/time by a signed number of minutes.
pub fn advance_datetime(yr: i32, mo: i32, dy: i32, hr: i32, mi: i32, dminutes: i32) -> DateTime {
    let total = hr * 60 + mi + dminutes;
    // `modulo` takes the sign of the divisor, which is `rem_euclid` for a positive divisor.
    let minute_of_day = total.rem_euclid(1440);
    let dday = (total - minute_of_day) / 1440;
    let (year, month, day) = civil_from_days(days_from_civil(yr, mo, dy) + dday);
    DateTime {
        year,
        month,
        day,
        hour: minute_of_day / 60,
        minute: minute_of_day % 60,
    }
}

/// `day_of_year`: days since Jan 1 of the same year (Jan 1 -> 0).
///
/// `mo` must be 1 to 12. The Fortran indexes `cum(mo)` unchecked; [`Domain::init_transfer`]
/// rejects a start month outside that range, and every month computed from it is in range.
pub fn day_of_year(yr: i32, mo: i32, dy: i32) -> i32 {
    const CUM: [i32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    let mut d = CUM[mo as usize - 1] + dy - 1;
    if mo > 2 {
        d += nfeb(yr) - 28;
    }
    d
}

/// What `calc_declin_components` computes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Declination {
    /// cosine of the solar zenith angle, corrected for slope and aspect
    pub cosz: f32,
    /// cosine of the solar zenith angle for flat ground
    pub cosz_horiz: f32,
    /// days in this year
    pub yearlen: i32,
    /// floating-point day of year
    pub julian: f32,
}

/// `calc_declin_components`. `iday` is the day-of-year offset as computed by [`day_of_year`].
pub fn calc_declin_components(
    iyear: i32,
    iday: i32,
    ihour: i32,
    iminute: i32,
    isecond: i32,
    latitude: f32,
    longitude: f32,
    slope: f32,
    azimuth: f32,
) -> Declination {
    // `REAL, PARAMETER :: DEGRAD = 3.14159265/180.`
    //
    // This is *not* the DEGRAD in ConstantsModule, which uses the literal 3.1415926 -- one
    // digit shorter. Two constants of the same name and different value; each stays where it
    // was found.
    const DEGRAD: f32 = 3.14159265 / 180.;
    const DPD: f32 = 360. / 365.;

    // Open-coded rather than `nfeb(iyear) + 337`, because that is how upstream writes it and
    // the two agree only as long as both keep the 3600-year rule.
    let mut yearlen = 365;
    if iyear % 4 == 0 {
        yearlen = 366;
        if iyear % 100 == 0 {
            yearlen = 365;
            if iyear % 400 == 0 {
                yearlen = 366;
                if iyear % 3600 == 0 {
                    yearlen = 365;
                }
            }
        }
    }

    // Determine the Julian time (floating-point day of year). Minutes are not included.
    let julian = iday as f32 + ihour as f32 / 24.;

    // OBECL : OBLIQUITY = 23.5 DEGREE.
    let obecl = 23.5 * DEGRAD;
    let sinob = fi::sin(obecl);

    // longitude of the sun from the vernal equinox
    let sxlong = if julian >= 80. {
        DPD * (julian - 80.) * DEGRAD
    } else {
        DPD * (julian + 285.) * DEGRAD
    };
    let arg = sinob * fi::sin(sxlong);
    let declin = fi::asin(arg);

    let mut tloctim =
        ihour as f32 + iminute as f32 / 60.0 + isecond as f32 / 3600.0 + longitude / 15.0;
    tloctim = fi::modulo_trunc(tloctim + 24.0, 24.0);
    let hrang = 15. * (tloctim - 12.) * DEGRAD;

    // Corripio (2003): the cosine of the zenith angle is the dot product of the surface normal
    // and the solar vector.
    let nvx = fi::sin(azimuth * DEGRAD) * fi::sin(slope * DEGRAD);
    let nvy = -fi::cos(azimuth * DEGRAD) * fi::sin(slope * DEGRAD);
    let nvz = fi::cos(slope * DEGRAD);

    let svx = -fi::sin(hrang) * fi::cos(declin);
    let svy = (fi::sin(latitude * DEGRAD) * fi::cos(hrang) * fi::cos(declin))
        - (fi::cos(latitude * DEGRAD) * fi::sin(declin));
    let svz = (fi::cos(latitude * DEGRAD) * fi::cos(hrang) * fi::cos(declin))
        + (fi::sin(latitude * DEGRAD) * fi::sin(declin));

    let cosz = (nvx * svx) + (nvy * svy) + (nvz * svz);

    // For a horizontal plane the normal is (0, 0, 1), so this is just svz -- but it is written
    // out, because the multiply by zero is what the Fortran evaluates.
    let (nvx, nvy, nvz) = (0.0f32, 0.0f32, 1.0f32);
    let cosz_horiz = (nvx * svx) + (nvy * svy) + (nvz * svz);

    Declination {
        cosz,
        cosz_horiz,
        yearlen,
        julian,
    }
}

#[cfg(test)]
mod tests {
    //! Upstream's `test/datetime_test.f90` calendar tables, which are exact. Its solar-geometry
    //! tables are compared under a tolerance; `tests/dates_vs_fortran.rs` checks the same
    //! routine bit for bit against a Fortran sweep instead.
    use super::*;

    /// label, (yr, mo, dy, hr, mi), dminutes, expected (yr, mo, dy, hr, mi)
    type Advance = (&'static str, [i32; 5], i32, [i32; 5]);
    #[rustfmt::skip]
    const ADVANCE_CASES: &[Advance] = &[
        ("identity zero minutes",                     [2023,  6, 15, 10, 30],      0, [2023,  6, 15, 10, 30]),
        ("23:59 plus 1 min rolls to next day",        [2023,  6, 15, 23, 59],      1, [2023,  6, 16,  0,  0]),
        ("Jan 31 23:59 plus 1 min -> Feb 1",          [2023,  1, 31, 23, 59],      1, [2023,  2,  1,  0,  0]),
        ("Apr 30 23:59 plus 1 min -> May 1",          [2023,  4, 30, 23, 59],      1, [2023,  5,  1,  0,  0]),
        ("Feb 28 23:59 plus 1 min non-leap -> Mar 1", [2023,  2, 28, 23, 59],      1, [2023,  3,  1,  0,  0]),
        ("Feb 28 23:59 plus 1 min leap -> Feb 29",    [2024,  2, 28, 23, 59],      1, [2024,  2, 29,  0,  0]),
        ("Feb 29 23:59 plus 1 min leap -> Mar 1",     [2024,  2, 29, 23, 59],      1, [2024,  3,  1,  0,  0]),
        ("Dec 31 23:59 plus 1 min -> new year",       [2023, 12, 31, 23, 59],      1, [2024,  1,  1,  0,  0]),
        ("Feb 28 2000 plus 1 day, div-400 leap",      [2000,  2, 28, 12,  0],   1440, [2000,  2, 29, 12,  0]),
        ("Feb 28 1900 plus 1 day, div-100 not leap",  [1900,  2, 28, 12,  0],   1440, [1900,  3,  1, 12,  0]),
        ("Feb 28 2100 plus 1 day, div-100 not leap",  [2100,  2, 28, 12,  0],   1440, [2100,  3,  1, 12,  0]),
        ("Feb 28 2024 plus 1 day, div-4 leap",        [2024,  2, 28, 12,  0],   1440, [2024,  2, 29, 12,  0]),
        ("Feb 28 3600 +1 day, mod-3600 not leap",     [3600,  2, 28, 12,  0],   1440, [3600,  3,  1, 12,  0]),
        ("Mar 1 3600 -1 day, mod-3600 not leap",      [3600,  3,  1,  0,  0],  -1440, [3600,  2, 28,  0,  0]),
        ("plus 1440 min is exactly 1 day",            [2023,  3, 10,  6, 45],   1440, [2023,  3, 11,  6, 45]),
        ("plus 43200 min is 30 days crossing month",  [2023,  1, 15,  0,  0],  43200, [2023,  2, 14,  0,  0]),
        ("plus 527040 min full leap year",            [2024,  1,  1,  0,  0], 527040, [2025,  1,  1,  0,  0]),
        ("00:00 minus 1 min -> previous day",         [2023,  6, 15,  0,  0],     -1, [2023,  6, 14, 23, 59]),
        ("Mar 1 minus 1440 min leap -> Feb 29",       [2024,  3,  1,  0,  0],  -1440, [2024,  2, 29,  0,  0]),
        ("Mar 1 minus 1440 min non-leap -> Feb 28",   [2023,  3,  1,  0,  0],  -1440, [2023,  2, 28,  0,  0]),
        ("Jan 1 00:00 minus 1 min -> Dec 31",         [2023,  1,  1,  0,  0],     -1, [2022, 12, 31, 23, 59]),
        ("1998-01-01 00:00 plus 1560 min (26 h)",     [1998,  1,  1,  0,  0],   1560, [1998,  1,  2,  2,  0]),
    ];

    fn advance(d: [i32; 5], dminutes: i32) -> [i32; 5] {
        let n = advance_datetime(d[0], d[1], d[2], d[3], d[4], dminutes);
        [n.year, n.month, n.day, n.hour, n.minute]
    }

    #[test]
    fn advance_datetime_table() {
        for &(label, start, dminutes, want) in ADVANCE_CASES {
            assert_eq!(advance(start, dminutes), want, "{label}");
        }
    }

    /// Upstream's `minutes_between` check, reused here to pin `days_from_civil`: the advance
    /// table read backwards.
    #[test]
    fn day_numbers_invert_the_advance_table() {
        let minutes = |d: [i32; 5]| days_from_civil(d[0], d[1], d[2]) * 1440 + d[3] * 60 + d[4];
        for &(label, start, dminutes, want) in ADVANCE_CASES {
            assert_eq!(minutes(want) - minutes(start), dminutes, "{label}");
        }
    }

    #[test]
    fn day_of_year_table() {
        #[rustfmt::skip]
        let cases = [
            (2024, 1, 1, 0), (2024, 2, 28, 58), (2024, 2, 29, 59), (2024, 3, 1, 60),
            (2023, 2, 28, 58), (2023, 3, 1, 59), (2024, 12, 31, 365), (2023, 12, 31, 364),
            (1900, 2, 28, 58), (1900, 3, 1, 59), (1900, 12, 31, 364),
            (2000, 2, 28, 58), (2000, 2, 29, 59), (2000, 3, 1, 60), (2000, 12, 31, 365),
            (2100, 2, 28, 58), (2100, 3, 1, 59), (2100, 12, 31, 364),
            (3600, 2, 28, 58), (3600, 3, 1, 59), (3600, 12, 31, 364),
        ];
        for (yr, mo, dy, want) in cases {
            assert_eq!(day_of_year(yr, mo, dy), want, "{yr}-{mo}-{dy}");
        }
        let fifteenths = [14, 45, 73, 104, 134, 165, 195, 226, 257, 287, 318, 348];
        for (mo, want) in (1..=12).zip(fifteenths) {
            assert_eq!(day_of_year(2023, mo, 15), want, "2023-{mo}-15");
        }
    }

    #[test]
    fn stepping_does_not_drift() {
        let mut d = [1998, 1, 1, 0, 0];
        for _ in 0..26 {
            d = advance(d, 60);
        }
        assert_eq!(d, advance([1998, 1, 1, 0, 0], 26 * 60));
    }

    #[test]
    fn civil_from_days_round_trips_four_centuries_and_the_year_3600() {
        for days in (0..146_097).chain(year_start_day(3599)..year_start_day(3602)) {
            let (yr, mo, dy) = civil_from_days(days);
            assert_eq!(days_from_civil(yr, mo, dy), days, "{yr}-{mo}-{dy}");
        }
    }
}
