# Actor-template composition audit

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
| runtime hosting metadata | `LogicalHostRequirements` | the owner-authored transitive product of only the logical routes selected above |

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
| `ProxyWithParent` | stable slot and fresh-incarnation replacement lifecycle | retain core |
| `SuperviseWithParent` | adopt application creations into fixed proxy ownership while preserving inner actions | retain transformation |
| `SupervisorWithParent` | standalone fixed proxy-fleet ownership | retain core, with a concrete implementation rather than a command-policy engine |
| `SupervisedWorkers` and selector policy types | parameterize fixed ownership with one routing selector | delete; route commands with an ordinary typed routing actor/application behavior |
| `BackoffSuperviseWithParent` | delays accepted replacement effects from a supervised application | retain transformation |
| `BackoffSupervisorWithParent` | delays accepted replacement effects from standalone fixed ownership | retain transformation |
| `FixedBackoff`, `BackoffWorkers` | implementation/policy parameterizations of the two retained delayed folds | delete |
| `DynamicSupervisorWithParent` | changing stable-child membership and replacement lifecycle | retain core |
| `WorkerPoolWithParent` | bounded FIFO admission, assignment, completion, and interruption | retain core and its direct `Behavior` fold |
| `KeyedWorkerPoolWithParent` | the FIFO law plus persistent key-to-slot affinity and rebalance transitions | retain separately and restore its direct `Behavior` fold |
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
of the same name (`Watch`, `SupervisorWithParent`, and
`BackoffSupervisorWithParent`); they add no public type name. The five additions
are `ProxyUnavailable`, `CommandSupervisionEvent`, `DeclareShutdownPhase`,
`FinishShutdownPhases`, and `LogicalHostRequirements`. Eight constructors or
methods were also removed, for 34 removed public names in total. No public
wrapper or policy-marker type was added.

The later semantic regressions retain this classification while adding no
actor wrapper. Dynamic supervision now joins its two initial creation facts in
either order. Proxy commands carry an explicit typed logical unavailability
recipient, and pools join that return with worker-stop/replacement facts.
`DeclareShutdownPhase` and `FinishShutdownPhases` expose the retained builder to
generic consumers through associated outputs. `LogicalHostRequirements` is an
owner-authored, duplicate-preserving closed product of all transitive logical
destinations; exact-only endpoints remain absent.

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
