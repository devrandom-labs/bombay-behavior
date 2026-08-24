# Behavior Actors template-law audit

This is the working record of a proof-driven audit of every reusable `Behavior` implementation
exported by `bombay-behavior-actors`. It replaces the earlier routing-only
review, which incorrectly accepted creator-local delivery from a standalone
`MessageAdapter` and did not test `BehaviorBase` at the runtime resolution
boundary.

This file is not a completion certificate. A verdict is final only after its
negative cases have an independent oracle and the full repository gates pass
on the recorded revision. Earlier versions of this file incorrectly declared
the catalogue complete while several action-loss and hard-coded-path cases
were still untested; those declarations are not evidence.

The audit question is not merely whether a template's fold compiles. For every
capability or lifecycle fact, the review traces:

1. who produced or supplied it;
2. which concrete type retains it;
3. which actor namespace owns its interpretation;
4. which `Actions` lane emits it;
5. which static interpreter authority realizes it; and
6. how rejection or stale input remains typed and observable.

## Laws and policies used

The source classification matters. Framework convention is not presented as
actor-model law.

### Actor-model laws

- **A1 — serialized turns:** one actor processes one communication at a time;
  the transition determines communications, fresh creation, and the next
  behavior.
- **A2 — finite acquaintance:** a transition may communicate with prior
  acquaintances, acquaintances in the current communication, and freshly
  created actors.
- **A3 — fresh creation:** allocation is fresh; replacing behavior with
  `become` is distinct from actor creation.
- **A4 — no primitive effect ordering:** the actor model does not require a
  general order among send, create, and become effects.

The primary-source extraction and the exact boundary between these laws and
Bombay constructions are recorded in
[`research/architecture-critical-review-loop`](../research/architecture-critical-review-loop/ACTOR-RESEARCH-SURVEY.md)
and
[`research/established-recipient-capability`](../research/established-recipient-capability/REPORT.md).

### Behavior Core laws

- **B1 — explicit effects:** a successful pure fold returns complete
  `Actions`; it does not deliver, allocate, schedule, observe, or stop actors
  through an ambient side channel.
- **B2 — concrete protocols:** protocol, event sum, send product, phase, error,
  and birth algebra remain statically visible.
- **B3 — capability distinctions:** `Recipient`, `ChildRoute`, and
  `EstablishedRecipient` mean logical name, creator-local occurrence, and
  exact installed incarnation respectively. They are not interchangeable.
- **B4 — structural child authority:** a local child effect resolves only when
  the running emitter's direct `Birth` algebra proves the named occurrence.
- **B5 — transparent projection:** nominal roles cross `BehaviorBase` only
  while both canonical protocol and complete direct-birth algebra are equal.
- **B6 — compositional initialization:** wrappers initialize their inner fold
  once, preserve its full result, and define how wrapper effects accumulate.
- **B7 — typed rejection:** rejected creation carries no established
  capability; controlled transition failures do not partially commit a new
  behavior state.

### Declared Bombay policy

- **P1 — staged child routes:** a creator-local nonce may name a requested
  child before interpretation but is neither an address nor freshness proof.
- **P2 — commit before dependency:** interpreters commit same-action creation
  before dependent local sends and observation requests.
- **P3 — explicit provenance:** birth, replacement incarnation, observation
  relationship, timer generation, and shutdown request provenance travel as
  typed data.
- **P4 — structural return paths:** interpreter facts return through the exact
  typed path selected by the owning composition.
- **P5 — explicit root shutdown:** the root is composed directly as
  `StopOnShutdown`, `FinalizeOnShutdown`, or a shutdown coordinator; it does
  not discover a nested shutdown handler.
- **P6 — stable logical domains:** discovery membership, configured downstream
  destinations, stable proxy identity, and transport names deliberately remain
  logical recipients.
- **P7 — truthful customer routes:** a template that accepts an arbitrary
  customer capability retains its logical or exact form and emits the matching
  concrete effect without conversion.
- **P8 — creation-dependent shutdown plans:** a coordinator may begin before
  its committed children are known. The topology owner reports its validated
  plan through `Actions` to an explicit parent return path; installation is one
  typed event, happens at most once, and retains any earlier shutdown request.

## Hypotheses and verdicts

`Failed → fixed` means the hypothesis was false in the audited revision and
this change repairs it. A pass means both the public type surface and the
relevant fold/interpreter seam support the statement.

