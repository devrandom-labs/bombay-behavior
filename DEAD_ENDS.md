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

On 2026-09-29, two unrelated generated behaviors (`Printer` and
`GeneratedBase`) and both `SendLayer` orders reproduced the missing trait with
only `E0277` after their expected inner-before-outer type order was corrected.
The same unconditional 24-line projection candidate again failed the full
core caller check: an existing `BootstrapSends` implementation conflicted
(`E0119`), and public generated products exposed private `ProxyOperation` and
`AssignmentDelivery` request types (`E0446`). The testkit's private `Sink`
protocol produced the same `E0446`. Removing or widening those domain values
would change their ownership contract solely to satisfy the candidate. The
candidate and both focused caller edits were removed. This repeats the earlier
falsifier and provides no independent basis to retain the projection.

Post-experiment aggregate-drift checkpoint: generated actors retain the same
control states, subordinate alternatives, transition branches, and authored
send fields; production lines, modules, and public spellings are unchanged
before and after the retained batch (0/0 delta). The rejected candidate added
one trait implementation template and no actor transition. The residue scan
found no arrival-history state, repeated cause, false cardinality, nested
transition authority, semantic boolean, or positional caller syntax. The
logical-host law, `SendLayer` ordering in `behavior-layer-laws.md`, the
recursive host product in `actor/creation.rs`, and the A11 equation inventory
were cross-checked. Disposition: `reopen`.

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

## Opaque generated logical-host product

Date: 2026-09-29. Disposition: `reopen`.

A Rust 1.95 scratch probe proved that a public generated host-product wrapper
can keep a private request type inside a trait-impl bound without E0446. The
wrapper forwarded `BirthProtocolAt<P, Position>` to the exact private
projection. It fails the current recursive-consumer contract: an external
interpreter can traverse the concrete `NoBirthProtocols` and
`BirthProtocol<P, Tail>` forms, while the opaque wrapper is neither form.
Membership at a known position cannot enumerate every protocol in an
unknown product. A new traversal port would change the public host contract
without a prior failing consumer law. No production code or fixture changed.

Aggregate-drift checkpoint against signed `a43e0d6`: the two focused
generated behaviors retain their prior control states (Bootstrap's creation
sequence and LaneFamilies' unit state), zero subordinate state alternatives,
and the same transition branches. Production lines changed: 0; modules
changed: 0; public spellings changed: 0. Each generated send field still owns
its exact declared lane. The residue scan found no arrival-history state,
repeated cause, false cardinality, nested actor authority, semantic boolean,
or positional application access. The recursive-consumer law in
`transition.rs`, the structural birth product in `actor/creation.rs`, and the
A11 equation inventory were cross-checked. The wrapper hypothesis is
rejected pending a representation that preserves recursive host traversal.
## Installed actor on every endpoint address

Date: 2026-09-30. Disposition: `reopen`.

The shutdown-authority experiment added `EndpointAddress::Installed<B>` and
recipient projection to every address namespace, plus a `B`-indexed actor
transfer and direct committed-child product. Focused issuance, creation, and
shutdown tests passed. Compiling the existing unit tests exposed a legitimate
delivery-only `DeliveryAddr` in `atomic/pool/assignment.rs`: it can issue
exact protocol endpoints but has no installed actor or lifecycle control. The
candidate required that caller to supply an unused `Installed<B>` family
solely because `EndpointAddress` demanded it. The same pressure appeared in
other protocol-only fixtures. This is the first no-op symptom and invalidates
the address-wide association. Giving the fixtures endpoint-only installed
aliases or dummy control tokens would conceal the ownership gap.

Pre-experiment creation control states were established, initialization
rejected, initialization panicked, and host rejected. The named report had
installed and rejected, with one impossible nested child state. The temporary
candidate kept four child states, removed the impossible nested result, and
introduced one committed-child product plus one interpretation trait. Its
nine production files had +193/-160 lines (net +33), two new public types,
and no new modules. The direct product owned ID, kind, occurrence, and exact
actor; each rejected variant retained its prior affine child or actions.
The residue scan found no arrival-history state, duplicated cause, false
cardinality, nested transition authority, semantic boolean, or structural
user syntax. The actual falsifier was a mandatory unused capability in a
protocol-only address implementation. Cross-checks: the shutdown authority
PRD, the normalized atomic-actor essence/architecture/retained-core/downstream
documents, `AGENTS.md` creation law, and the actor-model fresh-allocation
law. The production experiment was removed; the failing law regressions and
ledger remain for the next hypothesis.
## Separate installed-address bound through every actor template

Date: 2026-09-30. Disposition: `reopen`.

The second shutdown-authority experiment kept `EndpointAddress` protocol-only
and introduced `InstalledAddress` solely for installed actor values. A
delivery-only address no longer needed an unused installed family, and the
focused Behavior tests passed. The actors crate then emitted 2,839 diagnostics
because `EstablishedActor<B>`, `ChildCreationOutcome<C, O>`, and every
creation product acquired a new `InstalledAddress` bound, while generic
template surfaces across 47 actors source files still declared the prior
`EndpointAddress` law. This is one repeated bound cluster, not 2,839
independent defects. No catalogue caller was patched to satisfy it.

The intended user syntax remains ordinary `BehaviorLayer` composition with
two wrapper orders and no explicit installed-handle parameter. Pool and stable
proxy are unrelated real templates that already own exact actors; neither
should force clients to count wrapper depth or write placeholder control
values. The candidate deleted recipient strengthening and nested impossible
creation state in its focused core, but its new trait bound spread through the
catalogue instead of deleting a repeated application-side mechanism. The
candidate therefore failed the compiler-friction and public-surface audit
before migration. It was removed.

The pre/post retained creation control sum remains the baseline four outcomes
with the nested report's two alternatives. The temporary candidate had four
direct outcomes and a separate installed/rejected report; the committed-child
product owned ID, kind, occurrence, and exact actor. The focused production
files had +208/-157 lines (net +51), no new module, and three new public types
(`InstalledAddress`, `InterpretInstalledActor`, `CommittedChild`). No
historical state, duplicate cause, false cardinality, nested dispatcher,
semantic boolean, or positional user syntax was added. The structural residue
was the new bound repeated at many generic template declarations. The
shutdown PRD, normalized atomic-actor documents, `BehaviorLayer` law, and
`AGENTS.md` compiler-friction rule were cross-checked.
