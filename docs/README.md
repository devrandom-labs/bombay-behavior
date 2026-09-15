# Bombay Behavior

Bombay Behavior provides a pure, statically typed actor transition algebra and
a catalogue of reusable actors. A behavior processes one communication at a
time and returns explicit `Actions`: typed communications, staged fresh actor
creations, and its next behavior or termination decision. Scheduling,
transport, clocks, allocation, and effect execution remain interpreter
responsibilities.

This guide has three documentation classes:

- **Canonical contracts** describe the current behavior algebra, application
  authoring syntax, composition laws, and interpreter obligations.
- **Actor catalogue** documents the current reusable atomic actors and their
  normalized aggregate laws.
- **Engineering records** preserve research, rejected alternatives, audit
  evidence, and implementation campaign decisions. They explain why the
  current design exists but are not an additional public API contract.

The generated API references are published alongside this guide:

- [Behavior primitives](../behavior/)
- [Reusable behavior actors](../behavior_actors/)

When a historical engineering record conflicts with the crate API or a
canonical contract, the current public types and canonical contract govern.
