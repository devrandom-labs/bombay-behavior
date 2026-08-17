# Actor-template semantic and verification audit

This is the verification index for every reusable behavior template currently
exported by `bombay-behavior-actors`. It states the property set that must hold,
the source of each law, and the test layer that checks it. A passing test suite
is not itself the property inventory; this table is the inventory against which
the suite is reviewed.

## Claim classification and research basis

Only the actor transition nucleus is an actor-model law: one accepted
communication is processed at a time and may produce communications, fresh
actors, and a next behavior. The separation of a statically known [`Protocol`]
signature from its `Behavior` fold is a Bombay/Rust derivation that prevents
recipient types from recursively demanding the destination's complete effect
algebra. It does not add an actor-model operation.

Primary sources reviewed for applicable external properties:

- Agha, Mason, Smith, and Talcott, [*A Foundation for Actor
  Computation*](https://osl.cs.illinois.edu/publications/journals/jfp/AghaMST97.html):
  actor configurations, acquaintances, communication, fresh allocation, and
  behavior replacement. It does not prescribe Bombay supervision, queues,
  timers, caches, workflows, or Rust protocol representation.
- Karger et al., [*Consistent Hashing and Random
  Trees*](https://doi.org/10.1145/258533.258660): deterministic key placement
  and limited disruption under membership changes.
- Thaler and Ravishankar, [*Using Name-Based Mappings to Increase Hit
  Rates*](https://www.microsoft.com/en-us/research/wp-content/uploads/2017/02/HRW98.pdf):
  highest-random-weight selection, deterministic agreement, balance, and
  minimal disruption.
- Lamport, [*Time, Clocks, and the Ordering of Events in a Distributed
  System*](https://lamport.azurewebsites.net/pubs/time-clocks.pdf): causal
  partial order and the fact that a distributed total order requires an
  explicit ordering construction. Bombay's local sequence/watermark policies
  are derived policies, not Lamport-clock implementations.
- IETF [RFC 2212](https://datatracker.ietf.org/doc/html/rfc2212) and
  [RFC 2216](https://www.ietf.org/rfc/rfc2216.html): positive token rate and
  bucket depth, bounded accumulation, and burst capacity. Bombay's
  `RateLimiter` is only a discrete token-admission fold driven by typed refill
  evidence; it does not claim continuous-time or network-QoS conformance.
- Erlang/OTP's official [Supervisor Behaviour
  specification](https://www.erlang.org/doc/system/sup_princ.html): reference
  terminology for one-for-one, one-for-all, rest-for-one, restart intensity,
  and child ordering. These are not Agha laws, and Bombay's proxy, freshness,
  provenance, and realization rules remain explicit Bombay policy.

All other properties below are derived constructions or deliberate Bombay
policies. Where no primary source determines a choice, the implementation must
not present that choice as a research guarantee.

## Complete property-to-test matrix

| Template | Required properties | Classification | Verification evidence |
|---|---|---|---|
| `Activate` / `Active` | Initialization occurs exactly once before mailbox input; initialization actions remain paired with the active behavior; raw and repeatedly initialized definitions do not compile. | Bombay lifecycle policy | activation unit tests, `init_contract`, composition tests, compile-fail rustdoc |
| `Machine` | One event produces one deterministic move/actions pair; stay, phase change, stop, replay suffix restoration, and controlled failure are distinct; no event is duplicated or lost. | Actor nucleus plus Bombay FSM derivation | algebra unit tests, exhaustive/model/property tests, `fsm_properties`, FSM fuzz target |
| `MessageAdapter` | Empty initialization; one input invokes one function pointer exactly once; one ordinary delivery is emitted to the configured protocol; continuation is preserved; mapper panic follows ordinary fold panic semantics. | Bombay protocol-composition derivation | module unit tests, concrete supervisor/pool root tests, 20-family recursive reply matrix |
| `Stash` | Held inputs retain FIFO order; release delivers its trigger before draining; stop skips draining; failed replay restores the unprocessed suffix; wrapper lanes and initialization are preserved. | Bombay selective-receive policy | algebra tests, independent model, exhaustive/properties, stash fuzz target |
| `Guardian` | Inner initialization and user transitions are preserved exactly; shutdown selects normal stop without hidden effects; wrapper order preserves all lanes and errors. | Bombay lifecycle policy | guardian unit/error tests and both `Guardian<Watch<_>>` / `Watch<Guardian<_>>` composition orders |
| `Watch` / `Link` | One exact observation request; unrelated/stale peer facts are forwarded or inert according to lane ownership; a matching fact invokes one reaction; reaction actions/errors remain complete. | Bombay observation/link policy | algebra composition tests, lifecycle model, runtime lane contract |
| `TerminationMonitor` aliases | Awaiting and consumed observations are distinct; only the exact peer is consumed; a rejected reaction does not commit consumption. | Bombay lifecycle-publication policy | matching, unrelated, duplicate/consumed, and reaction-error unit paths |
| Shutdown wrappers | Inner initialization is preserved; typed shutdown is routed once; stop/finalize reactions preserve required effects and controlled failures. | Bombay shutdown policy | algebra composition and independent shutdown model |
| `ShutdownCoordinator` / `TreeShutdown` | Empty/duplicate/cyclic plans reject atomically; phases follow declared dependency order; every current child must stop before advancement; duplicate/stale facts are inert; rejection preserves phase. | Bombay coordination policy | six boundary/state tests plus shutdown model |
| `Task` | Pending, completed, and cancelled are distinct; terminal ownership is delivered once; terminal transition stops atomically; invalid terminal repetition cannot become another success. | Bombay task policy | completion, cancellation, and terminal-state unit paths |
| `Proxy` | Stable proxy and fresh incarnation are distinct; one replacement attempt is in flight; creation provenance is explicit; only matching committed creation becomes routable; stale/rejected facts cannot report restart success. | Fresh-allocation law plus Bombay stable-identity derivation | algebra, independent model, exhaustive/property/fuzz, runtime lane contract |
| `Supervisor` | Topology is validated before existence; restart strategy candidate sets are exact; eligibility and budget are atomic; retired slots are skipped; creation success/rejection and replacement provenance remain distinct. | OTP terminology plus Bombay policy | algebra, fleet/incarnation/budget unit tests, independent model, properties/fuzz |
| `BackoffSupervisor` | Attempt numbers are one-based and checked; delays are bounded; replacement is withheld until the exact timer; stale/colliding timers are typed; repeated failure advances generation without wrapping. | Bombay timed-supervision policy | backoff and backoff-supervisor boundary tests |
| `DynamicSupervisor` | Installing, available, stopping, replacing, and retired are exhaustive; request acceptance differs from committed realization; duplicate rejection returns owned child; stop/replace wait for exact facts; query is observational; failed creation cannot report birth/restart. | Bombay dynamic-topology policy | dynamic-supervisor state tests and concrete recursive-root adapter test |
| `WorkerPool` | Positive unique topology; FIFO backlog; one assignment token per attempt; accepted ownership is retained until completion/interruption; stale completion rejects; worker stop follows configured interruption policy; admission/retry is atomic across application clones. | Bombay pool policy | unit, independent ownership model, exhaustive/property/fuzz, recursive-root adapter test |
| `KeyedWorkerPool` | All worker-pool laws plus persistent key affinity; rebalance alone changes an established binding; already accepted jobs retain their slot; retired/unavailable affinity is explicit. | Bombay keyed-pool policy | dedicated 14-test suite, independent model, property/fuzz |
| `Router<RoundRobin>` | Membership is ordered/idempotent; each route advances once; removal repairs the cursor; empty membership returns ownership. | Bombay routing policy | router unit boundaries |
| `Router<Broadcast>` | Membership is ordered/idempotent; one delivery per member in membership order; empty membership returns ownership. | Bombay routing policy | router unit boundaries |
| `Router<LeastLoaded>` | Selection requires current typed load evidence; stale/unknown/conflicting evidence rejects atomically; ties use membership order. | Bombay evidence policy | router unit boundaries |
| `Router<ConsistentHash>` | Same key/membership gives the same target; removal moves only keys owned by the removed member; membership/token conflicts reject without mutation. | Karger property adapted as Bombay local policy | deterministic/removal unit model |
| `Router<RendezvousHash>` | Highest deterministic weight wins; clients with equal membership/tokens agree; removal disturbs only keys selecting that member; conflicting evidence rejects. | Thaler–Ravishankar property adapted as Bombay local policy | deterministic/conflict/remapping unit model |
| `WorkQueue` | Available workers and waiting values are FIFO; a capability is consumed once; bounded overflow returns the unaccepted value; zero capacity is explicit. | Bombay queue policy | work-queue FIFO/overflow tests |
| `PriorityQueue` | Greater priority wins; equal priority is stable FIFO; full and empty differ; insertion sequence exhaustion rejects before ownership/state mutation. | Bombay queue policy | three exhaustive boundary tests |
| `Buffer` | Positive capacity; FIFO release; empty release is explicit; reject-newest, drop-oldest, and drop-newest preserve or return every owned value exactly once. | Bombay buffering policy | all overflow policies and boundary tests |
| `CircuitBreaker` | Closed idle/awaiting, open, and probe available/awaiting are exhaustive; threshold opens; open denies; exact reset permits one probe; stale attempt/timer facts are inert; counters never wrap. | Bombay circuit-breaker policy | threshold, denial/probe, stale evidence, and both exhaustion tests |
| `RateLimiter` | Capacity and requested token counts are positive; admission subtracts exactly; insufficient/over-capacity rejection returns ownership; refill saturates at capacity without overflow. | RFC token-bucket analogy plus explicit Bombay discrete policy | admission/rejection and saturation tests |
| `Correlator` | Pending, resolved, and cancelled are distinct; one terminal reply; unknown/stale reply returns its value; cancellation cannot become resolution. | Bombay correlation policy | resolve/duplicate/cancel/unknown tests |
| `Acknowledgements` | Required membership is normalized; acknowledgements are idempotent; completion occurs exactly once after the complete set; cancellation and rejection are distinct terminal outcomes; stale input is inert/typed. | Bombay acknowledgement policy | membership/completion/rejection/cancellation tests |
| `Sequencer` | Expected position is monotonic; gaps retain values; contiguous suffix releases in order; stale/duplicate offers return ownership; maximum sequence exhausts without wrap. | Lamport-informed ordering distinction plus Bombay policy | unit plus independent gap-map property sequences |
| `OrderGate` | Watermark is monotonic; held keys release in key order; future-open keys deliver immediately; duplicate/stale opening is atomic; duplicate holds preserve ownership policy. | Bombay watermark policy | unit plus independent watermark-map properties |
| `Deduplicator` | Positive bounded FIFO retention; duplicate returns value without refreshing age; eviction is explicit; evicted keys may re-enter. | Bombay deduplication policy | unit plus independent FIFO-window properties |
| `Registry` | Bind/unbind/lookup outcomes are exhaustive; duplicate/conflicting mutation rejects atomically; stale unbind is typed; insertion order is stable. | Bombay mutable-discovery policy | mutation and lookup boundary tests |
| `Resolver` | Definition is immutable and duplicate-free; construction rejection preserves the source definition; lookup reports found/missing without mutation authority. | Bombay immutable-discovery policy | construction and lookup tests |
| `Topic` | Subscription is idempotent and ordered; publication snapshots membership and emits once per subscriber; empty publication returns ownership. | Bombay pub/sub policy | membership/order/empty tests |
| `PubSub` | Topic and subscriber insertion order is stable/idempotent; known-empty and unknown topics are distinct; undelivered publication is returned. | Bombay keyed pub/sub policy | keyed membership/publication tests |
| `Presence` | Present, expired/tombstone, and version/generation evidence are explicit; identical evidence is idempotent; stale/conflicting evidence and timer collision reject atomically; exhaustion never wraps. | Bombay timed-presence policy | four lifecycle/evidence/exhaustion tests |
| `Deadline` | Initialization schedules once; only exact timer identity/generation fires; stale timer is consumed locally; inner actions/errors and wrapper orders remain complete. | Bombay timer policy | algebra/composition, timer model/properties/fuzz |
| `ReceiveTimeout` | User activity rearms with a fresh generation; stale timers cannot fire/rearm; exhaustion retires only the timer and preserves the inner fold. | Bombay timer policy | algebra, dedicated exhaustion regression, independent model/properties/fuzz |
| `OneShot` | Initialization schedules one generation; exactly matching evidence reacts once; wrong/stale evidence is inert; consumption cannot repeat. | Bombay timer policy | one-shot plus shared timer-domain tests |
| `Periodic` | Initialization schedules; each matching tick reacts once and rearms only on continuation; stop does not rearm; stale/consumed generations cannot fire. | Bombay timer policy | periodic plus shared timer-domain tests |
| `Lease` | Vacant, held with holder/generation, and exhausted are exhaustive; acquire/renew/release preserve provenance; wrong holder and stale expiry differ; matching expiry releases; generation never wraps. | Bombay lease policy | three state/boundary tests |
| `Cache` | Positive capacity; deterministic LRU recency; hit refreshes; replacement/removal/eviction returns displaced ownership; zero capacity rejects before ownership. | Bombay cache policy | three recency/ownership/configuration tests |
| `Workflow` | Definition is a finite non-empty DAG; duplicate/unknown/cyclic definitions reject; readiness follows prerequisites; each step activates once; failure blocks descendants; duplicate completion is atomic. | DAG construction plus Bombay workflow policy | graph rejection, diamond order, failure/duplicate tests |
| `Latch` | Counting and released are disjoint; zero starts released; threshold releases all waiters once in arrival order; terminal repetition cannot release twice. | Bombay coordination policy | threshold/order and zero/terminal tests |
| `Barrier` | Fixed membership is non-empty/unique; arrivals are per explicit generation; stale/future/duplicate arrivals are distinct; exact membership releases in arrival order; generation exhaustion never wraps. | Bombay cyclic-barrier policy | configuration, generation, exhaustive exhaustion tests |
| `Health` | Versioned present/tombstone state; stale/conflicting evidence rejects atomically; tombstone prevents resurrection; report chooses deterministic worst status. | Bombay operational policy | evidence/tombstone/report tests |
| `Readiness` | Fixed dependencies; unknown and observed evidence differ; readiness requires every dependency ready; stale/conflicting/unknown evidence is atomic; empty set is ready by policy. | Bombay operational policy | all-ready, evidence rejection, empty tests |
| `Configuration` / `Features` | Unconfigured/configured states differ; versions are monotonic; identical updates are idempotent; stale/conflicting candidates return ownership; feature identities normalize without reordering. | Bombay operational policy | configuration and feature-specialization tests |

## Audit result and corrective action

The prior adapter test proved only a leaf destination and therefore did not
cover recursive topology evidence. That was a real verification gap. The
corrective tests now include:

1. an actual root whose send algebra contains a command delivery to its
   `DynamicSupervisor`, whose reply protocol is a `MessageAdapter` targeting
   `Guardian<Root>`;
2. the corresponding real `WorkerPool` root topology; and
3. a compile-time matrix covering every other shipped reply-oriented template.

The type-level defect was that `Recipient<B>` and `Delivery<B>` required the
complete `B: Behavior` fold merely to name a route. `Protocol` now owns only
`Addr` and `Msg`; `Behavior: Protocol` owns events, sends, phases, errors,
births, initialization, and transition. Reply and delivery-only generic slots
require `Protocol`; wrapper/delegation and creation slots continue to require
`Behavior`. This breaks the false recursive proof without an erased callable,
dynamic dispatch, untyped envelope, runtime registry, custom interpreter lane,
or template-specific routing actor.
