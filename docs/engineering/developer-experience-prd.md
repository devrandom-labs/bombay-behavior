# Behavior and Bombay developer experience PRD

Status: active. Scope: one canonical application path, simpler component
contracts, and a smaller documentation system. This is a specification for
staged work, not an implemented API or authorization for a broad rewrite.

The selected direction is to keep the precise Behavior algebra and make each
layer own the assembly that it can derive. Applications should declare domain
messages, state, named roles, effects, and meaningful policy. They should not
restate child positions, wrapper types, settlement products, hosting proofs,
or runtime retirement bookkeeping. Correctness and usability are joint release
criteria: a shorter example that drops owned failures fails acceptance.

The first production work should be in **Bombay application assembly and terminal
consumption**, followed by demonstrated gaps in Behavior composition. The
current evidence does not justify starting with a new Behavior trait, effect
language, application macro, or builder framework.

## Evidence and scope

Implementation claims below come from source and test bodies at immutable
revisions, not repository prose or recorded completion marks:

| Repository | Inspected main revision | Selected packages |
| --- | --- | --- |
| Behavior | `d69f992b371c12ab34e73b18e45b8112c90a1508` | Behavior and Actors 0.23.0, macros 0.14.0 |
| Bombay | `c3afb3011090ac4c9112fd39e4a76980db856df0` | bombay-rs 0.1.1, Engine 0.2.2, macros 0.1.2 |

Both remote main branches were fetched for this review. Bombay's lock selects
the published Behavior 0.23.0; its cached package's `.cargo_vcs_info.json`
identifies the same Behavior commit. The Behavior documentation branch was advanced from
`804b2bf25325a523884ec49d8a4ae6d2d2b6e9da` to this inspected main baseline
before cleanup. Both repositories use `codex/developer-experience-docs`.
Bombay's unrelated working edits
were excluded using a detached main checkout.

Bombay also selects Address 0.3.0, Communication 0.1.3, private Observe source,
and Timers 0.1.0 patched to
`13e884da7ab41781f52337b0038060e375b00ee0`. These owners remain responsible for
their current primitives. The review inspected their relevant source surfaces;
it does not certify every upstream race or future integration.

The [source coverage and documentation disposition](developer-experience-evidence.md)
records all seven executable example packages, the catalogue and capability
inventory, every backlog requirement group, and the documents to retire.
Test-source inspection establishes that a witness exists, not that it was
executed successfully in this review. Proposed requirements are labeled as
such. No claim of an exhaustive proof of all executions is made.

### Concrete findings

| Finding | Source evidence | Consequence |
| --- | --- | --- |
| Small behaviors already have concise authoring. | Bombay counter's `#[bombay::actor]`, generated `send_values`, and `ActorExt::stop_on_shutdown`. | Preserve these; do not invent another authoring system. |
| The ordinary application still exposes execution machinery. | Counter main is 144 physical lines, including a terminal enum, six-parameter `run_with` selection and a full retirement match. | Reduce required application ceremony; count teaching assertions separately. |
| Topology is declared repeatedly. | Application-topology main declares birth roles, concrete managed-child aliases, structural `ChildHead`/`ChildTail` failures, and a recursive terminal sum. | Derive structural products from the owning declaration without conflating repeated roles. |
| Atomic construction inference is defeated by hosting and retirement. | Worker-pool names the seven-parameter `SearchPool`, three protocol aliases, `SearchSpaces`, and `SearchTerminal`; supervision repeats a longer proxy/worker tree. | Fix downstream consumers before adding more constructor sugar. |
| Two application assembly surfaces remain. | `Application<Root, Members>` and `App<Root, Spaces, Families>`; pool, supervision and entity examples use `App`. | Make `Application` the sole ordinary composition spine. |
| Runtime retirement repeats a large owned product. | `ActorRetirement` has 24 variants; five custody fields each occur in 20 variant bodies. | Audit the ownership equation; factor only genuinely coexisting custody, preserving distinct failure phases. |
| Partial interpretation custody is an actual contract. | `ActionItem`, `InterpretationProgress`, `SourceProgress`, Driver ports and `completed_application_receiving.rs`. | Replacing borrowed custody slots with a consuming async call requires proof, not aesthetics. |
| Runtime configuration remains inaccessible in ordinary entry points. | `DEFAULT_USER_CAPACITY` feeds execution; external clients use `EXTERNAL_USER_CAPACITY = 1_024`. | Expose existing capacity policy coherently instead of requiring a custom host. |
| `ActorExt` covers six wrappers. | `actors/actor_ext.rs`. | Extend only for actual same-actor decorations; do not turn every catalogue constructor into a method. |
| The public surface exposes mixed audiences. | Core has 57 top-level public trait declarations; Actors has 22. Bombay re-exports the entire `atomic` module. | Classify application, component and interpreter ports; counts alone justify no deletion. |
| Existing tests teach structural access. | Timer tests inspect `sends.inner`/`sends.owned`; several application fixtures spell recursive products. | Consumer tests must prove semantic access survives an unrelated layer. |
| The finite test driver is deliberately lossy on failure. | Testkit `drive` returns only `B::Error` after `?`, dropping prior accumulated state/actions. | It cannot certify rejection custody or turn ordering; use per-turn traces for those claims. |
| Documentation is a second large maintenance surface. | Behavior has 46 Markdown files under docs, 25,410 lines; Bombay has 27, 27,053 lines. | Consolidation and deletion belong in this programme, not another permanent audit series. |

Counts include comments, blanks and embedded tests; they are review-surface
measurements, not production complexity scores. The evidence appendix links
the source for these observations. A required field is not redundant merely
because its spelling is long.

## One path and its owners

One path means one documented way for each semantic operation, built on one
algebra. It does not mean making exact delivery, logical discovery, child
creation, and lifecycle authority interchangeable.

```text
Application domain: messages, state, roles, explicit operational policy
                            |
Bombay: actor facade, ActorExt, Application, typed external interface
                            |
Behavior Actors: reusable policies and concrete compositions
                            |
Behavior: Protocol, Behavior, Actions, static composition and custody laws
                            |
Driver and primitive owners: execute, deliver, allocate, observe and schedule
```

Read this as ownership, not a new dependency from Behavior to a runtime.
Behavior remains usable without Bombay. A lower layer may never require its
consumer to reproduce an invariant that it already owns.

| User task | Selected canonical path | What must cease being ordinary caller work |
| --- | --- | --- |
| Author a Bombay domain actor | State plus `#[bombay::actor]`, ordinary enum messages and `BehaviorActed<Self>` | Manual protocol/event/empty-lane implementations and address boilerplate |
| Author a runtime-independent nominal behavior | The existing owning `#[behavior::behavior]` expansion | A second macro implementation of the same semantics |
| Author a component with service events or phases | A direct implementation of the same `Behavior` contract | Pretending a custom event algebra is a nominal message-only actor |
| Author a pool assignment worker | The existing owning `pool_worker` contract and `Assignment::complete` | Manually implementing parent completion routing or minting assignment identity |
| Emit effects | Named declared lanes and generated `Actions::send_<lane>` methods | Generic positional selectors, implicit sends and hand-written send-product traversal |
| Decorate an existing Bombay actor | `ActorExt` methods returning the owning concrete wrappers | A second normal spelling using nested constructors or application wrapper aliases |
| Construct a standalone supervisor, pool or service | Its single owning constructor, with explicit policy values | A universal actor builder or empty policies demanded by unused capabilities |
| Assemble and run an application | `Application`, on a caller-owned async host | Switching to `App`, spelling the final actor type, constructing spaces or terminal projections |
| Send from HTTP, CLI or another external client | `ActorInterface` and one real typed `ExternalActor` endpoint | Invented sender addresses, channels inside a behavior, or framework-specific request execution |
| Consume application termination | Inferred, owned application result with domain access to complete retirement | A mandatory recursive terminal enum and manual `ProjectTerminal` call |
| Add an interpreter | Explicit component/interpreter ports with concrete types | Replacing the standard Environment, using a provider registry or erasing futures |
| Test a domain actor | Consuming initialization, typed inputs, complete per-turn `Actions` | Starting Tokio for pure logic or pretending an aggregate effect trace preserves turn order |

