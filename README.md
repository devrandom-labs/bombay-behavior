# Bombay Behavior

Bombay Behavior is a small, composable actor behavior algebra for Rust. A
behavior receives its statically associated event protocol and returns an
`Actions` value containing typed sends, fresh child creations, and its next
behavior or termination. `Behavior::transition` is synchronous: evaluation is
a pure fold, while waiting for a mailbox belongs only to the interpreter.
Timing, peer watching, supervision, stashing, and finite-state behavior
are ordinary composable protocols rather than runtime queries or erased
messages.

Bounded FIFO worker pools are likewise pure typed behaviors: admission,
assignment ownership, completion correlation, and interruption policy remain
in the fold, while an interpreter only realizes their existing creation,
delivery, observation, and timing effects. Their laws and the Behavior versus
Actorpass ownership boundary are documented in
[Worker Pool Semantics](docs/worker-pool.md).

Its core guarantees are:

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

## Installation

```toml
[dependencies]
bombay-behavior = "0.1"
```

The package is named `bombay-behavior`; Rust code imports its library as
`behavior`:

```rust
use behavior::{Acted, Actions, Handler, MailAddr, Never, NoBirths, Pure};

struct Counter(u64);

impl Handler for Counter {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

let behavior = Pure::new(Counter(0));
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

For function-first code, `Pure::from_fn` accepts a capturing `FnMut`. Complete
event streams can be evaluated with `fold_events`; it uses the same
`ActionReducer` as the mailbox interpreter and stops at the first controlled
failure or termination verdict.

For a nominal user-message behavior, `#[behavior::behavior(...)]` may annotate
an ordinary inherent impl containing `init(&mut self)` and
`receive(&mut self, from, message)`. It preserves those methods and generates
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

## License

Licensed under either of Apache License, Version 2.0 or the MIT license, at your
option. See `LICENSE-APACHE` and `LICENSE-MIT`.
