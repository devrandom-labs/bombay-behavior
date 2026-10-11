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

## Empty direct-child send proof

A source-copy empty trait over the closed direct-child tuple did not let a
nominal actor factory infer its children's send/parent-report premises. Rust
cannot recover an implementation's where clauses from a generic trait predicate.
The restored pure ProxyParent witness still fails on actual event ingress.
The trait and its bounds were removed; no production or catalogue migration was
retained. Child-source compatibility must have an operational or associated
contract that directly proves the original law, without fabricated ingress,
callbacks, protocol equality or public runtime representation.

### Conditional creation execution closes the wrong recursion

Moving ChildCreation::establish's complete CommitActions premise into a sealed
ExecutableCreation interface lets a pure parent definition compile, but making
its ActorCreation availability depend on that interface produces E0275 for all
four existing recursive journeys. The constraint recursively asks for itself
through birth interpretation. No flags, extra bounds, erasure or specialization
were added. The hypothesis is removed in full; the prior working nominal
creation cut is restored. Logs: /tmp/bombay-devx-conditional-creation-proof.log
and /tmp/bombay-devx-conditional-pure-parent.log.

### Scalar context association loses the birth-product equation

One private sealed context equality preserves a finite send constraint in an
ordinary-Rust probe. The same association to CreatedActorCapabilities in the
real CreationScope prematurely normalizes ActorCreation's generic Birth and
requires Birth::Child to be Behavior. Closed heterogeneous child sums are
valid and do not satisfy that restriction. All six E0277 diagnostics precede
facade lowering. The association is removed; no suggested bound or extra
context parameter is retained. Two overlapping association equations also
fail E0284. The scalar probe does not prove the complete constructor interface.

### Birth dispatch does not repair creator-context association

Moving leaf/nonempty launch selection to the actual actor's birth algebra
removes the factory's redundant generic key and lets the library compile.
Direct-child scope guards still produce conflicting sealed context equations
(E0271) in recursive declarations and ingress failures in pure ProxyParent.
The actual caller requirements do not pass together. Both factory/dispatch
changes are removed; no additional context generic, alias or bound is retained.

### Direct-child creator equations and source supertraits remain insufficient

The complete Parent/Event/Birth/Spaces equation removes recursive context
mismatch diagnostics, but pure actor declaration still requires runtime report
ingress. A CreationSources supertrait port reduces those diagnostics without
making the valid pure definition compile. A separate context generic also
fails E0284 in ordinary Rust. All combined production representations are
removed. No actor event, context generic or capability was added to repair
these failures; pure declaration and recursive execution remain jointly required.

### Relational child-source proof covers declarations rather than the birth algebra

Parent/Child/Spaces-keyed send supertraits make named-child pure declarations
and recursive execution pass together, including 20 real consumer tests. The
facade cannot enumerate children behind an existing birth-mode alias: that
valid declaration still requires report ingress at definition time (two E0277
diagnostics). Its retained-library counterpart passes. A head/tail product
probe cannot derive a heterogeneous tail's source capability from an empty
recursive predicate. All four source-copy edits are removed. No mandatory
structural spelling, invented ingress, or arbitrary bound is retained.

### Selected source port does not follow from a whole-product marker

Independent pinned Rust probes distinguish source-proof operations from empty
proof predicates. A selected port on the existing head/tail binding product
accepts the actual child sends for heterogeneous and repeated child types and
preserves rejected allocation. The same factory with only a whole-product
predicate fails E0277; projecting a tail source also fails E0308 when given the
concrete send. Moving complete execution into associated/method scope proofs
still produces E0275. A separate recursive communications sum can execute but
would duplicate the existing birth sum. None supplies the joint pure-alias and
recursive runtime law. No production model is retained or compiler-suggested
ingress bound added. Keep the actual parent report, complete source custody and
parent-owned bindings in the next hypothesis.

A subsequent operational continuation over the existing birth sum can consume
actual InterpreterRequests and preserve Core source settlement custody, without
a duplicate communications sum. It does not yet establish fresh child execution.
The immediate-source nominal method passes recursive execution but fails a
nested pure Nursery declaration inside its body (E0277); adding another depth
premise merely moves that restriction. Bombay's retained facade fixture proves
owned staged creation and pure initialization must remain available.

