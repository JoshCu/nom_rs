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

Exit status 0 means every intrinsic has a Rust candidate that is bit-identical **at both `-O`
and `-O0`**, and the output names it. Non-zero names the intrinsics that have none.

Both optimisation levels are checked because at `-O` LLVM rewrites `x.powf(2.0)` into `x * x`,
which makes `powf` look bit-identical when the real `powf` call is not. Debug builds -- how
`cargo test` runs -- would then disagree with release. The runner flags any candidate that wins
at one level only.

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
| `pow0` `pow1` | the identity (`1.0`, `x`), `powi`, `powf` |
| `pow2` `pow3` | `mul` (repeated multiply), `powi`, `powf`, `libm` |
| `pow4` | `mul` (left to right), `sq_sq` ((x²)²), `powi`, `powf` |
| `pow7` | `mul` (left to right), `binary` (x⁴·x²·x), `powi`, `powf` |
| `powm1` (`x ** (-1)`) | `recip` (`1.0 / x`), `powi`, `powf` |
| `pown2` `pown3` `pown4` (runtime exponent) | `powi`, `powf`, `loop` (multiply n times) |
| `powr` (`x ** 0.6666667`) | `std`, `libm`, `f64`, `exp_log` (`exp(y·log x)`) |

0 through 4 are the only literal integer exponents anywhere in `noah-owp-modular/src/`; `**7`
is kept only because it is where the candidates diverge most, which makes it the sharpest test
that the winning spelling wins for the right reason. The `pown*` cases cover `x ** ifrc` and
`x ** (CVFRZ - J)`, where the exponent is a variable -- a libcall on both sides rather than an
inline expansion, so a different code path from the literal cases. Integer-power inputs include
negative bases; the rest stay inside their domain so neither probe produces a NaN.

## Verdict

**GO.** Run 2026-09-18 against GNU Fortran 15.2.0 (Ubuntu 15.2.0-16ubuntu1), rustc 1.98.1, at
20k and 200k inputs per intrinsic: every intrinsic the model uses has a bit-identical Rust
candidate at both optimisation levels. Strict bit-identity is achievable, and the plan's
section 6.4 fallback is not needed.

The winners are recorded in `noahowp/src/fortran/intrinsics.rs`, whose tests carry gfortran's
own answers as vectors -- so a future rustc that expands `llvm.powi` differently fails CI there
rather than silently in the physics.

The set probed is every intrinsic that appears in `noah-owp-modular/src/`, not a sample.

| | Winner | What loses, and by how much |
|---|---|---|
| `exp` `log` `sin` `cos` | `f32`'s own method | widening to f64: 1 ULP on 0.07% (`exp`) to 1.4% (`sin`) of inputs |
| `log10` `sqrt` `tan` `asin` `acos` `atan` `tanh` | `f32`'s own method | nothing -- all candidates agree |
| `x ** <integer>` | **`powi`, always** | `x*x*x*x` is 2 ULP out on 34% of inputs for `**4`; `powf` 1 ULP out on 26% for `**3` |
| `x ** <real>` | `powf` | `exp(y * log x)`: up to 12 ULP |
| `SIGN(a, b)` | `copysign` | `if b >= 0.0` is wrong for `b = -0.0` |
| `MOD(a, p)` | `%` | `rem_euclid` (Fortran's `MODULO`) differs on every negative argument |
| `NINT(x)` | `round() as i32` | `round_ties_even` differs on every exact `.5` |
| `INT(x)` | `as i32` | `floor` differs on every negative argument |

**`SIGN` does not follow the standard's plain reading.** Fortran 2018 says `SIGN(A, B)` is
`|A|` when `B >= 0`, and `-0.0 >= 0` is true -- but gfortran returns `-|A|` for `B = -0.0`,
following IEEE `copysign`. A linear sweep never lands exactly on zero, so the sweep now forces
`-0.0` into the input set rather than leaving the case untested.

**`NINT` and `INT` are compared as integers**, not as reals. Converting the result back to a
real to travel through the probe would compare `real(int(-0.5)) = +0.0` against Rust's
`trunc(-0.5) = -0.0`: a sign-of-zero artifact of the harness, and one that made both look
wrong until the probe was fixed.

**Integer exponents were the real hazard, and `powi` is the answer to all of it.** gfortran
expands `x ** n` by squaring, so left-to-right multiplication is simply a different computation
once `n >= 4`: `x * x * x * x` matches `x ** 4` on only 66% of inputs. `powi` matched on every
exponent probed -- 0, 1, 2, 3, 4, 7, -1, and each of those supplied at runtime. Nothing else
matched across the board, so the rule is uniform and needs no per-exponent judgement.

**`powf` for an integer exponent is the trap that hides at `-O`.** It appears bit-identical for
`**2` and `**(-1)` in an optimised build and is not; LLVM folded the call into a multiply. The
runner now checks both levels and reports any candidate that only wins at one.

**Constant folding is a trap for tests, not for the port.** Both compilers evaluate a
compile-time-constant argument in higher precision than the libm they would otherwise call, so
`exp(-29.5739784)` folds to `2A215188` where glibc's `expf` returns `2A215189`. gfortran folds
to the same value LLVM does, so ported code agrees with the Fortran in either regime -- but a
test comparing a folded Rust value against a Fortran value the probe computed at runtime will
fail. `intrinsics.rs` puts the inputs behind `black_box` for this reason.

## Interpreting the result

- **All EXACT** -> strict bit-identity is achievable. Wire the winning candidates into
  `fortran/intrinsics.rs` and treat any later use of a bare `f32::exp` in ported physics as a
  bug (worth a CI grep). This is what happened; see the verdict above.
- **Some intrinsics have no match** -> do not abandon the port. Take the plan's section 6.4
  route: pin the divergence to a named function list, quantify the drift over a full Bondville
  year, and decide whether "agrees to N ULP over 17,520 timesteps" is good enough for
  calibration. It very likely is; what matters is that the number is measured rather than
  assumed.
