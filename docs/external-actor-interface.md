# External actor-system interface

This note records the intended public boundary between a Bombay actor system
and every external consumer: HTTP handlers, command-line programs, tests,
embedded callers, and future authenticated transports. It is an architecture
decision, not a claim that the downstream Bombay runtime already implements
the complete contract.

## Decision

Bombay should expose one transport-neutral, statically typed actor interface:

```rust,ignore
ActorInterface<Api>
```

`Api` is an application-defined named product of actor capabilities exported
to the environment. The interface can also establish a real external
actor/customer with a fresh typed address, an exact reply capability, and one
affine receive authority.

Conceptually:

```rust,ignore
struct Api {
    orders: EstablishedRecipient<Orders>,
    discovery: EstablishedRecipient<ServiceDiscovery>,
}

let interface: ActorInterface<Api> = application.interface();
let mut caller = interface.external::<OrderReplies>()?;

caller
    .send(
        &interface.api().orders,
        OrderCommand::Get {
            reply_to: caller.recipient(),
        },
    )
    .await?;

let reply = caller.receive().await?;
```

The names above are illustrative. The semantic contract, rather than a
particular spelling, is the decision.

The interface is not the actor runtime. Scheduling, mailbox interpretation,
allocation, transport, topology ownership, and lifecycle control remain
private runtime responsibilities. In particular, an untrusted external
consumer does not receive the complete application topology or root shutdown
authority merely because it can communicate with exported actors.

## Source classification

### Actor-model law

An actor configuration exposes receptionists: internal actor names known to
the environment. It may also know external actors: actor names outside the
configuration. Communication can expand these sets by transferring actor
names. This is the actor-model basis for an explicit interface formed from
exported actor capabilities and real external customer endpoints.

Primary sources:

