! Differential-fixture generator: runs the reference Fortran and dumps the state around each
! physics call for a sample of timesteps.
!
! Deliberately not a BMI program. `noah-owp-modular` has no CMake of its own -- `libsurfacebmi.so`
! is built by a wrapper that lives in ngen's `extern/`, together with the iso_c_fortran_bmi
! middleware -- so going through BMI here would drag in ngen for no benefit. Driving the model
! directly also keeps the ASCII forcing reader available, which is what lets this run the
! Bondville year from `data/bondville.dat` with no NetCDF anywhere in the build.
!
! Usage:  difftest <namelist> <fixture_out> [nsamples]

program difftest_driver

  use RunModule
  use DiffTestModule

  implicit none

  type(noahowp_type)  :: model
  character(len=512)  :: config_file, fixture_file, arg
  integer             :: nsamples, head, stride

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: difftest <namelist> <fixture_out> [nsamples]'
    stop 2
  end if
  call get_command_argument(1, config_file)
  call get_command_argument(2, fixture_file)

  nsamples = 200
  if (command_argument_count() >= 3) then
    call get_command_argument(3, arg)
    read(arg, *) nsamples
  end if

  call initialize_from_file(model, trim(config_file))

  call dt_open(trim(fixture_file))
  call dt_static(model%levels, model%options, model%parameters)

  ! Spin-up is where the state moves fastest and where a ported routine is most likely to be
  ! exercised away from its steady state, so the opening timesteps are all sampled. The rest
  ! are spread evenly over the run so every season is represented -- snow physics is only
  ! reachable in winter, and a fixture set drawn from July would not touch it at all.
  head   = min(24, max(1, nsamples / 4))
  stride = max(1, (model%domain%ntime - head) / max(1, nsamples - head))

  do while (model%domain%time_dbl < model%domain%ntime * model%domain%dt)
    call dt_arm(model%domain%itime <= head .or. mod(model%domain%itime, stride) == 0)
    call advance_in_time(model)
  end do

  call dt_close()

  write(*, '(A,I0,A,I0,A,I0)') 'timesteps=', model%domain%itime - 1, ' head=', head, &
                               ' stride=', stride

end program difftest_driver
