#!/usr/bin/env python3
"""Generate f32 bit patterns for the intrinsic probe, one 8-hex-digit value per line.

Ranges are chosen to cover the arguments Noah-OWP actually passes, and to stay inside each
intrinsic's domain so neither probe produces a NaN (a NaN comparison tells us nothing -- the
payload bits are not specified).

Usage:  gen_inputs.py <intrinsic> [count]
"""
import math
import struct
import sys

# (lo, hi, spacing) per intrinsic. "log" spacing samples decades evenly.
RANGES = {
    # Arguments to exp() in the physics: Clausius-Clapeyron, stability functions, soil
    # hydraulics. Kept well inside the f32 overflow bound (~88).
    "exp": (-30.0, 30.0, "linear"),
    "log": (1e-8, 1e8, "log"),
    "sqrt": (0.0, 1e6, "linear"),
    # Solar angles and stability corrections, in radians.
    "sin": (-10.0, 10.0, "linear"),
    "cos": (-10.0, 10.0, "linear"),
    "tanh": (-10.0, 10.0, "linear"),
    # Bases for integer powers: fractions through to temperatures. 0 through 4 are the only
    # literal exponents that occur in the model; pown* are the same exponents supplied at
    # runtime, which is a different code path on both sides.
    "pow0": (1e-3, 1e3, "log"),
    "pow1": (1e-3, 1e3, "log"),
    "pow2": (1e-3, 1e3, "log"),
    "pow3": (1e-3, 1e3, "log"),
    "pow4": (1e-3, 1e3, "log"),
    "powm1": (1e-3, 1e3, "log"),
    "pown2": (1e-3, 1e3, "log"),
    "pown3": (1e-3, 1e3, "log"),
    "pown4": (1e-3, 1e3, "log"),
    "pow7": (0.1, 10.0, "log"),
    # Base for a real exponent, e.g. soil-moisture retention curves.
    "powr": (1e-6, 1e6, "log"),
}


# Integer powers are the only case where a negative base is legal, and the model does pass
# them (temperature and flux differences). An odd exponent keeps the sign, so a candidate that
# gets sign handling wrong would go unnoticed on a positive-only sweep.
SIGNED = {"pow0", "pow1", "pow2", "pow3", "pow4", "pow7", "powm1", "pown2", "pown3", "pown4"}


def values(lo, hi, spacing, n):
    if spacing == "log":
        lo_l, hi_l = math.log10(lo), math.log10(hi)
        return [10.0 ** (lo_l + (hi_l - lo_l) * i / (n - 1)) for i in range(n)]
    return [lo + (hi - lo) * i / (n - 1) for i in range(n)]


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in RANGES:
        sys.exit(f"usage: {sys.argv[0]} <{'|'.join(RANGES)}> [count]")

    name = sys.argv[1]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 20000
    lo, hi, spacing = RANGES[name]

    signed = name in SIGNED
    magnitudes = values(lo, hi, spacing, n // 2 if signed else n)

    seen = set()
    for m in magnitudes:
        for v in ((m, -m) if signed else (m,)):
            # Round-trip through f32 so the bit pattern is exactly what both probes read.
            bits = struct.unpack("<I", struct.pack("<f", v))[0]
            if bits in seen:
                continue
            seen.add(bits)
            print(f"{bits:08X}")


if __name__ == "__main__":
    main()
