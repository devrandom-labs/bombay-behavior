#!/usr/bin/env bash
# Canonical benchmark entrypoint for the behaviorpass PERF loop (the Supervising
# children mechanism).
#
# Workload: .auto/measure.sh builds + execs the frozen ruler
# (crates/behaviorpass/examples/perf_supervising.rs), which measures the SPACE
# footprint of Supervising's liveness table through the public API.
#
# Primary metric:   score = 1e6 / (1 + space_bytes)   (MAXIMIZE — smaller table)
# Secondary (info): space_bytes / alloc_bytes / struct_size / step throughput
#
# A compile/run failure makes measure.sh emit METRIC score=0, which passes
# through unchanged so the loop auto-reverts.
#
# Correctness is NOT measured here; .auto/checks.sh is the hard gate (frozen
# surfaces + the whole suite green, which pins the generic Behavior contract).
#
# Run UNSANDBOXED (cargo hangs under a sandboxed shell).
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		if [ -x "${d}/cargo" ]; then
			PATH="${d}:${PATH}"
			break
		fi
	done
fi
if ! command -v cargo >/dev/null 2>&1; then
	echo "harness error: cargo not found" >&2
	exit 1
fi
export PATH

out=$(bash .auto/measure.sh 2>&1)

score=$(printf '%s\n' "$out" | grep -oE '^METRIC score=[0-9.]+' | head -n1 | cut -d= -f2)
if [ -z "${score}" ]; then
	echo "harness error: no score metric in measure.sh output" >&2
	printf '%s\n' "$out" >&2
	exit 1
fi

loc=$(printf '%s\n' "$out" | grep -oE 'code_loc=[0-9]+' | head -n1 | cut -d= -f2)

echo "METRIC score=${score}"
[ -n "${loc}" ] && echo "METRIC code_loc=${loc}"
# Re-emit measure.sh's info/survivor lines so the loop sees the remaining gaps.
printf '%s\n' "$out" | grep -E '^(info|surviving|MISSED|crates/)' || true
exit 0
