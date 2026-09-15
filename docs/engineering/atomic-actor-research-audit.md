# DevX research cross-audit for atomic supervisors and pools (engineering record)

This audit was performed after the feature catalogue, solution, DevX target,
minimal core decision, and type equations were written. The research corpus is
evidence and a dead-end register; it is not an architecture to copy.

Source corpus:
`/Users/joel/Code/devrandom/bombay/research/devx-usability-loop`.

## Scope read

The audit covered every research document and the machine-readable campaign
inventories:

- `GOAL`, `REQUIREMENT-AUDIT`, `INVARIANT-MATRIX`, `BASELINE`, `PROGRESS`,
  `HYPOTHESES`, and `ACTOR-TEMPLATE-AUDIT`;
- `DEAD-ENDS`, `RUST-AUTHORING-MECHANISMS`, `ORDINARY-AUTHORING`,
  `OUTER-AUTHORING-PROBE`, `MACRO-LAST-COMPARISON`, and `PERFORMANCE`;
- `APPLICATION-TOPOLOGY-CONTRACT`, `SHUTDOWN-PLAN-AUTHORING`,
  `ESTABLISHED-CREATION-BLOCKER`, `REPOSITORY-AUDIT`, and `RUNBOOK`;
- `ENTITY-OBSERVE-LEVERAGE`, `ENTITY-INTEGRATION-CROSS-VERIFICATION`,
  `FRAMEWORK-COMPARISON`, and `SOURCES`;
- `evidence.json`, `framework-corpus.json`, `ratchets.json`, both checking
  scripts, and the four stable-Rust mechanism probes.

Generated Cargo build output and captured compiler stderr are evidence owned by
their reports; they are not separate design documents.

## Relevant research laws retained