The worker attribute is a different assignment protocol, not an alternative
way to implement an ordinary message actor. It must remain discoverable from
the same guide. Do not add a second worker facade solely to shorten its import.
Advanced implementation ports remain available where they own a real contract;
they are not competing beginner recipes.

For standalone atomic families, keep the current owning `fixed`, `dynamic`,
`fifo` and `keyed` functions as the constructor entry points. Simplify their
policy arguments and inferred results there; do not introduce parallel builders
or same-actor extension methods. Keep `MessageAdapterWithRoute::new` for the
standalone protocol hop, with its concrete destination determining routing.
The catalogue appendix selects construction versus decoration for every entry.
Named child shutdown phases use the existing `shutdown_after_children` entry
and its consuming `finish`: the intermediate type expresses the remaining
child authority, so it is not an ordinary required-fields builder to eliminate.

### Target application journey

1. Declare domain state and commands once using the existing actor facade.
2. Compose actual policies through `ActorExt`; use owning constructors for
   standalone actors. Policy choices stay visible.
3. Declare application roles once. Behavior-owned dynamic births remain
   explicit `Actions`; application startup children use `Application::child`.
   These differ in ownership and timing and must not create the same child twice.
4. Run `Application::new(actor).run_with(work).await` with type inference.
   This call shape is a **target**, not a claim that the current unannotated
   expression compiles. It must infer hosting and complete result custody.
5. Publish a named product of public recipients, establish a typed external
   customer, send a command, and distinguish admission from its later reply.
6. Select shutdown policy, await actual retirement, and inspect the inferred
   result without rebuilding its generic product. A failed attempt returns
   all recoverable original inputs alongside the reason.

The synchronous entry point is the existing explicit configured-host adapter
over this same execution; the independently retained execution/result pair
serves cancellation-sensitive embedding. Neither is a second actor system.
HTTP and Entity must use this composition spine as well.

No hypothetical full program is presented as copy-pasteable. The first
implementation deliverable is a complete compiling counter journey, including
failure and retirement handling, followed by a heterogeneous child journey.
If their inferred result requires an unproven recursive projection mechanism,
that is a design blocker; substituting `()` or dropping custody is forbidden.

## Laws that simplification must preserve

The research basis distinguishes fresh actor creation, communication and
behavior designation, and models an open system with receptionists and external
actors. These support preserving distinct capabilities and observable behavior
when changing syntax. They do not select Rust builders, shutdown policy or
Bombay's effect interpretation order. See Agha, Mason, Smith and Talcott,
[A foundation for actor computation](https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf),
sections 2 and 3, particularly the fresh-address rule on pages 19–20.

The following acceptance laws are derived static constructions or deliberate
Bombay policies unless explicitly identified above as research foundations:

| Law | Required preservation |
| --- | --- |
| Pure transition | A behavior computes state and explicit `Actions`; I/O, allocation, timers, observation and execution stay in interpreters. One incarnation processes one event at a time. |
| Complete effects | Sends, creations and next decision survive every wrapper and macro expansion. A controlled transition error does not emit a fictional successful partial `Actions`. |
| Freshness and provenance | A local creation ID is correlation, not identity. Collision rejects; replacement still allocates fresh and carries explicit replacement provenance. Restart reporting follows successful commitment. |
| Initialization and publication | Preserve the actual reserve, initialize, settle and publish order. No rejected initialization becomes publicly ready; an accepted effect prefix is never described as rolled back. |
| Affine custody | A move-only request and its recovered result have one owner. Rejection returns what is still owned; receipts cannot be interchanged or settled twice. |
| Source progress | Accepted, rejected, blocked, corrupt and unattempted items remain distinct. Source-return chains preserve causal order and terminal retention after emitter closure. |
| Capability separation | Message recipient, installed behavior authority, creator-local occurrence and stable logical name cannot be strengthened or substituted for each other. |
| Composition order | Preserve each lane's defined order and initialization order. Two wrappers are not assumed commutative. Repeated payload/protocol occurrences remain distinguishable. |
| Lifecycle knowledge | Requested stop, admitted stop, observed termination and joined retirement are different. Timeout/cancelled waiting does not prove work never happened. |
| Complete application result | Work output and cleanup outcome may coexist, including success plus cleanup failure. Unstarted work, startup rejection, interrupted work and actual task failure retain their distinct owned values. |
| Cancellation and panic | Keep already acquired inputs/results outside disposable work where required. Test construction, polling, ready-result and destructor panic cuts; `Drop` cannot promise async joining. |
| Resource bounds | Capacity, overload, retries and retirement retention have explicit owners and outcomes. Syntax cannot make unbounded retention disappear. |

The review does not infer a production bug from a large enum or many `Option`
fields. Their co-occurrence must first be mapped against source and independent
failure witnesses. Standard Rust panic payloads already appear at Bombay's
runtime edge as `Box<dyn Any + Send>`; this is not permission for erased actor
protocols. Reconcile that existing host contract explicitly in the instruction
and contract cleanup. Do not delete original panic evidence or generalize it
into a dynamic escape hatch.

## Requirements

All requirements below are proposed work. P0 blocks the canonical application
path; P1 completes its usability and evidence; P2 extends it to subsequent
product stages. Each item names the owner, required deletion or simplification,
and an observable acceptance witness. Existing safeguards are retained even
where their ergonomics change.

### Application assembly and result consumption

**DX01 — One application composition spine. P0, Bombay.** Extend the existing
`Application` ownership path to cover standard heterogeneous actors, supervisors,
pools, external capabilities and entity families. Retire `App` as an ordinary
public construction choice in the explicit breaking migration. Do not implement
a facade that merely forwards to another independently maintained lifecycle.
Acceptance: all seven example packages use the same assembly path; a domain
change never requires switching application systems. Preserve cold declaration
recovery and prove root plus two heterogeneous children, including duplicate
protocols in distinct roles and a startup failure.

**DX02 — Infer the installed topology. P0, Bombay consuming Behavior proofs.**
Derive internal occurrence products and standard local hosting from declared
births, logical-host requirements and application roles. Eliminate mandatory
`ActorSpaces` structs, repeated protocol lists, `ChildHead`/`ChildTail` and nested
failure tuples in application source. Exact external recipients must not require
local hosting; a logical external destination needs an explicit domain binding,
not a fabricated local actor. Repeated occurrences must not create independent
conflicting logical namespaces or be deduplicated into one child identity.
Acceptance: add or reorder a child without editing spaces or terminal plumbing;
an absent genuine logical binding still fails with the protocol and role named.
Do not promise automatic Rust type-level deduplication before a concrete witness
proves the projection and sharing law using the existing owning primitives.

**DX03 — Infer complete terminal custody. P0, Bombay.** Applications receive
complete concrete results without defining recursive `TerminalProjection` enums
or naming composed actor types. Keep origin and child-role distinctions and the
complete failed child definition/actions. Eliminate projection boilerplate from
counter, topology, pool and supervision before declaring success. The owning
library must account for transitive and recursive birth structures, not only
leaf actors. Acceptance: a root with duplicate child protocols and a supervisor
with a proxy/worker tree returns distinguishable exact values; wrong-role
projection and forged authority still fail to compile. A leaf-only shortcut is
not the solution for this requirement.

