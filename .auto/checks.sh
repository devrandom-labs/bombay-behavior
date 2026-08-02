#!/usr/bin/env bash
# GATE for the behaviorpass concision loop: runs after a measured experiment. A
# non-zero exit blocks `keep`, so a terser-but-wrong (or cheating) design is
# REVERTED even when its SCORE improved. This is what lets the loop golf LOC
# aggressively without regressing correctness — the lint bar is the regularizer
# that stops a line being bought with unreadability.
#
# Gates:
#   1. Frozen surfaces unchanged vs the baseline commit — the reference (the
#      gold fold + layers), the oracle (testkit), the metric harness, and (once
#      they exist) the frozen conformance test files. The loop may rewrite
#      crates/behaviorpass/src/** and its Cargo.toml, but not what defines
#      "correct" or "how few lines".
#   2. Trace-equality: the oracle suite is green (SUT actor ≡ reference fold at
#      every lattice point).
#   3. The 17 illegal lattice points still fail to compile (trybuild).
#   4. The god-level clippy bar holds on the SUT (the LOC regularizer).
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		if [ -x "${d}/cargo" ]; then
			PATH="${d}:${PATH}"
			export PATH
			break
		fi
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	if [ -d "${d}" ]; then
		LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"
		export LIBRARY_PATH
		break
	fi
done

base=$(cat .auto/BASELINE 2>/dev/null || true)
# Frozen surfaces. Phase-0: the three research crates. As the frozen
# conformance/trybuild test files are authored (phase-1), ADD them here — a new
# frozen test becomes part of the freeze the moment it lands (the #298 rule).
FROZEN=(
	crates/behaviorpass-reference
	crates/behaviorpass-testkit
	crates/behaviorpass-perf
	crates/behaviorpass/tests/oracle.rs
)
if [ -n "${base}" ]; then
	if ! git diff --quiet "${base}" -- "${FROZEN[@]}"; then
		echo "CHECK FAIL: a frozen surface (reference / oracle / metric harness) was modified"
		exit 1
	fi
fi

# Gate 2 — trace-equality oracle. Phase-0 has no oracle tests yet, so this is
# vacuously green; it becomes load-bearing the moment behaviorpass-testkit's
# suite + the SUT's generated actors land.
if ! cargo test -p behaviorpass --tests --no-fail-fast >/dev/null 2>&1; then
	echo "CHECK FAIL: trace-equality oracle suite is not green"
	exit 1
fi

# Gate 3 — illegal-point compile_fails (trybuild). Phase-1 adds
# crates/behaviorpass/tests/compile_fail.rs with the 17 illegal stacks; until
# then this gate is a no-op placeholder. DO NOT let it pass silently once the
# cases exist — wire the trybuild runner here.

# Gate 4 — the LOC regularizer: the SUT must clear the workspace clippy bar
# (all=deny). A line bought with unreadability is reverted.
if ! cargo clippy -q -p behaviorpass --all-targets >/dev/null 2>&1; then
	echo "CHECK FAIL: SUT does not clear the clippy bar (the concision regularizer)"
	exit 1
fi

echo "CHECK OK"