| Research finding | Atomic-actor consequence | Catalogue/solution coverage |
|---|---|---|
| A `Behavior` transition may only return typed `Actions`; Tokio sends, clocks, hydration, and callbacks inside the fold are invalid. | All five templates are direct pure folds. Activation is an interpreter transaction and time arrives as typed facts. | `SH-ACTOR`, `SH-READY`, `EFFECT`, `VERIFY` |
| Creator-local nonce/occurrence is correlation, not actor identity. Successful installation must retain the exact committed capability, and collision is rejection rather than replacement. | Proxy/supervisor/pool creation reserves correlation before commit, observes explicit acceptance/rejection, and never derives incarnation identity from a nonce. | `SH-CREATE`, `SH-CORRELATE`, `PX-INIT`, `PX-REPLACE` |
| Same-action creation is interpreted before dependent child delivery/observation. | This governs ordinary behavior-authored creation actions, but it does not make initialization effects transactional. A worker host commits installed-but-not-ready before its initialization `Actions` are interpreted; later rejection drains the installed incarnation and cannot roll back prior effects. | `SH-CREATE`, `SH-READY`, proxy causal design |
| Logical, established, and mixed routes are different static capabilities. Selecting an exact runtime variant does not erase a logical host requirement. | Lifecycle, customer, diagnostic, and reply routes preserve their concrete `DeliveryRoute`; no template silently upgrades a logical identity. | `SH-STATIC`, `SH-DELIVERY`, `FS-BUILD`, `DS-BUILD`, `FP-BUILD` |
| BAX6: a supervised domain command can become unavailable after mailbox admission, and a terminal fold error is not an observable customer rejection. | The stable proxy must retain the full original sender/command and emit one typed unavailability fact. Delivery rejection has an explicit ownership law. | `PX-ROUTE`, `FS-FACTS`, `SH-DELIVERY` |
| BAX7: proxy installation and worker facts can arrive in either order; assuming creation resolution precedes a child report is invalid. | The proxy alone joins worker creation, initialization, activation, and stop facts order-independently. Fixed/dynamic owners receive one atomic proxy outcome; direct pools own their corresponding closed joins. No pair of flags coordinates them. | `SH-READY`, `PX-ACTIVATE`, `FS-FACTS`, `DS-START`, `FP-TOPOLOGY` |
| BAX10: pool shutdown must run through the pool-owned lifecycle path and return every owned accepted job. | FIFO/keyed draining closes admission, settles assignments/queues, drains direct workers, and emits exact forced facts under the selected deadline policy. | `SH-SHUTDOWN`, `FP-SHUTDOWN`, `KP-END`, `EFFECT` |
| A terminal supervision report is not a causal acknowledgement that the supervisor has stopped. Waiting on its termination before requesting shutdown is circular. | Lifecycle facts, delivery attempts, and actor termination are separate observations. Shutdown always has its own typed policy and completion. | `SH-DELIVERY`, `SH-SHUTDOWN`, `FS-SHUTDOWN` |
| Public examples must not expose generated products, structural selectors, nonces, effect paths, actor-space products, or shutdown coordinator representations. | Ordinary supervisor/pool source sees builders, domain values, policies, and public protocols only. `ReportToParent` is hidden behind `assignment.complete`. | `API`, DevX document, minimal-core visibility rule |
| Builders/typestate are acceptable only when they express real missing/selected semantic choices and compile diagnostics are measured. | Builder proof markers remain inferred/private; there is one canonical documented call order and no type per builder axis. | `API`, type inventory |
| Macros are last resort and may generate syntax only; a macro cannot resolve associated-type identity or repair a missing semantic composition. | No supervisor/pool macro, definition trait, or generated ownership engine is selected. Compile prototypes precede any syntax mechanism. | `API`, `NONFEATURE` |
| Exact external reply requires a real typed endpoint; admission is not execution or durable completion. | Request reply, durable lifecycle ownership, customer outcome, diagnostic outcome, and activation readiness remain separate capabilities/facts. | `SH-DELIVERY`, `DS-START`, `FP-COMPLETE` |
| Async Entity hydration completes before routability; installation alone is insufficient. | The same general lifecycle distinction is required for every worker incarnation without importing Entity machinery into actors. | `SH-READY`, `PX-ACTIVATE`, all worker-owning templates |
| Family capacity research distinguishes waiter, hydration, residency, and binding bounds. | Dynamic entry capacity, pool backlog, keyed binding capacity, and activation authorization are independent named policies. The authorization count has one law; supervisors reserve earlier than pools because their proxy outcome is opaque. | `SH-READY`, `FS-TOPOLOGY`, `FS-TIMING`, `DS-START`, `FP-TOPOLOGY` |
| Forced retirement must preserve exact provenance; `Drop`, abort, logging, or a coarse crash is not orderly completion. | `RetireActorGraphAfter` produces typed per-child/per-job forced-retirement facts, transfers outstanding ownership, and counts the member actor-side drained. A root run future remains live until transferred external work and late exact drains settle. | `SH-SHUTDOWN`, actor shutdown sections |

## Research results deliberately not copied

Several historical successes are dead ends for the new atomic design.

### Old pool `complete_to` capability

`ORDINARY-AUTHORING` treated an assignment-carried `complete_to` recipient as
an improvement over a fabricated address. It is still unsafe: a worker can
substitute another capability of the same protocol. The new law is stricter:
the worker receives an opaque pool-issued token, and only the pool retains and
selects the customer route. Stale results use a separate diagnostic lane.

### Existing supervisor/pool recipes

The catalogue demonstrates feature intent and interpreter seams, but its
`ChildTopology`, stable-proxy ingress plumbing, ownership/fleet/slot types,
positional effect paths, and nested wrapper equations are not retained. The
new architecture re-derives five direct folds from laws.

### Universal behavior-layer composition

The research showed useful independent wrappers such as stash, timeout, and
shutdown adaptation. That does not imply that a supervisor or pool is a stack
of tiny behaviors. Each atomic template owns one coherent state machine and
one action product. Orthogonal wrappers may still surround the finished actor.

### Entity and topology machinery

Entity families, application host normalization, Axum boundaries, Mnesis
durable outcomes, named application shutdown plans, and external ask are
separate owners. Their discoveries inform readiness, capability, and shutdown
laws, but their registries/directories/typestate must not be moved into
supervisors or pools.

### Framework APIs

OTP/Akka/Pekko/Orleans/Ractor comparisons corroborate lifecycle and supervision
concerns. They are not semantic authority for Bombay and do not justify an
untyped mailbox, dynamic registry, callback API, virtual actor, cluster pool,
or another framework's restart defaults.

## Complete dead-end disposition

The following dead-end families remain rejected:

