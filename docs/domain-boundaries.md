# Domain boundaries

This document records the semantic ownership extracted from behavior wrappers.
The purpose is not to turn every collection into a subsystem. A domain earns a
module when it owns invariants and transitions that can be stated and tested
without an interpreter.

## Worker incarnation

`supervision::incarnation` owns the lifecycle behind one stable proxy:

```text
Dormant -> Installing -> Running -> Vacant
                |           |
                |           `-> AwaitingStop -> Installing
                `-> Vacant
```

It distinguishes creation-attempt nonces from the last successfully installed
incarnation, correlates exact creation results, queues at most one successor,
and decides whether forwarding is legal. Its fold returns named creation,
delivery, and report effects. `Proxy` only translates those effects to
`Actions`.

Fresh allocation is the actor-model law. Stable proxy identity, staged
realization, explicit replacement provenance, and result ordering are Bombay
constructions and policies; see [replacement-realization.md](replacement-realization.md).

## Supervised fleet

`supervision::fleet` owns stable child membership, birth sequence, retirement,
and strategy candidate selection. The strategy laws are explicit:

- one-for-one addresses the exact stable slot for each stop observation;
- one-for-all selects every available slot;
- rest-for-one selects available slots at or after the failed slot's birth
  sequence.

These are Bombay supervision policies, not actor-model primitives.

`ChildTopology` is the public construction product for this domain: it keeps
ordered stable nonces and their slot factory together. `RestartConfiguration`
separately owns strategy, eligibility policy, maximum accepted replacements,
and the budget window. The products prevent those independent facts from
being encoded as a positional constructor protocol.

## Restart admission

`supervision::restart_budget` owns the sliding time window and atomic admission
of a replacement set. Rejection never partially charges the set. Future
timestamps remain in the window because the fold cannot assume monotonically
ordered environmental observations.

## Timer lifecycle

`timing` owns two distinct state machines:

- a re-armable lease is `NeverIssued`, `Armed(generation)`, or
  `Idle(last_generation)`;
- an absolute one-shot schedule is `Unscheduled` or `Scheduled`.

This makes contradictory combinations of `live` and `last_issued`
unrepresentable. `ReceiveTimeout` and `Deadline` remain protocol adapters that decide
which typed environmental events belong to their timer.

## Named effect products

Every wrapper send product is a named semantic product because its lanes
coexist independently. Fields describe the protocol owner—for example,
`behavior`, `timers`, and `child_observations`—so adding or reordering wrappers
does not force consumers to navigate positional nesting.

## Intentionally retained structures

`Stash` already owns one explicit route sum and one FIFO buffer in a single
cohesive module. Splitting the queue from the pure replay adapter would not
introduce an additional invariant or pure transition boundary.

The event-lane traits and wrapper event enums are static protocol composition,
not hidden mutable domains. Their repeated forwarding implementations are
deliberately explicit compile-time proofs of which lanes a wrapper consumes or
forwards. A macro or registry would reduce lines while making those proofs less
inspectable.
