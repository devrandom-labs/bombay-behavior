# Phase-Indexed Actor Protocol Derivation Report

## Concrete protocol [CASE-01] [TRACE-01]

**Supervised Worker Lifecycle** — a realistic Bombay actor protocol with three
phases and direction-sensitive messages.

### Actor relevance

This is not a generic channel protocol. The lifecycle includes actor-specific
concerns: supervision linkage during startup, peer watching only when Running,
child actor creation during Running, and shutdown/drain coordination with the
supervisor. A worker that receives `Work` before it's configured is a
supervision-significant error; catching it at compile time would eliminate a
class of runtime faults.

### Specification

**Phase 1: Starting** — waiting for configuration from supervisor.
- Valid IN: `Configure { max_concurrent, supervisor }`
- Valid OUT: `Configured` (send to supervisor)
- Invalid IN: `Work`, `DrainStatus`
- On `Configure`: transition to Running, send `Configured` to supervisor

**Phase 2: Running** — processing work, watching peers, sending results.
- Valid IN: `Work { payload, reply_to }`
- Valid OUT: `Result { output }` (send to `reply_to`)
- Also valid: `Watch(peer)` creates observation links
- Invalid IN: `Configure`, `DrainStatus`
- On `Work`: enqueue, send `Result` when done
- On `DrainStatus`: transition to Draining

**Phase 3: Draining** — rejecting new work, finishing in-flight, signaling done.
- Valid IN: `DrainStatus`
- Valid OUT: `Draining { remaining }`, `DrainComplete`
- Invalid IN: `Configure`, `Work`
- On `DrainStatus` with empty queue: `DrainComplete`, Stop
- On `DrainStatus` with pending work: report `Draining { remaining }`, Stay

### Valid traces

```
Start -> Configure -> Running -> Work* -> DrainStatus -> Draining -> DrainStatus* -> Stop(Normal)
```

### Invalid transitions that should fail to compile [STATIC-01]

1. `Work` event constructed while behavior is in `Starting` phase
2. `Configure` event constructed while behavior is in `Running` phase
3. `Work` event constructed while behavior is in `Draining` phase
4. A `Result` (outbound) constructed where a `Work` (inbound) is expected

### Direction sensitivity

Messages have direction: `Configure` comes FROM supervisor, `Work` comes FROM
clients, `Result` goes TO clients. The existing `Recipient<A, M>` couples
address + message type but does not encode send-vs-receive direction. An
outbound `Result` and an inbound `Work` are both `M` values — the type system
does not distinguish them.

## Primary research and classifications [RESEARCH-01]

### Actor-model laws (Hewitt, Agha, Greif, Clinger)

- **LAW (per-actor serialization):** One actor processes one communication at a
  time (Agha 1986 §3.1.1). Phase-indexed protocols are a higher-level
  construction on top of this guarantee, not a replacement for it.
- **LAW (acquaintance addressing):** Actors communicate via addresses they have
  acquired; addresses are communicable values (Agha 1986 §3.2.1). This means an
  actor's communication partners are discovered at runtime, not fixed at compile
  time.
- **LAW (become):** After processing a communication, an actor designates its
  replacement behavior (Agha 1986 §3.1.3). Phase transitions are a specific
  pattern of `become` that changes the set of acceptable messages.

### Session-type research (Honda CONCUR'93, ESOP'98)

- **SESSION-TYPE (not actor-model law):** Session types establish a duality
  relation between two endpoints: !T.S (send T, then S) is dual to ?T.S
  (receive T, then S). This requires both endpoints to be known at compile time
  for duality checking.
- **Incompatibility with actors:** Actor acquaintance addressing means
  communication partners are dynamically discovered. Two actors cannot know each
  other's protocol state at compile time unless they form a closed system.
  Session-type duality between dynamically-discovered peers is not a static
  guarantee.

### Bombay derivations and policy

- **BOMBAY-DERIVED:** `Fsm` provides runtime finite-state sequencing via
  `Move::Goto(P)`, `Move::Defer`, and drain-on-change. It is derived from
  receive+become only; no new primitive was added.
- **BOMBAY-DERIVED:** `Behavior::Event` is a single associated type, fixed for
  the lifetime of a behavior instance. This is a deliberate design: it enables
  all wrappers to compose over a single, statically-known event protocol.
- **BOMBAY-POLICY:** Compile-time phase safety is not a design goal of the
  current algebra. The `Ph` type parameter tracks phase VALUES for `Step::Goto`,
  not phase TYPES that constrain `Event`.