**DX04 — Make terminal consumption proportional to the decision. P0, Bombay.**
Provide borrowed domain inspection and consuming access to owned results without
requiring every caller to destructure runtime fields. Audit `ActorRetirement`'s
24 alternatives for a common coexisting custody product and phase-specific
causes. Retain every distinction a future decision or exact return needs. Remove
repeated cause labels and duplicated reconstruction only after proving their
equations identical. No success-only `.unwrap()` adapter, generic erased error,
automatic log-and-drop or blanket `Ok(())` discharge qualifies. Acceptance:
an ordinary consumer can decide success/failure and recover unread commands,
rejected effects, child failures, original work output and cleanup failures
without knowing acquisition-slot layout. Application-selected disposal remains
explicit; classification alone never consumes the detailed value.

**DX05 — Infer runner types and preserve host ownership. P0, Bombay.** Remove
public generic arguments that merely restate associated projections from root,
members and installed composition. Keep the user-supplied work callable local
when it need not cross a task boundary. Async execution remains caller-hosted;
configured blocking execution must reject an already entered runtime without
losing its original application/builder. The independent execution/result pair
must remain available when cancellation requires it. Acceptance: run the same
application on current-thread and multithread hosts; a non-`Send` caller-local
work value stays supported where current code permits it, while non-`Send`
transported actor values fail with a useful diagnostic. Test cancellation both
before startup and after work returns but before cleanup finishes.

**DX06 — Configure resources at their semantic owner. P1, Bombay and primitive
owners.** Expose validated user-mailbox and external-client capacities through
the canonical path; reuse Communication's configuration and admission behavior.
Define root/child/family override scope once. Preserve distinct pool backlog,
worker activation and Entity capacity limits. Include control traffic, pending
capability work, observed failures and terminal retention in resource accounting.
Acceptance: small configured capacities saturate predictably, preserve exact
rejected values and still permit lifecycle progress; invalid values fail at the
owning constructor. Do not add configurable knobs for private implementation
details or silently choose unlimited queues.

### Authoring and policy composition

**DX07 — Keep the existing authoring expansion authoritative. P1, Behavior
macros and Bombay facade.** Preserve message inference, capability-free defaults,
named sends, named births and explicit creation-settlement disposition. Remove
redundant empty declarations from the teaching path. Scope generated names and
visibility to what callers actually need, including cross-module use of named
send methods. No visibility widening of private protocols to repair generated
projection errors. Acceptance: facade and owning expansion yield the same full
actions and static denials for private/public, generic, renamed-crate, sibling
target and duplicate-role cases. No new macro is selected by this PRD.

**DX08 — Preserve semantic action authoring through composition. P0, Behavior
and Actors.** Reuse the current `Actions` methods and named `SendProduct`
generation. Remove caller-authored traversal, nested `.inner/.owned` access and
structural selectors for operations whose semantic lane is already declared.
Do not flatten two same-typed lanes by payload type. Acceptance: two unrelated
templates and both wrapper orders preserve all sends, creations, errors and
next decisions, including `Stop` with effects. Adding an unrelated layer does
not require edits to the domain actor or its semantic assertions. Internal
structural proof machinery may remain when it has a real interpreter owner.

**DX09 — Complete discoverable decoration without a second catalogue. P1,
Bombay `ActorExt`, backed by Actors.** Review watch, termination propagation,
finalization and other real decorations against the existing six methods.
Add only missing methods with an actual compositional use and delete the
competing ordinary recipe. Every method must return the existing owner type
and preserve the owner's construction error. Standalone `Machine`, queues,
supervisors and pools keep owning constructors. Acceptance: intended method
chains infer their result and demonstrate actual policy; a two-order inversion
test catches a semantically significant reorder. An extension method is not
proof that a wrapper order is lawful.

**DX10 — Distill constructors around policy, not generic slots. P1, Actors.**
Audit all catalogue constructors using the coverage appendix. Fixed and FIFO
currently take long positional policy lists; select named products only for
genuinely coexisting policy, and consuming choices only when operations differ
by phase. Keep one final spelling per family. Temporary recovery must not
require an unused source; immediate activation must not require a dummy plan;
termination diagnostics must not require a dummy route. Required choices such
as retry versus fail, shutdown drain and restart budget remain explicit.
Acceptance: fixed, dynamic, FIFO and keyed construction infer concrete types
from domain values without `Never`/`Infallible` support aliases or turbofish at
ordinary call sites. Missing semantic policy is denied statically, and duplicate
roles return complete construction custody. No default may conceal a missing
policy decision.

**DX11 — Simplify workers without collapsing preparation phases. P1, Actors
and Bombay.** Keep `Assignment` move-only completion and the existing worker
macro; applications never implement assignment settlement. Audit the repeated
worker-source declaration and execution port for what can be inferred from the
source, without moving async execution into Behavior. Initial source rejection
and a later role rejection remain distinct because the latter owns a prepared
prefix. Acceptance: immediate and async activation, source refusal, later
failure, shutdown during preparation and late completion work through fixed
supervision and a pool without no-op callbacks, cloned plans or fabricated
receipts. Remove duplicated type plumbing, not the owned prefix.

**DX12 — Make creation and roles usable without runtime identity knowledge.
P1, Behavior and Bombay.** Static startup roles are declared once; Behavior's
dynamic creation remains an explicit staged action with checked correlation.
Application tests compare issued identities relationally, not predicted numeric
nonces. Same-action child sends and observation remain creation-dependent.
Acceptance: two same-typed children, one rejected birth, transitive births and
wrapper composition retain the correct role; collision never overwrites an
actor; no API equates a creation request with a committed recipient.

**DX13 — Make timer composition local to the timer owner. P1, Actors with
Bombay and Timers.** The current wrapper API requires caller `TimerId` values.
Prove whether the ordinary wrapper's identity can derive from its occurrence
without changing the timer queue contract; this is a required focused design
experiment, not permission to delete IDs. Eliminate manual collision management
from ordinary wrapper use if that proof succeeds. Retain explicit IDs for
domain-addressed timer operations where they carry meaning. Acceptance: two
instances of one timer wrapper plus another timer layer, reorder, cancellation,
zero duration, schedule rejection, generation exhaustion and stale replay
cannot fire each other's reactions. No global counter or registry in a fold.

**DX14 — Preserve lawful restrictions on stash and state machines. P1,
Actors.** Keep stash's statically infallible replay requirement unless a new
complete custody law independently proves a fallible alternative. Do not remove
`Clone` bounds from `Machine` by discarding deferred messages or weaken phase
denials to make a fluent chain compile. Audit where borrowing, ownership return
or direct sums can remove genuine unnecessary bounds. Include bounded retention
and overflow policy for stashed/deferred work as a separate behavior change,
with a pre-edit regression. Acceptance: replay preserves successful prefixes,
stops at the correct verdict, retains deferred ownership and explains invalid
composition at the caller's expression.

### Runtime extension and Rust interfaces

**DX15 — One concrete interpreter extension seam. P0, Bombay consuming Behavior
ports.** Allow an application to supply a typed external request interpreter
beside the standard local capabilities without implementing the entire
Environment. Reuse current `ActionItem`, `InterpretItem`, source settlement and
task custody rather than introducing a second effect language. Acceptance:
two unrelated real capability kinds, mixed with standard timer/delivery lanes,
work in both layer orders; unsupported leaves fail compilation; rejection,
late completion and retirement preserve the exact request/result. Do not add
provider lookup, `Any`, boxed futures or blanket fallback interpretation.

**DX16 — Reduce interpreter boilerplate only after the custody equation is
proved. P1, Behavior, Engine and Bombay.** Inventory repeated prepare/input/
finish and source-progress implementations. Reuse existing `prepare_item`,
`finish_item`, `settle_item` and product derivation when equations agree. Keep
different custody equations separate. Acceptance: an external interpreter
crate can author a real move-only leaf without copying an unrelated lifecycle
engine; acquired results survive producer destruction/panic; corrupt traversal
retains accepted prefix and unattempted suffix. A convenient consuming async
function that loses originals on cancellation is an explicit falsifier.

