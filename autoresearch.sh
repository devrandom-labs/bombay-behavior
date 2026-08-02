#!/usr/bin/env bash
# Canonical benchmark entrypoint for the behaviorpass concision loop.
#
# Workload: the behaviorpass metric harness (.auto/measure.sh →
# behaviorpass-perf), which reports the code-only LOC of the SUT capability
# machinery and SCORE = K / LOC.
#
# Primary metric:   score      = K / code_loc   (MAXIMIZE — fewer lines)
# Secondary metric: code_loc   (parsed from the perf bin)
#
# Determinism: SCORE is a pure function of the source tree — no network, no
# time-of-day dependence. A compile failure makes measure.sh emit
# METRIC score=0, which passes through unchanged so the loop auto-reverts.
#
# Correctness is NOT measured here; .auto/checks.sh is the hard gate
# (trace-equality + compile_fail + clippy + frozen files).
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