## Derivation attempts

All derivation code is in
`crates/behavior-testkit/tests/session_protocol_derivation.rs`. Key findings:

### Attempt 1: Existing Fsm [FSM-01]

The FSM expresses the Worker Lifecycle correctly at runtime. Tests confirm:
- Valid phase transitions work (Starting -> Running -> Draining -> Stop)
- Invalid-phase messages are deferred, not rejected
- Deferred messages replay after phase change
- Invalid-phase message construction is NOT prevented at compile time

**What FSM guarantees:** Runtime sequencing and deferral semantics. Messages
are never dropped or duplicated (proven by existing proptest suites).

**What FSM does NOT guarantee:** Compile-time prevention of invalid-phase
messages. `User<MailAddr, WorkerMsg>` is constructible with any `WorkerMsg`
variant regardless of the current phase.

### Attempt 2: Phase-indexed typestate via Behavior trait [DERIVE-01]

Goal: `WorkerBehavior<InitPhase>` has `Event = InitEvent`,
`WorkerBehavior<RunningPhase>` has `Event = RunningEvent`, etc.

**Obstruction:** `Behavior::Event` is a single associated type, fixed per
implementation. Each phase type would need its own `impl Behavior`, producing
different concrete types (`WorkerBehavior<InitPhase>`, `WorkerBehavior<RunningPhase>`).
The driver holds `&mut B` for a single `B` — it cannot change the concrete type
at runtime. This is not a Rust limitation per se; it's a consequence of the
Behavior trait's design contract: one behavior = one event type.

**Secondary obstruction:** `Step::Goto(Ph)` transitions by value. Using
uninhabited phase markers (empty enums) means no Goto value is constructible.
Using inhabited markers (unit structs) loses type discrimination — all phases
share the same `Ph` type.

### Attempt 3: Application-local enum dispatch [APP-01]

The `WorkerApp` enum dispatches manually on `AppMsg`. This works identically to
the FSM: all phase-specific messages are flattened into one `AppMsg` enum,
making invalid-phase messages representable. Runtime `if let` checks handle
phase validity — no compile-time improvement over the FSM.

### Attempt 4: Per-phase wrapper trait [SURFACE-01]

A `PhaseBehavior` trait with varying `Event` per phase cannot implement the
existing `Behavior` trait because `Behavior::Event` must be a single concrete
type. Bridging would require a sum type over all phase events, recreating the
flat enum problem.

### Attempt 5: Session-type duality (COUNTER-01: falsification)

Honda-style session typing requires both endpoints known at compile time.
Actor acquaintance addressing (dynamically discovered communication partners)
is fundamentally incompatible with static session duality. No derivation
attempted because the semantic mismatch is at the actor-model law level.

### Summary of obstructions [OBSTRUCTION-01]

1. **Fixed `Event` type:** `Behavior::Event` is a single associated type — it
   cannot vary by phase without breaking the trait contract used by all
   wrappers.
2. **No runtime type change:** The driver pattern (`&mut B`) requires a single
   concrete behavior type. Phase transitions that change the Event type would
   change the concrete behavior type — impossible without dynamic dispatch
   (prohibited by AGENTS.md).
3. **`Ph` is a value index, not a type index:** `Step::Goto(Ph)` carries a
   runtime phase value. It does not constrain `Event` or change the behavior's
   type-level interface.
4. **Actor addressing precludes session duality:** Dynamic acquaintance (Agha
   1986 §3.2.1) means communication partners are not known at compile time.
   Static duality requires both endpoints to be known.
5. **Wrapper composition:** Every existing wrapper (`Supervising`, `Watching`,
   `At`, `ReceiveTimeout`, `Stashing`, `Shutdown`) composes over
   `Behavior::Event` as a single type. A phase-varying Event would break every
   wrapper's type-level contract.

## Compile-fail probes [COMPILE-01]

Positive probes (compile):
- `InitEvent` and `RunningEvent` are distinct types — you cannot pass one where
  the other is expected.
- FSM composes with `Watching` (and all other wrappers) because `Event` is
  single.

