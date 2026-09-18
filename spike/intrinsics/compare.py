#!/usr/bin/env python3
"""Compare the Fortran and Rust intrinsic probes, per candidate implementation.

Reads the Fortran output (input_hex output_hex) and the Rust output
(input_hex candidate output_hex) and reports, for each candidate, how often it matches the
Fortran bit for bit and the worst ULP gap when it does not.

Usage:  compare.py <intrinsic> <fortran_out> <rust_out>
Exit status is 0 if at least one candidate matches everywhere, 1 otherwise -- so a runner
script can treat "no candidate matches" as the failure it is.
"""
import struct
import sys
from collections import defaultdict


def to_ordered(bits):
    """Map an f32 bit pattern to a monotonic integer, so subtraction counts ULPs."""
    if bits & 0x8000_0000:
        return 0x8000_0000 - (bits & 0x7FFF_FFFF)
    return bits | 0x8000_0000


def ulp_gap(a, b):
    return abs(to_ordered(a) - to_ordered(b))


def as_float(bits):
    return struct.unpack("<f", struct.pack("<I", bits))[0]


def main():
    if len(sys.argv) != 4:
        sys.exit(f"usage: {sys.argv[0]} <intrinsic> <fortran_out> <rust_out>")
    name, f_path, r_path = sys.argv[1:4]

    reference = {}
    with open(f_path) as fh:
        for line in fh:
            parts = line.split()
            if len(parts) == 2:
                reference[int(parts[0], 16)] = int(parts[1], 16)

    if not reference:
        sys.exit(f"{f_path}: no reference values -- did the Fortran probe run?")

    # candidate -> [total, matches, worst_ulp, worst_example]
    stats = defaultdict(lambda: [0, 0, 0, None])
    with open(r_path) as fh:
        for line in fh:
            parts = line.split()
            if len(parts) != 3:
                continue
            xb, label, yb = int(parts[0], 16), parts[1], int(parts[2], 16)
            want = reference.get(xb)
            if want is None:
                continue
            s = stats[label]
            s[0] += 1
            if yb == want:
                s[1] += 1
            else:
                gap = ulp_gap(yb, want)
                if gap > s[2]:
                    s[2] = gap
                    s[3] = (xb, want, yb)

    print(f"=== {name} ({len(reference)} inputs) ===")
    exact = []
    for label in sorted(stats):
        total, matches, worst, example = stats[label]
        pct = 100.0 * matches / total if total else 0.0
        verdict = "EXACT" if matches == total else f"worst {worst} ULP"
        print(f"  {label:<8} {matches}/{total} ({pct:6.2f}%)  {verdict}")
        if example and matches != total:
            xb, want, got = example
            print(
                f"           worst at x={as_float(xb):.9g} ({xb:08X}): "
                f"fortran {want:08X} vs rust {got:08X}"
            )
        if matches == total:
            exact.append(label)

    if exact:
        print(f"  -> bit-identical candidate(s): {', '.join(exact)}")
        return 0
    print("  -> NO candidate is bit-identical")
    return 1


if __name__ == "__main__":
    sys.exit(main())