**DX17 — Minimize bounds, exports and duplicate spellings. P1, all owners.**
For each exported type/trait and repeated generic, record its actual consumer
and invariant. Keep extension ports open only for legitimate third-party
implementations; seal closed algebra where generated external code does not
need implementation access. Remove unnecessary bounds at their origin rather
than adding aliases to silence diagnostics. Audit `MessageAdapter` versus
`MessageAdapterWithRoute`, termination-monitor spellings, root/module duplicate
exports and Bombay's full atomic re-export by complete semantics. Select one
canonical route-polymorphic operation where equations are identical; preserve
distinct semantics where they differ. Acceptance: compile-pass examples use
opaque non-`Clone` payloads and minimally constrained keys; public docs and
autocomplete do not make ordinary applications choose interpreter machinery.
Do not make an associated-type implementation unnameable for real downstream
interpreter authors merely to reduce a public-item count.

**DX18 — Use Rust and existing crates for their actual jobs. P1, all owners.**
Prefer standard sums/products, `NonZero*`, collections, borrowing and exhaustive
matching. Use existing `thiserror` for genuine errors and total `#[from]`
embeddings where every owned input survives. Do not force rejection reports
into `Error`, require `Debug` on every generic payload, or replace domain errors
with `anyhow`/`Box<dyn Error>`. Retain `syn`, `quote`, `proc-macro2` and
`proc-macro-crate` for the existing generators; no ad hoc parser or crate-name
heuristic. No new production crate is justified by this review. Acceptance:
deleted manual boilerplate has equivalent error sources, visibility and bounds;
dependency/MSRV/features and compile costs are measured. A builder or enum
dispatch crate is not a substitute for a missing domain law.

**DX19 — Treat diagnostics as a supported interface. P1, all owners.** Extend
the existing `Hosts` diagnostic practice to missing interpreters, unavailable
shutdown authority, wrong occurrence, invalid stash error/phase, missing policy,
wrong reply capability and transported non-`Send` values. Point at the domain
declaration or composition call with one actionable correction. Acceptance:
positive and negative consumer fixtures run under the pinned compiler,
crate renaming and minimal features; snapshot changes are reviewed, not blindly
blessed. The ordinary path never recommends writing `ChildTail`, adding a dummy
route or weakening a capability. Use stable
[`diagnostic::on_unimplemented`](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-diagnosticon_unimplemented-attribute)
and macro spans; do not promise control over every rustc diagnostic.

### Clients, lifecycle and downstream growth

**DX20 — One external-client journey with exact ownership. P1, Bombay.** Keep
`ActorInterface` as the named public capability product and `ExternalActor` as
the real sender/reply endpoint. Provide one clear one-way-message pattern and
one correlated reply pattern, with capacity and close behavior visible. Avoid
mandatory hand-written `Protocol` boilerplate where existing `MessageProtocol`
already gives the intended identity; retain nominal protocols when domains must
remain distinct. Do not add universal `ask` semantics over arbitrary message
enums. Acceptance: multi-request, slow/closed customer, caller cancellation and
late reply cases preserve exact payloads. Local acceptance is not business
completion. A no-reply endpoint is a real sender capability, not a fake worker.

**DX21 — Keep lifecycle authority explicit while simplifying access. P0,
Actors and Bombay.** One ordinary `ApplicationLifecycle` path requests shutdown
and observes termination; message-only interfaces never acquire it. Generic
installed actor authority stays indexed by the actual behavior. Include
child-derived phased shutdown and separately selected bounded drain/forced
retirement. Acceptance: unrelated observers coexist, old exact handles cannot
stop a replacement, shutdown during initialization/backoff/preparation remains
truthful, and elapsed drain deadline reports unfinished work. Removing lifecycle
fields from user source must not remove the underlying evidence.

**DX22 — Integrate Entity and HTTP without custom application engines. P1,
Bombay.** Entity hydration, admission, passivation, reactivation and family
retirement attach to the same `Application` path and typed result. Preserve the
existing Entity owner and fencing; simplify required callback and family-return
plumbing only where there is an actual lawful shared disposition. HTTP remains
an Axum adapter to the same external interface. Remove forced cloning in the
HTTP admission path where ownership can return the original value on endpoint
establishment failure; keep serialization copies when an actual HTTP response
needs separate owned data. Acceptance: an entity reactivates after retirement,
hydration rejection is visible, and HTTP 202 means admission only; domain
completion requires an explicit reply. Family and root cleanup failures coexist.

**DX23 — Preserve one progression to remote and durable use. P2, downstream
owners.** Networking, identity, placement and persistence extend typed request
lanes and application configuration; they do not change domain authoring into
another actor API. Map all downstream backlog groups through the appendix.
Zenoh is the requested networking direction, Mnesis owns persistence and
mnesis-bombay owns integration; those requirements are not claims of current
implementation. Do not add these dependencies to Behavior or implement their
protocols speculatively in this programme. Acceptance when selected: the local
domain actor participates in a two-process typed journey and a separate durable
restart journey without a new behavior trait; wire export, authority, retry,
unknown completion and commit policy remain explicit.

**DX24 — Observability must expose decisions without consuming evidence. P1,
Bombay and existing policy owners.** Expose bounded diagnostics for pressure,
activation, readiness, rejection, retries and drain through the existing owners.
Keep inspection separate from authoritative terminal custody. Avoid requiring
applications to traverse retirement internals to answer routine operational
questions. Acceptance: failed telemetry never consumes a failure or changes
actor progress; sensitive payloads are excluded by selected policy; trace
correlation is not authority; dimensions and retained diagnostics are bounded.

### Verification, migration and documentation

**DX25 — Test the same interface users write. P0, all owners.** Keep pure tests
runtime-free. Use per-turn complete traces for custody/order and the finite
testkit driver only for its narrower accumulation contract. Reuse existing
`trybuild`, `proptest`, Loom, fuzz and mutation infrastructure rather than adding
a second test DSL. Maintain a corpus spanning all seven examples, all five
atomic families and the catalogue groups in the appendix. Acceptance: every
selected change has a failing pre-edit law witness, inferred compile-pass
syntax, invalid-construction denial and an actual downstream interpreter
witness. Pure, compile-only and live evidence are reported separately.

**DX26 — Measure real developer effort. P1, Behavior and Bombay.** Record
before/after executable statements, domain declarations, plumbing declarations,
required aliases/annotations, policy decisions, imports and edits needed to add
one wrapper or child. Count test assertions separately; moving code to an
example-only support module earns no reduction. Also record clean/incremental
check time, diagnostic size, macro expansion cost, release binary/metadata size
and relevant runtime allocations. Acceptance targets are zero application
structural paths, zero hand-written standard interpreters/spaces/terminal
projections, zero unused policy placeholders, and zero full composed-type
annotations in ordinary examples. Every required domain decision remains.
Performance thresholds must be set from repeated comparable measurements
before implementation; no invented percentage reduction is a correctness gate.

**DX27 — Make cross-repository delivery a release gate. P0, Behavior and Bombay.**
Use immutable dependency revisions for owner/interpreter tests. Prove the new
owner contract with the smallest Bombay consumer before catalogue migration.
Then release/pin compatible packages and exercise extracted-package consumers,
not only local path patches. Acceptance: direct owner, facade-only and renamed
consumer builds preserve the same syntax and static denials; supported feature
combinations are checked; a Behavior release cannot claim Bombay compatibility
without the corresponding executed consumer. Migration deletes the competing
ordinary spellings explicitly rather than retaining a parallel compatibility
framework. Recheck actual Mnesis integration dependencies when that work is
selected; this PRD does not assume they are already aligned.

