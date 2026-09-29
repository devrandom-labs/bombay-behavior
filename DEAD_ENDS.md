# Reopened design experiments

## Generated logical-host projection for named send products

Date: 2026-09-28. Disposition: `reopen`.

The intended derived law was that a `#[behavior]` send product projects every
declared logical destination in order, preserving duplicates and excluding
nonlogical lanes. Removing the handwritten `LogicalDeliveryProtocols` impl for
`BootstrapSends` made the focused `behavior_generation` caller fail with E0277,
as expected. A second generated product covered an interpreter-request lane
and repeated destination.

The candidate emitted an unconditional `LogicalDeliveryProtocols` impl whose
associated `Protocols` type appended each field's projection. It then failed
with E0446: existing public generated send products contain private
interpreter-request types such as `LocalRequest`, and the public associated
type exposed those private types through its projection expression. The
existing test matrix produced ten such failures. Widening those request types
or adding a caller placeholder would answer a compiler diagnostic rather than
the public interface law. The generated impl and focused fixture edits were
removed. The retained design still needs a lawful way to project generated
products without forcing unrelated request visibility changes.

A separate Rust 1.95 scratch probe tried to hide the field projection behind
a private helper trait on the public product. Its public associated type used
`<Self as PrivateProjection>::Output`; rustc still emitted E0446 for both the
private trait and its associated type. That indirection does not establish a
lawful public interface and was not added to the repository.

Aggregate-drift checkpoint: the retained control-state sums, subordinate
alternatives, transition branches, production lines, modules, and public
spellings are unchanged from `1af8a3d`. Each generated product still owns its
original named send fields; no current value moved. The rejected candidate
added one trait impl template but no actor state. The residue scan found no
arrival history, repeated cause, false cardinality, nested transition
authority, semantic boolean, or positional consumer syntax. The mismatch is
the public type boundary, cross-checked against the logical-host law in
`docs/actor-transition-algebra.md` and the normalized A03/A11 audit record.
