# Phase-Indexed Actor Protocol Independent Validation Report

This report is the Resource Pool validation of the earlier focused session
result. It is not an architecture-audit report. Architecture-wide capability
classification remains in
`research/architecture-critical-review-loop/REPORT.md`; the original focused
derivation remains in
`research/.session-protocol-derivation-loop-DONE/REPORT.md`.

## Concrete protocol [CASE-01] [TRACE-01]

**Resource Pool** — a realistic Bombay actor protocol with three phases,
direction-sensitive messages, and actor-specific lifecycle concerns.

### Actor relevance

This is not a generic channel protocol. The pool:
- Creates N child workers from a factory spec in Initializing (birth capability)
- Watches children and leases them to clients in Serving
- Coordinates supervised drain with the supervisor in Draining

A client that sends `Acquire` to an uninitialized pool wastes a message and
may get a stale reply after initialization. Catching this at compile time
would eliminate a class of actor lifecycle bugs.

### Specification

**Phase 1: Initializing** — awaiting configuration from supervisor.
- Valid IN: `Configure { size, spec }`
- Valid OUT: `PoolReady` (send to supervisor)
- Valid creation: N child workers via `Create`
- Invalid IN: `Acquire`, `Release`, `DrainStatus`

**Phase 2: Serving** — leasing workers, watching them.
- Valid IN: `Acquire`, `Release(addr)`, `ChildStopped` observations
- Valid OUT: `Granted(addr)` / `NoWorkersAvailable` (send to client)
- Invalid IN: `Configure`, `DrainStatus`

**Phase 3: Draining** — rejecting new leases, finishing in-flight.
- Valid IN: `DrainStatus`
- Valid OUT: `Draining { remaining }`, `DrainComplete`
- Invalid IN: `Configure`, `Acquire`, `Release`

### Valid traces

```
Start -> Configure -> Serving -> Acquire* -> DrainStatus -> Draining -> DrainStatus* -> Stop(Normal)
```

### Direction sensitivity

`PoolReady`, `Granted`, `Draining`, `DrainComplete` are OUTBOUND (sends).
`Configure`, `Acquire`, `Release`, `DrainStatus` are INBOUND (receives).
A `PoolReady` presented to `step()` as an event is a category error — the pool
sends it, it never receives it. The type system does not distinguish these.

### Invalid programs that should fail to compile [STATIC-01]

1. `Acquire` event constructed while pool is in `Initializing` phase
2. `Configure` event constructed while pool is in `Serving` phase
3. `Acquire` event constructed while pool is in `Draining` phase
4. An outbound `PoolReady` (send) presented as an inbound event to `step()`

Runtime rejection (FSM deferral) is insufficient because deferral holds
messages for later replay rather than rejecting them. A stale `Configure`
replayed after the pool has already started would reconfigure a live pool —
a semantic error.

## Primary research and classifications [RESEARCH-01]

### Actor-model laws (Hewitt, Agha, Greif, Clinger)

- **LAW (per-actor serialization):** One actor processes one communication at a
  time (Agha 1986 §3.1.1). Phase-indexed protocols are a higher-level
  construction, not a replacement.
- **LAW (acquaintance addressing):** Actors communicate via addresses they have
  acquired; addresses are communicable values (Agha 1986 §3.2.1). Communication
  partners are discovered at runtime, not fixed at compile time.
- **LAW (become):** After processing a communication, an actor designates its
  replacement behavior (Agha 1986 §3.1.3). Phase transitions are a specific
  pattern of `become` that changes the set of acceptable messages.

### Session-type research (Honda CONCUR'93, ESOP'98)

- **SESSION-TYPE (not actor-model law):** Session types establish a duality
  relation between two endpoints: !T.S is dual to ?T.S. This requires both
  endpoints to be known at compile time for duality checking.
- **Incompatibility with actors:** Actor acquaintance addressing means
  communication partners are dynamically discovered. Two actors cannot know each
  other's protocol state at compile time.

### Bombay derivations and policy

- **BOMBAY-DERIVED:** `Fsm` provides runtime finite-state sequencing via
  `Move::Goto(P)`, `Move::Defer`, and drain-on-change. It is derived from
  receive+become only.
