#!/usr/bin/env bash
# Build the reference Fortran and generate differential-test fixtures.
#
#     ./build.sh              # build only
#     ./build.sh --fixtures   # build, then run Bondville and write the fixtures
#
# Environment:
#   NOM_SRC     upstream noah-owp-modular checkout (searched for if unset)
#   NOM_COMMIT  commit to build (default: the pin in docs/porting.md)
#   FC          Fortran compiler (default: gfortran)
#   NSAMPLES    timesteps to sample per physics call (default: 200)
#
# The upstream checkout is only ever read: sources are extracted from the pinned commit with
# `git archive`, so a dirty working tree there cannot leak into the reference build, and this
# script never writes to it.

set -euo pipefail
cd "$(dirname "$0")"
HERE=$PWD

# Somewhere to find the upstream checkout without making the caller say so every time.
if [ -z "${NOM_SRC:-}" ]; then
  for c in "$PWD/../../noah-owp-modular" "$PWD/../../../NOAA-OWP/noah-owp-modular" \
           "$HOME/code/noah-owp-modular"; do
    [ -d "$c/.git" ] && NOM_SRC=$(cd "$c" && pwd) && break
  done
fi
NOM_SRC=${NOM_SRC:-}
NOM_COMMIT=${NOM_COMMIT:-0242a96c2a90b2c1501ff56a1e457a932a49c8ac}
FC=${FC:-gfortran}
NSAMPLES=${NSAMPLES:-200}
BUILD=$PWD/build

# -ffp-contract=off keeps the compiler from fusing multiply-add, which would change results
# the Rust port has to reproduce. Never -ffast-math.
#
# NGEN_OUTPUT_ACTIVE compiles out OutputModule -- the only NetCDF user in the codebase -- while
# NGEN_FORCING_ACTIVE is deliberately left undefined so the ASCII reader stays in and the run
# can be driven from data/bondville.dat.
FFLAGS="-O2 -ffp-contract=off -cpp -DNGEN_OUTPUT_ACTIVE -ffree-form -ffree-line-length-none"

if [ -z "$NOM_SRC" ] || [ ! -d "$NOM_SRC/.git" ]; then
  echo "set NOM_SRC to a git checkout of NOAA-OWP/noah-owp-modular (got: '${NOM_SRC:-unset}')" >&2
  exit 2
fi
echo "upstream: $NOM_SRC"

echo "== extracting $NOM_COMMIT =="
rm -rf "$BUILD"
mkdir -p "$BUILD/src"
git -C "$NOM_SRC" archive "$NOM_COMMIT" src driver \
  | tar -x -C "$BUILD" --strip-components=0
mv "$BUILD"/driver/AsciiReadModule.f90 "$BUILD"/driver/OutputModule.f90 "$BUILD/src/"
rm -rf "$BUILD/driver"

echo "== generating serializer =="
./gen_serializer.py "$BUILD/src" "$BUILD/src/DiffTestModule.f90" ../noahowp/src/difftest/manifest.rs

echo "== instrumenting RunModule =="
./instrument.py "$BUILD/src"
cp difftest_driver.f90 paramsweep_driver.f90 datesweep_driver.f90 atmsweep_driver.f90 interceptsweep_driver.f90 \
   watersweep_driver.f90 "$BUILD/src/"

echo "== compiling ($($FC --version | head -1)) =="
cd "$BUILD/src"
# Compile order comes from the `use` graph rather than the upstream Makefile's OBJS list, which
# is a link order and not a valid compile order (DomainType uses DateTimeUtilsModule, which
# appears after it).
ORDER=$(python3 "$HERE/compile_order.py" .)
for f in $ORDER; do
  $FC $FFLAGS -c "$f.f90" -o "$f.o" 2>&1 | grep -vE "Legacy Extension|GOTO statement|^\s*[0-9]+ \||^\s*\||^\s*$|Warning:" || true
done
# Each driver is a `program`, so they cannot be linked into the same binary.
LIB_OBJS=$(for f in $ORDER; do case $f in *_driver) ;; *) echo "$f.o" ;; esac; done)
# shellcheck disable=SC2086
$FC -o "$BUILD/difftest" difftest_driver.o $LIB_OBJS
# shellcheck disable=SC2086
$FC -o "$BUILD/paramsweep" paramsweep_driver.o $LIB_OBJS
# shellcheck disable=SC2086
$FC -o "$BUILD/datesweep" datesweep_driver.o $LIB_OBJS
# shellcheck disable=SC2086
$FC -o "$BUILD/atmsweep" atmsweep_driver.o $LIB_OBJS
# shellcheck disable=SC2086
$FC -o "$BUILD/interceptsweep" interceptsweep_driver.o $LIB_OBJS
# shellcheck disable=SC2086
$FC -o "$BUILD/watersweep" watersweep_driver.o $LIB_OBJS
echo "built $BUILD/{difftest,paramsweep,datesweep,atmsweep,interceptsweep,watersweep}"

[ "${1:-}" = "--fixtures" ] || exit 0

echo "== running Bondville =="
cd "$BUILD"
mkdir -p run
# The shipped namelist uses paths relative to the upstream run/ directory.
git -C "$NOM_SRC" show "$NOM_COMMIT:run/namelist.input" \
  | sed -e "s|\"../data/|\"$NOM_SRC/data/|" -e "s|\"../parameters/\"|\"$NOM_SRC/parameters/\"|" \
  > run/namelist.input
FIX=$HERE/../noahowp/tests/fixtures/difftest
mkdir -p "$FIX"
./difftest run/namelist.input "$FIX/bondville.difftest" "$NSAMPLES"
./paramsweep run/namelist.input "$FIX/param_sweep.difftest"
./datesweep "$FIX/advance_sweep.bin" "$FIX/declin_sweep.bin"
./atmsweep run/namelist.input "$FIX/atm_sweep.difftest"
./interceptsweep run/namelist.input "$FIX/intercept_sweep.difftest"
./watersweep run/namelist.input "$FIX/water_sweep.difftest"
ls -lh "$FIX"/*.difftest "$FIX"/*.bin
