#!/usr/bin/env python3
"""Insert differential-test dumps around the physics calls in a copy of RunModule.f90.

`solve_noahowp` calls exactly five `*Main` subroutines. Each gets its state written immediately
before and immediately after, so a ported Rust function can be handed the pre-state and checked
against the post-state.

This edits a *copy* of the upstream source in the build directory -- `reference/build.sh` copies
it there first, and the upstream checkout is never touched.

Each call site must be found exactly once. Upstream reordering or renaming a call then stops the
build instead of silently producing fixtures with a tag that means something else.

Usage:  instrument.py <build_src_dir>
"""
import re
import sys
from pathlib import Path

# Tag values are part of the fixture format; `reference/README.md` lists them, and the Rust
# reader maps them back to names. Append, never renumber.
CALLS = [
    (1, "UtilitiesMain"),
    (2, "ForcingMain"),
    (3, "InterceptionMain"),
    (4, "EnergyMain"),
    (5, "WaterMain"),
]

DUMP = "call dt_record({tag}, {phase}, domain%itime, domain, forcing, energy, water, parameters)"


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <build_src_dir>")
    run = Path(sys.argv[1]) / "RunModule.f90"
    text = run.read_text()

    if "DiffTestModule" in text:
        sys.exit("RunModule.f90 is already instrumented")

    # The module's own `use` list, so dt_record resolves inside solve_noahowp.
    text, n = re.subn(r"(?m)^(\s*)use DateTimeUtilsModule\s*$",
                      r"\1use DateTimeUtilsModule\n\1use DiffTestModule", text, count=1)
    if n != 1:
        sys.exit("could not find `use DateTimeUtilsModule` to hang `use DiffTestModule` off")

    for tag, name in CALLS:
        # `call EnergyMain (domain, levels, ...)` -- possibly spanning continuation lines.
        pattern = re.compile(rf"(?m)^([ \t]*)(call {name}\s*\([^\n]*(?:&\n[^\n]*)*\))")
        matches = pattern.findall(text)
        if len(matches) != 1:
            sys.exit(f"expected exactly one call to {name}, found {len(matches)}")

        def repl(m, tag=tag):
            ind, call = m.group(1), m.group(2)
            before = ind + DUMP.format(tag=tag, phase=0)
            after = ind + DUMP.format(tag=tag, phase=1)
            return f"{before}\n{ind}{call}\n{after}"

        text = pattern.sub(repl, text, count=1)

    run.write_text(text)
    print(f"instrumented {len(CALLS)} call sites in {run}", file=sys.stderr)


if __name__ == "__main__":
    main()
