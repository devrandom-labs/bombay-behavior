# Actor-template composition audit

## Authoritative redesign ledger

This section supersedes every historical "fixed" or provisional design claim
later in this file. Those sections are retained only until the corresponding
code and tests have been deleted; they are not completion evidence.

### Laws

1. `BehaviorLayer` constructs one concrete same-mailbox transformation. A
   higher-order behavior may apply the layer and may delegate the resulting
   behavior, but it may not inspect, remove, or restore a field inside another
   behavior's event or send product.
2. Fixed topology, restart eligibility, restart budget, optional restart
   delay, timer generations, replacement retention, and shutdown cancellation
   are one state-transition law owned by `FixedFleetOwnership`. Delayed restart
   is configuration of that law, not a wrapper around arbitrary send shapes.
3. A private parent/child communication is an explicit `Actions` effect.
   `ChildInput` targets one declared child occurrence; `ReportToParent` travels
   one established parent edge and the interpreter attaches the exact local
   child nonce. Neither operation fabricates a logical recipient.
4. A report crossing two parent edges is relayed by one stateless behavior
   transformation. The relay owns exactly one input-to-effect law and neither
   interprets nor deduplicates the report.
5. A pool worker receives only assignment data and reports only a completion.
   It does not name the pool's public protocol, customer reply route, pool
   behavior, stable-proxy behavior, or a logical recipient at the pool address.
6. Logical hosting requirements are derived from real route selections and
   real composed templates. An application-authored dummy product is not
   evidence that a catalogue template is hostable.
7. User-level composition tests observe complete interpreted traces. They may
   not manually transfer an effect into the next fold or assert structural
   `.owned`/`.inner` paths as proof of composability.

### Deletion ledger

- Delete `SupervisionOwnership`, `BackoffSupervision`, `BackoffSends`, and the
  application/standalone duplicate backoff surfaces.
- Delete `PoolAssignmentProtocol` and remove `WorkerPoolProtocol` /
  `KeyedWorkerPoolProtocol` from worker assignment and completion types.
- Delete Bombay's template-specific `ParentReporting` methods after the one
  generic parent-report interpreter is installed.
- Delete metadata-only logical-host tests and every documentation claim they
  were used to justify.
- Delete public aliases and `With...` names that only expose a concrete generic
  nesting and own no law.

### Initial containment

```text
exact blocker: delayed restart and pool completion currently depend on concrete
               supervision/pool product layouts
smallest regressions: one standalone delayed restart trace; one worker that
                      completes without naming a pool protocol
expected production files: Behavior Core effects/creation; shared supervision
                           ownership/protocol; proxy relay; FIFO/keyed pool;
                           Bombay report/action interpretation
expected production delta: net-negative after specialized paths are removed
new public semantic types: at most one restart-configuration sum, one generic
                           relayed report, and one pool completion value
```

### Stored-layer removal checkpoint

Exact blocker: `PoolWorkerLayer<L, R>` stores an existing layer only so
`FixedFleetOwnership` can apply it while staging initial stable children. It
owns no state, event, effect transformation, or policy. The ownership fold
does not otherwise use the layer: worker replacement is an explicit
`ChildInput<ReplacementRequested<C>>` to the already-established stable child.

Invariant: fixed-fleet ownership owns lifecycle state and names the stable
child type in its effects, while the containing behavior owns construction and
supplies a borrowed `BehaviorLayer` only when initialization stages those
children. Pool initialization may compose its user layer with the lawful
`RelayChildReports` transformation in a local inferred closure; no stored
adapter type is required.

```text
smallest regression: FIFO and keyed pools stage RelayChildReports<L::Output,
                     C, PoolCompletion<R>> without PoolWorkerLayer
expected production files: ownership.rs, fixed_supervisor.rs,
                           adapter/supervisor.rs, pool.rs
expected production delta: net-negative
public API: +0 types / -0 types (PoolWorkerLayer is private and deleted)
reused laws: BehaviorLayer, RelayChildReports, FixedFleetOwnership,
             ChildInput<ReplacementRequested<C>>
```

## Scope and decision rule

This audit covers the complete `bombay-behavior-actors` catalogue as it exists,
not one commit or one pair of reported gaps. Git history is used only to identify
when a public name or implementation shape appeared.

A reusable actor template is retained only when its own fold implements a
distinct state-transition law or a distinct typed event/effect transformation.
Sharing data structures or similar branches is not evidence that two actors are
one template. Conversely, a constructor, alias, policy marker, or wrapper is not
a template merely because it gives a long concrete composition a shorter name.

The governing semantic classification is Bombay policy. The actor-model laws
remain serialized turns, finite acquaintance, fresh creation, and explicit
communications/creations/next behavior. This audit does not change those laws.
It changes how the library packages derived constructions around them.

## Pre-edit change ledger

Exact blocker: the existing H37 audit treats “uses one shared implementation”
as the success criterion. That criterion caused public policy markers, aliases,
builders, and wrappers to be counted as actor templates even when they only
select or nest existing folds. The smallest end-to-end regressions are the
existing pure-fold constructions that already exercise `StopOnShutdown`,
`FinalizeOnShutdown`, fixed supervision, delayed supervision, both pool laws,
both shutdown-coordinator products, and `Configuration<FeatureSet<_>>` without
requiring a second semantic template. Those constructions will remain and the
bespoke names will be removed around them.

Expected surface:

- 26 files directly name the redundant public surface; separating the pool,
  watch/monitor, and shutdown folds and updating exhaustive callers is expected
  to take the cumulative change above 15 files.
- Production delta is expected to be net-negative, primarily from deleting the
  child-shutdown planner, policy-parameterized fixed-supervisor forms, recipe
  forwarders, and name-only lifecycle/feature specializations.
- No new public type is expected. At least 20 public types/aliases/traits and
  seven forwarding functions are expected to be removed.
- The retained folds are `StopOnShutdown`, `FinalizeOnShutdown`, `Watch`,
  `TerminationMonitor`, `Supervisor`, `BackoffSupervisor`, `WorkerPool`,
  `KeyedWorkerPool`, `ShutdownCoordinator`, and
  `HeterogeneousShutdownCoordinator`. Their existing typed products and
  interpreter requests are reused.

## Universal layer composition redesign: stage 1 pre-edit ledger

The completed audit above proved individual template laws, but it did not
provide one static construction contract by which a generic consumer can apply
an arbitrary concrete behavior transformation and name its associated output.
Callers can manually nest `Stash<B>`, `Watch<B>`, timing, shutdown, supervision,
and other transformations, but frameworks must reproduce each constructor and
spell or independently project the resulting concrete type.  The same omission
encourages higher-order templates to integrate lower-order policy instead of
accepting an already-composed behavior.

The exact stage-1 blocker is therefore catalogue-wide, not a priority-queue or
proxy special case.  The smallest compile regression is a generic function that
accepts any `B: Behavior` and any static layer `L`, applies `L` once, and returns
the layer's associated concrete behavior without naming that output.  Black-box
witnesses must use unrelated real catalogue combinations, including routing
inside supervision, persistence under timing/lifecycle transformations, and
different wrapper orders.  Tests must assert initialization, event ingress,
all send and creation lanes, errors, phases, and next-behavior decisions—not
just successful construction.

This is a deliberate Bombay construction law, not an actor-model guarantee:

- a layer is a statically dispatched value-to-value construction from one
  concrete `Behavior` to another concrete `Behavior`;
- the associated output retains its complete protocol, event, sends, birth,
  initialization, error, phase, and next-behavior algebra through ordinary
  `Behavior` associated types;
- a layer performs no actor effect; only the resulting behavior's pure fold
  returns `Actions`;
- inline layers retain the current actor namespace, while separately running
  behaviors still connect through transferable logical or established
  capabilities; and
- `ChildRoute<C, O>` remains usable only by the topology owner whose direct
  birth algebra proves that occurrence.

Expected stage-1 surface:

- at most eight files: this ledger, the minimal core construction contract and
  export, one generic-consumer suite, and directly affected documentation or
  compile tests;
- production: at most `+80 / -0 / net +80`; this stage establishes the missing
  generic contract, while later template deletion must make the cumulative
  redesign production-negative;
- tests: compile and pure-fold witnesses across at least three unrelated
  catalogue families and more than one layer order;
- public API: one construction trait, no wrapper, builder, marker, effect
  product, dynamic dispatch, or new effect algebra.

The implementation must reuse the existing concrete wrapper types,
`EventLayer`, `SendLayer`, `Actions`, `Behavior::Birth`, and their structural
ingress proofs.  A closure or concrete constructor may implement the common
contract, so catalogue templates do not require a parallel family of `*Layer`
configuration wrappers merely for inference.  Before the later audit crosses
15 cumulative files, 500 net new production lines, or three new public types,
its measured ledger must be reported at the mandatory checkpoint.

The higher-order state map driving the later deletion audit is:

| Owner | Irreducible correlation retained | Lower-order law currently duplicated or coupled |
|---|---|---|
| `Proxy` | one fresh worker-incarnation installation/replacement relationship | customer return route is fixed to logical delivery |
| `Supervisor` | fixed topology, restart provenance, and worker/proxy fact correlation | fleet membership and terminal drain overlap registry/shutdown policies |
| `DynamicSupervisor` | two-producer initial-install join and replacement correlation | mutable membership, customer correlation, and drain are integrated |
| delayed supervisors | timer generation to accepted replacement-batch correlation | delay progression is already a lower `Backoff` law |
| `WorkerPool` | assignment identity joined with worker return/stop facts | FIFO backlog, availability routing, fixed supervision, and drain are integrated |
| `KeyedWorkerPool` | persistent key-to-slot assignment and rebalance correlation | inherits the same FIFO, routing, supervision, and drain machinery |
| shutdown coordinators | active phase to exact outstanding child-stop facts | plan declaration/building is separate from phase execution |

This table is a falsifiable work list, not a claim that every apparent overlap
can be split without changing asynchronous semantics.  Each deletion requires
a real composition test that preserves ordering, ownership, and every effect
lane first.

### Universal layer stage-1 checkpoint

The generic construction regression failed on the baseline because no layer
contract or associated output existed. `BehaviorLayer<B>` now provides that
static associated output, closures implement it without allocation or erasure,
and `Behavior::layer` supports inferred chaining. Real routing/supervision and
persistence/stash/shutdown compositions pass in debug and optimized builds
while retaining complete creation, observation, domain-send, and stop lanes.

```text
production: +111 / -2 / net +109
tests:      +90 / -0 / net +90
docs:       +74 / -0 / net +74
public API: +1 types / -0 types
```

The production delta exceeds the pre-edit estimate by 29 lines because the
single trait's rustdoc states the complete semantic boundary and includes a
compile-checked generic-consumer example. It remains below every mandatory
stop threshold. This is new generic capability code, not consolidation or code
reduction; deletion work is a later independently measured stage.

### Owner-scoped delivery checkpoint

