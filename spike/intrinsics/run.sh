#!/usr/bin/env bash
# Phase 0: does any Rust spelling of each intrinsic match gfortran bit for bit?
#
# Requires gfortran, rustc and python3. Run from this directory:
#     ./run.sh
#
# Exit status 0 means every intrinsic has at least one bit-identical candidate -- the "byte
# identical" goal is achievable and the port should use the candidates named in the output.
# Non-zero means at least one intrinsic has no exact match; read docs/RUST_REWRITE_PLAN.md
# section 6.4 before continuing.

set -uo pipefail
cd "$(dirname "$0")"

INTRINSICS=(exp log sqrt sin cos tanh pow2 pow3 pow7 powr)
COUNT="${COUNT:-20000}"
OUT=out
mkdir -p "$OUT"

echo "== building probes =="
# -ffp-contract=off stops the compiler fusing multiply-add, so we measure the libm, not the
# optimiser. No -ffast-math, ever.
gfortran -O2 -ffp-contract=off -o "$OUT/probe_f90" probe.f90 || exit 2
rustc -O -o "$OUT/probe_rs" probe.rs || exit 2

echo "gfortran: $(gfortran --version | head -1)"
echo "rustc:    $(rustc --version)"
echo

failed=()
for f in "${INTRINSICS[@]}"; do
  ./gen_inputs.py "$f" "$COUNT" > "$OUT/$f.in" || exit 2
  "$OUT/probe_f90" "$f" < "$OUT/$f.in" > "$OUT/$f.f90.txt" || exit 2
  "$OUT/probe_rs"  "$f" < "$OUT/$f.in" > "$OUT/$f.rs.txt"  || exit 2
  ./compare.py "$f" "$OUT/$f.f90.txt" "$OUT/$f.rs.txt" || failed+=("$f")
  echo
done

echo "==================================================================="
if [ ${#failed[@]} -eq 0 ]; then
  echo "GO: every intrinsic has a bit-identical Rust candidate."
  echo "Record the winning candidate per intrinsic in noahowp/src/fortran/intrinsics.rs."
  exit 0
fi
echo "NO-GO for strict bit-identity on: ${failed[*]}"
echo "Options, in the plan's section 6.4:"
echo "  - accept a documented ULP-level exception for these, quantified over a full year run"
echo "  - or gate them behind a strict-libm feature that links the exact symbols"
exit 1
