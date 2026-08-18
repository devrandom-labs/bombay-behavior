# Functional Core

The fold described here is orthogonal to public actor identity. `Protocol`
names only an established `Addr`/`Msg` destination; `Behavior::Event` is the
larger internal input algebra, and `Actions` is the explicit output algebra.
See [Protocol, event, behavior, and effect algebras](protocol-algebra.md).

Bombay models a behavior as a typed, deterministic transition:

```text
(behavior state, event) -> Result<(behavior state, Actions), Error>
```

Rust represents the retained state by `&mut self`, but `Behavior::transition`
performs no scheduling, mailbox receive, clock access, transport, or actor
creation. Its complete observable result is `Actions`. This is the repository's
pure-core architecture law, not a claim that Agha prescribes a Rust API.

`Actions` is Bombay's typed realization of communications, staged fresh actor
creation, and the next behavior or termination. Creation-before-send
interpretation and initialization-before-mailbox ordering remain deliberate
Bombay policies.

## Reduction

`ActionReducer` is the single left-fold accumulator used by finite stream,
model, testkit, and fuzz evaluation. Production actor execution is owned by the
universal `bombay-engine::Driver`, which delegates one event at a time through
Bombay Transition and Machine Executor and hands each complete `Actions` value
to its environment. It does not use `ActionReducer` as a second mailbox loop.
The reducer obeys these laws:

- send accumulation has identity and is associative;
- creation vectors preserve transition order;
- initialization is the first transition;
- the first `Stop` short-circuits the fold;
- actions after a controlled error or `Stop` are not accumulated.

`fold_events` applies those laws to any `IntoIterator` using
`Iterator::try_fold` and `ControlFlow`. This keeps finite test vectors,
generated sequences, and interpreter-driven streams on the same reduction
path.

## Products and coproducts

Event wrappers are exhaustive coproducts. Effect wrappers are named products:
`DeadlineSends` exposes `behavior` and `schedules`; `WatchSends` exposes
`behavior` and `observations`; `ReceiveTimeoutSends` exposes `behavior` and
`schedules`. Wrapper order remains visible in the type and repeated capability
types remain distinguishable by their typed nesting and timer identities.

Transparent wrappers set `type Protocol = B::Protocol`. They may extend the
event coproduct and effect product, but they do not create a new public
recipient identity.

Frunk's HLists and coproducts were evaluated for this role. They provide useful
generic mapping, folding, selection, and sculpting, but selection of repeated
lane types still requires type-level position or labels. That would relocate
rather than remove Bombay's identity and naming problem, while adding a generic
dependency to the foundational algebra. Concrete named products therefore fit
the domain more truthfully.
