! Interception sweep fixture generator: every dveg branch and every special vegetation type.
!
! Bondville runs dynamic_veg_option = 1, vegtyp = 1, croptype = 0 for the whole year. That
! leaves eight of the nine dveg branches in PHENOLOGY unreached, along with the southern
! hemisphere shift, the short-canopy snow-burial formula, the water/barren/ice/urban zeroing,
! the crop path, and -- because LAI and SAI never reach zero at Bondville -- the entire
! buried-canopy branch of CanopyWaterIntercept.
!
! The configuration each case ran under travels in the record id:
!
!     id = ((dveg * 32 + vegtyp) * 2 + croptype) * 100 + sample
!
! Everything else the port reads comes back out of the recorded pre-state, so the two sides
! agree on one integer and nothing more.
!
! Usage:  interceptsweep <namelist> <fixture_out>

program interceptsweep_driver

  use RunModule
  use InterceptionModule
  use DiffTestModule

  implicit none

  type(noahowp_type) :: model
  character(len=512) :: config_file, fixture_file
  integer            :: idveg, iveg, icrop, i, ncase
  integer(kind=8)    :: rng

  ! vegtyp 1 is Bondville's. Under MODIFIED_IGBP_MODIS_NOAH, 13 is urban, 15 ice, 16 barren
  ! and 17 water -- the four that zero LAI and SAI outright -- while 10 (HVT = 1.00) and 20
  ! (HVT = 0.50) are the short canopies that take the SNOWHC branch instead of the linear one.
  integer, parameter :: NVEG = 7
  integer, parameter :: VEGS(NVEG) = [1, 10, 13, 15, 16, 17, 20]
  ! Two samples per configuration is enough for the branch sweep, which is about reaching code
  ! rather than about numerical spread; the depth comes from the second phase below.
  integer, parameter :: NBRANCH = 2, NDEPTH = 40

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: interceptsweep <namelist> <fixture_out>'
    stop 2
  end if
  call get_command_argument(1, config_file)
  call get_command_argument(2, fixture_file)

  call initialize_from_file(model, trim(config_file))
  call dt_open(trim(fixture_file))
  call dt_static(model%levels, model%options, model%parameters)
  call dt_arm(.true.)

  rng = 1

  ! Phase one: reach every branch.
  do idveg = 1, 9
    do iveg = 1, NVEG
      do icrop = 0, 1
        call configure(idveg, VEGS(iveg), icrop)
        do i = 1, NBRANCH
          ncase = i - 1
          call random_case(idveg, VEGS(iveg), icrop, ncase)
        end do
      end do
    end do
  end do

  ! Phase two: numerical depth, on a tall canopy and a short one. The canopy water balance is
  ! the same code whatever dveg selected, so it only needs covering once -- but it needs
  ! covering well.
  do iveg = 1, 2
    call configure(1, merge(1, 20, iveg == 1), 0)
    call edge_cases(1, merge(1, 20, iveg == 1), 0, ncase)
    do i = 1, NDEPTH
      call random_case(1, merge(1, 20, iveg == 1), 0, ncase)
    end do
  end do

  ! Phase three: the 0.05 cutoffs, under a dveg that leaves LAI and SAI where they were put.
  do iveg = 1, 2
    call configure(2, merge(1, 20, iveg == 1), 0)
    call cutoff_cases(2, merge(1, 20, iveg == 1), 0, ncase)
  end do

  call dt_close()
  write(*, '(A)') 'interception sweep written'

