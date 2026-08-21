# Established recipient capability research

## Objective

Determine whether Bombay Behavior can distinguish an address-only recipient
from a runtime-issued, inert, typed established recipient so an interpreter can
deliver directly to an exact endpoint without an application-wide
per-protocol resolver.

This is a derivation and falsification probe. It does not authorize production
API changes. Production code remains untouched unless a separate implementation
decision follows the completed report.

## Semantic classification

- **LAW:** an actor may send only to a known recipient; actor creation supplies
  a fresh actor name that may become an acquaintance.
- **BOMBAY-DERIVED:** `Protocol`, `Recipient<P>`, `ChildRecipient<P>`, typed
  delivery products, and structural interpreter dispatch.
- **BOMBAY-POLICY:** staged child nonces, creation-before-dependent-send order,
  and reporting a committed creation through a later typed event.
- **INTERPRETER:** concrete mailbox/transport endpoint representation,
  allocation, installation, child binding, and endpoint liveness.

The actor-model law does not require a globally searchable address or a
protocol-indexed registry. It also does not require direct endpoint storage;
that is an implementation choice to test.

## Required invariants

1. `Behavior::Protocol` remains the sole protocol identity.
2. The endpoint is navigation evidence selected by that protocol, never a key
   or second identity.
3. Ordinary domain protocols declare no endpoint/key associated type.
4. `Recipient<P>` remains parameterized only by `P`; runtime endpoint types do
   not propagate through domain state and message signatures.
5. Different concrete protocols cannot exchange recipients even when their
   address and message types are identical.
6. A staged child route contains no endpoint before successful installation.
7. A committed creation can return the exact typed established recipient;
   rejection returns no capability.
8. Same-action create-then-send still resolves through the creator-local child
   binding after creation commits.
9. All use remains inert data inside `Actions`; a recipient exposes no direct
   send operation.
10. No `dyn`, `Any`, `TypeId`, boxing, `unsafe`, erased envelope, hashing,
    string dispatch, serialization, or runtime protocol registry.

## Candidate under test

Bind a protocol-indexed endpoint family once to the address/runtime namespace:

```rust,ignore
trait EndpointAddress: Address {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

struct Recipient<P: Protocol>
where
    P::Addr: EndpointAddress,
{
    endpoint: <P::Addr as EndpointAddress>::Established<P>,
}
```

This keeps the endpoint type as a projection from the already-authored
`Protocol::Addr`. One runtime address implementation chooses the complete
endpoint family for every hosted protocol. Domain protocols do not choose keys
or endpoints individually.

## Falsifiers

- The projection cannot be expressed on pinned stable Rust.
- Endpoint type parameters reappear on recipients, messages, behaviors, or
  wrappers.
- Exact protocol mismatch compiles.
- A capability can be forged from a raw logical address when the runtime
  endpoint constructor is unavailable.
- Heterogeneous creation requires erasure or runtime protocol inspection.
- Same-action staged-child delivery cannot retain the existing ordering law.
- An interpreter cannot implement the projection with its concrete typed actor
  reference without exposing runtime effects to Behavior.
