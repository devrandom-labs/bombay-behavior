"""Check the compiler error named by each annotated Rustdoc failure."""

from pathlib import Path
from tempfile import TemporaryDirectory
import json
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
ANNOTATION = re.compile(r"^```compile_fail,(E\d{4})\b")


def examples():
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        current = None
        for number, line in enumerate(path.read_text().splitlines(), 1):
            stripped = line.lstrip()
            if not stripped.startswith(("///", "//!")):
                continue
            content = stripped[3:].removeprefix(" ")
            if content.startswith("```"):
                if current is not None:
                    start, expected, body = current
                    yield path, start, expected, body
                    current = None
                elif match := ANNOTATION.match(content):
                    current = number, match.group(1), []
            elif current is not None:
                current[2].append(content.removeprefix("# "))


def libraries():
    command = [
        "cargo", "build", "--locked", "--lib", "--message-format=json",
        "-p", "bombay-behavior", "-p", "bombay-behavior-actors",
    ]
    built = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if built.returncode:
        raise RuntimeError(f"cargo build failed:\n{built.stderr}")
    artifacts = {}
    for line in built.stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get("reason") != "compiler-artifact":
            continue
        name = item.get("target", {}).get("name")
        if name in ("behavior", "behavior_actors"):
            artifact = next(
                (Path(file) for file in item["filenames"] if file.endswith(".rlib")),
                None,
            )
            if artifact is not None:
                artifacts[name] = artifact
    if set(artifacts) != {"behavior", "behavior_actors"}:
        raise RuntimeError("cargo did not report both behavior library artifacts")
    return artifacts


def main() -> int:
    artifacts = libraries()
    dependency_dir = artifacts["behavior"].parent
    if dependency_dir.name != "deps":
        dependency_dir /= "deps"
    failures = []
    checked = 0
    with TemporaryDirectory(prefix="bombay-rustdoc-errors-") as temporary:
        probe = Path(temporary) / "probe.rs"
        for path, line, expected, body in examples():
            checked += 1
            probe.write_text(
                "#![allow(dead_code, unused_variables)]\nfn main() {\n"
                + "\n".join(body)
                + "\n}\n"
            )
            command = [
                "rustc", "--edition=2024", "--crate-name=probe", "--emit=metadata",
                "--error-format=json", str(probe), "--out-dir", temporary,
                "-L", f"dependency={dependency_dir}",
                "--extern", f"behavior={artifacts['behavior']}",
                "--extern", f"behavior_actors={artifacts['behavior_actors']}",
            ]
            compiled = subprocess.run(command, capture_output=True, text=True)
            codes = []
            for diagnostic in compiled.stderr.splitlines():
                try:
                    item = json.loads(diagnostic)
                except json.JSONDecodeError:
                    continue
                if item.get("level") == "error" and item.get("code"):
                    codes.append(item["code"]["code"])
            if compiled.returncode == 0 or expected not in codes:
                failures.append(
                    f"{path.relative_to(ROOT)}:{line}: expected {expected}, got {codes}"
                )
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"Checked {checked} Rustdoc failure diagnostics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
