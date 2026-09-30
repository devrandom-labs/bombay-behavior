# Shutdown authority implementation ledger

## Baseline and law (2026-09-30)

Branch baseline: `4d44a63`. The worktree was clean. The other Bombay and
Behavior checkouts are outside this worktree and are read-only for this task.

The actor-model law is fresh allocation, communication with known recipients,
and behavior replacement as a separate operation. Agha, Mason, Smith, and
Talcott, *A foundation for actor computation*, §§3 and 5, distinguish
`newadr`, `initbeh`, and `become`; none specifies orderly shutdown.

Bombay derives staged creation and the creator-local nonce. Bombay policy is
that successful fresh installation and binding commit issue one exact
`B`-indexed capability whose protocol endpoint and `B::Event` lifecycle
authority belong to the same incarnation. A rejected creation issues neither.
Projection to a protocol recipient loses lifecycle authority. Shutdown
acceptance consumes the request; `AlreadyStopping` and `AlreadyStopped` return
the original request; later termination is separate. Reusing an address cannot
retarget a stale handle.

```text
successful fresh B installation
  -> one runtime-owned installed B value (endpoint + matching control)
  -> EstablishedActor<B>
  -> either committed child settlement or later named-child report
  -> optional protocol-only EstablishedRecipient<B::Protocol> projection

rejected installation -> complete rejected child and actions, no installed B
```

## Pre-edit aggregate-drift checkpoint

The creation aggregate has one authority: the interpreter selects one
`ChildCreationOutcome` and the behavior root consumes that settlement. The
control alternatives are `Established`, `InitializationRejected`,
`InitializationPanicked`, and `HostRejected`; the named report independently
has `Installed` and `Rejected`. The first alternative currently nests the
second sum, admitting an impossible `Established(Rejected)` value. The
subordinate outcome alternatives survive for current values: installed actor
plus ID/kind/occurrence; rejected initialized child plus error; panicked child;
or host-rejected child plus complete initialization actions and reason.
`CreationKind` survives for later lifecycle provenance. No arrival-history
alternative is needed. Current creation selection does not imply single-child
cardinality; `ChildChoice` preserves distinct occurrences.

The focused production owners contain 4,031 physical lines across three
modules (`addressing.rs` 834, `creation.rs` 2,558, `established.rs` 639). The
child outcome has four variants; the nested report has two, creating five
nominal child result combinations, one impossible. The focused transition
branches are four child-outcome arms plus two report arms. The public spellings
under review are `EndpointAddress::Established`, `EstablishedActor::issued`,
`EstablishedActor::from_recipient`, `EstablishedActor::interpret`,
`EstablishedCreation::installed`, `EstablishedCreation::into_actor`,
`ChildCreationOutcome::Established`, and `InterpretEstablishedShutdown::shutdown`.
No production edit exists yet. Cross-checks: the shutdown authority PRD,
`atomic-actor-essence.md`, `atomic-actor-retained-core.md`,
`atomic-actor-architecture.md`, `atomic-actor-downstream-impact.md`, and the
creation/lifecycle law in `AGENTS.md`. Disposition: **pass** for a focused
model experiment; the current representation itself fails the ownership law.

## Design experiment 1: exact installed value

Proposed user syntax: a runtime implementing `EndpointAddress` declares
`Installed<B>` and projects its `Established<B::Protocol>` endpoint. Only a
successful runtime installation calls `EstablishedActor::<B>::issued(installed)`.
An actor projects `recipient()` for ordinary messaging or transfers its full
installed value through a `B`-indexed interpretation operation. The named
creation report is indexed by the concrete child `C`, carries
`EstablishedActor<C>` on success, and returns a typed rejection on failure.
`ChildCreationOutcome<C, Occurrence>::Established` directly carries the ID,
kind, and actor; it cannot contain another rejection. `ShutdownEstablished`
transfers the full installed value and ingress proof to its interpreter.

The address-associated candidate owns one runtime-chosen installed
representation and permits the two implementations of a shared protocol to
select distinct event authorities. A free endpoint/control product fails the
same-incarnation construction law; a creator-local-only shutdown scope loses
unrelated targets; protocol-indexed control cannot distinguish their events.
The concrete regression must test the chosen candidate before retention.

