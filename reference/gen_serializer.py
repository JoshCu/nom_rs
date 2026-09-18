#!/usr/bin/env python3
"""Generate the differential-test serializer from the Fortran derived-type definitions.

The physics port is verified by replaying the Fortran's own state through the Rust: for each
call to one of the five `*Main` subroutines, the fixture holds the complete state before and
after, and the Rust function has to reproduce the post-state bit for bit from the pre-state.
That only works if both sides agree on what "the state" is, down to field order.

Rather than hand-writing ~500 field writes and keeping them in step with upstream, this parses
`src/*Type.f90` and emits both sides of that agreement:

  DiffTestModule.f90   the Fortran writer
  manifest.rs          the same field list as Rust tables, so the reader needs no JSON crate
                       and the model crate keeps its zero dependencies

Re-run it whenever the pinned upstream commit moves; a field added to a derived type then shows
up on both sides at once, and a stale fixture fails the magic/length checks rather than
silently shifting every field after the insertion point.

Usage:  gen_serializer.py <upstream_src_dir> <out_fortran> <out_rust>
"""
import json
import re
import sys
from pathlib import Path

# The state the differential test needs. `domain` carries the clock, so it is per-record even
# though most of it is constant; the rest of the run configuration is written once.
PER_RECORD = ["domain", "forcing", "energy", "water"]
STATIC = ["levels", "options", "parameters"]
TYPES = PER_RECORD + STATIC

# `domain%sim_datetimes` is one f64 per timestep of the whole run -- 17,522 for Bondville, or
# 140 KB in every record. It is derived from (start, end, dt), which are themselves in the
# record, so dumping it would only inflate the fixtures.
EXCLUDE = {("domain", "sim_datetimes")}

# The attribute list runs up to the `::` separator and may itself contain colons, as in
# `real, allocatable, dimension(:) :: zsoil` -- so it is "anything that is not the separator"
# rather than "anything that is not a colon".
DECL = re.compile(
    r"""^\s*
    (?P<base>real\*8|real|double\s+precision|integer|logical|character\(len=\d+\))
    (?P<attrs>(?:(?!::)[^!])*)
    ::\s*(?P<name>\w+)\s*
    (?P<dims>\([^)]*\))?
    """,
    re.IGNORECASE | re.VERBOSE,
)


def kind_of(base):
    base = base.lower().replace(" ", "")
    if base == "real":
        return "r4"
    if base in ("real*8", "doubleprecision"):
        return "r8"
    if base == "integer":
        return "i4"
    if base == "logical":
        return "l4"
    m = re.match(r"character\(len=(\d+)\)", base)
    if m:
        return f"c{m.group(1)}"
    raise SystemExit(f"unhandled declaration type: {base!r}")


def parse_type(path, type_name):
    """Fields of `type, public :: <type_name>`, in declaration order."""
    text = path.read_text(errors="replace")
    start = re.search(rf"(?mi)^\s*type,\s*public\s*::\s*{type_name}\s*$", text)
    if not start:
        raise SystemExit(f"{path}: no `type, public :: {type_name}`")

    fields = []
    for line in text[start.end():].splitlines():
        stripped = line.strip().lower()
        # The type's bound procedures follow `contains`; `end type` closes it.
        if stripped.startswith("contains") or stripped.startswith("end type"):
            break
        if not line.strip() or line.strip().startswith("!"):
            continue
        m = DECL.match(line)
        if not m:
            # Every component declaration carries `::`. Refusing to skip one silently is the
            # point: a regex that quietly drops a field shifts every field after it in the
            # fixture, and the mismatch would surface as unrelated physics being "wrong".
            if "::" in line.split("!")[0]:
                raise SystemExit(f"{path}: cannot parse declaration: {line.strip()!r}")
            continue
        attrs, dims = m.group("attrs") or "", m.group("dims") or ""
        alloc = "allocatable" in attrs.lower()
        # `dimension(n)` in the attributes, or `name(n)` after the name.
        dim = re.search(r"dimension\s*\(([^)]*)\)", attrs, re.IGNORECASE)
        extent = (dim.group(1) if dim else dims.strip("()")).strip()
        if not extent:
            shape = "scalar"
        elif extent == ":":
            shape = "alloc"
        elif extent.isdigit():
            shape = int(extent)
        else:
            raise SystemExit(f"{path}: unhandled extent {extent!r} on {m.group('name')}")
        fields.append({"name": m.group("name"), "kind": kind_of(m.group("base")), "shape": shape})
    if not fields:
        raise SystemExit(f"{path}: parsed no fields from {type_name}")
    return fields


