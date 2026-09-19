//! Port of `src/UtilitiesModule.f90` @ 0ff055e.
//!
//! The first of the five physics calls: advance the calendar to this timestep, then work out
//! where the sun is. Nothing here is a column process, which is why upstream keeps it separate.
//!
//! # Scope
//!
//! `geth_newdate` and `geth_idts` are general date routines that between them handle punctuated
//! and unpunctuated strings at six resolutions, with fractional seconds. The model calls each
//! one way and one way only:
//!
//! - `geth_newdate(domain%startdate, idt)` with a 12-character `YYYYMMDDHHMM` date and `idt` in
//!   minutes, producing another 12-character date;
//! - `geth_idts(nowdate(1:10), year//"-01-01")` with two 10-character `YYYY-MM-DD` dates,
//!   producing a whole number of days.
//!
//! Only those shapes are ported. Anything else is a [`DateError`] rather than a quietly wrong
//! answer -- upstream reaches `call abort()` on most of them anyway. The rest of the general
//! machinery has no caller and porting it would be untested code with no way to test it.

#![allow(non_snake_case)]

use crate::date_time_utils::DateError;
use crate::domain::Domain;
use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::fortran::intrinsics as fi;

/// `UtilitiesMain`.
pub fn utilities_main(
    itime: i32,
    domain: &mut Domain,
    forcing: &mut Forcing,
    energy: &mut Energy,
) -> Result<(), DateError> {
    // `idt = itime * (domain%dt / 60)`. The division is real and the product is real; the
    // assignment to an integer then truncates toward zero. Computing `itime * dt as i32 / 60`
    // instead would round differently for any dt that is not a whole number of minutes.
    let idt = fi::int(itime as f32 * (domain.dt / 60.0));

    // current 'nowdate' from start date + integer length of run to current time
    domain.nowdate = geth_newdate(&domain.startdate, idt)?;

    // calc_declin wants the punctuated form, which UtilitiesMain builds by hand from nowdate.
    // Seconds are always "00": nowdate carries only minutes.
    let n = &domain.nowdate;
    if n.len() < 12 {
        return Err(DateError::BadLength(n.len()));
    }
    let punctuated = format!(
        "{}-{}-{}_{}:{}:00",
        &n[0..4],
        &n[4..6],
        &n[6..8],
        &n[8..10],
        &n[10..12]
    );

    let declin = calc_declin(
        &punctuated,
        domain.lat,
        domain.lon,
        domain.terrain_slope,
        domain.azimuth,
    )?;
    energy.COSZ = declin.cosz;
    energy.COSZ_HORIZ = declin.cosz_horiz;
    forcing.YEARLEN = declin.yearlen;
    forcing.JULIAN = declin.julian;

    Ok(())
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

const MDAY: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn digits(s: &str, range: std::ops::Range<usize>, what: &'static str) -> Result<i32, DateError> {
    s.get(range)
        .ok_or(DateError::BadField(what))?
        .trim()
        .parse()
        .map_err(|_| DateError::BadField(what))
}

/// `geth_newdate` for a 12-character `YYYYMMDDHHMM` date and `idt` in minutes.
///
/// See the module docs for why the other shapes are absent.
pub fn geth_newdate(odate: &str, idt: i32) -> Result<String, DateError> {
    if odate.len() != 12 || odate.as_bytes()[4] == b'-' {
        return Err(DateError::BadLength(odate.len()));
    }

    let yrold = digits(odate, 0..4, "year")?;
    let moold = digits(odate, 4..6, "month")?;
    let dyold = digits(odate, 6..8, "day")?;
    let hrold = digits(odate, 8..10, "hour")?;
    let miold = digits(odate, 10..12, "minute")?;

    let mut mday = MDAY;
    mday[1] = nfeb(yrold);

    // The Fortran prints each failed check and then calls abort().
    if !(1..=12).contains(&moold) {
        return Err(DateError::BadField("month"));
    }
    if dyold < 1 || dyold > mday[moold as usize - 1] {
        return Err(DateError::BadField("day"));
    }
    if !(0..=23).contains(&hrold) {
        return Err(DateError::BadField("hour"));
    }
    if !(0..=59).contains(&miold) {
        return Err(DateError::BadField("minute"));
    }

    // idt is in minutes: the `idtmin` branch.
    let mut nday = idt.abs() / 1440;
    let mut nhour = idt.abs() % 1440 / 60;
    let nmin = idt.abs() % 60;

    let (mut yrnew, mut monew, mut dynew, hrnew, minew);
    if idt >= 0 {
        let mut m = miold + nmin;
        if m >= 60 {
            m -= 60;
            nhour += 1;
        }
        minew = m;

        let mut h = hrold + nhour;
        if h >= 24 {
            h -= 24;
            nday += 1;
        }
        hrnew = h;

        dynew = dyold;
        monew = moold;
        yrnew = yrold;
        for _ in 0..nday {
            dynew += 1;
            if dynew > mday[monew as usize - 1] {
                dynew -= mday[monew as usize - 1];
                monew += 1;
                if monew > 12 {
                    monew = 1;
                    yrnew += 1;
                    mday[1] = nfeb(yrnew);
                }
            }
        }
    } else {
        let mut m = miold - nmin;
        if m < 0 {
            m += 60;
            nhour += 1;
        }
        minew = m;

        let mut h = hrold - nhour;
        if h < 0 {
            h += 24;
            nday += 1;
        }
        hrnew = h;

        dynew = dyold;
        monew = moold;
        yrnew = yrold;
        for _ in 0..nday {
            dynew -= 1;
            if dynew == 0 {
                monew -= 1;
                if monew == 0 {
                    monew = 12;
                    yrnew -= 1;
                    mday[1] = nfeb(yrnew);
                }
                dynew = mday[monew as usize - 1];
            }
        }
    }

    // `format(i4,i2.2,i2.2,i2.2,i2.2)`. The year field is `i4`, not `i4.4`: a year below 1000
    // would come out space-padded, which is upstream's behaviour and not worth diverging from.
    Ok(format!("{yrnew:4}{monew:02}{dynew:02}{hrnew:02}{minew:02}"))
}

/// `geth_idts` for two 10-character `YYYY-MM-DD` dates, giving whole days.
pub fn geth_idts(newdate: &str, olddate: &str) -> Result<i32, DateError> {
    if newdate.len() != 10 || olddate.len() != 10 {
        return Err(DateError::BadLength(newdate.len()));
    }
    if newdate.as_bytes()[4] != b'-' || olddate.as_bytes()[4] != b'-' {
        return Err(DateError::BadField("separator"));
    }

    // "If olddate > newdate, swap them and negate at the end" -- a plain string comparison,
    // which works because the format is fixed width and big-endian.
    let (ndate, odate, isign) = if olddate > newdate {
        (olddate, newdate, -1)
    } else {
        (newdate, olddate, 1)
    };

    let yrnew = digits(ndate, 0..4, "year")?;
    let monew = digits(ndate, 5..7, "month")?;
    let dynew = digits(ndate, 8..10, "day")?;
    let yrold = digits(odate, 0..4, "year")?;
    let moold = digits(odate, 5..7, "month")?;
    let dyold = digits(odate, 8..10, "day")?;

    for (mo, dy, yr) in [(monew, dynew, yrnew), (moold, dyold, yrold)] {
        if !(1..=12).contains(&mo) {
            return Err(DateError::BadField("month"));
        }
        let mut mday = MDAY;
        mday[1] = nfeb(yr);
        if dy < 1 || dy > mday[mo as usize - 1] {
            return Err(DateError::BadField("day"));
        }
    }

    // Days from 1 January of the old year up to each date. `337 + nfeb(i)` is the length of
    // year `i`.
    let mut newdys = 0;
    for i in yrold..yrnew {
        newdys += 337 + nfeb(i);
    }
    if monew > 1 {
        let mut mday = MDAY;
        mday[1] = nfeb(yrnew);
        newdys += mday[..monew as usize - 1].iter().sum::<i32>();
    }
    newdys += dynew - 1;

    let mut olddys = 0;
    if moold > 1 {
        let mut mday = MDAY;
        mday[1] = nfeb(yrold);
        olddys += mday[..moold as usize - 1].iter().sum::<i32>();
    }
    olddys += dyold - 1;

    // At this resolution the hour/minute/second refinements below are all skipped: the guard
    // is `olen > 10`, and olen is exactly 10.
    Ok((newdys - olddys) * isign)
}

/// What `calc_declin` computes.
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

/// `calc_declin`, for a 19-character `YYYY-MM-DD_HH:mm:ss` date.
pub fn calc_declin(
    nowdate: &str,
    latitude: f32,
    longitude: f32,
    slope: f32,
    azimuth: f32,
) -> Result<Declination, DateError> {
    if nowdate.len() != 19 {
        return Err(DateError::BadLength(nowdate.len()));
    }

    // `REAL, PARAMETER :: DEGRAD = 3.14159265/180.`
    //
    // This is *not* the DEGRAD in ConstantsModule, which uses the literal 3.1415926 -- one
    // digit shorter. Two constants of the same name and different value; each stays where it
    // was found.
    const DEGRAD: f32 = 3.14159265 / 180.;
    const DPD: f32 = 360. / 365.;

    let iyear = digits(nowdate, 0..4, "year")?;
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

    let iday = geth_idts(&nowdate[0..10], &format!("{}-01-01", &nowdate[0..4]))?;
    let ihour = digits(nowdate, 11..13, "hour")?;
    let iminute = digits(nowdate, 14..16, "minute")?;
    let isecond = digits(nowdate, 17..19, "second")?;
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

    Ok(Declination {
        cosz,
        cosz_horiz,
        yearlen,
        julian,
    })
}