| ID | Source | Falsifiable hypothesis | Verdict and evidence |
|---|---|---|---|
| H01 | A1, B1 | Every template transition is a deterministic fold returning all effects in `Actions`. | **Failed → fixed.** `Machine` committed a prefix of a failed drain, `Stash` could replay a fallible inner fold after earlier actions became unreturnable, and `Supervise` could reject child adoption after the application fold succeeded. `Machine` now stages the complete drain, `Stash` statically requires an infallible inner fold, and adoption is an explicit reported outcome that preserves the rest of the application's `Actions`. Independent models compare state and complete outputs after every generated step. |
| H02 | B2 | Every exported behavior has concrete event, send, phase, error, and birth types. | Pass. All `Behavior` implementations use associated concrete types; no catch-all envelope or registry exists. |
| H03 | B2 | No template uses `dyn`, `Any`, `TypeId`, downcasting, `unsafe`, serialization, or type-name dispatch for protocol composition. | Pass. Static source scan is clean; the only `type_name` use is a test assertion. |
| H04 | B5, B6 | A topology-transparent wrapper preserves its inner `BehaviorBase`, protocol, birth algebra, initialization effects, and order. | Pass. `Stash`, timing wrappers, watch/monitor, shutdown wrappers, and termination propagation project the inner base; equality constraints guard nominal role resolution. Composition and initialization tests exercise nested orders. |
| H05 | B4, B5 | Each standalone behavior that authors a direct topology exposes itself as `BehaviorBase<Base = Self>`. | **Failed → fixed.** `ProxyWithParent`, `WorkerPoolWithParent`, and `KeyedWorkerPoolWithParent` lacked the projection. `runtime_contracts::every_topology_owner_exposes_itself_as_its_behavior_base` now proves proxy, fixed, dynamic, FIFO, and keyed owners. |
| H06 | B4, B5 | A topology-changing composition cannot inherit an inner nominal child role whose birth algebra it replaced. | Pass. `ResolveChildOccurrence` requires exact protocol and birth equality. `Supervise` may expose its application base for inspection, but its proxy birth rewrite prevents stale role resolution; raw structural positions resolve against the running wrapper. |
| H07 | B4, P1 | Every production `ChildDelivery`, `ObserveChild`, `ObserveCreation`, `ShutdownChild`, or `ChildTermination` emitter owns the matching direct occurrence in its `Birth`. | Pass after H05. All such emissions are confined to proxy/supervision/pool/lifecycle owners with the corresponding child leaf; occurrence propagation is tested through nested wrappers. |
| H08 | B3, B4 | A standalone `MessageAdapter` with `NoBirths` cannot emit creator-local child delivery. | **Failed → fixed.** `DeliveryRoute` no longer accepts `ChildRoute`; a compile-fail example rejects the foreign route before runtime. The adapter retains logical, established, and closed mixed delivery modes only. |
| H09 | A2, B3, P6, P7 | Every delivery destination is supplied by configuration, a received message, discovery membership, or stable-proxy policy—not fabricated from child correlation. | Pass. Customer-bearing routing, workflow, operations, persistence, pool, supervision, and discovery messages retain their supplied `DeliveryRoute`; stable internal destinations remain explicit `Recipient` values. The sole production `Recipient::global(address)` constructs the deliberately stable dynamic-proxy identity from a committed proxy creation result. |
| H10 | B3, P7 | A received or configured exact endpoint is never weakened to a logical recipient before delivery, observation, or shutdown. | Pass. Exact modes retain `EstablishedRecipient`/`EstablishedActor` and emit `EstablishedDelivery`, `ObserveEstablished`, or `ShutdownEstablished`; `ReplyRoute` retains mixed alternatives and its interpreter visits them in original order. No exact-to-logical conversion exists. |
| H11 | B7, P3 | A rejected `EstablishedCreation` cannot produce a recipient, actor, child route, birth, or restart-success fact. | Pass. The rejected variant owns only nonce, kind, and `CreationRejection`; independent established-creation models cover allocation through binding failure. |
| H12 | B3, B4, P1 | After a named child commits, callers can retain both its exact incarnation and its creator-local role/nonce without reconstructing either. | **Failed → fixed.** `established_child` now returns `EstablishedChild<C, Role>`, a named product of `ChildRoute` and `EstablishedActor`; rejection yields neither. Its `shutdown_target` method selects a heterogeneous plan branch from the retained role. |
| H13 | B4, P3 | Child observation and shutdown preserve the declared occurrence when equal child protocols appear more than once. | Pass. All local lifecycle request types carry `Occurrence`; compile-fail tests reject head/tail or nominal-role substitution. |
| H14 | B2, B4 | A shutdown plan accepts only child behaviors whose exact event algebra owns `ShutdownRequested`. | Pass. Homogeneous and heterogeneous coordinator compile-fail tests reject non-shutdown-capable children; request interpretation uses the resolved concrete child. |
| H15 | B4, P4 | Proxy reports retain the final parent event path through an outer shutdown transformation for every proxy-owning template. | **Failed → fixed.** The reporting path is generic. Action-interpreted outer-`StopOnShutdown` tests execute creation and stop reports for application-owned supervision, standalone fixed supervision, both delayed forms, dynamic supervision, FIFO pools, and keyed pools. None supplies a structural path at the composition call. |
| H16 | B2, P4 | Every timer, observation, creation, shutdown, and parent-report request names the exact structural return path accepted by the enclosing event sum. | Pass. `runtime_contracts` proves request/fact duals and nested path injection; send-product interpretation tests exercise each lane at the same path exactly once. |
| H17 | B6, P5 | Root shutdown can reach a `FinalizeOnShutdown`, retaining its final sends, creations, and stop result. | Pass. The finalizer or coordinator is placed directly at the root; algebra tests prove delivery to the finalizer and full action preservation. No guardian alias selects that policy indirectly. |
| H18 | B3, P3 | Recurring logical watch and exact-once correlated monitoring remain distinct laws. | Pass. `Watch` emits `ObservePeer` and accepts every matching logical stop fact, including later incarnations. `TerminationMonitor` owns the requested/observing/terminal state sum; its established-target form emits `ObserveEstablished` and correlates the complete fact algebra by `ObservationId`. |
| H19 | B3, B7, P3 | Termination monitoring represents requested, observing, rejected/cancelled, observed, and already-consumed relationships without correlated flags. | Pass. `TerminationObservation` is exhaustive; exact monitor model/property tests prove single terminal consumption and controlled-error atomicity. |
| H20 | B3, B4 | Termination propagation chooses either an occurrence-aware local child or an explicitly late-bound logical peer, never an inferred destination. | Pass. `ChildTermination<A, O>` and `PeerTermination<A>` are distinct target types with distinct request effects. |
| H21 | A3, B7, P3 | Supervision distinguishes replacement request, installation attempt, committed incarnation, rejection, stale result, and retirement. | Pass. `Incarnation` and ownership folds use exhaustive phase enums and explicit creation kinds; independent supervision models and exhaustive/property suites compare the full sequence behavior. |
| H22 | A3, P3 | `Restarted` or replacement success is reported only after a replacement-designated creation commits. | Pass. Proxy reports a request separately, checks `CreationKind::Replacement`, and emits committed resolution only after installation success; failure remains typed. |
| H23 | B7, P3 | Stale and duplicate lifecycle/timer facts cannot be reinterpreted as fresh success or consume another relationship. | **Failed → fixed.** Timer reactions are now statically infallible, so a matching generation has one total consume-and-react transition. A later audit found consumption hidden inside `debug_assert!`, which made deadline, one-shot, periodic, and receive-timeout accept duplicates in optimized builds. Acceptance now validates and consumes in the production guard; the redundant preflight helpers were removed. Duplicate lifecycle facts are returned through exact typed errors; stale timer facts remain inert. Debug and optimized regressions, wrapper-order properties, and stack fuzzing exercise these cases. |
| H24 | B1, B2 | Named multi-lane send products preserve every lane exactly once and in their documented structural order. | Pass. Each product has explicit `SendEffects`, `SendsFor`, and `InterpretSends`; runtime-contract and cross-lane tests record complete traces. |
| H25 | A2, B3, P6 | A dynamic supervisor exposes the stable proxy as the returned logical child identity, never the replaceable worker incarnation. | **Failed → fixed.** Proxy and worker creation facts are retained and joined in either arrival order. Exactly one `Started` is produced only after both have committed. It returns `Recipient<Proxy<C>>`; later worker replacement stays local to that proxy. |
| H26 | B1, P3 | Time is always a typed input or schedule request; no fold reads wall-clock time or sleeps. | Pass. Production transitions consume `Instant` carried by lifecycle facts or `TimerElapsed`, and emit `ScheduleAt`/`ScheduleAfter`; `Instant::now` occurrences are test setup. |
| H27 | B2 | Semantic alternatives entering transition logic are sums, not booleans. | **Failed → fixed.** Circuit-breaker `Succeeded`/`Failed` messages were collapsed into `succeeded: bool`; the private `Completion::{Succeeded, Failed}` sum now preserves the domain through the helper boundary. Query predicates remain ordinary booleans. |
| H28 | B2, B7 | `Option<T>` denotes one value or absence, not overlapping lifecycle phases or correlated capabilities. | Pass after targeted inspection. Dynamic child, proxy incarnation, pool slot, breaker, lease, workflow, watch, and monitor phases use enums. Remaining options are exact absence queries, optional independent metadata, or optional one-effect outputs. |
| H29 | B1, B6 | Wrapper reactions cannot drop inner sends or creations when selecting `Goto` or `Stop`. | Pass. Wrappers transform the complete action product with `map_sends`/named reconstruction; terminal and initialization tests assert retained creates, sends, and verdicts. |
| H30 | A3, P1 | No actor address or exact endpoint is derived from nonce arithmetic, sequence position, timing, or address reuse. | Pass. `From<u64>` in fleet code creates configured local nonces only; exact addresses enter exclusively through interpreter facts. |
| H31 | B7 | Invalid configuration, overlap, exhaustion, unknown targets, and interpreter rejection remain typed results rather than production panics. | **Failed → fixed.** The conservation pass found errors that named only a key, nonce, or reason while consuming the remaining owned input. Machine, router policies, circuit breaker, rate limiter, priority queue, sequencer, lease, acknowledgements, correlation, task, barrier, workflow, health, readiness, presence, registry, pub-sub, supervision, and pools now return the complete rejected command or lifecycle fact. The only production `expect` is the proved `positive capacity + full buffer => oldest value exists` invariant. |
| H32 | A4, B6, P2 | Any ordering relied upon beyond actor-model law is declared as Bombay policy and tested at the interpreter boundary. | Pass. Create-before-dependent-send/request and wrapper initialization order are documented as policy; send products define their own deterministic interpretation order without claiming it as an Agha guarantee. |
| H33 | B2, B3, P7 | Every genuine customer-passing template accepts logical, exact, or deliberately mixed reply capabilities without allowing the route protocol to disagree with the reply message. | **Failed → fixed.** All customer fields now carry a `Route: DeliveryRoute<P>`; dynamic supervision projects `P` from `DeliveryRouteProtocol`. A catalogue compile matrix instantiates every affected family with `EstablishedRecipient` and `ReplyRoute`, protocol mismatch is compile-fail, and logical-only recursive protocol tests remain finite. |
| H34 | B1–B3, B7, P4, P8 | Homogeneous and heterogeneous shutdown coordinators can receive their validated plans after committed child creation without out-of-band mutation, flags, plan substitution, lost early shutdown, or repeated installation. | **Failed → fixed.** `ShutdownState` is the complete lifecycle sum. A topology owner emits `ReportShutdownPlan` through `Actions`; the interpreter constructs the exact outer `InstallShutdownPlan<P>` event from the carried typed return path. Unit, independent model/property, fuzz, and compile-fail coverage exercise both plan families, duplicate installation, early shutdown, stale stops, ordered phases, and empty-plan termination. |
| H35 | B2–B4, B7, P1, P3 | A standalone fixed supervisor owns only supervision, while application command selection is expressed by ordinary typed actor composition. | **Failed → fixed.** The former `SupervisedWorkers` policy engine combined fixed ownership with a selector but owned no distinct lifecycle law. It and its availability-policy surface were removed. `Supervisor` now implements the concrete fixed-fleet fold directly; an application behavior or routing actor owns command selection and its own typed rejection law. |
| H36 | B1–B4, B6–B7, P1–P4, P8 | Creation-dependent shutdown planning retains a single typed implementation that both direct applications and generic frameworks can carry. | **Failed → fixed.** `ChildShutdownPlan` owns the distinct join from committed direct-child creation facts to one reported heterogeneous plan. Its direct builder remains the semantic implementation. `DeclareShutdownPhase` and `FinishShutdownPhases` expose only associated output types, so a framework neither names nor copies the hidden availability and phase proofs. |
| H37 | B1–B2, B6–B7 | A reusable actor template is retained only when it owns a distinct state-transition or typed event/effect transformation law. | **Failed → fixed.** Guardian aliases, established-watch policy aliases, feature aliases, selector-policy supervisors, backoff parameterization wrappers, and recipe functions were removed. The child shutdown planner remains because it owns creation-fact correlation and plan reporting. `Watch` and exact-once monitoring, application and standalone backoff, FIFO and keyed pools, and homogeneous and heterogeneous coordinators remain separate because their recurrence, input, assignment, or typed effect transitions differ. |
| H38 | B1–B4, B6–B7, P3 | Expected supervised-command unavailability is observable after mailbox admission and never collapses into an opaque actor crash. | **Failed → fixed.** `ProxyCommand::Forward` carries a concrete logical return recipient. Startup, vacant, replacement, and shutdown phases return `ProxyUnavailable` with the exact phase and command through `Actions`. Pools join returned assignments with worker-stop and replacement facts in both orders, including restart exhaustion, without duplicating or losing the customer outcome. |
| H39 | B1–B2, P1–P4 | A generic consumer can carry the child-shutdown builder without naming hidden typestate or implementing a second planner. | **Failed → fixed.** Generic full-interpreter tests declare and finish phases using only the two associated-output operation traits; direct syntax, reverse order, early shutdown, and all existing typed failures retain the same builder implementation. |
| H40 | B1–B2, P1–P3 | A composition owner exposes every transitive intentional logical protocol host statically, preserving duplicate occurrences and excluding exact-only endpoints. | **Failed → fixed.** `LogicalHostRequirements` supplies one closed owner-authored protocol product. Generic consumer tests recurse over the product, prove duplicate positions independently, and require no registry, `dyn`, `Any`, `TypeId`, envelope erasure, or runtime lookup. |

