# Bombay Behavior Macros

`bombay-behavior-macros` contains the procedural macros used by Bombay
Behavior.

The primary `#[behavior]` attribute generates the nominal protocol, closed
send and birth products, and the concrete `Behavior` implementation for an
inherent actor implementation. Applications normally receive this macro
through the `bombay-behavior` crate rather than depending on this crate
directly.

- [API documentation](https://docs.rs/bombay-behavior-macros)
- [Behavior attribute contract](https://devrandom-labs.github.io/bombay-behavior/guide/behavior-attribute.html)
- [Source repository](https://github.com/devrandom-labs/bombay-behavior)

Licensed under either Apache-2.0 or MIT.
