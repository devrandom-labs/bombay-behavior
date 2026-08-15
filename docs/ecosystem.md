# Bombay ecosystem ownership

Bombay is one actor-system architecture distributed across focused Rust
repositories. Repository boundaries assign invariants and dependency direction;
they do not make sibling projects third-party alternatives to one another.

When a Bombay component already owns a capability, new behavior-template work
composes that component through typed events and effects. It does not recommend,
wrap, or recreate a competing framework.

## Owned components

| Repository or published crate | Architectural ownership |
|---|---|
| `bombay-behavior` | Foundational pure `Behavior -> Actions` algebra: send, fresh create, and next behavior or termination |
| `bombay-behavior-actors` | Reusable passive behavior templates and concrete typed protocol transformations |
| `bombay-engine` | One universal generic Driver over a behavior and a statically sufficient environment |
| `bombay-rs` and `bombay-framework` | Tokio-backed `System`, incarnation construction, capability interpretation, handles, and application composition |
| `bombay-communication` | Typed mailbox mechanics, bounded user admission, control priority, fairness, backpressure, and draining |
| `bombay-address` | Generation-safe endpoint registration, resolution, and affine registration authority |
| `bombay-observe` | Exact-generation terminal publication and observation |
| `bombay-timers` | Keyed generation-safe monotonic timer scheduling |
| `bombay-transition` and `bombay-machine-executor` | Deterministic machine composition and exclusive/ordered execution machinery |
| `bombay-entity` | Stable local entity identity, single-flight activation, admission, routing, draining, and passivation |
| Mnesis (repository checkout `nexus`) | Durable authority: event sourcing, repositories, append positions, subscriptions, snapshots, projections, sagas, codecs, adapters, and store conformance |
| `mnesis-bombay-core`, `mnesis-bombay-execution`, and `mnesis-bombay` | Runtime-neutral durable command semantics and the typed Mnesis/Bombay hosting boundary |
| CESR/KERI (repository checkout `cesr`) | CESR primitives and framing, canonical KERI events/codecs, and the sans-I/O KERI key-state and verification fold |

CESR and KERI are not generic internal actor envelopes. They are the owned
representation, framing, and verifiable-identity substrate where a Bombay
boundary or application protocol deliberately speaks CESR/KERI. Internal actor
protocols remain concrete Rust sums and products.

## Dependency direction

```text
application domain
       |
       +--> Bombay behavior templates --> bombay-behavior
       |
       +--> Mnesis domain/durability
       |
       +--> CESR/KERI protocol values where required
                    |
                    v
        focused typed integration adapters
                    |
                    v
       Bombay System environment + universal Driver
                    |
                    v
 communication / address / observe / timers / entity
```

The arrows represent dependency and interpretation, not semantic ownership
transfer. In particular:

- Bombay does not redefine Mnesis durability or aggregate decisions.
- Mnesis does not define actor lifecycle, mailboxes, or scheduling.
- CESR/KERI does not become an untyped universal message bus.
- Behavior templates do not execute runtime mechanisms.
- The Driver does not learn template-specific or domain-specific semantics.

At the user-facing boundary, concrete templates from
`bombay-behavior-actors` are constructed directly and consumed by Bombay's
`System`. `Activate` expresses the one initialization typestate transition;
the optional `Compose` extension trait only builds concrete wrapper types.
Neither API asks users to select, initialize, or wire the owned runtime
components in this table. Bombay's top-level façade should re-export both
traits beside `Behavior` and `Actions`, while component interpreter functions
remain framework-extension surface.

Exact wrapper stacks remain inferred across generic `B: Behavior` boundaries.
Supervisor and pool topology and policy cross the construction boundary through `ChildTopology`,
`RestartConfiguration`, and `PoolConfiguration`; none of these products owns
runtime interpretation.

## Reuse rule

Before adding a crate or subsystem:

1. Check this owned component map and the implementation inventory in the
   behavior-template catalogue.
2. If an owned component supplies the mechanism, define the missing typed
   template protocol and System interpreter rather than duplicating it.
3. If a capability is genuinely absent, assign its laws and lifecycle to a
   focused Bombay repository before selecting implementation dependencies.
4. Use an external Rust crate only as an implementation component inside that
   owned boundary. It must not become a competing actor, persistence, identity,
   protocol, or application framework.

This rule still permits carefully reviewed algorithm, data-structure,
transport, storage-engine, and operating-system crates. It rejects outsourcing
Bombay's semantic architecture.