- **BOMBAY-DERIVED:** `Behavior::Event` is a single associated type, fixed for
  the lifetime of a behavior instance. This enables all wrappers to compose
  over a single, statically-known event protocol.
- **BOMBAY-POLICY:** Compile-time phase safety is not a design goal of the
  current algebra. The `Ph` type parameter tracks phase VALUES for `Step::Goto`,
  not phase TYPES that constrain `Event`.

## Derivation attempts

All derivation code is in
`crates/behavior-testkit/tests/session_protocol_derivation_loop.rs`. This is a
fresh protocol (Resource Pool) distinct from the previous campaign's Worker
Lifecycle, chosen to exercise creation capability and peer watching.

### Attempt 1: Existing Fsm [FSM-01]

The FSM expresses the Resource Pool lifecycle correctly at runtime. Tests confirm:
- Valid phase transitions (Initializing -> Serving -> Draining -> Stop)
- Invalid-phase messages are deferred, not rejected
- Deferred messages replay after phase change
- Invalid-phase message construction is NOT prevented at compile time

**What FSM guarantees:** Runtime sequencing and deferral semantics. Messages
are never dropped or duplicated (proven by existing proptest suites).

**What FSM does NOT guarantee:** Compile-time prevention of invalid-phase
messages. `User<MailAddr, PoolMsg>` is constructible with any `PoolMsg`
variant regardless of the current phase. The FSM cannot carry `Create` actions
(Birth = NoBirths) or send typed outputs (Sends = Vec<Delivery<A, Never>>).

### Attempt 2: Phase-indexed typestate via Behavior trait [DERIVE-01]

Goal: `_PoolBehavior<InitPhase>` has `Event = InitEvent`,
`_PoolBehavior<ServingPhase>` has `Event = ServingEvent`, etc.

**Obstruction 1:** `Behavior::Event` is a single associated type, fixed per
implementation. Each phase type would need its own `impl Behavior`, producing
different concrete types. The driver holds `&mut B` for a single `B` — it
cannot change the concrete type at runtime.

**Obstruction 2:** `Step::Goto(Ph)` transitions by VALUE. Using uninhabited
phase markers (empty enums) means no Goto value is constructible. Using
inhabited markers (unit structs) loses type discrimination.

**Obstruction 3:** Rust has no dependent types. `&mut _PoolBehavior<InitPhase>`
cannot become `&mut _PoolBehavior<ServingPhase>` — different types.

**Obstruction 4:** Even a successful per-phase impl cannot transition:
`Behavior::step` returns `BehaviorActed<Self>` where `Self` is fixed. There
is no mechanism to return a `_PoolBehavior<ServingPhase>` from
`_PoolBehavior<InitPhase>::step()`.

### Attempt 3: Application-local enum dispatch [APP-01]

The `PoolApp` enum dispatches manually on `AppPoolMsg`. This provides zero
additional compile-time safety over the FSM: `AppPoolMsg` flattens all
phase-specific messages into one enum, making invalid-phase messages
representable. Runtime match arms handle phase validity — identical pattern to
the FSM's transition function.

### Attempt 4: Phase token gating [DERIVE-01 continued]

Phase tokens (`InitToken`, `ServingToken`, `DrainingToken`) gate message
construction. **Obstruction:** These are ZSTs with public unit fields —
`InitToken(())` is constructible by any code. Making tokens uninhabited (empty
enums) means no one can construct them. There is no safe-Rust mechanism to make
a type constructible ONLY by specific phase-transition code paths.

### Attempt 5: Per-phase Behavior wrapper trait [SURFACE-01]

A `PhaseProtocol` trait with varying `Event` per phase cannot implement the
existing `Behavior` trait because `Behavior::Event` must be a single concrete
type. Bridging would require a flat event enum and runtime dispatch — defeating
the purpose.

### Attempt 6: Session-type duality (COUNTER-01: falsification)

Honda-style session typing requires both endpoints known at compile time. Actor
acquaintance addressing (Agha 1986 §3.2.1) is fundamentally incompatible with
static session duality. Even ignoring this, session duality encodes two-party
protocol duality (!T.S vs ?T.S), which is a different guarantee than per-actor
phase-indexed message validity.

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
- `InitEvent` and `ServingEvent` are distinct types — you cannot pass one where
  the other is expected.
