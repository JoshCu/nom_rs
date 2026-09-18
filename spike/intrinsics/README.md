# Phase 0: intrinsic drift

**The go/no-go on "byte identical".** Everything else in the port is mechanical translation
that a differential test can verify. This is the one question that could make strict
bit-identity unachievable no matter how carefully the physics is translated, so it is answered
first, before the ~9k lines of physics get written.

## The question

Noah-OWP's physics calls `EXP`, `LOG`, `SQRT`, `SIN`, `COS`, `TANH` and `**` on `real` (f32)
values, tens of thousands of times per timestep, inside iteration loops (`EtFluxModule` runs up
to `NITERC = 20` surface-temperature iterations). A 1-ULP difference in `exp` compounds.

gfortran lowers these to glibc's `expf`, `logf`, `powf` and friends. Rust's `f32::exp` also
calls `expf` on glibc targets -- but that is an implementation detail, not a guarantee, and
`**` with an integer exponent is not a libm call at all: gfortran expands it inline, and the
expansion order decides the last bit.

So the real question is not "does Rust match gfortran" but **"which spelling of each operation
matches gfortran"**. That is what this harness answers.

## Running it

Needs `gfortran`, `rustc` and `python3`.

```sh
./run.sh              # all intrinsics, 20k inputs each
COUNT=200000 ./run.sh # slower, wider
```

Exit status 0 means every intrinsic has at least one bit-identical Rust candidate, and the
output names it. Non-zero names the intrinsics that have none.

Then record the winning candidate per intrinsic in `noahowp/src/fortran/intrinsics.rs` and have
the physics port call through it rather than reaching for `f32::exp` directly.

## Pieces

| File | Role |
|---|---|
| `probe.f90` | reference side: applies one intrinsic, prints input and output bit patterns |
| `probe.rs` | applies **every candidate** spelling of that intrinsic |
| `gen_inputs.py` | f32 bit patterns over each intrinsic's realistic argument range |
| `compare.py` | per-candidate match rate and worst-case ULP gap |
| `run.sh` | builds both probes, runs every intrinsic, prints the verdict |

Comparison is on raw bit patterns (`to_bits`), never printed decimals -- a decimal round-trip
would hide exactly the differences being hunted. Inputs are exchanged as hex bit patterns so
both probes see byte-identical arguments, with no parsing in the loop.

Build flags are deliberate: `-O2 -ffp-contract=off`, no `-ffast-math`. With contraction on, the
comparison would measure the optimiser rather than the libm.

## Candidates probed

| Intrinsic | Candidates |
|---|---|
| `exp` `log` `sqrt` `sin` `cos` `tanh` | `std` (`f32::exp`), `libm` (the C symbol via FFI), `f64` (widen, compute, narrow) |
| `pow2` `pow3` | `mul` (repeated multiply), `powi`, `powf`, `libm` |
| `pow7` | `mul` (left to right), `binary` (x⁴·x²·x), `powi`, `powf` |
| `powr` (`x ** 0.6666667`) | `std`, `libm`, `f64`, `exp_log` (`exp(y·log x)`) |

## Findings so far, without gfortran

The harness was validated by using one Rust candidate as a synthetic reference. That tells us
nothing about gfortran, but it does measure how far the candidates sit from **each other** --
and two results already matter:

**1. Widening to f64 is not a safe substitute.** Computing in double and narrowing differs from
direct f32 libm by 1 ULP on ~0.1-1% of inputs for `exp`, `log`, `sin`, `cos` and `powf`, and by
up to 2 ULP on ~6% of inputs for `tanh`. A port that reaches for `(x as f64).exp() as f32`
"for accuracy" will not be bit-identical. `sqrt` is the exception -- IEEE-exact either way.

**2. Integer exponents are the real hazard.** For `x ** 7` the four candidates disagree with
each other by up to **4 ULP**, and `powi` disagrees with repeated multiplication on 59% of
inputs. For `x ** 3`, `powf` differs from `powi`/`mul` on 25% of inputs. Whichever spelling
gfortran uses, the other three are wrong, and the error is well above the noise floor. This is
the case most likely to force the `strict-libm` fallback in
`docs/RUST_REWRITE_PLAN.md` section 6.4.

Note the circularity: for `pow2`/`pow3`/`pow7` there is no `libm` candidate to serve as
reference, so `mul` was used, which makes `mul`'s own "EXACT" result meaningless. The *spread*
between candidates is the real observation. Only a gfortran run resolves which is right.

## Interpreting the result

- **All EXACT** -> strict bit-identity is achievable. Wire the winning candidates into
  `fortran/intrinsics.rs` and treat any later use of a bare `f32::exp` in ported physics as a
  bug (worth a CI grep).
- **Some intrinsics have no match** -> do not abandon the port. Take the plan's section 6.4
  route: pin the divergence to a named function list, quantify the drift over a full Bondville
  year, and decide whether "agrees to N ULP over 17,520 timesteps" is good enough for
  calibration. It very likely is; what matters is that the number is measured rather than
  assumed.
