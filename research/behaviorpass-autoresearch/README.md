# behaviorpass autoresearch

This is a test-only, independently built adversarial harness. It treats the
public behaviorpass crate as an immutable subject and never uses its mailbox
driver. `tests/support` supplies a deterministic FIFO mailbox and transition
recorder using only `Behavior::init` and `Behavior::step`.

Research lanes:

- `tests/core.rs`: examples, boundary cases, exhaustive small state spaces;
- `tests/properties.rs`: generated traces checked against independent models;
- `benches/protocol_matrix.rs`: deterministic complexity and throughput ruler;
- `fuzz/fuzz_targets/protocol_sequences.rs`: coverage-guided byte-sequence
  attacks with assertions serving as the oracle.

Run everything except the long-lived fuzzer with:

```sh
.auto/checks.sh
.auto/measure.sh
```

Run coverage-guided fuzzing from the Nix development shell:

```sh
nix develop
cargo fuzz run protocol_sequences \
  --manifest-path research/behaviorpass-autoresearch/fuzz/Cargo.toml
```

When a counterexample appears, minimize it and preserve it as a deterministic
regression test. Do not change production code from this loop.
