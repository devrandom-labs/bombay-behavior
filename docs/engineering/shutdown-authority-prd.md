# Exact actor shutdown authority: Behavior-side PRD

Status: proposed design for a dedicated implementation branch. This document
defines the contract to prove in `bombay-behavior`; it does not change production
API. The downstream blocker is `DG-SHUTDOWN` in Bombay's
`docs/prds/execution-ownership/shutdown-authority.md` (reviewed in the sibling
checkout on 2026-09-30). That research used the released Behavior 0.17 contract;
this audit uses `bombay-behavior` main at `3f08364` (Behavior 0.18). The relevant
capability and creation shapes are still present. Bombay's working tree is a
separate, active research checkout and is not part of this change.

## Outcome and scope

An installed `EstablishedActor<B>` must carry an exact-incarnation shutdown
authority indexed by the installed behavior `B`. A committed child-creation
result and a later named-child creation report must preserve that authority.
Callers that only send messages must be able to retain
`EstablishedRecipient<B::Protocol>` without acquiring or reconstructing
shutdown authority. `ShutdownEstablished<B, Path>` must transfer the stronger
capability to its interpreter; it may not recover `B::Event` control from a
protocol endpoint, logical address, registry, or erased callback.

The implementation scope is this repository: the Behavior primitive in
`crates/behavior`, the existing shutdown and creation-observation protocol in
`crates/actors`, generated and handwritten birth products, tests, examples,
benchmarks, and documentation. The downstream Bombay interpreter is an
end-to-end feasibility witness and later migration consumer, not a production
edit in this work package. Do not fold root application lifecycle, direct-child
shutdown, Entity retirement, or unrelated actor catalogue redesign into this
PRD. A release and downstream adoption are separate deliverables.

## Semantic law and ownership equation

The actor-model law is that an actor processes one communication at a time,
may communicate with known recipients, create fresh actors, and designate its
next behavior. Fresh allocation is distinct from `become`. Agha, Mason, Smith,
and Talcott define `newadr`, `initbeh`, and `become` as distinct operations and
require a newly chosen address to be fresh in the configuration: [A foundation
for actor computation](https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf),
§3 and §5. The paper does not prescribe graceful shutdown, weak control
senders, admission closure, or Rust capability types.

Bombay derives a typed staged-creation result from fresh allocation. Its policy
is to commit fresh creation before dependent same-action sends and observation
requests. Graceful shutdown is another deliberate Bombay policy: a request
for an exact installed `B` checks its terminal state, acquires matching control
authority, closes that incarnation's user admission, and sends the event
selected by `B::Event: InjectEvent<ShutdownRequested, Path>`. Acceptance proves
request admission, not eventual termination; observation reports the later
terminal fact. A stale capability cannot affect a later occupant of the same
logical address. Rejection returns the complete original request and exact
capability.

The required value relationship is:

```text
one successful installation of B
  -> exact protocol recipient for B::Protocol
  -> exact B-indexed lifecycle authority for the same incarnation
  -> committed creation/report may transfer both
  -> protocol-only projection discards lifecycle authority

Rejected creation -> no established recipient and no lifecycle authority
```

The endpoint and control cannot be paired after installation merely because
their address and protocol types match. Two installed behaviors can share one
protocol while having different event sums; replacement can reuse a logical
address. The installation product must establish the pairing once and retain
it through every transfer.

## Repository audit

| Surface | Current contract | Consequence |
| --- | --- | --- |
| `actor/addressing.rs` | `EndpointAddress::Established<P>` is indexed only by protocol. `EstablishedActor<B>` stores `EstablishedRecipient<B::Protocol>` and `PhantomData<B>`. `issued`, `from_recipient`, and `interpret` accept or transfer only the protocol endpoint. | `B` is static evidence about an event algebra but no `B`-indexed runtime authority survives. `from_recipient` is an invalid strengthening once shutdown authority is required. |
| `actor/creation.rs` | `EstablishedCreation<P, Occurrence>::Installed` contains only an established recipient. Its `into_actor<Parent>` strengthens that recipient. `ChildCreationOutcome<C, Occurrence>::Established` nests this report; `into_actor()` strengthens it again. | Both creation paths lose control authority. The nested sum also admits `ChildCreationOutcome::Established { established: EstablishedCreation::Rejected { .. } }`, a contradictory result. |
| Birth products and interpreter | `ChildCreationProduct` selects `ChildCreationOutcome<C, Occurrence>` for a concrete child and retains it through `ChildChoice`; `EstablishChild` and `DispatchBirth` carry complete rejected child and initialization actions. | The fix belongs in the concrete child result and installer seam. Preserve the existing heterogeneous structure and affine rejection custody. |
| `actors/protocol/established.rs` | `ObserveEstablishedCreation<P, Occurrence>` reports a protocol-only `EstablishedCreation`; `established_child()` strengthens it using a parent role. `ShutdownEstablished<B, Path>` transfers its actor via `InterpretEstablished<B::Protocol>`, so `InterpretEstablishedShutdown` receives only `Established<B::Protocol>`. | Both later creation observation and generic shutdown must change with the core. Direct-child `ShutdownChild` remains a creator-local request with a different ownership equation. |
| Generated behavior | `behavior-macros` generates birth topology and settlement disposition, but delegates concrete results to `BirthMode`/`ChildCreationProduct`. | Preserve the handwritten/generated equivalence. Macro syntax should change only if the focused test proves that delegation cannot express the law. |
| Catalogue | Pools, stable proxy, and supervisors retain `EstablishedActor<...>` and emit `ShutdownEstablished`; creation settlements create those actors via `into_actor()`. | Migration must check at least pool and proxy/supervision plus two wrapper orders. Do not add placeholder control routes or per-template adapters. |
| Existing tests | `child_creation_actor` checks a reconstructed actor's recipient. `exact_shutdown_action` checks the interpreter call using an endpoint-only fixture. Compile-fail examples deny missing shutdown ingress, but do not deny recipient-to-actor strengthening or cross-incarnation pairing. | Current green tests do not prove the required authority equation. |
| Downstream feasibility | Bombay's `CreationBinding::Established` co-owns `ActorRef<Child::Protocol>`, `ControlSender<Child::Event>`, and task. `LocalEnvironment<B>` co-owns exact endpoint, admission, control, and termination. Generic `InterpretEstablishedShutdown` currently calls the erased `ActorRef<P>::request_shutdown`. | A concrete installation site exists, but the current upstream transfer type cannot carry its typed control to unrelated established targets. |

