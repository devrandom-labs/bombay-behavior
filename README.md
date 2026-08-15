# Bombay Behavior

Bombay Behavior provides two published Rust packages. `bombay-behavior`
defines the pure typed behavior boundary: a behavior receives its statically
associated event protocol and returns `Actions` containing typed sends, fresh
child creations, and its next behavior or termination.
`bombay-behavior-actors` builds the modular reusable template catalogue on that
boundary: composition, lifecycle and supervision, routing and delivery,
discovery, time, persistence policy, workflows, and operational boundaries.

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

- message and event protocols remain statically typed;
- independent send protocols compose without `dyn`, `Any`, or a global envelope;
- `NoBirths` makes creation uninhabited, while `Births<C>` permits only fresh,
  typed `Create` values;
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

Ordinary communications name their destination behavior protocol, not an
address/message pair:

```rust,ignore
let worker: Recipient<Worker> = Recipient::child(worker_nonce);
let send: Delivery<Worker> = Delivery::new(worker, assignment);
```

`Worker::Addr` determines the route namespace and `Worker::Msg` determines the
payload. Therefore two behaviors accepting the same payload in the same
address namespace still produce different communication types. Recipients are
pure route intent; endpoint tables, registration, lookup, and mailbox delivery
belong to the runtime interpreter.

There are two behavior-definition paths: use `#[behavior::behavior(...)]` for
an ordinary nominal user-message actor, and implement `Behavior` directly for
a type that owns service-event variants, phases, or wrapper semantics. Complete
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
`NotInitialized` or `AlreadyInitialized` runtime branches. Import `Compose`
only when applying typed wrappers such as `watch`, `deadline`, `stash`, or
`stop_on_shutdown`; constructors and policy configuration remain on their
owning concrete template types.

For a nominal user-message behavior, `#[behavior::behavior(...)]` may annotate
an ordinary inherent impl containing `receive(&mut self, from, message)` and an
optional `init(&mut self)`. It preserves those methods and generates
only the explicit `Behavior` wiring, making the type usable in `Births`,
`Proxy`, `Supervisor`, pools, and interpreter endpoints. The exact expansion
and deliberately narrow scope are documented in
[Nominal Behavior Attribute](docs/behavior-attribute.md).

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

The canonical reusable template names, their algebraic compositions, package
ownership, and eligible implementation dependencies are recorded in the
[Bombay Behavior-Template Catalogue](docs/actor-catalogue.md).
The actor roles that require runtime interpreters or external capability
adapters are recorded in
[Runtime-Backed Actor Capabilities](docs/runtime-backed-actors.md).
Their shared execution contract is documented in the
[Universal Behavior Driver](docs/driver.md).
Ownership across Bombay, Mnesis/Nexus, and CESR/KERI is recorded in the
[Bombay Ecosystem](docs/ecosystem.md).

The completed behavior-owned cleanup and its verification evidence are listed
in [Behavior-owned Wart Audit](docs/behavior-wart-audit.md).

## License

Licensed under either of Apache License, Version 2.0 or the MIT license, at your
option. See `LICENSE-APACHE` and `LICENSE-MIT`.
