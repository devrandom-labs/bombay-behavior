# Assumptions and judgment calls

- The current `Fsm` is treated as runtime finite-state sequencing, not as session typing.
- Honda-style duality is a session-type result, not an Agha actor-model law.
- A useful application-local typestate derivation is evidence against adding a
  reusable core primitive, not a failure of the campaign.
- The Resource Pool protocol was chosen as a fresh case distinct from the
  previous campaign's Worker Lifecycle to test whether the obstructions
  generalize. They do.
- Phase tokens cannot enforce unforgeability in safe Rust — this is a
  language-level limitation, not a Bombay-specific gap.
- The Behavior trait's single Event type is a deliberate design choice that
  enables universal wrapper composition. Changing it would be a trait redesign,
  not a minimal addition.
