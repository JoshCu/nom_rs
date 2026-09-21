! ATM sweep fixture generator: every OPT_SNF branch, not just Bondville's.
!
! Bondville runs `precip_phase_option = 1`, so six of the seven precipitation-phase schemes in
! AtmProcessing are never entered over the whole year -- and neither is the wet-bulb
! calculation, the logistic regression, the weather-model phase path, or the direct-irradiance
! clamp in the shortwave split. A port could get all of them wrong and the year-long recording
! would still agree.
!
! Inputs are generated here rather than agreed with the Rust side: the port reads them back out
! of the recorded pre-state, so the only thing the two sides have to agree on is the option,
! which travels in the `itime` slot as `opt_snf * 100000 + sample`.
!
! Records go through the same `dt_record` the instrumented RunModule uses, so this fixture
! parses with the same reader and picks up new fields automatically when upstream adds them.
!
! Usage:  atmsweep <namelist> <fixture_out>

program atmsweep_driver

  use RunModule
  use ForcingModule
  use DiffTestModule

  implicit none

  type(noahowp_type) :: model
  character(len=512) :: config_file, fixture_file
  integer            :: isnf, i, ncase
  ! A 32-bit LCG (Numerical Recipes), so the sweep is wide but reproducible from the source
  ! alone -- no random seed, no recorded input list.
  integer(kind=8)    :: rng

  ! NEDGE must match the number of hand-placed cases in edge_cases below.
  integer, parameter :: NRANDOM = 48, NEDGE = 34

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: atmsweep <namelist> <fixture_out>'
    stop 2
  end if
  call get_command_argument(1, config_file)
  call get_command_argument(2, fixture_file)

  call initialize_from_file(model, trim(config_file))
  call dt_open(trim(fixture_file))
  ! The fixture format opens with one static block. It records the namelist's own option set,
  ! which is not the option each case runs under -- the sweep rebuilds `options` and
  ! `parameters` per OPT_SNF, and the case's option travels in the record id instead.
  call dt_static(model%levels, model%options, model%parameters)
  call dt_arm(.true.)

  do isnf = 1, 7
    ! Rebuild options and parameters for this option: `rain_snow_thresh` is resolved from it
    ! inside paramRead, so it is not enough to poke `options%opt_snf`.
    model%namelist%precip_phase_option = isnf
    ! A threshold offset that is neither zero nor the default, so options 5 and 6 are
    ! distinguishable from option 3.
    model%namelist%rain_snow_thresh = 1.5
    call model%options%InitTransfer(model%namelist)
    if (allocated(model%parameters%bexp)) then
      deallocate(model%parameters%bexp, model%parameters%smcmax, model%parameters%smcwlt, &
                 model%parameters%smcref, model%parameters%dksat, model%parameters%dwsat, &
                 model%parameters%psisat)
    end if
    call model%parameters%Init(model%namelist)
    call model%parameters%paramRead(model%namelist)

    rng   = 1
    ncase = 0

    ! Hand-placed cases first: the branch boundaries, and the inputs that are singular rather
    ! than merely unusual.
    call edge_cases(isnf, ncase)

    do i = 1, NRANDOM
      call random_case(isnf, ncase)
    end do
  end do

  call dt_close()
  write(*, '(A,I0,A)') 'atm sweep written (', 7 * (NEDGE + NRANDOM), ' cases)'

