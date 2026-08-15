# Baseline

## Environment

- Scaffold commit: `52900f35f4bbaef9d6c0132d8dd755f1f82f5765`
- Branch: `split-behavior-actors`
- Rust toolchain and workspace MSRV: 1.95.0
- Platform at scaffold time: aarch64-darwin
- Model profile: OMP `autoresearch`, `deepseek/deepseek-v4-flash`
- Flake lock, hardware, and build-mode details: capture when the loop begins.

## Verification

Not run during scaffolding. The loop must record exact output for at least:

```sh
cargo fmt --all -- --check
cargo nextest run --workspace
cargo test --workspace --doc
nix flake check
git diff --check
```

It must also run applicable compile-fail, fuzz, architecture-review,
dependency-policy, and dependency-audit gates discovered in the repository.

## Measurements

Not established during scaffolding. Store raw results under `.research/` and
record workload, sample count, dispersion, build mode, and environment before
comparing experiments.

## Rust surface

The scoped packages are `bombay-behavior` (`crates/behavior`) and
`bombay-behavior-actors` (`crates/actors`). Public-item counts, duplicate type
representations, boolean/sentinel protocols, errors, dependencies, warnings,
and unsafe usage remain unmeasured until the first audit.
