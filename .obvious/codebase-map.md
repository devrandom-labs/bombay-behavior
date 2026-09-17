# bombay-behavior — Codebase Map

Rust workspace of 5 library crates (264 `.rs` files). No apps, no services,
no servers. Dependencies flow strictly: `behavior` ← `actors` ←
`behavior-testkit`; `behavior-macros` emits types from `behavior`; the core
never depends on an executor or transport.

| Path | What lives there |
|---|---|
| `crates/behavior` | `bombay-behavior` — public behavior primitives: pure `Behavior` transition fold, `Actions` effect boundary, actor/event algebra. `examples/behavior.rs` demo, `tests/` |
| `crates/actors` | `bombay-behavior-actors` — reusable behaviors and typed interpreter requests on the core algebra. `src/atomic/` (fixed & dynamic supervisors, FIFO & keyed pools, stable proxy, workers, restart/roster/schedule), plus composition, routing, lifecycle, time, watch, stash, protocol, workflow, persistence, discovery, operations; `benches/` (criterion), `tests/` |
| `crates/behavior-macros` | `bombay-behavior-macros` — `#[behavior::behavior(...)]` attribute; emits the same concrete types a careful user could hand-write. `tests/` includes compile-pass and compile-fail crate-resolution fixtures |
| `crates/behavior-testkit` | `bombay-behavior-testkit` — independent models, adversarial suites, exhaustive checks, properties, benchmarks. `fuzz/` holds libFuzzer targets and corpus (own Cargo.toml, run via `cargo fuzz`) |
| `crates/mutants-gate` | internal `behavior-mutants-gate` — judges cargo-mutants output against `mutants-baseline.json` (strict per-function viability ratchet) |
| `docs/` | mdbook book: `SUMMARY.md`, `book.toml`, 36 law/architecture pages (actor transition algebra, layer laws, established capabilities, pool/supervisor docs, `engineering/` working notes) |
| `scripts/` | `check_published_docs.py` + its unittest, `check_published_packages.sh` — published crate/docs consistency checks |
| `.github/workflows` | `checks.yml` (semver audit, `nix flake check`, fuzz build, mutation gate), `docs.yml`, `fuzz.yml`, `release-plz.yml`, `reserve-crate.yml`, `prune-release-plz-branches.yml` |
| repo root | `Cargo.toml` (workspace + clippy/rust lint policy), `rust-toolchain.toml` (1.95.0), `flake.nix`/`flake.lock` (crane/fenix build matrix), `deny.toml`, `audit.toml`, `mutants-baseline.json`, `release-plz.toml`, `rustfmt.toml`, `taplo.toml`, `AGENTS.md` (authoritative design contract), `README.md` |