`DeliveryRouteFor<Owner>` is the second static composition law. It selects the
existing logical, established, or child delivery product without erasure.
`Recipient` and `EstablishedRecipient` require the same address namespace as
the owner; `ChildRoute` additionally requires
`Owner: ResolveChildOccurrence<Occurrence, Child = C>`. A compile-fail test
proves that `NoBirths` cannot claim a foreign child route. `Proxy` now uses this
contract for its real worker-forwarding leg, so the trait is exercised by a
production topology owner rather than existing only as test syntax.

The cumulative stage ledger is:

```text
production: +245 / -8 / net +237
tests:      +180 / -0 / net +180
docs:       +96 / -0 / net +96
public API: +2 types / -0 types
changed files: 9
```

Focused layer and route regressions pass in debug and optimized builds;
Behavior and Actors rustdoc/compile-fail tests pass; the workspace all-targets
check is warning-clean. This remains capability work, not code reduction.

The next conversion applies the owner-scoped route contract to all
caller-supplied delivery destinations (buffer, priority, ordering, sequencing,
deduplication, rate limiting, and their shared named product) and updates their
independent models. Together with affected public compile matrices this will
cross the 15-file mandatory checkpoint. Production editing for that expansion
must not start until the expanded surface is explicitly authorized.

### Catalogue-wide composition axes

Queue and proxy are witnesses, not privileged composition categories. The
catalogue has two orthogonal composition axes, and every retained template must
be classified on both:

| Axis | Static contract | Applicability |
|---|---|---|
| same actor, inline | `BehaviorLayer<B, Output = ...>` | every transformation that consumes one `B` and returns another concrete `Behavior` |
| separate actor, transferable | `DeliveryRoute` | logical and established recipients passed between actors; the route projects its protocol and send product |
| topology-owner local | `DeliveryRouteFor<Owner>` | the same transferable routes plus a direct `ChildRoute` proven by `Owner::Birth` |
| runtime hosting projection | not currently provided | owner-authored repetition was rejected because it cannot prove transitive completeness |

The direct-child case deliberately does not make `ChildRoute` transferable. A
separate queue, router, workflow, or cache actor cannot send through another
actor's child namespace. It connects to that topology through a logical or
established proxy capability; the proxy then emits the proven child delivery.
This is the same architecture for every catalogue family, not a proxy-specific
exception.

The current endpoint audit partitions the full catalogue as follows:

- transparent transformations (`Stash`, shutdown, timing, watching,
  termination propagation, supervision adapters, and shutdown coordination)
  must preserve every inner event, send, birth, phase, error, initialization,
  and next-behavior lane while adding only their owned law;
- ordinary request/reply cores already parameterize customer return routes
  through `DeliveryRoute`, including persistence, workflow, operational,
  correlation, acknowledgement, presence, and admission-result actors;
- payload-forwarding cores (`Buffer`, `PriorityQueue`, `OrderGate`,
  `Sequencer`, `Deduplicator`, and `RateLimiter`) use the same transferable
  route projection for their destination leg rather than six adapters;
- membership cores (`Router`, `WorkQueue`, `Topic`, and `PubSub`) retain the
  concrete route capability in their ordered state, preserving equality and
  removal evidence without assuming logical identity. `Registry`
  and `Resolver` intentionally model logical-name bindings and therefore remain
  logical rather than pretending to be universal endpoint stores;
- lifecycle owners already use dedicated exact or direct-child capabilities;
  their remaining gap is truthful availability evidence at the network
  boundary, not another routing implementation; and
- pure state cores with no destination leg (`Machine`, lease state, barriers,
  latches, and similar folds) compose through layers and typed protocol hops;
  they need no invented route parameter.

This classification prevents two opposite errors: forcing every template into
one recipient representation, and leaving six or more bespoke destination
implementations merely because their state laws differ. Route construction is
shared; the queueing, ordering, membership, lifecycle, and workflow folds stay
separate exactly when they own different transitions.

The expanded black-box layer suite now covers routing, persistence, stashing,
both shutdown transformations, absolute and relative timing, periodic and
activity-driven timing, logical observation, terminal monitoring, and terminal
propagation. It asserts initialization and transition lanes, including a
finalization fold whose delivery must survive the outer stop decision. The
current cumulative ledger, including untracked tests, is:

```text
production: +193 / -9 / net +184
tests:      +294 / -0 / net +294
docs:       +238 / -0 / net +238
public API: +2 types / -0 types
changed files: 9
```

This remains below the current thresholds. The next production conversion is
catalogue-wide by design and will exceed 15 files once independent models,
compile checks, and documentation are included; it remains paused at the
required explicit expansion checkpoint.

### Universal route projection: expanded pre-edit ledger

The route audit found a more fundamental duplication than the six destination
fields. The former `DeliveryRoute<P>` and `DeliveryRouteProtocol` expressed one
law twice: the former repeated a protocol parameter that the latter already
projected as an associated type. Thirty production files mentioned one of
these contracts, and 26 repeated `DeliveryRoute<P>` bounds. Adding another route
parameter only to the six forwarding actors would preserve that catalogue-wide
duplication and make their concrete types longer.

The next stage therefore converges on one transferable route contract with an
associated `Protocol` and `Sends`, keeps `DeliveryRouteFor<Owner>` as only the
additional direct-child ownership proof, and removes
`DeliveryRouteProtocol`. Existing reply-route users migrate mechanically to
the single projection. Payload-forwarding and truthful membership actors then
use the same route law for their destination/member leg. No `*Route` wrapper,
layer configuration type, adapter actor, effect algebra, registry, or dynamic
dispatch is added.

Expanded expected surface:

- 30 existing production files for the contract and every current route-bound
  catalogue actor, plus the six destination folds and any membership fold whose
  independent identity test proves logical-only storage is not its law;
- approximately 10–15 existing model, property, compile, and black-box test
  files, with both logical and established traces and negative direct-child
  ownership cases;
- production target `+250 / -300 / net -50` or smaller; the stage must stop
  independently if net-new production approaches 500 lines;
- public API `+0 types / -1 types` by deleting `DeliveryRouteProtocol`; no new
  wrapper, alias, builder, marker, or effect product; and
- existing `Delivery`, `EstablishedDelivery`, `ChildDelivery`, `SendEffects`,
  `DeliveryOutcomes`, and named actor products are reused rather than
  reimplemented.

Because this intentionally crosses the 15-file threshold, no production edit
for this stage is permitted without explicit expanded-surface authorization.

### Universal route projection: implementation result

The authorized conversion now has one transferable `DeliveryRoute` contract
with associated `Protocol` and `Sends`. Logical and established capabilities
implement it directly. `DeliveryRouteFor<Owner>` adds only the separate proof
needed for an owner-local `ChildRoute`; it does not make that route
transferable. `Proxy` exercises the owner-local law for its real worker child.

Every payload-forwarding and membership core listed above now stores or accepts
the concrete route capability and accumulates its concrete send product. A
black-box matrix runs all ten actors with established endpoints and checks the
actual endpoint, payload, outcome, creation lane, and continuation decision.
The logical model/property suites remain unchanged in vocabulary and therefore
continue to test the state law independently of the new route implementation.

The associated protocol also made separate `Reply`/`D` parameters redundant.
They were removed from acknowledgement, circuit-breaker, configuration,
correlation, health, lease, presence, readiness, registry, resolver, task,
workflow, message-adapter, FIFO-pool, and keyed-pool signatures. Four private
phantom aliases were inlined. This is deletion rather than another wrapper or
compatibility layer.

The higher-order audit does not justify replacing supervisor or pool
correlation with a queue implementation. `Proxy` owns the unique join between
fresh incarnation creation, stop, replacement, shutdown, and typed command
return. Fixed and dynamic supervisors own provenance-sensitive proxy/worker
fact joins. Pools additionally join assignment identity, completion, returned
commands, worker stops, replacement availability, and terminal drain. A
separate queue or router can be their child, peer, or supervised payload through
ordinary layers and routes, but substituting its fold would split those atomic
joins across mailboxes and change the law. The earlier selector-only supervised
wrappers were deleted; the retained higher-order actors contain only these
correlations plus explicit effect products.

Current cumulative checkpoint, including untracked tests:

```text
production: +1160 / -1058 / net +102
tests:      +833 / -195 / net +638
docs:       +290 / -12 / net +278
public API: +2 types / -1 type
```

## Complete catalogue classification

“Core” means a standalone actor fold. “Transformation” means a wrapper that
owns a distinct typed event/effect law. “Composition” means the public use case
must be written by nesting or connecting retained concrete actors. A truthful
structural alias such as a direct `Here` form may remain when it names the same
template rather than advertising another actor law.

| Catalogue surface | Owned transition law | Verdict |
|---|---|---|
| `Machine` | finite-state receive/become transition and staged drain atomicity | retain core |
| `MessageAdapterWithRoute` | one pure protocol map followed by one typed delivery | retain core |
| `Stash` | bounded hold/replay order over an inner fold | retain transformation |
| `StopOnShutdown` | direct shutdown-to-stop event transformation | retain transformation |
| `FinalizeOnShutdown` | shutdown reaction returning complete inner `Actions` | retain transformation |
| `Guardian`, `CoordinatedGuardian` | no state beyond the two shutdown transformations above | delete; compose the retained transformations directly |
| `Watch` | recurring logical-name observation across later incarnations | retain as its own concrete transformation |
| `TerminationMonitor`, exact monitor form | one correlated observation lifecycle with terminal consumption | retain one monitor implementation with typed target form |
| `EstablishedWatch` and watch target policy aliases | restrict or parameterize the monitor reaction without another lifecycle law | delete; use the exact monitor and an ordinary reaction |
| `PropagateTermination` | one correlated source fact and explicit propagation disposition | retain transformation |
| `Task` | pending-to-completed/failed one-result terminal lifecycle | retain core |
| `ShutdownCoordinator` | ordered homogeneous phase shutdown | retain transformation |
| `HeterogeneousShutdownCoordinator` | ordered phase shutdown over a closed heterogeneous effect product | retain separately; its typed selection/effect transition is distinct |
| `ChildShutdownPlan`, its typestate builder, and `shutdown_after_children` | joins committed direct-child creations, rejects mismatched facts, and reports one typed heterogeneous plan | retain transformation; generic consumers use associated-output operation traits instead of copying its hidden proof vocabulary |
| `Proxy` | stable slot and fresh-incarnation replacement lifecycle | retain core |
| `Supervise` | adopt application creations into fixed proxy ownership while preserving inner actions | retain transformation |
| `Supervisor` | standalone fixed proxy-fleet ownership | retain core, with a concrete implementation rather than a command-policy engine |
| `SupervisedWorkers` and selector policy types | parameterize fixed ownership with one routing selector | delete; route commands with an ordinary typed routing actor/application behavior |
| delayed `Supervise` policy | delays accepted replacement effects from a supervised application | retain as an explicit timing policy of the same transformation |
| delayed `Supervisor` policy | delays accepted replacement effects from standalone fixed ownership | retain as an explicit timing policy of the same core |
| `FixedBackoff`, `BackoffWorkers` | implementation/policy parameterizations of the two retained delayed folds | delete |
| `DynamicSupervisor` | changing stable-child membership and replacement lifecycle | retain core |
| `WorkerPool` | bounded FIFO admission, assignment, completion, and interruption | retain core and its direct `Behavior` fold |
| `KeyedWorkerPool` | the FIFO law plus persistent key-to-slot affinity and rebalance transitions | retain separately and restore its direct `Behavior` fold |
| `Deadline` | absolute one-shot schedule and single matching-generation reaction | retain transformation |
| `OneShot` | relative one-shot schedule and single matching-generation reaction | retain transformation |
| `Periodic` | matching-generation reaction followed by explicit rearm | retain transformation |
| `ReceiveTimeout` | activity-driven generation reset and one notification per idle period | retain transformation |
| `Lease` | exclusive ownership, generation-safe expiry, release, and rejection | retain core |
| `Router` with its strategies | membership plus strategy-specific selection/evidence transitions | retain core; strategies are values of this law, not actors |
| `WorkQueue` | bounded FIFO work plus worker-availability transitions | retain core |
| `Buffer` | bounded FIFO buffering and overflow ownership policy | retain core |
| `PriorityQueue` | stable immutable-priority admission/release | retain core |
| `OrderGate` | monotonic keyed release | retain core |
| `Sequencer` | sequence-gap buffering and ordered release | retain core |
| `Deduplicator` | bounded first-seen admission | retain core |
| `RateLimiter` | explicit token consumption/refill | retain core |
| `CircuitBreaker` | closed/open/probing single-flight admission | retain core |
| `Correlator` | keyed request/result ownership | retain core |
| `Acknowledgements` | multi-participant acknowledgement lifecycle | retain core |
| `Registry` | mutable typed binding ownership and lookup | retain core |
| `Resolver` | immutable definition and read-only resolution capability | retain core; mutation is unrepresentable in its protocol |
| `Topic` | one ordered subscription set and snapshot publication | retain core |
| `PubSub` | keyed topic introduction, known-empty retention, and per-topic membership | retain core; these keyed states are not actor nesting hidden in a wrapper |
| `Presence` | versioned presence evidence and generation-safe expiry | retain core |
| `Configuration` | versioned atomic configuration acceptance/query | retain core |
| `FeatureSet` | a domain product invariant, not an actor | retain product |
| `Features`, `FeaturesState` | name-only aliases of `Configuration<FeatureSet<_>>` | delete; write that concrete composition |
| `Health` | versioned component evidence and aggregate health | retain core |
| `Readiness` | fixed dependency evidence and aggregate readiness | retain core |
| `Cache` | bounded deterministic LRU ownership | retain core |
| `Latch` | one-generation countdown and single release | retain core |
| `Barrier` | cyclic fixed-membership generations | retain core |
| `Workflow` | dependency activation and terminal run lifecycle | retain core |
| composition recipe functions | forward to constructors while inferring structural parameters | delete; construct and nest the concrete types directly |

