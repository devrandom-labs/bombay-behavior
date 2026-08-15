# Immutable contract

## Objective

Audit and tighten the type surfaces of the `bombay-behavior` and
`bombay-behavior-actors` crates. Identify loose or duplicated semantic types,
demonstrably reusable products/sums, and lawful composition opportunities;
retain only changes supported by repository evidence, primary research, and
complete verification.

## Preserved behavior and guarantees

- Preserve the actor transition law and the pure `Behavior` -> typed `Actions`
  boundary defined by `AGENTS.md`.
- Keep `bombay-behavior` minimal and keep reusable actor templates in
  `bombay-behavior-actors`; the dependency direction must remain actors ->
  behavior.
- Preserve concrete static protocols, exhaustive semantic state, explicit
  sends/creations/next behavior, creation freshness, initialization ordering,
  lifecycle provenance, and the universal-driver boundary.
- Do not introduce dynamic dispatch, type erasure, untyped envelopes, runtime
  capability lookup, hidden effects, serialization-based internal protocol
  composition, or `unsafe` type escapes.
- Do not weaken public bounds, widen visibility, add default generics, or add
  compatibility shims to make a proposed abstraction compile.
- Preserve all documented behavior and catalogue classifications unless the
  governing research and implementation boundaries prove a correction is
  necessary; update every affected caller, test, inventory, and document.
- Preserve unrelated work and use reversible, separately evidenced
  experiments.

## Performance contract

No performance change is assumed beneficial. Establish representative
workloads and baseline measurements before accepting any change that can alter
runtime cost, allocation, code size, or compile time. A retained change must
preserve or improve those measured workloads; record any workload-specific
tradeoff explicitly.

## Operator constraints

- Scope is limited to tightening `crates/behavior` and `crates/actors`, plus
  directly required tests and synchronized documentation.
- Treat the operator's observations about loose types, possible reuse, and
  composition as hypotheses to test, not conclusions.
- Research before changing semantics. Distinguish research-mandated law,
  Bombay-derived construction, and deliberate Bombay policy.
- Implement concrete uses before extracting shared machinery. Require at least
  two demonstrated semantic recurrences and validate against a third use where
  applicable.
- Do not start application-domain work or duplicate capabilities owned by
  other Bombay repositories.
- Do not commit, push, publish, or modify release automation unless the
  operator explicitly authorizes it during the loop.
- The loop runs through OMP profile `autoresearch`, whose configured main,
  plan, slow, and smol model is `deepseek/deepseek-v4-flash`.
- Follow `.research/` as the durable ledger and the `devrandom-research` skill
  as the loop protocol.

## Rust and Nix conventions

- Use Nix for development and verification.
- Prefer sum/product modeling over boolean or sentinel protocols.
- Use `thiserror` unless an evidenced constraint prevents it.
- Minimize public API and redundant type specification without erasing domain types.
- Preserve or improve representative performance.
- Run the repository's pinned Rust 1.95.0 toolchain and authoritative Nix
  verification gates.