The repairs require no new Behavior Core algebra, runtime registry, dynamic
type, forwarding actor, or address reconstruction. The verification record
below is authoritative only when every listed gate is green on the same tree.

## Complete template coverage

The hypotheses above were applied to every exported behavior family, not just
the templates implicated by the initial blockers.

| Family | Behavior templates audited | Principal hypotheses |
|---|---|---|
| Base composition | `Machine`, `MessageAdapter`, `MessageAdapterWithRoute` | H01–H03, H08–H10, H24, H27–H31 |
| Transparent state/lifecycle wrappers | `Stash`, `StopOnShutdown`, `FinalizeOnShutdown`, `Watch`, `TerminationMonitorWith`, `PropagateTermination` | H04, H06, H13, H16–H20, H23–H24, H29 |
| Shutdown ownership | `ShutdownCoordinator`, `HeterogeneousShutdownCoordinator` | H07, H12–H17, H23–H24, H29–H32, H34, H36–H37 |
| Lifecycle task | `Task` | H01–H03, H09–H10, H24, H27–H31, H33 |
| Supervision | `ProxyWithParent`, `SuperviseWithParent`, `SupervisorWithParent`, `BackoffSuperviseWithParent`, `BackoffSupervisorWithParent`, `DynamicSupervisorWithParent` and their truthful direct aliases | H05–H07, H10, H13, H15–H16, H21–H25, H28–H33, H35, H37 |
| Worker pools | `WorkerPoolWithParent`, `KeyedWorkerPoolWithParent` and direct aliases | H05, H07, H09–H10, H13, H15–H16, H21–H24, H28–H31, H33 |
| Timing | `Deadline`, `OneShot`, `Periodic`, `ReceiveTimeout`, `Lease` | H01–H04, H10, H16, H23–H24, H26, H28–H33 |
| Routing | `Router` with all strategies, `WorkQueue`, `Buffer`, `PriorityQueue`, `OrderGate`, `Sequencer`, `Deduplicator`, `RateLimiter`, `CircuitBreaker`, `Correlator`, `Acknowledgements` | H01–H03, H09–H10, H23–H24, H27–H31, H33 |
| Discovery | `Registry`, `Resolver`, `Topic`, `PubSub`, `Presence` | H01–H03, H09–H10, H16, H23–H24, H26–H31, H33 |
| Workflow | `Latch`, `Barrier`, `Workflow` | H01–H03, H09–H10, H23–H24, H27–H31, H33 |
| Operations/persistence | `Configuration<FeatureSet<_>>`, `Health`, `Readiness`, `Cache` | H01–H03, H09–H10, H23–H24, H27–H31, H33, H37 |