## Composition law for the removals

The replacements must preserve complete `Actions`; they may not intercept,
drop, duplicate, reorder, or reinterpret an effect lane. Actor-to-actor
composition uses ordinary typed recipients and sends. Wrapper composition uses
the retained concrete event layers and named send products. A topology owner,
not a generic planner wrapper, remains responsible for correlating its own
creation results and reporting any creation-dependent shutdown plan. These are
derived Bombay constructions and policy choices, not additional Agha laws.

## Implementation checkpoint

The cumulative catalogue rewrite remains production-negative even though the
retained pool and shutdown actors now expose their own folds directly. These
figures include all tracked and untracked files; documentation is reported
separately:

```text
production: +1204 / -2117 / net -913
tests:      +1366 / -581 / net +785
docs:       +331 / -175 / net +156
public API: +5 types / -19 types
```

Seven forwarding functions are also removed. The three apparent public type
additions in the textual diff replace existing aliases with concrete structs
of the same name (`Watch` and `Supervisor`); they add no public type name.
Delayed restart is one policy of `Supervisor`, not another public wrapper. The five additions
were `ProxyUnavailable`, `CommandSupervisionEvent`, `DeclareShutdownPhase`,
`FinishShutdownPhases`, and `LogicalHostRequirements`; the last was subsequently
deleted after the end-to-end audit proved it was assertion-only metadata. Eight constructors or
methods were also removed, for 34 removed public names in total. No public
wrapper or policy-marker type was added.

The later semantic regressions retain this classification while adding no
actor wrapper. Dynamic supervision now joins its two initial creation facts in
either order. Proxy commands report complete unavailability through their
established parent relationship, and pools join that report with
worker-stop/replacement facts.
`DeclareShutdownPhase` and `FinishShutdownPhases` expose the retained builder to
generic consumers through associated outputs. The former logical-host trait did
not derive anything from the composed behavior and is not retained as evidence.

## Composition-hierarchy tightening

The executable `Router → Supervisor-owned Proxy → PriorityQueue → Target`
hierarchy exposed one remaining false coupling. `Router` required every
destination message to implement `Clone` because its policy family also
contained `Broadcast`. Consequently a round-robin router could not carry
`ProxyCommand`: its `Replace` alternative truthfully owns a behavior and is not
cloneable.

This was not a proxy or priority-queue defect. The router combined two distinct
laws. Bombay now defines `Router` as single-recipient ownership transfer;
`RoundRobin`, `LeastLoaded`, `ConsistentHash`, and `RendezvousHash` each return
at most one membership index. `Topic` and `PubSub` retain snapshot fan-out and
its explicit payload-cloning bound. The `Broadcast` policy and the router's
multi-index/clone path are deleted. No replacement type or compatibility layer
was added.

The black-box regression constructs the real supervisor-staged proxy and the
real proxy-staged priority queue, commits the queue creation, then passes offer
and release commands through router delivery, proxy child delivery, and queue
target/outcome delivery. It asserts every creation, observation, rejection,
delivery, and become lane at each boundary. The public composition guide now
shows both same-mailbox layers and independent actor topology, including this
executable hierarchy.

This tightening stage changes no foundational effect algebra and adds no
public type:

```text
production: +48 / -80 / net -32
tests:      +252 / -11 / net +241
docs:       +164 / -5 / net +159
public API: +0 types / -1 type
```

## Alias-free application birth composition: pre-edit ledger

The downstream Bombay audit found one remaining static-composition blocker.
Bombay currently requires application-provisioned actors to occupy slots in the
domain root's own `Behavior::Birth` algebra.  That makes an item-level root
declaration name every fully composed child type before value inference can
apply, producing mechanical aliases such as
`ManagedHealth = StopOnShutdown<Health<...>>`.

The actor-model law remains fresh creation.  Combining two already-declared
creation capabilities is a derived Bombay construction: it must preserve every
creator-local nonce, `CreationKind`, child value, creation-vector position, and
pre-existing child occurrence.  Application policy—what is provisioned, nonce
selection, and initialization order—remains downstream in Bombay.

The smallest end-to-end regression is an inferred application value that adds
two differently composed children to a root which already stages its own
child.  The root creation must remain first, application creations must follow
in declaration order, and the root's existing nominal child route must still
resolve at its original position.  No alias may name either application child
or the resulting application type.

Expected stage surface:

```text
changed files: 6-8
production:    approximately +150 / -10
tests:         approximately +200 / -0
docs:          approximately +100 / -0
public API:    +1 trait / -0 types
```

The implementation must reuse `Actions`, `Create`, `BirthMode`, `ChildChoice`,
`ChildProduct`, and `BirthNodeAt`.  It must not add a behavior wrapper, layer
marker, alias generator, erased child value, runtime registry, or arbitrary
creation callback capable of rewriting provenance.

The initial black-box regression failed before the production edit because
`BirthNodeAppend` did not exist.  With the static append law present, the same
test proves an existing root child role across a lifecycle wrapper and the
application composition, while two application child expressions and the
complete application type remain inferred.  Deterministic tests cover empty
left/right identity, associativity, repeated child types at every occurrence,
and exact vector order.  A 256-case property test preserves every generated
nonce, birth/replacement provenance value, lane, and position.

Final stage ledger:

```text
changed files: 7
production:    +170 / -22 / net +148
tests:         +402 / -0 / net +402
docs:          +171 / -0 / net +171
public API:    +1 trait / -0 types
```

No behavior wrapper, builder, marker, alias, registry, or erased value was
added.  The one public trait is the closed child-algebra operation needed by a
generic composition owner.  The private non-empty-node proof only closes its
recursive implementation and adds no public name.

## Verification

- `cargo check --workspace --all-targets`: pass without Rust warnings.
- Focused birth-composition regressions: 5 passed in debug and optimized
  builds; the property test ran 256 cases in each build.
- `cargo nextest run --workspace`: 527 passed, none skipped.
- Actor rustdoc and compile-fail tests: 29 passed.
- `supervision_sequences`, `pool_sequences`, and
  `catalogue_sequences`: 5,000 fuzz executions each under the locked Fenix
  input's nightly sanitizer toolchain.
- `nix flake check`: passes all seven checks, including build, optimized
  nextest, docs, Rust/TOML formatting, dependency audit, and dependency policy.

## Layer-owned parent reporting: pre-edit ledger

> **Rejected experiment.** The implementation following this ledger added a
> mandatory `on_unavailable` callback to `Supervise::new`. Forty-three test and
> fuzz call sites could satisfy it only with `|_, _| Ok(Actions::cont())`.
> That is caller migration driven by the new signature, not evidence of an
> application-owned transition law. The experiment is quarantined and must be
> removed; none of its passing compilations count as verification.

The remaining blocker is not construction inference: `BehaviorLayer` already
constructs every concrete output without naming it.  The blocker is that the
first parent-report redesign made an application event implement
`EventIngress` merely so `Supervise` could hand an unavailable command back to
the application.  That exposes interpreter/template routing as application
composition work and creates a second user-facing mechanism beside
`BehaviorLayer`.

The smallest regression composes an application with `Supervise` and an outer
shutdown layer, without naming the output, an ingress path, or a composed event
type.  A proxy-unavailability fact must invoke one explicit application policy,
preserve the complete command, and return the policy's complete `Actions` for
ordinary supervision wrapping.  The application event algebra must contain
only its domain events and must not implement an interpreter-routing trait.

This is a derived Bombay policy: a fixed supervision layer owns proxy reports;
the application supplies the pure transition selected for expected command
unavailability.  Source-indexed event construction remains an
interpreter/template proof used to deliver child facts to the owning layer.  It
does not become an application protocol, another actor effect, or an alternate
behavior composition API.

Expected stage surface:

```text
changed files: the supervisor fold, focused layer regression, and direct callers
production:    approximately +20 / -10
tests:         migration-heavy but no new protocol or ingress fixtures
public API:    +0 types / -0 types
```

The stage reuses `BehaviorLayer`, `SupervisionEvent`, `ProxyUnavailable`,
`BehaviorActed`, and the existing supervisor action wrapping.  It adds no
wrapper, builder, marker, alias, event variant, effect lane, registry, or erased
callback.  The unavailability policy is a monomorphized function pointer over
the already-concrete application and command types.

## Compiler-origin provenance audit

The rejected callback exposed a broader process failure. The current working
tree is therefore audited by design provenance before any further compilation
or caller migration. Compiler success is not evidence for a public algebra.

The quarantined cluster is:

