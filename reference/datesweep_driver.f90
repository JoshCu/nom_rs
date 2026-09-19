! Date and solar-geometry fixtures for UtilitiesModule.
!
! The Bondville year is 1998 at one location on flat ground, so replaying it exercises neither
! leap-year handling nor the slope/aspect correction in calc_declin -- both of which are live
! code the port has to get right. This sweeps them directly.
!
! Writes two flat streams, each a fixed-width record with no framing, since the layouts are
! fixed and the reader knows them:
!
!   newdate: odate(12) idt(i4) ndate(12)
!   declin:  nowdate(19) lat lon slope azimuth cosz cosz_horiz julian (7 x r4) yearlen(i4)
!
! Usage:  datesweep <newdate_out> <declin_out>

program datesweep_driver

  use UtilitiesModule

  implicit none

  character(len=512) :: newdate_file, declin_file
  character(len=12)  :: odate, ndate
  character(len=19)  :: nowdate
  integer            :: idt, yearlen, i, j, k, l, mo, hr
  real               :: lat, lon, slope, azimuth, cosz, cosz_horiz, julian

  ! Leap year, common year, the 100-year exception, the 400-year exception, and dates sitting
  ! on month and year boundaries.
  character(len=12), parameter :: STARTS(8) = [ &
      "199801010630", "200002280000", "199902280000", "210002280000", &
      "200002290000", "201612312330", "199912312345", "200412310000" ]
  ! Minutes. Includes zero, sub-day, day and year strides, and the negative direction, which
  ! the model never uses but the routine supports.
  integer, parameter :: DELTAS(14) = [ &
      0, 1, 30, 60, 1439, 1440, 1441, 44640, 525600, 527040, &
      -1, -30, -1440, -525600 ]

  real, parameter :: LATS(7)    = [ -80.0, -40.0, -10.0, 0.0, 10.0, 40.01, 80.0 ]
  real, parameter :: LONS(5)    = [ -180.0, -88.37, 0.0, 90.0, 180.0 ]
  real, parameter :: SLOPES(3)  = [ 0.0, 10.0, 30.0 ]
  real, parameter :: AZIMUTHS(4) = [ 0.0, 90.0, 180.0, 270.0 ]

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: datesweep <newdate_out> <declin_out>'
    stop 2
  end if
  call get_command_argument(1, newdate_file)
  call get_command_argument(2, declin_file)

  open(81, file=trim(newdate_file), access='stream', form='unformatted', status='replace')
  do i = 1, size(STARTS)
    do j = 1, size(DELTAS)
      odate = STARTS(i)
      idt = DELTAS(j)
      call geth_newdate(odate, idt, ndate)
      write(81) odate, idt, ndate
    end do
  end do
  close(81)

  open(82, file=trim(declin_file), access='stream', form='unformatted', status='replace')
  do mo = 1, 12
    do hr = 0, 18, 6
      write(nowdate, '(A,I2.2,A,I2.2,A)') '2000-', mo, '-15_', hr, ':30:00'
      do i = 1, size(LATS)
        do j = 1, size(LONS)
          do k = 1, size(SLOPES)
            do l = 1, size(AZIMUTHS)
              lat = LATS(i); lon = LONS(j); slope = SLOPES(k); azimuth = AZIMUTHS(l)
              call calc_declin(nowdate, lat, lon, slope, azimuth, &
                               cosz, cosz_horiz, yearlen, julian)
              write(82) nowdate, lat, lon, slope, azimuth, &
                        cosz, cosz_horiz, julian, yearlen
            end do
          end do
        end do
      end do
    end do
  end do
  close(82)

  write(*, '(A)') 'date sweep written'

end program datesweep_driver
