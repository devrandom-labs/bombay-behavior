#!/usr/bin/env bash
# The MONOMORPHIZATION SLOPE (bombay card #298, the #295 pre-read): build one
# release binary per lattice point — each stacks capabilities over the SAME base
# actor — and measure the byte-granular `__text` section. The delta of a point
# vs `plain` is that stack's monomorphized code cost; comparing depth-1 vs
# depth-2/3 answers the #295 question: does binary size grow LINEARLY with the
# number of capabilities, or blow up combinatorially (which would make the open
# source set unaffordable)?
set -uo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	for d in /nix/store/*rust-*/bin; do
		[ -x "${d}/cargo" ] && { PATH="${d}:${PATH}"; export PATH; break; }
	done
fi
for d in /nix/store/*libiconv-1.*/lib; do
	[ -d "${d}" ] && { LIBRARY_PATH="${d}${LIBRARY_PATH:+:${LIBRARY_PATH}}"; export LIBRARY_PATH; break; }
done

# `size -m` prints the byte-granular `__text` SECTION (not the page-aligned
# segment) — the actual machine code.
text_of() {
	size -m "target/release/examples/$1" 2>/dev/null \
		| awk '/Section __text:/ { print $3; exit }'
}

POINTS=(plain deadlined watched supervised stack2 stack3)

echo "building ${#POINTS[@]} points…" >&2
for pt in "${POINTS[@]}"; do
	cargo build --release --example "$pt" -q 2>/dev/null || echo "  $pt: BUILD FAIL" >&2
done

base=$(text_of plain)
printf '%-12s %10s %14s\n' point __text delta_vs_plain
for pt in "${POINTS[@]}"; do
	t=$(text_of "$pt")
	if [ -z "$t" ]; then printf '%-12s %10s\n' "$pt" MISSING; continue; fi
	printf '%-12s %10d %14d\n' "$pt" "$t" "$((t - base))"
done
