# Established recipient capability report

## Result

**The full Behavior-side representation is viable as research.** A stable-Rust
GAT, a non-resolving fresh-address allocator, and creator-local structural role
bindings cover every responsibility previously attributed to the proposed
global protocol resolver. The probe does not require a runtime protocol key,
endpoint erasure, or endpoint generics in ordinary domain types.

This is not a production contract. It has not been integrated into the public
Behavior or Behavior Actors crates, and Bombay must not consume it as though it
were released API.

## Governing semantics

- **Actor-model law:** allocation chooses a fresh actor name. The 1997
  operational semantics makes the `newadr` result fresh with respect to the
  current configuration. A known actor name is first-class communication
  information.
- **Bombay derivation:** protocols index recipient and endpoint types; child
  roles form a closed structural product.
- **Bombay policy:** a creator-local nonce correlates staged effects;
  installations commit before dependent same-action sends; committed results
  return through typed event lanes.
- **Interpreter responsibility:** claim a fresh address, initialize and commit
  the actor, issue the exact endpoint, and realize delivery and lifecycle
  requests.

The experiment preserves the pure fold and `Actions`. It does not turn an
endpoint into a direct behavior-side operation.

The probe also implements an actual `Behavior` fold, rather than testing only
standalone carrier types. A committed creation fact moves the behavior from
`Awaiting` to `Active(EstablishedRecipient<P>)` and returns the first delivery
through `Actions::sends`. A rejected fact moves it into the disjoint
`Rejected(CreationRejection)` phase without effects. A later duplicate fact is
a typed `StaleCreationFact` error and cannot replace the retained capability.
Initialization remains the ordinary pure default and all three `Actions` legs
remain visible and unchanged.

## Complete creation state model

The research separates three facts that the old deterministic child-address
derivation could conflate:

```text
Staged(Parent, Role, Nonce, CreationKind)
    -> AllocationRejected(reason)
    -> InstallationRejected(reason)
    -> Installed(EstablishedRecipient<Protocol>)
```

`Nonce` is correlation in the creator's child namespace. It is neither the
allocated address nor evidence of freshness. `FreshAllocator` stores claimed
addresses but no endpoints. A repeated candidate produces the typed
`AddressAlreadyClaimed` rejection; exhaustion is also explicit.

Initialization or environment failure produces a rejected creation with no
recipient and does not bind the nonce. The already allocated address remains a
consumed fresh claim in the probe, so a retry receives a different address.
Only successful initialization and commit add a creator-local binding and
return the capability.

`CreationResolved<Parent, Role>` is an exhaustive sum:

- `Installed { nonce, kind, recipient }`; or
- `Rejected { nonce, kind, reason }`.

The rejected variant cannot expose a recipient. `CreationKind` is preserved
unchanged, so replacement provenance is never inferred from allocation,
sequence arithmetic, or address reuse.

## GAT representation and authority

The endpoint family is selected once by a runtime-owned address namespace:

```rust,ignore
<P::Addr as EndpointAddress>::Established<P>
```

`Behavior::Protocol` remains canonical identity. The projection is navigation
evidence selected by `P`, not another protocol identity. Ordinary domain
protocols do not author endpoint types or keys.

`EstablishedRecipient<P>` contains only the runtime endpoint. The fake
`ActorRef<P>` binds its freshly claimed address internally, avoiding a second
public address field that could disagree with the endpoint.

EstablishedRecipient<P> exposes no direct endpoint accessor. Endpoint transfer
occurs only through the explicit interpretation boundary, which remains a
power-user API boundary rather than exclusive Bombay authority.

Because `InterpretEstablished<P>`, `InterpretObservation<P>`, and
`InterpretShutdown<P>` are public, any downstream implementation receives the
endpoint. The positive probe demonstrates that deliberately. Exclusive
Bombay-only extraction would require a separate authority-inversion
experiment.

## Protocol and duplicate-role separation

Two protocols with identical address and message types still select different
`EstablishedRecipient<P>` and `ActorRef<P>` types. A compile-fail probe rejects
cross-protocol delivery.

Two named roles may deliberately select the same concrete protocol. Their
creation facts remain different types:

```text
CreationResolved<Parent, PrimaryRole>
CreationResolved<Parent, SecondaryRole>
```

A second compile-fail probe rejects exchanging them. The creator-local binding
product is selected by sealed-style `RoleHead`/`RoleTail<Position>` structural
evidence. A single nonce-claim set spans the complete product, so duplicate
nonces across different roles reject rather than silently alias.

This role position is topology navigation evidence. It is not a protocol key,
runtime identity, or application-wide registry index.

## Same-action ordering

The probe performs creation in this order:

1. reject an already-bound creator-local nonce;
2. claim a fresh address independently of the nonce;
3. initialize the child;
4. bind its exact endpoint at the selected role; and
5. interpret dependent sends.

A local staged route resolves only through the creator's typed role product.
After commit, the same exact capability is returned in the creation fact and
may be transferred in an ordinary domain message. No endpoint type parameter
appears in that message.

## Delivery, observation, and shutdown

Established delivery moves the exact endpoint and message through
`InterpretEstablished<P>`; it performs no address lookup.

Observation moves the exact endpoint through `InterpretObservation<P>` and
uses a Behavior-issued `ObservationId` only to correlate the observer-local
relationship and its cancellation. The ID is not actor identity. The terminal
fact returns to its structurally owned event lane. Duplicate IDs,
cancellation without a matching observation, and repeated/stale completion are
explicit `ObservationRejection` outcomes; completion consumes the relationship
exactly once.