- FSM composes with `Watching` because `Event` is single.

Negative probes (what SHOULD fail to compile but doesn't):
- `PoolMsg::Acquire(...)` is constructible regardless of phase.
- `PoolMsg::Configure(...)` is constructible regardless of phase.
- No type-level distinction between inbound `Acquire` and outbound `PoolReady`.

A compile-fail doctest for phase-invalid construction is not possible with the
current algebra because the FSM's `Event = User<A, PoolMsg>` accepts all
`PoolMsg` values in all phases. The test
`fsm_cannot_prevent_invalid_phase_message_at_compile_time` documents this
limitation explicitly.

## Composition checks [COMPOSE-01]

The existing FSM composes with `Watching` (confirmed by test). By extension,
it composes with all wrappers because it implements `Behavior` with a single
`Event` type. This is both a strength (universal composition) and the root
obstruction (Event cannot vary by phase).

A hypothetical phase-indexed behavior would need each wrapper to be generic over
the phase type and to propagate phase transitions through its own Event type —
doubling or tripling the generic complexity of every wrapper for a guarantee
that only benefits protocols with statically-known phase structure.

## Counter-derivation [COUNTER-01]

**Competing derivation:** Use phase tokens as proof-of-phase to gate message
construction within the application crate. The application defines per-phase
event types and a single flat `PoolMsg` enum with `pub(crate)` constructors
that require phase tokens. External code can only construct `PoolMsg` values
through approved channels.

**Falsification:** Phase tokens are forgeable — `InitToken(())` is constructible
anywhere. Making them uninhabited prevents anyone from constructing them.
Making their fields private requires `unsafe` internal construction, which is
prohibited and would only be convention anyway — the crate author controls all
code paths. This is discipline, not enforcement.

**Adversarial example:** `PoolMsg::Acquire(Acquire { ... })` sent to an
Initializing-phase FSM compiles, runs, and is deferred. The FSM holds it for
later replay — the type system did not prevent the programmer from sending
an acquire to an uninitialized pool.

## Final decision [DECISION-01]

**Outcome: `application-local`**

The existing algebra is sufficient for runtime phase-indexed protocol
sequencing. The `Fsm` combinator provides correct deferral, drain-on-change, and
no-drop/no-duplication semantics — all proven by the existing testkit proptest
suites.

Applications that want per-phase event type discrimination CAN define their own
phase-specific types (`InitEvent`, `ServingEvent`, etc.) and use them within
their crate. This provides type-level distinction between phase-specific
messages. However, they cannot bridge these per-phase types to the `Behavior`
trait's single `Event` type while preserving both phase safety AND wrapper
composition. The application must choose: either use per-phase types without
wrappers, or use wrappers with a flat event enum.

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
gap is real: per-phase event types are possible and useful, but they cannot
integrate with the Behavior trait. The algebra is not "sufficient" for
compile-time phase safety — it merely dodges the question by making Event
single.

### Why not `minimal-core-gap`

No concrete typed derivation failed in a way that demonstrated an algebraic gap
requiring a new primitive. The obstruction is in the `Behavior` trait's design
contract (single `Event` type), not in a missing algebraic construct. Changing
this contract would be a redesign of the Behavior trait, not the addition of a
minimal new wrapper or combinator. Furthermore, the derivation loop protocol
requires a successful outcome-3 campaign to provide a concrete caller, public
rustdoc laws, positive tests, independent behavior tests, and compile-fail tests
— none of which exist.

### Why not `insufficient-evidence`

The evidence is concrete: the derivation test file compiles and runs (8 tests
pass), the FSM tests exercise all phase transitions, per-phase types are
distinct at the type level, and the obstruction to type-level phase transitions
is documented with exact trait signatures and enumerated obstruction points.

### Reversion [DOC-01] [VERIFY-01]

No production code was changed. The derivation test file
(`crates/behavior-testkit/tests/session_protocol_derivation_loop.rs`) is
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
- The Resource Pool protocol's creation capability (birth) was noted but not
  fully exercised as a compile-time check because the FSM has `Birth = NoBirths`.
  A future campaign could focus specifically on creation-indexed protocol typing.