```text
pathless proxy report
    -> source-indexed EventIngress construction
    -> SupervisionEvent command-unavailability lane
    -> mandatory application callback
    -> 43 no-op test/fuzz policies
```

Only the final two links are already disproven. `EventIngress` and pathless
parent reports are not accepted or rejected by association; each must still
prove a general interpreter/event-composition law through ordinary
`BehaviorLayer` construction, two unrelated templates, two wrapper orders,
and a complete end-to-end interpreter trace. If that proof needs an
application event variant, explicit wrapper-depth path, supervision-specific
handler, or ignored transition, the whole route is removed.

`RequestProxyWorker` is audited separately. Its candidate law is exact
delivery of an owner-created control input to a child occurrence while keeping
the child's public domain protocol unchanged. It remains only if a pre-edit
regression proves that law without logical hosting, a second proxy protocol,
runtime lookup, or application aliases. Downstream compiler demand is not
provenance.

No production edit follows from this audit section. The next design stage must
first write the desired alias-free `BehaviorLayer` syntax and complete
availability trace as a failing regression. Expected correction is
production-negative because the mandatory callback field, constructor
argument, transition branch, generic event parameters used only by it, and all
placeholder policies are deletion candidates. No new public type is authorized
by this audit.

### General typed-reaction layer: pre-edit ledger

> **Rejected as the availability boundary.** The focused composition worked,
> but the first catalogue check showed that it would force every
> lifecycle-only `Supervise<B, C>` to add a domain-recovery event even when the
> composition exposes no domain route. Migrating those callers would reproduce
> the mandatory-callback defect as a mandatory wrapper. No `React` production
> type or migration is retained. The evidence instead locates the missing sum
> at the proxy boundary: lifecycle ownership and public domain forwarding are
> currently fused and must become separately composable capabilities.

The user-level syntax under test is:

```rust,ignore
let behavior = application
    .layer(|inner| React::new(inner, retain_unavailable))
    .layer(|inner| Supervise::new(inner, topology, restart).unwrap())
    .layer(StopOnShutdown::new);
```

The resulting type is inferred. `React` is not a supervision policy: it owns
the general same-mailbox law “one additional typed input invokes one pure,
infallible reaction and preserves its complete `Actions`; every inner event is
delegated unchanged.” Its concrete event sum selects that owned input while
retaining the complete inner event algebra. The reaction is infallible because
it receives mutable access to the inner behavior; a fallible reaction could
mutate and then reject the same event, violating transition atomicity.

For supervised availability, the source is a proxy child report and the input
is the complete `ProxyUnavailable`. `Supervise` owns only lifecycle
correlation: it forwards that input to the already-composed reaction instead
of storing an application callback. Startup, replacement, restart exhaustion,
and shutdown remain values of the same input law.

Acceptance requires all of the following before catalogue migration:

- a focused test fails on the prior design because `React` does not exist;
- the complete outer `StopOnShutdown<Supervise<React<...>>>` fold preserves
  every send, creation, and become lane while recording each unavailable
  command exactly once;
- an unrelated typed input and the reverse relevant wrapper order use the same
  mechanism without supervision-specific bounds or aliases;
- the mandatory `Supervise` callback, its field and transition branch, and all
  43 placeholder policies are deleted; and
- at least one existing one-off event-only transformation is deleted or
  expressed directly by this general law, so the abstraction reduces rather
  than relocates the catalogue surface.

Expected surface for the design stage:

```text
production files: 4-7
production delta: at most +180 / at least -100
focused tests:    2 files before any catalogue migration
public API:       +2 general types / -1 or more one-off types
```

The later caller migration will exceed 15 files and is separately authorized
by the resumed holistic audit. It may add no further semantic surface.

### Typed unavailable route: rejected replacement

> **Rejected.** Adding `UnavailableRoute` to `Supervise` merely moved the
> callback obligation into a public generic and then propagated that generic
> through delayed supervision. It did not remove a repeated behavior law, and
> one unrelated outer layer would still force caller edits. The focused route
> regression and all production edits from this experiment were removed. None
> of its compiler results count as design evidence.

The reaction experiment proved that application recovery cannot be a required
inner transition of every lifecycle composition. The command is instead
returned through an explicit customer capability already modeled by
`DeliveryRoute`. `Supervise` stores that concrete route and emits its concrete
send product; it never invokes an application callback and never asks the
runtime to host a fabricated protocol at the supervising actor's own address.

The complete same-mailbox product is structural rather than bespoke:

```text
Event = SupervisionEvent<
            EventLayer<ProxyUnavailable<A, C::Msg>, B::Event>
        >

Sends = SendLayer<
            SupervisorSends<A, C>,
            SendLayer<UnavailableRoute::Sends, B::Sends>
        >
```

Lifecycle facts remain owned by `SupervisionEvent`. One unavailable command is
owned by the existing `EventLayer`; `Supervise` converts it to exactly one
delivery through the supplied route. Application events and actions remain
the innermost lane. The application behavior is not mutated, invoked, or
failed by expected unavailability.

Pools and dynamic supervision keep their stronger owner-specific laws. Their
proxy reports already return to the owning parent through the typed report
path; the pool joins the returned assignment with worker-stop/replacement
facts, while dynamic supervision selects the per-child route retained from
`Start`. Neither needs a second logical host at its own address.

The focused regression must fail on the callback design, then prove all four
unavailable phases through `StopOnShutdown<Supervise<...>>`. It asserts the
exact destination, complete returned command, every empty lifecycle and
application lane, creations, next decision, and that the application fold was
not invoked. A separate routing hierarchy must exercise unavailability before
the old manually-fed `Router → Proxy` test can count as evidence.

Expected design-stage surface:

```text
production files: 3-6
production delta: net-negative or at most +80
public API:       +0 types; remove the callback and one event generic/variant
```

The later constructor migration supplies real typed routes. A placeholder
route, fabricated self-recipient, ignored delivery, or route used only to make
a test compile invalidates the design.

### Ownership inversion: current design checkpoint

The repeated failure is caused by choosing `Supervise` as the unit of design.
Current `BehaviorLayer` is a static construction and inference contract; it
does not turn a hard-coded higher-order fold into semantic composition.
`Supervise` still constructs `Proxy<C>` internally, so proxy forwarding and
availability necessarily leak into supervision, delayed supervision, pools,
tests, and downstream application types.

The behavior laws must instead be separated before another signature is
proposed:

| Law owner | State and correlation it may own | What it must not own |
|---|---|---|
| stable incarnation | one pending fresh installation, current incarnation, replacement provenance, and terminal child shutdown | customer recovery, restart selection, backlog, or application events |
| domain availability | exactly one selected law for every admitted command during installing, running, vacant/exhausted, and shutdown phases | worker creation or supervisor restart policy |
| supervision | proxy/worker lifecycle facts, restart eligibility/budget, and terminal fleet drain | the worker's domain message, customer route, queue, or persistence operation |
| delayed supervision | generation-safe delay of an already accepted replacement effect | a second supervisor state machine or availability policy |
| pool | the atomic join of job, assignment, completion/return, worker loss, and terminal interruption | a second implementation of proxy incarnation or restart timing |

The intended construction must name no resulting behavior type and must not
thread an availability type through supervision:

```rust,ignore
let stable_worker = worker
    .layer(availability(customer_policy))
    .layer(stable_incarnation());

let application = application.layer(supervise(
    topology([slot], |_| stable_worker),
    restart_policy,
));
```

The names above are domain-level acceptance syntax, not authorized Rust API.
An implementation is acceptable only if its concrete layers each own the law
listed in the table, the application and higher-order output types remain
inferred, and it deletes the mandatory callback, the unavailable-message
generic on supervision events, and the hard-coded construction of a forwarding
proxy inside supervision. A type added merely to realize this spelling is not
accepted.

Before production work resumes, the audit must select the one availability law
for the public stable destination and write its full transition table in
customer vocabulary. The same table must drive pure-fold tests for startup,
running delivery, replacement, restart exhaustion, shutdown, duplicates, and
stale lifecycle facts. Only then can the existing incarnation, routing,
buffering, acknowledgement, and event/effect products be evaluated as
lower-order composition candidates.

### Live claim-versus-implementation failures

The following earlier audit statements are currently false and must not be
used as completion evidence:

1. The catalogue says the standalone fixed `Supervisor` core is retained, but
   `fixed_supervisor.rs` is deleted and no `Supervisor` is exported. Only the
   application wrapper `Supervise<B, C>` remains. Fixed fleet ownership without
   an application fold is therefore no longer a public feature despite the
   table claiming otherwise.
2. The higher-order actors are not assembled from an already-composed stable
   child. `FixedFleetOwnership`, `Supervise`, `DynamicSupervisor`,
   `WorkerPool`, and `KeyedWorkerPool` all name `Proxy<C>` in their birth,
   child-route, observation, shutdown, or replacement effects and construct it
   with `Proxy::new` internally.
3. The documented `Router -> supervised Proxy -> PriorityQueue` witness is a
   sequence of manually invoked pure folds. Manual transfer can be useful
   unit evidence, but the test also supplies an ignored unavailable-command
   callback and exercises only the running phase. It does not prove that the
   complete outer interpreter stack delivers, returns, or recovers a command
   during startup, replacement, exhaustion, or shutdown.
4. The former `LogicalHostRequirements` was documentation-only metadata. No
   catalogue behavior implemented it, and its tests exercised only a locally
   authored dummy product. It has been deleted rather than advertised as a
   complete transitive requirement.
5. `BehaviorLayer` currently abstracts only value construction and associated
   output inference. It does not by itself compose state, events, actions,
   births, initialization ordering, or independently running actor
   configurations. Describing hard-coded higher-order actors as layer-composed
   because their constructors can appear inside a closure is incorrect.

These failures reopen the catalogue verdict. Production work remains frozen
until the replacement design shows both actor-configuration composition and
same-mailbox transformation separately, preserves the actual fixed-supervisor
feature, and replaces the hard-coded stable child in at least fixed
supervision, dynamic supervision, and one pool without propagating a new
caller-named type or placeholder policy.

### Layer-owned availability: pre-edit ledger

**Provisional design rejected after the catalogue check.** Requiring every
domain message to implement the recovery law makes a framework concern part of
the worker protocol and makes a lifecycle-only `Never` protocol invent an
impossible capability. The brief implementation was useful evidence, but is
not the general architecture and must not remain as the completion design.

The replacement law belongs to composition:

| Direction | Algebra | Owner |
|---|---|---|
| mailbox input | closed sum of the inner user event and each layer's private lifecycle facts | the concrete composed behavior |
| transition output | named product of inner/user sends and every layer's system sends | the concrete composed behavior |
| construction | reusable `BehaviorLayer` from a worker behavior to one stable-child behavior | the topology owner receives the layer value; callers do not name its output |
| unavailability | policy owned by the stable-incarnation layer | neither the raw domain message nor the supervisor |

System creation, observation, stop, replacement, and shutdown values remain
framework-owned typed lanes. Application authors do not define or manually
union system commands. A supervisor, dynamic supervisor, or pool must consume
the already-composed stable-child contract instead of naming `Proxy<C>` or
reimplementing its lanes. User commands need a customer route only when the
selected availability policy is an explicit typed return; buffering or
capability withholding must not be encoded as a fake route.

