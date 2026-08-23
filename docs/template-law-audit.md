# Behavior Actors template-law audit

This is a proof-driven audit of every reusable `Behavior` implementation
exported by `bombay-behavior-actors`. It replaces the earlier routing-only
review, which incorrectly accepted creator-local delivery from a standalone
`MessageAdapter` and did not test `BehaviorBase` at the runtime resolution
boundary.

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
  `Ingress` path selected by the owning wrapper.
- **P5 — explicit root shutdown:** a `Guardian` chooses direct root stop or
  coordinated delegation; it does not discover a nested shutdown handler.
- **P6 — stable logical domains:** discovery membership, stable proxy identity,
  transport names, and caller-supplied reply addresses deliberately remain
  logical recipients.

## Hypotheses and verdicts

`Failed → fixed` means the hypothesis was false in the audited revision and
this change repairs it. A pass means both the public type surface and the
relevant fold/interpreter seam support the statement.

| ID | Source | Falsifiable hypothesis | Verdict and evidence |
|---|---|---|---|
| H01 | A1, B1 | Every template transition is a deterministic fold returning all effects in `Actions`. | Pass. Production scans find no runtime handle, task spawn, I/O, or direct interpreter call in a fold; catalogue and algebra tests assert complete actions. |
| H02 | B2 | Every exported behavior has concrete event, send, phase, error, and birth types. | Pass. All `Behavior` implementations use associated concrete types; no catch-all envelope or registry exists. |
| H03 | B2 | No template uses `dyn`, `Any`, `TypeId`, downcasting, `unsafe`, serialization, or type-name dispatch for protocol composition. | Pass. Static source scan is clean; the only `type_name` use is a test assertion. |
| H04 | B5, B6 | A topology-transparent wrapper preserves its inner `BehaviorBase`, protocol, birth algebra, initialization effects, and order. | Pass. `Stash`, timing wrappers, watch/monitor, shutdown wrappers, guardian, and termination/shutdown compositions project the inner base; equality constraints guard nominal role resolution. Composition and initialization tests exercise nested orders. |
| H05 | B4, B5 | Each standalone behavior that authors a direct topology exposes itself as `BehaviorBase<Base = Self>`. | **Failed → fixed.** `ProxyWithParent`, `WorkerPoolWithParent`, and `KeyedWorkerPoolWithParent` lacked the projection. `runtime_contracts::every_topology_owner_exposes_itself_as_its_behavior_base` now proves proxy, fixed, dynamic, FIFO, and keyed owners. |
| H06 | B4, B5 | A topology-changing composition cannot inherit an inner nominal child role whose birth algebra it replaced. | Pass. `ResolveChildOccurrence` requires exact protocol and birth equality. `Supervise` may expose its application base for inspection, but its proxy birth rewrite prevents stale role resolution; raw structural positions resolve against the running wrapper. |
| H07 | B4, P1 | Every production `ChildDelivery`, `ObserveChild`, `ObserveCreation`, `ShutdownChild`, or `ChildTermination` emitter owns the matching direct occurrence in its `Birth`. | Pass after H05. All such emissions are confined to proxy/supervision/pool/lifecycle owners with the corresponding child leaf; occurrence propagation is tested through nested wrappers. |
| H08 | B3, B4 | A standalone `MessageAdapter` with `NoBirths` cannot emit creator-local child delivery. | **Failed → fixed.** `DeliveryRoute` no longer accepts `ChildRoute`; a compile-fail example rejects the foreign route before runtime. The adapter retains logical and established delivery modes only. |
| H09 | A2, B3, P6 | Every logical `Delivery` destination is supplied by configuration, a received message, discovery membership, or the stable-proxy policy—not fabricated from child correlation. | Pass. Routing, workflow, operations, persistence, pool replies, and discovery retain their supplied `Recipient`; the sole production `Recipient::global(address)` constructs the deliberately stable dynamic-proxy identity from a committed proxy creation result. |
| H10 | B3 | A received or configured exact endpoint is never weakened to a logical recipient before delivery, observation, or shutdown. | Pass. Exact modes retain `EstablishedRecipient`/`EstablishedActor` and emit `EstablishedDelivery`, `ObserveEstablished`, or `ShutdownEstablished`; no exact-to-logical conversion exists. |
| H11 | B7, P3 | A rejected `EstablishedCreation` cannot produce a recipient, actor, child route, birth, or restart-success fact. | Pass. The rejected variant owns only nonce, kind, and `CreationRejection`; independent established-creation models cover allocation through binding failure. |
| H12 | B3, B4, P1 | After a named child commits, callers can retain both its exact incarnation and its creator-local role/nonce without reconstructing either. | **Failed → fixed.** `established_child` now returns `EstablishedChild<C, Role>`, a named product of `ChildRoute` and `EstablishedActor`; rejection yields neither. Its `shutdown_target` method selects a heterogeneous plan branch from the retained role. |
| H13 | B4, P3 | Child observation and shutdown preserve the declared occurrence when equal child protocols appear more than once. | Pass. All local lifecycle request types carry `Occurrence`; compile-fail tests reject head/tail or nominal-role substitution. |
| H14 | B2, B4 | A shutdown plan accepts only child behaviors whose exact event algebra owns `ShutdownRequested`. | Pass. Homogeneous and heterogeneous coordinator compile-fail tests reject non-shutdown-capable children; request interpretation uses the resolved concrete child. |
| H15 | B4, P4 | Proxy reports retain the final parent event path through an outer `Guardian` for every proxy-owning template. | Pass. `ProxyParentIngress<A, ParentPath>` is stored in every proxy birth; runtime-contract tests cover fixed, delayed, dynamic, FIFO, and keyed owners under `Guardian`. |
| H16 | B2, P4 | Every timer, observation, creation, shutdown, and parent-report request names the exact structural return path accepted by the enclosing event sum. | Pass. `runtime_contracts` proves request/fact duals and nested path injection; send-product interpretation tests exercise each lane at the same path exactly once. |
| H17 | B6, P5 | Root shutdown can reach an inner `FinalizeOnShutdown`, retaining its final sends, creations, and stop result. | Pass. `Guardian::coordinated` is the explicit composition; algebra tests prove delivery to the finalizer and full action preservation. |
| H18 | B3, P3 | Watch supports a deliberate late-bound logical peer and a separate exact-incarnation mode with explicit relationship correlation. | Pass. `Watch` emits `ObservePeer`; `EstablishedWatch` emits `ObserveEstablished` and filters the complete fact algebra by `ObservationId`. |
| H19 | B3, B7, P3 | Termination monitoring represents requested, observing, rejected/cancelled, observed, and already-consumed relationships without correlated flags. | Pass. `TerminationObservation` is exhaustive; exact monitor model/property tests prove single terminal consumption and controlled-error atomicity. |
| H20 | B3, B4 | Termination propagation chooses either an occurrence-aware local child or an explicitly late-bound logical peer, never an inferred destination. | Pass. `ChildTermination<A, O>` and `PeerTermination<A>` are distinct target types with distinct request effects. |
| H21 | A3, B7, P3 | Supervision distinguishes replacement request, installation attempt, committed incarnation, rejection, stale result, and retirement. | Pass. `Incarnation` and ownership folds use exhaustive phase enums and explicit creation kinds; independent supervision models and exhaustive/property suites compare the full sequence behavior. |
| H22 | A3, P3 | `Restarted` or replacement success is reported only after a replacement-designated creation commits. | Pass. Proxy reports a request separately, checks `CreationKind::Replacement`, and emits committed resolution only after installation success; failure remains typed. |
| H23 | B7, P3 | Stale and duplicate lifecycle/timer facts cannot be reinterpreted as fresh success or consume another relationship. | Pass. Exact IDs, nonces, incarnation numbers, and timer generations gate transitions; boundary, cross-lane, receive-timeout, and supervision properties cover redelivery and stale inputs. |
| H24 | B1, B2 | Named multi-lane send products preserve every lane exactly once and in their documented structural order. | Pass. Each product has explicit `SendEffects`, `SendsFor`, and `InterpretSends`; runtime-contract and cross-lane tests record complete traces. |
| H25 | A2, B3, P6 | A dynamic supervisor exposes the stable proxy as the returned logical child identity, never the replaceable worker incarnation. | Pass. `Started` is produced only from committed proxy creation and returns `Recipient<Proxy<C>>`; worker replacement stays local to that proxy. |
| H26 | B1, P3 | Time is always a typed input or schedule request; no fold reads wall-clock time or sleeps. | Pass. Production transitions consume `Instant` carried by lifecycle facts or `TimerElapsed`, and emit `ScheduleAt`/`ScheduleAfter`; `Instant::now` occurrences are test setup. |
| H27 | B2 | Semantic alternatives entering transition logic are sums, not booleans. | **Failed → fixed.** Circuit-breaker `Succeeded`/`Failed` messages were collapsed into `succeeded: bool`; the private `Completion::{Succeeded, Failed}` sum now preserves the domain through the helper boundary. Query predicates remain ordinary booleans. |
| H28 | B2, B7 | `Option<T>` denotes one value or absence, not overlapping lifecycle phases or correlated capabilities. | Pass after targeted inspection. Dynamic child, proxy incarnation, pool slot, breaker, lease, workflow, watch, and monitor phases use enums. Remaining options are exact absence queries, optional independent metadata, or optional one-effect outputs. |
| H29 | B1, B6 | Wrapper reactions cannot drop inner sends or creations when selecting `Goto` or `Stop`. | Pass. Wrappers transform the complete action product with `map_sends`/named reconstruction; terminal and initialization tests assert retained creates, sends, and verdicts. |
| H30 | A3, P1 | No actor address or exact endpoint is derived from nonce arithmetic, sequence position, timing, or address reuse. | Pass. `From<u64>` in fleet code creates configured local nonces only; exact addresses enter exclusively through interpreter facts. |
| H31 | B7 | Invalid configuration, overlap, exhaustion, unknown targets, and interpreter rejection remain typed results rather than production panics. | Pass. Public constructors and folds expose concrete errors. The only production `expect` is Buffer's documented private invariant: validated positive capacity plus the full/drop-oldest branch proves a non-empty queue. |
| H32 | A4, B6, P2 | Any ordering relied upon beyond actor-model law is declared as Bombay policy and tested at the interpreter boundary. | Pass. Create-before-dependent-send/request and wrapper initialization order are documented as policy; send products define their own deterministic interpretation order without claiming it as an Agha guarantee. |

