# Baseline

## Environment

- Scaffold commit: `52900f35f4bbaef9d6c0132d8dd755f1f82f5765`
- Branch: `split-behavior-actors`
- Rust toolchain and workspace MSRV: 1.95.0
- Platform at scaffold time: aarch64-darwin
- Model profile: OMP `autoresearch`, `deepseek/deepseek-v4-flash`
- Flake lock: repository `flake.lock` at the scaffold commit plus the live
  uncommitted audit diff.
- Build modes exercised: Cargo dev/test profiles and the Nix flake checks.

## Verification

The loop and independent audit recorded:

```sh
cargo fmt --all -- --check
cargo nextest run --workspace
cargo test --workspace --doc
nix flake check
git diff --check
```

- `cargo nextest run --workspace`: 283 passed.
- `cargo test --workspace --doc`: 10 passed, including compile-fail doctests.
- `cargo fmt --all -- --check`: passed.
- Scoped Clippy: passed warning-clean.
- `git diff --check`: passed.
- `nix flake check`: 7/7 checks passed, including build, nextest, docs,
  formatting, TOML formatting, dependency audit, and dependency policy.
- Independent audit reran pinned-Nix formatting, all-target workspace checking,
  and documentation tests after correcting E05; all passed.

Applicable fuzz and architecture-review results must remain attached to the
catalogue's existing verification records; this loop did not produce a new
fuzz corpus or claim a new architecture-gate result beyond `nix flake check`.

## Measurements

No performance claim was used to accept a semantic or public-API change. E08
and E11 remove evident intermediate allocations; no benchmark numbers were
claimed. Reopen them if representative workload measurements show a reversal.

## Rust surface

The scoped packages are `bombay-behavior` (`crates/behavior`) and
`bombay-behavior-actors` (`crates/actors`). The loop inventoried 622 public
items and found zero production `unsafe`. Its retained/rejected type findings
are recorded in `EXPERIMENTS.jsonl` and `DEAD_ENDS.md`. E05's unused primitive
conversion APIs were subsequently reverted because they violated the explicit
no-convenience-API rule in `AGENTS.md`.
