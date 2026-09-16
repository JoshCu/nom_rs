//! Port of `src/DateTimeUtilsModule.f90` @ eaa8282 -- date handling only.
//!
//! The upstream module also vendors a general string-utility library (`parse`, `compact`,
//! `shiftstr`, `writeq_*`, ...). None of it is reachable from the BMI path, so none of it is
//! ported. See `PORTING.md`.
//!
//! Entry points actually used by the model:
//! `date_to_unix` (DomainType), `unix_to_date` and `get_utime_list` (RunModule).

use core::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DateError {
    /// `parse_date` accepts only YYYYMMDD, YYYYMMDDHH, YYYYMMDDHHMM, YYYYMMDDHHMMSS.
    BadLength(usize),
    BadField(&'static str),
    /// `get_utime_list`: the run length is not a whole number of timesteps.
    NotWholeTimesteps { start: f64, end: f64, dt: f32 },
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateError::BadLength(n) => {
                write!(f, "date string must be 8, 10, 12 or 14 characters, got {n}")
            }
            DateError::BadField(which) => write!(f, "could not parse {which} from date string"),
            DateError::NotWholeTimesteps { start, end, dt } => write!(
                f,
                "start and end datetimes are not an even multiple of dt -- \
                 start {start}, end {end}, dt {dt}"
            ),
        }
    }
}

impl std::error::Error for DateError {}

/// Broken-down calendar time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParts {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub min: i32,
    pub sec: i32,
}

/// `parse_date`. Accepts `YYYYMMDD[HH][MM][SS]`.
pub fn parse_date(date: &str) -> Result<DateParts, DateError> {
    let d = date.trim_end();
    let n = d.len();
    if !matches!(n, 8 | 10 | 12 | 14) {
        return Err(DateError::BadLength(n));
    }

    let field = |range: std::ops::Range<usize>, what: &'static str| -> Result<i32, DateError> {
        d.get(range)
            .ok_or(DateError::BadField(what))?
            .trim()
            .parse::<i32>()
            .map_err(|_| DateError::BadField(what))
    };

    let year = field(0..4, "year")?;
    let month = field(4..6, "month")?;
    let day = field(6..8, "day")?;
    let (hour, min, sec) = match n {
        8 => (0, 0, 0),
        10 => (field(8..10, "hour")?, 0, 0),
        12 => (field(8..10, "hour")?, field(10..12, "minute")?, 0),
        _ => (
            field(8..10, "hour")?,
            field(10..12, "minute")?,
            field(12..14, "second")?,
        ),
    };

    Ok(DateParts { year, month, day, hour, min, sec })
}

/// `julian_date`. Julian Day number, zero at noon on 1 Jan -4712.
///
/// **This is not the inverse of [`calendar_date`].** The two upstream routines use conventions
/// one day apart: `calendar_date(julian_date(d, m, y) + 1) == (d, m, y)`. For 1970-01-01 this
/// returns 2440587, where the astronomical Julian Day Number is 2440588. `unix_to_date` is
/// written around that offset (it adds 1 to `i_day`), and `date_to_unix` takes a *difference*
/// of two calls, so the offset cancels there. Preserve it -- do not "fix" either routine.
///
/// The Fortran declares `yr_corr` with an initialiser, which in Fortran implies `SAVE`: the
/// 0.75 correction set for a negative year would persist into later calls. That path is
/// unreachable here -- years come from a 4-digit field of a date string and are never negative
/// -- so this uses a plain local. Any future caller passing `year < 0` would diverge from the
/// Fortran's second and subsequent calls, not its first.
pub fn julian_date(day: i32, month: i32, year: i32) -> i32 {
    let d = day as f64;
    let mut m = month as f64;
    let mut y = year as f64;
    let mut b = 0i32;
    let mut yr_corr = 0.0f64;

    // There is no year 0.
    if year < 0 {
        y += 1.0;
        yr_corr = 0.75;
    }

    if month <= 2 {
        y -= 1.0;
        m = month as f64 + 12.0;
    }

    if y * 10000.0 + m * 100.0 + d >= 15821015.0 {
        let a = year / 100; // integer division of the original year, not y
        b = 2 - a + (a / 4);
    }

    // INT() truncates toward zero; the final assignment to an integer function result
    // truncates the double expression the same way.
    let acc = (365.25 * y - yr_corr).trunc() + (30.6001 * (m + 1.0)).trunc() + d + 1720994.0
        + b as f64;
    acc.trunc() as i32
}