Routing strategies are policy values owned by `Router`, not actors with their
own effect boundary. Redundant aliases for the same observation and shutdown
state machines were removed rather than counted as separate templates.

## Adversarial law coverage

The audit does not count a constructor smoke test as proof of a state-machine
law. The following independent suites model state after every generated step:

| Surface | Independent or adversarial evidence |
|---|---|
| Creation and exact capabilities | `established_creation_model`, `heterogeneous_births`, `birth_sequences` |
| Machine and replay | `fsm_properties`, `exhaustive`, `fsm_sequences` |
| Stash and wrapper lane isolation | `stash_properties`, `two_buffer`, `cross_lane`, `stack_sequences` |
| Watch, monitoring, and terminal propagation | `exact_termination_model`, `terminal_fact_model`, `catalogue_sequences` |
| Fixed, delayed, and application-composed supervision | `supervision_model`, `supervision_ownership`, `exhaustive`, `supervision_sequences` |
| FIFO and keyed worker pools | `worker_pool_model`, `keyed_worker_pool`, `pool_sequences` |
| Homogeneous and heterogeneous shutdown | `shutdown_model`, `heterogeneous_shutdown`, `shutdown_plan_sequences`, plus coordinator compile-fail and ordering tests |
| Versioned operations and cache | `catalogue_invariants` models configuration, feature-set normalization, readiness, health tombstones, and LRU ownership |
| Routing and correlation | `catalogue_models`, `routing_invariants`, and `correlation_invariants` model stable priority, bounded ownership, token arithmetic, round robin, FIFO worker availability, sequencing, deduplication, ordering, correlation, and acknowledgements |
| Time | `receive_timeout`, `timing_invariants`, `receive_timeout_sequences`; timer-wrapper composition and initialization are attacked by `properties`, `cross_lane`, and `stack_sequences` |
| Workflow | `workflow_invariants` models barrier generations, latch single release, dependency activation, and terminal rejection; `catalogue_sequences` fuzzes workflow inputs |
| Discovery | registry and topic generated models live in `catalogue_invariants`; presence is generation-fuzzed by `catalogue_sequences`; resolver and keyed publication retain focused atomic owner tests because their immutable/snapshot state spaces are already exhaustively covered there |