Focused pre-production regressions: installed issuance and recipient
projection for two behaviors sharing one protocol; concrete direct and
`ChildChoice` settlement custody; named report custody; generic shutdown
acceptance and both complete-request rejections; forged strengthening and
cross-behavior compile failures; missing shutdown ingress; and a minimal
Bombay concrete-control witness. The initial compile tests must fail on the
baseline because the installed value and `B`-indexed interpreter transfer are
absent. Existing `child_creation_actor` and `exact_shutdown_action` tests
currently prove only recipient reconstruction and protocol-endpoint transfer.

Expected focused production files: `crates/behavior/src/actor/addressing.rs`,
`creation.rs`, `actor/mod.rs`, `lib.rs`, and
`crates/actors/src/protocol/established.rs`, `protocol/mod.rs`, `lib.rs` only
where re-exports actually change. Expected later mechanical migration: pool,
proxy, and supervision consumers, their tests, examples, benches, docs, and
interpreter-contract fixtures. Expected focused production delta: roughly
+70/-70 lines; migration delta is not yet estimated and must be measured.
Expected new public types: one `CommittedChild<C, Occurrence>` product and one
`B`-indexed interpretation trait; the installed address association is an
associated type, not another wrapper. `CommittedChild` owns the inseparable
ID, kind, occurrence, and exact actor produced by one committed creation; both
the direct settlement and later report consume that same product.
Expected deleted public spellings: the recipient-based `EstablishedActor` and
`EstablishedCreation` constructors/conversions; `EstablishedCreation<P, ...>`
as a protocol-indexed report. Reused lower-order laws: `Behavior`,
`EndpointAddress`, `EstablishedRecipient`, `ChildCreationProduct`,
`ChildChoice`, `Births`, `RetirementBirths`, `BehaviorLayer`, `ItemSettlement`,
`InjectEvent`, and `Ingress`. No new route, registry, or no-op control value.

The cumulative stop thresholds are more than 15 changed files, more than 500
net new production lines, or more than three new public types. Stop before
further production edits upon crossing one and request explicit authorization.

## Baseline regression record

Before production edits, `installed_actor_authority` failed with missing
`EndpointAddress::Installed`, `EndpointAddress::recipient`, and
`InterpretInstalledActor`. `child_creation_actor` failed with missing
`CommittedChild`, the direct established variant, and `into_committed`.
`exact_shutdown_action` failed because `InterpretEstablishedShutdown`
requires an endpoint instead of the installed actor value. These are expected
contract failures on baseline `4d44a63`, not fixture setup failures.

The aggregate-drift checkpoint for this first experiment remains **pass**:
the new product owns a current committed value needed by both settlement
paths, removes the impossible nested report state, and does not become a
second transition authority. Direct product, `Option`, and `Result` cannot
encode the four distinct rejected custody alternatives with the committed
product while keeping occurrence and provenance together. The residue scan
finds no proposed history state, repeated cause, false cardinality, semantic
boolean, or positional wrapper syntax. The same normalized documents listed
above were rechecked before lowering.

## Experiment 1 disposition and experiment 2 syntax

Experiment 1 is **reopen**. Its address-wide `Installed<B>` association forced
the delivery-only `DeliveryAddr` fixture to author unused installed authority.
The experiment's nine production files were restored to the baseline. Its
temporary production delta was +193/-160, net +33. The falsifier and complete
checkpoint are in `DEAD_ENDS.md`.

Experiment 2 keeps `EndpointAddress` protocol-only. A separate
`InstalledAddress: EndpointAddress` owns `Installed<B>` and recipient
projection, and only addresses that host installed actors implement it.
`EstablishedActor<B>` and committed-creation results require this narrower
runtime port. Delivery-only addresses retain their existing exact messaging
surface with no installed value or no-op implementation. The focused syntax
regressions now implement the separate trait. The same committed product and
`B`-indexed transfer remain under test.

