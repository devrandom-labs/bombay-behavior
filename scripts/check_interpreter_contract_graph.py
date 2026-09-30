#!/usr/bin/env python3
"""Reject a fixture graph that tests a registry or duplicate Behavior crate."""

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/interpreter-contract/Cargo.toml"
LOCAL_CRATES = {
    "bombay-behavior": ROOT / "crates/behavior",
    "bombay-behavior-actors": ROOT / "crates/actors",
    "bombay-behavior-macros": ROOT / "crates/behavior-macros",
}


def main() -> int:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
            str(MANIFEST),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    graph = json.loads(result.stdout)
    packages = graph["packages"]
    errors = []
    for name, expected in LOCAL_CRATES.items():
        selected = [package for package in packages if package["name"] == name]
        if len(selected) != 1:
            errors.append(f"{name}: expected one resolved package, found {len(selected)}")
            continue
        package = selected[0]
        actual = Path(package["manifest_path"]).resolve().parent
        if actual != expected.resolve() or package["source"] is not None:
            errors.append(f"{name}: expected local {expected}, found {package['id']}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Interpreter fixture resolves one local version of each Behavior crate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
