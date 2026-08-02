#!/usr/bin/env bash
# The MONOMORPHIZATION SLOPE across the full behaviorpass lattice (bombay card
# #298, the #295 pre-read). One release binary per legal cap-set point (each
# stacks capabilities over the same base actor). Two axes per point:
#   __text : byte-granular code size (`size -m`, the section not the aligned
#            segment). Delta vs plain = that stack's monomorphized code cost.
#   build  : marginal compile time — touch the example, rebuild with deps warm.
# The question: do BOTH grow LINEARLY with capability count, or blow up
# combinatorially (which would make the open source set unaffordable)?
#
# behaviorpass lattice = 15 legal cap-set stacks. (#298's "24" adds bombay's
# Phased inner seats — NoDefer/Bounded x NoTimeout/seat — which the simplified
# behaviorpass Phased does not model.)
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done

text_of() {
	size -m "target/release/examples/$1" 2>/dev/null \
		| awk '/Section __text:/ { print $3; exit }'
}

# Points, plain first then by capability count.
mapfile -t POINTS < <(cd crates/behaviorpass/examples && ls p*.rs | sed 's/\.rs$//' | sort)

echo "warming deps + building ${#POINTS[@]} points…" >&2
for pt in "${POINTS[@]}"; do
	cargo build --release --example "$pt" -q 2>/dev/null || echo "  $pt: BUILD FAIL" >&2
done

TIMEFORMAT='%R'
printf '%-16s %8s %8s %8s\n' point __text d_plain build_s
base=$(text_of p00_plain)
for pt in "${POINTS[@]}"; do
	t=$(text_of "$pt")
	[ -z "$t" ] && { printf '%-16s %8s\n' "$pt" MISSING; continue; }
	touch "crates/behaviorpass/examples/${pt}.rs"
	secs=$( { time cargo build --release --example "$pt" -q 2>/dev/null; } 2>&1 )
	printf '%-16s %8d %8d %8s\n' "$pt" "$t" "$((t - base))" "$secs"
done
