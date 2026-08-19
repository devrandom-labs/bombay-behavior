# Universal behavior driver

The Driver owns a concrete `Behavior`, not a public `Protocol`. Protocols name
ordinary delivery destinations; the Driver obtains the behavior's complete
internal event algebra and interprets its explicit effects. See
[Protocol, ingress, behavior, and effect algebras](protocol-algebra.md).

This document defines the execution boundary shared by every Bombay
actor-behavior template. It is a design contract for `bombay-engine::Driver`
and the Bombay `System`; it does not place runtime code in Bombay Behavior.
Repository ownership across Bombay, Mnesis/Nexus, and CESR/KERI is defined in
[Bombay ecosystem ownership](ecosystem.md).

## Status of the law

The actor-model law is that an actor processes one communication at a time and
may send communications, create fresh actors, and designate the behavior used
for the next communication.

Bombay derives one universal execution architecture from that law:

```text
running actor
    = concrete domain behavior
    + zero or more behavior templates
    + a statically sufficient runtime environment
    + one generic Driver loop
```

The packaging into `Behavior`, `Actions`, `Environment`, `Driver`, and `System`
is a Bombay construction. Initialization ordering, creation-before-send
interpretation, transactional activation, mailbox admission, terminal
classification, and runtime capability selection are deliberate Bombay
policies rather than general Agha guarantees.

## Universal driver law

One Driver algorithm must run every fully composed Bombay behavior:

```text
initialize exactly once
interpret every initialization action

repeat:
    obtain one event
    fold the event through the current behavior exactly once
    interpret the complete successful Actions value
    continue with the designated next behavior

until:
    the behavior stops,
    the environment closes,
    behavior or interpretation fails,
    execution is cancelled, or
    execution panics and is poisoned
```

In schematic Rust, not as a promised public signature:

```rust,ignore
async fn drive<B, E>(behavior: B, environment: E) -> RunResult
where
    B: Behavior + Send + 'static,
    B::Event: Send,
    E: Environment<Event = B::Event, Effect = RuntimeEffects<B>> + Send,
{
    // The same implementation is monomorphized for every concrete B and E.
}
```

The Driver is generic source code, not an erased runtime object. Rust produces
a concrete instantiation for each closed composition:

```text
Driver<OrderBehavior, OrderEnvironment>
Driver<CheckoutBehavior, CheckoutEnvironment>
Driver<ProjectionHost, MnesisEnvironment>
```

There is no supervisor Driver, proxy Driver, task Driver, or persistence
Driver. Adding a template must never add another execution loop.

Catalogue construction does not introduce a driver-facing definition wrapper.
A standalone or explicitly wrapped concrete behavior is consumed directly
through `Activate::initialize`. Public owning-type constructors return ordinary
concrete behavior types, so every topology delivers the same `Initialized<B>`
product to the universal Driver without an alternate authoring abstraction.

## Domain injection and templates

An application supplies its statically known domain protocol, state, and pure
decision logic. A behavior template wraps that domain value and remains an
ordinary concrete `Behavior`:

```text
Order
Proxy<Order>
Supervisor<MailAddr, Worker>
Supervise<Root, Worker>
Task<Supervise<Root, Worker>>
```

Each wrapper transforms the complete event sum, action product, state, and
error sum while preserving the inner behavior's effects according to its
documented composition law. The resulting Rust type is verbose but truthful.
Local construction relies on inference. A boundary that must store or expose
the exact stack can use an ordinary Rust alias or newtype. Runtime entry points
should remain generic over `B: Behavior`, so ordinary users do not need one.

Concrete catalogue templates and wrappers are constructed and configured
through their owning types. Standalone supervision uses `Supervisor::new`, and
application composition uses `Supervise::new`; both take explicit
`ChildTopology` and `RestartConfiguration`. No constructor supplies hidden
topology or restart defaults.

The `System` integration should take the fully composed value generically and
allow Rust to infer `B`; application code must not spell the nested wrapper
type just to spawn it. Naming a composition remains possible for framework
extensions that store it or expose it in a signature, but that is an explicit
static protocol boundary rather than ordinary actor startup ceremony.

Normal construction and adapter integration use inference:

```rust,ignore
let behavior = Deadline::new(
    Stash::new(machine, route),
    timer,
    when,
    on_elapsed,
);

system.spawn(behavior)?;
```

