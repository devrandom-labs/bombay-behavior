# Atomic actor verification contract (engineering record)

Status: normative verification owner. No family document duplicates this
matrix; each links here and adds only family-specific law references.

## Current campaign state

`feature-complete` means every currently named aggregate-local gate passes. It
does not mean the whole campaign, Bombay integration, or the Bombay system is
complete. `active` means production behavior exists but a named local gate is
still open. The family law documents remain semantic oracles; they do not by
themselves establish implementation status.

| Family | State | Executable evidence | Remaining gate |
|---|---|---|---|
| [`StableProxy`](../stable-proxy.md) | `feature-complete` locally | independent models, compile contracts, two stateful fuzz targets | final fresh audits and Bombay root custody |
| [`FixedSupervisor`](../fixed-supervisor.md) | `feature-complete` locally | focused models, exact multi-role examples, four stateful fuzz targets, 34 worker-rejection cases, 30 source/corrupt/unattempted preparation-return cases, all 5,040 coordinated preparation/shutdown arrival orders, every distinct-role `RestForOne` overlap pair, 540 lawful three-recovery correlation traces, complete 753-candidate owner reconciliation, type-valid root/protocol inversions, exact lifecycle/management/event projections, and full aggregate-residue audit | final fresh audits and Bombay custody |
| [`DynamicSupervisor`](../dynamic-supervisor.md) | `feature-complete` locally | independent models, compile contracts, three closed mutation partitions, and four stateful fuzz targets | final fresh audits and Bombay root custody |
| [`FifoPool`](../fifo-pool.md) | `feature-complete` locally | independent customer/recovery/retirement/queue models, mutation audit, performance workload, one stateful fuzz target | final fresh audits and Bombay custody |
| [`KeyedPool`](../keyed-pool.md) | `feature-complete` locally | two bounded independent models, shared customer law suite, two stateful fuzz targets, and complete 232-candidate owner reconciliation with all 72 executable mutations caught and 160 compiler-unviable substitutions retained only as inventory | final fresh audits and Bombay custody |

The current fuzz manifest therefore contains thirteen replacement-family targets:
two proxy, four fixed-supervisor, four dynamic-supervisor, one FIFO, and one
binding-only plus one assignment/retirement keyed target. A prior combined keyed assignment/binding/retirement
target was rejected because it mixed three independently testable
responsibilities; its absence is not evidence that those gates passed.

## Evidence order

1. State the actor-model, derived, or Bombay policy law.
2. Record the user syntax and complete observable transition.
3. Add a focused compile or pure-fold regression in domain vocabulary that
   fails against the prior design for that law.
4. Run deliberate inversion and show failure for the intended reason.
5. Implement the smallest representation only after the law and interpreter
   path are coherent.
6. Audit every production symbol against its pre-edit law and regression.

Compiler diagnostics can reject an encoding but cannot originate a type,
trait, bound, callback, wrapper, alias, route, or policy.

## Per-aggregate matrix

Every aggregate requires:

- exhaustive state/input classification and complete action/settlement
  assertions;
- an independently structured model using different vocabulary;
- bounded exhaustive interleaving exploration;
- property tests over longer sequences with invariants checked after every
  step;
- fuzzing of stale, duplicate, foreign, overlap, cancellation, retry, shutdown,
  and wrong-generation facts as applicable;
- debug and optimized replay;
- compile-pass canonical construction and use;
- compile-fail forged capabilities, tokens, identities, routes, and incomplete
  builders wherever those values are application-constructible or
  type-distinct;
- deliberate inversion for every law-sensitive regression;
- complete drain from every live phase;
- no lost affine value, duplicated terminal outcome, or inferred provenance;
  and
- no production panic for ordinary exhaustion or rejection.

Tests assert the complete `Actions` value or an independent trace. They do not
discard actions, predict private nonces, copy implementation branches into the
model, or put required transitions inside assertions.

Compile-time denial does not manufacture instance identity. When two actor
instances intentionally share one concrete protocol, same-signature foreign
correlations remain well-typed and their owning aggregate must reject them as
stale without mutation. The compile contract instead prevents application
forgery, duplication of affine authority, and substitution across genuinely
different semantic types.

## Shared-model gate

One independent law suite runs unchanged against every proposed consumer. A
shared model is rejected if any consumer needs a mode flag, ignored result,
placeholder, weakened error, alternate ordering, or special terminal policy.
Extraction follows the second real consumer and must remove more semantic
machinery than it adds.

Generic interpretation additionally requires two unrelated catalogue templates
and two wrapper orders with no placeholder settlement event or per-template
adapter. The real Bombay integration probe must conserve every affine value
through the unchanged single Driver and retirement barrier.

## Clean-room gate

Before deletion, preserve implementation-independent black-box traces, compile
denials, adversarial cases, independent model expectations, and performance
workloads. Commit that evidence separately, then create one explicit legacy
deletion commit.

After deletion verify that every scoped symbol is removed or classified outside
scope, no replacement module imports the baseline implementation, no legacy
alias or alternate application path remains, and characterization tests cover
every retained law. Deliberate policy changes are documented and tested rather
than hidden as compatibility regressions.

## DevX and performance evidence

Compare meaningful application lines, annotations, aliases, turbofish,
structural concepts, public spellings, diagnostic size/distance, compiler time,
monomorphization, binary size, allocations, and representative throughput.
Define workloads and measure noise before judging differences. Code reduction
is never inferred from a net-positive production delta.

## Repository gates

All Rust/Cargo commands run through the pinned Nix environment. Feature-local
gates precede the full matrix:

```sh
nix develop -c cargo nextest run --workspace
nix develop -c cargo fmt --all -- --check
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix flake check
```

Applicable rustdoc, compile-fail, exhaustive, property, fuzz, benchmark,
optimized, mutation, and model-checking gates are additional requirements, not
substitutes. Candidate fixed point requires two consecutive fresh adversarial
whole-repository audits with no credible untested high-value hypothesis.
