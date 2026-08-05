#!/usr/bin/env bash
set -euo pipefail

baseline=$(cat .auto/BASELINE)

if ! git diff --quiet "${baseline}" -- crates Cargo.toml README.md docs; then
	echo "CHECK FAIL: production or product documentation changed"
	exit 1
fi

if ! git diff --quiet "${baseline}" -- .auto autoresearch.sh; then
	echo "CHECK FAIL: the research loop modified its own rules"
	exit 1
fi

cargo test --manifest-path research/behaviorpass-autoresearch/Cargo.toml --all-targets
cargo clippy --manifest-path research/behaviorpass-autoresearch/Cargo.toml --all-targets -- -D warnings

if rg -n 'fastpass' research/behaviorpass-autoresearch --glob '*.rs' --glob 'Cargo.toml'; then
	echo "CHECK FAIL: the isolated research harness must not use fastpass"
	exit 1
fi

echo "CHECK OK"
