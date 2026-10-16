# Bombay Behavior

`bombay-behavior` is the foundational, runtime-independent actor behavior
algebra for the Bombay actor stack.

A behavior consumes one typed event at a time and returns explicit `Actions`:
typed communications, staged requests for fresh child actors, and the behavior
or termination decision for the next communication. Scheduling, mailbox
transport, clocks, allocation, and interpretation remain outside the pure
transition.

The crate contains no dynamic protocol registry, type erasure, executor, or
transport.

For a custom interpreter request that lends its whole owned input, implement
`OwnedItem` with its accepted, rejection and prerequisite types. The library
supplies its standard `ActionItem` receiving loan; keep `InterpretItem` as the
runtime implementation. Existing requests with a different custody equation
continue to implement `ActionItem` directly. When migrating a standard request,
replace that implementation rather than implementing both traits, and preserve
its accepted-receipt retention policy.

From the repository root, run the current `#[behavior]` example:

```sh
cargo run -p bombay-behavior --example behavior
```

- [API documentation](https://docs.rs/bombay-behavior)
- [Guide and semantic contracts](https://devrandom-labs.github.io/bombay-behavior/guide/)
- [Source repository](https://github.com/devrandom-labs/bombay-behavior)

Licensed under either Apache-2.0 or MIT.