Pre-edit drift checkpoint for experiment 2: the retained child control sum is
again four alternatives with the nested impossible established/rejected state;
the named report is installed/rejected. The current value needed in each
alternative, transition branches, 4,031 focused production lines, three
focused modules, and public spellings are the baseline recorded above.
`InstalledAddress` owns the distinct authority to issue an exact actor; it
does not own transition state. No arrival-history, repeated cause, false
cardinality, nested authority, semantic boolean, or positional syntax is
proposed. The PRD and normalized documents listed above were rechecked.
Disposition: **pass** for this revised model experiment.

Experiment 2 is **reopen** after its focused core compiled: the single new
`InstalledAddress` bound propagated through generic creation and lifecycle
surfaces in 47 actors source files. The actors crate reported 2,839 clustered
diagnostics. No caller migration was attempted. The temporary focused
production delta was +208/-157, net +51, across five core/protocol files plus
re-export roots; the candidate added three public types. The falsifier and
complete residue scan are recorded in `DEAD_ENDS.md`.

## Experiment 3: separate recipient and installed address ports

Before this edit the retained states and branch counts are again the baseline
above, with 4,031 focused production lines and no new production symbol.
The protocol-only address law belongs to `RecipientAddress`, selecting
`Established<P>`. The existing `EndpointAddress` name now refines that
port for namespaces capable of installing actors, selecting `Installed<B>`
and projecting its exact message recipient. Existing generic actor templates
already require `EndpointAddress`; they gain no new bound or wrapper path.
Delivery-only code and fixtures use `RecipientAddress` and cannot construct
`EstablishedActor<B>`. This is a deliberate source break at the runtime
address implementation boundary.

Public additions remain three: `RecipientAddress`, `InterpretInstalledActor`,
and `CommittedChild`. `EndpointAddress::Established` moves to
`RecipientAddress::Established`; `EndpointAddress::Installed` is new.
The focused regressions now spell both traits and prove that a delivery-only
address implements only the weaker port. The expected production owners add
`effects/sending.rs` and the exact-delivery/observation users in actors to
weaken their bounds. The candidate must keep the 47 generic actor-template
files unchanged; if it cannot, reopen instead of sweeping their bounds.

The direct committed product still owns ID, kind, occurrence, and installed
actor. Rejected variants retain original child/actions. No arrival history,
repeated cause, cardinality assumption, nested transition authority,
semantic boolean, or structural user syntax is introduced. The PRD,
normalized atomic-actor documents, and `AGENTS.md` creation and
compiler-friction laws were cross-checked. Disposition: **pass** to attempt a
focused model implementation.

## Experiment 3 checkpoint at the AGENTS.md file limit

The focused Behavior issuance and creation tests pass in debug and optimized
builds. A protocol-only `DeliveryOnlyAddr` implements only
`RecipientAddress`, while two behaviors with one protocol issue distinct
`EndpointAddress::Installed<B>` values. The focused actors shutdown test
passed for accepted and both rejected requests during experiment 1, before
the address-port split. It is currently blocked at actors crate compilation:
67 old `<A as EndpointAddress>::Established<P>` qualified projections still
name the moved protocol-only association. They occur in 12 actors source
files, chiefly pool and diagnostic products. They must be mechanically
changed to `RecipientAddress::Established<P>`, with each bound checked for
actual installed-actor use. No such migration has started.

An isolated scratch probe at
`/private/tmp/bombay-shutdown-authority-probe` used Bombay Communication
0.1.2's concrete `ControlSender<B::Event>` with the new Behavior trait.
One generic interpreter consumed installed values for two concrete behaviors
sharing one protocol. After the old consumer closed, its stale value returned
the exact rejected event while a new value at the same endpoint and an
unrelated target accepted their own events. The probe passed in debug and
optimized builds. It does not use Bombay's private `ActorRef` constructor or
its actual installation path; that end-to-end witness remains required.

The retained control state is the same four child outcomes with
`Established(CommittedChild<C, Occurrence>)` instead of a nested report.
`EstablishedCreation<C, Occurrence>` separately has installed and rejected;
the impossible installed/rejected combination is gone. `CommittedChild`
owns the current ID, kind, occurrence, and exact actor; the three rejected
child alternatives own the current child and their error or initialization
actions. The child outcome still has four transition arms, and the report has
two. No arrival-history state, duplicate cause, false cardinality, nested
transition authority, semantic boolean, or positional wrapper access was
added. No catalogue template has gained a no-op authority. Cross-checks:
the shutdown PRD, the normalized atomic-actor essence/architecture/
retained-core/downstream documents, `AGENTS.md`, and the primary fresh
allocation law. Disposition: **pass** for the focused Behavior model; the
full PRD remains unverified.

