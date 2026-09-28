"""Keep Rustdoc examples explicit about their external dependencies."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
IMPORT = re.compile(r"(?:#\s*)?use\b")


def main() -> int:
    violations = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        in_example = False
        for number, line in enumerate(path.read_text().splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith(("///", "//!")):
                example_line = stripped[3:].strip()
                if example_line.startswith("```"):
                    in_example = not in_example
                elif in_example and IMPORT.match(example_line):
                    violations.append(f"{path.relative_to(ROOT)}:{number}")
    if violations:
        print("Rustdoc examples must use fully qualified names:", file=sys.stderr)
        print("\n".join(violations), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
