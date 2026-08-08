#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

evidence=research/architecture-critical-review-loop/evidence.json
matrix=research/architecture-critical-review-loop/capability-matrix.json
baseline=research/architecture-critical-review-loop/baseline.json
target=research/architecture-critical-review-loop/REPORT.md

set +e
python3 - "$evidence" "$matrix" "$baseline" "$target" <<'PY'
import json
import re
import subprocess
import sys
from pathlib import Path

evidence_path, matrix_path, baseline_path, target_path = map(Path, sys.argv[1:])

expected_obligations = {
    "RESEARCH-AGHA","RESEARCH-CREATION","RESEARCH-LABELS","SURVEY-SEARCH",
    "SURVEY-TAXONOMY","SURVEY-BASIS","SURVEY-GAPS","SURVEY-ACTORPASS",
    "CORE-BEHAVIOR","CORE-ACTIONS","CORE-INITIALIZATION","CORE-ERRORS","CORE-SENDS",
    "MODULE-CORE","MODULE-PROTOCOL","MODULE-TRANSFORMS",
    "COMPOSE-AT","COMPOSE-RECEIVE-TIMEOUT","COMPOSE-WATCHING","COMPOSE-SUPERVISING",
    "COMPOSE-SHUTDOWN","COMPOSE-STASHING","COMPOSE-FSM","COMPOSE-SPEC","COMPOSE-MIXED",
    "CREATE-FRESHNESS","CREATE-NONCE","CREATE-ORDER","CREATE-PROVENANCE","CREATE-FAILURE",
    "RETAIN-SEND-PRODUCT","RETAIN-EVENT-SUMS","RETAIN-FUNCTIONS","RETAIN-SPEC",
    "RETAIN-BIRTH-MODE","RETAIN-PROXY","BOUNDARY-FRESHNESS","BOUNDARY-ERROR",
    "BOUNDARY-LIFECYCLE","BOUNDARY-IDENTITY","REJECT-ERASURE","REJECT-ENVELOPE",
    "REJECT-REGISTRY","REJECT-SERIALIZATION","REJECT-REPLACEMENT-LANE",
    "REJECT-INFERENCE","REJECT-ALLOCATOR","REJECT-SPECULATION","SURFACE-PUBLIC",
    "SURFACE-TRAITS","GEN-CORE","GEN-WRAPPERS","PHANTOM-01","COMPLEXITY-01",
    "PANIC-01","STATIC-01","ERGO-01","TEST-MODELS","TEST-COMPILE-FAIL","DOC-01","VERIFY-01",
}
required_capabilities = {
    "core-transition","become","termination","typed-send","send-products","creation",
    "child-topology","behavior-delegation","forwarding","protocol-sum","protocol-product",
    "initialization","controlled-error","deadline","receive-timeout","timer-generation",
    "selective-receive","stashing","finite-state","shutdown","finalization",
    "peer-observation","child-observation","worker-reporting","linking",
    "supervision-strategy","restart-policy","restart-budget","replacement-provenance",
    "routing","request-reply","correlation","acknowledgement","retry","deduplication",
    "backpressure","mailbox-priority","fairness","failure-detection","persistence",
    "event-sourcing","durable-state","distribution","location-transparency","remoting",
    "serialization","security-capability","protocol-session","workflow-saga","streaming",
    "scheduling","resource-ownership","lifecycle-publication",
}
allowed_dispositions = {"existing", "derived", "new-primitive", "interpreter", "application", "rejected"}
errors = []