**DX28 — Replace the document archive with a small maintained contract. P1,
both repositories.** Apply the file-level disposition below and in the appendix.
Retain tested usage, concise normative laws, package/API reference and unresolved
requirements. Remove superseded proposals, duplicate inventories and completed
chronological ledgers from the live tree once their still-required facts have
an owner. Git preserves historical text. Acceptance: every maintained fact has
one owner; user code samples are included from compiled fixtures; source-backed
support tables distinguish tested levels; no deleted link or active obligation
is stranded. This PRD and its appendix retire after their open requirements
have been implemented or transferred, rather than becoming another permanent
architecture authority.

## Documentation deletion and consolidation

Yes, delete substantial documentation. Do not make implementation accidents the
only specification. Source is the authority for what exists; concise contract
text plus law tests records what must remain true. A conflict between them is
an explicit issue to resolve, not grounds to trust stale prose or silently
declare current behavior correct.

The retained documentation system has these distinct owners:

1. Package READMEs contain installation and links, not separate tutorial copies.
2. One Bombay application guide progresses from the counter to composition,
   topology, a worker family, embedding and operational failure handling. Its
   code comes from executable examples. Component authoring has a clearly
   separate reference for its different audience, using the same algebra.
3. Rustdoc owns API-local invariants, required capability, errors and panics.
4. Compact law documents own cross-component semantics that an individual item
   cannot express: transition/composition, installation/custody, Driver execution
   and the genuinely distinct aggregate policies. Each law links actual tests.
5. One backlog index owns unresolved requirements and reciprocal dependencies.
   Each active change has one bounded PRD and pre-edit evidence/change ledger.
   Durable design decisions retain only rationale not recoverable from code.

Behavior's `docs/SUMMARY.md` currently publishes the engineering campaign
alongside user contracts. Remove the engineering archive from normal navigation
first. Then reconcile/delete `atomic-actor-solution`, `atomic-actor-retained-core`,
type inventories, duplicate feature descriptions and closed audits as described
in the appendix. Merge current family usage into the guide/rustdoc; reduce
normalized aggregate laws to state, ownership, policy and linked independent
witnesses. Do not retain a second prose reconstruction of each match arm.

Bombay's `execution-ownership.md` is 13,280 lines before its supporting files.
Extract remaining actionable obligations, compact invariant rationale and test
pointers, then remove the chronological campaign from the working tree.
Consolidate public API, capability, module and testing documentation by owner;
file paths and test inventories should be generated where practical.

The upstream backlog currently contains **270** unique requirement rows across
its four inventory files, despite the README saying 260. It also has 22 LAW,
68 F and 10 C records. Retain these as requirement inputs, not implementation
status. Recompute counts; move completed requirements to immutable delivery
references, and keep open requirements in focused groups. Do not delete them
because a broad status row says a larger programme merged.

Do not add another policy script that interprets all prose. Link/check fixture
inclusions, support evidence and open IDs with small deterministic checks using
the existing documentation pipeline. Preserve `AGENTS.md` provenance and
aggregate-drift obligations in active work; consolidate duplicated instructions
only with a faithful comparison and an explicit instruction change. Documents
do not themselves grant permission to weaken a law.

## Acceptance matrix

Every affected seam must be checked across the relevant rows; a row's presence
here is not a claim that the current implementation fails it.

| Dimension | Required witnesses |
| --- | --- |
| Authoring | Plain domain actor, service-event component, assignment worker, generic/private/public types, renamed dependency, direct/facade import |
| Composition | Two unrelated templates, both orders, duplicate owned event types, duplicate effect types, zero/one/many lanes, inner initialization plus wrapper initialization |
| Topology | Leaf, two heterogeneous children, same protocol twice, transitive/recursive births, application-declared and behavior-created children, absent logical host |
| Effects | Continue/goto/stop with effects, accepted prefix plus rejection, dependency-blocked item, corruption plus untouched suffix, source return after emitter closes |
| Startup | Allocation refusal, initialization error/panic, accepted prefix followed by failure, immediate stop, late readiness, exact publication visibility |
| Atomic policies | All five families; immediate/async activation, fixed/dynamic membership, retry/fail, FIFO versus keyed dispatch, replacement failure, stale completion, shutdown during preparation |
| Execution | Current-thread/multithread hosts, no host, already-entered blocking call, non-`Send` caller work, actor serialization, independent progress |
| Custody | Non-`Clone` payloads, construction/poll/disposal panic, cancellation before/after admission, independent result receiver, work success with cleanup failure, missing result without fabricated success |
| Capabilities | Exact versus logical route, old exact handle after replacement, message-only versus lifecycle authority, independent observers, wrong child occurrence |
| Resources | Capacity one, saturation, closing customer, pending work, bounded retry, control flood, retained terminal growth, graceful deadline versus forced retirement |
| Client integration | Counter, heterogeneous application, templates, supervisor, FIFO/keyed pool, Axum, Entity; request admission and business reply distinguished |
| Distribution growth | Explicit wire export, authenticated authority, unknown delivery, stale incarnation, durable commit/fencing, schema incompatibility; only executed evidence supports claims |

Required type denials must remain compile failures; runtime checks do not
replace them. Model/property oracles must be independently structured. Replay
lifecycle and generation cases in optimized builds. A deliberately restored
defect must break its regression for the intended reason. No mutation, transfer
or required call may live only inside an assertion.

## Delivery sequence and stop conditions

| Stage | Scope and prerequisites | Required exit and removed work |
| --- | --- | --- |
| 0 | DX25–28 evidence baseline; select source revisions and preserve open obligations | Reproducible consumer corpus, measured syntax, document disposition; no production changes |
| 1 | DX01–05 and DX21 counter plus heterogeneous child witnesses | Inferred application and full retirement with no manual terminal projection; smallest real interpreter proof |
| 2 | DX02–03 and DX10–12 through fixed supervisor and FIFO, then dynamic/keyed/proxy | No structural topology aliases or repeated host declarations; exact failure/replacement custody |
| 3 | DX07–09, DX13–19 through two unrelated compositions | Owner-level gap fixed only if a failing witness requires it; duplicated mechanisms removed |
| 4 | DX06 and DX20–24 through HTTP, Entity and typed external work | Same application spine, resource controls, explicit terminal decisions and no second Environment |
| 5 | Mechanical catalogue/example migration, DX26–28 package gates and final documentation pruning | All canonical examples and selected support claims verified; retired surface deleted |
| Later product stages | DX23 and the explicit downstream backlog | Remote/durable/deployment proofs owned by their own PRDs; no speculative core expansion |

Stages are dependency order, not authorization for one giant PR. Before each
production experiment record the exact law, classification, desired syntax,
failing regression, existing owners reused/deleted, expected files, line delta
and public types. Separate design from mechanical migration. The first new
no-op input, structural workaround or compiler-invented bound reopens the model.
Do not migrate a second caller around it.

Each retained semantic batch must record the repository's complete
aggregate-drift checkpoint: control states and subordinate alternatives before
and after, current value needed by each survivor, branches/production lines/
modules/public spellings, residue scan, cross-checked laws and `pass` or `reopen`.
For stale normalized documents, record the contradiction and reconcile it;
do not use the document's title as evidence. A reopened candidate is removed
without touching unrelated work and its falsifier recorded in `DEAD_ENDS.md`.

Keep the cumulative production budget checkpoints: more than 15 changed files,
500 net new production lines or three new public types requires the instructed
expanded-scope approval. This PRD does not waive them. Each stage needs its own
truthful ledger and reviewable result; artificial slicing cannot bypass the
cumulative limits.

Run focused debug and optimized tests before broadening. Required final gates
include Behavior's pinned-toolchain workspace nextest and `nix flake check`,
Bombay's pinned-Nix build/tests/fmt/Clippy and authoritative flake checks,
applicable model/fuzz/mutation checks, published-package consumers, all-target
examples and documentation links. Broaden testing only for changed or unresolved
contracts. No compatibility claim follows from an owner-only green build.

## Rust tooling selection

The selected approach follows the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
on conventional interfaces, useful diagnostics, documentation and appropriate
generality. It does not use guideline names to justify a new abstraction.