The proposed unconditional routed-child InterpretItem applicability also fails
the no-op check: existing Births<Never> and RetirementBirths<Never> hosts need
only batch reservation. A private dispatcher restricted to the three built-in
child cases would remove valid external ChildCreationProduct + DispatchBirth
implementations. Three executable existing-contract probes pass in pinned Nix
at /private/tmp/bombay-empty-creation.EZUnHl. No owner production edit is retained;
preserve these empty and open ownership ports in the next hypothesis.

### Routed receiving metadata narrows valid standalone failure custody

Replacing all inhabited DispatchBirth defaults with RoutedCreation: ActionItem
preserves a sendable runtime's outside slots and makes the whole-choice consumer
run, but ActionItem's accepted result must be Send. Existing standalone
DispatchBirth only promises a Send future; its final result can be non-Send.
A lawful current interpreter initializes a Send worker at final poll and returns
an Rc-bearing error with both original allocations, current state, route, ID and
replacement provenance. Debug/optimized tests pass; the new default excludes it
with E0277. EndpointAddress also explicitly permits non-Send capabilities.
Both source-copy edits are removed, zero actual repository production/API delta.
Do not move runtime transport bounds onto standalone creation. Evidence:
/private/tmp/bombay-local-child-creation.FRZITT and the rejected routed-creation
patch/logs. The distinct receiving/owned-return timing obligations remain open.

### Unconstrained creation dispatch output weakens static custody

Owned RoutedCreation and a borrowed existing request/reply row are two real
custody inputs. DispatchBirth<Input> with arbitrary associated Output does not
preserve their fixed return law: a custom child product with Result=Never can
discard an owned request and return u64. Candidate compiles; original exact
settlement signature rejects it with E0271. Passing non-Send error, empty/custom,
replay and cancellation tests cannot waive this static forgery. Both private
production edits are removed. Formatted source-only +218/-81, zero actual API
change. Keep the input-to-return equation closed in the next model; do not add
caller bounds to excuse the trait. Independent evidence:
/private/tmp/bombay-creation-reply.nx4amE and
/private/tmp/bombay-devx-creation-row-forged-receipt.log.

### Native creation input seal omits the child relationship

Sealing only the owned-request/borrowed-row shape still permits an external
CreationInput<Addr, OtherChild> implementation for an already-sealed native
RoutedCreation<Addr, Child>. It can choose u64 and dispatch the wrong child's
original request although both declared results are Never. Pinned Nix accepts
the cross-child forgery. The complete address/child/input/settlement relationship
must be sealed, not only the storage shape. Four private owner edits are removed;
no actual production or API change. Evidence:
/private/tmp/bombay-creation-reply.nx4amE/cross_child/src/main.rs.

### Finite own-birth source admission on every creator scope

Requiring the actor's finite own-birth source premise through CreationScope::Actor
preserves actual three-generation Self/NoSends execution but rejects valid pure
Nursery→Parent declarations. Parent's own ProxyOutcome/ProxyDiagnostic ingress
becomes another execution obligation in Nursery's generated factory body.
The proposed operational port had no concrete continuation; it does not prove
source execution or full establishment. Removed 97 private lines and their
alias/traits/function; actual production unchanged. Evidence:
/private/tmp/bombay-devx-actual-scope-sources/pure-own-source.log.
The method-only variant fails library normalization before reaching the body;
its public naming surface is also removed. It is an incomplete proof rather
than a semantic falsifier: /private/tmp/bombay-devx-method-source-scope/method-library.log.

### Receiving creation delegates ownership to an unrecorded startup future

The new native receiving adapter preserves final reply-before-disposal ordering
but loses the original child when a Pending startup attempt is disposed. The
same real witness through ORIGINAL Births::InterpretCreations compiles and fails
in debug/optimized: request/reply absent, joined children/failures empty, original
weak ownership zero. Startup cancellation requests cleanup while disposal drops
the sole unrecorded join. Receiving syntax cannot cure the interpreter's custody
transfer. Reject the forwarding adapter; put startup authority and join into
parent-owned storage before awaiting. The reopened correlated candidate measured
owner production +253/-84/net169, owning tests +20/-8/net12 across four existing
modules, one sealed metadata trait; runtime production +36/-2/net34 and tests
+39/-0/net39. Control/progress/settlement cardinalities stayed unchanged, and
arm tokens fell 95→93. Every surviving value retained original request/reply,
initialization custody or committed authority; no erasure, new history, repeated
cause, nested actor authority or structural caller requirement survived the
scan. Creation/lifecycle, total settlement and normalized atomic-family laws
were cross-checked. Disposition: reopen; all candidate production edits removed.
 No actual production change. Evidence:
