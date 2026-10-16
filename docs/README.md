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
- **Development requirements** retain active proposals and acceptance criteria.
  Historical implementation campaigns live in Git, outside the user guide.

The generated API references are published alongside this guide:

- [Behavior primitives](../behavior/)
- [Reusable behavior actors](../behavior_actors/)

Source and executable tests establish what the selected revision implements.
Contract prose records intended guarantees; conflicts are issues to reconcile,
not evidence that a feature works. Proposed syntax is never current API merely
because it appears in a requirement document.