The first user-syntax regression is the existing complete hierarchy in
`behavior-testkit/tests/universal_layers.rs`. It now passes the stable-child
construction as `|worker| worker.layer(Proxy::new)` to fixed supervision and
names no output type. Against the current production surface it fails for two
independent, intended reasons:

1. `Supervise::new` accepts only the application, raw-worker topology, and
   restart configuration, so the stable-child layer is rejected as an extra
   argument and proxy construction remains hard-coded.
2. `PriorityQueueMessage` is rejected for not implementing the provisional
   `ReturnUnavailable` trait, proving that the provisional design leaks
   stable-incarnation policy into an otherwise complete domain protocol.

These are the two production seams the next stage must remove. Adding an
implementation of `ReturnUnavailable` to `PriorityQueueMessage`, deleting the
layer argument, or replacing it with a callback would game the regression.

The actor-model acquaintance law rules out reconstructing a customer from
`User::from`: an origin address is not proof that the origin accepts the
worker's command or rejection protocol. The actor model also does not select
Bombay's availability policy. A stable-incarnation layer must choose and expose
one truthful policy. An explicit customer-return policy needs a real typed
customer capability; a parent-recovery policy uses the already-established
child/parent relationship and a typed parent ingress; a buffering policy owns
its capacity and typed overflow result. None may invent a logical host from an
address or require an unrelated domain message to implement framework policy.

The concrete pool use proves the narrow owner contract needed now: when its
stable child cannot accept an assignment, the child returns the complete
assignment through the existing typed parent-report path, and the pool handles
that private input in its own event sum. The runtime already retains the
concrete child type and occurrence in `LocalParentReports`; `EventIngress`
selects the exact parent event lane without a caller-named wrapper path. The
same structural rule must work when the child is the output of a
`BehaviorLayer`, rather than being hard-coded as `Proxy<C>`.

The complete transition table is:

| Proxy phase when command is admitted | Observable result |
|---|---|
| `Running` | exactly one owner-proven child delivery; no unavailable delivery |
| `Dormant`, initial `Installing`, or install during shutdown | exactly one policy-owned unavailable effect retaining phase and complete command |
| `AwaitingStop` or replacement `Installing` | exactly one policy-owned unavailable effect; never send new work to the stopping or not-yet-committed worker |
| `Vacant`, including restart exhaustion/rejection | exactly one policy-owned unavailable effect |
| `ShuttingDown` | exactly one policy-owned unavailable effect |

Wrong, stale, duplicate, and contradictory lifecycle facts remain typed proxy
errors and cannot consume or duplicate a command. Shutdown and replacement
inputs retain their existing state law. Parent recovery keeps one typed report
and ingress path; it removes the fabricated logical host and the mandatory
domain-message trait. No callback or absolute wrapper-depth parameter is part
of the contract.

User-level construction names no composed output type:

```rust,ignore
let stable = worker.layer(Proxy::new);
```

`Proxy::new` denotes the concrete parent-recovery policy demonstrated by pools.
Its mailbox event is the closed sum of the worker's user lane and proxy-owned
lifecycle inputs; its sends are the named product of worker delivery and
proxy-owned lifecycle/report effects. The application author neither defines
those system inputs nor names the composed output type. No command trait,
proxy route generic, callback, policy marker, registry, erased envelope, or
runtime lookup is introduced.

Expected design-stage surface:

```text
production files: 6-10
production delta: net-negative
public API:       at most +2 general child-input contracts / remove ReturnUnavailable
focused tests:    direct proxy table plus one real pool and one dynamic trace
```

The general child-input contract, if the existing concrete composition cannot
express the regression without it, must replace `RequestProxyWorker`; it may
not coexist as a second delivery mechanism. It must target the concrete
layer-produced child and one private input lane, and Bombay must interpret that
same type without constructing `ProxyEvent<C>` itself. Lifecycle-only fixtures
use an uninhabited domain protocol; they may not invent a command or empty
recovery effect merely to compile.

The compositional gap is the direction of ingress, not replacement itself.
`ChildInput` transfers one private value from an established parent to its
direct child. `ReportToParent` transfers one owned value in the opposite
direction and the interpreter attaches the authoritative child nonce. These
are different capabilities and must not share one catch-all ingress trait.
The child-input contract therefore constructs only the concrete child's
private event. A report-owning behavior transformation delegates that contract
to its inner behavior structurally; it never names the delegated input.
`EventIngress` remains the parent-side selection law for an incoming child
report or same-actor owner input. This is a derived Bombay communication law,
not an actor-model primitive.

### Narrow pool completion capability: pre-edit ledger

**Exact blocker.** A worker assignment currently carries a recipient for the
pool's complete public protocol. The worker must therefore name the recursive
`WorkerPoolProtocol` or `KeyedWorkerPoolProtocol`, including the customer reply
route, even though it only needs to report one completion. This is not
essential protocol information for the worker; it is an ownership leak from
the pool actor.

The smallest end-to-end regression constructs a worker whose public message is
an assignment containing only job, assignment token, stable slot, and payload.
The worker emits one typed completion report. Through the stable-incarnation
layer and an outer shutdown layer, the runtime must deliver that report to the
pool's private completion event exactly once. The pool must then emit the
customer response through its existing reply route. The worker construction
must not name the pool protocol, customer reply route, final stable-child type,
or a structural parent path.

This is a derived Bombay communication law, not an Agha primitive. The actor
model permits communication only through acquired acquaintances; Bombay's
creator/child relationship is the acquired structural capability used here.
The report remains an ordinary typed send effect in `Actions`. The interpreter
adds the exact creator-local child nonce and injects the resulting child fact
into the parent's closed event sum. It performs no lookup and may not silently
discard a closed-parent failure.

The proposed reusable laws are:

| Law | Input | Output | State |
|---|---|---|---|
| structural parent report | one owned report value | exactly one parent event containing the exact child nonce and report | none |
| stable-child relay | one matching direct-child report | exactly one structural report to its own parent | none |
| pool completion | one matching stable-slot report | the existing completion transition and response effects | existing pool state only |

The pool must also have exactly one lifecycle authority. `FixedFleetOwnership`
owns stable installation, current worker incarnation, replacement admission,
restart exhaustion, and terminal drain. A pool slot owns only the work that
the pool accepted: vacant, assigned, returned before delivery, or permanently
retired with the customer-facing reason. `Installing` and `Stopping` are
derived observations of ownership and must not be stored again by the pool.
No reconciliation transition is permitted.

| Authoritative ownership observation | Pool-owned work | Public pool phase |
|---|---|---|
| draining | any active work after it has been returned | `Stopping` |
| current worker incarnation is routable | vacant | `Idle` |
| current worker incarnation is routable | assigned | `Assigned` |
| no routable incarnation, slot restartable | vacant or returned | `Installing` |
| slot permanently retired | retained retirement reason | `Retired` |

Every accepted lifecycle fact is folded by ownership once. The pool may then
move an accepted job or retain a customer-facing retirement reason, but it may
not infer lifecycle provenance from its own work state. A rejected, duplicate,
stale, or contradictory fact leaves both ownership and work unchanged.

The first law must replace the template-specific proxy report interpreter
requests; it may not become a parallel reporting mechanism. The relay is a
valid same-mailbox layer only because it owns a distinct event/effect
transformation. It must delegate unrelated user, lifecycle, initialization,
send, birth, error, and become lanes without positional caller knowledge.
Duplicates and stale completion facts are still judged by the pool's existing
assignment-token law; the relay neither deduplicates nor reinterprets them.

Desired application syntax:

```rust,ignore
let workers = ChildTopology::new([7], |_| Some(worker));
let pool = WorkerPool::new(workers, configuration, replies, Proxy::new)?;
```

The worker's assignment and completion types may name real job and result
types. They may not name `WorkerPoolProtocol`, `KeyedWorkerPoolProtocol`, the
customer reply route, `Proxy`, the pool behavior type, or a wrapper output.

Expected stage surface:

```text
production files: 8-12 in Behavior Actors, plus the Bombay interpreter
production delta: replace specialized reporting; do not retain both paths
public API:       at most 2 general report/child-fact types; remove the three
                  template-specific interpreter-request identities
tests:            pure report/relay folds, complete FIFO and keyed pool traces,
                  outer-wrapper interpreter trace, compile-fail wrong source,
                  and optimized duplicate replay
```

### Dynamic management protocol identity: pre-edit ledger

**Exact blocker.** `DynamicSupervisor` currently implements `Protocol` as the
complete behavior type. Adding or changing its stable-child `BehaviorLayer`
therefore forces every unrelated sender to change its delivery type or alias,
even though the management message law is unchanged.

The user-level invariant is that a sender names only the dynamic-management
domain: address, worker behavior, and the selected customer reply-route kind.
The topology owner separately constructs a value with an inferred stable
layer. Adding an unrelated worker layer must change neither sender code nor
the public protocol identity.

This is a Bombay nominal-capability correction, not a new actor or transition
law. One zero-state protocol type replaces `DynamicSupervisor<..., L>` as the
destination identity. It owns no state, fold, policy, builder, or wrapper.

```text
production files: 2-4
production delta: approximately neutral
public API:       +1 nominal protocol; remove the Protocol implementation from
                  the composed behavior type
tests:            two stable layers share one sender protocol; wrong worker or
                  reply outcome remains a compile-time error
```

### Returned-assignment ordering correction: pre-edit ledger

**Exact blocker.** After the pool emits one assignment, the stable proxy may
later report both that its worker stopped and that the still-mailbox-admitted
assignment could not be forwarded. The current pool joins these facts only
when unavailability arrives first. Worker-stop first moves or resolves the job;
the matching later return is then misclassified as
`UnexpectedAssignmentUnavailable` and fails the pool actor.

The actor-model law is only sequential processing of each actor's accepted
communications. Message paths and delays do not determine a global arrival
order; Karmani and Agha explicitly describe actor message arrival as
indeterminate. Bombay therefore may not derive a FIFO guarantee between the
worker lifecycle observation and the proxy's returned command. The pool join
is a derived Bombay construction, while retry-versus-interrupt remains the
existing explicit pool policy.

The smallest regression submits one assignment, folds the matching
`WorkerStopped` first, then folds the exact `ProxyUnavailable`. It must retain
the one accepted job, emit no duplicate response or assignment, and accept the
return once. Replaying the same return must preserve the complete fact in the
existing typed duplicate/stale error. The complete interpreter regression
must execute the same order through
`StopOnShutdown<WorkerPool<...>> -> RelayChildReports<Proxy<...>>`, rather than
injecting the pool event directly.

The state law is a product of two independently coexisting facts:

- current slot work is vacant, assigned, returned-before-stop, or retired; and
- zero or more exact assignment/job correlations were already retried or
  customer-resolved while their proxy-return fact may still be in flight.

The second component is not another lifecycle owner and does not retain a job,
payload, route, proxy phase, or worker incarnation. It exists only after the
pool has resolved the job side of the join. A matching return consumes exactly
one correlation; a wrong nonce, wrong assignment, wrong job, duplicate, or
unrelated stale return remains the existing complete typed error. Shutdown and
restart exhaustion record the same correlation before resolving the customer,
so a later authoritative return cannot become an opaque actor failure.

