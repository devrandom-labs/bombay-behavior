#!/usr/bin/env bash
# GATE for the behaviorpass PERF loop: runs after each measured run. A non-zero
# exit blocks `keep`. The loop optimizes the `Supervising` children mechanism
# and MAY ONLY edit `crates/behaviorpass/src/supervising.rs` (+ `lib.rs`'s
# supervising re-export) and the Supervising-specific tests it needs to keep
# green. Everything else — the OTHER capability modules, the manifests, the
# ruler (the perf example), and this harness — is FROZEN. Enforcement (a
# BASELINE diff), not trust.
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done

base=$(cat .auto/BASELINE 2>/dev/null || true)

# Gate 1 — FROZEN surfaces. The loop may reshape Supervising's REPRESENTATION and
# its bespoke getters, but never: another capability's source, the generic
# Behavior grammar, a manifest, the measurement ruler, or the harness itself.
FROZEN=(
	crates/behaviorpass/src/behavior.rs
	crates/behaviorpass/src/deadlined.rs
	crates/behaviorpass/src/stashing.rs
	crates/behaviorpass/src/watching.rs
	crates/behaviorpass/src/fsm.rs
	crates/behaviorpass/src/exit.rs
	crates/behaviorpass/examples/perf_supervising.rs
	Cargo.toml
	crates/behaviorpass/Cargo.toml
	.auto/measure.sh
	.auto/checks.sh
	.auto/prompt.md
	.auto/hooks/before.sh
)
if [ -n "${base}" ]; then
	if ! git diff --quiet "${base}" -- "${FROZEN[@]}"; then
		echo "CHECK FAIL: a frozen surface was modified — the loop may only touch src/supervising.rs (+ its tests). Other modules / manifests / the ruler / the harness are immutable."
		exit 1
	fi
fi

# Gate 2 — the generic Behavior contract must hold. The whole suite (every
# capability's existing tests, incl. Supervising's behavior assertions) must
# compile and pass on the REAL code, and every target (examples/benches) must
# still build. A representation change that alters what `step`/`next_deadline`
# observably do breaks a test here.
if ! cargo test -p behaviorpass --all-targets >/dev/null 2>&1; then
	echo "CHECK FAIL: the suite is not green on the real code (behavior changed, or a test/example no longer compiles)"
	exit 1
fi

echo "CHECK OK"
