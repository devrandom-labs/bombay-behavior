# bombay-behavior — Agent Guide

Repo: **devrandom-labs/bombay-behavior**. Read root `AGENTS.md` before any
semantic change — it is the authoritative design contract (actor-model research
fidelity, static-dispatch rules, naming laws, change method, verification
gates).

## What this is

A Rust workspace of **library crates**: the pure, statically typed functional
core of the Bombay actor stack. `bombay-behavior` defines the `Behavior` fold
and its explicit `Actions` (sends, fresh creations, next behavior/termination);
`bombay-behavior-actors` provides reusable behaviors and typed interpreter
requests built on that algebra.

There is **no application, no server, no ports, no database, and no required
environment variables**. "Running the repo" = building the workspace and
passing its gates (tests, doctests, clippy, fmt, rustdoc).

## Stack

| Item | Value |
|---|---|
| Language | Rust, edition 2024, `rust-version = 1.95.0` |
| Toolchain | 1.95.0 pinned in `rust-toolchain.toml` (single source of truth; rustup auto-selects it) |
| Build | Cargo workspace, 5 crates, `Cargo.lock` committed |
| Reproducible env | Nix flake (crane + fenix) — optional, provides the authoritative `nix flake check` |
| Test runner | cargo-nextest; doctests via `cargo test --doc` |
| Lint / format | clippy (workspace lints in root `Cargo.toml`), rustfmt, taplo (TOML) |
| Docs | rustdoc (`RUSTDOCFLAGS=-D warnings`) + mdbook book in `docs/` |
| CI | `.github/workflows/checks.yml`: cargo-semver-checks, `nix flake check`, fuzz build, strict mutation gate |

## Commands (from repo root)

| Purpose | Command |
|---|---|
| Build workspace | `cargo build --workspace` |
| Run test suite (canonical) | `cargo nextest run --workspace` |
| Run doctests | `cargo test --workspace --doc` |
| Lint | `cargo clippy --workspace --all-targets` |
| Format check / fix | `cargo fmt --check` / `cargo fmt` |
| Rustdoc (warnings fatal) | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` |
| Run example | `cargo run --example behavior` |
| Full authoritative gate (needs Nix) | `nix flake check` |
| Benchmarks (only when evaluating perf) | `cargo bench` (criterion; not nextest binaries) |
| Fuzz (when fuzz surfaces change) | `cargo fuzz` from `crates/behavior-testkit/fuzz`, or `nix run .#fuzz -- build` |
| Mutation gate (expensive, on demand) | `nix build .#mutants -L` |

No services to start. No env vars required (`PROPTEST_CASES=64` is used only by
the mutation lane).

## Codebase map

See `codebase-map.md`.

## Local Verification Summary

Validated 2026-09-17 on a fresh checkout (rustup Rust 1.95.0, cargo-nextest
0.9.145), cold build included:

- `cargo nextest run --workspace` — **779 tests run: 779 passed, 0 skipped**
- `cargo test --workspace --doc` — **44 doctests passed**
- `cargo clippy --workspace --all-targets` — clean (workspace lints active)
- `cargo fmt --check` — clean
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — clean
- `cargo run --example behavior` — exit 0

Primary flow exercised: the `#[behavior::behavior(...)]` macro generating a
concrete `Behavior`, one event accepted through the pure transition fold
returning `Actions::cont()`, with the resulting state asserted
(`crates/behavior/examples/behavior.rs`), plus the full workspace suite above.

## Sandbox snapshot

| Field | Value |
|---|---|
| Snapshot (template) | `pweudotnbpfp0o15dcx5:default` |
| Built at | 2026-09-17T15:23:09.144Z |
| Contains | rustup Rust 1.95.0 (rustfmt + clippy), cargo-nextest 0.9.145, warm `target/` build cache, clean `main` checkout |

Resume from this snapshot to skip toolchain install and the cold build.

## Conventions that will bite you

- `AGENTS.md` bans: `dyn`/`Any`/`TypeId`/downcasting/runtime registries in the
  core, `unsafe` (forbidden workspace-wide), semantic booleans, inline `use`
  statements, and several identifier names ("manager", "helper", "utils",
  "Installation", "Incarnation", …).
- Ordinary PRs must preserve the published API (cargo-semver-checks enforces
  minor unless the PR is labeled `semver-breaking`).
- Compile-fail fixtures in `crates/behavior-macros/tests` prove invalid
  constructions do not compile; keep them meaningful when touching the macro.
- Prefer the narrowest relevant tests first; run the full gates before
  considering a change complete.
