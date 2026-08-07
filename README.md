# Bombay Behavior

Bombay Behavior is a small, composable actor behavior algebra for Rust. A
behavior receives its statically associated event protocol and returns an
`Actions` value containing typed sends, fresh child creations, and its next
state. Timing, peer watching, supervision, stashing, and finite-state behavior
are ordinary composable protocols rather than runtime queries or erased
messages.

Its core guarantees are:

- message and event protocols remain statically typed;
- independent send protocols compose without `dyn`, `Any`, or a global envelope;
- `NoBirths` makes creation uninhabited, while `Births<C>` permits only fresh,
  typed `Create` values;
- supervision replaces workers through stable proxy actors, preserving the
  meaning of fresh creation;
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
use behavior::{Acted, Actions, Base, Delivery, MailAddr, Never, NoBirths, State};

struct Counter(u64);

impl State for Counter {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

let behavior = Base::new(Counter(0));
```

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

## License

Licensed under either of Apache License, Version 2.0 or the MIT license, at your
option. See `LICENSE-APACHE` and `LICENSE-MIT`.
