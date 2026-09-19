#!/usr/bin/env bash
# Phase 0: does any Rust spelling of each intrinsic match gfortran bit for bit?
#
# Requires gfortran, rustc and python3. Run from this directory:
#     ./run.sh
#
# Exit status 0 means every intrinsic has a candidate that is bit-identical *at both Rust
# optimisation levels* -- the "byte identical" goal is achievable and the port should use the
# candidates named in the output. Non-zero means at least one intrinsic has no such candidate;
# read docs/RUST_REWRITE_PLAN.md section 6.4 before continuing.
#
# Why both optimisation levels: at -O, LLVM rewrites `x.powf(2.0)` into `x * x`, so `powf`
# looks bit-identical to gfortran when the real powf call is not. Debug builds -- which is how
# `cargo test` runs -- would then disagree with release. Requiring agreement at -O and -O0
# keeps that artifact out of the verdict.

set -uo pipefail
cd "$(dirname "$0")"

# The literal exponents 0..4 are the only ones in noah-owp-modular's src/; pown* are the same
# exponents supplied at runtime (`x ** ifrc`), which is a libcall on both sides rather than an
# inline expansion. pow7 is not used by the model and is kept only because it is where the
# candidates diverge most.
INTRINSICS=(exp log log10 sqrt sin cos tan asin acos atan tanh
            pow0 pow1 pow2 pow3 pow4 pow7 powm1 pown2 pown3 pown4 powr
            sign1 nint int mod24)
COUNT="${COUNT:-20000}"
OUT=out
mkdir -p "$OUT"

echo "== building probes =="
# -ffp-contract=off stops the compiler fusing multiply-add, so we measure the libm, not the
# optimiser. No -ffast-math, ever.
gfortran -O2 -ffp-contract=off -o "$OUT/probe_f90" probe.f90 || exit 2
rustc -O -o "$OUT/probe_rs" probe.rs || exit 2
rustc    -o "$OUT/probe_rs_O0" probe.rs || exit 2

echo "gfortran: $(gfortran --version | head -1)"
echo "rustc:    $(rustc --version)"
echo

# The candidate names from a compare.py report, one per line, sorted.
winners() { sed -n 's/.*bit-identical candidate(s): //p' "$1" | tr ',' '\n' | tr -d ' ' | sort; }

failed=()
for f in "${INTRINSICS[@]}"; do
  ./gen_inputs.py "$f" "$COUNT" > "$OUT/$f.in" || exit 2
  "$OUT/probe_f90" "$f" < "$OUT/$f.in" > "$OUT/$f.f90.txt" || exit 2

  for tag in O2 O0; do
    bin="$OUT/probe_rs"; [ "$tag" = O0 ] && bin="$OUT/probe_rs_O0"
    "$bin" "$f" < "$OUT/$f.in" > "$OUT/$f.rs.$tag.txt" || exit 2
    ./compare.py "$f" "$OUT/$f.f90.txt" "$OUT/$f.rs.$tag.txt" > "$OUT/$f.$tag.cmp"
  done

  cat "$OUT/$f.O2.cmp"
  both=$(comm -12 <(winners "$OUT/$f.O2.cmp") <(winners "$OUT/$f.O0.cmp") | paste -sd', ')
  if [ -z "$both" ]; then
    failed+=("$f")
    echo "  !! no candidate is bit-identical at BOTH -O and -O0"
    echo "  -- unoptimised report --"
    sed 's/^/     /' "$OUT/$f.O0.cmp"
  elif ! diff -q <(winners "$OUT/$f.O2.cmp") <(winners "$OUT/$f.O0.cmp") > /dev/null; then
    # A candidate that wins at one level only is an optimiser artifact, not a real match.
    echo "  !! winners differ by optimisation level -- safe at both: $both"
    echo "  -- unoptimised report --"
    sed 's/^/     /' "$OUT/$f.O0.cmp"
  fi
  echo
done

echo "==================================================================="
if [ ${#failed[@]} -eq 0 ]; then
  echo "GO: every intrinsic has a candidate that is bit-identical at -O and -O0."
  echo "Record the winning candidate per intrinsic in noahowp/src/fortran/intrinsics.rs."
  exit 0
fi
echo "NO-GO for strict bit-identity on: ${failed[*]}"
echo "Options, in the plan's section 6.4:"
echo "  - accept a documented ULP-level exception for these, quantified over a full year run"
echo "  - or gate them behind a strict-libm feature that links the exact symbols"
exit 1
