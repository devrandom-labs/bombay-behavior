#!/usr/bin/env bash
set -euo pipefail

output=$(cargo bench \
	--manifest-path research/behaviorpass-autoresearch/Cargo.toml \
	--bench protocol_matrix 2>&1)

printf '%s\n' "${output}"
score=$(printf '%s\n' "${output}" | sed -n 's/^METRIC score=//p' | tail -n 1)
if [ -z "${score}" ]; then
	echo "METRIC score=0"
	exit 1
fi