Source anchors: `crates/behavior/src/actor/addressing.rs:92,263,292,314`;
`crates/behavior/src/actor/creation.rs:540,625,646,698,1886,2021`;
`crates/actors/src/protocol/established.rs:23,153,478,603`;
`crates/behavior-macros/src/lib.rs:1153`;
Bombay `crates/bombay/src/child_bindings.rs:71`,
`application_runtime.rs:1641,2041,2374`, and `local.rs:393,638`.

## Preferred design hypothesis

Extend the runtime-owned address projection with one concrete, cloneable
**installed-actor representation indexed by `B`**. It contains the exact
protocol endpoint and matching weak lifecycle authority as one runtime-issued
value. `EstablishedActor<B>` owns that value and can project an
`EstablishedRecipient<B::Protocol>` for messaging. A separate interpretation
operation transfers the installed-actor representation with `B` intact.
`ShutdownEstablished` uses that operation; ordinary exact delivery and
observation continue to use the protocol recipient.

This is a hypothesis about ownership, not approval of a particular associated
type, trait name, or constructor signature. The first design stage must compare
an `EndpointAddress` associated representation, an ordinary concrete product,
and a narrower request scope against the same regressions. The preferred
representation wins only if its public syntax is inferable, its installer can
issue it truthfully, and it does not force no-op values on behavior types that
cannot accept shutdown. A `B`-indexed installed-actor capability may exist for
all `B`; constructing `ShutdownEstablished<B, Path>` remains impossible unless
the event ingress proof exists.

| Candidate | Design test |
| --- | --- |
| Address-owned `B`-indexed installed representation | Can one runtime-owned value project the `P` endpoint and carry exact `B::Event` authority without making every protocol author define a control type? Preferred if the installer and generic shutdown consume it directly. |
| Concrete product of endpoint and control | Accept only if construction itself establishes same-incarnation pairing. A public `(recipient, control)` constructor with independent inputs does not prove the law. |
| Restrict shutdown to creator-local child IDs | Reject unless every current `ShutdownEstablished` producer and ordinary wrapper composition remains expressible; unrelated established targets are part of the existing contract. |
| Add control to `EndpointAddress::Established<P>` | Reject for two implementations of one `P` with distinct event types: `P` does not determine `B::Event`. |

The power-user issuance boundary remains explicit. It must accept one
runtime-owned installed-actor value, not an arbitrary protocol recipient plus
an independently supplied control sender that a caller can accidentally pair
with another incarnation. Its rustdoc must state that only a successful fresh
installation may issue it. This API cannot cryptographically stop a malicious
runtime implementation from lying, but ordinary safe application code must not
be able to strengthen a message recipient into lifecycle authority.

The committed child success must contain the installed `C` capability and
creation provenance directly. A success must not nest a second
success/rejection sum. Rejected initialization, panic, and host establishment
continue returning their current child and, where applicable, complete
initialization actions. The later named-child report must likewise be able to
transfer the `C` capability when its contract promises one. Either make that
report concrete-child-indexed, or split a protocol-only report from a distinct
concrete-child report; select by comparing complete ownership and timing
equations. The existing `EstablishedCreation<P, Occurrence>::into_actor<Parent>`
and `EstablishedActor::from_recipient` must disappear when they no longer have
truthful source authority. A protocol-only report may remain if it has a real
consumer; it must offer only recipient projection.

This plan should delete strengthening paths and the contradictory nested
success/rejection state. It should not add a second shutdown channel, a global
actor registry, `dyn`, `Any`, `TypeId`, raw pointers, structural-path aliases,
or template-specific wrappers. No executor object enters `Behavior` or
`Actions`.

## Complete transition and composition checks

