# Bombay Behavior

Bombay Behavior provides two published Rust packages. `bombay-behavior`
defines the pure typed behavior boundary: a behavior receives one value from
its closed event algebra and returns `Actions` containing typed sends, fresh
child creations, and its next behavior or termination.
`bombay-behavior-actors` builds the modular reusable template catalogue on that
boundary: composition, lifecycle and supervision, routing and delivery,
discovery, time, persistence policy, workflows, and operational boundaries.

The unified mathematical contract and code-navigation map are in
[Actor transition algebra](docs/actor-transition-algebra.md).

`Behavior::transition` is synchronous: evaluation is a pure fold, while
mailboxes, scheduling, clocks, transport, and effect interpretation remain at
the runtime boundary.

Bounded FIFO worker pools are likewise pure typed behaviors: admission,
assignment ownership, completion correlation, and interruption policy remain
in the fold, while an interpreter only realizes their existing creation,
delivery, observation, and timing effects. Their laws and the Behavior versus
Bombay runtime ownership boundary are documented in
[Worker Pool Semantics](docs/worker-pool.md).

The packages preserve these guarantees:

- public message protocols and internal event algebras remain statically typed
  and orthogonal;
- independent send protocols compose without `dyn`, `Any`, or a global envelope;
- `NoBirths` makes creation uninhabited, while `Births<C>` permits fresh,
  staged children through typed `Create` values; `Children` builds an ordered
  heterogeneous creation product while retaining every installed child's
  concrete protocol;
- each `Create` carries Behavior-owned provenance distinguishing an ordinary
  birth from a replacement of one exact incarnation, while a typed creation
  result reports whether the interpreter actually committed it;
- supervised proxies keep a replacement pending and unroutable until its
  matching installation result succeeds; rejection never becomes a restart;
- supervision replaces workers through stable proxy actors, preserving the
  meaning of fresh creation;
- restart denial and stable-proxy loss remain typed behavior inputs to a pure,
  configurable reaction; stopping propagates through ordinary observation
  rather than a hidden runtime escalation effect;
- initialization effects are interpreted before mailbox events;
- timers, observation, stashing, and state transitions compose without hidden
  runtime side channels.

## The orthogonal actor algebras

Bombay does not use “protocol” as another name for an actor, its mailbox, or
its complete behavior. The foundational roles remain distinct:

| Role | Meaning |
|---|---|
| `Protocol` | Stable public destination identity: exactly `Addr` plus `Msg` |
| `Behavior::Event` | Complete structural ingress algebra: public messages plus typed timer, observation, creation, lifecycle, and supervision facts |
| `Behavior` | Current state and the pure fold from one event to explicit actions |
| `Actions` | Named sends, staged fresh creations, and the next behavior or termination verdict |

```text
Recipient<P> ── requires only P: Protocol
                         │
                         ▼ public message
Behavior<B>  ── projects B::Protocol, folds B::Event
                         │
                         ▼
Actions { sends, creates, become }
```

Ingress ownership is structural. `EventLayer<Owned, Inner>` adds one layer;
`InjectEvent<Input, Here>` selects its owner and
`InjectEvent<Input, Inside<Path>>` selects a nested owner. No wrapper maintains
a list of other templates' event types, and stale facts never search inward by
payload type.

Every `Behavior::Sends` must implement `SendsFor<Behavior::Event>`. A local
callback cannot remain at `Here` after an outer event layer is added; the
wrapper must expose matching effect reindexing. Established, child, and
ancestor destinations are not local callbacks and remain invariant when the
emitter is wrapped.

Interpretation retains the same proof operationally.
`InterpretSends<Interpreter, RootEvent, Path>` starts at `Here`; `SendLayer`
keeps owned requests at the current path and visits inner requests at
`Inside<Path>`. Identical request types at different wrapper depths therefore
construct different root events without payload search or runtime path lookup.

A protocol can never serve as the behavior algebra: it has no state,
initialization, service-event lanes, effects, errors, birth capability, or
transition. Conversely, `Behavior` is not a `Protocol` supertrait. Otherwise a
sender would have to prove the destination's entire current implementation
merely to retain an address. Wrapping or replacing that implementation would
then change identity, and recursive root/pool/worker reply topologies would
become recursive type proofs.

A nominal actor template may implement both traits and set
`Behavior::Protocol = Self`; the roles remain separate because protocol-generic
code can observe only `Addr` and `Msg`. Transparent wrappers such as
`Guardian<B>`, `Watch<B>`, `Deadline<B>`, and `Supervise<B, C>` preserve
`B::Protocol` and do not implement `Protocol` themselves. Thus a typed
recipient's identity remains usable regardless of which actor later emits its
delivery.

`Guardian::new(inner)` owns direct root shutdown;
`Guardian::coordinated(coordinator)` delegates root shutdown to the typed
coordinator owner. Both forms infer the concrete policy and composed ingress.

An actor template owns the protocol, state, and behavior wiring it can know.
Users supply only irreducible domain policy such as destinations, initial
state, topology, configuration, or pure reactions. `MessageProtocol<A, M>` is
the structural zero-state endpoint for cases without a nominal actor type;
recursive seams with additional laws use concrete products such as
`WorkerPoolProtocol`.

The complete rationale and wrapper laws are in
[Protocol, ingress, behavior, and effect algebras](docs/protocol-algebra.md).
The repository-wide template and composition review is recorded in the
[Foundational Actor-Template Algebra Audit](docs/foundational-algebra-audit.md).