/private/tmp/bombay-original-creation-cancellation.znm6zpay and
/private/tmp/bombay-devx-creation-input-runtime-cancel-{debug,optimized}.log.

### Child authority reads expose unrelated startup custody

Returning the entire startup row from ChildBindingAt forces ordinary local
Behavior readers to prove settlements, terminal phase, base identity and direct
failure storage. The focused generic reader fails with five E0271/E0277 on
those unrelated premises. A CommittedChild projection removes storage exposure
but cannot perform existing typed control admission without an unproven local
actor extractor; no extractor or visibility change is added. A direct borrowed
endpoint/control/kind projection expresses the actual readers' values.

Keeping raw storage methods on the read traits with method-local premises also
fails: six blanket/tail method declarations cannot normalize the same explicit
child Base projection, even when signatures name the exact cursor child/root.
No diagnostic-origin guard is retained. Both private candidates reopen; their
read/raw representation is replaced by a separately reviewed ownership split,
not a second registry. Actual production delta is zero. Evidence:
/private/tmp/bombay-startup-authority-proof/read-before.log and
read-raw-owner-projection.log.


### Concrete runtime metadata on pure child declarations

Unconditional nominal retirement metadata passed finite counter/component
journeys but rejected an unchanged pure foreign Births alias: original passes,
candidate E0271/E0277 demands local address/endpoint capability before execution.
Generic pure child declarations have the same owner-level defect. Reopen;
retain no production from that representation. Baseline Bombay 4fd7ad6: seven
modules, CFG-scoped production +394/-69/net325, four new public spellings;
full-file net190 included moved test-only code. Binding/startup/failure counts
remain 5/2/6, arrow tokens 307→322; no new transition authority or cardinality.
The surviving metadata owns child report/failure type equations, but attaches
execution applicability to pure construction. Cross-check: pure initialization,
affine custody, local runtime readiness and unchanged alias authoring. A deferred
mode-family hypothesis passes finite pure/runtime guards; complete source,
custom native projection and recursive execution remain unproved, so it is not
retained. Evidence: /private/tmp/bombay-inferred-application.qm31zb9h.

### Projection selection with an unused custom occurrence

An associated-input replacement for ProjectTerminal makes raw descendant
selection explicit before child launch and removes the blanket coherence
collision in a pinned Rust probe. Canonical repeated roles and a custom recursive
sum preserve their original allocations. The next ordinary custom child call
fails E0282: its generic occurrence selects nothing beyond the already exact
origin. Requiring callers to annotate it would introduce a placeholder.
Disposition: reopen before production lowering. No defaults, origin extractors,
wrappers or production edits are retained. Control sums remain 5/2/6;
subordinate alternatives, branches, production lines, modules and public
spellings change by zero. Existing pending/acquired tasks and original failures
remain the only custody values. Residue: an unused structural user parameter;
no new history, repeated runtime cause, false cardinality, nested authority or
semantic boolean. Cross-check: DX03/DX05, affine retirement custody and the
no-op/compiler-friction gates. Evidence:
/private/tmp/bombay-terminal-associated.iesuua3i/RESULTS.md.

### Native input selection rejects valid pure terminal inclusions

Per-variant native ProjectTerminal associated-input selection passes four full
native journeys and a real source refusal but rejects an original-valid pure
enum: two child-source variants share one exact origin alias and accept u16 and
String. Original compiles; candidate E0119 conflates their different inputs.
Reopen; retain no production. The private five-owner batch is +139/-140 physical
lines (not a CFG production metric), zero new types and unchanged 5/2/6 control
sums; original pending/acquired tasks and native causes remain the only state.
No transition branches or modules change. Public changes were two associated
projection names and a reshaped existing port, not new control authorities.
Residue: repeated static source selection; no arrival history, semantic boolean,
false cardinality or caller placeholder. Cross-check DX03/DX05, total affine
inclusion, native projection timing and pure alias legality. Snapshot:
/private/tmp/bombay-selected-projection._oivv31s/rejected-associated-terminal.patch.
Original/candidate witness logs:
/private/tmp/bombay-inferred-application.qm31zb9h/pure-projection-{original,candidate}.log.


