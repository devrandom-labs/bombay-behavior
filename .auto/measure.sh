#!/usr/bin/env bash
# METRIC for the behaviorpass PERF loop: SCORE = a SPACE fitness for the
# `Supervising` children mechanism (bytes allocated constructing supervisors
# across a spread of child counts + the struct footprint). MAXIMIZE — smaller
# is better. The measurement lives in the FROZEN example
# `crates/behaviorpass/examples/perf_supervising.rs`, which touches Supervising
# ONLY through its public constructor + `Behavior::step` — the loop optimizes
# `src/supervising.rs`, it never edits the ruler.
#
# Run UNSANDBOXED. cargo may be absent from a bare loop shell — fall back to the
# pinned nix-store binary (matches rust-toolchain.toml).
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done
export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native"

BIN="target/release/examples/perf_supervising"
LOG=$(mktemp)

# Build once, then exec the bin directly (skill contract — never `cargo run`).
if ! cargo build --release -p behaviorpass --example perf_supervising >"${LOG}" 2>&1; then
	echo "METRIC score=0"
	echo "info: build broke — the loop's change does not compile. Reverting."
	grep -iE 'error' "${LOG}" | head -10
	rm -f "${LOG}"
	exit 0
fi

out=$("${BIN}" 2>&1)
score=$(printf '%s\n' "${out}" | grep -oE '^METRIC score=[0-9.]+' | head -n1 | cut -d= -f2)
if [ -z "${score}" ]; then
	echo "METRIC score=0"
	echo "info: perf bin produced no score — runtime failure. Reverting."
	printf '%s\n' "${out}" | tail -10
	rm -f "${LOG}"
	exit 0
fi

echo "METRIC score=${score}"
# Re-emit the space/throughput breakdown so the loop sees where the bytes are.
printf '%s\n' "${out}" | grep -E '^info' || true
rm -f "${LOG}"
