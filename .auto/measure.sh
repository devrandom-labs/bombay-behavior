#!/usr/bin/env bash
# METRIC for the behaviorpass ADVERSARIAL loop: SCORE = mutants CAUGHT by the
# test suite over crates/behaviorpass/src (cargo-mutants). MAXIMIZE — a MISSED
# (surviving) mutant is an invariant no test pins. src is FROZEN (checks.sh);
# the loop may ONLY add test files under crates/behaviorpass/tests/.
#
# Run UNSANDBOXED. cargo / cargo-mutants may be absent from a bare loop shell —
# fall back to the pinned nix-store binaries (matches rust-toolchain.toml).
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
if ! command -v cargo-mutants >/dev/null 2>&1; then
	for d in /nix/store/*cargo-mutants*/bin; do
		[ -x "${d}/cargo-mutants" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done
export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native"

# --in-place is REQUIRED: sibling path-deps (../bombay, ../fastpass) break under
# cargo-mutants' default copy-to-tmp. --timeout bounds a timeout-removing mutant.
LOG=$(mktemp)
cargo mutants --in-place --package behaviorpass --timeout 60 >"${LOG}" 2>&1 || true

summary=$(grep -E '[0-9]+ mutants tested' "${LOG}" | tail -1)
caught=$(printf '%s' "${summary}" | grep -oE '[0-9]+ caught' | grep -oE '^[0-9]+')
missed=$(printf '%s' "${summary}" | grep -oE '[0-9]+ missed' | grep -oE '^[0-9]+')

if [ -z "${caught}" ]; then
	# No summary ⇒ build broke OR the baseline suite failed (a test fails on the
	# REAL code: a bad test, or a genuine FINDING). Either way the run is invalid.
	echo "METRIC score=0 unit=mutants_caught"
	echo "info: no mutants summary — build broke or baseline suite failed (test fails on real code). Reverting; surface if it looks like a finding."
	grep -iE 'error|FAILED|baseline' "${LOG}" | tail -10
	rm -f "${LOG}"
	exit 0
fi

echo "METRIC score=${caught} unit=mutants_caught"
echo "info: ${summary#*: }"
if [ "${missed:-0}" -gt 0 ]; then
	echo "surviving mutants (the gaps to close):"
	grep '^MISSED' "${LOG}" | sed 's/ in [0-9].*//' | head -20
fi
rm -f "${LOG}"