/// `calendar_date`. Inverse of `julian_date` (Richards 2013 via Wikipedia).
pub fn calendar_date(jdate: i32) -> (i32, i32, i32) {
    let (y, j, m, n, r, p) = (4716, 1401, 2, 12, 4, 1461);
    let (v, u, s, w, b, c) = (3, 5, 153, 2, 274277, -38);

    let f = jdate + j + (((4 * jdate + b) / 146097) * 3) / 4 + c;
    let e = r * f + v;
    let g = (e % p) / r;
    let h = u * g + w;

    let day = (h % s) / u + 1;
    let month = (h / s + m) % n + 1;
    let year = e / p - y + (n + m - month) / n;

    (day, month, year)
}

/// `date_to_unix`. Seconds since 1970-01-01 00:00:00.
pub fn date_to_unix(date: &str) -> Result<f64, DateError> {
    let p = parse_date(date)?;

    let u_day = julian_date(1, 1, 1970) as f64;
    let i_day = julian_date(p.day, p.month, p.year) as f64;
    let days = i_day - u_day;

    Ok((days * 86400.0) + (p.hour as f64 * 3600.0) + (p.min as f64 * 60.0) + p.sec as f64)
}

/// `unix_to_date`.
pub fn unix_to_date(itime: f64) -> DateParts {
    let u_day = julian_date(1, 1, 1970);

    let mut i_day = (itime / 86400.0).trunc() as i32;
    if i_day < 0 {
        i_day -= 1;
    }
    i_day += 1;

    let (day, month, year) = calendar_date(u_day + i_day);

    // mod(itime, 86400.0) on a double, then truncated by assignment to an integer.
    let mut rem = (itime % 86400.0).trunc() as i32;
    if rem < 0 {
        rem += 86400;
    }

    let hour = rem / 3600;
    let min = (rem / 60) - (hour * 60);
    let sec = rem % 60;

    DateParts { year, month, day, hour, min, sec }
}

