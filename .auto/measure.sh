#!/usr/bin/env bash
# METRIC for the behaviorpass concision loop: SCORE = K / code-only-LOC of the
# SUT capability machinery (crates/behaviorpass/src/**). MAXIMIZE — fewer lines
# ⇒ higher score. This is CONCISION golf, not throughput.
#
# Correctness (trace-equality to the frozen reference) and the 17 illegal-point
# compile_fails are hard GATES in .auto/checks.sh, NOT measured here. A compile
# break in the perf bin ⇒ no SCORE ⇒ parsed as 0 ⇒ auto-revert.
#
# Run UNSANDBOXED (cargo hangs under a sandboxed shell).
set -uo pipefail

# cargo may be absent from a non-interactive loop shell; fall back to a pinned
# nix-store rust (matches rust-toolchain.toml).
if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		if [ -x "${d}/cargo" ]; then
			PATH="${d}:${PATH}"
			export PATH
			break
		fi
	done
fi

# The nix-store rust links via system clang, which cannot find libiconv outside
# a nix shell; point it at the nix store copy.
for d in /nix/store/*libiconv-1.*/lib; do
	if [ -d "${d}" ]; then
		LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"
		export LIBRARY_PATH
		break
	fi
done

export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native"

# Build the (frozen) metric bin, then exec it directly.
if ! out_build=$(cargo build -q -p behaviorpass-perf --release 2>&1); then
	echo "METRIC score=0 unit=inv_loc"
	echo "info: perf bin failed to build — reverting"
	printf '%s\n' "${out_build}" | tail -8
	exit 0
fi

out=$(./target/release/behaviorpass-perf 2>&1)
score=$(printf '%s\n' "${out}" | grep -oE 'SCORE=[0-9.]+' | head -1 | cut -d= -f2)
loc=$(printf '%s\n' "${out}" | grep -oE 'CODE_LOC=[0-9]+' | head -1 | cut -d= -f2)

if [ -z "${score}" ] || [ "${score}" = "0" ]; then
	echo "METRIC score=0 unit=inv_loc"
	echo "info: SUT has no scorable machinery yet (code_loc=${loc:-0}) — the trace-equality gate reverts an empty SUT"
	exit 0
fi

echo "METRIC score=${score} unit=inv_loc"
echo "info: code_loc=${loc}"
