#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

loop_dir=research/session-protocol-derivation-loop

set +e
python3 - "$loop_dir/evidence.json" "$loop_dir/REPORT.md" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

evidence_path, report_path = map(Path, sys.argv[1:])
expected = {
    "CASE-01", "TRACE-01", "STATIC-01", "RESEARCH-01", "FSM-01",
    "DERIVE-01", "APP-01", "OBSTRUCTION-01", "COMPILE-01", "COMPOSE-01",
    "SURFACE-01", "COUNTER-01", "DECISION-01", "DOC-01", "VERIFY-01",
}
allowed_outcomes = {
    "existing-algebra-sufficient", "application-local", "minimal-core-gap",
    "insufficient-evidence",
}
errors = []

try:
    data = json.loads(evidence_path.read_text())
except (OSError, json.JSONDecodeError) as exc:
    print(f"CHECK: invalid evidence: {exc}")
    raise SystemExit(2)

items = data.get("obligations", [])
ids = [item.get("id") for item in items if isinstance(item, dict)]
if len(ids) != len(set(ids)):
    errors.append("duplicate obligation IDs")
if missing := sorted(expected - set(ids)):
    errors.append("missing obligations: " + ", ".join(missing))
if extra := sorted(set(ids) - expected):
    errors.append("unexpected obligations: " + ", ".join(extra))

resolved = 0
for item in items:
    if not isinstance(item, dict) or item.get("id") not in expected:
        continue
    item_id = item["id"]
    if item.get("status") not in {"pending", "resolved"}:
        errors.append(f"{item_id}: invalid status")
        continue
    if item.get("status") != "resolved":
        continue
    if not isinstance(item.get("decision"), str) or len(item["decision"].strip()) < 40:
        errors.append(f"{item_id}: decision is too short")
        continue
    valid = True
    for field in ("evidence", "validation"):
        values = item.get(field)
        if not isinstance(values, list) or not values or any(not isinstance(v, str) or len(v.strip()) < 20 for v in values):
            errors.append(f"{item_id}: needs concrete {field}")
            valid = False
    if valid:
        resolved += 1

outcome = data.get("outcome")
if outcome not in allowed_outcomes:
    errors.append("outcome must select exactly one allowed value")

report = report_path.read_text() if report_path.exists() else ""
if "## Final decision" not in report:
    errors.append("report lacks ## Final decision")
if outcome in allowed_outcomes and f"Outcome: `{outcome}`" not in report:
    errors.append("report lacks the exact selected outcome")
if missing := sorted(item_id for item_id in expected if item_id not in report):
    errors.append("report omits obligation IDs: " + ", ".join(missing))

if outcome in allowed_outcomes and outcome != "minimal-core-gap":
    prepared = data.get("prepared_at_commit")
    if not isinstance(prepared, str):
        errors.append("non-gap outcome needs prepared_at_commit")
    elif subprocess.run(
        ["git", "diff", "--quiet", prepared, "--", "crates/behavior/src"],
        check=False,
    ).returncode:
        errors.append("non-gap outcome must revert all production algebra experiments")

prepared = data.get("prepared_at_commit")
if isinstance(prepared, str):
    protected = [
        "AGENTS.md", "Cargo.toml", "Cargo.lock", "flake.nix", "flake.lock",
        ".github", "CHANGELOG.md",
    ]
    if subprocess.run(
        ["git", "diff", "--quiet", prepared, "--", *protected],
        check=False,
    ).returncode:
        errors.append("off-limits manifests, CI, instructions, release files, or changelog changed")

print(f"resolved obligations: {resolved}/{len(expected)}")
print(f"outcome: {outcome}")
for error in errors:
    print("CHECK: " + error)
if errors or resolved != len(expected):
    pending = sorted(item.get("id") for item in items if isinstance(item, dict) and item.get("status") != "resolved")
    if pending:
        print("UNRESOLVED: " + ", ".join(pending))
    raise SystemExit(2)
PY
check_status=$?
set -e

if [ "$check_status" -ne 0 ]; then
    exit "$check_status"
fi

if rg -n '\bdyn\s+[A-Za-z_]|\bAny\b|\bTypeId\b|\bunsafe\b' crates/behavior/src; then
    echo "CHECK: dynamic/static escape found in production algebra"
    exit 2
fi

cargo nextest run --workspace
nix flake check
echo "CHECK: session protocol derivation evidence and repository gates pass"
