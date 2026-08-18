# Protocol, event, behavior, and effect algebras

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

The public protocol and the behavior event are deliberately different:

```text
Protocol::Msg                       ordinary actor communication
Behavior::Event = Msg + timer + observation + lifecycle + supervision + ...
```

Only `Protocol::Msg` may arrive through an ordinary `Recipient<P>`. Runtime
facts enter through the concrete `Behavior::Event` sum. Service requests leave
through named `Behavior::Sends` lanes. Neither direction is an ambient effect.

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
Supervisor<B, C>::Protocol       = B::Protocol
ShutdownCoordinator<B, C>::Protocol = B::Protocol
```

The wrapper itself does not implement `Protocol`. Consequently
`Recipient<Guardian<B>>` is invalid: callers retain `Recipient<B::Protocol>`.
An adapter whose purpose is to change the public message algebra is a new actor
template and declares a new nominal protocol explicitly.

## Purity and composition laws

The separation preserves these laws:

- a recipient remains usable regardless of which actor later emits its
  delivery;
- wrapping, supervising, timing, or observing an actor does not change its
  public identity;
- event routing preserves an unowned input unchanged until a nested owner
  accepts it;
- named effect products cannot silently reinterpret an inner lane;
- a behavior fold performs no delivery, creation, clock access, observation,
  or scheduling itself; and
- the interpreter receives the complete `Actions` value and realizes only
  those explicit effects.

Fresh allocation and the actor's ability to send, create, and designate its
next behavior are actor-model laws. The Rust protocol projection, typed event
and effect products, creator-local child routing, initialization ordering, and
wrapper ownership rules are Bombay's derived constructions and documented
policies.