The complete working tree has 15 changed files, including untracked files.

Production diff: +233/-198, net +35 lines; 11 changed production files,
no new modules. Tests: +265/-30, net +235 lines including the 146-line
untracked focused test. Documentation: +266/-0 including this ledger and
`DEAD_ENDS.md`. Public API: +3 types (`RecipientAddress`,
`InterpretInstalledActor`, `CommittedChild`), -0 types; recipient-based
strengthening methods were removed. The next necessary production file would
cross `AGENTS.md`'s more-than-15-changed-files threshold. The required
authorization has been requested. Workspace, nextest, Nix, wrapper-order,
compile-fail, and complete Bombay installation gates remain pending.

## Experiment 3 refinement after authorization

The user explicitly authorized crossing the 15-file threshold on
2026-09-30. Before another production edit, the focused caller syntax was
changed so an installable `RuntimeAddr` implements only `EndpointAddress`,
declaring both `Established<P>` and `Installed<B>`. A blanket
`RecipientAddress` implementation for `EndpointAddress` implementers
projects the same endpoint representation. `DeliveryOnlyAddr` implements
only `RecipientAddress`. The compile regression currently fails because the
blanket implementation and retained strong `Established<P>` association do
not yet exist.

This refinement retains the same creation control sum, four child
alternatives, two named-report alternatives, current-value ownership, and
four/two branch counts. It changes no actor transition. It removes the need
for 67 mechanical qualified-projection edits in 12 actors source files,
while maintaining one endpoint representation per installable address. No
second endpoint cause, no-op authority, historical state, false cardinality,
nested transition authority, semantic boolean, or positional wrapper access
is added. `RecipientAddress` remains the one new weak protocol-only port.
The shutdown PRD, normalized atomic-actor documents, and `AGENTS.md`
compiler-friction checkpoint were rechecked. Disposition: **pass** to test
the refined type equation; the earlier unrefined split is not retained.

## Retained composition batch (2026-09-30)

The user explicitly authorized exceeding the 15-file threshold. The refined
address ports compile across the actors library without the 47-file generic
bound migration: actor-hosting namespaces implement `EndpointAddress`, and
delivery-only namespaces implement `RecipientAddress`. Pool assignment uses
the weak port because it performs exact delivery only. The focused Behavior
tests, actors shutdown-action test, actors library test target, and public
`established_capabilities` test pass in debug. The latter settles concrete
installed shutdown for both `StopOnShutdown<ReceiveTimeout<Watch<Stash<B>>>>`
and `Stash<StopOnShutdown<B>>`, and retains committed/rejected creation and
exact delivery.

The creation control sum remains `Established(CommittedChild)`,
`InitializationRejected`, `InitializationPanicked`, and `HostRejected`, against
the baseline four alternatives with a nested installed/rejected report. The
named report still has `Installed(CommittedChild)` and `Rejected`. The child
settlement retains four branches; the named report retains two. The
`CommittedChild` product owns the ID, kind, occurrence, and actor required by
both result paths. The three rejection variants own the current child and
their exact error or actions; no arrival-history label survives. The parent
fixture still has awaiting/active/rejected states. Pool and stable-proxy
aggregate state sums were not changed in this batch. No cardinality,
selection policy, or transition authority was inferred from their names.

The working tree now has 20 changed files, including two untracked files.
The conservative source-file count, including unit-test modules, is
**+510/-265, net +245**; integration tests are **+368/-48, net +320**;
documentation is **+340/-0**. The touched source-module count is 15. New
public types remain three (`RecipientAddress`, `InterpretInstalledActor`, and
`CommittedChild`), with no fourth public type. These measurements include
fixture control channels, so actual production-only net growth is lower.
Public spellings removed are recipient-to-actor strengthening and the
protocol-indexed creation-to-actor conversion. No second label duplicates
the successful creation cause. The residue scan found no new arrival
history, repeated cause, false cardinality, nested transition authority,
semantic boolean, or positional wrapper syntax. Cross-checks: the shutdown
PRD, normalized atomic-actor essence/architecture/retained-core/downstream
documents, `AGENTS.md` creation/ownership law, and the primary actor paper.
Disposition: **pass** for this batch; the broader fixture migration, docs,
compile-fail proofs, and runtime gate remain outstanding.

