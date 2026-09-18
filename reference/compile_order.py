#!/usr/bin/env python3
"""Print a valid compile order for a directory of Fortran sources, from the `use` graph.

The upstream `src/Makefile` lists objects in link order, which is not a compile order -- it has
DomainType before DateTimeUtilsModule, and DomainType uses it. Deriving the order instead of
hard-coding one also means a new upstream module needs no change here.
"""
import glob
import os
import re
import sys


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    provides, uses = {}, {}
    for path in glob.glob(os.path.join(root, "*.f90")):
        base = os.path.basename(path)[:-4]
        text = open(path, errors="replace").read()
        for m in re.findall(r"(?mi)^\s*module\s+([A-Za-z]\w*)\s*$", text):
            provides[m.lower()] = base
        uses[base] = {u.lower() for u in re.findall(r"(?mi)^\s*use\s+([A-Za-z]\w*)", text)}

    deps = {b: {provides[u] for u in us if u in provides and provides[u] != b}
            for b, us in uses.items()}

    order, done = [], set()

    def visit(node, stack=()):
        if node in done:
            return
        if node in stack:
            sys.exit(f"circular module dependency: {' -> '.join(stack + (node,))}")
        for dep in sorted(deps.get(node, ())):
            visit(dep, stack + (node,))
        done.add(node)
        order.append(node)

    for node in sorted(deps):
        visit(node)
    print(" ".join(order))


if __name__ == "__main__":
    main()