Use the already selected [`thiserror` 2.0.20](https://docs.rs/thiserror/2.0.20/thiserror/)
for standard error implementations and truthful source relationships.
Use [`trybuild` 1.0.121](https://docs.rs/trybuild/1.0.121/trybuild/) for caller
syntax and diagnostic regression fixtures. Keep the existing property,
concurrency, fuzz and mutation tools in their owning test layer. Tokio continues
to own executor construction; ordinary Rust `impl Future` keeps static dispatch.
Crate upgrades require a separate compatibility reason, not a desire to shorten
the manifest. No new crate is a prerequisite for the initial application and
terminal experiments.

## Completion and change record

### Requirement checklist

This is the single implementation checklist. `Open` includes unexecuted
acceptance; no source inventory or focused pass closes an entire requirement.

| ID | Implementation and tests | Remaining work / disposition |
| --- | --- | --- |
| DX01 | Source-copy Application owns explicit hosts; real supervisor/FIFO journeys and both namespace-refusal branches pass | Open: retain the public model, unify App, all seven journeys, cold recovery |
| DX02 | Child bindings, logical Hosts and exact delivery inspected | Open: inferred hosting, repeated roles, missing-binding denial |
| DX03 | Five candidate caller fixtures and original invalid-child denial pass; nominal born-role borrow/consume preserves original recursive boxes and unmatched siblings | Open: retain the public model, cover all legal bases, inferred hosting and complete owner gates; finite authoring passes but recursive execution remains red |
| DX04 | Lossless work/cleanup and borrowed state, custody and completion; all 24 outcomes retained; completion passes 299 debug/optimized tests and independent owner/consumer inversions | Open: published macro dependency and final package gates; inferred descendants and complete proportional recovery |
| DX05 | Existing execute/run work and caller-host contracts inspected | Open: infer associated runner types; cancellation and host corpus |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Pure actor authoring retained, including single/heterogeneous birth-mode aliases and owned nested staged creation; ordinary and pool-worker rustdoc fixes, renamed consumer and original denials pass | Open: publish macro fix; resolve factory availability with recursive creation; complete generic/private corpus |
| DX08 | Composed SendInput routes unique lanes; six owner regressions, OneShot/Periodic in both shutdown orders, complete Actions and two static denials pass | Open: downstream dependency delivery and full named-lane catalogue evidence |
| DX09 | Existing ActorExt inspected | Open: prove missing real decorations and delete duplicate recipes |
| DX10 | Existing constructor owners retained | Open: measured policy syntax across all five families |
| DX11 | Existing preparation and completion custody retained | Open: source refusal, prepared prefix, shutdown and late completion |
| DX12 | Existing occurrence/creation owners inspected | Open: inferred roles, relational identity and rejection corpus |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Receiving loans and child retirement inspected | Open: independently proved leaf custody/boilerplate reduction |
| DX17 | Retained custody accessor/product and composed send changes have bounded interface provenance | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial executed | Open: complete pinned diagnostic corpus and renamed/minimal consumers |
| DX20 | Existing ActorInterface/ExternalActor retained | Open: correlated replies, cancellation and slow/closed customers |
| DX21 | Existing lifecycle and child retirement retained | Open: inferred access with complete shutdown/retirement evidence |
| DX22 | Existing Entity/HTTP owners retained | Open: same Application spine, exact rejection and family/root failures |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inferred counter/recursive/component fixtures; timer readiness uses actual reports and duplicate rejection | Open: retain inferred API; complete downstream corpus and owner gates |
| DX26 | Counter baseline 144 lines; native linking exhausts 4.8/8.3 GB; compact Nix profiles execute finite witnesses with space remaining; current counter main is 136 lines, with terminal/runner ceremony still present | Open: canonical caller/compiler/runtime measurements and final resource gates |
| DX27 | Timers 0.2 released/pinned; extracted/MSRV and renamed facade evidence; owner PR #89 passes exact-head native ten-gate/927-test, package preflight and all CI including strict mutation | Open: owner review, release/pin; package final inferred API and execute downstream denials and journeys |
| DX28 | Initial documentation pruning preserved | Open: compiled guide inclusions, reconciliation and final link gates |

### Retained laws and evidence

The following are bounded retained changes. A passing row does not close an
entire DX requirement. Detailed pre-edit records and exact source diffs remain
in the linked commits; this record states the current contract once.

| Change and commit | Law, ownership and pre-edit falsifier | Retained surface and verification |
| --- | --- | --- |
| Application result inspection, Bombay `6d1d493` | Derived ownership law: borrow with `work_output()`/`cleanup()`; `try_into_completed()` transfers both output and independent cleanup Result. All other phases return the entire original outcome. Fourteen missing-method E0599 diagnostics preceded production. | Production including example +64/-8/net +56; tests +310/-0; three methods, zero types. Seven focused tests pass debug/optimized; 65 custody/runner tests pass. Independently rejecting completed work when cleanup fails breaks the ownership test. |
| Retained state inspection, Bombay `a9b88dc` | Derived: `behavior()` borrows the original actor in all 21 state-owning alternatives, including rejection. Only ActorTaskFailed/Panicked/Cancelled return absence. Six missing-method E0599 diagnostics preceded production. Presence certifies no success. | Production including example +38/-0; tests +74/-1; one method, zero types. Nine consumer tests pass debug/optimized; 357 owner/consumer preservation tests pass. Returning absence breaks allocation, counter and startup witnesses. |
| Common terminal custody, Bombay `fe831e9` | Derived: the identical 14 coexisting fields belong to one `ActorCustody`; all phase-specific effects, settlement/progress values and primary causes remain in their original alternatives. `custody()` borrows the same product. Two real counter/startup E0599 failures preceded production. | One public product and accessor; no bound or transition added. Production excluding embedded tests +570/-876/net -306. Thirty-five paths migrated after focused proof. 357 debug/369 optimized tests, all original denials, workspace/all-target/all-feature Clippy and actual counter pass. Independent absence inversion breaks exactly the two focused journeys. |
| Normal completion inspection, Bombay `66d1aaa` | Derived: `completion()` borrows the actual Stopped/Exhausted field only from Completed. It certifies no absence of child/late cleanup failures. Six E0599 failures preceded production. | Production +36/-0; tests +39/-3 including example; docs +8/-0; one method, zero types. 290 owner + nine consumer tests pass debug/optimized. Independent absence inversion breaks owner and real consumer. Strict workspace Clippy, formatting and actual counter pass. |
| Composed named sends, Behavior `e09ccfa` | Derived: unique `Actions::with_send`/`SendEffects::send` inputs route through wrapper effects without dropping or reinterpreting inner effects. Pre-edit owner and real Bombay witnesses fail inner routing. Owned selection is `(Own, Path)`; inner selection is `Inside<Path>`. Repeated/missing capabilities remain E0283/E0277. | Production +35/-2 (24 net rustdoc lines); tests +231/-1; guide +9; zero new types/methods. Four core and two Actors regressions, both timer/shutdown orders, preserved original allocation/replacement provenance and complete Actions pass. Removing inner emission breaks all four core regressions. Explicit owned-path syntax is a breaking interface change. |
| Facade macro crate roots, Bombay `a431c2a` and Behavior `2ea569a` | Derived static namespace law: Bombay's existing `extern crate self as bombay` resolves its library, sibling targets and rustdoc. Valid real actor/worker doctests fail independently under the old CARGO_CRATE_NAME guesses. Reuse proc_macro_crate for renamed external crates. | Delete the guesses; Bombay source +2/-5, no new symbol/type/bound/alias. Six unit and six resolution tests including executable actor/worker doctests pass. Both original defects were restored independently and fail the intended witnesses. Actual Bombay still locks the old published owner macro. |
| Pinned Nix artifact profiles, Bombay `ec362f1` | Verification policy: check derivations disable debug information/incremental artifacts; developer shells unset these controls. No check is removed or weakened. | No Rust semantic surface. Task-owned completed build caches may be reclaimed with source/logs preserved; unrelated checkouts and the other AI's main-2 are preserved. |

For common custody, the terminal owner shrinks 2630 -> 2323 physical lines.
The counter retirement object measures 688 -> 696 bytes on pinned Rust 1.99,
an eight-byte alignment cost with no new allocation. Counter main currently
has 136 lines (218 total Rust lines), compared with the initial 144-line main.
It still declares a terminal enum, explicit runner parameters and projection;
no canonical application-ceremony acceptance is claimed.

Retained aggregate checkpoint: ApplicationOutcome remains six alternatives;
ApplicationWorkCustody five; ActorRetirement 24, with 21 state-owning cases.
Twenty retirement alternatives share the same 14-field product; allocation
refusal and three unavailable-state cases keep their distinct custody laws.
CreationBinding remains Established/Rejected/StartupRejected. Actor/wrapper
control states, initialization order, runtime transition branches and module
ownership are unchanged. Inspection adds exhaustive borrowed selections,
not transition authority. SendLayer retains its two existing named fields and
two disjoint static delegations. Every surviving value owns its original actor,
unread communications, current effects/progress, descendants, exact failures,
allocation or receiving/cancellation slot needed for later recovery.

Residue scan: no new arrival history, repeated cause, false cardinality,
nested actor engine, semantic boolean, erased protocol, structural caller
requirement or compiler-origin bound. Cross-checks: affine result law,
execution-ownership DG-API/DG-PROJECTION/DG-WORK, Driver D-RETIRE-1, complete
source/retirement custody, Core transition/effect laws, timer generation and
initialization, and both shutdown orders. Normalized atomic-family state laws
are unchanged by these batches. Disposition: pass for the bounded retained
laws; inferred topology and recursive runtime creation remain open.

The expanded selected-programme surface was explicitly authorized on
2026-10-09. That authorization does not waive per-stage law, provenance,
composition, aggregate-drift or verification gates. Stage 1 must finish before
catalogue migration or later programme stages.

### Current inferred-creation proof

The desired caller is one `Application` with domain roles, inferred hosts and
complete root/descendant retirement. Caller code must not declare structural
spaces, composed aliases or recursive terminal enums. Pure facade authoring,
including existing single/heterogeneous/empty birth-mode aliases, remains valid
without runtime ingress. A creation request retains freshness/provenance and
must return its complete rejection; acceptance follows successful fresh launch,
binding and initialization, before same-action sends and dependent observation.

Retained Bombay `cff086b` registers five caller pass fixtures and the original
invalid-child denial. Four new fixtures contribute 986 test lines and zero
production lines/types. They cover counter, heterogeneous/repeated children,
plain state base, recursive creation/refusal, born roles and timer readiness.
The retained library lacks the inferred API; its full suite is intentionally
red. `b3dee64` reinforces existing pure birth aliases with 26 test lines and
zero production: both authoring tests and four original denials pass debug and
optimized. The nonlocal facade regression (`ec79d9f`) likewise preserves pure
construction without manufacturing local runtime capabilities.

A deeper pure-authoring regression is retained before the next runtime edit:
an ordinary actor may stage a child whose own birth alias contains a reporting
actor, even when that child has no runnable report ingress. Pure initialization
returns the complete staged request; it neither allocates nor certifies runtime
installation. Derived ownership equation: pure parent -> Actions containing
original child request/ID/provenance; owned extraction -> same child, still
independently pure-initializable. Reuse existing actor facade, Births,
CreationSequence, CreateChild, Creations and Behavior initialization. Extend the
existing actor_facade_full compile-pass fixture, zero production/API changes;
retain the four original denials. The immediate-source generic-method probe
fails this nested declaration with E0277 inside the factory body. That failure
must not become a bound or ingress requirement on ordinary actor authoring.
The retained fixture adds 39 test lines, zero production lines/types. Both
authoring tests and all four original denials pass in debug and release harnesses;
trybuild's fixture compilation itself uses the development profile. An independent
registry-backed consumer runs that exact fixture as an optimized executable
(4.99 seconds). Logs: /private/tmp/bombay-devx-nested-authoring-{debug,optimized}.log
and /private/tmp/bombay-devx-nested-authoring-consumer-optimized.log.

The unretained source copy at /private/tmp/bombay-devx-custody-context now uses
the retained ActorCustody/completion schema and the prior nominal retirement
metadata experiment. Its separately patched consumer at
/private/tmp/bombay-devx-custody-consumer passes 16 finite tests in debug and
optimization: counter/component, namespace refusals, FIFO, supervisor restart,
timer/shutdown orders and pure existing/empty birth aliases. Timer evidence
uses actual elapsed reports and replay rejection, never a fixed-delay claim.
A fresh recursive check fails six E0275 diagnostics. Actual Bombay remains
registry-backed; these source-only results do not certify its package or gates.
Logs: /private/tmp/bombay-devx-custody-consumer-{finite,optimized,recursive}.log.

Earlier nominal source-copy factories proved recursive runtime journeys but
invalidly required report ingress on pure declarations. A relational source
port made named-child declarations and all 20 consumer tests pass, then failed
the valid existing birth alias with two E0277 errors. All four candidate edits
were removed. No placeholder policy, widened protocol, erased future, forged
source capability or additional context bound is retained.

The authorized independent design reviews refine the falsifier:

- Complete CommitActions proof belongs inside a nominal future body. Making
  associated/method scope applicability depend on that proof still fails E0275.
- An opaque whole-product source marker does not reveal heterogeneous leaf
  premises. A projected tail proof compiles but rejects actual concrete sends
  with E0308.
- A method-bearing recursive source sum can preserve exact rejection, but it
  would duplicate the existing closed sum and is not retained.
- A selected source port on the existing binding product accepts actual child
  sends, heterogeneous/repeated types and exact rejected allocation. Removing
  the separate selected-port premise still fails E0277 despite the whole-product
  marker. Missing ingress also remains E0277. This is not the complete factory.
- An operational continuation over the existing birth sum now interprets real
  InterpreterRequests<ReportToParent<_>> and offers the exact retained source
  settlements. Single/heterogeneous/repeated aliases pass debug/optimized with
  warnings denied; missing parent ingress is E0277. No copied settlement kernel
  or second communications sum is used. Fresh installation and cancellation
  remain unproved.
- Moving only immediate-child source premises to the nominal factory method
  passes four recursive generations but rejects nested pure staging with E0277
  inside the method body. Calling the nominal factory after actual Core source
  interpretation also fails E0277 for the child's own birth-source premise in
  debug and optimized builds (core_source_continuation/descendant_{debug,optimized}.log).
  Additional depth bounds would move that defect.
- A proposed unconditional routed-child InterpretItem port rejects valid empty
  Births<Never>/RetirementBirths<Never> interpreters needing only batch reservation.
  Restricting dispatch to Behavior/ChildChoice/Never also removes an executable
  custom ChildCreationProduct + DispatchBirth ownership port. Three focused
  pinned-Nix tests establish those existing contracts before any owner edit.

The next hypothesis must provide runtime-owned full child execution with finite,
nonrecursive applicability, jointly proving pure aliases, recursive creation and
operational selected-source interpretation/settlement offering. It must preserve
actual LocalParentReports, create-before-send order, SourceSettlementCustody
identity, original receiving loans and parent-owned binding maps/ProjectedTask.
A child task may not borrow or take its parent's binding row. Existing
ChildBindingAt selects exact occurrence; structural routing must not acquire
unrelated runtime source requirements.

The routed-creation experiment is reopened. Derived interpreter law: the whole
existing request reaches the runtime before selecting a child, preserving exact
ID, route, provenance, actor allocation and independent later effects. Reuse
DispatchBirth, batch reservation, existing request/reply rows and InterpretItem;
no second dispatch protocol or logical send capability. The whole-choice
consumer fails nine E0277 diagnostics before source edits. Scope was two private
owner files, zero new states/modules/types; no actual production migration.

Two source-only designs are removed. Making every default request an ActionItem
adds Result: Send and excludes a lawful Send future producing a non-Send
initialization error. Splitting actual owned-request and borrowed-row inputs on
DispatchBirth preserves that caller, empty aliases, custom products and pending
cancellation, but an unconstrained associated Output permits a custom product
with Result=Never to drop its request and return u64. The original trait denies
that program with E0271; the candidate accepts it. The exact input-to-return
equation must remain statically closed. These laws are derived ownership laws,
not actor-model mandates.

Focused evidence: whole-choice allocation rejections plus independent report
and replay pass debug/optimized; four owner tests cover standalone non-Send
error, two empty birth aliases and a custom receiving-only product; cancellation
retains both exact requests and suffix. An optimized cancellation inversion
fails independently and restored source passes. A real Behavior::Sends declaration
still denies routed creation with E0277 (missing InterpreterRequest). Generic
low-level InterpretSends availability alone is not that static protocol proof.
Sources/logs: /private/tmp/bombay-devx-{routed-creation,owned-creation}-consumer,
/private/tmp/bombay-devx-creation-row-*.log, /private/tmp/bombay-local-child-creation.FRZITT,
and the independent forged-receipt comparison /private/tmp/bombay-creation-reply.nx4amE.

Aggregate checkpoint, removed row-input candidate: progress alternatives 3→3;
creation custody 2→2; item settlements 4→4; child outcomes 4→4; creation settlements
3→3. Original request/reply, current initialization error/actions and committed
child remain the exact owned values; no arrival-history state is added. Two
owner modules and 146 public declaration tokens remain; associated Output and
Input change the public promise. Match-arm tokens 97→95, if tokens 19→19 across
the two affected modules (diagnostics, not minimality proofs). Formatted private
production +218/-81/net +137; zero actual production/API delta after removal.
Residue scan: no semantic flags, erasure, new cardinality or nested authority;
borrowed structural rows are interpreter-only. Cross-check: creation/lifecycle
law, complete settlement law and normalized atomic actor documents. Disposition
reopen because the static return equation is weakened; no factory, retirement,
wrapper-order or recursive execution acceptance follows from these focused passes.

Disposition: reopen; zero actual repository production states/modules/public
types added by these reviews. Falsifiers are consolidated in [DEAD_ENDS](../../DEAD_ENDS.md).
Probe sources/logs remain under /private/tmp/bombay-birth-source-proof.BUziey
and /private/tmp/bombay-devx-nominal-creation-review. No proof-only marker or
selected port is promoted into production. Earlier candidate denials proved
wrong role E0277, forged scope E0277, forged creation E0451, nonlocal application
E0599, wrong birth E0271/E0277, extracted creator E0616 and foreign born role
E0277. Those results must be re-established for the final retained interface;
missing/deleted symbols do not substitute for its authority denials.

### Dependency delivery and exact gates

Initial source revisions remain the inventory baseline in the appendix.
Current actual Bombay selects registry Behavior/Actors 0.23.0, macros 0.14.0,
Address 0.3.0, Communication 0.1.3 and Timers 0.2.0, with no Behavior/Timers
Cargo source patch. It uses its pinned Rust 1.99 shell/MSRV 1.96; Behavior's
owner gates use pinned 1.95. Foreign-platform execution is not inferred.

Timers was a concrete package blocker: registry 0.1 lacked ScheduleError and
returned Token where checked source 13e884d returned Result. Publishing Timers
0.2.0 preserved the checked scheduler and exhaustion law. Timers PR #8 merged
as 9369d1f after all eight native gates (eight tests, zero skipped), optimized
unit/semantic tests and verified package. Registry 0.2 source equals the selected
checked source; MSRV is 1.96. Metadata-only owner change was five paths,
+18/-8, zero Rust/API. Bombay `9ce1afd` removed the patch and pinned registry
0.2: five metadata paths +17/-11, zero Rust/API.

At that pin, verified Bombay's 211-file archive and unpatched extracted default
and all-feature consumers pass, also all features on pinned MSRV 1.96. The
renamed facade-only extracted consumer executes the real generic/hygiene/
typed-product regression. Immutable Bombay `9ce1afd` and `ec79d9f` each pass all
21 native Nix gates, including all seven examples. These results precede the
new inferred acceptance fixtures; they do not certify the current branch.

Current owner delivery [PR #89](https://github.com/devrandom-labs/bombay-behavior/pull/89)
on codex/devx-owner-contracts at b0e9c93 contains only proven send routing,
facade macro hygiene, their regressions/law and mutation-baseline reconciliation:
eight files, +337/-14 physical lines. It is ready for review and labelled
semver-breaking for explicit owned SendInput paths. Exact-head fresh native
`nix flake check --show-trace` passes all ten gates: build, nextest (927 passed,
zero skipped), Clippy, docs, doctests, package, fmt, TOML fmt, audit and deny.
Release preflight passes all four Cargo workspace lockfiles, README versions
and extracted archives. Exact-head GitHub CI run 38058209730 passes API audit,
Linux Nix, all eight mutation shards and strict aggregate gate; CodeQL and
dependency policy pass. No merge or publication is claimed.

The first aggregate campaign failed bookkeeping after all shards passed:
the old emit floor named a superseded function and omitted the two replacement
paths. Both new emit-no-op mutants were caught after successful compilation.
Preserve the original owned floor of one, add one for inner emission and remove
only the nonexistent key; no viable floor is lowered or mutant waived. The
strict rebased eight-artifact campaign passes 205 viable/415 total candidates;
new-head CI independently passes. Feature/delivery branches both retain the
same correction (`1a0f516`/`b0e9c93`).

Previous Darwin signing and disk failures are not waived. Native gates above
actually succeed. Disk-starved attempts, interrupted compilations and stale
mutated binaries are excluded from passing evidence. Completion/accessor
inversions compile and fail independently; their original source is restored.

Fresh Bombay `3c75768` full `nix flake check --show-trace --keep-going
--max-jobs 1 --cores 2` is interrupted after 23m26s. Its recursive caller fixture
uses 19m18s compilation without a terminal diagnostic and reaches 4.84 GB RSS;
free disk falls to 1.2 GB. Completed task-owned targets are reclaimed without
touching the other AI's checkout/builds. Only formatting has a verified valid
Nix output; no full pass, signing fix or waiver is inferred. Cancellation ends
both observed processes and leaves 2.7 GB free. Full gates remain open until
the inferred recursive interface is implemented. Log:
/private/tmp/bombay-devx-nested-full-nix.log.
Current full Bombay gates remain open on inferred application compilation and
the published owner macro's rustdoc path. They must be rerun after delivery
and retention of the final inferred interface; historical native passes do
not override those blockers. Package final canonical syntax and every required
authority denial against actual released dependencies before closing DX27.

### Remaining completion obligations

Use the checklist above as the sole requirement disposition. Next exit is the
joint counter/heterogeneous/recursive law, then supervision/pools and their
complete failure/retirement journeys. Continue dependency-ordered stages;
ordinary ActorExt, owning constructors, named Actions and one Application
remain selected. Catalogue migration, HTTP/Entity unification, capacities,
external lanes, interface simplification, comparable compile/runtime costs,
final package gates and compiled guide inclusions remain open. Networking,
durability, identity and deployment stay explicitly deferred under DX23.

The initial documentation pruning and immutable requirement/failure crosswalk
are preserved. This consolidated contract replaces repeated chronological
experiment notes; Git commits preserve their pre-edit provenance. Do not
restore retired ledgers or declare the programme complete from this record,
finite happy paths or partial green gates.