The generic `spawn<B: Behavior>` boundary infers the concrete stack. No macro,
hidden executor, dynamic registry, or erased envelope is required. See
[Nominal Behavior Attribute](behavior-attribute.md) for the sole optional
domain-authoring macro.

## Conditions for universal driveability

The same Driver can run every template if every template satisfies the
following closure conditions.

### Complete behavior type

The final composition passed to `System` implements `Behavior`. Its associated
types statically expose its complete address, user message, event sum, send
product, child birth capability, phase menu, and controlled error sum.

The runtime-ready outer composition must close any template-private phase
menu. Under the present algebra this is expressed as `Behavior<Ph = Never>`.
An internal template may use phase values, but the template that owns those
phases must consume them. The universal Driver must not learn supervisor,
proxy, task, workflow, or application phase semantics.

### Events are the only inputs

Every external occurrence enters the fold as a statically known event. This
includes user communications and typed observations such as timer expiry,
child or peer termination, creation result, delivery result, persistence
completion or conflict, projection progress, shutdown, transport failure, and
runtime admission results.

The Driver only obtains the next `B::Event`. It does not inspect which event
lane produced the value.

### Actions are the only outputs

Every successful fold returns `Actions`: Bombay's typed realization of sends,
staged fresh creation, and next behavior or termination. A template never
spawns, sleeps, opens storage, resolves an address, performs transport, or
subscribes through an ambient service.

The environment interprets the complete action value. Results that affect
later behavior return through typed events rather than synchronous mutation of
the fold from outside.

### Capabilities are statically sufficient

The concrete environment must be able to interpret every send and creation
lane declared by the final behavior and produce its runtime observation lanes.
Unsupported compositions fail to compile.

| Template requirement | Runtime realization |
|---|---|
| Ordinary delivery | Bombay Address and Communication |
| Physical mailbox admission and fairness | Bombay Communication |
| Fresh child installation | Bombay `System` child runtime |
| Peer and child observation | Bombay Observe integration |
| Absolute and relative schedules | Bombay Timers integration |
| Local entity activation/passivation | Bombay Entity integration |
| Durable command execution | Mnesis through `mnesis-bombay` |
| Remote delivery or membership | A future typed Bombay subsystem |

The absence of a runtime capability does not require a new Driver. It requires
a typed environment adapter, or means that the template cannot yet be
instantiated by that `System`.