- Gul Agha et al., [A Foundation for Actor Computation](https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf).
- Gul Agha, [Actors: A Model of Concurrent Computation in Distributed Systems](https://www.ics.uci.edu/~jajones/INF102-S18/readings/28_Actors-AghaThesis.pdf).

The actor model does not require a global application router, a service
registry, a public/private modifier on behavior types, or a generic `ask`
operation.

### Derived Bombay construction

- `Api` is a closed, statically typed product that names the initial
  receptionist set.
- An external actor owns a fresh address, a cloneable exact send capability,
  and an affine receiver.
- A request carries its reply capability explicitly. `User::from` records the
  truthful origin of the communication; it is not implicitly the reply
  protocol.
- Logical and exact destinations remain distinct. A stable logical name may
  be appropriate for discovery, proxies, or transport. An exact recipient
  identifies one installed incarnation and must never retarget.
- Discovery is an ordinary typed actor protocol that can return additional
  actor capabilities. It is not privileged runtime lookup.

### Deliberate Bombay policy

- Application composition explicitly chooses the initial exports. Root is not
  automatically public, and declared children are not automatically public.
- HTTP, CLI, tests, embedded integrations, and future transport gateways use
  the same interface contract.
- A future Zenoh/KERI boundary authenticates an external identity and decides
  which typed interface or capabilities it may receive. Authentication,
  serialization, and transport do not enter the `Behavior` algebra.
- Lifecycle ownership and the external communication interface are separate
  capabilities.

## Visibility law

Bombay does not need a `PublicActor` behavior marker or inferred visibility.
The same behavior may be public in one application and private in another.

An internal actor is externally reachable exactly when:

1. its capability occurs in the initial `Api` receptionist product; or
2. an already reachable actor later sends its capability to an external
   actor.

An actor that has not crossed that boundary remains private because the
external party cannot name it. Discovery changes reachability only by
returning a typed capability through ordinary actor communication.

## Discovery

`Registry` and `Resolver` already supply the basic reusable discovery
construction. No new Behavior Actors template and no new Behavior Core
algebra are required.

Passing only a discovery receptionist to a particular boundary is a valid
application policy when discovery is intentionally the sole bootstrap
capability. It is not a universal actor-system rule. Another application may
export several static receptionists, only its root protocol, or no discovery
actor at all.

A heterogeneous global registry would require erasure or a closed
application-specific protocol product. Bombay must not add an untyped global
envelope, runtime protocol registry, `Any`, or string-based lookup to simulate
heterogeneous discovery.

## Request and reply

Actor-native request/reply is customer passing: the request contains a typed
reply capability. A truthful external caller therefore needs a real endpoint
that an internal actor can retain and use. Fabricating the target address as
the sender, using an unclaimed raw address, or treating `User::from` as an
implicit reply destination violates this contract.

The fundamental interface operation is external actor creation plus typed
send and receive. A generic `ask` convenience may be derived later, but only
after Bombay defines:

- timeout ownership;
- cancellation and overlap;
- whether the reply actor remains live after caller cancellation;
- late-reply handling;
- correlation and stale-reply behavior; and
- the typed result when an exact late delivery is rejected.

This is not cosmetic. In the current exact-delivery runtime contract, sending
to a closed exact endpoint is an interpretation failure. Closing a temporary
reply port on timeout could therefore make a correct but late reply fail the
replying actor's action. Bombay must choose and test that policy explicitly
rather than silently adopting dead letters or dropping the message.

## Behavior Actors support

The catalogue audit classified every stored or received recipient by semantic
role. All arbitrary customer/reply destinations are now parameterized by the
sealed `DeliveryRoute<P>` construction instead of being hard-coded as
`Recipient<Reply>`:

| Route capability | Produced effect |
|---|---|
| `Recipient<P>` | `Delivery<P>` |
| `EstablishedRecipient<P>` | `EstablishedDelivery<P>` |
| `ReplyRoute<P>` | ordered `ReplyDelivery<P>` values retaining each logical/exact alternative |

`DeliveryRouteProtocol` projects the protocol and concrete sends product from
the route itself. Stateful templates such as dynamic supervision therefore
cannot pair a retained route with an unrelated nominal reply protocol.

`ChildRoute` must remain excluded from a standalone reply or adapter route.
Creator-local delivery is lawful only when the emitting behavior owns the
matching direct child occurrence in its birth algebra.

Registry membership, configured downstream destinations, worker completion
targets, stable proxy identity, and transport names remain logical where that
is their actual domain. The route migration changes only customer-passing
seams. Compile-contract tests instantiate every such template with exact and
mixed routes, while interpreter tests prove that a mixed lane neither converts
capabilities nor reorders deliveries.

The shutdown coordinators also support creation-dependent topology. They may
start in `AwaitingPlan`. The child composition constructs the plan only after
every configured creation commits and emits `ReportShutdownPlan` through its
returned `Actions`. The report's final destination is inferred from the
complete static actor composition; application code supplies neither a path
nor a route. Interpretation enqueues exactly one `InstallShutdownPlan` event
for the owning coordinator. An earlier shutdown request remains retained until
that event is folded. Homogeneous and heterogeneous plans are different event
types and cannot be substituted. This is explicit Bombay lifecycle policy,
not a new actor-model primitive.

Both additions live wholly in Behavior Actors and use existing Core events,
effect products, exact recipients, child routes, and composition paths. No new
Behavior Core algebra is required.

## Rejected designs

The following alternatives fail the intended laws:

| Design | Reason rejected |
|---|---|
| `Application::tell/ask` as a central router | Makes the application object an ambient runtime service and obscures the sending actor. |
| Discovery as the mandatory only public actor | Confuses one useful bootstrap policy with the actor-system interface law. |
| Publicness inferred from actor type or topology | Visibility is capability reachability chosen by composition. |
| `PublicActor` marker trait | Couples reusable behavior semantics to one application's export policy. |
| Expose the complete topology handle | Leaks private children and installation structure. |
| Give all clients shutdown authority | Conflates communication capability with lifecycle ownership. |
| Fabricate target-as-origin | Claims an external actor identity and endpoint that do not exist. |
| Use `User::from` as reply capability | Provenance and the reply protocol are distinct facts. |
| Dynamic heterogeneous registry | Requires erasure, runtime protocol lookup, or a closed application-specific sum. |
| Treat generic `ask` as harmless sugar | Hides timeout, cancellation, and late exact-delivery semantics. |

## Required runtime work

The downstream Bombay runtime should implement this contract holistically:

1. establish a fresh external actor endpoint with truthful typed origin;
2. expose a named application-specific receptionist product;
3. retain cloneable send capability and affine receive authority;
4. keep exact recipients incarnation-bound and return the original message on
   admission rejection;
5. separate the client interface from topology and lifecycle ownership;
6. adapt HTTP, CLI, tests, and later transport gateways to this same boundary;
7. preserve accepted-prefix draining and deterministic endpoint closure; and
8. keep raw runtime address and mailbox representations private.

The complete composition must prove that a protocol mismatch cannot compile,
receive authority cannot be cloned, private topology cannot be acquired from
the client interface, a stale exact endpoint cannot retarget, and capability
transfer can deliberately expand the reachable receptionist set.

## Audit hypotheses

The architecture audit produced the following verdicts:

1. A central application router is not the actor interface.
2. Discovery is not universally the sole boundary.
3. Publicness cannot be inferred from names or topology.
4. Publicness is not a property of a reusable behavior type.
5. The initial interface is an explicit typed receptionist product.
6. Capability transfer may expand the reachable interface.
7. An external caller must own a real endpoint and truthful origin.
8. A fabricated or unclaimed origin is invalid.
9. `User::from` is provenance, not an implicit reply protocol.
10. Requests carry reply capabilities explicitly.
11. Receive authority is affine.
12. Send capability is transferable and cloneable where its endpoint permits.
13. An exact capability never retargets to a later incarnation.
14. Exact admission rejection preserves the rejected message.
15. A logical reply route is not universally sufficient.
16. `DeliveryRoute` supplies the existing static logical/exact abstraction.
17. The public interface must not expose the complete topology.
18. The public interface must not imply lifecycle authority.
19. Generic `ask` requires a separate explicit lifecycle policy.
20. The interface is transport-neutral and exposes no raw mailbox address.

These conclusions require no new actor template and no new Behavior Core
algebra. The Behavior Actors route-capability and late shutdown-plan work is
complete; the explicit unified interface itself remains downstream runtime
work.