Expected stage surface:

```text
production files: 1
production delta: approximately +30 / -5
public API:       +0 types / -0 types
tests:            the independent pool model and complete outer interpreter
                  hierarchy, in debug and optimized builds
```

The implementation may add one private correlation product to the existing
pool slot. It may not add a behavior wrapper, protocol, route, layer, callback,
registry, marker, alias, or second ownership fold.

### Transitive logical-delivery projection: pre-edit ledger

**Exact blocker.** `BirthProtocols` truthfully projects only the
protocols installed by a behavior's transitive birth algebra. Intentional
logical `Delivery<P>` lanes remain visible in each concrete sends product, but
there is no structural projection that lets a generic application owner prove
it hosts every such `P` used by the root and every transitive child. The former
`LogicalHostRequirements` let owners manually repeat a list and therefore
proved neither completeness nor correspondence with the actual sends tree; it
and its gamed dummy test were deleted.

This is static Bombay owner/interpreter metadata, not a new actor effect and
not an Agha law. The send algebra already distinguishes logical delivery from
established-incarnation delivery, creator-local child delivery, private child
input, and interpreter requests. The projection must follow those existing
concrete distinctions without inspecting values or changing `Actions`.

The desired compile-time law is:

```text
logical(Vec<Delivery<P>>)       = P × ∅
logical(established/child/input/request lanes) = ∅
logical(named product)          = append each field projection in interpretation order
logical(Behavior B)             = logical(B::Sends)
                                  ++ logical(each transitive B::Birth child)
```

The result reuses the existing `BirthProtocol<P, Tail>` /
`NoBirthProtocols` duplicate-preserving product and its structural append and
membership proofs. A repeated protocol in two lanes or two child occurrences
therefore remains repeated. A framework may recursively require `Hosts<P>`
for each product element; repeated bounds are lawful and do not require
normalizing the product into Bombay's unique runtime host map.

The first regression derives the exact product from an application with one
logical root delivery, one exact-only root delivery, and two births of a leaf
with the same logical delivery. It contains no manual requirements impl. A
second catalogue regression covers ordinary recipient, established recipient,
mixed recipient, named send products, wrappers, pools, and transitive worker
layers. An unsupported custom sends product must fail at the projection trait,
not silently contribute an empty product.

Expected stage surface:

```text
production files: 5-8
production delta: approximately +180 / -20
public API:       +2 consumer traits and +1 doc-hidden birth-node fold;
                  no public data type
tests:            replace the deleted manual-metadata test with structural
                  derivation and add catalogue exact-product assertions
```

Every existing named sends product will implement the same structural
projection in `actors::requirements`; this avoids spreading metadata through
template folds. No behavior wrapper, host value, registry, `dyn`, `Any`,
`TypeId`, protocol erasure, macro-only route, callback, or runtime lookup is
authorized.

### Transitive logical-delivery projection: core/catalogue checkpoint

The implementation follows the stated structural law. Core send leaves and
`SendLayer` implement `LogicalDeliveryProtocols`; `LogicalHostRequirements` is
blanket-derived from one behavior's sends and every transitive birth node. The
actors catalogue implements the same fold for its handwritten semantic
products. Unsupported custom send products fail at
`LogicalDeliveryProtocols` rather than silently projecting an empty list.

Evidence is independent of an owner-authored metadata list:

- the structural test derives `P, Q, Q` from one root logical lane, one exact
  lane, and two child occurrences of the same logical destination;
- a framework-style recursive consumer accepts the duplicate-preserving
  product without canonicalizing it;
- real worker-pool and dynamic-supervisor birth trees derive only their actual
  customer reply protocol; and
- the compile-fail contract rejects an opaque custom send product.

No behavior wrapper, runtime host value, registry, erased protocol, callback,
or manual requirements implementation was added.

The attempted automatic implementation for `#[behavior]`-generated named
products is rejected and removed. It made every declared field require
`LogicalDeliveryProtocols` at actor definition time, so a valid custom effect
lane such as `Vec<u8>` stopped being a valid `SendEffects` product even when no
logical-host projection was requested. Hidden generics, marker traits, and
syntax-based macro classification would only move that compiler constraint and
are not authorized.

Generated and custom products use the same owner contract as handwritten
catalogue products: the product owner implements `LogicalDeliveryProtocols`
when, and only when, a framework asks for complete hosting metadata. This does
not narrow `Behavior`, `SendEffects`, or custom interpretation. A generated
two-delivery product projects both destinations in authored order. A separate
custom product containing one delivery lane and one uninterpreted `Vec<u8>`
marker lane projects only its actual delivery. The existing generated
`Vec<u8>`-only products continue to compile without any projection impl.

This is not the deleted manual behavior-level requirements list. The contract
is co-located with the concrete sends product whose `InterpretSends` law gives
those fields meaning, and the blanket behavior projection still traverses all
root and transitive birth products. As with `SendEffects` and
`InterpretSends`, a downstream implementation is responsible for satisfying
the documented trait law; an arbitrary custom interpreter's semantics cannot
be discovered by inspecting Rust syntax.

### Compiler-driven relapse checkpoint

The automatic `#[behavior]` projection was not the only compiler-shaped
assumption in this stage. A complete workspace test build exposed two more:

- the hierarchy regression declared `Births<Queue>` on an application that
  never stages a queue creation, then treated the resulting appended
  `ChildChoice<Queue, Proxy<Queue>>` as though it were exactly the proxy; and
- after the fabricated pool completion recipient was removed, the pool
  constructor no longer receives or derives its customer result and
  reply-route contract. A later real `Submit` value still cannot determine
  `R` through the non-injective `Route::Protocol::Msg` equality.

The false birth capability is removed from the test. The complete seven-test
layer hierarchy now builds and passes with `NoBirths`; its sole creation is the
stable child actually staged by supervision. No production type was changed
to accommodate `ChildChoice`.

The initial pool inference failure was also classified too quickly as a
production blocker. Topology, replacement policy, and the worker layer contain
no customer result schema or reply-route kind. `BehaviorLayer` can infer the
complete concrete wrapper stack, but no static construction can truthfully
invent public protocol facts absent from both its inputs and its expected
context. The owner must state that domain contract somewhere.

An actual command value alone does not provide reverse inference through
`Route::Protocol::Msg`; claiming that it did was incorrect. The application
owner instead states the public protocol it hosts:

```text
B::Protocol = WorkerPoolProtocol<MailAddr, Job, Result, ReplyRoute>
```

That is domain identity, not the concrete composed actor type. From this owner
boundary Rust infers the complete `WorkerPool<...>`, `BehaviorLayer::Output`,
proxy, relay, event sum, sends product, and birth tree. The corrected
regression retains the inferred value and executes initialization plus a real
`PoolMessage`; it is not a discarded compile-only witness. It names no
composed behavior, recursive worker protocol, parent path, or effect-product
alias. A turbofish on `WorkerPool`, phantom witness, unused recipient, default
generic, or constructor overload remains rejected because each would encode
compiler evidence instead of the protocol an actor owner must actually host.

The semantic boundary follows the actor model's acquaintance rule: an actor
can communicate only through a known name, and names may be communicated in
messages. Bombay then adds four derived, statically typed communication forms:

| direction | emitted effect | authority/provenance | realized input |
| --- | --- | --- | --- |
| external or peer to actor | `Delivery` / `EstablishedDelivery` | known logical or exact recipient | public `User` lane |
| creator to direct child | `ChildDelivery` | committed creator-local child binding | child's public `User` lane |
| creator to direct child | `ChildInput` | committed binding plus owner-selected private lane | private child event |
| direct child to creator | `ReportToParent` | established creator relationship | `ChildReport` with interpreter-attached child nonce |
| interpreter back to emitter | `InterpreterRequest` return | request's declared local continuation | selected event in the same incarnation |

The model justifies `ChildInput` and `ReportToParent`/`ChildReport` as distinct
effect laws: they travel in opposite directions, use different established
relationships, and attach different provenance. `ChildInputIngress` is the
receiver contract for the former; `EventIngress` selects owner/report or local
continuation inputs for the latter cases. The two traits may remain because
they correspond to different interpreter capabilities, not because one blanket
implementation overlaps another. `ReplacementRequested<C>` also remains a
distinct request value: carrying a behavior definition is not evidence that a
replacement was created or installed, and the request must remain distinct
from both those later facts.

```text
production edits authorized by this checkpoint: 0
public types authorized by this checkpoint:      0
next evidence: complete workspace build, then catalogue classification of
               remaining public wrappers and hard-coded higher-order laws
```

# Delayed replacement composition: provisional pre-edit ledger

The standalone-supervisor fuzz contract still requires delayed replacement,
but the current worktree exposes that law only through `BackoffSupervise`,
whose input is the application-specific `Supervise` behavior. Restoring a
second standalone backoff state machine would duplicate ownership and repeat
the compiler-driven design that this audit is removing.

- Exact blocker: one timer-delay law cannot consume both existing owners,
  `Supervisor` and `Supervise`; the standalone fuzz target therefore cannot
  express its previously covered behavior.
- Smallest regression: construct `Supervisor::new(..., Proxy::new)`, apply the
  same delayed-replacement layer used for `Supervise`, observe a worker stop,
  prove no replacement input is emitted before the matching timer, and prove
  exactly one is emitted after it.
- Expected files: the shared backoff implementation and exports, its unit and
  interpretation tests, the standalone fuzz target, and current documentation
  (fewer than ten files).
- Expected production delta: net negative, because one generic transformation
  replaces template-specific backoff products and avoids restoring the deleted
  standalone implementation.
- Public types: add one owner/consumer contract and one genuine delayed-law
  behavior; replace the current backoff send product with its schedule-only
  lane; remove the application-specific `BackoffSupervise` surface.
- Reused law: `Supervisor` and `Supervise` remain the sole owners of topology,
  restart policy, observation, shutdown, and replacement input creation. The
  delayed layer may only retain and later re-emit those existing replacement
  inputs; it must not recreate or reinterpret the supervision fold.

### Delayed replacement: completed ownership-law checkpoint

The proposed outer delayed layer is rejected. Intercepting an emitted
replacement input after the inner ownership fold has advanced would let the
owner record a request as issued while the child has not received it. A worker
creation fact, shutdown, or another stop arriving during that interval would
therefore be folded against a false ownership state. Making the wrapper repair
that state would duplicate and reinterpret supervision.

Delay instead participates in the existing replacement-admission sum inside
`FixedFleetOwnership`. `RestartTiming::Immediate` opens the replacement gate in
the admitting transition. `RestartTiming::Delayed(Backoff)` retains the exact
batch and its per-member replacement requests behind one generation-tagged
timer gate. Timer arrival and worker readiness join in either order; the last
required fact emits each replacement exactly once. Shutdown cancels the
retained batch, and stale or duplicate timer facts emit nothing.