contains

  !> Uniform in [lo, hi) from the LCG. Advancing `rng` here is what makes every draw in a case
  !> independent of the others.
  real function draw(lo, hi)
    real, intent(in) :: lo, hi
    rng  = mod(rng * 1664525_8 + 1013904223_8, 4294967296_8)
    draw = lo + (hi - lo) * real(rng) / 4294967296.0
  end function draw

  subroutine run_case(isnf, ncase)
    integer, intent(in)    :: isnf
    integer, intent(inout) :: ncase
    integer :: id
    id = isnf * 100000 + ncase
    call dt_record(2, 0, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    call ForcingMain(model%options, model%parameters, model%forcing, model%energy, model%water)
    call dt_record(2, 1, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    ncase = ncase + 1
  end subroutine run_case

  !> A plausible mid-range case, overwritten field by field by whichever caller wants to move
  !> one thing at a time.
  subroutine baseline()
    model%forcing%SFCPRS   = 96000.0
    model%forcing%SFCTMP   = 283.15
    model%forcing%Q2       = 0.008
    model%forcing%SOLDN    = 400.0
    model%forcing%PRCP     = 0.002
    model%forcing%PRCPCONV = 0.0005
    model%forcing%PRCPNONC = 0.0015
    model%forcing%PRCPSHCV = 0.0002
    model%forcing%PRCPSNOW = 0.0004
    model%forcing%PRCPGRPL = 0.0003
    model%forcing%PRCPHAIL = 0.0002
    model%forcing%UU       = 3.0
    model%forcing%VV       = -2.0
    model%forcing%FPICE    = 0.0
    model%energy%COSZ       = 0.5
    model%energy%COSZ_HORIZ = 0.5
  end subroutine baseline

  subroutine edge_cases(isnf, ncase)
    integer, intent(in)    :: isnf
    integer, intent(inout) :: ncase
    real    :: tfrz
    integer :: k

    tfrz = model%parameters%TFRZ

    ! The three Jordan (1991) temperature boundaries, approached from both sides, plus the
    ! rain-snow threshold itself. Exact equality decides which way each `<=` falls.
    do k = 1, 8
      call baseline()
      select case (k)
        case (1); model%forcing%SFCTMP = tfrz + 0.5
        case (2); model%forcing%SFCTMP = nearest(tfrz + 0.5, 1.0)
        case (3); model%forcing%SFCTMP = tfrz + 2.0
        case (4); model%forcing%SFCTMP = nearest(tfrz + 2.0, 1.0)
        case (5); model%forcing%SFCTMP = tfrz + 2.5
        case (6); model%forcing%SFCTMP = nearest(tfrz + 2.5, 1.0)
        case (7); model%forcing%SFCTMP = model%parameters%rain_snow_thresh
        case (8); model%forcing%SFCTMP = nearest(model%parameters%rain_snow_thresh, -1.0)
      end select
      call run_case(isnf, ncase)
    end do

    ! Shortwave: below the horizon on each cosine in turn, the sunrise case where the two
    ! cosines disagree, and a direct irradiance over the solar constant so the clamp fires.
    do k = 1, 6
      call baseline()
      select case (k)
        case (1); model%energy%COSZ = -0.1
        case (2); model%energy%COSZ_HORIZ = -0.1
        case (3); model%energy%COSZ = 0.0
        case (4); model%energy%COSZ_HORIZ = 0.0
        case (5); model%energy%COSZ_HORIZ = 0.001; model%forcing%SOLDN = 900.0
        case (6); model%energy%COSZ = 0.9; model%energy%COSZ_HORIZ = 0.2
      end select
      call run_case(isnf, ncase)
    end do

    ! Precipitation: none at all (FP stays zero), convective-only and non-convective-only, and
    ! -- for OPT_SNF 4 -- frozen precipitation of exactly zero, where `PRCPSNOW / PRCP_FROZEN`
    ! is 0/0 and bdfall goes NaN. That case is live upstream and the port must reproduce it.
    do k = 1, 8
      call baseline()
      select case (k)
        case (1)
          model%forcing%PRCP = 0.0; model%forcing%PRCPCONV = 0.0
          model%forcing%PRCPNONC = 0.0; model%forcing%PRCPSHCV = 0.0
        case (2); model%forcing%PRCPCONV = 0.0; model%forcing%PRCPSHCV = 0.0
        case (3); model%forcing%PRCPNONC = 0.0
        case (4)
          model%forcing%PRCPSNOW = 0.0; model%forcing%PRCPGRPL = 0.0
          model%forcing%PRCPHAIL = 0.0
        case (5); model%forcing%PRCPSNOW = 0.0; model%forcing%PRCPGRPL = 0.0
        case (6); model%forcing%PRCPNONC = 0.01; model%forcing%PRCPSNOW = 0.05
        case (7); model%forcing%UU = 0.0; model%forcing%VV = 0.0
        case (8); model%forcing%UU = 0.6; model%forcing%VV = 0.8
      end select
      call run_case(isnf, ncase)
    end do

    ! Inside the Jordan ramp, TFRZ+0.5 < SFCTMP <= TFRZ+2.0, which is the only place FPICE is
    ! anything but 0, 0.6 or 1 and so the only place an arithmetic slip in it is visible at
    ! all. The band is 1.5 K wide out of the 75 K the random sweep covers, so it needs
    ! sampling on purpose rather than by chance.
    do k = 1, 8
      call baseline()
      model%forcing%SFCTMP = tfrz + 0.5 + 1.5 * (real(k) - 0.5) / 8.0
      call run_case(isnf, ncase)
    end do

    ! Wind speeds at which `UU ** 2.` and a powf call disagree, taken from the intrinsic
    ! spike's own report rather than hoped for: the two spellings differ on 0.04% of inputs,
    ! which a sweep this size would reach by luck about once. All four are plausible wind
    ! speeds, so nothing here is contrived.
    do k = 1, 4
      call baseline()
      select case (k)
        case (1); model%forcing%UU =  0.861972749; model%forcing%VV =  8.78201866
        case (2); model%forcing%UU = 11.1071997;   model%forcing%VV = 66.1122665
        case (3); model%forcing%UU = -8.78201866;  model%forcing%VV = -11.1071997
        case (4); model%forcing%UU = 66.1122665;   model%forcing%VV =  0.861972749
      end select
      call run_case(isnf, ncase)
    end do
  end subroutine edge_cases

  subroutine random_case(isnf, ncase)
    integer, intent(in)    :: isnf
    integer, intent(inout) :: ncase

    model%forcing%SFCPRS   = draw(60000.0, 105000.0)
    ! Wide enough to cross every phase boundary, including the logistic regression's transition.
    model%forcing%SFCTMP   = draw(240.0, 315.0)
    model%forcing%Q2       = draw(0.0001, 0.030)
    model%forcing%SOLDN    = draw(0.0, 1400.0)
    model%forcing%PRCP     = draw(0.0, 0.01)
    model%forcing%PRCPCONV = draw(0.0, 0.004)
    model%forcing%PRCPNONC = draw(0.0, 0.008)
    model%forcing%PRCPSHCV = draw(0.0, 0.002)
    model%forcing%PRCPSNOW = draw(0.0, 0.006)
    model%forcing%PRCPGRPL = draw(0.0, 0.002)
    model%forcing%PRCPHAIL = draw(0.0, 0.001)
    ! Wind is swept over four decades because UR squares it: `UU ** 2.` is a real exponent, and
    ! the multiply gfortran folds it into disagrees with a powf call only on rare magnitudes.
    model%forcing%UU       = draw(-40.0, 40.0) * draw(0.001, 1.0)
    model%forcing%VV       = draw(-40.0, 40.0) * draw(0.001, 1.0)
    model%forcing%FPICE    = draw(0.0, 1.0)
    model%energy%COSZ       = draw(-0.2, 1.0)
    model%energy%COSZ_HORIZ = draw(-0.2, 1.0)

    call run_case(isnf, ncase)
  end subroutine random_case

end program atmsweep_driver
