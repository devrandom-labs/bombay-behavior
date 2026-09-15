# Bombay Behavior Actors

`bombay-behavior-actors` provides reusable, statically typed actor behaviors
built on the `bombay-behavior` transition algebra.

The catalogue includes composition and lifecycle layers, discovery actors,
routing and admission actors, timing behaviors, workflows, and atomic
supervisors and worker pools. Every actor returns explicit effects through
`Actions`; runtime scheduling and transport remain interpreter concerns.

This is the component crate used by the broader Bombay application facade.
Direct use is intended for interpreter implementations, component tests, and
advanced framework extension.

- [API documentation](https://docs.rs/bombay-behavior-actors)
- [Actor catalogue and laws](https://devrandom-labs.github.io/bombay-behavior/guide/stable-proxy.html)
- [Source repository](https://github.com/devrandom-labs/bombay-behavior)

Licensed under either Apache-2.0 or MIT.
