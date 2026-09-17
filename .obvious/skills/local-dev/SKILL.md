---
name: local-dev
description: Stand up a working dev environment for the bombay-behavior Rust library workspace and verify it end to end
---

# Local dev onboarding record

Validated 2026-09-17 on sandbox `cmp_yFTqlREH`, fresh checkout of `main`
(b9642e8), no prior environment.

## Environment facts

- Debian 13, 8 vCPU, ~8 GB RAM. No Rust, no Docker, no Nix preinstalled.
- Network: rust-lang.org, crates.io index, and GitHub all reachable; gh CLI is
  pre-authenticated. `get.nextest.site` is NOT resolvable — install
  cargo-nextest from the GitHub release asset instead.
- Nothing to start: this is a pure library workspace. No services, ports,
  databases, or required env vars.

## Setup (rustup path — used and validated)

```sh
curl -sSf https://sh.rustup.rs | sh -s -- -y \
  --default-toolchain 1.95.0 --profile minimal --no-modify-path
export PATH="$HOME/.cargo/bin:$PATH"
rustup component add rustfmt clippy --toolchain 1.95.0

# cargo-nextest (get.nextest.site is blocked here; fetch the release asset):
URL=$(gh api repos/nextest-rs/nextest/releases/latest --jq \
  '.assets[].browser_download_url | select(contains("x86_64-unknown-linux-gnu.tar.gz"))')
curl -LsSf "$URL" -o /tmp/nextest.tgz && tar zxf /tmp/nextest.tgz -C ~/.cargo/bin
```

`rust-toolchain.toml` pins 1.95.0; running cargo inside the repo selects it
automatically via rustup. The Nix flake (`nix develop`) is the preferred
reproducible route when Nix is available; the rustup path covers the same core
gates without it.

## Verification (all must pass)

```sh
cargo nextest run --workspace    # 779/779 passed, ~68s including cold build
cargo test --workspace --doc     # 44 doctests passed
cargo clippy --workspace --all-targets
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo run --example behavior     # exit 0
```

Primary flow: `crates/behavior/examples/behavior.rs` — the behavior attribute
generates a concrete `Behavior`; one accepted message runs the pure transition
and returns `Actions::cont()`, with the new state asserted.

## Notes for future workers

- The whole gate suite completes in about a minute on 8 cores once built; the
  cold build dominates. Run long builds detached (tmux) and poll.
- `nix flake check` is the authoritative repository gate (adds TOML format,
  dependency audit, and deny policy) but requires Nix, which the onboarding
  sandbox did not have.
- `cargo bench` builds criterion benchmarks deliberately; they are not nextest
  binaries. The mutation lane (`nix build .#mutants -L`) is expensive and
  on-demand — skip both unless the relevant surfaces changed.
- A snapshot (template `pweudotnbpfp0o15dcx5:default`, sandbox session `iw3bm4r5yc2xq8gz5tjgu`, built 2026-09-17T15:23:09.144Z) already contains the installed toolchain,
  cargo-nextest, and a warm `target/` cache; resuming from it skips all setup.
