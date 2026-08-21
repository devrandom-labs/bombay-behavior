# Established recipient capability report

## Baseline

Current `Recipient<P>` contains `P::Addr` and explicitly proves no mailbox,
endpoint, or interpreter-owned capability. `DeliveryTarget<P>` resolves a
creator-local child by deriving an address from the emitter and nonce.
`CreationResolved<A>` reports only the committed address. Consequently a
runtime that uses live typed actor references must resolve an established
address outside the `Delivery<P>` value.

## Representation candidates

### Arbitrary endpoint type parameter

```rust,ignore
Recipient<P, Endpoint>
```

This is mechanically static, but every state, message, reply protocol,
delivery lane, wrapper, and application boundary retaining a recipient gains
the runtime endpoint parameter. Attempting to hide that parameter only in a
trait impl is rejected by E0207. This candidate fails the zero-domain-
boilerplate requirement.

### Core-owned opaque token

A core token can remain one-parameter, but a runtime must map the token back to
its live endpoint. That changes the lookup key without removing the resolver.
It is not a direct-capability result.

### Address-owned generic associated endpoint family

```rust,ignore
<P::Addr as EndpointAddress>::Established<P>
```

This binds the endpoint family once to the runtime/address namespace and keeps
`Recipient<P>` one-parameter. The probe tests this candidate against protocol
separation, capability transfer, staged creation, committed creation, and
same-action child delivery.

## Probe result

The address-owned generic associated endpoint family compiles on the pinned
Rust 1.95.0 toolchain.

The positive probe establishes all of the following without an endpoint type
parameter on domain state or messages:

- two protocols with the same logical address and message type select distinct
  `ActorRef<P>` endpoint types;
- `EstablishedRecipient<P>` is cloneable even when the endpoint is not `Copy`
  or `Eq`;
- the recipient contains no direct send or endpoint-accessor operation;
- `EstablishedDelivery<P>` crosses a statically selected interpreter boundary;
- a child nonce exists before installation without containing an endpoint;
- a successful commit returns `EstablishedRecipient<ChildProtocol>`;
- a rejected duplicate commit returns no capability and preserves the first
  binding;
- a same-action local-child target resolves after the creation commits; and
- the committed capability can be transferred in a domain message without an
  endpoint generic appearing in that message's type.

The compile-fail probes establish:

- an arbitrary endpoint parameter hidden only in an impl is rejected with
  E0207;
- `EstablishedRecipient<Worker>` cannot be supplied to a
  `Delivery<Queue>`, even when both protocols use the same address and message
  types; and
- direct endpoint access through `EstablishedRecipient<P>` is unavailable.

`EstablishedRecipient<P>` exposes no direct endpoint accessor. Endpoint
transfer occurs only through the explicit interpretation boundary, which
remains a power-user API boundary rather than exclusive Bombay authority.
Because `InterpretEstablished<P>` is public, any downstream implementation of
that trait receives the endpoint; the positive binary intentionally
demonstrates this. If exclusive Bombay-only extraction is required, the design
needs another experiment and a corresponding authority inversion.

`check.sh` runs the positive tests, validates each negative failure reason, and
rejects `dyn`, `Any`, `TypeId`, `unsafe`, or boxing in the probe source.

## Read-only runtime validation

The sibling Bombay repositories were inspected without modification or build
output.

### Concrete endpoint

Bombay's `ActorRef<P>` is already the required exact-incarnation capability:
it contains the protocol-indexed mailbox endpoint and the exact incarnation's
termination observation. It is `Clone`, but deliberately not `Copy` or `Eq`.
Sending to a closed exact incarnation returns the owned rejected message;
resolving a later actor at the same logical address produces a different
reference and observation.

Consequently, an established capability must be a separate clone-only type.
Replacing the current address value inside `Recipient<P>` while preserving its
unconditional `Copy + Eq` contract would lie about exact-incarnation identity.

### Current lookup consumers

The current `ActorSpace<P>` combines two different responsibilities:

1. exclusive address claim and generation-scoped lease; and
2. address-to-`ActorRef<P>` resolution.

The current runtime resolves through it for ordinary delivery, peer
observation, and child shutdown. Child termination observation is already
retained in the creator's local capabilities. Entity activation already
retains and returns a concrete `ActorRef<P>`.

The associated endpoint family can remove the resolution leg for established
delivery and established peer observation. A creator-local child route still
requires a binding after successful installation, and address freshness still
requires exclusive ownership or an allocator. Neither remaining fact implies
an application-wide lookup registry, but neither disappears merely because a
direct capability exists.

## End-to-end capability matrix

