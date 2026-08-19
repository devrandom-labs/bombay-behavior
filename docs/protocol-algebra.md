# Identity, ingress, behavior, and effect algebras

Bombay keeps four semantic roles orthogonal even when one nominal Rust actor
template implements more than one of their traits.

```text
Protocol ── names an established communication destination
    │
    ├── Addr: address namespace
    └── Msg:  public message algebra

Behavior ── owns current state and the pure transition fold
    │
    ├── Protocol: stable public identity
    ├── Event:    complete input algebra
    ├── Sends:    explicit communication/service-effect product
    ├── Birth:    staged fresh-creation capability
    └── Ph / Error / transition state

Actions ── explicit output of one successful fold
    ├── sends
    ├── creates
    └── become or stop
```

Protocol identity and behavior ingress are deliberately different:

```text
Protocol::Msg                       ordinary actor communication
Behavior::Event = Msg + timer + observation + lifecycle + supervision + ...
```

Only `Protocol::Msg` may arrive through an ordinary `Recipient<P>`. Runtime
facts enter through the concrete `Behavior::Event` sum. Service requests leave
through named `Behavior::Sends` lanes. Neither direction is an ambient effect.
`Behavior::Sends: SendsFor<Behavior::Event>` makes the dependency between
return-to-emitter effects and the complete event algebra explicit without
turning protocol identity into behavior identity.

## Structural ingress

A wrapper adds ownership structurally:

```text
EventLayer<Owned, Inner>
├── Owned  selected by Here
└── Inner  selected by Inside<Path>
```

`InjectEvent<Input, Path>` is construction evidence, not a runtime router.
Adding a new input never requires modifying unrelated wrappers. If two nested
layers accept the same input type, `Here` and `Inside<Here>` remain different
capabilities; Rust does not guess from the payload. A request returning a later
fact selects the corresponding named effect lane and ingress owner when it is
emitted. Stale facts are consumed according to that owner's policy and never
search inward.

## Why a protocol is not a behavior

A protocol is stable destination identity. It contains no actor state, next
behavior, initialization fold, error, birth capability, or interpreter-facing
event lanes. Requiring `P: Behavior` merely to hold `Recipient<P>` would make
the sender prove facts about the recipient's current implementation that have
nothing to do with addressing it.

That false dependency causes three concrete failures:

1. wrapping a behavior would appear to create a new public destination;
2. replacement or internal implementation changes would invalidate retained
   recipients; and
3. recursive topologies—such as a root sending to a pool whose worker reports
   completion to that root—would require a recursively defined behavior proof
   just to name an address.

Therefore `Protocol` has no `Behavior` bound, and `Behavior` is not a
`Protocol` supertrait. `Recipient<P>` and `Delivery<P>` require only
`P: Protocol`. Creation, activation, wrapping, and interpretation require
`B: Behavior` and project its public identity through `B::Protocol`.

A concrete actor template may use one nominal Rust type as both witnesses:

```rust,ignore
impl Protocol for Counter {
    type Addr = MailAddr;
    type Msg = CounterMessage;
}

impl Behavior for Counter {
    type Protocol = Self;
    // state and fold algebra ...
}
```

This does not merge the concepts. In the protocol role, `Counter` proves only
the `Addr`/`Msg` signature. In the behavior role, it owns state and a fold.
Generic APIs retain the distinction through their bounds.

When no state-owning actor type is an appropriate nominal identity,
`MessageProtocol<A, M>` supplies a zero-state structural endpoint. Recursive
seams that carry additional laws use concrete protocol products instead. For
example `WorkerPoolProtocol` binds the pool command algebra, assignment job
type, address namespace, and completion destination without naming the pool's
worker topology or mutable state.

## Actor templates and wrappers

A concrete actor template normally owns all three things users should not have
to plumb separately:

```text
actor template = nominal public protocol + state + Behavior fold
```

Constructor arguments contain only application policy or capabilities that
cannot be known by the template: destinations, initial data, topology,
configuration, and pure reactions. Rust infers the composed behavior and send
products.

A transparent wrapper changes internal event/effect algebra while preserving
public identity:

```text
Guardian<B>::Protocol            = B::Protocol
Watch<B>::Protocol               = B::Protocol
Deadline<B>::Protocol            = B::Protocol
Supervise<B, C>::Protocol       = B::Protocol
Supervisor<A, C>::Protocol = SupervisorProtocol<A>
BackoffSupervise<B, C>::Protocol = B::Protocol
BackoffSupervisor<A, C>::Protocol = SupervisorProtocol<A>
ShutdownCoordinator<B, C>::Protocol = B::Protocol
```

The wrapper itself does not implement `Protocol`. Consequently
`Recipient<Guardian<B>>` is invalid: callers retain `Recipient<B::Protocol>`.
An adapter whose purpose is to change the public message algebra is a new actor
template and declares a new nominal protocol explicitly.

`Supervise<B, C>` and `BackoffSupervise<B, C>` are compositions around a
real application behavior whose own transitions may emit `Births<C>`.
`Supervisor<A, C>` and `BackoffSupervisor<A, C>` are standalone
templates: their uninhabited public message protocol makes application commands
impossible, while their concrete lifecycle event sums and proxy-birth products
are owned directly. No inert behavior is needed to manufacture creation
authority.

## Purity and composition laws

The separation preserves these laws:

- a recipient remains usable regardless of which actor later emits its
  delivery;
- wrapping, supervising, timing, or observing an actor does not change its
  public identity;
- structural ingress delivers an input to exactly the owner selected by its
  compile-time path, without payload-based fallback;
- named effect products cannot silently reinterpret an inner lane;
- a behavior fold performs no delivery, creation, clock access, observation,
  or scheduling itself; and
- the interpreter receives the complete `Actions` value and realizes only
  those explicit effects.

At an application root, `Guardian::new(inner)` selects direct normal stop.
`Guardian::coordinated(coordinator)` selects delegation to the coordinator's
inner `Here` owner. The constructor therefore resolves root shutdown policy;
application callers do not spell a nesting path.

Fresh allocation and the actor's ability to send, create, and designate its
next behavior are actor-model laws. The Rust protocol projection, typed event
and effect products, creator-local child routing, initialization ordering, and
wrapper ownership rules are Bombay's derived constructions and documented
policies.
