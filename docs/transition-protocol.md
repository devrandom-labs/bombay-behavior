# Compositional actor protocol

Behaviorpass has one primitive effect algebra, the Agha triple:

```text
Actions<A, Ph, Sends, Birth> = Sends × Create<A, Birth::Child>* × Become
```

`Birth` is purely type-level: `NoBirths` selects the uninhabited `Never` child
type, while `Births<C>` selects `C`. This separates whether creation exists
from the concrete child type without adding an effect, runtime query, handle,
or interpreter operation.

Timers, monitors, links, supervision, identity, and mailboxes are not extra
fields on `Actions`. A capability is an actor protocol: requests are ordinary
typed sends and observations are ordinary typed events. An interpreter may
provide actors such as a clock or failure detector, but their addresses and
protocol nesting are not part of the `Spec` builder API.

## Composition

Each behavior has associated event, effect, done, and error protocols. Event
wrappers form coproducts; independent send batches form products:

```text
Base<S>
  Event  = User<S::Message>
  Sends  = Vec<Delivery<S::Outbound>>

At<B>
  Event  = B::Event + TimeReached
  Sends  = B::Sends × Vec<Delivery<ScheduleAt>>

Watching<B>
  Event  = B::Event + PeerStopped
  Sends  = B::Sends × Vec<Delivery<ObservePeer>>

Base<S> / Fsm       Birth = NoBirths
At<B> / Watching<B> / Stashing<B>
                    Birth = B::Birth
Supervising<B, C>   Birth = Births<Proxy<C>>
Proxy<C>            Birth = Births<C>
```

`Spec` is only a typestate/DX layer over these concrete types. It does not
contain a second intent representation and does not ask users for runtime
service addresses. Injection traits on each concrete wrapper recursively
construct the final event type, so Process remains generic over the composed
protocol without knowing which capabilities are present.

## Environmental assumptions

A clock and a perfect failure detector cannot be implemented inside the pure
asynchronous actor model: time and reliable crash observation are information
supplied by the environment. Behaviorpass represents communication with those
services using actor messages. The interpreter is responsible for providing
implementations, just as it is responsible for delivering all other sends.
This assumption must remain explicit; it does not add timer or monitoring
operations to the core effect algebra.

Keep-address restart is likewise not an Agha `create`. Stable identity is
derived with a proxy actor: the stable proxy survives and forwards to a newly
created implementation actor. The core create operation always creates a new
actor/address.