# ---------------------------------------------------------------------------- Fortran writer

# Each `write` is a separate statement so a field that fails to write points at its own line.
# Allocatables carry their length so a fixture built for a different nsoil/nsnow is detected on
# read rather than silently reinterpreted.
WRITE = {
    "scalar": "    write(DT_UNIT) {expr}",
    "alloc": "    call dt_put_len(size({expr})); write(DT_UNIT) {expr}",
}


def fortran_write(var, f):
    expr = f"{var}%{f['name']}"
    kind = f["kind"]
    if kind == "l4":
        # Fortran logical has no portable byte pattern; normalise to 0/1.
        expr = f"merge(1, 0, {expr})"
    if f["shape"] == "scalar":
        return f"    write(DT_UNIT) {expr}"
    if f["shape"] == "alloc":
        if kind == "l4":
            raise SystemExit(f"allocatable logical not handled: {f['name']}")
        return f"    call dt_put_len(size({var}%{f['name']})); write(DT_UNIT) {expr}"
    return f"    write(DT_UNIT) {expr}"  # fixed extent: length is in the manifest


MAGIC = 0x4E4F4D44  # 'NOMD'


def emit_fortran(schema):
    use = "\n".join(
        f"  use {t.capitalize()}Type" if t != "levels" else "  use LevelsType" for t in TYPES
    )
    dumps, calls = [], []
    for t in TYPES:
        body = "\n".join(fortran_write(t, f) for f in schema[t])
        dumps.append(
            f"  subroutine dt_dump_{t}({t})\n"
            f"    type({t}_type), intent(in) :: {t}\n"
            f"{body}\n"
            f"  end subroutine dt_dump_{t}\n"
        )
    per = "\n".join(f"    call dt_dump_{t}({t})" for t in PER_RECORD)
    per_args = ", ".join(PER_RECORD)
    per_decl = "\n".join(f"    type({t}_type), intent(in) :: {t}" for t in PER_RECORD)
    static_args = ", ".join(STATIC)
    static_decl = "\n".join(f"    type({t}_type), intent(in) :: {t}" for t in STATIC)
    static = "\n".join(f"    call dt_dump_{t}({t})" for t in STATIC)

    magic = MAGIC
    return f"""! GENERATED by reference/gen_serializer.py -- do not edit.
!
! Writes the model state as raw little-endian bytes to a stream file, in the field order given
! by reference/manifest.rs. See reference/README.md for the record layout.

module DiffTestModule

{use}

  implicit none
  private
  public :: dt_open, dt_close, dt_arm, dt_record, dt_static

  integer, parameter :: DT_UNIT = 71
  integer, parameter :: DT_MAGIC = {magic}   ! 'NOMD'
  logical :: dt_open_ = .false.    ! fixture file is open
  logical :: dt_enabled = .false.  ! this timestep is being sampled

contains

  subroutine dt_open(path)
    character(len=*), intent(in) :: path
    open(DT_UNIT, file=path, access='stream', form='unformatted', status='replace')
    dt_open_ = .true.
  end subroutine dt_open

  subroutine dt_close()
    if (dt_open_) close(DT_UNIT)
    dt_open_ = .false.
    dt_enabled = .false.
  end subroutine dt_close

  ! Sampling is the driver's decision, not this module's: a full Bondville year is 17,522
  ! timesteps and dumping every one of them would be gigabytes.
  subroutine dt_arm(on)
    logical, intent(in) :: on
    dt_enabled = on
  end subroutine dt_arm

  subroutine dt_put_len(n)
    integer, intent(in) :: n
    write(DT_UNIT) n
  end subroutine dt_put_len

  ! tag: which *Main subroutine. phase: 0 before the call, 1 after.
  subroutine dt_record(tag, phase, itime, {per_args})
    integer, intent(in) :: tag, phase, itime
{per_decl}
    if (.not. (dt_open_ .and. dt_enabled)) return
    write(DT_UNIT) DT_MAGIC
    write(DT_UNIT) tag
    write(DT_UNIT) phase
    write(DT_UNIT) itime
{per}
  end subroutine dt_record

  ! The run configuration, written once after initialisation.
  subroutine dt_static({static_args})
{static_decl}
    if (.not. dt_open_) return
    write(DT_UNIT) DT_MAGIC
{static}
  end subroutine dt_static

{"".join(dumps)}
end module DiffTestModule
"""


