# Dynamic supervisor

Status: `feature-complete` locally. `DynamicSupervisor` implements bounded keyed
services, all five management commands, replacement, unexpected worker exit,
lifecycle and diagnostic publication, shutdown, and forced retirement. Focused
unit and model evidence plus four stateful fuzz targets cover cancellation,
transferred cancellation, shutdown, and unexpected exit. Three viable mutation
partitions have no survivors, direct construction inference and authority
forgery have compile contracts, and the obsolete legacy supervision corpus is
gone. Final fresh campaign audits and Bombay root custody remain open.

The complete transition law and exhaustive input matrix have one owner:
[`actor-laws/dynamic-supervisor.md`](actor-laws/dynamic-supervisor.md).
This document is the shorter component guide.

## Responsibility

`DynamicSupervisor` owns a bounded keyed service table; a fresh non-reused
generation for every accepted start or semantic rebind; start, replace, stop,
query, and cancel commands; exact operation correlation and affine cancellation
authority; overlap rejection; lifecycle and diagnostic publication; unexpected
worker-exit policy; capacity release; and complete shutdown or forced retirement.

It delegates stable forwarding and worker replacement to the single
`StableProxy` implementation. It owns no fixed roster, coordinated restart
strategy, restart budget, worker factory, pool queue, customer outcome, runtime
task, mailbox, address allocation, or actor loop. It is not a configurable
`FixedSupervisor`.

Fresh proxy and worker allocation follows the actor-model freshness law. The
keyed table, generation policy, operation correlations, phase-sensitive
cancellation, bounded terminal-operation retention, and durable lifecycle
reporting are Bombay policies.

## Construction

The canonical constructor is:

```rust,ignore
dynamic(
    entries,
    activation,
    unexpected_exit,
    actor_drain,
    lifecycle,
    diagnostics,
)
```

`entries` is a validated `EntryCapacity`; `activation` is a validated
`ActivationPolicy`. Both have checked constructors from `usize`, return
`ZeroCapacity` for zero, and cannot be exchanged accidentally. Every other
argument is a distinct required policy or typed capability. There is no default,
callback, marker, empty route, structural path, application alias, or public
proxy factory.

The complete application syntax and public products are owned by
[`atomic-actor-devx.md`](engineering/atomic-actor-devx.md).

## Commands and outcomes

The application protocol has exactly five commands:

- `Start` admits a new key and complete worker submission;
- `Replace` changes the worker behind an existing stable service;
- `Stop` drains and removes one ready, empty, creating, or waiting service;
- `Query` returns the current public service projection; and
- `Cancel` uses the exact affine authority returned by an accepted start or
  replacement.

Start and replace share `WorkerChangeReceipt` and
`WorkerChangeRejection<Reason>` because their accepted and rejected ownership is
identical; their reason sums remain distinct. Stop, query, and cancellation keep
their own result sums because their outcomes and retained values differ.
`DynamicLifecycle` owns durable later outcomes. `DynamicDiagnostic` owns
malformed, stale, contradictory, or rejected runtime input according to the
configured disposition.

If global shutdown arrives while proxy creation is unresolved, the entry keeps
one of two private current phases: supervisor shutdown, or drain of an already
admitted explicit Stop. Both are publicly `Draining`, but later creation
settlement preserves their different terminal lifecycle outcomes. No optional
stop marker or repeated operation correlation encodes that distinction.

## Service and cancellation rules

An accepted start reserves one fresh entry generation, proxy creation ID,
ordered management operation, and cancellation authority before committing the
entry. The management-operation order also determines which waiting service
receives the next activation authorization. The table itself is the source of
that ordering and capacity use; there is no second queue or occupancy flag. The
affine cancellation authority owns the key and this globally non-reused
operation. Entry generation remains lifecycle identity rather than a duplicate
cancellation coordinate.

Cancellation returns a worker submission only while the supervisor still owns
it locally. After transfer to the proxy, cancellation is logical: the worker is
not reconstructed or returned, and a later successful proxy report cannot
publish `Started` or `Replaced`. The supervisor retains the exact worker result,
proxy-shutdown result, and proxy exit until all three resolve. Only then does it
publish `OperationCancelled` and `EntryRetired(Cancellation)`, remove the entry,
and release entry capacity. Private proxy-input custody names input rejection
and a later proxy report as distinct terminal outcomes; rejection is never
encoded as an absent report. Cancellation completion constructs the ordered
`OperationCancelled` then `EntryRetired(Cancellation)` pair as one closed domain
operation; callers cannot select another retirement cause or omit either
message. The aggregate selects the exact entry once from the creation carried by
the proxy input result; retirement custody does not accept or repeat that
correlation. Arrival order does not change the result.

One matching terminal cancellation record is retained until the next accepted
change. This makes immediate replay truthful without an unbounded history. Once
the entry retires or its key is rebound, the old authority is stale. A rebound
key always receives a fresh generation.

Replacement does not copy StableProxy's predecessor-drain logic. StableProxy
reports replacement only after its own predecessor has drained. The supervisor
validates that report against the retained current service and stores the exact
successor or typed unavailability outcome.

Explicit stop and automatic unexpected-exit retirement remain different
operations. Its terminal failure is one shutdown-settlement reason together
with zero or one exact proxy stop. Those values are independent: the reason is
reported unchanged, while stop absence permits restoration and stop presence
requires retirement. The representation stores neither arrival order nor four
copies of the same optional stop. Stale, duplicate, foreign, wrong-generation,
and wrong-operation inputs return or diagnose their complete values without
mutating the current entry. Proxy input and exit select their exact entry once;
phase transitions do not repeat that same creation comparison or expose an
unreachable second wrong-proxy failure.

## Shutdown

Global shutdown arrives through the existing typed `ShutdownRequested` control
event; it is not a sixth application command. The supervisor closes new
mutations, converts every retained entry exactly once, preserves each unfinished
worker change, and drains according to `ActorDrainPolicy`.

`WaitForActorGraph` waits for every entry. `RetireActorGraphAfter` schedules one
deadline and stops after its exact timer or scheduling rejection while retaining
the unresolved supervisor state for runtime custody. Behavior Actors creates no
second Driver, mailbox, task, registry, or shutdown service.

## Bombay collaboration

Bombay must assemble applications role-first: establish lifecycle and diagnostic
actors, supply their typed capabilities to one pure root construction, and only
then activate the root. DynamicSupervisor stores those capabilities; it never
looks them up or creates them through an application callback.

Bombay must also carry stopped-child output through parent admission to a
non-rejecting root custodian and its existing retirement barrier. The required
Engine and custody changes are specified in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md). They are runtime
composition work, not DynamicSupervisor behavior or a reason to adapt this
aggregate to Bombay's current limitations.

## Verification

Current evidence covers admission and overlap, cancellation replay and stale
authority, pre- and post-transfer cancellation, all six worker/shutdown/exit
orders, retained capacity, fresh same-key reuse, replacement, explicit stop,
unexpected exit, repeated shutdown, forced residual transfer, and complete
named action lanes. Four dedicated fuzz targets each pass 4,096-input replays in
development and optimized profiles. Direct real construction infers without a
final aggregate type, and compile-fail contracts reject duplicated or forged
cancellation authority. Same-signature foreign authority remains AA-20's
runtime-stale case rather than a fabricated compiler-only owner brand. The
verification plan and remaining whole-catalogue gates are owned by
[`atomic-actor-verification.md`](engineering/atomic-actor-verification.md).
