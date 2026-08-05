# behaviorpass adversarial test research

## Objective

Find real counterexamples in behaviorpass by adding tests only. Maximize the
number and diversity of meaningful invariant checks across deterministic tests,
model-based properties, coverage-guided fuzzing, performance experiments, and
combinations of those methods.

The goal is not a green score. The goal is honest attempts to falsify the actor
algebra and its compositions. Preserve the smallest reproducer for every bug.
Never weaken, ignore, or special-case a failing assertion to make the loop pass.

## Immutable subject

All production code is frozen. Never edit anything under `crates/`, either
workspace manifest, or existing product documentation. Do not use `fastpass`,
including behaviorpass's `run` helper. Emulate delivery, mailboxes, scheduling,
monitoring, and child routing with test-owned deterministic data structures.

The only writable research surface is `research/behaviorpass-autoresearch/**`.
The loop may add unlimited test modules, generators, reference models, fuzz
targets, corpora, regression seeds, and benchmark workloads there. It must not
alter `.auto` or `autoresearch.sh` while researching.

## Core invariants to attack

1. Every transition is exactly typed sends, fresh creates, and become.
2. Initialization effects are observable and occur exactly once.
3. Send/create order and wrapper preservation are lossless under composition.
4. Event injection reaches exactly one intended layer; stale and duplicate
   environmental events are harmless.
5. Stash/release preserves order, multiplicity, origin, and terminal verdicts.
6. FSM deferral/replay is deterministic and never drops or duplicates input.
7. Supervision policy, strategy, restart window, birth order, stable proxy
   routing, and fresh generations agree with an independent reference model.
8. `workers!` sums preserve the selected concrete variant and protocol.
9. Deep wrapper stacks and every feasible wrapper ordering remain lawful.
10. Work and memory growth remain proportional to emitted algebra.

Prefer exhaustive small-state enumeration before random generation. Properties
must use independent models instead of restating implementation branches. Test
zero, one, maximums, equal timestamps, window edges, duplicate nonces, empty
fleets, terminal transitions, and long queues.

## Required commands

```sh
.auto/checks.sh
.auto/measure.sh
cargo fuzz run protocol_sequences --manifest-path research/behaviorpass-autoresearch/fuzz/Cargo.toml
```

Fuzzing is optional only when `cargo-fuzz` is unavailable; the target remains
part of the scaffold and must compile in a fuzz-capable environment.
