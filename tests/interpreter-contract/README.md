# Interpreter contract fixture

This isolated Cargo workspace exercises the public Behavior and Actors
interpreter ports. It uses the current source tree and has its own lockfile, so
it is not covered by the repository workspace test command.

Run from the repository root with the pinned Nix toolchain:

```sh
nix develop -c python3 scripts/check_interpreter_contract_graph.py
nix develop -c cargo test --locked --manifest-path tests/interpreter-contract/Cargo.toml
nix develop -c cargo test --locked --release --manifest-path tests/interpreter-contract/Cargo.toml
```

The graph check rejects a registry copy or a second version of the core,
Actors, or macros crate. The tests cover complete assignment rejection,
same-typed assignment correlation, accepted and residual source custody,
precommit host refusal and panic custody, and compile-time denial of forged
assignment and proxy evidence.

The fixture's local interpreters are deliberately narrow. They prove the
Behavior-side ownership shape; they do not prove Address reservation,
Communication delivery, Bombay Driver continuation, endpoint publication, or
parent-to-root retirement. Those require a later immutable downstream graph
and the real runtime witnesses in the
[ownership and startup PRD](../../docs/engineering/creation-and-delivery-custody-prd.md).
