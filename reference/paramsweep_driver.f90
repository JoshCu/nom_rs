! Parameter-sweep fixture generator: `paramRead` over every table row, not just Bondville's.
!
! The Bondville configuration exercises exactly one vegetation type, one soil texture and one
! soil colour. That leaves most of every table unread, so a column misread in the Rust port
! would go unnoticed. This walks each class index in turn and records the parameters it
! produces.
!
! One dimension at a time rather than the full cross product: the class indices select
! independent table rows, and the only expressions that mix them -- `kdt` and `frzx` -- are
! functions of the soil row alone. 65 cases per dataset instead of 6,480, for the same
! coverage of every table entry.
!
! Indices deliberately run past the number of rows the file supplies, up to the declared table
! size, because that is in bounds for the Fortran and returns the -1.E36 sentinel. Reproducing
! the sentinel is part of the contract.
!
! Usage:  paramsweep <namelist> <fixture_out>

program paramsweep_driver

  use NamelistRead
  use ParametersType
  use DiffTestModule

  implicit none

  type(namelist_type)   :: namelist
  type(parameters_type) :: parameters
  character(len=512)    :: config_file, fixture_file
  integer               :: ids, ix

  ! Must match reference/gen_serializer.py's table sizes.
  integer, parameter :: MVT = 27, MAX_SOILTYP = 30, MSC = 8

  if (command_argument_count() < 2) then
    write(0, '(A)') 'usage: paramsweep <namelist> <fixture_out>'
    stop 2
  end if
  call get_command_argument(1, config_file)
  call get_command_argument(2, fixture_file)

  call namelist%ReadNamelist(trim(config_file))
  call dt_open(trim(fixture_file))

  do ids = 1, 2
    if (ids == 1) then
      namelist%veg_class_name = "USGS"
    else
      namelist%veg_class_name = "MODIFIED_IGBP_MODIS_NOAH"
    end if

    do ix = 1, MVT
      call one_case(ids, ix, 1, 1)
    end do
    do ix = 1, MAX_SOILTYP
      call one_case(ids, 1, ix, 1)
    end do
    do ix = 1, MSC
      call one_case(ids, 1, 1, ix)
    end do
  end do

  call dt_close()
  write(*, '(A)') 'parameter sweep written'

contains

  subroutine one_case(dataset, vegtyp, isltyp, soilcolor)
    integer, intent(in) :: dataset, vegtyp, isltyp, soilcolor
    ! InitAllocate allocates unconditionally, so a second Init on the same object would
    ! abort on an already-allocated component.
    if (allocated(parameters%bexp)) then
      deallocate(parameters%bexp, parameters%smcmax, parameters%smcwlt, parameters%smcref, &
                 parameters%dksat, parameters%dwsat, parameters%psisat)
    end if
    namelist%vegtyp    = vegtyp
    namelist%isltyp    = isltyp
    namelist%soilcolor = soilcolor
    call parameters%Init(namelist)
    call parameters%paramRead(namelist)
    call dt_params(dataset, vegtyp, isltyp, soilcolor, parameters)
  end subroutine one_case

end program paramsweep_driver
