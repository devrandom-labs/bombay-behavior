#!/usr/bin/env bash
# Canonical benchmark entrypoint for the behaviorpass PERF loop (the Supervising
# children mechanism).
#
# Workload: build + exec the FROZEN ruler
# (crates/behaviorpass/examples/perf_supervising.rs), which measures the SPACE
# footprint of Supervising's liveness table through the public constructor and
# the generic Behavior::step only. Same binary, same flags, no network, no
# time-of-day inputs — deterministic given the same source.
#
# Primary metric:  score = 1e6 / (1 + space_bytes)   (MAXIMIZE — smaller table)
# Secondary:       space_bytes / alloc_bytes / struct_size / step_throughput_per_s
#
# exit 0 => valid measurement emitted; non-zero => harness or ruler failure
# (release build broke, or the ruler produced no score) — the loop treats a
# non-zero exit as a crash and reverts the change.
#
# Correctness is NOT measured here; .auto/checks.sh is the hard gate (frozen
# surfaces + the whole suite green, which pins the generic Behavior contract).
#
# Run UNSANDBOXED (cargo hangs under a sandboxed shell).
set -uo pipefail

# The loop shell may be bare — fall back to the pinned nix-store toolchain
# (matches rust-toolchain.toml), as .auto/measure.sh does.
if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
if ! command -v cargo >/dev/null 2>&1; then
	echo "harness error: cargo not found" >&2
	exit 1
fi
# macOS: libiconv from the nix store for linking.
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done
export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native"

BIN="target/release/examples/perf_supervising"
LOG=$(mktemp)
trap 'rm -f "${LOG}"' EXIT

# Build once, then exec the bin directly (never `cargo run`).
if ! cargo build --release -p behaviorpass --example perf_supervising >"${LOG}" 2>&1; then
	echo "harness error: release build of the ruler failed" >&2
	grep -iE '^error' "${LOG}" | head -10 >&2
	exit 1
fi

out=$("${BIN}" 2>&1)

score=$(printf '%s\n' "${out}" | grep -oE '^METRIC score=[0-9.]+' | head -n1 | cut -d= -f2)
if [ -z "${score}" ]; then
	echo "harness error: ruler produced no METRIC score" >&2
	printf '%s\n' "${out}" | tail -10 >&2
	exit 1
fi

echo "METRIC score=${score}"
# Secondary metrics as METRIC lines so the loop records where the bytes are.
for key in space_bytes alloc_bytes struct_size step_throughput_per_s; do
	val=$(printf '%s\n' "${out}" | grep -oE "${key}=[0-9.]+" | head -n1 | cut -d= -f2)
	[ -n "${val}" ] && echo "METRIC ${key}=${val}"
done
# Re-emit the full breakdown so the loop's log shows where the bytes are.
printf '%s\n' "${out}" | grep -E '^info' || true
exit 0