## Catalogue migration checkpoint (2026-09-30)

The generated and handwritten Behavior creation tests and the Actors pool,
proxy, fixed supervisor, dynamic supervisor, and exact-reply fixture suites
now compile and pass in debug. Their old success constructors have been
replaced by direct `CommittedChild` values carrying installed concrete child
actors. Rejected custody remains unchanged. One old test compared two
independently issued actors at the same endpoint; it now passes one actor
through the exact operation and compares a clone of that same capability.
Another test that deliberately submits the wrong creation kind now issues
distinct installed values at the reused endpoint, so the test does not treat
an address as an incarnation identifier.

The aggregate control states remain the four child settlement alternatives
and two named-report alternatives recorded above. Proxy and pool aggregate
state sums, cardinality, and selection policy were not changed. Their tests
still consume the same current values: a committed child carries ID/kind/
actor; a rejected creation retains the routed child and, where applicable,
its initialization actions and reason. No subordinate result was added to
record arrival order. Transition branch counts remain four and two. The
test-only `InstalledControl<B, Endpoint>` fixture owns a concrete typed
control channel and incarnation identity; it is not a new production or
public API type. Its endpoint projection is the only operation used by the
catalogue, while the focused shutdown regression exercises event admission.

The complete working tree has 50 changed files, including four untracked
files. Conservative source-file measurement, including unit-test modules, is
**+561/-278, net +283**. Integration tests are **+1,433/-416, net +1,017**;
documentation is **+382/-0**. New public types remain three. The wider
test-file delta reflects explicit replacement of recipient strengthening
throughout the catalogue. The residue scan finds no arrival-history field,
repeated cause, false cardinality, nested actor transition authority,
semantic boolean, or structural user syntax. The earlier trial of making
`exact_reply_templates` protocol-only failed because its concrete actor
templates themselves require an installing address namespace; the fixture
now supplies the same real typed installed control as other actor-hosting
namespaces. A protocol-only target still cannot be strengthened by a sender.
Cross-checks: the PRD, normalized atomic-actor essence/architecture/
retained-core/downstream documents, and `AGENTS.md` law and no-op checkpoint.
Disposition: **pass** for the mechanical migration; compile-fail, rustdoc,
optimized, workspace, Nix, and Bombay interpreter gates remain.

## Worker creation outcome deletion: pre-edit checkpoint

The remaining `WorkerCreationOutcome<W>` sum has `Established(actor)`,
`Rejected(WorkerCreationRejection<W>)`, and
`InvalidSettlement(WorkerCreationSettlement<W>)`. Its third alternative is
reachable only through `Err(ChildCreationOutcome::Established)` after calling
`into_actor()`. With direct `Established(CommittedChild)`, this is an
unrepresentable nested rejection: a successful child result always transfers
its exact actor. The worker aggregate must therefore select only established
or rejected custody. The focused caller syntax is an exhaustive match over
those two alternatives; it must fail against the current third variant before
production deletion.

The child creation sum remains four alternatives and the named report two;
their current-value ownership and branch counts are unchanged. The worker
outcome will fall from three alternatives to two. The pool and stable proxy
will each lose their `InvalidSettlement` arm, without adding a state or
changing selection policy. Source-module count will rise by one touched
worker module, while the production line delta is expected negative. Public
types added: none. The residue scan finds that `InvalidSettlement` is a
repeated cause and an impossible-state remnant, not future decision data.
Direct matching on `ChildCreationOutcome` preserves complete rejected
affine values. Cross-checks: the shutdown PRD direct success law, normalized
atomic-actor essence/architecture/retained-core/downstream documents, and
`AGENTS.md` aggregate-drift and custody rules. Disposition: **pass** to write
the failing regression and delete the branch.