## Component installation

Ordinary applications should use Bombay's top-level façade once the split
catalogue integration lands there. They should not assemble behavior, engine,
address, observation, timer, communication, entity, Mnesis, or CESR components
themselves. The dependencies and imports below document this repository's
component-extension boundary and its tests, not the ordinary application
composition root.

```toml
[dependencies]
bombay-behavior = "0.12"
bombay-behavior-actors = "0.12"
```

Rust code imports `bombay-behavior` as `behavior` and
`bombay-behavior-actors` as `behavior_actors`:

```rust
use behavior::{Actions, MailAddr, Never, NoBirths};
use behavior_actors::Activate;

struct Counter(u64);

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Counter {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

let initialized = Counter(0).initialize().unwrap();
let mut worker = initialized.behavior;
worker.receive(MailAddr(0), 1).unwrap();
```

Ordinary communications name their destination's stable public protocol, not
its current behavior implementation and not an untyped address/message pair:

```rust,ignore
let worker = Recipient::<WorkerProtocol>::global(committed_worker_address);
let send = Delivery::new(worker, assignment);
```

`WorkerProtocol::Addr` determines the route namespace and
`WorkerProtocol::Msg` determines the payload. Therefore two protocols with the
same payload and address namespace still produce different communication
types. Recipients are pure route intent; endpoint tables, registration,
lookup, and mailbox delivery belong to the runtime interpreter.

There are two deliberately nested behavior-definition paths: use the single
`#[behavior::behavior(...)]` attribute for a nominal user-message actor, with
capability-denying defaults and optional named send and birth products; and
implement `Behavior` directly for a type that owns service-event variants,
phases, or wrapper semantics. Complete
event streams can be evaluated with `fold_events`; it uses the same
`ActionReducer` as the deterministic test/model path and stops at the first
controlled failure or termination verdict. Production mailbox execution uses
the one universal `bombay-engine::Driver`; `fold_events` is not a second actor
runtime.

Initialization is a consuming typestate transition. Every concrete `Behavior`
implements the `Activate` extension trait, so a standalone catalogue template
or domain behavior initializes directly without a composition container.
`initialize` returns `Initialized<B>`, whose `actions` must be interpreted
before using its `Active<B>` behavior. `Active<B>` can process events but cannot
be initialized again. The call order is enforced by types rather than
`NotInitialized` or `AlreadyInitialized` runtime branches. Typed wrappers are
constructed explicitly through their owning types, for example
`Deadline::new(Stash::new(behavior, route), timer, when, on_elapsed)`.

For a nominal user-message behavior, `#[behavior::behavior(...)]` may annotate
an ordinary inherent impl containing `receive(&mut self, from, message)` and an
optional `init(&mut self)`. It preserves those methods and generates
only the explicit `Behavior` wiring, making the type usable in `Births`,
`Proxy`, `Supervisor`, pools, and interpreter endpoints. The exact expansion
and deliberately narrow scope are documented in
[Nominal Behavior Attribute](docs/behavior-attribute.md).

Wrapper stacks remain ordinary inferred Rust values. Adapter and spawn
boundaries are generic over `B: Behavior`; the library does not add a second
macro for naming compositions. Supervisor and pool construction uses the named
`ChildTopology`, `RestartConfiguration`, and `PoolConfiguration` products
instead of positional argument lists.

## Development and testing

Enter the pinned development environment with:

```sh
nix develop
```

Run the retained workspace tests directly:

```sh
cargo nextest run --workspace
```

Run every repository gate, including build, tests, documentation, Rust and TOML
formatting, dependency audit, and dependency policy checks:

```sh
nix flake check
```

Criterion benchmarks remain available explicitly with `cargo bench`; they are
not executed as nextest test binaries.

## Design notes

The extracted lifecycle domains and their ownership are documented in
[Domain Boundaries](docs/domain-boundaries.md). Replacement realization and its
research/policy classification are documented in
[Replacement Realization](docs/replacement-realization.md).
Peer observation resolution, authoritative already-stopped results, and
cancellation are documented in
[Peer Observation](docs/peer-observation.md).

The functional core and its reduction laws are documented in
[Functional Core](docs/functional-core.md).
Nominal domain behavior authoring and exact static stack naming are documented
in [Behavior Attributes](docs/behavior-attribute.md). Worker admission,
assignment, interruption, and construction laws are documented in
[Worker Pool Semantics](docs/worker-pool.md).

The canonical reusable template names, their algebraic compositions, package
ownership, and eligible implementation dependencies are recorded in the
[Bombay Behavior-Template Catalogue](docs/actor-catalogue.md).
The actor roles that require runtime interpreters or external capability
adapters are recorded in
[Runtime-Backed Actor Capabilities](docs/runtime-backed-actors.md).
Their shared execution contract is documented in the
[Universal Behavior Driver](docs/driver.md).
The runtime-neutral obligations for any adapter that drives these templates
are documented in the [Behavior Adapter Contract](docs/adapter-contract.md).
Ownership across Bombay, Mnesis/Nexus, and CESR/KERI is recorded in the
[Bombay Ecosystem](docs/ecosystem.md).

## License

Licensed under either of Apache License, Version 2.0 or the MIT license, at your
option. See `LICENSE-APACHE` and `LICENSE-MIT`.