### Conditional descendant metadata recurses before execution

The actual inferred facade Node declaring StopOnShutdown<Node> fails E0275
when only Option<NodeDescendants> is named: no launch, source interpretation or
CommitActions is present. A conditional universal Births/RetirementBirths
projection admits pure foreign/generic/empty/nested aliases, actual FIFO and
fixed-supervisor metadata and both timer/shutdown orders, but the same recursive
Node remains red. An unconditional nominal family with the exact existing typed
report in its field fails E0277 at the field before any constructor or accessor.
The equation is Family(Base,Children) well-formedness → the closed child product
implements ChildRetirements<Base> → each child Base supplies its birth retirement
family → the original Node family obligation. Adding the suggested guard merely
restores the E0275 cycle. No extra bound, erased payload or source operation is
retained. This falsifies the current equation, not every possible nominal model.

Disposition: reopen. Actual production/test/API/module/branch deltas are zero;
control sums remain 5/2/6, with original task/request/native-failure custody.
The private alternatives select types only and add no transition authority.
Residue: recursive eligibility; no arrival history, repeated runtime causes,
false cardinality, semantic booleans or structural caller placeholders.
Cross-check: DX03/DX07, complete affine retirement, pure authoring and normalized
atomic-family laws. Evidence: /private/tmp/bombay-retirement-metadata.o4obk6bk/
{node_metadata.rs,current-node.log,blanket_node.log,unconditional_family.log}.
Published frunk_core HCons/Coproduct substitution independently preserves that
E0275 in debug/optimized; /private/tmp/bombay-devx-crate-review/frunk-metadata*.log.

Follow-up falsifies a narrower shortcut: a receipt-ready parent's own native
metadata does not imply that every pure child can name a complete native
retirement. Actual ReceiptParent with Births<OriginalActor> compiles; the pure
child's LedgerDispatch sends lack ActionItem, so its ActorRetirement fails
E0277. DriverRetirement can already carry original Actions without that guard,
but substituting original requests for accepted receipts/acquired-source custody
would discard required information. Distinct positive evidence: manually
supplied Node/Leaf schemas guarded only by each actor's own settlement contract
compile exact recursive boxed retirement data. Automatic schema selection and
execution remain unproved. This review changes zero actual production/test/API/
state/branch/module counts; existing 5/2/2/3/6/24 sums and fourteen custody fields
retain original tasks, prefixes and native failures. No new transition authority,
arrival history, repeated cause, flag, erasure or structural caller requirement.
Cross-check pure Actions, atomic-runtime-settlement, execution-ownership
DG-PROJECTION and normalized atomic laws. Disposition: reopen the proposed
parent-readiness implication, not the positive own-only nominal representation.
Evidence: /private/tmp/bombay-retirement-data.l5tbw4o8/{RESULTS.md,CHECKPOINT.json,COMMANDS.md}.

## Creator-generic factories selected by associated Base equality

The exact closed recursive child schemas and native loan execute successfully;
that does not prove arbitrary decorated creator selection. A genuine root safety
timer keeps the same Base-owned child role but the original concrete factory
rejects its changed creator type. The private comparison makes only creator C
generic per concrete Base/Role, leaving all child task types, raw descendants,
failure products and three source-custody equations concrete/body-local.

Root and Node operations nevertheless overlap (E0119): distinct associated
`C::Base` equalities do not establish disjoint native implementations. The child
task also retains C::Birth in its typed source context, exposing E0310 lifetime
premises absent from the candidate. Both profiles reject it. No marker, wrapper,
source bound or lifetime restriction was added to repair these diagnostics.
The three candidate implementations were removed. Handwriting Trace does not
prove automatic report-payload inference from arbitrary composed sends.

Disposition: reopen creator selection. Actual production/test/API/state/module
deltas zero; private runtime control sums remain5/2/24 and initialization/order/
source custody remain unchanged. The failed probe adds no actor or transition
authority, runtime registry, erased protocol, semantic flag or caller placeholder.
Cross-check: DX02/03/07/08, pure birth authoring, existing role resolution,
timer initialization/cancellation, source custody and native retirement laws.
Evidence: /private/tmp/bombay-scoped-application-p0fuke2_/
CREATOR-REJECTED.rs and CREATOR-REJECTED-{DEBUG,OPTIMIZED}.log. The corrected
caller baseline records only the missing decorated-creator operation; initial
timer constructor/reaction mistakes are not counted as semantic evidence.

