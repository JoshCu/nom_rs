! Water sweep fixture generator: every runoff, drainage, infiltration and subsurface option,
! and a climate that actually builds a layered snowpack.
!
! Bondville runs runoff_option = 8, drainage_option = 8, frozen_soil_option = 1,
! dynamic_vic_option = 1 and subsurface_option = 1 for the whole year, so seven of the eight
! surface runoff schemes, both TOPMODEL water tables, the MMF drainage, the second
! diffusivity form and the one-way-coupled subsurface are never entered. Its winter is thin as
! well: over 200 sampled timesteps WaterMain never starts with more than one snow layer, so
! DIVIDE and COMBO are never reached at all.
!
! Each configuration replays the Bondville year from initialisation, in the recorded climate
! and in a colder, wetter one (SFCTMP - 8 K, PRCP x 2) where the pack builds up in layers and
! melts out again. The five physics calls are made here in solve_noahowp's order rather than
! through it, so that only WaterMain is recorded and the record id can carry the
! configuration:
!
!     id = ((((run * 10 + drn) * 10 + inf) * 10 + infdv) * 10 + sub) * 10 + climate) * 100 + sample
!
! A final phase starts from states the replays reached and pushes single fields to where no
! replay goes: the glacier cap, a lake, frozen ground under a snow-free surface, the snowpack
! gone after COMBINE. Those carry climate = 9 and the configuration they were derived under.
!
! Usage:  watersweep <namelist> <fixture_out>

program watersweep_driver

  use RunModule
  use UtilitiesModule
  use ForcingModule
  use InterceptionModule
  use EnergyModule
  use WaterModule
  use AsciiReadModule
  use DateTimeUtilsModule
  use DiffTestModule
  use, intrinsic :: ieee_arithmetic, only: ieee_is_nan

  implicit none

  type(noahowp_type), allocatable :: model
  character(len=512) :: config_file, fixture_file
  integer            :: irun, iinf, idv, iclim
  integer(kind=8)    :: rng
  logical            :: static_written
  ! The forcing year, read once. read_forcing_text keeps its position in saved locals that only
  ! reset at end of file, so it cannot be restarted for a second replay.
  real, allocatable  :: f_uu(:), f_vv(:), f_sfctmp(:), f_q2(:), f_sfcprs(:), f_soldn(:), &
                        f_lwdn(:), f_prcp(:)

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: watersweep <namelist> <fixture_out>'
    stop 2
  end if
  call get_command_argument(1, config_file)
  call get_command_argument(2, fixture_file)

  call read_forcing_year()
  call dt_open(trim(fixture_file))
  static_written = .false.
  rng = 1

  ! Every runoff scheme paired with the drainage scheme of the same number -- which is how
  ! the options are documented to be used, and which reaches every drainage branch too --
  ! under both diffusivity forms and both climates.
  do irun = 1, 8
    do iinf = 1, 2
      do iclim = 0, 1
        call replay(irun, irun, iinf, 1, 1, iclim)
      end do
    end do
  end do
  ! The other two dynamic VIC infiltration equations.
  do idv = 2, 3
    do iinf = 1, 2
      do iclim = 0, 1
        call replay(8, 8, iinf, idv, 1, iclim)
      end do
    end do
  end do
  ! One-way coupling holds the soil column at its initial state.
  do iclim = 0, 1
    call replay(8, 8, 1, 1, 2, iclim)
    call replay(3, 3, 2, 1, 2, iclim)
  end do
  ! Runoff and drainage need not agree; these are the cross pairings where one reads what the
  ! other writes -- ZWT from ZWTEQ, FCRMAX, the MMF water table.
  do iclim = 0, 1
    call replay(1, 2, 1, 1, 1, iclim)
    call replay(2, 1, 1, 1, 1, iclim)
    call replay(5, 2, 1, 1, 1, iclim)
    call replay(3, 5, 1, 1, 1, iclim)
    call replay(8, 5, 2, 1, 1, iclim)
    call replay(6, 4, 1, 1, 1, iclim)
  end do

  call dt_close()
  write(*, '(A)') 'water sweep written'