The compile-only regression failed against the prior worker outcome with E0004:
`InvalidSettlement` was not covered. The retained worker outcome now has
exactly `Established(actor)` and `Rejected(rejection)`. Direct matching on
all four `ChildCreationOutcome` alternatives transfers the committed actor
and returns each rejected owned value. Two pool arms and one stable-proxy
arm for the impossible outcome were deleted. Focused actor library, pool,
proxy, and stable-proxy tests pass. Child creation and named report alternatives
remain four and two; the worker outcome fell from three to two. No new
aggregate state, result alternative, or public spelling was added.

The conservative source-file total, including rustdoc and unit tests, is
**+756/-349, net +407**. Integration tests are **+1,433/-416, net +1,017**;
the task has 51 changed files and three new public types. The residue scan
finds no remaining `InvalidSettlement` cause, arrival history, false
cardinality, nested transition authority, semantic boolean, or structural
user syntax. Cross-checks are the same law documents named above.
Disposition: **pass**.

## Final retained batch: interpreter proof and repository gates

The single creation authority still selects `Established(CommittedChild)`,
`InitializationRejected`, `InitializationPanicked`, or `HostRejected` for a
direct child. The later named report still selects `Installed(CommittedChild)`
or `Rejected`. The worker creation outcome now selects `Established(actor)` or
`Rejected(rejection)`, down from three alternatives. Direct child settlement
still has four branches and the named report two. Pool, proxy, and supervisor
control-state sums, cardinality, and selection policy were not changed.

`CommittedChild` owns the current ID, kind, occurrence, and exact installed
actor needed by future creator decisions. The three direct rejection variants
own the child and its current error or uninterpreted initialization actions.
The named rejection owns ID, kind, and reason. The worker rejection owns its
complete settlement. The test fixture's installed control owns a typed event
sender and identity of one channel; it records no arrival history. No new
subordinate state or transition authority survives solely to navigate the
parent aggregate. The residue scan found no arrival-history state, repeated
cause, false cardinality, nested transition authority, semantic boolean, or
structural user syntax. Cross-checks: the shutdown PRD; normalized atomic
actor essence, architecture, retained-core, and downstream documents; the
primary fresh-allocation law; and `AGENTS.md`. Disposition: **pass**.

The benchmark now issues its concrete worker handle from the same typed
installed-control fixture used by the catalogue tests. The generic shutdown
rejection regression checks the returned request's distinct control value for
both `AlreadyStopping` and `AlreadyStopped`. The isolated Bombay Communication
probe consumes `ControlSender<B::Event>` through one generic
`InterpretInstalledActor<B>` implementation for two behaviors sharing a
protocol. A closed stale handle returns its exact event, while a fresh handle
at the reused endpoint and an unrelated target accept their own events. The
probe does not claim that Bombay's existing private `ActorRef` or application
installer has adopted this contract; downstream production adoption is outside
this PRD's Behavior-side scope.

Final full-tree measurement before commit: **82 changed files, zero
untracked**; production `src` files **+766/-349, net +417** (a conservative
upper bound including rustdoc and unit-test modules); tests, benches, fuzz
fixtures, and probe **+2054/-515, net +1539**; documentation and research
record **+607/-55, net +552**. Sixteen production source files are touched,
with no new production module. Public API adds three types
(`RecipientAddress`, `InterpretInstalledActor`, `CommittedChild`) and removes
no public type. It deletes `EstablishedActor::from_recipient` and the
recipient-based `EstablishedCreation::into_actor`; direct child success no
longer nests a rejectable named report. The 15-file threshold was explicitly
authorized by the user. The conservative production delta remains below 500
net lines; public additions do not exceed three types.

Verification: focused Behavior and Actors debug and optimized regressions;
both optimized creation and shutdown custody paths; both wrapper orders;
isolated Bombay probe in debug and release; fuzz-target all-targets check;
workspace Clippy with warnings denied; `cargo nextest run --workspace`
(852/852); and `nix flake check` (all checks, including its nextest, docs,
doctests, format, package, audit, and dependency policy). The dedicated
Rustdoc failure-code script checked 51 expected compiler diagnostics. The
fuzz targets were compile-checked because this task changes the shape of a
creation result, not the stateful sequence or message parsing law they fuzz.