This is Bombay policy rather than an actor-model timing guarantee. It adds one
policy sum carrying real state and deletes both former `BackoffSupervise` and
`BackoffSupervisor` behavior families, their wrapper events, send products,
errors, parent-path variants, aliases, and duplicated folds. The same private
ownership implementation is exercised by standalone `Supervisor`,
application-owned `Supervise`, and both worker-pool variants.

Independent evidence compares the complete schedule and replacement lanes of
standalone and application-owned supervision for the same trace. A pool trace
additionally proves that an interrupted assignment stays queued until the
exact timer releases replacement, that a duplicate timer cannot release it
twice, and that installation dispatches the retained job once.

### Standalone failure reaction: pre-edit ledger

Exact blocker: standalone `Supervisor::with_failure_reaction` accepts
`Step<Never>`. Because that spelling defaults both the phase and terminal
payload to `Never`, callers cannot construct its documented stop result; the
configuration can only continue. The supervision sequence target exposes the
defect by attempting the advertised terminal reaction.

```text
expected production files: fixed_supervisor.rs
expected production delta: 3 substitutions, net zero
public API: +0 types / -0 types
reused algebra: Become = Step<Never, Stopped>
regression: budget denial returns the complete failure report and Stop(Stopped)
```

No policy enum, marker, wrapper, alias, or convenience constructor is needed.

The contract now uses the existing `Become` alias, making both `Continue` and
`Stop(Stopped)` constructible. The regression proves that budget denial keeps
the complete typed failure report while the configured standalone owner
selects termination; the supervision sequence target exercises the same path.

### Redundant installation-proof facade: pre-edit ledger

Exact blocker: `InstallationRequirements`, `RequirementAt`, and four public
aliases merely repeat Behavior Core's already-exported `BirthProtocols`,
`BirthProtocolAt`, `BirthProtocol`, `NoBirthProtocols`, and structural position
types. They add no state, transition, effect, proof, or inference capability;
the actors facade already re-exports the core vocabulary wholesale.

```text
expected production files: requirements.rs, lib.rs
expected production delta: approximately -75 lines
public API: +0 types / -6 types
tests/docs: spell the same exact products with the existing core proof
```

The logical-delivery projection implementations remain in `requirements.rs`;
only the parallel installation vocabulary is removed.

The six-name facade is deleted. All exact-product, repeated-occurrence,
wrapper, proxy, pool, and dynamic-supervisor proofs now use the Behavior Core
vocabulary directly; the workspace check passes without a compatibility
alias.

### Nominal public capability consistency: pre-edit ledger

Exact blocker: `Cache`, `Barrier`, and `Latch` each implement their complete
nominal public `Protocol` for `Self`, but their `Behavior::Protocol` is an
equivalent `MessageProtocol`. A sender can therefore name a
`Recipient<Cache<...>>`, `Recipient<Barrier<...>>`, or `Recipient<Latch<...>>`
that is not the protocol identity the runtime hosts for that behavior. No
protocol-adaptation law justifies the second identity.

This is Bombay's derived stable-capability law, not an actor-model guarantee:
a standalone catalogue actor that authors one nominal public protocol exposes
that exact protocol through `Behavior::Protocol`; transparent outer layers
preserve it.

```text
smallest regression: each actor and StopOnShutdown<actor> satisfies
                     Behavior<Protocol = actor> and accepts an ordinary
                     Recipient<actor> capability
expected production files: cache.rs, barrier.rs, latch.rs
expected production delta: 3 substitutions, net zero
public API: +0 types / -0 types
reused algebra: the existing Protocol impl for Self and transparent wrapper law
```

The regression must fail on the prior definitions because the protocol
identities differ, not because a helper alias is missing. No alias, wrapper,
conversion, compatibility implementation, or constructor is authorized.

The pre-fix regression produced six `E0271` identity failures: one for each
standalone actor and one for each transparent shutdown composition. The three
behaviors now expose `Self`; ordinary nominal capabilities and the wrapped
forms compile and pass in debug and optimized builds. No transition branch,
message, effect, error, or constructor changed.

```text
production: +8 / -9 / net -1
tests:      +31 / -0 / net +31
public API: +0 types / -0 types
```

### Keyed-pool delegation: pre-edit ledger

Exact blocker: `KeyedWorkerPool` owns only key-to-stable-slot binding,
first-admission selection, and explicit rebalance, but it stores the private
`PoolState` and repeats the ordinary pool's complete lifecycle, completion,
returned-assignment, shutdown, and dispatch transition. Sharing the state
helper is not behavior composition; the two public folds can drift while
claiming one pool law.

The keyed actor remains a distinct transformation because its affinity table
is genuine state. It must store and delegate to the existing concrete
`WorkerPool`, handling only keyed submit and rebalance itself. Common events
must enter the ordinary pool fold exactly once; its complete actions pass
through unchanged and its uninhabited-key error is widened exhaustively by the
existing `widen_pool_failure` function.

```text
smallest conservation trace: FIFO and one-key pools receive equivalent
                             initialization, installation, submit, completion,
                             worker-stop, replacement, shutdown, duplicate,
                             and stale facts; common action/error snapshots
                             remain equal
expected production files: pool.rs only
expected production delta: net negative (delete the repeated common match)
public API: +0 types / -0 types
reused laws: WorkerPool Behavior fold, KeyedWorkerPool affinity sum,
             widen_pool_failure, identical PoolSends and Births products
```

This is a deletion/refactor claim, not a new observable actor law. A black-box
test cannot distinguish two identical implementations from delegation; source
deletion is the evidence for composition, while the independent and exhaustive
keyed/FIFO models protect observational conservation. No structural proof
marker, constructor overload, wrapper, policy trait, or source-text test is
authorized merely to make the old implementation fail a test.

### Keyed-pool delegation: completed checkpoint

`KeyedWorkerPool` now stores the concrete `WorkerPool` behavior. Keyed submit
and rebalance remain its only authored transitions. Initialization and every
common completion, returned-command, creation, stop, timer, and shutdown event
enter the ordinary pool fold exactly once. The one shared private subfold is
targeted admission: the keyed layer needs its `Accepted | Rejected` result to
commit a new affinity binding atomically, and that result is not an effect that
may truthfully be recovered by inspecting `Actions`.

No public declaration, wrapper, alias, constructor, event, or effect product
was added. The FIFO and keyed independent models pass in debug and optimized
builds; the complete outer-layer interpreter tests pass in both profiles; and
the pool sequence fuzz target completed 5,000 runs. Workspace Clippy is clean.

The cumulative working-tree checkpoint after this stage is:

```text
production:     +3762 / -3753 / net +9
tests:          +4251 / -2300 / net +1951
docs:           +1599 / -70   / net +1529
infrastructure: +28   / -2    / net +26
```

### Pool effect product: pre-edit ledger

Exact blocker: the pool owns three semantic effect lanes—customer responses,
worker assignments, and fleet supervision—but its complete effect product is a
private `SendLayer<SupervisorSends, PoolBehaviorSends>`. Direct users and
interpreters must consequently know that pool effects are `.inner` and
supervision effects are `.owned`; another transparent wrapper adds another
positional hop. The paths describe implementation nesting rather than the pool
contract.

This is Bombay's named-product law. Replace the existing public
`PoolBehaviorSends` with one public `PoolSends` product whose fields are
`responses`, `assignments`, and `supervision`. It must preserve the current
interpretation order—responses, then assignments, then supervision—and preserve
the current return-to-emitter paths. It must append each lane once and project
only the logical response destinations. This is a replacement, not an
additional layer.

```text
smallest regression: construct PoolSends by semantic field name and interpret
                     one value in every lane; assert the complete ordered trace
expected production files: pool.rs, requirements.rs, lib.rs
expected test/docs files: runtime_contracts.rs, pool model/property/fuzz users,
                          worker-pool.md
expected production delta: approximately net zero
public API: +1 product / -1 product
deleted machinery: private PoolSends alias and the pool's SendLayer nesting
reused laws: SupervisorSends, SendEffects, SendsFor, InterpretSends,
             LogicalDeliveryProtocols, existing concrete pool events
```

No wrapper, builder, alias, marker, conversion, compatibility spelling, or
generic lane registry is authorized.

### Pool effect product: completed checkpoint

`PoolBehaviorSends` and the private `SendLayer` alias are gone. `PoolSends`
now owns the complete named product: `responses`, `assignments`, and
`supervision`. Its `SendsFor` proof keeps customer effects at the inner pool
event path and supervision requests at the outer supervision path. Its
interpreter visits responses, assignments, and supervision once each in the
previously authored order. Logical-host projection remains exactly the
customer response protocols; creator-local assignments and interpreter-owned
supervision requests do not fabricate hosts.

The pre-fix compile regression failed because `PoolSends` was not public. The
complete interpreter regression now constructs every semantic lane by name and
observes the exact eight-effect trace in both debug and optimized builds. FIFO
and keyed independent/model suites, wrapper-order tests, and user-facing pool
construction tests pass in both profiles. Workspace check and Clippy are clean.

A neighbouring test incorrectly expected the supervision law to observe an
inner behavior's unrelated child creation. The production fold already
preserved the correct boundary. The strengthened test now proves that the inner
creation remains the prefix birth occurrence and precedes the two owned stable
creations, while only the stable topology is observed.

This stage is not code reduction: the explicit named-product implementation is
18 net new production lines after deleting the generic nesting.

```text
stage production: +53 / -35 / net +18
stage tests:      +78 / -85 / net -7
stage public API: +1 product / -1 product

cumulative production:     +3815 / -3788 / net +27
cumulative tests:          +4329 / -2385 / net +1944
cumulative docs:           +1665 / -73   / net +1592
cumulative infrastructure: +28   / -2    / net +26
```

### Supervision-test semantic correction checkpoint

The complete workspace run exposed seven tests whose expected traces described
a different ownership law from the production types:

- several strategy/model tests stopped workers before their authoritative
  creation facts had installed them, then expected unresolved peers to emit
  replacements immediately; and
- two tests treated unrelated inner-application births as though
  `Supervise` had adopted them into its fixed fleet.

No production fold was changed. The corrected tests first join every declared
stable child with its committed creation fact, then exercise replacement
strategy over that installed topology. Inner births are asserted to remain the
preserved prefix of the composed birth algebra and outside fixed ownership.
The replacement trace now also proves that `RestForOne` follows declared
topology order rather than numeric nonce order.

The corrected focused suites pass in debug and optimized profiles. The full
workspace then passes 542 of 542 tests and workspace Clippy is warning-clean.
This checkpoint also removes the now-unused dynamic-birth fixture; it adds no
production code or public API.

```text
cumulative production:     +3822 / -3802 / net +20
cumulative tests:          +4358 / -2486 / net +1872
cumulative docs:           +1697 / -67   / net +1630
cumulative infrastructure: +28   / -2    / net +26
public API: replacements and deletions recorded by the per-stage ledgers;
            no public type was added by this checkpoint
```

### Unconsumed transition effects: pre-edit ledger

**Exact blocker.** `Actions` is the explicit, still-uninterpreted result of one
actor transition, but the value type is not `must_use`. The workspace denies
`unused_must_use`, yet a test or interpreter can unwrap a successful fold and
silently discard its sends, fresh creations, and next-behavior verdict. The
current tree contains direct examples, including lifecycle and wrapper tests.
This defeats the audit rule that tests observe complete actions and leaves the
compiler lint unable to enforce the underlying semantic law.

