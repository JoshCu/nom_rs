! Phase 0 intrinsic-drift probe -- reference side.
!
! Reads f32 bit patterns as 8 hex digits, one per line, on stdin. Applies the intrinsic named
! by the first command-line argument. Writes "<input hex> <output hex>" per line.
!
! Comparing bit patterns rather than printed decimals is the whole point: a decimal round-trip
! would hide exactly the 1-ULP differences we are hunting.
!
! Build:  gfortran -O2 -ffp-contract=off -o probe_f90 probe.f90
!
! Flags matter. -ffast-math or -ffp-contract=fast would let the compiler transform these
! expressions and the comparison would be measuring the flags, not the libm.

program probe
  implicit none

  character(len=32) :: fname
  character(len=8)  :: hexin
  integer           :: ios, ibits, obits
  real              :: x, y

  call get_command_argument(1, fname)

  do
    read(*, '(A8)', iostat=ios) hexin
    if (ios /= 0) exit
    read(hexin, '(Z8)') ibits
    x = transfer(ibits, x)

    select case (trim(adjustl(fname)))
      case ('exp')
        y = exp(x)
      case ('log')
        y = log(x)
      case ('sqrt')
        y = sqrt(x)
      case ('sin')
        y = sin(x)
      case ('cos')
        y = cos(x)
      case ('tanh')
        y = tanh(x)
      ! Integer exponents: gfortran expands small literal powers inline rather than calling
      ! powf, and the expansion order decides the result.
      case ('pow2')
        y = x ** 2
      case ('pow3')
        y = x ** 3
      case ('pow7')
        y = x ** 7
      ! Real exponent: a genuine powf call.
      case ('powr')
        y = x ** 0.6666667
      case default
        write(0, '(A)') 'unknown function: ' // trim(adjustl(fname))
        stop 1
    end select

    obits = transfer(y, obits)
    write(*, '(A8,1X,Z8.8)') hexin, obits
  end do

end program probe