/// `get_utime_list`. End-of-timestep unix times spanning the run.
pub fn get_utime_list(
    start_datetime: f64,
    end_datetime: f64,
    dt: f32,
) -> Result<Vec<f64>, DateError> {
    let dt_d = dt as f64;

    if ((end_datetime - start_datetime) % dt_d).abs() > 1e-5 {
        return Err(DateError::NotWholeTimesteps {
            start: start_datetime,
            end: end_datetime,
            dt,
        });
    }

    let ntimes = ((end_datetime - start_datetime) / dt_d) as i32 + 1;
    let mut times = Vec::with_capacity(ntimes.max(0) as usize);

    let mut utime = start_datetime;
    for _ in 0..ntimes {
        if utime > end_datetime + 1e-5 {
            break;
        }
        times.push(utime);
        utime += dt_d;
    }

    Ok(times)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_accepted_length() {
        assert_eq!(
            parse_date("19980101").unwrap(),
            DateParts { year: 1998, month: 1, day: 1, hour: 0, min: 0, sec: 0 }
        );
        assert_eq!(
            parse_date("1998010106").unwrap(),
            DateParts { year: 1998, month: 1, day: 1, hour: 6, min: 0, sec: 0 }
        );
        assert_eq!(
            parse_date("199801010630").unwrap(),
            DateParts { year: 1998, month: 1, day: 1, hour: 6, min: 30, sec: 0 }
        );
        assert_eq!(
            parse_date("19980101063045").unwrap(),
            DateParts { year: 1998, month: 1, day: 1, hour: 6, min: 30, sec: 45 }
        );
    }

    #[test]
    fn rejects_other_lengths() {
        assert_eq!(parse_date("1998010").unwrap_err(), DateError::BadLength(7));
        assert_eq!(parse_date("19980101063").unwrap_err(), DateError::BadLength(11));
    }

    #[test]
    fn unix_epoch_is_zero() {
        assert_eq!(date_to_unix("19700101").unwrap(), 0.0);
    }

    #[test]
    fn known_unix_timestamps() {
        // 2000-01-01T00:00:00Z
        assert_eq!(date_to_unix("20000101").unwrap(), 946_684_800.0);
        // 1998-01-01T06:30:00Z -- the shipped namelist's startdate.
        assert_eq!(date_to_unix("199801010630").unwrap(), 883_636_200.0);
        // 1999-01-01T06:30:00Z -- its enddate.
        assert_eq!(date_to_unix("199901010630").unwrap(), 915_172_200.0);
    }

    #[test]
    fn round_trips_through_unix() {
        for s in [
            "19700101", "19980101", "199801010630", "20000229", "20200229", "20241231",
        ] {
            let want = parse_date(s).unwrap();
            let got = unix_to_date(date_to_unix(s).unwrap());
            assert_eq!(got.year, want.year, "{s} year");
            assert_eq!(got.month, want.month, "{s} month");
            assert_eq!(got.day, want.day, "{s} day");
            assert_eq!(got.hour, want.hour, "{s} hour");
            assert_eq!(got.min, want.min, "{s} min");
        }
    }

    #[test]
    fn leap_years_are_handled() {
        // 2000 is a leap year, 1900 is not.
        let feb29_2000 = date_to_unix("20000229").unwrap();
        let mar01_2000 = date_to_unix("20000301").unwrap();
        assert_eq!(mar01_2000 - feb29_2000, 86400.0);

        // One year of 1998 (not a leap year) is 365 days.
        let a = date_to_unix("199801010630").unwrap();
        let b = date_to_unix("199901010630").unwrap();
        assert_eq!(b - a, 365.0 * 86400.0);
    }

    #[test]
    fn julian_date_matches_the_fortran_convention() {
        // The upstream routine returns one less than the astronomical JDN: 1970-01-01 is
        // 2440587 here, not 2440588. date_to_unix differences two calls, so this cancels.
        assert_eq!(julian_date(1, 1, 1970), 2440587);
        assert_eq!(julian_date(1, 1, 2000), 2451544);
    }

    #[test]
    fn calendar_date_inverts_julian_date_offset_by_one() {
        // calendar_date uses a convention one day ahead of julian_date. This asymmetry is why
        // unix_to_date adds 1 to i_day before calling calendar_date.
        for (d, m, y) in [(1, 1, 1970), (1, 1, 2000), (29, 2, 2000), (31, 12, 2024)] {
            let jd = julian_date(d, m, y);
            assert_eq!(calendar_date(jd + 1), (d, m, y), "{y}-{m}-{d}");
            // And the un-offset call lands on the previous day.
            assert_ne!(calendar_date(jd), (d, m, y), "{y}-{m}-{d}");
        }
    }

    #[test]
    fn utime_list_spans_the_shipped_run() {
        // The Bondville year at dt = 1800 s.
        let start = date_to_unix("199801010630").unwrap();
        let end = date_to_unix("199901010630").unwrap();
        let times = get_utime_list(start, end, 1800.0).unwrap();

        // 365 days / 30 min, plus the start point.
        assert_eq!(times.len(), 365 * 48 + 1);
        assert_eq!(times[0], start);
        assert_eq!(*times.last().unwrap(), end);
    }

    #[test]
    fn utime_list_rejects_partial_timesteps() {
        let start = date_to_unix("199801010630").unwrap();
        let end = start + 2700.0; // 1.5 timesteps at dt = 1800
        assert!(matches!(
            get_utime_list(start, end, 1800.0),
            Err(DateError::NotWholeTimesteps { .. })
        ));
    }

    #[test]
    fn utime_list_steps_by_dt() {
        let times = get_utime_list(0.0, 5400.0, 1800.0).unwrap();
        assert_eq!(times, vec![0.0, 1800.0, 3600.0, 5400.0]);
    }

    #[test]
    fn unix_to_date_recovers_time_of_day() {
        let p = unix_to_date(883_636_200.0); // 1998-01-01T06:30:00Z
        assert_eq!((p.year, p.month, p.day), (1998, 1, 1));
        assert_eq!((p.hour, p.min, p.sec), (6, 30, 0));
    }
}
