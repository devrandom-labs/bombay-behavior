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
from collections import Counter
from pathlib import Path

evidence_path, matrix_path, baseline_path, target_path = map(Path, sys.argv[1:])

expected_obligations = {
    "RESEARCH-AGHA","RESEARCH-CREATION","RESEARCH-LABELS","SURVEY-SEARCH",
    "RESEARCH-BIBLIOGRAPHY","RESEARCH-FORMALISMS",
    "SURVEY-TAXONOMY","SURVEY-BASIS","SURVEY-GAPS","SURVEY-ACTORPASS",
    "CALCULUS-NUCLEUS","CALCULUS-SOUNDNESS","CALCULUS-MINIMALITY","CALCULUS-CLOSURE",
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
allowed_classifications = {"actor-model-law", "bombay-derived", "bombay-policy", "interpreter-boundary", "application-policy"}
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
disposition_counts = Counter()
for item in capabilities:
    if not isinstance(item, dict): continue
    item_id = item.get("id")
    if item.get("status") not in {"pending", "resolved"}:
        errors.append(f"capability {item_id}: invalid status")
        continue
    disposition = item.get("disposition")
    if disposition not in allowed_dispositions:
        errors.append(f"capability {item_id}: invalid disposition")
        continue
    disposition_counts[disposition] += 1
    if item.get("status") != "resolved": continue
    sources = item.get("sources")
    laws = item.get("laws")
    validation = item.get("validation")
    composition = item.get("composition")
    classifications = item.get("claim_classification")
    limitations = item.get("limitations")
    valid = True
    if not isinstance(sources, list) or not sources or any(not isinstance(v, str) or len(v.strip()) < 16 for v in sources):
        errors.append(f"capability {item_id}: needs primary-source evidence"); valid = False
    if not isinstance(composition, str) or len(composition.strip()) < 20:
        errors.append(f"capability {item_id}: needs a derivation or explicit boundary explanation"); valid = False
    if not isinstance(laws, list) or not laws or any(not isinstance(v, str) or len(v.strip()) < 12 for v in laws):
        errors.append(f"capability {item_id}: needs applicable laws/boundary laws"); valid = False
    if not isinstance(validation, list) or not validation or any(not isinstance(v, str) or len(v.strip()) < 12 for v in validation):
        errors.append(f"capability {item_id}: needs validation"); valid = False
    if not isinstance(classifications, list) or not classifications or any(v not in allowed_classifications for v in classifications):
        errors.append(f"capability {item_id}: needs explicit claim_classification values"); valid = False
    if not isinstance(limitations, list) or not limitations or any(not isinstance(v, str) or len(v.strip()) < 20 for v in limitations):
        errors.append(f"capability {item_id}: needs explicit limitations"); valid = False
    if valid: resolved_capabilities += 1

count_order = ("existing", "derived", "interpreter", "application", "new-primitive", "rejected")
count_summary = ", ".join(f"{name}={disposition_counts[name]}" for name in count_order)
print("capability dispositions: " + count_summary)

source_paths = sorted(Path("crates/behavior/src").rglob("*.rs"))
source_paths += sorted(Path("crates/actors/src").rglob("*.rs"))
source_by_path = {str(path): path.read_text() for path in source_paths}
all_source = "\n".join(source_by_path.values())
test_paths = [
    *Path("crates/behavior/tests").rglob("*.rs"),
    *Path("crates/actors/tests").rglob("*.rs"),
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
    uses_baseline = item.get("coverage_source") == "baseline.json"
    if item.get("status") == "resolved" and not uses_baseline and set(item.get("coverage", [])) != frozen:
        errors.append(f"{obligation}: coverage must list every frozen item exactly")

declarations = {
    "Behavior": ("trait","Behavior"), "Actions": ("struct","Actions"),
    "Create": ("struct","Create"), "Compose": ("struct","Compose"),
    "Initialized": ("struct","Initialized"), "Active": ("struct","Active"),
    "Deadline": ("struct","Deadline"), "Watch": ("struct","Watch"),
    "Supervisor": ("struct","Supervisor"), "Proxy": ("struct","Proxy"),
    "ReceiveTimeout": ("struct","ReceiveTimeout"), "Stash": ("struct","Stash"),
    "Machine": ("struct","Machine"),
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
uses_baseline = complexity_item.get("coverage_source") == "baseline.json"
if complexity_item.get("status") == "resolved" and not uses_baseline and set(complexity_item.get("coverage", [])) != frozen_complexity:
    errors.append("COMPLEXITY-01: coverage must list every frozen file exactly")

baseline_commit = baseline.get("captured_at_commit")
protected = baseline.get("protected_paths")
reviewed_changes = set(baseline.get("reviewed_changed_paths", []))
if not isinstance(baseline_commit, str) or not isinstance(protected, list):
    errors.append("baseline must define captured_at_commit and protected_paths")
elif subprocess.run(["git","diff","--quiet",baseline_commit,"--",*(path for path in protected if path not in reviewed_changes)], check=False).returncode:
    errors.append("off-limits manifests, CI, instructions, release files, or changelogs changed")

target_text = target_path.read_text() if target_path.exists() else ""
if "## Actor behavior algebra evidence" not in target_text:
    errors.append("research report lacks final actor behavior algebra evidence section")
for heading in (
    "## Comprehensive actor research method",
    "## Agha bibliography and disposition",
    "## Foundational semantics comparison",
    "## Post-2000 actor algebra and formalism comparison",
    "## Research-to-primitive claim map",
    "## Candidate primitive basis",
    "## Primitive soundness",
    "## Primitive eliminability",
    "## Capability derivation trees",
):
    if heading not in target_text:
        errors.append("research report lacks required calculus section: " + heading)
if resolved_obligations == len(expected_obligations):
    if missing := sorted(item_id for item_id in expected_obligations if item_id not in target_text):
        errors.append("research report omits obligation IDs: " + ", ".join(missing))
if resolved_capabilities == len(capabilities):
    report_count_line = "Disposition totals: " + count_summary + "."
    if report_count_line not in target_text:
        errors.append("research report lacks the checker-derived disposition totals: " + report_count_line)

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

basis=research/architecture-critical-review-loop/primitive-basis.json
bibliography=research/architecture-critical-review-loop/research-bibliography.json
derivations=research/architecture-critical-review-loop/capability-derivations.json

set +e
python3 - "$basis" "$bibliography" "$derivations" "$matrix" <<'PY'
import json
import sys
from pathlib import Path

basis_path, biblio_path, deriv_path, matrix_path = map(Path, sys.argv[1:])
errors = []

def load(path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        errors.append(f"invalid {path}: {exc}")
        return {}

basis = load(basis_path)
biblio = load(biblio_path)
derivations = load(deriv_path)
matrix = load(matrix_path)

prims = basis.get("primitives", [])
prim_ids = [p.get("id") for p in prims if isinstance(p, dict)]
if len(prim_ids) != len(set(prim_ids)):
    errors.append("duplicate primitive IDs")
retained = {p["id"] for p in prims if isinstance(p, dict) and p.get("public_api_status") in {"retain", "reclassify"}}
representation_primitives = {p["id"] for p in prims if isinstance(p, dict) and p.get("representation_status") == "primitive"}
all_prim = set(prim_ids)

for p in prims:
    if not isinstance(p, dict):
        continue
    pid = p.get("id", "?")
    for field in ("rust_type", "formation_rule", "operational_semantics"):
        if not isinstance(p.get(field), str) or len(p[field].strip()) < 20:
            errors.append(f"primitive {pid}: missing {field}")

    if p.get("classification") not in {"actor-model-law", "bombay-derived", "bombay-policy"}:
        errors.append(f"primitive {pid}: invalid classification")
    if p.get("layer") not in {"actor-nucleus", "host-calculus", "bombay-derived", "bombay-policy"}:
        errors.append(f"primitive {pid}: invalid layer")
    # Schema-v2 independent status fields
    sem = p.get("semantic_status")
    rep = p.get("representation_status")
    pub = p.get("public_api_status")
    if sem not in {"primitive", "derived", "extension", "not-applicable"}:
        errors.append(f"primitive {pid}: invalid semantic_status '{sem}'")
    if rep not in {"required-encoding", "preferred-encoding", "derived-combinator", "policy", "not-applicable"}:
        errors.append(f"primitive {pid}: invalid representation_status '{rep}'")
    if pub not in {"retain", "reclassify", "redesign", "remove", "not-public"}:
        errors.append(f"primitive {pid}: invalid public_api_status '{pub}'")
    # Actor-nucleus: role validation + semantic must be primitive
    if p.get("layer") == "actor-nucleus":
        role = p.get("role", "")
        if p["id"] == "N-fold" and role != "transition-form":
            errors.append(f"primitive {pid}: N-fold must have role=transition-form")
        if p["id"] in {"N-send", "N-create", "N-become"} and role != "effect-primitive":
            errors.append(f"primitive {pid}: must have role=effect-primitive")
        if sem != "primitive":
            errors.append(f"primitive {pid}: actor-nucleus layer requires semantic_status=primitive")
    # Host-calculus: semantic must be not-applicable
    if p.get("layer") == "host-calculus" and sem != "not-applicable":
        errors.append(f"primitive {pid}: host-calculus layer requires semantic_status=not-applicable")
    # Bombay-derived/policy: semantic must be derived or extension
    if p.get("layer") in {"bombay-derived", "bombay-policy"} and sem not in {"derived", "extension"}:
        errors.append(f"primitive {pid}: {p['layer']} layer requires semantic_status=derived or extension")

    if not isinstance(p.get("laws"), list) or not p["laws"]:
        errors.append(f"primitive {pid}: needs algebraic laws")
    snd = p.get("soundness", {})
    for field in ("obligations_covered", "evidence", "validation"):
        if not isinstance(snd.get(field), list) or not snd[field]:
            errors.append(f"primitive {pid}: soundness needs {field}")
    elim = p.get("eliminability", {})
    if not isinstance(elim.get("attempted_signature"), str) or len(elim["attempted_signature"].strip()) < 10:
        errors.append(f"primitive {pid}: eliminability needs the exact attempted type signature")
    attempts = elim.get("attempts")
    if not isinstance(attempts, list) or not attempts:
        errors.append(f"primitive {pid}: needs at least one eliminability attempt")
    else:
        for a in attempts:
            if a.get("obstruction_kind") not in {"type", "compiler", "semantic", "law"}:
                errors.append(f"primitive {pid}: attempt with invalid obstruction_kind")
            if not isinstance(a.get("obstruction"), str) or len(a["obstruction"].strip()) < 12:
                errors.append(f"primitive {pid}: attempt needs exact obstruction evidence")
    verdict = elim.get("verdict")
    if verdict not in {"primitive", "derived"}:
        errors.append(f"primitive {pid}: invalid eliminability verdict")

    # (representation-derivation check handled above at line 386)

    # Derived-combinator/preferred-encoding/policy: must reference retained primitives
    if rep in {"derived-combinator", "preferred-encoding", "policy"}:
        rep_refs = set(elim.get("derived_from", []))
        if not rep_refs or not rep_refs <= retained:
            errors.append(f"primitive {pid}: {rep} must reference retained primitives in derivable_from")

    if verdict == "derived":
        refs = set(elim.get("derived_from", []))
        if not refs or not refs <= retained:
            errors.append(f"primitive {pid}: eliminability says derived but derivation references are missing or invalid")
    # Schema-v2: derived representation must reference retained representation primitives
    if rep == "derived":
        rep_refs = set(elim.get("derived_from", []))
        if not rep_refs or not rep_refs <= retained:
            errors.append(f"primitive {pid}: representation-derived but derivation references missing or reference non-retained primitives")
    # Mechanical probe: if claimed EXECUTED, path must reference a real file
    mp = elim.get("mechanical_probe", "")
    if "EXECUTED" in mp:
        import os
        # Extract probe directory name from the mechanical_probe string
        probe_dir = None
        for part in mp.split():
            if "probes/" in part:
                probe_dir = part.rstrip("/")
                break
        if probe_dir and not os.path.isdir(probe_dir):
            errors.append(f"primitive {pid}: mechanical_probe claims EXECUTED but probe dir '{probe_dir}' not found")
    # Limitations must be present (non-empty for bombay-derived/host-calculus layers)
    lim = p.get("limitations")
    if not isinstance(lim, list):
        errors.append(f"primitive {pid}: missing limitations field")
    elif p.get("layer") != "actor-nucleus" and not lim:
        errors.append(f"primitive {pid}: non-nucleus primitive needs explicit limitations")

caps = derivations.get("capabilities", [])
cap_ids = [c.get("id") for c in caps if isinstance(c, dict)]
matrix_ids = {c.get("id") for c in matrix.get("capabilities", []) if isinstance(c, dict)}
if set(cap_ids) != matrix_ids:
    errors.append("derivation rows must exactly cover the capability matrix")
for c in caps:
    if not isinstance(c, dict):
        continue
    cid = c.get("id", "?")
    if c.get("kind") == "pure":
        d = c.get("derivation")
        if not isinstance(d, dict):
            errors.append(f"capability {cid}: pure row needs a derivation tree")
            continue
        refs = set(d.get("primitive_refs", []))
        if not refs:
            errors.append(f"capability {cid}: derivation tree must reference primitives")
        elif unknown := sorted(refs - all_prim):
            errors.append(f"capability {cid}: unknown primitive reference: " + ", ".join(unknown))
        elif not refs <= retained:
            errors.append(f"capability {cid}: references a demoted (non-retained) primitive")
        # Validate derived_combinator_refs
        combo_refs = set(d.get("derived_combinator_refs", []))
        for cr in combo_refs:
            if cr not in all_prim:
                errors.append(f"capability {cid}: unknown combinator ref '{cr}'")
            else:
                cp = next((pp for pp in prims if pp.get("id") == cr), None)
                if cp:
                    cs = cp.get("semantic_status", "")
                    if cs not in {"derived", "extension"}:
                        errors.append(f"capability {cid}: combinator ref '{cr}' semantic_status={cs}, expected derived/extension")
                    cp_pub = cp.get("public_api_status", "")
                    if cp_pub not in {"retain", "reclassify"}:
                        errors.append(f"capability {cid}: combinator ref '{cr}' public_api_status={cp_pub}, expected retain/reclassify")
                    c_elim = cp.get("eliminability", {})
                    c_derived = set(c_elim.get("derived_from", []))
                    if c_derived and not c_derived <= retained:
                        errors.append(f"capability {cid}: combinator ref '{cr}' derivation does not bottom out in retained primitives")
        for field in ("event_sums", "effect_products", "initialization", "composition_order", "errors", "termination", "freshness"):
            if not isinstance(d.get(field), str) or not d[field].strip():
                errors.append(f"capability {cid}: derivation needs {field}")
        if not isinstance(d.get("static_limitations"), list) or not d["static_limitations"]:
            errors.append(f"capability {cid}: needs explicit static limitations")
    elif c.get("kind") == "boundary":
        arg = c.get("boundary_argument")
        if not isinstance(arg, str) or len(arg.strip()) < 20:
            errors.append(f"capability {cid}: boundary row needs an explicit boundary argument")
    else:
        errors.append(f"capability {cid}: invalid kind")

sources = biblio.get("sources", [])
src_ids = [x.get("id") for x in sources if isinstance(x, dict)]
if len(src_ids) != len(set(src_ids)):
    errors.append("duplicate bibliography source IDs")
src_by_id = {x["id"]: x for x in sources if isinstance(x, dict)}
required_core = {
    "hewitt-bishop-steiger-1973", "greif-1975", "baker-hewitt-1977", "hewitt-1977",
    "clinger-1981", "agha-1986", "agha-cacm-1990", "agha-rex-1990", "amst-1997",
    "talcott-1997", "mason-talcott-1997", "agha-thati-ziaei-2001",
    "thati-ziaei-agha-fmoods-2002", "thati-ziaei-agha-amast-2002", "thati-ms-2001",
    "thati-phd-2003", "agha-thati-2004", "kumar-sen-meseguer-agha-2003",
    "agha-meseguer-sen-2006", "agha-dos-2005", "karmani-agha-2011",
    "charalambides-dinges-agha-2012", "charalambides-palmskog-agha-2019",
    "paul-agha-2021", "plyukhin-agha-2020", "plyukhin-agha-2018",
    "plyukhin-agha-montesi-2025", "agha-kim-1999", "varela-agha-2001",
    "agha-callsen-1993", "kim-agha-1995", "de-koster-de-meuter-2025",
    "rebeca-2004", "honda-1993", "honda-vasconcelos-kubo-1998",
    "honda-yoshida-carbone-2016",
}
if missing := sorted(required_core - set(src_ids)):
    errors.append("bibliography lacks required actor-relevant records: " + ", ".join(missing))
for x in sources:
    if not isinstance(x, dict):
        continue
    sid = x.get("id", "?")
    for field in ("authors", "title", "venue", "year"):
        if not x.get(field):
            errors.append(f"source {sid}: missing {field}")
    if x.get("read_status") not in {"unread", "abstract-only", "partial", "complete"}:
        errors.append(f"source {sid}: invalid read_status")
    if x.get("inclusion") not in {"included-semantic", "included-capability", "included-framework-comparison", "excluded"}:
        errors.append(f"source {sid}: invalid inclusion")
    claims = x.get("semantic_claims", [])
    supports = x.get("supports_primitive_claims", [])
    if x.get("read_status") in {"unread", "abstract-only"} and claims:
        errors.append(f"source {sid}: semantic claims rest on an unread or abstract-only source")
    if supports and x.get("read_status") not in {"partial", "complete"}:
        errors.append(f"source {sid}: supports primitive claims while only {x.get('read_status')}")
    if unknown := sorted(set(supports) - all_prim):
        errors.append(f"source {sid}: unknown primitive reference: " + ", ".join(unknown))
    if x.get("inclusion") == "excluded" and not x.get("exclusion_reason"):
        errors.append(f"source {sid}: excluded without a specific reason")

if errors:
    for error in errors:
        print("CHECK: " + error)
    raise SystemExit(2)
print(f"artifact gate: {len(prims)} primitives, {len(caps)} derivations, {len(sources)} bibliography sources validated")
PY
artifact_status=$?
set -e

if [ "$artifact_status" -ne 0 ]; then exit "$artifact_status"; fi

unexpected_bombay_dependencies=$(rg --pcre2 -n \
  'package = "bombay-(?!behavior(?:-actors|-macros|-testkit)?")|name = "bombay-(?!behavior(?:-actors|-macros|-testkit|-fuzz)?")' \
  --glob 'Cargo.toml' --glob 'Cargo.lock' . || true)
if [ -n "$unexpected_bombay_dependencies" ]; then
  echo "CHECK: behavior workspace must not depend on other Bombay crates"
  echo "$unexpected_bombay_dependencies"
  exit 2
fi

cargo nextest run --workspace
nix flake check
echo "CHECK: behavior architecture closure and authoritative repository gates pass"
