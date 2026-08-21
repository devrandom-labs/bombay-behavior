# Established recipient capability research

## Objective

Determine whether Bombay Behavior can distinguish creator-local correlation
from a runtime-issued, inert, typed established recipient so a runtime can
remove its application-wide address-to-endpoint resolver without weakening
the actor transition algebra.

This is a derivation and falsification probe. It does not authorize production
API changes, and Bombay must not consume the candidate as a released contract.

## Semantic classification

- **LAW:** actor creation allocates a name fresh with respect to the actor
  configuration; a known actor name may be communicated and later used to
  send.
- **BOMBAY-DERIVED:** `Protocol`, typed recipients and deliveries, structural
  child roles, closed products, and interpreter capability traits.
- **BOMBAY-POLICY:** creator-local nonces, creation-before-dependent-send
  interpretation, explicit creation-resolution facts, exact-incarnation
  observation, and orderly shutdown requests.
- **INTERPRETER:** concrete endpoints, fresh-address claims, installation,
  creator-local binding, mailbox admission, observation subscriptions, and
  shutdown realization.

The actor-model law does not require a globally searchable address or a
protocol-indexed registry. A creator-local nonce is not the allocated actor
name and cannot prove freshness.

## Required invariants

1. `Behavior::Protocol` remains the sole protocol identity.
2. The endpoint is navigation evidence selected by that protocol, never a key
   or second identity.
3. Ordinary domain protocols declare no endpoint or key associated type.
4. `EstablishedRecipient<P>` has no endpoint type parameter.
5. Different concrete protocols cannot exchange established recipients.
6. Different named child roles remain distinct even when they select the same
   concrete protocol.
7. A staged child route contains only its creator-local nonce and role.
8. Address allocation is independent of `Address::birth` and rejects a claim
   collision explicitly.
9. Failed allocation or installation returns no established capability and
   binds no nonce.
10. A committed creation returns the exact typed capability and preserves its
    authored birth/replacement provenance.
11. Same-action create-then-send resolves only after the creation commits.
12. Established delivery, observation, cancellation, and shutdown use the
    exact endpoint without address-to-endpoint lookup.
13. Wrapper composition preserves initialization order, creations, and the
    complete next-behavior verdict.
14. All use remains inert data inside the explicit interpretation boundary;
    recipients expose no send or endpoint-accessor operation.
15. No dynamic dispatch, reflection, erased envelope, runtime protocol key,
    hashing, serialization, or unchecked type escape.

## Candidate under test

Bind a protocol-indexed endpoint family once to the address/runtime namespace:

```rust,ignore
trait EndpointAddress: Address {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

struct EstablishedRecipient<P: Protocol>
where
    P::Addr: EndpointAddress,
{
    endpoint: <P::Addr as EndpointAddress>::Established<P>,
}
```

The complete experiment adds three independent structural pieces:

```text
creator-local route = Parent × Role × Nonce
fresh claim         = allocator-owned address, independent of Nonce
installed fact      = Installed(capability) | Rejected(reason)
```

Creator-local heterogeneous bindings form a closed role product. The allocator
stores address claims only, never address-to-endpoint associations. Established
delivery and lifecycle requests carry exact endpoints.

## Falsifiers

- The GAT projection cannot be expressed on pinned stable Rust.
- Endpoint type parameters reappear on domain state, messages, behaviors, or
  wrappers.
- Protocol or role mismatch compiles.
- A rejected creation exposes an established recipient.
- Freshness still depends on deterministic nonce/address arithmetic.
- Heterogeneous role bindings require runtime protocol inspection.
- Same-action child delivery cannot retain commit-before-send ordering.
- Observation or shutdown must rediscover the endpoint from an address.
- Wrapper nesting drops, duplicates, or reorders effects or creations.
- Removing the global resolver requires weakening the pure `Actions` boundary.
