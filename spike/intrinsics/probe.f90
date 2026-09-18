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

  character(len=32) :: arg, fname
  character(len=8)  :: hexin
  integer           :: ios, ibits, obits, n
  real              :: x, y

  call get_command_argument(1, arg)
  fname = trim(adjustl(arg))

  ! Runtime exponent for the pown* cases. Parsed from the name before the loop so it is a
  ! plain integer variable at the point of use: gfortran cannot constant-fold it, and must
  ! emit the same generic integer-power path the physics gets for `x ** ifrc`.
  n = 0
  if (fname(1:4) == 'pown') read(fname(5:5), '(I1)') n

  do
    read(*, '(A8)', iostat=ios) hexin
    if (ios /= 0) exit
    read(hexin, '(Z8)') ibits
    x = transfer(ibits, x)

    select case (trim(fname))
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
      ! Literal integer exponents: gfortran expands these inline rather than calling powf, and
      ! the expansion order decides the last bit. 0 through 4 are the only literal exponents
      ! that appear anywhere in noah-owp-modular's src/.
      case ('pow0')
        y = x ** 0
      case ('pow1')
        y = x ** 1
      case ('pow2')
        y = x ** 2
      case ('pow3')
        y = x ** 3
      case ('pow4')
        y = x ** 4
      ! Not used by the model; kept because it is where the candidates diverge most, so it is
      ! the sharpest test of whether the winning spelling is right for the right reason.
      case ('pow7')
        y = x ** 7
      ! `x ** (-1)`, as in SoilWaterRetentionCoeff.
      case ('powm1')
        y = x ** (-1)
      ! Runtime integer exponent, as in `x ** ifrc` and `x ** (CVFRZ - J)`. This is a
      ! different code path from the literal cases -- a libcall, not an inline expansion.
      case ('pown2', 'pown3', 'pown4')
        y = x ** n
      ! Real exponent: a genuine powf call.
      case ('powr')
        y = x ** 0.6666667
      case default
        write(0, '(A)') 'unknown function: ' // trim(fname)
        stop 1
    end select

    obits = transfer(y, obits)
    write(*, '(A8,1X,Z8.8)') hexin, obits
  end do

end program probe
