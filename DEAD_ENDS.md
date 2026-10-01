# Reopened design experiments

## Separate late worker-preparation return wrapper

Date: 2026-10-01. Disposition: `reopen` for the wrapper only.

The consuming-start model initially added a public
`WorkerPreparationReturn::Prepared | SourceRejected` sum around the existing
complete `WorkerPreparation`. It preserved the late source rejection, but
inspection showed that the existing complete preparation already owns the
ticket, affine source, selected roles, and terminal outcome. The extra sum
held no independent event or decision: its `Prepared` arm simply forwarded
the complete product, and its `SourceRejected` arm duplicated an outcome the
complete product could own. Retaining it would make two public result
spellings for one late fact and increase the generic event arity without a
unique semantic owner. The wrapper was removed in this isolated branch.

The revised model extends the existing private `WorkerPreparationOutcome`
with a source-rejected alternative and lets only the distinct public
`StartingWorkerPreparation` construct it. That first-attempt type cannot
contain a prepared prefix, so the typed source-rejection law remains intact.
The public late event carries the existing `WorkerPreparation` directly.
This refinement removes one proposed public type; no aggregate root phase is
added. The existing FIFO/keyed/fixed consumer migration and Bombay task
custody still must pass before the revised model can be retained.

## Borrowed worker-preparation start receipt

Date: 2026-10-01. Disposition: `reopen`.

The intended template policy is to let a pool fold shutdown while its worker
source is still preparing. A first source-only candidate added a
`PrepareWorkers::started(&self)` receipt and changed the action's accepted
result to that receipt. The Bombay live probe and owner FIFO compile probe
preceded this production edit. The candidate compiled no aggregate: fixed
supervision and shared pool recovery still consumed the old complete result.
Those errors were expected migration fallout, not an architecture source.

Ownership inspection found the actual falsifier before migration: borrowing
the request permits multiple independently owned start receipts from one
affine source action. The aggregate could not distinguish a repeated start
from the one actual start without adding an arrival-history flag. That would
weaken the exact-once action law. The three candidate production files were
restored to the selected revision; the temporary candidate measured
`+76/-17/net +59` production lines, no new module, and two proposed public
types. The retained representation is unchanged.

The next hypothesis consumes `PrepareWorkers` into one start receipt and the
existing `PendingWorkerPreparation`, which owns the source, current role,
prepared prefix, and remaining roles. It removes the initial progress methods
from `PrepareWorkers` instead of adding another task-progress wrapper. The
FIFO, keyed, and fixed aggregate control-state sums remain at their original
five, five, and five alternatives; fixed source custody remains three. The
candidate adds no arrival history, repeated cause, false cardinality, nested
transition authority, semantic boolean, or structural user syntax. The owner
worker-preparation, FIFO/keyed/fixed transition tests, normalized atomic actor
documents, and `AGENTS.md` aggregate checkpoint were cross-checked. The
new hypothesis must still prove exact late return and runtime task custody.

## Initial and later worker-preparation attempts sharing one pending type

Date: 2026-10-01. Disposition: `reopen`.

The consuming-start hypothesis returned a receipt and the existing
`PendingWorkerPreparation`. That type can contain an already prepared prefix
after the first selected role. Its proposed public late `SourceRejected`
alternative accepted any `PendingWorkerPreparation`, so an interpreter could
classify a later role failure as a source rejection before any worker was
prepared while carrying a nonempty prefix. The fixed supervisor and direct
pool policies distinguish these cases and must not lose the prepared workers.

The source-only candidate had three modified production files, net `+48`
lines (`+105/-57`), two new public types, and no new module. Its library
check failed at existing fixed-supervisor and pool consumers of the old
single-stage action. No catalogue edit was made to silence those errors.
All candidate production files were restored. The retained aggregate state
sums remain FIFO five, keyed five, fixed roster five, and fixed source custody
three; the worker preparation owner again has its original three public
progress products and two outcome alternatives.

A distinct first-attempt value is required to make source rejection available
only before any worker submission. The next hypothesis will use a consuming
start to produce `WorkerPreparationStarted` and `StartingWorkerPreparation`.
After a successful first submission, only the existing
`PendingWorkerPreparation` can continue. No arrival-history flag, repeated
cause, false cardinality, nested transition authority, semantic boolean, or
structural user syntax was retained. The worker preparation source, fixed
diagnostic law, pool recovery law, and normalized atomic actor documents were
cross-checked. Exact runtime task custody remains unproved.

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