The distilled dependency decisions for template algorithms and runtime
adapters are maintained in the
[behavior-template catalogue](actor-catalogue.md#distilled-crate-adoption-map).
No dependency selected there changes the universal Driver law.

Independent runtimes should use the
[Behavior Adapter Contract](adapter-contract.md) as the executable integration
checklist. It restates this law in terms of the concrete `Behavior`,
`Initialized`, `Active`, and named action products exposed here; it deliberately
does not define an adapter trait or duplicate a runtime.

## Driver and Environment responsibilities

The Driver owns only universal orchestration:

- one-time initialization;
- exclusive one-event-at-a-time folds;
- handing complete actions to the environment;
- advancing only after interpretation completes;
- terminal execution state; and
- poison protection around the pure transition.

The environment owns live or simulated meaning:

- supplying the next event;
- interpreting every named send lane;
- committing fresh child creation;
- delivering later typed observations;
- preserving required effect order;
- classifying interpretation rejection; and
- retiring generation-local runtime resources.

This boundary permits the same Driver to run against a Tokio-backed Bombay
environment, a deterministic test environment, a simulation, or another
executor adapter without changing behavior semantics.

## Effect interpretation ordering

The Driver hands one complete action product to the environment. The
environment must obey the `Actions` contract:

1. Resolve creations in vector order.
2. Commit each accepted fresh binding without replacement.
3. Make committed same-action children available to dependent observation and
   send lanes.
4. Interpret the named send lanes without dropping or duplicating any lane.
5. Preserve the documented order within each lane.
6. Only then permit the next behavior event to be folded.

The Driver must not destructure template-specific send products. The System's
statically dispatched interpreters do that work.

## System construction and spawning

`System` is the composition root. Given an inert, fully composed behavior, it
constructs the concrete runtime resources required to drive one incarnation:

- mailbox and typed event source;
- exact address registration;
- observation subjects;
- timer state;
- child namespace and birth runtime;
- lifecycle reporter;
- environment;
- Driver; and
- Tokio task, handle, and terminal observation.

The conceptual path is:

```text
domain value
    -> concrete template composition
    -> inert actor definition
    -> System activation
    -> registered incarnation + environment
    -> tokio::spawn(Driver::run)
```

Transactional activation must interpret initialization effects before making
the endpoint externally resolvable. Initialization failure publishes neither a
successful birth nor a usable registration.

### Root addresses

A generic `System` cannot manufacture an arbitrary application-defined address
type. The application may supply the root address, or a separately typed root
allocator may issue it. The System then claims its exact registration.

Child creation is different: behavior stages a creator-local nonce and a child
definition, and the interpreter derives the child route and commits a fresh
installation. Neither a root identifier nor a child nonce alone proves
freshness.

## Completion and terminal classification

“Drive every template to completion” means that the same loop correctly drives
every template until its actual terminal condition. It does not mean every
actor naturally terminates. Registries, routers, supervisors, and servers may
intentionally run indefinitely.

The generic engine should expose only execution facts such as behavior-requested
stop, environment closure, controlled behavior failure, controlled environment
failure, and poisoned execution.

Actor lifecycle meaning belongs to Bombay. Bombay may combine those execution
facts with typed reports already emitted by the behavior and runtime provenance
to publish normal completion, collection, linked death, supervision failure,
panic, cancellation, or other concrete lifecycle outcomes. Actor-specific
`Exit` and `Crash` types must not leak back into the foundational Driver
contract.

## Static capability interpretation

Each named send product needs a statically dispatched interpreter. For a
behavior `B`, the compiler must prove that the selected environment can:

```text
interpret B::Sends
realize B::Birth
produce B::Event
retire every owned runtime resource
```

Adding `Periodic`, for example, adds a concrete timer-send product and timer
event lane. It may reuse the existing timer queue, but Bombay still needs the
corresponding static interpreter implementation. Adding a Mnesis projection
host requires typed Mnesis request/result lanes and an adapter. Neither change
modifies the Driver loop.

There is no dynamic fallback. A missing interpreter is a compile-time
integration gap, not a runtime capability lookup.

When `B::Birth::Child` is a recursive `ChildChoice`, `realize B::Birth` means
proving `InstallBirth` for every contained concrete behavior. Exhaustive
recursive dispatch runs inside the existing ordered creation leg; it neither
changes the Driver algorithm nor turns the sum into an installed actor
protocol.
For ordinary `Births<C>`, the blanket `DispatchBirth` implementation invokes
the one concrete `InstallBirth<A, C, ...>` exactly once with the original nonce
and creation provenance. Both shapes therefore use the same Driver bound and
ordered interpretation path.

Both installation and dispatch return `Send` futures. This is an
interpreter-facing concurrency guarantee, not a new behavior effect: it lets a
recursive Driver remain eligible for a thread-safe executor spawn while the
pure creation request and concrete child protocol stay unchanged. Recursive
heterogeneous dispatch holds its selected sum across the await, so every child
alternative, the creator-local nonce, and the installer must be `Send`.

## Backpressure and liveness policy

Whether outbound delivery waits for bounded destination admission is an
environment policy. Awaited admission provides physical backpressure but can
stall mutually dependent actors when bounded mailboxes fill cyclically.
Immediate rejection, deferred runtime-owned delivery, and awaited admission
have different ownership, ordering, and failure semantics.

Before delivery-oriented templates rely on one of these modes, Bombay must
name the selected policy and expose its acceptance or rejection facts through
typed protocols. The Driver remains unchanged; it only awaits the environment's
interpretation of the complete action.

## Required verification

The universal-driver claim requires evidence across both halves of the
boundary:

- one model showing initialization followed by repeated event/action turns;
- property tests proving no event is folded concurrently or more than once;
- tests proving all initialization effects precede mailbox events;
- tests proving creation commits precede dependent same-action sends and
  observations;
- tests proving every named send lane is interpreted exactly once and in its
  declared order;
- failure tests for behavior errors, interpreter errors, source closure,
  cancellation, panic, and poison reuse;
- compile-fail tests for missing event, send, birth, or environment capability;
- composition tests across representative wrapper orders; and
- an end-to-end `System` test for every new capability-backed template family.

The acceptance rule is simple:

> A new template may add types, folds, protocols, and environment interpreters,
> but it must not require a new Driver algorithm.

If a proposed template appears to require its own loop, first determine whether
that loop is actually a pure behavior state machine, a runtime capability, or
an application protocol that has not yet been expressed through typed events
and actions.