Negative probes (what SHOULD fail to compile but doesn't):
- `WorkerMsg::Work(...)` is constructible regardless of phase.
- `WorkerMsg::Configure(...)` is constructible regardless of phase.
- No type-level distinction between inbound `Work` and outbound `Result`.

A compile-fail doctest for phase-invalid construction is not possible with the
current algebra because the FSM's `Event = User<A, M>` accepts all `M` values
in all phases. The test
`fsm_cannot_prevent_invalid_phase_message_at_compile_time` documents this
limitation explicitly.

## Composition checks [COMPOSE-01]

The existing FSM composes with all wrappers because it implements `Behavior`
with a single `Event` type. This is both a strength (universal composition) and
the root obstruction (Event cannot vary by phase).

A hypothetical phase-indexed behavior would need each wrapper to be generic over
the phase type and to propagate phase transitions through its own Event type —
doubling or tripling the generic complexity of every wrapper for a guarantee
that only benefits protocols with statically-known phase structure.

## Counter-derivation (COUNTER-01)

**Competing derivation:** What if we DON'T try to vary `Event` by phase, and
instead use the existing `UserEvent` extraction traits to filter messages?

The existing extraction traits (`UserEvent`, `TimeEvent`, `PeerEvent`,
`ChildEvent`, `WorkerEvent`, `ShutdownEvent`) extract specific lanes from a
compound event type. A phase-filter pattern could define per-phase extraction
traits. But this still operates at runtime: the extraction traits return
`Option<Self>`, and the behavior decides at runtime whether to accept or reject.
It does not make invalid-phase events unrepresentable.

**Adversarial example:** A `WorkerMsg::Work` sent to a `Starting`-phase FSM
compiles, runs, and is deferred. The FSM does not reject it — it holds it for
later replay. This is semantically reasonable (the work is not lost) but it
means the type system did not prevent the programmer from sending work to an
uninitialized worker.

## Final decision [DECISION-01]

**Outcome: `application-local`**

The existing algebra is sufficient for runtime phase-indexed protocol
sequencing. The `Fsm` combinator provides correct deferral, drain-on-change, and
no-drop/no-duplication semantics — all proven by the existing testkit proptest
suites.

Applications that want per-phase event type discrimination CAN define their own
phase-specific types (`InitEvent`, `RunningEvent`, etc.) and dispatch manually.
This provides type-level distinction between phase-specific messages but does
not prevent the construction of invalid-phase events at the call site — because
the Behavior trait requires a single `Event` type, and all wrappers compose over
that single type.

No reusable core construction is lawful under the current `Behavior` trait
design because:

1. `Behavior::Event` is a single associated type — it cannot vary by the
   behavior's current phase without changing the concrete type.
2. Changing the concrete type at runtime requires dynamic dispatch (`dyn
   Behavior`), which is prohibited by the repository's static-dispatch rule
   (AGENTS.md).
3. Making `Event` vary by phase would break every existing wrapper's
   composition contract — each wrapper would need to be generic over the phase
   type and handle phase transitions.

### Why not `existing-algebra-sufficient`

While the FSM handles runtime sequencing, the campaign asked whether a reusable
core construction could provide compile-time phase safety beyond the FSM. The
answer is no — but the application CAN build its own per-phase types. The gap is
not in the algebra's expressive power; it's in the `Behavior` trait's contract:
one event type per behavior instance. This is a deliberate design choice that
enables universal wrapper composition.

### Why not `minimal-core-gap`

No concrete typed derivation failed in a way that demonstrated an algebraic gap
requiring a new primitive. The obstruction is in the `Behavior` trait's design
contract (single `Event` type), not in a missing algebraic construct. Changing
this contract would be a redesign of the Behavior trait, not the addition of a
minimal new wrapper or combinator.

### Why not `insufficient-evidence`

The evidence is concrete: the derivation test file compiles and runs, the FSM
tests pass, the per-phase types are distinct at the type level, and the
obstruction to type-level phase transitions is documented with exact trait
signatures.

### Reversion [DOC-01] [VERIFY-01]

No production code was changed. The derivation test file
(`crates/behavior-testkit/tests/session_protocol_derivation.rs`) is
application-local test code demonstrating the protocol case and obstruction.
All production algebra remains at `prepared_at_commit`.

### Remaining risks

- The derivation test file is deliberately not a compile-fail test harness; it
  documents what the FSM CAN and CANNOT do at runtime, and demonstrates why
  type-level phase transitions are obstructed.
- If a future redesign of the `Behavior` trait allows `Event` to vary by phase
  (e.g., via GATs or a phase-indexed associated type), this campaign's
  obstruction analysis should be revisited.
- Honda-style session typing remains fundamentally incompatible with actor
  acquaintance addressing; no trait redesign would close this gap without
  changing the actor model itself.