Each model uses different vocabulary and ordinary collections. It compares
observable state and complete owned outputs after every operation, including
rejection and stale input. Fuzz targets are retained as a separate layer; they
do not replace the reference models.

## Verification record — 2026-08-24 working tree

- `cargo check --workspace --all-targets`: pass without Rust warnings.
- Actor rustdoc and compile-fail tests: 24 passed.
- `cargo nextest run --workspace`: 490 passed, none skipped. The exact-monitor
  property test was transiently marked leaky during a concurrent Nix toolchain
  build and passed alone without a leak marker.
- The supervision, pool, and shutdown-plan fuzz targets completed 5,000
  executions each under the locked Fenix input's nightly sanitizer toolchain.
- `nix flake check` and `nix flake check 'path:.'`: all seven checks pass,
  including optimized nextest, documentation, Rust and TOML formatting,
  dependency audit, and dependency policy. The explicit `path:` form includes
  the untracked catalogue audit in the evaluated source snapshot.

## Capability conclusions

The three recipient forms have non-overlapping authority:

| Capability | Meaning | Lawful use in templates |
|---|---|---|
| `Recipient<P>` | Logical protocol name | caller reply, discovery membership, transport name, or stable proxy identity |
| `ChildRoute<C, O>` | One role and nonce in the current creator's namespace | only a topology owner whose direct `Birth` proves `O` |
| `EstablishedRecipient<P>` / `EstablishedActor<C>` | One exact installed incarnation | exact delivery, observation, shutdown, or retained post-creation capability |
| `ReplyRoute<P>` | Closed logical-or-exact customer capability | one running template intentionally accepts both forms without conversion |

`EstablishedChild<C, O>` intentionally retains the latter two facts together.
It is an Actors-level named product over existing capabilities. It neither
allocates an actor nor extends the Behavior algebra.

Future changes must repeat the six-step provenance trace above. A test that
only inspects the emitted value is insufficient for creator-local effects: the
running emitter must also prove that its interpreter can lawfully resolve the
same occurrence.