| Path | Capability result | Remaining runtime responsibility |
|---|---|---|
| Established delivery | `EstablishedRecipient<P>` carries the exact endpoint | mailbox admission and closed-endpoint rejection |
| Peer observation | request carries the same exact capability | subscribe to its retained termination observation |
| External/root reference | runtime wraps the `ActorRef<P>` it already returns | issue capability only after activation commits |
| Entity activation | directory already retains `ActorRef<P>` | return/wrap that exact activation capability |
| Staged same-action child send | `ChildRecipient<P>` remains nonce-only | resolve against the just-committed creator-local binding |
| Later child use | committed fact may return `EstablishedRecipient<P>` | retain local binding until Behavior accepts the committed fact |
| Child shutdown/observation | direct endpoint after commit | retain termination and shutdown capabilities per exact child |
| Fresh address ownership | no change | allocator or exclusive claim/lease, without endpoint lookup |

## Required Behavior shape

The smallest truthful Behavior-side construction is additive:

1. Keep the current address-only `Recipient<P>` and `ChildRecipient<P>` while
   migration is evaluated.
2. Add a runtime/address-owned endpoint-family subtrait rather than adding an
   associated endpoint to every `Protocol`.
3. Add an opaque clone-only `EstablishedRecipient<P>` whose endpoint field is
   private.
4. Add an established delivery effect whose explicit structural interpretation
   boundary moves the endpoint into the selected runtime capability trait.
5. Add protocol- or role-indexed committed-creation facts when Behavior needs
   to retain or transfer the newly installed endpoint. Rejection variants own
   no recipient.
6. Make exact peer observation consume an established recipient instead of
   asking the runtime to rediscover an incarnation from an address.

This preserves `Behavior::Protocol` as canonical identity. The associated
endpoint projection is navigation evidence selected by `P`; it is neither a
key nor a second protocol identity.

## Important obstruction: `MailAddr` ownership

A downstream runtime cannot implement a Behavior-owned endpoint-family trait
for Behavior-owned `MailAddr`: Rust's orphan rule forbids implementing a
foreign trait for a foreign type. Putting the family directly on the existing
`Address` trait would instead force Behavior's `MailAddr` implementation to
choose a concrete runtime endpoint it cannot depend on.

Therefore a direct-capability runtime needs a runtime-owned address namespace
type that implements both `Address` and the endpoint-family subtrait. Domain
protocols still write only their existing `type Addr = ...`; they do not write
keys or endpoint types. For Bombay this would be a downstream migration from
Behavior's `MailAddr` to a Bombay-owned address type, not a change made on this
branch.

No crate can remove this coherence boundary or make the E0207 impl sound. The
viable mechanism is stable Rust's generic associated types plus a runtime-owned
implementing type.

## Why this does not yet prove ActorSpaces removable

The probe proves that application-wide established-recipient lookup is not
required by the actor model or by Rust. It does **not** prove that Bombay can
delete its complete `ActorSpaces` product without further downstream design,
because the current product also supplies:

- address claim/lease ownership;
- persistent creator-local child resolution;
- heterogeneous child endpoint storage; and
- lifecycle operations that still accept raw addresses.

A downstream design can remove the public product only after replacing those
responsibilities explicitly. Likely replacements are a non-resolving freshness
allocator/claim capability plus creator-local typed child bindings. A single
untyped map would violate the static-dispatch rule, and a core token mapped
back to endpoints would merely recreate lookup.

## Exact-incarnation and stable-proxy semantics

An established capability names the exact installed incarnation. It remains a
valid value after termination but sends are rejected by that closed endpoint;
it must never retarget to a later actor at the same logical address. Stable
identity remains the existing proxy-derived construction. A proxy's
established capability names the proxy incarnation, not whichever worker it
currently forwards to.

This is a Bombay policy choice supported by the current `ActorRef<P>` behavior,
not an additional Agha law.

## Decision

**Viable minimal Behavior boundary; insufficient alone to delete
ActorSpaces.**

The established-recipient hypothesis succeeds at the type-representation
level and identifies a lawful, zero-domain-endpoint-boilerplate core shape.
It removes the cause of lookup for already-established recipients without
erasure, hashing, macros, runtime protocol keys, or a second protocol identity.

Production implementation should be a separate reviewed change. Before it is
accepted, it must define the complete committed-creation sum, update peer
observation to exact capabilities, prove every wrapper order, and obtain a
downstream Bombay design for runtime-owned addresses, local child bindings,
and non-resolving freshness claims. This research branch intentionally retains
no production API change.

## Production impact

None. All retained changes are confined to this research campaign.

## Verification

Prepared from released `main` at `cabc047c61a7d824a3d6b5846ea967f7cc115774`.

- `research/established-recipient-capability/check.sh`: passed; 2 positive
  semantic tests and 3 compile-fail contracts.
- `cargo nextest run --workspace`: passed; 450 tests.
- `nix flake check -L`: passed all seven compatible-system checks.