Shutdown moves the exact endpoint through `InterpretShutdown<P>`. A test creates
two endpoint incarnations with the same logical address and proves that
observing or stopping the old endpoint does not affect the newer one. Repeated
shutdown is an explicit `AlreadyStopping` or `AlreadyStopped` result.

## Wrapper composition

The composition probe uses the real `Actions`, `SendLayer`, and
`InterpretSends` implementations from `bombay-behavior`. Both wrapper orders
preserve:

- inner-to-outer initialization-effect order;
- the complete creation vector;
- the exact `Goto` verdict; and
- each owned effect lane exactly once.

The traces are `Base -> Observe -> Timer` and `Base -> Timer -> Observe` for
the two nesting orders. The established capability adds a normal typed effect
lane; it requires no privileged composition rule.

## ActorSpaces disposition

The full probe replaces, rather than merely renames, each relevant
responsibility:

| Existing responsibility | Research replacement |
|---|---|
| application-wide address-to-endpoint resolution | exact `EstablishedRecipient<P>` carried by the effect |
| fresh address collision prevention | non-resolving allocator/claim set |
| staged child correlation | creator-local `(Role, Nonce)` route |
| heterogeneous child endpoint retention | closed structural role-binding product |
| same-action child delivery | local binding populated before sends |
| peer observation | exact endpoint plus observer-local correlation |
| child/peer shutdown | exact endpoint request |

Therefore an application-wide `ActorSpaces` lookup product is not required for
established internal capabilities. A creator-local child-binding product still
exists, but it is derived from the parent's authored birth roles and cannot
resolve arbitrary addresses. It is not a second protocol identity.

Address-only `Recipient<P>` remains a distinct capability. Any path that keeps
only a raw address still requires an external transport/resolution boundary.
Removing Bombay's local `ActorSpaces` therefore also requires migrating
internal delivery, observation, and shutdown paths to established recipients;
the GAT alone cannot reinterpret an address as a capability.

## `MailAddr::birth` owner boundary

The probe constructs two `(creator, nonce)` pairs for which deterministic
`Address::birth` produces the same candidate, then proves the allocator issues
two distinct claimed addresses. This makes the separation executable:

```text
nonce correlation != address derivation != fresh allocation
```

A production Behavior contract must stop presenting `MailAddr::birth` as the
installed child address. It may retain deterministic derivation only as an
explicit routing hint with no freshness meaning, or remove that operation from
the installation contract. The runtime allocator is authoritative for the
address bound into the endpoint.

This is a stronger and more Agha-faithful boundary than the earlier
representation-only probe.

## Rust ownership obstruction

A downstream runtime cannot implement a Behavior-owned endpoint-family trait
for Behavior-owned `MailAddr`; the orphan rule forbids the foreign-trait/
foreign-type implementation. Putting the endpoint family directly on every
`Protocol` would reintroduce domain boilerplate.

The viable candidate therefore uses a runtime-owned address namespace that
implements both `Address` and `EndpointAddress`. Bombay would need to own and
re-export that address type for its protocols. No crate can bypass this
coherence rule or make the E0207 hidden-host-index pattern sound.

## What Bombay would need after a production contract exists

Bombay's later half would be:

1. own the concrete address namespace and its `ActorRef<P>` endpoint family;
2. replace deterministic child-address installation with a collision-free,
   non-resolving allocator/claim capability;
3. return role-indexed installed/rejected creation facts;
4. retain creator-local role bindings for staged same-action effects;
5. route established delivery directly through the carried `ActorRef<P>`;
6. observe and shut down exact carried endpoints;
7. return established references from root and entity activation; and
8. keep raw-address resolution only at genuine external transport boundaries.

Bombay must wait until Behavior and Behavior Actors publish and test that
production contract. This research branch changes neither repository's
released API.

## Mechanical evidence

Positive tests cover:

- deterministic address-hint collision versus distinct fresh allocation;
- address-claim collision rejection with no capability;
- initialization rejection and retry with a new address;
- duplicate roles and cross-role nonce collision;
- commit-before-dependent-send and later capability transfer;
- a pure `Behavior` fold whose installed, rejected, and stale-fact paths keep
  capability use inside explicit `Actions`;
- exact observation, cancellation, and shutdown; and
- both wrapper orders with complete `Actions` preservation.

An independent model exhaustively checks all 1,728 three-attempt creation
sequences across both duplicate protocol roles, two nonces, and all three
installation dispositions. After every attempt it compares the resolution
class, claimed-address sequence, and every role/nonce binding.

Compile-fail probes cover:

- E0207 for an arbitrary hidden endpoint parameter;
- cross-protocol established delivery;
- absence of direct endpoint extraction;
- cross-role creation-fact exchange;
- absence of a recipient field on rejected creation; and
- use of a staged child as an established delivery target.

The checker also rejects dynamic dispatch, reflection, unchecked code, erased
heap dispatch, and address-to-endpoint map machinery in the probe source.

## Decision

**Retain the full research result.** The experiment shows a coherent static
Behavior-side design that can remove Bombay's application-wide local endpoint
resolver while preserving the actor-model laws and the explicit `Actions`
boundary.

Do not call this production-ready. Production still requires a separately
reviewed public API change in Behavior and Behavior Actors, complete migration
of their lifecycle types and wrappers, compile-fail coverage in the shipped
crates, and a downstream Bombay implementation review.

## Verification

- `research/established-recipient-capability/check.sh`: 10 positive semantic
  tests and 6 compile-fail contracts passed.
- The probe uses pinned Rust 1.95.0-compatible dependencies.
- `cargo clippy --manifest-path
  research/established-recipient-capability/probe/Cargo.toml --all-targets --
  -D warnings` passed.
- `cargo nextest run --workspace`: 450 tests passed.
- `nix flake check -L`: all 7 compatible checks passed.
