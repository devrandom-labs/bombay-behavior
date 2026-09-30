"""Reject required transitions and ownership transfers inside test assertions."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ASSERTION = re.compile(r"\b(?:prop_|debug_)?assert(?:_eq|_ne)?!\s*\(")
CONSUMING_METHOD = re.compile(
    r"\.(?:into_[A-Za-z_]+|into_iter|take|pop|next|insert|remove|"
    r"transition|initialize|receive|on|accept|reject|offer_next_to_source|"
    r"interpret|settle)\s*\("
)
MUTATING_HELPER = re.compile(
    r"\b(?:query|put|acquire|release|hold|offer|deliver|dispatch|"
    r"search_capability)\s*\([^;]*?&mut\b",
    re.DOTALL,
)


def assertion_bodies(source: str):
    """Yield assertion locations and bodies, preserving nested calls."""
    for found in ASSERTION.finditer(source):
        position = found.end()
        depth = 1
        quoted = False
        escaped = False
        while depth and position < len(source):
            character = source[position]
            if quoted:
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == '"':
                    quoted = False
            elif character == '"':
                quoted = True
            elif character == "'" and position + 2 < len(source) and source[position + 2] == "'":
                position += 3
                continue
            elif (
                character == "'"
                and source[position + 1 : position + 2] == "\\"
                and source[position + 3 : position + 4] == "'"
            ):
                position += 4
                continue
            elif character == "(":
                depth += 1
            elif character == ")":
                depth -= 1
            position += 1
        if depth == 0:
            yield source.count("\n", 0, found.start()) + 1, source[found.end() : position - 1]


def violations(source: str):
    for line, body in assertion_bodies(source):
        if CONSUMING_METHOD.search(body) or MUTATING_HELPER.search(body):
            yield line


def main() -> int:
    failures = []
    for path in Path("crates").rglob("*.rs"):
        if "target" in path.parts:
            continue
        for line in violations(path.read_text()):
            failures.append(f"{path}:{line}: move the required call before the assertion")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
