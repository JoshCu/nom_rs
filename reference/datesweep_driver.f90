! Date and solar-geometry fixtures for UtilitiesModule.
!
! The Bondville year is 1998 at one location on flat ground, so replaying it exercises neither
! leap-year handling nor the slope/aspect correction in calc_declin_components -- both of which
! are live code the port has to get right. This sweeps them directly.
!
! Writes two flat streams, each a fixed-width record with no framing, since the layouts are
! fixed and the reader knows them:
!
!   advance: yr mo dy hr mi dminutes yr2 mo2 dy2 hr2 mi2 doy2 (12 x i4)
!   declin:  yr iday hr mi sc (5 x i4) lat lon slope azimuth cosz cosz_horiz julian (7 x r4)
!            yearlen(i4)
!
! doy2 is day_of_year(yr2, mo2, dy2), the value UtilitiesMain hands calc_declin_components.
!
! Usage:  datesweep <advance_out> <declin_out>

program datesweep_driver

  use UtilitiesModule

  implicit none

  character(len=512) :: advance_file, declin_file
  integer            :: yr, mo, dy, hr, mi, sc, dmin, yr2, mo2, dy2, hr2, mi2, iday
  integer            :: yearlen, i, j, k, l, m
  real               :: lat, lon, slope, azimuth, cosz, cosz_horiz, julian

  ! Leap year, common year, the 100-year, 400-year and 3600-year exceptions, dates sitting on
  ! month and year boundaries, and an out-of-range day, which the arithmetic accepts.
  integer, parameter :: STARTS(5, 12) = reshape([ &
      1998,  1,  1,  6, 30,   2000,  2, 28,  0,  0,   1999,  2, 28,  0,  0, &
      2100,  2, 28,  0,  0,   2000,  2, 29,  0,  0,   2016, 12, 31, 23, 30, &
      1999, 12, 31, 23, 45,   2004, 12, 31,  0,  0,   3600,  2, 28, 12,  0, &
      3600,  3,  1,  0,  0,      1,  1,  1,  0,  0,   2023,  2, 30,  0,  0 ], [5, 12])
  ! Minutes. Includes zero, sub-day, day, month, year and multi-century strides, and the
  ! negative direction, which the model never uses but the routine supports.
  integer, parameter :: DELTAS(20) = [ &
      0, 1, 30, 60, 1439, 1440, 1441, 44640, 525600, 527040, 5259600, 52596000, 1000000000, &
      -1, -30, -1440, -44640, -525600, -527040, -5259600 ]

  ! Years around each leap rule, crossed with the day, hour, minute and second that feed julian
  ! and the hour angle.
  integer, parameter :: YEARS(6)  = [ 1900, 1998, 2000, 2024, 2100, 3600 ]
  integer, parameter :: IDAYS(7)  = [ 0, 58, 59, 79, 80, 200, 365 ]
  integer, parameter :: HOURS(4)  = [ 0, 6, 12, 23 ]
  integer, parameter :: MINS(2)   = [ 0, 30 ]
  integer, parameter :: SECS(2)   = [ 0, 45 ]

  real, parameter :: LATS(4)     = [ -80.0, 0.0, 40.01, 80.0 ]
  real, parameter :: LONS(4)     = [ -180.0, -88.37, 90.0, 180.0 ]
  real, parameter :: SLOPES(2)   = [ 0.0, 30.0 ]
  real, parameter :: AZIMUTHS(2) = [ 90.0, 180.0 ]

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: datesweep <advance_out> <declin_out>'
    stop 2
  end if
  call get_command_argument(1, advance_file)
  call get_command_argument(2, declin_file)

  open(81, file=trim(advance_file), access='stream', form='unformatted', status='replace')
  do i = 1, size(STARTS, 2)
    yr = STARTS(1, i); mo = STARTS(2, i); dy = STARTS(3, i); hr = STARTS(4, i); mi = STARTS(5, i)
    do j = 1, size(DELTAS)
      dmin = DELTAS(j)
      ! Stay in years >= 1, where days_from_civil is documented as valid.
      if (yr == 1 .and. dmin < 0) cycle
      call advance_datetime(yr, mo, dy, hr, mi, dmin, yr2, mo2, dy2, hr2, mi2)
      write(81) yr, mo, dy, hr, mi, dmin, yr2, mo2, dy2, hr2, mi2, day_of_year(yr2, mo2, dy2)
    end do
  end do
  close(81)

  open(82, file=trim(declin_file), access='stream', form='unformatted', status='replace')
  do m = 1, size(YEARS)
  do iday = 1, size(IDAYS)
  do hr = 1, size(HOURS)
  do mi = 1, size(MINS)
  do sc = 1, size(SECS)
    do i = 1, size(LATS)
      do j = 1, size(LONS)
        do k = 1, size(SLOPES)
          do l = 1, size(AZIMUTHS)
            lat = LATS(i); lon = LONS(j); slope = SLOPES(k); azimuth = AZIMUTHS(l)
            call calc_declin_components(YEARS(m), IDAYS(iday), HOURS(hr), MINS(mi), SECS(sc), &
                                        lat, lon, slope, azimuth,                              &
                                        cosz, cosz_horiz, yearlen, julian)
            write(82) YEARS(m), IDAYS(iday), HOURS(hr), MINS(mi), SECS(sc), &
                      lat, lon, slope, azimuth, cosz, cosz_horiz, julian, yearlen
          end do
        end do
      end do
    end do
  end do
  end do
  end do
  end do
  end do
  close(82)

  write(*, '(A)') 'date sweep written'

end program datesweep_driver