contains

  real function draw(lo, hi)
    real, intent(in) :: lo, hi
    rng  = mod(rng * 1664525_8 + 1013904223_8, 4294967296_8)
    draw = lo + (hi - lo) * real(rng) / 4294967296.0
  end function draw

  subroutine read_forcing_year()
    integer :: n, forcing_timestep, ierr
    integer :: read_yr, read_mo, read_dy, read_hr, read_mi
    allocate(model)
    call initialize_from_file(model, trim(config_file))
    n = model%domain%ntime
    allocate(f_uu(n), f_vv(n), f_sfctmp(n), f_q2(n), f_sfcprs(n), f_soldn(n), f_lwdn(n), &
             f_prcp(n))
    associate(domain => model%domain)
    do while (domain%time_dbl < domain%ntime * domain%dt)
      ! The reader keys on the beginning-of-timestep date, as solve_noahowp computes it.
      forcing_timestep = domain%dt
      call advance_datetime(domain%start_year, domain%start_month, domain%start_day, &
                            domain%start_hour, domain%start_minute,                  &
                            int((domain%itime - 1) * (domain%dt / 60)),              &
                            read_yr, read_mo, read_dy, read_hr, read_mi)
      call read_forcing_text(10, read_yr, read_mo, read_dy, read_hr, read_mi, &
           forcing_timestep, f_uu(domain%itime), &
           f_vv(domain%itime), f_sfctmp(domain%itime), f_q2(domain%itime), &
           f_sfcprs(domain%itime), f_soldn(domain%itime), f_lwdn(domain%itime), &
           f_prcp(domain%itime), ierr)
      call UtilitiesMain(domain%itime, domain, model%forcing, model%energy)
      domain%itime    = domain%itime + 1
      domain%time_dbl = dble(domain%time_dbl + domain%dt)
    end do
    end associate
    close(10)
    deallocate(model)
  end subroutine read_forcing_year

  integer function pack_id(run, drn, inf, infdv, sub, clim, sample)
    integer, intent(in) :: run, drn, inf, infdv, sub, clim, sample
    if (sample > 99) stop 'watersweep: more than 100 samples under one configuration'
    pack_id = (((((run * 10 + drn) * 10 + inf) * 10 + infdv) * 10 + sub) * 10 + clim) * 100 &
              + sample
  end function pack_id

  subroutine record_water(id)
    integer, intent(in) :: id
    call dt_arm(.true.)
    call dt_record(5, 0, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    call WaterMain(model%domain, model%levels, model%options, model%parameters, &
                   model%forcing, model%energy, model%water)
    call dt_record(5, 1, id, model%domain, model%forcing, model%energy, model%water, &
                   model%parameters)
    call dt_arm(.false.)
  end subroutine record_water

  !> One year under one configuration.
  subroutine replay(run, drn, inf, infdv, sub, clim)
    integer, intent(in) :: run, drn, inf, infdv, sub, clim
    ! Per-category sampling budgets, so a year with a long snow season does not spend all of
    ! them in December. Candidates are thinned by a draw before the budget is charged.
    integer, parameter :: B_LAYERED = 6, B_SHALLOW = 3, B_WET = 3, B_FROZEN = 2, B_ANY = 2
    integer :: n_layered, n_shallow, n_wet, n_frozen, n_any, n_edge, sample
    integer :: curr_yr, curr_mo, curr_dy, curr_hr, curr_min, curr_sec
    logical :: take

    if (allocated(model)) deallocate(model)
    allocate(model)
    call initialize_from_file(model, trim(config_file))
    close(10)
    model%options%opt_run   = run
    model%options%opt_drn   = drn
    model%options%opt_inf   = inf
    model%options%opt_infdv = infdv
    model%options%opt_sub   = sub
    if (.not. static_written) then
      call dt_static(model%levels, model%options, model%parameters)
      static_written = .true.
    end if

    n_layered = 0; n_shallow = 0; n_wet = 0; n_frozen = 0; n_any = 0; n_edge = 0
    sample = 0

    associate(domain => model%domain, forcing => model%forcing, energy => model%energy, &
              water => model%water, parameters => model%parameters, &
              options => model%options, levels => model%levels)
    do while (domain%time_dbl < domain%ntime * domain%dt)
      domain%curr_datetime = domain%sim_datetimes(domain%itime)
      call unix_to_date(domain%curr_datetime, curr_yr, curr_mo, curr_dy, curr_hr, curr_min, &
                        curr_sec)
      forcing%UU     = f_uu(domain%itime)
      forcing%VV     = f_vv(domain%itime)
      forcing%SFCTMP = f_sfctmp(domain%itime)
      forcing%Q2     = f_q2(domain%itime)
      forcing%SFCPRS = f_sfcprs(domain%itime)
      forcing%SOLDN  = f_soldn(domain%itime)
      forcing%LWDN   = f_lwdn(domain%itime)
      forcing%PRCP   = f_prcp(domain%itime)
      if (clim == 1) then
        forcing%SFCTMP = forcing%SFCTMP - 8.0
        forcing%PRCP   = forcing%PRCP * 2.0
      end if

      call UtilitiesMain(domain%itime, domain, forcing, energy)
      call ForcingMain(options, parameters, forcing, energy, water)
      call InterceptionMain(domain, levels, options, parameters, forcing, energy, water)
      call EnergyMain(domain, levels, options, parameters, forcing, energy, water)

      take = .false.
      if (water%ISNOW < 0) then
        if (n_layered < B_LAYERED .and. draw(0.0, 1.0) < 0.01) then
          n_layered = n_layered + 1; take = .true.
        end if
      else if (water%SNEQV > 0.0) then
        if (n_shallow < B_SHALLOW .and. draw(0.0, 1.0) < 0.02) then
          n_shallow = n_shallow + 1; take = .true.
        end if
      else if (water%QRAIN > 0.0) then
        if (n_wet < B_WET .and. draw(0.0, 1.0) < 0.02) then
          n_wet = n_wet + 1; take = .true.
        end if
      else if (energy%TG <= parameters%TFRZ) then
        if (n_frozen < B_FROZEN .and. draw(0.0, 1.0) < 0.01) then
          n_frozen = n_frozen + 1; take = .true.
        end if
      else if (n_any < B_ANY .and. draw(0.0, 1.0) < 0.001) then
        n_any = n_any + 1; take = .true.
      end if

      if (take) then
        call record_water(pack_id(run, drn, inf, infdv, sub, clim, sample))
        sample = sample + 1
        ! Edge cases derived from the first few states sampled. Three sets, each on the
        ! replays where it means something; together they keep the sample number below 100.
        if (n_edge < 3) then
          n_edge = n_edge + 1
          if (clim == 1 .and. run == 8 .and. drn == 8 .and. inf == 1 .and. infdv == 1 &
              .and. sub == 1) call snow_and_surface_cases(run, drn, inf, infdv, sub, sample)
          if (run == 8 .and. sub == 1) &
            call infiltration_capacity_cases(run, drn, inf, infdv, sub, sample)
          if (drn == 5) call deep_water_table_cases(run, drn, inf, infdv, sub, sample)
        end if
      else
        call WaterMain(domain, levels, options, parameters, forcing, energy, water)
      end if

      domain%itime    = domain%itime + 1
      domain%time_dbl = dble(domain%time_dbl + domain%dt)

      ! Some legal option pairings drive the column out of physical range -- upstream has no
      ! guard -- and EnergyMain then STOPs on negative emitted longwave, taking the sweep with
      ! it. Everything recorded up to here is still a valid pre/post pair, so end this replay
      ! and keep what it produced.
      if (.not. sane()) then
        write(*, '(A,6I3,A,I6,A,F12.2,A,I3)') 'replay', run, drn, inf, infdv, sub, clim, &
          ' left physical range at itime', domain%itime, ' TG=', energy%TG, &
          ' samples=', sample
        exit
      end if
    end do
    end associate
  end subroutine replay

  logical function sane()
    associate(e => model%energy, w => model%water)
    sane = e%TG > 150.0 .and. e%TG < 400.0 .and. e%TV > 150.0 .and. e%TV < 400.0 &
           .and. .not. any(ieee_is_nan(w%SH2O)) &
           .and. all(e%STC(w%ISNOW+1:) > 150.0) .and. all(e%STC(w%ISNOW+1:) < 400.0)
    end associate
  end function sane

  !> Push the current (post-EnergyMain) state to places no replay reaches, one field at a
  !> time, recording WaterMain from each. The model is restored afterwards so the replay
  !> continues on its own trajectory.
  subroutine snow_and_surface_cases(run, drn, inf, infdv, sub, sample)
    integer, intent(in)    :: run, drn, inf, infdv, sub
    integer, intent(inout) :: sample
    type(noahowp_type), allocatable :: saved
    integer :: k

    allocate(saved)
    saved = model
    do k = 1, 11
      model = saved
      select case (k)
        ! Glacier: more than 5000 mm of snow water in the pack.
        case (1)
          if (model%water%ISNOW < 0) then
            model%water%SNICE(0) = model%water%SNICE(0) + 6000.0
            model%water%SNEQV    = model%water%SNEQV + 6000.0
          else
            cycle
          end if
        ! A lake point, over and under its storage cap.
        case (2); model%domain%IST = 2; model%water%WSLAKE = model%parameters%WSLMAX + 1.0
        case (3); model%domain%IST = 2; model%water%WSLAKE = 0.0
        ! Frozen ground with dew and evaporation both possible.
        case (4); model%energy%TG = model%parameters%TFRZ - 5.0
        ! Sublimation far larger than the surface layer holds, so COMBINE drops it.
        case (5)
          if (model%water%ISNOW < 0) then
            model%energy%FGEV = 5000.0
          else
            cycle
          end if
        ! Very heavy surface input: the NITER doubling, and the dynamic VIC saturated paths.
        case (6); model%water%QRAIN = 0.05; model%water%PONDING = 50.0
        ! COMBINE's two ways of merging a layer without the pack melting out: a top layer
        ! with almost no ice left, and one thinner than DZMIN that still has ice to keep.
        case (7)
          if (model%water%ISNOW <= -2) then
            model%water%SNICE(model%water%ISNOW+1) = 0.05
          else
            cycle
          end if
        case (8)
          if (model%water%ISNOW <= -2) then
            model%water%SNICE(model%water%ISNOW+1)   = 5.0
            model%domain%DZSNSO(model%water%ISNOW+1) = 0.01
          else
            cycle
          end if
        ! Every layer thin enough that the whole pack drops below 0.025 m and is folded back
        ! into shallow snow, its liquid water ponding.
        case (9)
          if (model%water%ISNOW < 0) then
            model%domain%DZSNSO(model%water%ISNOW+1:0) = 0.005
            model%water%SNICE(model%water%ISNOW+1:0)   = 1.0
            model%water%SNLIQ(model%water%ISNOW+1:0)   = 0.5
          else
            cycle
          end if
        ! A single layer thick enough for DIVIDE to split -- into two, and on into three.
        ! Forced whatever the pack was: the layers above 0 become empty, and SnowWater zeroes
        ! empty layers itself.
        case (10, 11)
          model%water%ISNOW      = -1
          model%water%SNLIQ(0)   = 0.5
          model%energy%STC(0)    = 270.0
          model%water%FICEOLD(0) = 1.0
          if (k == 10) then
            model%domain%DZSNSO(0) = 0.08; model%water%SNICE(0) = 15.0
          else
            model%domain%DZSNSO(0) = 0.60; model%water%SNICE(0) = 120.0
          end if
          model%water%SNEQV = model%water%SNICE(0) + model%water%SNLIQ(0)
      end select
      call record_water(pack_id(run, drn, inf, infdv, sub, 9, sample))
      sample = sample + 1
    end do
    model = saved
    deallocate(saved)
  end subroutine snow_and_surface_cases

  !> DYNAMIC_VIC's branches where more water arrives in a sub-step than the soil can take in
  !> (FMAX*DT < DP). The infiltration capacity is smallest with a wet top layer, which starves
  !> the sorptivity; the second layer then decides whether DP + I_0 overtops I_MAX. These are
  !> also the two paths where upstream reads YD before assigning it.
  subroutine infiltration_capacity_cases(run, drn, inf, infdv, sub, sample)
    integer, intent(in)    :: run, drn, inf, infdv, sub
    integer, intent(inout) :: sample
    type(noahowp_type), allocatable :: saved
    real, parameter :: TOP(2) = [0.999, 0.9], POND(3) = [5.0, 30.0, 150.0]
    integer :: i, j, k

    allocate(saved)
    saved = model
    do i = 1, 2
      do j = 1, 2
        do k = 1, 3
          model = saved
          associate(w => model%water, p => model%parameters)
          w%SICE(1:2) = 0.0
          w%SH2O(1)   = TOP(i) * p%SMCMAX(1)
          w%SH2O(2)   = merge(p%SMCWLT(2), p%SMCMAX(2), j == 1)
          w%SMC(1:2)  = w%SH2O(1:2)
          w%PONDING   = POND(k)
          end associate
          call record_water(pack_id(run, drn, inf, infdv, sub, 9, sample))
          sample = sample + 1
        end do
      end do
    end do
    model = saved
    deallocate(saved)
  end subroutine infiltration_capacity_cases

  !> MMF drainage with the water table below the auxiliary layer, where SSTEP accumulates
  !> recharge rather than updating SMCWTD.
  subroutine deep_water_table_cases(run, drn, inf, infdv, sub, sample)
    integer, intent(in)    :: run, drn, inf, infdv, sub
    integer, intent(inout) :: sample
    type(noahowp_type), allocatable :: saved
    integer :: k

    allocate(saved)
    saved = model
    do k = 1, 2
      model = saved
      model%water%ZWT = merge(-5.0, -3.5, k == 1)
      call record_water(pack_id(run, drn, inf, infdv, sub, 9, sample))
      sample = sample + 1
    end do
    model = saved
    deallocate(saved)
  end subroutine deep_water_table_cases

end program watersweep_driver