Result: 28 hypotheses passed in the initial revision and four failed. After
the repairs in this change, all 32 pass. No failure required a new actor
template or new Behavior Core algebra.

## Complete template coverage

The hypotheses above were applied to every exported behavior family, not just
the templates implicated by the initial blockers.

| Family | Behavior templates audited | Principal hypotheses |
|---|---|---|
| Base composition | `Machine`, `MessageAdapter`, `MessageAdapterWithRoute` | H01–H03, H08–H10, H24, H27–H31 |
| Transparent state/lifecycle wrappers | `Stash`, `StopOnShutdown`, `FinalizeOnShutdown`, `Guardian`, `WatchWith`, `TerminationMonitorWith`, `PropagateTermination` | H04, H06, H13, H16–H20, H23–H24, H29 |
| Shutdown ownership | `ShutdownCoordinator`, `TreeShutdown`, `HeterogeneousShutdownCoordinator` | H07, H12–H17, H23–H24, H29–H32 |
| Lifecycle task | `Task` | H01–H03, H09, H24, H27–H31 |
| Supervision | `ProxyWithParent`, `SuperviseWithParent`, `SupervisorWithParent`, both backoff forms, `DynamicSupervisorWithParent` and their direct aliases | H05–H07, H13, H15–H16, H21–H25, H28–H32 |
| Worker pools | `WorkerPoolWithParent`, `KeyedWorkerPoolWithParent` and direct aliases | H05, H07, H09, H13, H15–H16, H21–H24, H28–H31 |
| Timing | `Deadline`, `OneShot`, `Periodic`, `ReceiveTimeout`, `Lease` | H01–H04, H16, H23–H24, H26, H28–H32 |
| Routing | `Router` with all strategies, `WorkQueue`, `Buffer`, `PriorityQueue`, `OrderGate`, `Sequencer`, `Deduplicator`, `RateLimiter`, `CircuitBreaker`, `Correlator`, `Acknowledgements` | H01–H03, H09, H23–H24, H27–H31 |
| Discovery | `Registry`, `Resolver`, `Topic`, `PubSub`, `Presence` | H01–H03, H09, H16, H23–H24, H26–H31 |
| Workflow | `Latch`, `Barrier`, `Workflow` | H01–H03, H09, H23–H24, H27–H31 |
| Operations/persistence | `Configuration`/`Features`, `Health`, `Readiness`, `Cache` | H01–H03, H09, H23–H24, H27–H31 |

Routing strategies are policy values owned by `Router`, not actors with their
own effect boundary. Type aliases such as `Link`, `Reaper`, and
`LifecyclePublisher` were audited through their concrete `WatchWith` or
`TerminationMonitorWith` implementation.

## Capability conclusions

The three recipient forms have non-overlapping authority:

| Capability | Meaning | Lawful use in templates |
|---|---|---|
| `Recipient<P>` | Logical protocol name | caller reply, discovery membership, transport name, or stable proxy identity |
| `ChildRoute<C, O>` | One role and nonce in the current creator's namespace | only a topology owner whose direct `Birth` proves `O` |
| `EstablishedRecipient<P>` / `EstablishedActor<C>` | One exact installed incarnation | exact delivery, observation, shutdown, or retained post-creation capability |

`EstablishedChild<C, O>` intentionally retains the latter two facts together.
It is an Actors-level named product over existing capabilities. It neither
allocates an actor nor extends the Behavior algebra.

Future changes must repeat the six-step provenance trace above. A test that
only inspects the emitted value is insufficient for creator-local effects: the
running emitter must also prove that its interpreter can lawfully resolve the
same occurrence.