This is a derived Bombay effect-conservation rule: producing `Actions` is not
performing those effects. A caller must interpret, inspect, or explicitly
retain the complete value. Marking the existing product `must_use` adds no
effect, state, route, wrapper, or type.

```text
smallest regression: a successful Active::transition(...).unwrap(); statement
                     is rejected by the workspace unused_must_use lint
expected production files: effects/actions.rs only
expected production delta: +1 attribute, net +1
expected test files: the initial textual scan found fourteen; the diagnostic
                     contract exposed at least thirty-one source/unit files
                     before downstream testkit binaries compiled, so the final
                     inventory is the warning-clean full workspace
public API: +0 types / -0 types; one diagnostic attribute on Actions
repair law: each site asserts or feeds the complete sends, creations, and
            become verdict appropriate to its independent model
```

Binding a result to an underscore, calling `drop`, or locally allowing the
lint is not an accepted repair. The test must state why every effect lane is
empty or otherwise consume its complete observable result.

The stage exceeds fifteen files. This is inside the already-authorized
repository-wide audit, but the expansion is recorded rather than hidden: the
single production attribute revealed previously existing test evidence loss;
it did not cause or require another production abstraction.

#### Heterogeneous shutdown effect visibility

The unconsumed-effect regression exposed one independent testability defect:
`HeterogeneousShutdownSends<T>` is public, but its sole semantic lane is
private and the only slice accessor is compiled for crate unit tests. A
framework consumer can interpret the product, but cannot inspect or model-check
the complete transition result as data. That contradicts the named-product law
used by the other public effect products.

```text
smallest regression: an integration test cannot name the emitted ordered
                     shutdown selections in Actions.sends.owned
expected production files: lifecycle/shutdown_coordinator.rs
expected production delta: replace one test-only accessor with one public,
                           documented read-only accessor; approximately +3 net
expected test files: heterogeneous_shutdown.rs
public API: +0 types / -0 types; +1 read-only method
reused law: the existing HeterogeneousShutdownSends product and its exact
            phase-ordered request vector
```

This does not add a planner, layer, wrapper, route, or alternate effect lane.

### Unconsumed transition effects: completed checkpoint

`Actions` is now `must_use`. Every diagnostic exposed by the workspace was
repaired by observing or forwarding the complete result rather than binding it
to `_`, dropping it, or suppressing the lint. The public heterogeneous
shutdown product now exposes its existing ordered lane through a read-only
`as_slice` method, so integration and model tests can inspect the complete
effect without privileged test-only access.

The independent fuzz workspace now also denies `unused_must_use`. Its twelve
targets consume initialization and transition actions, including sends,
creations, and the next-behavior verdict. Each target completed 5,000 generated
runs after the discovered minimal sequences were replayed directly. The runs
corrected several copied or incomplete test assumptions: application births
are not adopted into fixed ownership, presence announcement always replies,
pool stop may both replace and assign in one action, and initial proxy creation
requests observation before the worker-resolution join completes. These are
test-oracle corrections; no production behavior was changed to satisfy a fuzz
branch.

The generated corpus and crash files were removed after replay. They are run
artifacts, not source evidence. The flake now exposes a locked `fuzz` package
and shell containing nightly Rust, `llvm-tools-preview`, and `cargo-fuzz`, while
the ordinary repository gate remains on pinned stable Rust. A clean consumer
can therefore build or run the same targets with `nix run .#fuzz -- ...`
without relying on a separately installed mutable toolchain.

```text
stage production: +12 / -0 / net +12
  Actions attribute: +1
  existing heterogeneous shutdown lane accessor: +11
stage public API: +0 types / -0 types; +1 read-only method
fuzz source/infrastructure currently visible in the working-tree diff:
  +826 / -299 / net +527
```

### Exact dynamic-start capability: pre-edit ledger

**Exact blocker.** A successful dynamic start currently receives an
authoritative committed creation fact and then returns
`Recipient::global(address)`. That discards incarnation identity and
reconstructs a weaker logical destination from allocation data. The
`Started` outcome must carry the exact established recipient produced by the
creation commit. A rejected creation must carry no recipient.

This is the existing creation-capability law, not a new dynamic-supervision
abstraction. Reuse `ObserveEstablishedCreation`, `EstablishedCreation`, and
`EstablishedRecipient`; do not add a wrapper, conversion registry, second
protocol, or address-to-endpoint lookup.

```text
smallest regression: both initial-fact orders return the exact endpoint issued
                     by the creation interpreter, exactly once
expected production files: dynamic_supervisor.rs and requirements.rs
expected production delta: approximately neutral
expected test files: dynamic_supervisor_join.rs, runtime_contracts.rs,
                     supervision_sequences.rs, and affected interpretation tests
public API: +0 types / -0 types; Started strengthens its existing child field
```

### Exact dynamic-start capability: completed checkpoint

`DynamicSupervisorOutcome::Started` now returns the exact
`EstablishedRecipient` issued by the committed proxy creation fact. The fold
retains that capability when it arrives first, joins it with the matching
worker-resolution fact in either order, and emits it exactly once. Rejections
carry no capability. Wrong nonces, wrong creation kinds, duplicate facts,
stale facts, and contradictory authoritative results retain their complete
typed errors; shutdown is total before, between, and after the joined facts.

The complete outer `StopOnShutdown<DynamicSupervisor<...>>` composition is
tested in both fact orders. The independent join suite passes six cases in
debug and optimized builds, and the supervision fuzz target completed 5,000
runs through both orders, worker rejection, and all intermediate shutdown
points using the flake-locked fuzz runner. The generated corpus was removed
after the run.

No public type, wrapper, registry, protocol alias, or endpoint lookup was
added. Existing logical-address tests now use test-local runtime address types
with opaque established endpoints where they model dynamic creation; fixed
supervision and pool contracts remain logical where no exact establishment
law applies.

```text
production files: dynamic_supervisor.rs and requirements.rs
public API: +0 types / -0 types
verification: workspace compile; focused debug/release folds; 5,000 fuzz runs
```

### Explicit restart timing: pre-edit ledger

**Exact blocker.** `RestartConfiguration::new` and
`PoolConfiguration::new` silently choose `RestartTiming::Immediate`. Immediate
versus delayed emission changes the transition trace and is therefore policy,
not a constructor default. Construction must receive the timing value
explicitly. The delayed convenience constructors then carry no distinct law
and should be deleted rather than retained as a second configuration path.

The mechanical caller migration spans more than fifteen files (the current
search finds 85 constructor occurrences across production, tests, benches,
fuzz targets, and documentation). No production edit for this stage begins
without treating that breadth as the required checkpoint rather than hiding it
behind an alias or compatibility constructor.

```text
expected production files: supervisor.rs and pool.rs, plus direct production callers
expected production delta: net-negative
expected public API: +0 types; remove `RestartConfiguration::delayed` and
                     `PoolConfiguration::delayed`
reused law: RestartTiming and the existing immediate/delayed ownership fold
```

### Explicit restart timing: completed checkpoint

Both configuration products now have one constructor, and that constructor
requires `RestartTiming`. Seventy-seven formerly implicit immediate choices and
seven delayed-constructor choices were migrated directly; no default, alias,
wrapper, or compatibility path remains. The existing immediate and delayed
ownership folds remain the only semantic implementations.

The focused public-contract regression and the independent supervision and
pool models pass in debug and optimized builds. All 542 workspace tests, the
warning-denied Clippy and rustdoc gates, and the authoritative Nix flake check
pass. The supervision and pool fuzz targets each complete 5,000 runs without a
crash; their generated untracked corpus expansions are removed afterward.

```text
production: +4893 / -4183 / net +710
tests:      +7403 / -2964 / net +4439
docs:       +2043 / -107 / net +1936
infra:      +52 / -2 / net +50
public API: +37 types / -53 types; this stage adds no type and removes two
            redundant public constructor methods
```

### Public relay classification

`RelayChildReports` is not compiler-only naming machinery. It owns one
observable transformation: one report from one statically selected direct
child becomes exactly one `ReportToParent` effect, while every inner event,
effect, birth, error, phase, and verdict is preserved. The pool uses that law
to move worker completion across the proxy edge without fabricating a logical
destination.

That justifies a concrete reusable composition in `actors::composition`; it
does not automatically justify placing the type and its event algebra in the
crate-root application facade. The export audit must distinguish the public
associated-type requirement from root-level convenience and remove the latter
if no direct application use requires it.

### Completion cleanup audit: evidence checkpoint

The cleanup pass distinguishes unreachable code from public-but-unnecessary
facade exposure and from concrete types that must remain nameable through a
public `Behavior` associated type.

Current evidence:

- workspace Clippy passes for all targets and features with warnings denied;
- every Rust source below `crates/behavior/src` and `crates/actors/src` is
  reachable from the current module graph; the deleted backoff-supervisor file
  has no remaining module or source reference;
- the working tree contains no untracked fuzz corpus or crash artifact;
- a source scan before each `#[cfg(test)]` module finds no production
  `Recipient::global(...)` construction; and
- current documentation no longer names `ProxyCommand::Forward` or describes
  dynamic `Started` as a reconstructed logical recipient. Released changelog
  sections retain their historical names as release history.

The warning-denied rustdoc gate exposed two source-documentation defects:
`Registry` linked to `Delivery` without a resolvable path, and public
`Supervisor` documentation linked to the crate-private `FixedFleetOwnership`
type. The links are repaired without widening the ownership fold.

`FixedFleetOwnership`, `OwnershipFold`, and `OwnershipError` are crate-private
and retained. The first owns the one shared fixed-fleet state-transition law;
the second is the named product of complete actions plus an optional failure
that the application-composed, standalone, and pool owners interpret
differently; the third retains complete rejected lifecycle facts for those
public error mappings. The former `WorkerDisposition` and
`WorkerOwnershipFold` names no longer exist.

`RelayChildReports` and `RelayChildReportEvent` must remain public inside
`actors::composition`: the concrete relay is both directly composable and
appears through the pool's public birth/event algebra. Its unused crate-root
re-export is removed; that deletes a duplicate spelling without removing the
relay feature.

`EventIngress` and `ChildInputIngress` are not classified as dead. They
currently name opposite established-parent directions: child-to-parent facts
enter the parent's event algebra, while private parent-to-child inputs enter
the concrete child's event algebra. However, the relay also relies on the
trait split to combine one owned report implementation with blanket
preservation of arbitrary inner child inputs. Before accepting that as two
laws, a separate pure/compile-only probe must establish whether one existing
owner-indexed ingress contract can express both directions without structural
paths, overlap, a new marker type, or a wrapper. Compiler coherence alone is
not sufficient justification under the layer laws.

The downstream Bombay tree confirms that no compatibility surface should be
restored here: its current migration branch still names removed parent-specific
composition types and asserts that `Started.child` has a logical address. It
already owns exact endpoint interpretation, so it must consume the strengthened
established capability and update its application test rather than Behavior
reintroducing a weaker alias or address reconstruction.