contains

  !> Rebuild options and parameters for one configuration. `paramRead` reads the vegetation
  !> row, so changing vegtyp without re-reading would leave LAIM, HVT and the rest on the
  !> previous type's values.
  subroutine configure(dveg, vegtyp, croptype)
    integer, intent(in) :: dveg, vegtyp, croptype
    model%namelist%dynamic_veg_option = dveg
    model%namelist%vegtyp             = vegtyp
    model%namelist%croptype           = croptype
    call model%options%InitTransfer(model%namelist)
    call model%domain%InitTransfer(model%namelist)
    if (allocated(model%parameters%bexp)) then
      deallocate(model%parameters%bexp, model%parameters%smcmax, model%parameters%smcwlt, &
                 model%parameters%smcref, model%parameters%dksat, model%parameters%dwsat, &
                 model%parameters%psisat)
    end if
    call model%parameters%Init(model%namelist)
    call model%parameters%paramRead(model%namelist)
  end subroutine configure

  real function draw(lo, hi)
    real, intent(in) :: lo, hi
    rng  = mod(rng * 1664525_8 + 1013904223_8, 4294967296_8)
    draw = lo + (hi - lo) * real(rng) / 4294967296.0
  end function draw

  subroutine run_case(dveg, vegtyp, croptype, ncase)
    integer, intent(in)    :: dveg, vegtyp, croptype
    integer, intent(inout) :: ncase
    integer :: id
    id = ((dveg * 32 + vegtyp) * 2 + croptype) * 100 + ncase
    call dt_record(3, 0, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    call InterceptionMain(model%domain, model%levels, model%options, model%parameters, &
                          model%forcing, model%energy, model%water)
    call dt_record(3, 1, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    ncase = ncase + 1
  end subroutine run_case

  subroutine baseline()
    model%domain%DT     = 3600.0
    model%domain%lat    = 40.0
    model%domain%ist    = 1
    model%forcing%JULIAN  = 120.0
    model%forcing%YEARLEN = 365
    model%forcing%UU      = 3.0
    model%forcing%VV      = -2.0
    model%energy%TV     = 285.0
    model%energy%TG     = 283.0
    model%water%SNOWH   = 0.1
    model%water%rain    = 0.002
    model%water%snow    = 0.0005
    model%water%bdfall  = 100.0
    model%water%FP      = 0.8
    model%water%canliq  = 0.3
    model%water%canice  = 0.05
    ! LAI and SAI carry over from one call to the next under dveg options that do not
    ! recompute them, so they are inputs as much as outputs.
    model%parameters%LAI = 2.0
    model%parameters%SAI = 0.5
  end subroutine baseline

  subroutine edge_cases(dveg, vegtyp, croptype, ncase)
    integer, intent(in)    :: dveg, vegtyp, croptype
    integer, intent(inout) :: ncase
    integer :: k

    ncase = 0
    do k = 1, 16
      call baseline()
      select case (k)
        ! The southern hemisphere half-year shift, and the month-interpolation wrap at both
        ! ends of the year where IT1 falls below 1 and IT2 above 12.
        case (1);  model%domain%lat = -40.0
        case (2);  model%domain%lat = -40.0; model%forcing%JULIAN = 0.0
        case (3);  model%forcing%JULIAN = 0.0
        case (4);  model%forcing%JULIAN = 365.0
        case (5);  model%forcing%JULIAN = 182.5; model%forcing%YEARLEN = 366
        ! No canopy at all: the buried-canopy branch, with and without water left on it.
        case (6);  model%parameters%LAI = 0.0; model%parameters%SAI = 0.0
        case (7)
          model%parameters%LAI = 0.0; model%parameters%SAI = 0.0
          model%water%canliq = 0.0; model%water%canice = 0.0
        case (8)
          model%parameters%LAI = 0.0; model%parameters%SAI = 0.0
          model%water%canliq = 5.0; model%water%canice = 3.0
        ! Just either side of the 0.05 cutoffs that zero LAI, SAI and ESAI.
        case (9);  model%parameters%LAI = 0.04; model%parameters%SAI = 0.04
        case (10); model%parameters%LAI = 0.06; model%parameters%SAI = 0.06
        ! Snow burial: none, partial, and deeper than the canopy is tall.
        case (11); model%water%SNOWH = 0.0
        case (12); model%water%SNOWH = 25.0
        case (13); model%water%SNOWH = 0.005
        ! The growing-season flag either side of TMIN, and the lake case that suppresses
        ! snowfall reaching the ground.
        case (14); model%energy%TV = model%parameters%TMIN - 1.0
        case (15); model%domain%ist = 2; model%energy%TG = model%parameters%TFRZ + 1.0
        case (16); model%domain%DT = 900.0; model%water%snow = 0.01; model%water%rain = 0.0
      end select
      call run_case(dveg, vegtyp, croptype, ncase)
    end do
  end subroutine edge_cases

  !> The four 0.05 cutoffs in PHENOLOGY, approached deliberately.
  !>
  !> Must run under a dveg that leaves LAI and SAI alone -- 1, 3 and 4 overwrite both from the
  !> monthly tables on entry, so a hand-set value never survives to be tested. dveg 2 keeps
  !> them and computes FVEG from them instead.
  subroutine cutoff_cases(dveg, vegtyp, croptype, ncase)
    integer, intent(in)    :: dveg, vegtyp, croptype
    integer, intent(inout) :: ncase
    integer :: k

    ncase = 0
    ! SAI and LAI exactly on the cutoff and at the next representable value either side. With
    ! SNOWH = 0 nothing is buried, so ESAI is SAI and ELAI is LAI.
    do k = 1, 6
      call baseline()
      model%water%SNOWH = 0.0
      select case (k)
        case (1); model%parameters%SAI = 0.05
        case (2); model%parameters%SAI = nearest(0.05, -1.0)
        case (3); model%parameters%SAI = nearest(0.05, 1.0)
        case (4); model%parameters%LAI = 0.05
        case (5); model%parameters%LAI = nearest(0.05, -1.0)
        case (6); model%parameters%LAI = 0.05; model%parameters%SAI = 0.05
      end select
      call run_case(dveg, vegtyp, croptype, ncase)
    end do

    ! ESAI and ELAI are never set directly -- they are SAI and LAI scaled by the unburied
    ! fraction -- and SAI below 0.05 is zeroed before they are computed, so the only way to
    ! land ESAI just under its own cutoff is to start above it and let the snow take the
    ! difference. Sweeping the snow depth across the burial fraction that does it is more
    ! robust than solving for one value, which would depend on the canopy heights of whichever
    ! vegetation type this configuration selected.
    do k = 1, 40
      call baseline()
      model%parameters%LAI = 0.06
      model%parameters%SAI = 0.06
      model%water%SNOWH    = 0.002 * real(k)
      call run_case(dveg, vegtyp, croptype, ncase)
    end do

    ! The same sweep with a stem area well clear of its own cutoff, so ESAI stays above 0.05
    ! while ELAI crosses it. With LAI and SAI equal, ESAI is zeroed first and `ESAI == 0.0`
    ! then forces ELAI to zero whichever way ELAI's own comparison falls -- masking it.
    do k = 1, 40
      call baseline()
      model%parameters%LAI = 0.06
      model%parameters%SAI = 0.50
      model%water%SNOWH    = 0.002 * real(k)
      call run_case(dveg, vegtyp, croptype, ncase)
    end do
  end subroutine cutoff_cases

  subroutine random_case(dveg, vegtyp, croptype, ncase)
    integer, intent(in)    :: dveg, vegtyp, croptype
    integer, intent(inout) :: ncase

    model%domain%DT       = merge(3600.0, 900.0, draw(0.0, 1.0) > 0.5)
    model%domain%lat      = draw(-60.0, 60.0)
    model%domain%ist      = merge(1, 2, draw(0.0, 1.0) > 0.3)
    model%forcing%JULIAN  = draw(0.0, 366.0)
    model%forcing%YEARLEN = merge(365, 366, draw(0.0, 1.0) > 0.25)
    model%forcing%UU      = draw(-20.0, 20.0)
    model%forcing%VV      = draw(-20.0, 20.0)
    model%energy%TV       = draw(250.0, 310.0)
    model%energy%TG       = draw(250.0, 310.0)
    model%water%SNOWH     = draw(0.0, 5.0)
    model%water%rain      = draw(0.0, 0.02)
    model%water%snow      = draw(0.0, 0.01)
    model%water%bdfall    = draw(50.0, 120.0)
    model%water%FP        = draw(0.0, 1.0)
    model%water%canliq    = draw(0.0, 5.0)
    model%water%canice    = draw(0.0, 5.0)
    model%parameters%LAI  = draw(0.0, 6.0)
    model%parameters%SAI  = draw(0.0, 1.5)

    call run_case(dveg, vegtyp, croptype, ncase)
  end subroutine random_case

end program interceptsweep_driver