1. **Installation:** allocation, initialization, endpoint/control construction,
   and creator binding commit before the installed capability is issued. A
   rejected or panicked installation issues neither capability. Initial
   birth and replacement retain their explicit `CreationKind`; the typed
   lifecycle handle cannot infer provenance from address or sequence.
2. **Transfer:** a committed child settlement retains its original creation ID,
   occurrence, kind, and exact actor handle. `ChildChoice`, direct births,
   generated births, `Births`, and `RetirementBirths` preserve it without
   conversion to a protocol-only endpoint. Later observation uses the same
   binding and cannot manufacture a stronger handle from a recipient.
3. **Messaging:** `recipient()` and `into_recipient()` retain the exact message
   endpoint. `EstablishedDelivery<P>` and protocol-indexed observation remain
   usable without `B::Event` or shutdown authority. An external actor has no
   installed `B` handle.
4. **Shutdown:** `ShutdownEstablished<B, Path>` transfers the `B`-indexed handle
   and ingress proof. Accepted settlement consumes the request and returns its
   ID. `AlreadyStopping` and `AlreadyStopped` return the full original request
   with its handle. Later termination is observed separately.
5. **Composition:** test two behaviors with one protocol and distinct event
   sums, `StopOnShutdown<B>` and a nested shutdown wrapper, and both wrapper
   orders involving a second event/effect layer. Adding a wrapper must not
   demand a caller-authored no-op policy or a different positional access path.
6. **Staleness:** an old exact handle, even at an address later reused, can
   reach only the old incarnation. A control send rejected because the old
   consumer closed returns its exact event to the interpreter; it does not
   become successful shutdown or target the replacement.

## Focused regressions before production edits

Write the failing tests in domain vocabulary on the implementation branch and
record their failure against its unmodified baseline:

- Compile-pass: issue one installed actor from a concrete runtime-owned handle,
  project its protocol recipient, transfer the actor into generic shutdown,
  and recover the complete request on rejection. Include an unrelated
  established target and two behaviors sharing one protocol.
- Compile-fail: a protocol recipient cannot become an `EstablishedActor<B>`;
  a handle for `B1` cannot satisfy `B2` even if their protocol is identical;
  no installed-actor handle exists after rejected creation; missing
  `InjectEvent<ShutdownRequested, Path>` cannot construct shutdown.
- Pure settlement: direct and `ChildChoice` creations preserve ID, occurrence,
  kind, handle, and complete rejected values. Impossible nested success plus
  rejection is absent from the public sum.
- Interpretation: accepted shutdown transfers the matching actor handle;
  both rejection reasons return the identical original request. Protocol-only
  exact delivery and observation still receive only the protocol endpoint.
- Minimal downstream witness in an isolated integration probe: one unrelated
  exact target, two concrete behaviors using one protocol, stale handle after
  address reuse, and exact rejected control event. The probe may depend on
  Bombay's concrete types but must not edit Bombay production.

After the model passes, migrate two unrelated real templates and two wrapper
orders without placeholders. Then migrate the remaining catalogue, examples,
benchmarks, macro fixtures, and docs mechanically. The `crates/behavior` and
`crates/actors` tests, compile-fail fixtures, `cargo nextest run --workspace`,
and `nix flake check` are required gates. Run focused debug and optimized
regressions; replay accepted/rejected shutdown and stale-incarnation cases in
optimized builds. Fuzz only if the change alters a stateful sequence surface.

## Change ledger and stop conditions

Before the first implementation edit, record the exact proposed public syntax,
the failing regressions, all new and deleted public spellings, expected files,
and production line delta. Run the aggregate-drift checkpoint before each
semantic experiment and after each retained batch. Count the complete
working-tree delta and the alternatives in the child outcome and creation
report; explain every surviving state by the value a future decision needs.

Expected focused production owners are `actor/addressing.rs`,
`actor/creation.rs`, and `actors/protocol/established.rs`, plus their narrow
re-export roots. A source-breaking migration across runtime implementers of
`EndpointAddress` and existing `EstablishedCreation` constructors is expected;
do not add a default generic or compatibility constructor to conceal it.
Macros should need no semantic generator change unless a focused fixture proves
otherwise. The design should remove at least the recipient-to-actor
strengthening paths and the nested impossible creation outcome. If production
line count rises, report the honest net delta; condensation is an acceptance
criterion about *fewer competing concepts and states*, not a promise of fewer
lines at any cost.

Stop and reopen the model if an unrelated caller needs a no-op authority,
the installer must pair independently obtained endpoint and control values,
another wrapper introduces positional plumbing, a normal rejection loses its
owned request, or the downstream generic interpreter still needs erasure or
address lookup. Follow `AGENTS.md`'s cumulative thresholds: more than 15
changed files, more than 500 net new production lines, or more than three new
public types require explicit authorization before further production edits.

## Acceptance

`DG-SHUTDOWN` can use this upstream result only when the exact actor handle is
issued at successful installation, survives both creation-result paths,
supports generic established shutdown without erased control, and remains
separate from protocol-only messaging. Passing Behavior tests alone is
insufficient: the minimal Bombay interpreter witness must show that the
concrete handle can be consumed with no registry or address re-resolution.
