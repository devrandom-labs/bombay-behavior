#!/usr/bin/env bash
# GATE for the behaviorpass ADVERSARIAL loop: runs after each measured run. A
# non-zero exit blocks `keep`. The loop may ONLY add new test files under
# crates/behaviorpass/tests/; it must NOT touch src, the existing oracle, or any
# manifest. Enforcement (a BASELINE diff), not trust.
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

# Gate 1 — FROZEN surfaces. The code under test, the existing oracle, and the
# manifests are immutable: the loop closes gaps by ADDING tests, never by
# editing the SUT or weakening what already passes.
FROZEN=(
	crates/behaviorpass/src
	crates/behaviorpass/tests/oracle.rs
	Cargo.toml
	crates/behaviorpass/Cargo.toml
)
if [ -n "${base}" ]; then
	if ! git diff --quiet "${base}" -- "${FROZEN[@]}"; then
		echo "CHECK FAIL: a frozen surface (src / oracle / manifest / metric) was modified — the loop may only ADD crates/behaviorpass/tests/*.rs"
		exit 1
	fi
fi

# Gate 2 — the suite must compile and pass on the REAL code. A red suite means a
# test fails on the real code (a bad test, or a finding); either way the
# mutation measurement is invalid.
if ! cargo test -p behaviorpass >/dev/null 2>&1; then
	echo "CHECK FAIL: the test suite is not green on the real code"
	exit 1
fi

echo "CHECK OK"