def load(path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        errors.append(f"invalid {path}: {exc}")
        return {}

evidence_data = load(evidence_path)
matrix_data = load(matrix_path)
baseline = load(baseline_path)

if matrix_data.get("derivation_discipline") != "functional-combinators-first":
    errors.append("capability matrix must preserve the functional-combinators-first discipline")

items = evidence_data.get("obligations", [])
ids = [item.get("id") for item in items if isinstance(item, dict)]
actual_ids = set(ids)
if len(ids) != len(actual_ids): errors.append("duplicate evidence obligation IDs")
if missing := sorted(expected_obligations - actual_ids): errors.append("missing obligations: " + ", ".join(missing))
if extra := sorted(actual_ids - expected_obligations): errors.append("unexpected obligations: " + ", ".join(extra))

resolved_obligations = 0
by_id = {}
for item in items:
    if not isinstance(item, dict) or item.get("id") not in expected_obligations: continue
    item_id = item["id"]
    by_id[item_id] = item
    if item.get("status") not in {"pending", "resolved"}:
        errors.append(f"{item_id}: invalid status")
        continue
    if item.get("status") != "resolved": continue
    if not isinstance(item.get("decision"), str) or len(item["decision"].strip()) < 20:
        errors.append(f"{item_id}: decision is too short")
        continue
    for field in ("evidence", "validation"):
        values = item.get(field)
        if not isinstance(values, list) or not values or any(not isinstance(v, str) or len(v.strip()) < 12 for v in values):
            errors.append(f"{item_id}: needs concrete {field}")
            break
    else:
        resolved_obligations += 1

capabilities = matrix_data.get("capabilities", [])
capability_ids = [item.get("id") for item in capabilities if isinstance(item, dict)]
actual_capabilities = set(capability_ids)
if len(capability_ids) != len(actual_capabilities): errors.append("duplicate capability IDs")
if missing := sorted(required_capabilities - actual_capabilities): errors.append("missing required capabilities: " + ", ".join(missing))
declared_required = set(matrix_data.get("required_categories", []))
if declared_required != required_capabilities:
    errors.append("required_categories must exactly match the checker-owned taxonomy")

resolved_capabilities = 0
for item in capabilities:
    if not isinstance(item, dict): continue
    item_id = item.get("id")
    if item.get("status") not in {"pending", "resolved"}:
        errors.append(f"capability {item_id}: invalid status")
        continue
    if item.get("status") != "resolved": continue
    disposition = item.get("disposition")
    if disposition not in allowed_dispositions:
        errors.append(f"capability {item_id}: invalid disposition")
        continue
    sources = item.get("sources")
    laws = item.get("laws")
    validation = item.get("validation")
    composition = item.get("composition")
    valid = True
    if not isinstance(sources, list) or not sources or any(not isinstance(v, str) or len(v.strip()) < 16 for v in sources):
        errors.append(f"capability {item_id}: needs primary-source evidence"); valid = False
    if not isinstance(composition, str) or len(composition.strip()) < 20:
        errors.append(f"capability {item_id}: needs a derivation or explicit boundary explanation"); valid = False
    if not isinstance(laws, list) or not laws or any(not isinstance(v, str) or len(v.strip()) < 12 for v in laws):
        errors.append(f"capability {item_id}: needs applicable laws/boundary laws"); valid = False
    if not isinstance(validation, list) or not validation or any(not isinstance(v, str) or len(v.strip()) < 12 for v in validation):
        errors.append(f"capability {item_id}: needs validation"); valid = False
    if valid: resolved_capabilities += 1

source_paths = sorted(Path("crates/behavior/src").glob("*.rs"))
source_by_path = {str(path): path.read_text() for path in source_paths}
all_source = "\n".join(source_by_path.values())
test_paths = [
    *Path("crates/behavior/tests").rglob("*.rs"),
    *Path("crates/behavior-testkit/src").rglob("*.rs"),
    *Path("crates/behavior-testkit/tests").rglob("*.rs"),
]
test_source = "\n".join(path.read_text() for path in sorted(test_paths))

current_turbofish = test_source.count("::<")
current_aliases = sum(1 for line in test_source.splitlines() if line.startswith("type ") and "=" in line)
current_panics = len(re.findall(r"panic!|\.expect\(", all_source))
for label, current, key in (
    ("test turbofish", current_turbofish, "test_turbofish_expressions"),
    ("test helper aliases", current_aliases, "test_helper_type_aliases"),
    ("production panic/expect", current_panics, "production_panic_expect_sites"),
):
    limit = baseline.get(key)
    if not isinstance(limit, int): errors.append(f"missing baseline counter: {key}")
    elif current > limit: errors.append(f"{label} regression: {current} > {limit}")
    print(f"ratchet: {label}={current}/{limit}")

dynamic_patterns = {
    "dyn": r"\bdyn\s+[A-Za-z_]", "Any": r"\bAny\b", "TypeId": r"\bTypeId\b",
    "unsafe": r"\bunsafe\b",
}
for label, pattern in dynamic_patterns.items():
    count = len(re.findall(pattern, all_source))
    print(f"zero-tolerance: {label}={count}")
    if count: errors.append(f"dynamic/static escape introduced: {label} ({count})")

public_symbols = set(re.findall(r"^pub\s+(?:struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)", all_source, re.M))
public_traits = set(re.findall(r"^pub\s+trait\s+([A-Za-z_][A-Za-z0-9_]*)", all_source, re.M))
for obligation, key, current in (
    ("SURFACE-PUBLIC", "public_symbols", public_symbols),
    ("SURFACE-TRAITS", "public_traits", public_traits),
):
    frozen = set(baseline.get(key, []))
    if new := sorted(current - frozen): errors.append(f"{obligation}: new unreviewed items: " + ", ".join(new))
    print(f"inventory: {obligation}={len(current)}/{len(frozen)}")
    item = by_id.get(obligation, {})
    if item.get("status") == "resolved" and set(item.get("coverage", [])) != frozen:
        errors.append(f"{obligation}: coverage must list every frozen item exactly")

declarations = {
    "Behavior": ("trait","Behavior"), "State": ("trait","State"),
    "Actions": ("struct","Actions"), "Create": ("struct","Create"),
    "SendProduct": ("struct","SendProduct"), "Spec": ("struct","Spec"),
    "At": ("struct","At"), "Watching": ("struct","Watching"),
    "Supervising": ("struct","Supervising"), "Proxy": ("struct","Proxy"),
    "ReceiveTimeout": ("struct","ReceiveTimeout"), "Stashing": ("struct","Stashing"),
    "Fsm": ("struct","Fsm"), "Base": ("struct","Base"), "FnState": ("struct","FnState"),
}
def arity(kind, name):
    declaration = re.search(rf"\b{kind}\s+{name}\b", all_source)
    if not declaration:
        errors.append(f"missing measured declaration: {name}"); return 10**6
    cursor = declaration.end()
    while cursor < len(all_source) and all_source[cursor].isspace(): cursor += 1
    if cursor >= len(all_source) or all_source[cursor] != "<": return 0
    depth = 0; start = cursor + 1; parts = []
    for cursor in range(cursor, len(all_source)):
        char = all_source[cursor]
        if char == "<": depth += 1
        elif char == ">":
            depth -= 1
            if depth == 0:
                parts.append(all_source[start:cursor])
                return len([part for part in parts if part.strip()])
        elif char == "," and depth == 1:
            parts.append(all_source[start:cursor]); start = cursor + 1
    errors.append(f"missing measured declaration: {name}"); return 10**6
for name, declaration in declarations.items():
    current = arity(*declaration); limit = baseline.get("generic_arities", {}).get(name)
    print(f"generic arity: {name}={current}/{limit}")
    if not isinstance(limit, int): errors.append(f"missing generic baseline: {name}")
    elif current > limit: errors.append(f"generic arity regression for {name}: {current} > {limit}")

phantom_count = len(re.findall(
    r"^\s*[A-Za-z_][A-Za-z0-9_]*\s*:\s*(?:core::marker::)?PhantomData<"
    r"|^pub struct Births<[^>]+>\(PhantomData<",
    all_source,
    re.M,
))
phantom_limit = baseline.get("phantom_field_count")
print(f"inventory: PHANTOM-01={phantom_count}/{phantom_limit}")
if not isinstance(phantom_limit, int) or phantom_count > phantom_limit:
    errors.append(f"phantom-field regression: {phantom_count} > {phantom_limit}")

complexity_files = {path for path, text in source_by_path.items() if "clippy::type_complexity" in text}
frozen_complexity = set(baseline.get("type_complexity_files", []))
if new := sorted(complexity_files - frozen_complexity): errors.append("new type-complexity files: " + ", ".join(new))
print(f"inventory: COMPLEXITY-01={len(complexity_files)}/{len(frozen_complexity)}")
complexity_item = by_id.get("COMPLEXITY-01", {})
if complexity_item.get("status") == "resolved" and set(complexity_item.get("coverage", [])) != frozen_complexity:
    errors.append("COMPLEXITY-01: coverage must list every frozen file exactly")

baseline_commit = baseline.get("captured_at_commit")
protected = baseline.get("protected_paths")
if not isinstance(baseline_commit, str) or not isinstance(protected, list):
    errors.append("baseline must define captured_at_commit and protected_paths")
elif subprocess.run(["git","diff","--quiet",baseline_commit,"--",*protected], check=False).returncode:
    errors.append("off-limits manifests, CI, instructions, release files, or changelogs changed")

target_text = target_path.read_text() if target_path.exists() else ""
if "## Actor behavior algebra evidence" not in target_text:
    errors.append("research report lacks final actor behavior algebra evidence section")
if resolved_obligations == len(expected_obligations):
    if missing := sorted(item_id for item_id in expected_obligations if item_id not in target_text):
        errors.append("research report omits obligation IDs: " + ", ".join(missing))

score = resolved_obligations * 10 + resolved_capabilities * 5
print(f"SCORE: {score}")
print(f"resolved obligations: {resolved_obligations}/{len(expected_obligations)}")
print(f"resolved capabilities: {resolved_capabilities}/{len(capabilities)} ({len(required_capabilities)} required)")
for error in errors: print("CHECK: " + error)
if errors or resolved_obligations != len(expected_obligations) or resolved_capabilities != len(capabilities):
    pending = sorted(item.get("id") for item in items if isinstance(item, dict) and item.get("status") != "resolved")
    if pending: print("UNRESOLVED OBLIGATIONS: " + ", ".join(pending))
    pending_caps = sorted(item.get("id") for item in capabilities if isinstance(item, dict) and item.get("status") != "resolved")
    if pending_caps: print("UNRESOLVED CAPABILITIES: " + ", ".join(pending_caps))
    raise SystemExit(2)
PY
check_status=$?
set -e

if [ "$check_status" -ne 0 ]; then exit "$check_status"; fi

cargo nextest run --workspace
nix flake check
echo "CHECK: behavior architecture closure and authoritative repository gates pass"