# ------------------------------------------------------------------------------- Rust tables

def emit_rust(schema):
    magic = MAGIC

    def rust_fields(fields):
        out = []
        for f in fields:
            shape = {
                "scalar": "Shape::Scalar",
                "alloc": "Shape::Alloc",
            }.get(f["shape"], None) or f"Shape::Fixed({f['shape']})"
            kind = f["kind"]
            rk = {"r4": "Kind::R4", "r8": "Kind::R8", "i4": "Kind::I4", "l4": "Kind::L4"}.get(kind)
            if rk is None:
                rk = f"Kind::Char({kind[1:]})"
            out.append(f'    Field {{ name: "{f["name"]}", kind: {rk}, shape: {shape} }},')
        return "\n".join(out)

    tables = "\n\n".join(
        f"/// Fields of `{t}_type`, in the order `DiffTestModule` writes them.\n"
        f"pub const {t.upper()}: &[Field] = &[\n{rust_fields(schema[t])}\n];"
        for t in TYPES
    )
    per = ", ".join(f'("{t}", {t.upper()})' for t in PER_RECORD)
    static = ", ".join(f'("{t}", {t.upper()})' for t in STATIC)
    return f"""//! GENERATED by reference/gen_serializer.py -- do not edit.
//!
//! The field layout of the differential-test fixtures, mirroring the Fortran derived types.
//! Generated rather than hand-written so that a field added upstream lands on both sides at
//! once; see `reference/README.md`.

/// Element type of a field, as the Fortran declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {{
    /// `real` -- 4 bytes.
    R4,
    /// `real*8` / `double precision` -- 8 bytes.
    R8,
    /// `integer` -- 4 bytes.
    I4,
    /// `logical`, written as an `integer` 0 or 1.
    L4,
    /// `character(len=N)` -- N bytes, space padded.
    Char(usize),
}}

impl Kind {{
    /// Bytes per element.
    pub const fn width(self) -> usize {{
        match self {{
            Kind::R4 | Kind::I4 | Kind::L4 => 4,
            Kind::R8 => 8,
            Kind::Char(n) => n,
        }}
    }}
}}

/// How many elements a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {{
    /// One element.
    Scalar,
    /// `allocatable` -- the fixture carries an `i4` length immediately before the data.
    Alloc,
    /// A fixed extent declared in the Fortran, e.g. `dimension(12)`.
    Fixed(usize),
}}

/// One component of a derived type.
#[derive(Debug, Clone, Copy)]
pub struct Field {{
    /// The Fortran component name, in its original case.
    pub name: &'static str,
    pub kind: Kind,
    pub shape: Shape,
}}

/// `'NOMD'`, at the head of every record.
pub const MAGIC: u32 = {magic:#010x};

/// The types written in each per-call record, in order.
pub const PER_RECORD: &[(&str, &[Field])] = &[{per}];

/// The types written once, after initialisation.
pub const STATIC: &[(&str, &[Field])] = &[{static}];

{tables}
"""


def main():
    if len(sys.argv) != 4:
        sys.exit(f"usage: {sys.argv[0]} <upstream_src_dir> <out_fortran> <out_rust>")
    src, out_f90, out_rs = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])

    schema = {}
    for t in TYPES:
        fields = parse_type(src / f"{t.capitalize()}Type.f90", f"{t}_type")
        schema[t] = [f for f in fields if (t, f["name"]) not in EXCLUDE]

    out_f90.write_text(emit_fortran(schema))
    out_rs.write_text(emit_rust(schema))
    counts = ", ".join(f"{t} {len(schema[t])}" for t in TYPES)
    print(f"fields: {counts}", file=sys.stderr)


if __name__ == "__main__":
    main()