## Child communications proved through a concrete sends supertrait

The finite child host is the same operational ApplicationCapabilities used by
the native kernel, independently of outer terminal/origin projection. A public
ChildCommunications supertrait reuses exact InterpretSends and associated
SourceSettlementCustody equality, leaving creation/full Commit in concrete
bodies. The external graph is red before edits on three absent-trait E0405s.
The candidate owner itself compiles, but its foreign concrete-child edges
still need nine E0277 parent-report ingress proofs. Replacing Self projections
with the already-assigned concrete Child/Descendants/Failures leaves the same
failure in both profiles. This does not prove that another generic parameter
would work or justify adding one merely to alter solver behavior.

The prototype port and export are byte-restored. The proposed two-report/timer
extension was never executed; no alias or complete-custody success is claimed.
Disposition: reopen the operational predicate. Actual production/test/API/state
deltas0; private binding/startup/retirement sums5/2/24, no added module, branch,
authority, repeated cause, flag, erasure or structural caller obligation.
Cross-check: original actual native host, typed report/source qualification,
exact sends/source custody, pure birth authoring and DX02/03/08. Evidence:
/private/tmp/bombay-scoped-application-p0fuke2_/COMMUNICATION-LAW.md,
COMMUNICATION-RESULTS.json and COMMUNICATION-CONCRETE[-OPTIMIZED].log.
## Public actor failure alias through a private occurrence shape

The derived public `ChildFailures<Actor>` alias attempts to reuse the exact
existing DirectChildFailures/ChildStartupFailures product without restating
birth rows. A real two-report recursive foreign consumer and a decorated leaf
reject the absent alias in both profiles solely on two E0425s. Making only the
existing alias public and re-exporting it then produces17 foreign privacy
errors: the private shape remains part of the projected type. The empty-birth
alias is affected too. Exposing that shape would violate the recorded candidate
and information-hiding constraint; no visibility workaround is retained.

All owner/consumer bytes are restored. Actual production/API/state/module deltas
are0; binding5/startup2/retirement24 and every failure value remain unchanged.
Evidence: /private/tmp/bombay-scoped-application-p0fuke2_/
FAILURE-PRODUCT-{RESULTS,RESTORED}.json. Disposition: reopen the representation;
next compare a closed failure sum and ordinary owned collection, preserving all
original failure causes, origins and independent retirement values.

## Inferred root sharing the child-only native custody index

A private Topology<ExecutedRoot> retains concrete recursive Node/Leaf native
operations and puts the exact complete child interpreter guard only on the root
edge. Declared domain Base equality preserves provenance but does not make the
existing NativeChild root and Node implementation headers disjoint. Both pinned
runtime profiles deny ONLY E0119 after unrelated probe placement and entry-point
errors are corrected. This falsifies that expansion before execution; it does
not show that its root-only interpreter guard itself cycles.

No exclusion marker, added bound or visibility workaround is retained. The
private ordinary effects owner is byte-restored; the denied owner-only binary
remains evidence. Actual production/API/state/module deltas0, binding5/startup2/
retirement24/native-failure6 unchanged. Disposition: reopen. Source and logs:
/private/tmp/bombay-retirement-schema-kgazqjhv/INFERRED-ROOT-OWNER-COHERENCE-*.

## HTTP shutdown eligibility for an impossible graceful-serving error

An inferred ShutdownPath added to execute_axum/run_axum repairs their existing
serve-error shutdown branch and passes four real bind-refusal/retry/serving runs.
Pinned Axum0.8.9 then vetoes its responsibility: with_graceful_shutdown promises
no error and its IntoFuture body awaits run then returns Ok(()), although its
output remains io::Result. The branch and actor capability have no executable
transition; no HTTP path parameter or caller migration is retained.

Keep real bind/startup rejection and original serving result beside independent
root custody. Prove finite natural-root HTTP completion before deleting the
unused branch. Any future upstream fallibility needs a fresh explicit owner
retirement law. Actor/runtime states remain unchanged; disposition: reopen.
Source: https://docs.rs/axum/0.8.9/src/axum/serve/mod.rs.html#334-350.
Candidate logs: /private/tmp/bombay-application-shutdown-pv2wsny6/HTTP-RESULTS.json.