- universal ingress inflation for all actors;
- a giant universal installation/hosting type equation;
- unindexed recursive type lookup and structural-key requirements on arbitrary
  domain types;
- protocol normalization by explicit positional allocation, keyed tries, or a
  closed local macro;
- per-role protocol/address spaces;
- an unconstrained shutdown target sum or a second shutdown DSL;
- positional paths hard-coded through Guardian/wrapper depth;
- direct Tokio reply, callback, channel, task, or clock effects inside a fold;
- using termination as a supervision-report acknowledgement;
- hard-coded parent paths or exposed `ReportToParent` in worker code;
- supervision rejection that discards the original command;
- readiness/restart provenance inferred from runtime ordering;
- a fabricated public proxy ingress that cannot return unavailable commands;
- a synchronous factory pretending to support asynchronous hydration;
- hard-coded outer `StopOnShutdown` around an already authored lifecycle;
- dynamic registries, `dyn`, boxed behaviors/futures, `Any`, `TypeId`, and
  erased envelopes;
- per-family or nonce-derived address namespaces;
- target-as-origin external delivery;
- abort-on-drop as normal lifecycle ownership;
- a single undifferentiated capacity counter;
- unbounded per-identity telemetry or tombstone tables;
- making Discovery or Entity mandatory for local supervision;
- treating private observation, mailbox admission, actor termination, or a
  customer reply as durable command completion;
- a macro, alias, extension trait, builder stage, or callback whose only job is
  to move current compiler plumbing to every caller.

## Features added or corrected because of the audit

The research audit changed the documents in these material ways:

1. installation became an authoritative installed-but-not-ready host commit
   before initialization effects, followed by settlement, activation, and
   readiness, with distinct pre- and post-commit failures;
2. no worker is advertised or assigned before `Ready`;
3. pool completion now carries an opaque token rather than customer authority;
4. stale completion became a diagnostic-only outcome, preserving the
   one-terminal-customer-outcome law;
5. delivery attempt, delivery rejection, state commitment, and rejected
   payload recovery are specified separately;
6. dynamic entries, keyed bindings, active correlations, and queues gained
   capacity/retirement/generation laws;
7. shutdown gained exact `ActorDrainPolicy` alternatives
   `WaitForActorGraph` and `RetireActorGraphAfter { deadline }`, explicitly
   separate from process exit;
8. dynamic request replies were separated from durable lifecycle ownership;
9. fixed supervision gained status/snapshot requirements and optional
   lifecycle events;
10. FIFO fairness became an exact circular-cursor algorithm;
11. retry is documented as at-least-once execution; and
12. typestate and fluent syntax were demoted from selected API to a hypothesis
    requiring compile-pass/fail and diagnostic measurement;
13. every pool readiness/recovery transition must dequeue older eligible work
    before it can commit an idle worker;
14. keyed admission became a total projection over every member phase;
15. root-run completion now waits for transferred residual activation and late
    drain ownership rather than returning a dead error value; and
16. terminal projection and same-action effect dependency now have explicit
    static equations whose Rust realizations remain prototype blockers.

## Remaining blockers after the audit

The research does not determine these Bombay policy/integration questions.
The specification has selected semantic answers for activation permits,
logical cancellation, typed host settlement, and non-recursive diagnostic
termination; their retained-core and interpreter realizations remain open:

1. the smallest concrete association of activation permit/request/resolution
   with installation and initialization without adding hidden behavior
   effects;
2. end-to-end typed host settlement for logical, exact, child, parent,
   lifecycle, customer, and diagnostic deliveries, including total
   heterogeneous terminal lifting and outward transfer through wrappers;
3. creation-scoped dependency lowering from current `Actions` without paths,
   lookup, or a general runtime graph;
4. `RetireActorGraphAfter` interpretation while non-cancellable external
   activation remains in flight, with the root run future owning residual
   settlement until it is empty;
5. a minimal public protocol/type spelling for status, management replies,
   durable lifecycle events, and pool acceptance/terminal outcomes; and
6. compile-pass/fail prototypes proving canonical builders without turbofish,
   public proof markers, aliases, `ReportToParent`, or path-counting.

These rows remain **Open** in the solution matrix. No production symbol is
authorized until its law, ordinary syntax, focused regression, lower-order
composition, and interpreter witness are recorded.
