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
| DX03 | Five candidate caller fixtures and original invalid-child denial pass; nominal born-role borrow/consume preserves original recursive boxes and unmatched siblings | Open: retain the public model, cover all legal bases, inferred hosting and complete owner gates; factory authoring remains red |
| DX04 | Lossless result access and borrowed state; one 14-field ActorCustody product for 20 outcomes, all 24 outcomes retained; 357 debug/369 optimized tests and accessor inversion pass | Open: published macro dependency and final package gates; inferred descendants and complete proportional recovery |
| DX05 | Existing execute/run work and caller-host contracts inspected | Open: infer associated runner types; cancellation and host corpus |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Pure actor authoring retained, including single/heterogeneous birth-mode aliases; ordinary and pool-worker rustdoc fixes, renamed consumer and original denials pass | Open: publish macro fix; resolve factory availability with recursive creation; complete generic/private corpus |
| DX08 | Composed SendInput routes unique lanes; six owner regressions, OneShot/Periodic in both shutdown orders, complete Actions and two static denials pass | Open: downstream dependency delivery and full named-lane catalogue evidence |
| DX09 | Existing ActorExt inspected | Open: prove missing real decorations and delete duplicate recipes |
| DX10 | Existing constructor owners retained | Open: measured policy syntax across all five families |
| DX11 | Existing preparation and completion custody retained | Open: source refusal, prepared prefix, shutdown and late completion |
| DX12 | Existing occurrence/creation owners inspected | Open: inferred roles, relational identity and rejection corpus |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Receiving loans and child retirement inspected | Open: independently proved leaf custody/boilerplate reduction |
| DX17 | No exports or bounds changed | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial executed | Open: complete pinned diagnostic corpus and renamed/minimal consumers |
| DX20 | Existing ActorInterface/ExternalActor retained | Open: correlated replies, cancellation and slow/closed customers |
| DX21 | Existing lifecycle and child retirement retained | Open: inferred access with complete shutdown/retirement evidence |
| DX22 | Existing Entity/HTTP owners retained | Open: same Application spine, exact rejection and family/root failures |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inferred counter/recursive/component fixtures; timer readiness uses actual reports and duplicate rejection | Open: retain inferred API; complete downstream corpus and owner gates |
| DX26 | Counter baseline 144 lines; native linking exhausts 4.8/8.3 GB; compact Nix profiles reach acceptance execution with space remaining | Open: canonical caller/compiler/runtime measurements and final resource gates |
| DX27 | Timers 0.2 released/pinned; extracted/MSRV and renamed facade evidence; owner PR #89 passes native ten-gate/927-test and four-workspace package preflight | Open: owner CI/review, release/pin; package final inferred API and execute downstream denials and journeys |
| DX28 | Initial documentation pruning preserved | Open: compiled guide inclusions, reconciliation and final link gates |

### Application result conservation experiment

The first bounded terminal-consumption change implements a derived custody
law: inspecting an application result borrows it; consuming completed work
returns its output **and** the independent cleanup Result; selecting completed
work from another phase returns that entire original outcome. Work completion
does not certify successful cleanup or even successful actor initialization.
This is a substep of stage 1, not a substitute for inferred terminal custody.

Desired caller syntax is `outcome.work_output()`, `outcome.cleanup()`, and
`outcome.try_into_completed()`. The ownership equation is
`Completed(Output, Result<Cleanup, ApplicationCleanupError>) ->
Ok((Output, Result<Cleanup, ApplicationCleanupError>))`; every other phase
maps to `Err(original_outcome)`. No callbacks, disposal policy, Clone, Debug,
Send or actor type annotation is required by these operations.

Pre-edit regression: Bombay `tests/application_outcome.rs` will exercise all
six outcome alternatives with move-only values, work success plus cleanup
failure, and a live counter journey. It must fail for the missing operations
before production edits. Reuse ApplicationOutcome, ApplicationCleanupError,
the existing execute_application_with receiving owner, actor facade, named
sends and lifecycle; delete the counter's duplicated completed-work match.
ActorRetirement, child bindings, projection, Driver and primitive semantics
remain their existing owners; no new actor algebra is needed for this law.

Expected paths: Bombay application/execution.rs, application_outcome test,
counter main, application guide and coordination/status; this owning PRD.
Expected production delta: at most +60 lines; public types +0/-0.
The full retained batch must record actual deltas, tests and drift below.

Pre-edit result: the five result-consumption regressions (plus the included
counter's two pure tests) fail compilation solely with 14 E0599 missing-method
diagnostics. The added live startup-refusal witness retains a non-Send local
work closure and exact rejected actor. No production edit existed at this cut.

Pre-edit drift checkpoint: ApplicationOutcome has six alternatives;
ApplicationWorkCustody has five; ActorRetirement has 24. These remain the
control/custody sums; this experiment adds no phase, result sum, actor dispatch,
public type or module. Every alternative owns the existing cold, staged,
prepared, uninvoked, completed or interrupted custody required for exact
return. Cardinality and child selection are unchanged. Direct borrowing and
Result with the original outcome express the law. There is no new arrival
history, repeated cause, cardinality assumption, transition authority, boolean,
structural path or effect traversal. Cross-checks: PRD affine custody and
complete application result laws; execution-ownership DG-API/DG-WORK and
Driver D-RETIRE-1. Atomic-family normalized laws are unaffected because no
actor input, Actions, creation, wrapper or interpreter changes. Disposition
awaits the post-edit measurements and debug/optimized witnesses.

Dependency verification: both worktrees are clean at initial docs commits
67318ac/d809174 on codex/developer-experience-docs. Bombay locks published
Behavior/Actors 0.23.0; registry vcs metadata identifies d69f992, whose complete
AGENTS.md is byte-identical to Behavior's current instructions. Address 0.3.0,
Communication 0.1.3, private Observe, and Timers 0.1.0 at 13e884d remain the
selected primitives; no patch to Behavior is used. Inspected owner transition,
settlement, creation, wrapper, mailbox admission, address lease, observation,
timer generation, application composition, launch and retirement paths.
Cross-checked execution-ownership DG-API/DG-PROJECTION/DG-WORK and Driver
D-RETIRE-1; no aggregate actor policy is changed. DX1 has no prerequisite
backlog edge. Stage 2 remains dependent on the full stage-1 inference witness.

Fresh environment baseline: pinned Nix shell enters successfully with Rust
1.99.0. `cargo test --locked -p bombay-rs --test application_children` passes
the caller compile-pass and invalid-child denial. It ran with incremental
compilation/debug information disabled in a task-owned temporary target to
contain disk use. This is not the authoritative flake pass. Initial free space
was 1.3 GB; the baseline consumed approximately 753 MB. Do not delete unrelated
caches or treat prior signing/disk failures as waived.

Post-edit checkpoint: outcome/work-custody/retirement alternatives remain
6/5/24; creation bindings remain Established/Rejected/StartupRejected. No
subordinate alternative or actor transition branch changes. The three
inspections contain six match arms, with no dispatch or state mutation.
The affected execution owner is 1887 -> 1944 physical lines, one module -> one;
public declarations 14 -> 17 (only work_output/cleanup/try_into_completed).
All retained values remain in their original phase or are transferred together
from Completed. The same residue scan and laws above pass; no atomic policy or
normalized-family contract changed. Disposition: **pass for this bounded
custody law**, with full programme and inferred topology still open.

Application-result batch delta from the initial documentation commits: Bombay
production (including executable example): +64/-8/net +56; tests +310/-0;
public types +0/-0, public methods +3/-0. Six Bombay paths plus this owning PRD
and its evidence appendix are changed; no source/dependency change in Behavior.
The retained Bombay commit is 6d1d493. Counter main is 144 ->
143 physical lines; its terminal enum, six runner arguments and retirement
assertions remain. This is new consumption capability, not claimed overall
ceremony reduction or performance acceptance.

Exact focused commands run in Bombay's pinned shell, using the task-owned
CARGO_TARGET_DIR and CARGO_INCREMENTAL=0/CARGO_PROFILE_DEV_DEBUG=0/
CARGO_PROFILE_TEST_DEBUG=0 to contain build space:

- `cargo test --locked -p bombay-rs --test application_outcome`: seven pass.
- The same command with `--release`: seven pass after final source changes.
- `cargo test --locked -p bombay-rs --test application_outcome --test completed_application_receiving --test application_terminal_custody --test run_with`: 7 + 38 + 4 + 16 = 65 pass; includes five independent authority denials.
- `cargo clippy --locked -p bombay-rs --test application_outcome -p bombay-example-counter --all-targets -- -D warnings`: passes.
- `cargo fmt --all -- --check` and both working-tree `git diff --check`: pass.

The real inversion rejected Completed when cleanup was Err. Its named
completed_work_preserves_cleanup_failure test compiled and failed at the
law assertion, not for a compiler error. Source-copy mutation left both
worktrees intact. Sharing its temporary Cargo target initially reused a
mutated binary; clearing that task-owned target and rebuilding established the
65 positive passes above. Future inversions must use independent targets.

Fresh Bombay `nix flake check` exited unsuccessfully while disk became full;
a subsequent shell entry explicitly failed to open Nix's SQLite fetch cache.
No gate pass is inferred. With user authorization to free space, unreferenced
Nix store paths and regenerable tool caches were reclaimed, restoring about
7.8 GB free. Neither the other AI's main-2 source nor its target was changed.
The two-job retry also exhausted disk during linking. Reclaiming regenerable
Cargo artifacts from inactive checkouts restored about 23 GB; active checkout
targets and benchmark reports were preserved. The final
`nix flake check --max-jobs 1 -L` passed all 21 aarch64-darwin checks, including
build, tests, mutation evidence, coverage, Loom, Clippy, docs, formatting and
all seven example gates. Other platform checks were not executed.
An earlier disk-starved trybuild batch falsely reported expected denials as
success; the rebuilt six-fixture runner corpus passes after reclamation.

Fresh Behavior `nix flake check --max-jobs 1 -L` passes all ten native checks:
build (including Darwin signing), fmt, audit, deny, TOML formatting, nextest,
docs, doctests, package and Clippy. Nextest reports 920 passed, zero skipped.
Its derivations use pinned Rust 1.95.0; Bombay's focused shell reports 1.99.0.
Neither prior signing failure nor disk failure is waived. Non-native platforms
and optional exhaustive mutation gates remain unexecuted at this cut.

The programme is complete only when ordinary users can follow the canonical
journey across every selected example without assembly plumbing, and all
affected semantic denials and complete ownership traces remain proved. Closing
this PRD does not mean all networking, durability or optional platform work is
implemented; their coverage mapping preserves those separate obligations.

### Inferred custody: reopened representation, active proof

Two persistent caller regressions in Bombay's application_children harness remain
expected-red: `application_inferred_custody.rs` originally fails E0283 at the unconstrained
terminal type; its strengthened role-consumption form also fails E0432/E0282/E0599; `application_recursive_custody.rs` fails E0275 at recursive child
interpretation. The latter now includes recursive retirement, distinct parent
roles, original failed child/work return and real timer/shutdown wrapper orders.
The prior declaration pass and invalid-child denial still pass. The current
worktree has no complete-gate pass; retained-commit checks are distinguished below.

The custody law is derived, not an Agha guarantee: a parent owns each direct
child's complete retirement, and that retirement owns its descendants. Its
ownership equation is `Child(origin, ActorRetirement<Child, ChildDescendants,
ChildFailures>)`. Boxed concrete retirement gives recursive storage a finite
representation without erasure. Fresh allocation, binding before dependent
sends, initialization, explicit provenance, accepted prefixes and cancellation
custody retain their existing laws. Target syntax omits runner arguments and
terminal enums; role inspection must borrow exact reports, and consuming
selection must return every unmatched value.

Before production edits, the registered heterogeneous regression proved the
inference gap. The plain-Rust metadata prototype reused actual Core occurrence
products: a nominal schema supports self-recursive stored reports and Send;
the blanket equivalent fails E0275. A separate consumer rejects a private
projection alias. The actor's own ChildOccurrenceShape removes a redundant
shape marker; no new macro is selected. Reuse ChildOrigin/ChildRole,
ActorRetirement, ordered bindings, receiving loans and native projection tasks.
Removing a task may not erase its distinct failure cause.

DEAD_ENDS disposition: `reopen`. The first child-local candidate compiled and
ran the heterogeneous counter (check 1.38 s; build/run 4.42 s), but recursive
interpretation did not finish after 210 s. A diagnostic-only depth-64 probe
exposes `EstablishChild -> DispatchBirth -> InterpretCreations -> CommitActions
-> EstablishChild`; shallower limits stop at finite Send proofs. Raising the
limit is not a solution. The original implementation with its explicit terminal
sum has the same cycle. All eight candidate production files (+167/-29,
net +138) and three public declarations were removed; no caller was migrated.

Pre/post aggregate checkpoint: retirement retains 24 alternatives and creation
bindings retain three. No control-state, transition alternative, effect lane,
module or lifecycle authority changed. Baseline owner lines were terminal 2596,
children 632, creation 1177, launch 4796, application execution 1944,
composition 457, ActorExt 79 and exports 108. The surviving values remain exact
creation/task custody and ordered receiving destinations. Residue scan found
no new semantic booleans, arrival labels, inferred provenance or erased actor
types. Missing complete recursive interpretation and structural consumption veto
retention; green leaf compilation cannot supply that evidence. Cross-checks are
locked Behavior creation/lifecycle contracts and Bombay Driver, capability and
module ownership documents. This falsifier stays here instead of recreating a
historical ledger.

The next lower-order law is a nominal creation-interpreter proof: the actor's
birth schema may establish a recursive edge without recursively re-proving its
whole interpreter. The creation request/receipt/rejection equation is unchanged.
An isolated ordinary-Rust source copy deletes the single blanket
NestedChildBindings indirection, uses the direct occurrence product, and keeps
one original establishment function. Port lookup uses the existing nominal
Base; the decorated actor remains the owned input. Actual OneShot,
StopOnShutdown and Stash owners preserve Birth/Base, so no second birth-mode
interpreter is needed. Leaf sends and source custody retain their exact existing
requirements, including the old static parent report tag at task handoff.

The child-local combination additionally requires the decoration law explicitly:
a decoration preserving the nominal Base and exact Birth owns the same
descendant/failure schema as that Base. Generic runtime ports must state that
equality rather than accepting a second arbitrary metadata implementation.
This is derived schema preservation, not a new capability or actor-model law.

That source copy passes four debug and optimized journeys: recursive
cancellation retirement, distinct parent/transitive roles, initialization refusal
retaining the original boxed child and boxed work output, and real OneShot/
StopOnShutdown policy in both orders. Timer children actually retire Completed;
elapsed time alone is not counted as firing. Debug build is 4.80 s and optimized
build 13.36 s after formatting; execution is 0.03 s. Restoring the recursive interpreter requirement in another source
copy makes the same tests fail compilation with E0275; targets are independent.
The establishment body is identical after receiver renaming and optional
rustfmt call commas: three matches, zero `if` branches, 356 to 363 physical
lines. It adds no task, actor transition authority or protocol.

An additional plain-Rust interface probe rejects implicit inheritance of method
constraints: an implementation cannot use a trait method’s Clone bound without
restating it (E0599). That mechanism cannot hide the creation requirements.
No production interface is justified by this compiler diagnostic. All successful runtime copies remain outside the feature worktree.

The combined child-local source copy now passes all four journeys without a
terminal enum or run_with arguments (debug build 3.46 s / execution 0.03 s;
optimized build 27.34 s / execution 0.02 s). Restoring the recursive interpreter
bound in an independent copy rejects the same corpus with E0275. Its copy-only
production diff is +620/-418/net +202 over nine substantive paths plus a module
newline; the private 439-line consumer includes the explicit nominal port.
Retirement alternatives remain 24, creation bindings remain three, and the
establishment operation/effect order remains unchanged. None is retained.

A required authoring checkpoint falsifies its metadata interface: the facade
already supports an inferred non-Bombay sender address without a local host.
The original source passes the full initialization/transition regression; the
candidate fails E0271 because its metadata trait requires MailAddr. A concrete
false where-Self bound cannot make this conditional (E0277). Do not branch on
sender syntax, invent a marker or require an empty runtime policy. Bombay test
commit ec79d9f retains this source-preservation witness (+39/-8 test lines,
zero production/public API). Two outer-authoring tests and all four existing
denials pass; a separate optimized executable runs the same fixture successfully
(build 6.37 s, exact locked owner versions match). The release outer harness also
passes, but trybuild's generated fixture build remains dev; it is not counted as
optimized fixture execution.

The next isolated model separates typed retirement data from local realization.
Law (derived): retained user inputs and host rejection/report data use the
behavior's exact associated address; standard Application interpretation still
requires Bombay MailAddr. No execution, endpoint or lifecycle capability is
added to a non-local actor. Desired syntax is the unchanged pure facade and
unannotated local Application. Reuse the existing generic LocalActivationRejection,
User and Termination products; keep local projection/constructor bounds and all
runtime denials. The sender witness must pass together with the four runtime
journeys, and a non-local Application must remain a compile denial. This is a
source-copy hypothesis, not permission to relax interpreter bounds or retain
unmeasured production surface.
The source copy passes the four runtime journeys and executes the original
sender-independent fixture. Non-local Application remains rejected at run_with
(E0599 on its unsatisfied local address bound). Standard creation/projection
bounds remain fixed to MailAddr; only owned protocol data names its associated
address. Public/family preservation and full owner checks are still open.

The next port hypothesis packages the old non-recursive requirements through a
sealed creation scope's associated types. A ChildCreation value owns exactly the
routed request and a temporary mutable loan of its creator's capabilities; its
private construction proves the selected child occurrence. The nominal Base
supplies the recursive interpreter proof, retaining the decorated actor as input.
The loan may establish only that request and cannot issue another lifecycle or
transport capability. Its ownership equation is (creator loan, RoutedCreation)
-> original ItemSettlement; no additional actor, effect or task is introduced.
The desired handwritten proof names the nominal actor and this creation loan,
not ApplicationCapabilities, child-binding paths or source slots. Reuse the
original establishment operation and all old leaf-send/source constraints;
delete the seven-parameter private nominal factory declaration only if the
ordinary-Rust caller and complete journeys prove the new port. This hypothesis
is isolated before any production retention or facade expansion.
The plain-Rust loan port now passes the four journeys in a separate crate using
only public factory/scope/loan names (debug build 3.50 s, execution 0.04 s).
All old scoped send/creation custody premises are reused. The sealed scope
carries the creator's exact event/birth tags and the actor's existing products;
the direct occurrence product replaces a redundant opaque child-bindings label.
Eighteen private-bound/interface warnings expose a publication question that
remains open; no visibility promotion or lint waiver is retained. The next
source-copy facade experiment derives only this already-proven declaration and
returns the loan's original future, avoiding another async capture of unused
actor type parameters.
The first expansion fails the pure sender witness by equating the actual
actor's protocol with its Base protocol. Reopen that constraint: the interpreter
operates on the actual actor and its concrete protocol; the Base selects the
birth schema. Base, exact Birth and custody-schema equality are required;
Base::Protocol equality is not read by the original establishment operation.
Dropping that experiment-only equation leaves the actual scoped actor and
Application fixed to MailAddr, and must preserve the non-local runtime denial.
This is removal of an unproven constraint, not a new protocol conversion.

A fresh enum-root source count corrects the earlier 22 figure: both inspected
c3afb30 and current ec79d9f contain 24 retirement alternatives, including the
native task-failure and legacy Panicked/Cancelled alternatives. No variant was
added in this work. All aggregate pre/post counts use the verified 24. The facade expansion with
only Base/Birth/custody equations executes the original pure sender fixture
(build 34.28 s), with private-interface warnings still open. That cost is
unaccepted until compared on a controlled warm and clean build; no programme
completion is inferred from these probes.
 Pure sender, generic/renamed and complete runtime callers
must all pass; deriving a factory may not impose new authoring bounds.



This remains lower-order evidence, not a selected public representation. The
current external factory/facade proof infers child-local custody without a
terminal enum; own-birth consumption still exposes structural choices. Public
metadata/port privacy, deterministic readiness, standalone supervision/pools
and their independent failure traces must be proved together before migration.
Caller ceremony, aggregate measurements and required denials remain gates.

The next terminal-consumption law is derived affine projection of application
roles: borrowing a direct child report never consumes it; selecting one declared
role transfers its exact origin and complete boxed retirement, or returns the
complete original unmatched sum. Desired syntax is
`report.child_retirement(&Audit)` and
`report.try_into_child_retirement(&Audit)` with inferred position. Role selection
is static, including two children of the same actor type. It grants no lifecycle
or sender authority. Reuse the existing ChildChoice sum, ChildOrigin, boxed
ActorRetirement and role declarations; delete caller Head/Tail navigation.
An external counter regression is written before the source-copy implementation:
actual reply, joined root retirement, three distinct child roles, borrowed and
owned preservation of a move-only audit allocation, and complete unmatched
return before selecting a different role. Production hypothesis is one sealed
extension trait with head/tail implementations in terminal.rs and its exports;
no new state/product, macro, task or interpreter branch. Expected delta is under
100 production lines and one public trait. Retirement alternatives remain 24,
creation bindings three; surviving data are exactly the original origin and
retirement. The direct sum remains the single representation. Own-birth role
projection, full publication, independent inversion and generic families remain
required before retention or migration.

The counter role-consumption regression first fails E0432/E0599 on the missing
selection interface. Its source-copy implementation adds one sealed trait and
head/tail implementations, with no new custody product or control state. The
external caller now executes the real counter reply, joins all three children,
borrows the audit state through existing BehaviorBase, and recovers the exact
original audit allocation. Borrowed origins/retirement pointers agree with
consuming selection; both unmatched counter reports preserve their original
pointers before successful selection by distinct roles. The stronger caller is
preserved in the owning inferred-custody fixture; it remains expected-red in the
feature worktree because the candidate is not retained.

Exact source-copy checks: the external counter corpus passes three tests,
including its two real Counter pure-transition tests; the recursive corpus
passes four. All seven pass debug (build 4.16 s, execution 0.03 s) and optimized
(build 13.21 s, execution 0.02 s). Four separately checked invalid callers fail
as required: wrong role E0277, forged scope E0277, forged creation loan E0451,
and non-local Application E0599. An independent source and target copy changes
only borrowed head selection to report absence: the ownership regression fails
its borrowed/consumed origin and pointer equation, while both Counter pure tests
still pass (build 7.62 s). This kills the selection inversion; it is not a full
owner-gate result. The source-copy outer-authoring suite also passes both tests
and all four existing compile denials (13.00 s) with the retained full sender
fixture. Its first invocation failed only because /tmp and /private/tmp produced
duplicate path-package IDs; the canonical-path rerun passes. Eighteen private
publication warnings remain, so this is not a warning-clean gate.

Current copy checkpoint: control states remain outcome/work/retirement 6/5/24;
creation-binding alternatives remain three and the establishment operation is
unchanged. Selection adds four matches/eight alternatives over the existing
sum; none selects an actor transition. All surviving subordinate values are
exact origin, complete retirement, mutable creation loan or original staged
request. No new semantic booleans, arrival history, duplicate causes, inferred
provenance, false single-child policy or nested actor engine occurs. The copy's physical Rust source diff in substantively changed owners against
retained ec79d9f is +942/-525/net +417
over eleven substantive source paths; that includes source-copy test disabling
and its private test module declaration. Nineteen additional tracked source
paths differ only in diagnostic test configuration. A private probe module and
temporary examples are also outside the worktree. These are not retained-production
measurements. Exported new types/traits number seven, versus zero retained.
Visibility-bearing declarations in those paths are 132 -> 147, including
private-module declarations. Actor creation/lifecycle, Driver, capability and
module laws remain the cross-checks. Disposition: focused role selection passes;
the combined public representation remains unaccepted until publication,
own-birth role access, real families and complete owner gates pass.

The next ordinary-Rust port experiment states nominal creation proof without
creator-context trait parameters. Law (derived): the Base owns its birth
interpreter proof; a valid scoped request supplies the parent/position and the
actual decorated actor. The same Base/Birth/custody equations remain mandatory,
while protocol identity is not inferred from Base. Target advanced component
syntax is a generic parent accepting `B: ActorCreation` and issuing its real
birth; ordinary callers still only construct actor values. A generic Nursery
is intended to accept recursive Node and real Counter as distinct child
substitutions; the first Counter witness exercises the prior two-parameter port. Reuse the same
sealed scope, affine loan and establishment function; move context parameters
to the factory method and delete them from the trait. No new type, interpreter,
phase or effect is proposed. This is a source-copy hypothesis, not a retained
API; the original sender and seven runtime journeys remain required together.
The first lowering incorrectly makes Base/Birth/custody equality part of every
creation scope. Reopen that placement: these are the nominal factory's selected
input equation, not requirements on unrelated scope data. Keep the scope's old
leaf premises and state the exact equations at EstablishChild's factory call.
The compiler's suggested new Base::Birth/Send bounds are not added.
After moving the unchanged input equation to the factory call, the library
compiles, but the generic parent still fails on twenty missing runtime leaf
properties (Send, protocol/source custody and creation interpretation). The
nominal proof alone does not establish that generic child's valid scope. This
hypothesis is reopened without adding those compiler-suggested bounds; both
source-copy production files are restored to the last seven-test representation.
The generic Counter witness and exact failing patch remain outside production;
a second child substitution was not migrated after the first falsifier.
The restored external corpus passes all seven tests again (6.22 s build,
0.03 s execution). The feature branch still has no inferred-custody production
changes; only the stronger expected-red caller fixture is kept there.

Timer qualification: the combined runtime probe observes both children actually
Completed, but it still uses a 20 ms wall-clock delay before root shutdown.
Source confirms relative scheduling and expiry use std::Instant while waiting
uses Tokio. These observed runs do not satisfy deterministic timer evidence;
remove that timing dependency before retaining this corpus. No clock semantics
are changed speculatively from this observation.

The next family witness copies the real supervision example's worker,
activation source, recovery policy and work trace into the external consumer,
changing only assembly/terminal plumbing to inferred Application. It exercises
actual first activation, capability admission, worker stop, successful replacement
activation, shutdown and both retained worker generations. Existing explicit
policy/type declarations stay to isolate runtime composition from constructor
migration. This is a pre-edit compile/transition witness; no new supervision,
observation or lifecycle law is proposed. The runtime must consume the owning
FixedSupervisor -> StableProxy -> Worker birth tree with complete custody.
No family implementation changes exist at this cut.

The supervision caller fails on missing StableProxy custody/factory ports and
the two genuine logical reply protocols (FixedSnapshot and CapabilityResult).
ReplyRoute owns both logical and established alternatives, so its existing
Hosts obligations must remain. Core already projects logical requirements;
that projection preserves occurrences rather than deduplicating namespace owners.
The custody substep implements the existing child-local equation once for
StableProxy in Bombay: its direct child is exactly StopOnShutdown<Worker>.
The factory substep requires the actual proxy's worker creation scope and the
worker Base's nominal proof at that scope, rather than inventing twenty leaf
bounds. These are existing runtime responsibilities, not actor-model laws or
new supervisor policy. Target remains the real constructor and Application;
no additional caller input is selected. Source-copy changes are limited to
terminal and creation owners, under 100 lines, zero new public types. Control
states/retirement/creation bindings and the establishment operation remain
unchanged; cross-check the exact locked StableProxy birth and Driver custody.
Complete hosting, failure/restart evidence and both downstream families remain
required before retention.
The child-scope-only factory is falsified: it does not state the proxy's full
interpretation/source-custody contract. Reuse the exact complete CommitActions
premise already required by the original establishment function at this generic
standalone owner. The documented birth equation is Births<StopOnShutdown<W>>;
this factory does not introduce a self-creation edge. Nominal domain recursion
still uses the proven facade port. Do not add the eighteen individual leaf
bounds suggested by the diagnostic.
The next lowering keeps the creation loan's original future and explicitly
publishes its existing Send promise. The async loan method currently makes the
caller re-prove opaque auto-trait properties after proxy metadata normalization.
Ownership equation and leaf/CommitActions premises are unchanged: return the
original establishment future directly from a function, with the loan lifetime
and Send stated at the owning seam. This removes one forwarding async state
machine; no Worker/Plan bounds or capabilities are added. The failing real
proxy factory is the pre-edit witness, and all prior authoring/denial/ownership
journeys remain required.
That future removes the opaque Send leakage; the remaining obligation is the
existing task-handoff lifetime of the nominal origin tag and its complete
descendant data. StructuralOrigins already requires a static owner when it
constructs ChildOrigin; the original receiving/retirement seam transports
static descendant data. State those same two premises on the proxy factory's
Self and Self::Descendants, rather than adding Worker/Plan lifetime bounds.
No borrowed actor/value is newly admitted or rejected at the local task seam.
The source-copy proxy metadata/factory and direct loan future now compile.
The real supervision caller is rejected only for its existing logical status
and capability reply bindings; no proxy custody/factory errors remain. This
is evidence of a separate hosting/interface decision, not proof of a complete
supervision journey. The user has been asked which static reply interface the
canonical local path should expose; no logical branch or requirement is removed.

The canonical Application owner must also own explicitly supplied logical
bindings: this is a deliberate composition policy already supported by App,
not another actor law. A with_hosts consumer first restores the real supervisor's
three required root/status/capability namespaces (no unused worker namespace)
while preserving inferred terminal custody and the restart/shutdown trace.
This is the advanced explicit-binding proof, not automatic namespace inference
or ordinary-catalogue migration. Reuse Hosts, ActorSpace and the same
execute_application_with kernel; do not forward through App or add a lifecycle
engine. Proposed source-copy change makes the owned Spaces product an inferred
Application parameter and adds one consuming binding method, no public wrapper
type. Default construction retains the standard root namespace. Cold rejection
must return the whole declaration including its exact namespaces and callable.
Expected owners: application/mod.rs and execution.rs, under 50 net production
lines; root/work/retirement alternatives and all control transitions unchanged.
This substep is required by DX01 independently of the selected ordinary reply
interface; it does not waive DX02's inferred topology criterion.
Default namespace allocation must remain deferred: cloning/copying an ordinary
cold declaration must not share a root registration scope. Represent absence of
supplied namespaces as the existing unit product and one supplied namespace
product as a one-element tuple; caller syntax never names either representation.
A private declaration trait has those two real alternatives, allocating the
standard root namespace or transferring the supplied product. No public marker,
wrapper or eager namespace allocation is proposed. The whole cold declaration
owns the original namespace product on rejection, and supplied shared scopes
retain their existing deliberate sharing semantics.
Staging rejection must extend the existing owned root/work/partial product with
its original namespace declaration; it cannot drop configured namespaces while
returning the other inputs. Supplying hosts is available on the empty namespace
declaration, so a second call cannot silently discard the first binding product.
Child declarations remain available before and after supplying hosts.

The explicit-binding witness exposes a publication falsifier: the public
prepared-input product mentions a private associated namespace type. Reopen
that signature, without exporting the declaration trait. The existing runtime
Spaces parameter owns the concrete hosting product in both real substitutions
(fresh root ActorSpace and supplied SupervisorSpaces); bind that same parameter
at the Application execution seam and return it directly, leaving the private
allocation choice private. No new type, capability, default or visibility is
proposed. The prior external supervision caller is the failing publication
regression. Namespace custody also applies when work is absent: a focused cold
staging refusal must return the original namespace scope with root, rejected
child and native cause, just as supplied work returns its namespace declaration.
Write that refusal regression before correcting the absent-work product. The
single execution kernel and 6/5/24 outcome/work/retirement alternatives remain
unchanged; no new transition authority or arrival state is introduced.

The real FIFO worker witness fails before production on missing
RetirementCustody for the existing pool_worker expansion. A second worker macro
or manual application metadata is rejected. The actor-indexed blanket no-birth
probe conflicts with wrapper implementations and is discarded. A plain Rust
Base/birth schema probe compiles: the actual BirthMode selects the retirement
schema, the nominal Base owns recursive declarations, and NoBirths has empty
descendants/failures for every Base. A boxed recursive child report remains
finite without erasure. This derived static law separates declaration ownership
from the actual decorated actor's protocol and complete retirement data.
The source-copy lowering reshapes the existing custody trait into that schema;
reports use Actor::Base and Actor::Birth, while preserving their actual Actor.
Delete all wrapper metadata forwarding and facade-generated leaf metadata.
The existing no-birth establishment path becomes the automatic factory port;
recursive nominal declarations retain their proven explicit factory expansion.
Reuse BehaviorBase, BirthMode, ChildOccurrenceProduct, ActorRetirement and the
original establishment operation. No Core change, second macro, placeholder,
new public type or actor transition is proposed. Two internal associated-product
aliases may name exact actor descendants and child failures; neither is a new
protocol or storage wrapper. Expected owners are terminal, children, creation,
ActorExt and facade expansion, under 200 net lines. Control states stay 6/5/24,
creation bindings stay three; current values remain exact origin, actor, complete
subtree, source custody and staged request. The pre-edit FIFO failure and all
counter/recursive/supervision/refusal journeys must pass together, with denials,
inversions and warning-clean publication before retention. Scope is the candidate
model, not catalogue migration.

The Base/birth metadata probe succeeds, including a concrete boxed recursive
retirement; the first full lowering exposes a separate overlapping factory
implementation for generic StableProxy. The previous local nominal coherence
probe did not cover that foreign generic owner. Do not merge metadata with a
generic factory method: generic worker hosting is a real context-dependent
interpreter obligation. Preserve the two responsibilities. The next factory
signature selects by the same actual BirthMode as the metadata schema, with
CreationScope statically equating that mode to its actual Actor::Birth. NoBirths
and the proxy's Births<StopOnShutdown<W>> are distinct meaningful substitutions;
no extra capability, marker/default, policy or runtime branch is introduced.
The real failing FIFO consumer and the existing proxy factory are the pre-edit
witness. Retain each factory's existing complete CommitActions premise and the
original future. Extend the same schema equation to launch's original actor
bound; its old BehaviorSettlements/Ph/Protocol and task handoff premises remain.
No individual compiler-suggested leaf bound is added. This is another unretained
candidate cut, with publication and all independent journeys still required.

A handwritten Journal component provides a concrete no-birth consumer with
ordinary JournalState as BehaviorBase::Base. Its real child-creation/retirement
journey fails CreateSelectedChild before the edit: the automatic leaf factory
requires its Base to be a Behavior with Base = Self. The original establishment
operation interprets the actual scoped Actor; it reads Base only as the nominal
origin/schema key. Remove those two factory-Base premises, while retaining the
actual Actor's NoBirths/Base equation, complete CreationScope and CommitActions
proof. This is removal of a model-invalid constraint demonstrated by an unrelated
component, not weakening the actual actor or interpreter. Metadata, protocol,
Send, freshness and authority premises remain. The test must recover the exact
owned journal allocation through the existing BehaviorBase borrow after joining.

Current source-copy result: all 15 downstream tests pass in debug (4.61 s
build, 0.04 s execution) and optimized (16.88 s build, 0.03 s execution).
These include four duplicated executions of the two real Counter pure tests,
one pure supervisor initialization test and ten runtime/refusal journeys;
15 is an executed-test count, not 15 independent laws. The Journal component
now recovers its original allocation and a real counter reply without requiring
its Base to implement Behavior. The supervisor retains both worker generations;
the FIFO worker completes and drains using its unchanged owning pool_worker
source. Both explicit-binding proofs retain their genuine logical namespaces:
this does not close automatic topology inference. The five standalone denials
fail at the required laws: wrong role/forged scope E0277, forged loan E0451,
non-local application E0599 and mismatched birth declaration E0271. The source
copy's full outer-authoring suite passes both tests and four original denials
(3.72 s execution). A separate namespace-inversion copy removes only absent-work
namespace custody; the refusal fixture fails E0308 on the missing original scope.

The actual feature worktree preserves the new component caller alongside the
inferred/recursive fixtures. Its pinned application_children run fails exactly
three of five cases (14.25 s): the three desired inferred callers remain red;
the original declared-child pass and invalid-child denial pass. Candidate
production remains outside the feature worktree. This is an explicit incomplete
stage, not a repository-gate pass.

Current physical source measurement against retained ec79d9f: twelve substantive
Rust paths are +1386/-592/net +794; nineteen more differ only in diagnostic test
configuration. Source-copy tests are disabled in some owners, so these are not
retained-production or full-gate measurements. Seven new public types/traits
remain unretained; this cut reshapes two interfaces and adds two crate-private
associated-product aliases, no new public type. Base/birth lowering removes both
wrapper metadata implementations and generated leaf metadata. Against the
immediate six-owner pre-cut snapshot, physical lines/matches/visibility-bearing
declarations are terminal 2767/15/23 -> 2887/15/25; children 644/7/14 -> 650/7/14;
creation 1362/11/6 -> 1485/11/6; ActorExt 96/0/1 -> 78/0/1; facade 548/9/3 ->
577/9/3; launch 4799/53/25 -> 4808/53/25. Counts include formatting and tests.

Aggregate checkpoint: outcome/work/retirement control alternatives remain
6/5/24; creation bindings remain three, ChildChoice remains two and the original
establishment operation and transition branches are unchanged. Every surviving
report owns the original origin, actual actor and complete subtree; every loan
owns its original request and exclusive creator borrow. The new schema stores
no state. No new actor dispatcher, arrival history, duplicated cause, semantic
boolean, inferred birth provenance or single-worker policy is introduced.
Cross-checks remain locked creation/lifecycle, Driver, capability and module
laws, plus the owning normalized supervisor/pool laws. Disposition: focused
Base/birth and namespace laws pass; the combined public representation remains
unaccepted. Nineteen compiler warnings (eighteen private-proof/interface issues
and one unused test-only import), own-birth role access, the timer probe's wall
sleep, full owner checks, generic standalone coverage and comparable costs still
prevent retention. No visibility promotion or lint waiver is made.

The publication checkpoint first targets the actual owner library with
RUSTFLAGS=-D warnings; a consumer cargo check without targets is not evidence.
The same sealed scope currently has a private-module Creator interface plus an
empty CreationScope forwarding interface. There is one responsibility: an
exclusive, valid creator occurrence loan. Consolidate their exact associated
proofs into CreationScope and preserve a private sealing trait for unforgeable
runtime ownership. No bound, value, return type, capability or actor transition
changes; the five denial fixtures and complete downstream corpus are the existing
focused regressions. This is removal of duplicate interface ownership, not
publication of ApplicationCapabilities or child/task machinery. Source-copy
owner is creation.rs, under 20 net lines, zero new public types. Control sums,
subordinate data and transition branches remain unchanged; disposition awaits
warning and complete owner checks.

The single sealed scope passes the same 15 focused tests. Publication keeps
the original private static proofs, matching existing Application runner
interfaces that deliberately use private CommitActions bounds. Ordinary callers
never receive a scope value; ActorCreation receives only a consuming ChildCreation
loan, whose fields are private. Publicizing runtime/task representation would
violate its owner. The next cut documents this intentional sealed proof boundary
and uses narrow expect(private_bounds) on CreationScope and ChildCreation::establish,
plus expect(private_interfaces) on the sealed scope's internal accessor. This
reconciles Rust visibility lints; it neither weakens a Rust bound nor waives an
acceptance gate. It explicitly supersedes the earlier blanket assertion that no
lint expectation could be considered. Expectations are local and become errors
if obsolete. A new pre-edit extract_scope denial must reject access to the
creator borrow (E0616), independently of forged construction. Strict owner and
consumer checks, rustdoc, all five existing denials and debug/release journeys
remain required; no broad allow or visibility promotion is proposed.

Full owner compilation restores 81 original test attributes, including four
compound test conditions missed by the first restoration. Strict library and
rustdoc pass; owner tests expose independent failures rather than a gate pass.
RootProbe is a valid already-owned no-birth task without BehaviorBase; task
projection must not require creation metadata. Preserve its existing
ProjectedTask::project contract and total ProjectTerminal embedding. Scoped child
retirement separately attaches its proven occurrence and complete child-local
subtree. Both transfer the same original task/cancellation authority through one
private consuming map_retirement implementation, with two genuine mappings
(application-selected projection and occurrence-indexed child custody), neither
a no-op. At the current source-copy cut, project is test-only because production
uses child_retirement; this does not preserve the advanced application projection
execution contract. That compatibility/migration obligation remains unresolved. The existing RootProbe projection and native child-projection-panic
regressions are pre-edit failures, alongside the inferred child journeys. No
public type, new state, capability or actor transition is proposed; owners are
launch and creation, under 70 lines. Do not restore global recursive custody or
weaken actual child/interpreter premises to make the advanced projection pass.

Restored owner tests also falsify unconditional nominal factory derivation:
a pure ProxyParent declaration compiles under the original facade but its
candidate factory tries to prove grandchild reporting for an arbitrary decorated
Actor::Event. Base/Birth equality does not prove event ingress. Do not equate the
actual event with the Base event or invent a report-dropping callback. The required
law is already the runtime's: every direct child's typed send lanes must be
interpretable with the actual parent event and existing namespaces. State this
non-recursive send proof at the creation scope, separately from recursive actor
creation. Reuse the existing tuple declaration, InterpretSends and actual
LocalParentReports; do not require full recursive CommitActions at the scope.
The existing pure ProxyParent owner regression is pre-edit evidence, and the
real supervisor/pool plus both wrapper orders remain the runtime witnesses.
One private ChildEffects proof folds the closed direct-child tuple (empty and
member cases); no runtime value, marker, callback, event, public type or actor
transition is proposed. Expected owner is creation.rs, under 70 lines. The model
reopens if proving it needs a new capability or per-template premise rather than
the original child-send interpretation law.

The ChildEffects empty proof trait is falsified: its tuple implementation's
where clauses cannot be inferred in reverse from a generic product predicate,
so the two ProxyParent event-ingress errors remain. It expresses no usable
source capability at the generic method. Reopen and remove that source-copy
trait and all its bounds, restoring the pre-experiment creation owner. Do not
add associated labels or arbitrary ingress bounds to rescue it. The actual
parent-source law remains an open factory/public-authoring gap.

The user explicitly authorized the expanded selected-programme surface on
2026-10-09. The cumulative task currently touches 22 repository paths, including
five Timers release paths and four uncommitted Bombay regression paths.
Retained Rust production growth remains +56; new public types remain zero and
new public methods remain three. Work stays on feature branches and Bombay's
isolated docs-devx worktree. Authorization does not waive model, provenance,
composition or aggregate-drift gates.

### Timers delivery prerequisite: resolved

The pre-edit extracted Bombay package failed E0432/E0599: registry Timers 0.1.0
lacks ScheduleError and returns Token instead of Result. The workspace-only
immutable patch selected checked owner 13e884d, while registry 0.1.0 identified
owner 4e515ed. Engine 0.2.2 and macros 0.1.2 identify 5ce81b6 and match their
workspace source byte-for-byte. Packaging without verification did not pass
release acceptance. Preserve the checked scheduler; do not hide exhaustion.

This change is delivery policy, with no Rust semantic/API change. Before edits,
the extracted failure established the prerequisite and the owning release was
prepared in a separate worktree. Timers commit 3f85315 changes five manifest,
lock, changelog, guide and Nix-version paths (+18/-8 metadata lines), zero Rust
production/test lines and zero public declarations. Clean pinned
`cargo package --locked -p bombay-timers` verifies its 11-file archive
(55.2 KiB, 16.5 KiB compressed). All eight native 1.96.0 Nix checks pass:
build, fmt, Clippy, nextest, doctest, doc, audit and deny. Nextest reports eight
passed, zero skipped; optimized tests pass six unit and two semantic cases.

[Release PR #8](https://github.com/devrandom-labs/bombay-timers/pull/8) passes
both required CI runs, with a signed head and zero required approving reviews,
and merges as 9369d1f. The release workflow passes and registry 0.2.0 is
published with MSRV 1.96. Its downloaded source is identical to the selected
checked scheduler. Superseded 0.1.1 bot PR #7 is closed with its branch preserved.

Bombay dependency commit 9ce1afd selects registry 0.2.0 and removes the workspace
patch. Five manifest/lock and existing contract records change (+17/-11 metadata
lines); Rust code, tests and public API do not change in this delivery batch.
Historical revision records stay dated. Exact verification:

- `cargo package --locked --allow-dirty -p bombay-rs` verifies the 211-file archive.
- Unpatched extracted `cargo check --locked` passes (6.30 s); `--all-features`
  passes (6.48 s). The same all-feature archive checks with pinned 1.96.0
  (6.62 s), matching Bombay's declared MSRV.
- A separate renamed, facade-only consumer runs the existing generic/hygiene/
  typed-products regression against the extracted archive: one passed,
  build 10.47 s. The workspace renamed library alone is empty and is not counted.
- The 65 existing custody/runner tests pass with registry Timers 0.2.0.
- `nix flake check git+file:///Users/joel/orca/workspaces/bombay/docs-devx?rev=9ce1afd947a3b82f8019ade9c7a8dc8b672145df --max-jobs 1 -L`
  passes all 21 native gates. Repeating the same command at
  `ec79d9f8a85813053a3fadd173e7de67354a56e0` also passes all 21. These
  immutable-commit runs exclude the current expected-red fixtures and all
  source-copy candidates; they certify only the retained commits.

Disk and Darwin signing checks succeed without waivers. Other Bombay platforms
and optional exhaustive gates were not executed in that run. Final inferred-API
packages, downstream denials and complete journeys remain open under DX27.

The fixed-sleep timer witness is not accepted evidence. Its replacement uses
the existing OneShot generation law and typed ReportToParent lane in both
shutdown/timer orders. This is a derived composition proof, not a clock change.
The root retains the set of outstanding wrapper orders; only two distinct
elapsed reports complete the reunion. Duplicate reports cannot discharge the
other order. The reactions continue their children after reporting, so parent
retirement must return both exact OwnerCancelled actors, report settlements,
and payload allocations. Work awaits actual root termination; a timeout is
only a failure guard. No sleep establishes readiness or completion. The probe
uses Application::child with nominal roles, automatic NoBirths creation and
existing generation-tagged TimerElapsed inputs. Production representation and
control-state cardinality remain unchanged. This replaces the unaccepted timing
claim; it does not close the recursive factory/event-ingress gap.

The downstream timed-custody probe fails E0277 at
`Actions::cont().with_send(ReportToParent::new(worker.base().order))` in the
StopOnShutdown/OneShot order. The locked owner delegates SendInput only to a
SendLayer's owned lane, although both lanes are typed send capabilities.
This proves an owner-level DX08 gap independent of the creation candidate.
The derived law is: appending one request to a uniquely typed composed lane
transfers it exactly once, preserving all other sends, births and the verdict.
If both lanes accept that input, inference must remain ambiguous; no runtime
selection is permitted. Caller syntax stays `actions.with_send(request)`.

The smallest owner hypothesis reuses SendInput, Own, Inside, SendLayer and
ordinary products: `(Own, Path)` proves a wrapper-owned lane, `Inside<Path>`
proves its inner lane. These disjoint proof shapes replace the existing
unqualified owned-only delegation; no new type, trait, event or bound is added.
InterpretSends, emitter paths, settlement ordering and custody are unchanged.
Expected owner edits: sending.rs delegation/rustdoc and focused send-lane tests
(+approximately 20 production lines); smallest downstream witness is the two
real timer orders. No catalogue migration precedes proof. The pre-edit core
regression checks owned and inner append, original Box allocation, two layers,
and unchanged birth/verdict; repeated-capability inference retains a denial.

Pre-edit aggregate checkpoint: this is an effect product, not an aggregate
engine. Actor control-state sums, subordinate custody sums, transition branches
and lifecycle authority are unchanged. SendLayer is still a two-field product;
its two current values are the complete owned and inner request sequences.
The current owner has one SendInput delegation with one emit call; the proposal
has two disjoint delegations with one emit call each. Production modules and
public spellings remain unchanged. Cross-check: Actions and SendsFor composition
laws, Driver action order/custody, OneShot generation and initialization policy,
and both shutdown wrapper policies. Post-edit measurement/inversion remains
required before retention. No atomic-family state model is changed.

Post-edit send-routing checkpoint: one owned-only delegation becomes two
disjoint static delegations; SendLayer remains the same two-field product.
Actor control sums, subordinate custody alternatives, transition branches and
lifecycle authority are unchanged. Both sequences retain their complete current
requests. No arrival history, repeated cause, semantic boolean, extra actor
engine, erased protocol, or structural caller syntax is added. The exact new
implementation is justified by the pre-edit typed consumer and core failures;
its only new premise is the existing inner SendInput capability. Cross-checks
above remain unchanged. Disposition: **pass for this bounded owner law**.

Retained batch measurement: production +35/-2/net +33 (24 net lines are rustdoc);
tests +231/-1/net +230; migration guide +9/-0. Zero new public types or methods,
one changed trait-implementation proof shape, four code/guide paths. The complete
programme surface remains authorized and larger; this measurement isolates the
bounded batch rather than resetting the cumulative budget.

Exact verification, pinned Nix: pre-edit core and downstream fail for missing
inner routing; four core regressions pass debug/optimized, two Actors regressions
pass debug, and two retained-Bombay facade/ActorExt tests pass debug/optimized.
The real Application timer-acknowledgement and duplicate-reunion tests pass
debug/optimized (no fixed sleep). Inferred noncapturing reactions also pass both
builds (2.03 s debug / 5.15 s optimized build, execution under 0.01 s):
`.with_one_shot(id, delay, |actor| Actions::cont().with_send(report))`.
These replace both hand-written reaction functions and their return types in
the source-copy witness; no application helper or structural send path is used. A separate no-op-inner mutation compiles and
kills all four core regressions at runtime. Independent binaries reject repeated
lanes with E0283 and missing lanes with E0277. Source-copy workspace tests plus
doctests report 1034 passed, zero failed/ignored before the two Actors tests were
added; strict workspace/all-target Clippy passes at that cut. Actual feature-branch
`nix flake check --max-jobs 1 -L` then passes all ten native checks with the full
new test corpus, including Darwin signing, nextest (926 passed, zero skipped), strict Clippy,
doctests, docs, formatting, audit/deny and packaging. Other platforms are unexecuted.

No published dependency is silently substituted: downstream proofs use explicit
local patches to the selected 0.23 owner plus this change. Bombay's retained
manifest still selects published 0.23, so delivery/adoption of the owner change
is open. This does not close inferred creation, topology, terminal projection,
or whole-programme acceptance. Temporary source/log evidence lives under
/tmp/bombay-devx-send-lanes-* and /tmp/bombay-devx-timed-custody-*; durable owner
regressions are send_lanes.rs and wrapped_send_requests.rs.

The current owner compilation remains red after selecting committed Behavior
e09ccfa explicitly in the source copy. Four internal projection records still
use a global recursive terminal parameter for NoBirths actors. Their law is
unchanged: projection transfers the complete exact child retirement; projection
panic remains an independently reported native failure; shutdown and other
capability lanes retain all custody. A NoBirths child's descendants are Never,
not the parent's selected projection type. Migrate only these focused law tests
to the proven ChildRetirementAt occurrence port and child-local Never custody,
without changing their transition, callback, assertion, actor-task or failure
trace. Existing RootProbe ProjectTerminal coverage stays. This is test lowering,
not a new API or broad catalogue migration; expected production delta is zero.
Control-state sums, subordinate alternatives, branch counts and public spellings
are unchanged; four projection records keep the exact same owned actor, origin,
settlements and native causes. The pre-edit regressions are the restored
projection-panic, two shutdown targets, capability reunion and cancellation
corpus. The factory event-ingress falsifier remains separate and must stay red.

The focused projection migrations reduce owner compilation from fifteen errors
to two, both at the pure ProxyParent facade declaration. Preserve that exact
declaration in an independent downstream pure_parent compile/initialization
fixture; it must pass against retained Bombay and remains red against the
candidate. For the unrelated interpreter-only ProxyOperation proof, express
its existing pure parent through the exact owning Behavior macro and an
uninhabited receive, with identical protocol/event/birth/settlement contracts.
This relocates authoring coverage to its consumer seam; it does not clear or
retain the broken factory, and the complete candidate gate must still include
the expected-pass failing authoring fixture. It permits executing every restored
projection, shutdown and custody assertion without test cfg suppression.

The isolated pure_parent fixture passes retained Bombay and fails the candidate
with two E0277 report-ingress errors. All 294 restored owner tests now execute
and pass, zero ignored, after the focused projection migration and relocating
that authoring case; this is not a complete gate because pure_parent remains
red and the older fixed-sleep probe remains unaccepted.

The next bounded hypothesis makes factory availability conditional on the
existing complete interpretation premise. Pure actor definition must not prove
that a local runtime can execute every declared effect. A sealed consuming
ExecutableCreation port moves the existing ChildCreation::establish promise
from an inherent private-bounded method to an explicit conditional interface.
The facade derives ActorCreation only when its exact loan implements that port;
ordinary syntax, actor effects and interpreter code stay unchanged. This is a
derived authoring/interpretation separation, with no new effect or runtime
state. The port replaces the existing method rather than adding another
execution path. Expected edits: creation, facade expansion and re-exports; one
public trait, under 50 net lines. Reuse the exact full CommitActions premise.
The pre-edit focused regressions are pure_parent (red) plus recursive and
heterogeneous journeys (green); both must pass together. In particular, a
recursive trait cycle falsifies this hypothesis rather than justifying extra
bounds, flags, erasure, or specialization.
Pre-edit control sums, custody alternatives and interpreter branches are
unchanged; no new stored value is proposed. The only surviving loan retains
the original actor request and exclusive creator borrow. Scope sealing and
all forged-creation/scope/borrow denials remain required. Disposition awaits
those witnesses, measurements and both complete code/consumer checks.

Conditional-execution disposition: **reopen**. Pure parent passes, while all
four recursive journeys fail E0275 at the complete interpretation/creation
constraint cycle. Experiment delta across four files: +32/-27/net 5, one
additional public trait; actor control states, custody alternatives, stored
values, transition branches and production modules remain unchanged. The added
trait only re-expresses the same execution prerequisite and cannot break its
recursive proof graph. That competing interface and its predicates are removed
entirely; the pre-experiment source is restored. The falsifier is in the existing
DEAD_ENDS.md. Both pure authoring and recursive runtime remain acceptance
requirements. Focused test projection migrations are independent and remain in
the source copy; no candidate production is retained in Bombay's feature tree.

The restored cut passes 294 owner tests in debug (5.39 s execution) and
optimized (5.34 s), zero failures/ignored, plus the selected 17 downstream
tests. This includes the independent projection-panic, source cancellation,
activation/preparation reunion, exact child custody and both shutdown-target
traces. The fixed-sleep recursive timer probe still has no acceptance credit.
The complete facade corpus remains red for the intended pair of E0277
ProxyOutcome/ProxyDiagnostic ingress failures after canonical /private/tmp
paths resolve the initial Cargo lock collision. No pass is inferred from that
collision. Pure definition is added to the existing actor_facade_full fixture
in the actual feature worktree: retained Bombay's outer_authoring tests pass
(two tests and the four original compile denials, 3.63 s). Test-only delta is
+19/-0, production and public API zero. Candidate production stays unretained.

Projection migration checkpoint: actor control-state sums, subordinate failure
alternatives, transition branches, production lines, modules and public
spellings are unchanged. The three single-child projection records still own
exact origin plus complete child retirement; the two-alternative shutdown sum
still distinguishes its two actual event targets. Their NoBirths subtrees now
use Never, deleting a false recursive terminal equation without discarding an
inhabited value. Existing callback bodies, panic payloads, received action/source
custody and allocation assertions are preserved. Residue scan finds no new
arrival history, repeated cause, false cardinality, nested authority, semantic
boolean or ordinary structural caller syntax. Cross-check: scoped retirement
law, Driver custody, projection task failure, and existing normalized atomic
lifecycle contracts. Disposition: **pass for focused test lowering**, with
authoring/factory representation **reopen** and full gates incomplete.

Born-child access substep: the existing Core-generated ChildRole<Parent> owns
the exact child and Position. Scoped origin already preserves that position.
The derived custody law is the same as declared Application roles: borrowed
selection retains ownership; consuming selection returns the exact unmatched
sum or transfers the original box and subtree. Desired syntax is
`report.child_retirement(&ParentChild::Branch)` and
`report.try_into_child_retirement(&NodeChild::Next)`. No actor identity or
authority is inferred from the role.
The smallest hypothesis adds one ChildRetirementSelection head implementation
using existing DeclaredChildOccurrence as its disjoint proof and reuses the
existing tail implementation. ChildRole's child/position equality supplies the
relationship; no new marker, trait, data, bound on actors or runtime mapping is
proposed. Expected owner: terminal.rs, under 40 lines, zero new public types.
Pre-edit focused recursive/parent consumer must fail with missing selection.
Control sums, retirement alternatives (24), custody phases and interpreter
branches remain unchanged; two selection head laws exist for two real origin
forms (Application role and born position). All scoped reports retain the same
exact origin, original box and complete descendant product. Recheck unmatched
ownership, static wrong-role denial and recursion before retention.

The born-role Clippy run exposes six existing candidate publication defects:
three complex exact settlement/creator signatures, the moved creation kernel's
original too_many_lines annotation, and two missing rustdoc conventions.
Restore the kernel's pre-existing narrowly justified annotation and document
its complete ownership equations with local expect(type_complexity) on the
three signatures. No alias, type, bound or visibility change is invented from
the lint. Fix rustdoc backticks and the exact unchanged Err contract. This
publication cut changes annotations/docs only, and does not waive model,
semantic or full gate acceptance.

The born-role selection remains an isolated candidate: a 27-line selection
addition, zero new public types or interpreter branches. Both focused
borrow/consume tests pass in debug and optimized builds; replacing the borrowed
head selection with absence makes both fail. The foreign-parent role denial
remains E0277. Existing retirement/control sums and owned data are unchanged.
Aggregate checkpoint: NodeStartup stays Grow/Refuse (2/2); parent and sibling
child custody stays Some/None (2/2); retirement alternatives stay 24/24.
ChildChoice, Option and Result keep their existing two alternatives. Selection
adds two methods implementing existing spellings, two matches/four arms, and
27 production lines in one existing module; exports and public spellings add
zero. Every surviving value owns its original origin, box and subtree; no
arrival history, repeated cause, false cardinality, nested transition authority,
semantic boolean or structural caller requirement is added. Cross-check the
scoped-retirement law, original Driver custody and nominal child-role laws.
Disposition: focused selection pass; combined factory model remains reopen.

Canonical caller coverage now registers five pass fixtures and the original
invalid-child denial. Recursive callers contain neither a terminal enum nor
runner type arguments. The fixed-delay timer journey was removed from that
fixture; its separate replacement waits for both actual elapsed reports and
checks replay rejection and both original cancelled payload allocations. The
pinned candidate application_children run passes all six cases (3.12 s cached
execution). Only the candidate diagnostic snapshot was changed, to include the
owned Namespaces parameter; both original missing-Behavior errors remain.

All seven current consumer denials remain rejected: wrong role E0277, forged
scope E0277, forged creation E0451, nonlocal application E0599, wrong birth
E0271/E0277, extracted creator E0616 and foreign born role E0277. The separate
pure_parent fixture still fails with exactly two E0277 report-ingress errors.
Strict candidate library publication and Clippy pass. These checks do not
certify the feature worktree or close the factory gap.

The retained-library caller run passes its original fixture and rejects the
new counter's missing custody API. Its recursive compile was interrupted after
over three minutes of compilation (99.7% CPU, approximately 3.6 GB resident);
the run ended 130 and is not counted as a gate pass. Source and diagnostic logs
remain; only terminal task build caches were removed. The four new acceptance
fixtures and registration are retained in Bombay cff086b: 986 test lines, zero
production lines/types. The branch is intentionally red pending implementation.
Evidence: /private/tmp/bombay-devx-event-ready-caller-fixtures.log,
/private/tmp/bombay-devx-retained-caller-regressions.log,
/private/tmp/bombay-devx-current-*-denial.log and
/private/tmp/bombay-devx-current-pure-parent.log.

Borrowed retirement state substep (DX04): this is a derived Rust ownership
law, not a new actor lifecycle policy. `retirement.behavior()` borrows the exact
retained actor in all 21 state-owning alternatives, including rejection and
failed execution. It returns None only for ActorTaskFailed, Panicked and
Cancelled, which already contain no behavior. Some does not certify success.
No cause, effect, command, descendant, initialization or receiving slot is
consumed, reconstructed or classified by this inspection. The complete original
retirement remains available for the caller's subsequent decision.

Before production, extend the real counter completion and startup-refusal
regressions with borrowed inspection followed by original owned consumption.
Add allocation-refusal and unavailable-state regressions retaining the original
boxed audit and native JoinError. The pre-edit witness must fail E0599 for the
missing method. Reuse ActorRetirement's existing exhaustive sum and field; no
new type, generic, bound, interpreter, dependency or semantic state is needed.
Expected files: terminal.rs, application_outcome.rs, existing migration
guide and the focused counter example. That example inspects retained state
before consuming the original terminal projection. Expected production
increase including the example: under 40 lines; one public method, zero
public types. Stage-1 scope only; the recursive factory gap remains separate.

Pre-edit aggregate checkpoint: 24 retirement alternatives, 21 with state and
three without; actor and application control sums unchanged. Surviving state
is the exact original actor needed for later inspection/owned return. Option
expresses exactly one borrowed actor or absence. The implementation will have
one exhaustive selection with two bodies; no transition authority or state
mutation. Existing Driver custody and application-result conservation laws
remain authoritative. Retention requires debug/optimized downstream tests,
independent absence inversion, strict library lint and exact measurements.

Borrowed-state retention result: the pre-edit application_outcome compile
fails with six E0599 errors for the missing inspection. After implementation,
all nine focused cases pass in debug (1.54 s build) and optimized (10.69 s
build), each 0.00 s reported execution. Replacing Some(behavior) with None
compiles and fails the allocation rejection, real counter completion and real
startup rejection witnesses; six other cases pass. The exact native task error
and both unavailable variants remain owned after inspection.

Pinned-shell preservation command: `cargo test --locked -p bombay-rs --lib
--test application_outcome --test completed_application_receiving
--test application_terminal_custody --test run_with`: 290 + 9 + 38 + 4 + 16
= 357 passed, zero failed/ignored. Optimized application_outcome: 9 passed.
Strict Clippy on the library/changed integration target and all counter targets,
RUSTDOCFLAGS='-D warnings' library docs, whole-workspace formatting and the live
bombay-example-counter run pass. Commands use Bombay's pinned Nix shell,
CARGO_INCREMENTAL=0, debug information disabled and task-owned target
/private/tmp/bombay-devx-retirement-state-target. Source, test and failure logs
live at /private/tmp/bombay-devx-retirement-state-*.log. The corrected example
package name is bombay-example-counter; the initial wrong-name invocation is
not counted as execution evidence.

Post-edit checkpoint: retirement alternatives 24/24; state-owning alternatives
21/21 and state-unavailable alternatives 3/3; application/work phases 6/6 and
5/5. No subordinate owned state, cause, effect, wrapper, interpreter, protocol
or transition changes. One exhaustive borrowed selection has two bodies and
24 patterns; every state reference points into the same original retirement.
No history, duplicated cause, false cardinality, nested authority, semantic
boolean or new structural caller syntax survives. Existing Driver custody and
application-result conservation laws still apply. Disposition: pass for this
inspection, with the common custody/product audit and factory model still open.

Retained Bombay commit: a9b88dc. Batch delta: production including the counter
+38/-0/net +38; tests
+74/-1/net +73; one existing production module, zero new public types, one
public method. Terminal owner: 2596 -> 2630 lines; application_outcome:
310 -> 383. Four Bombay paths change (source, test, guide, example), plus this
PRD. Cumulative retained/worktree delta from initial documentation commits:
Bombay production +102/-8/net +94, tests +1427/-8/net +1419; Behavior production
+36/-3/net +33, tests +230/-0/net +230. Retained public types add zero; Bombay
adds four public methods in total. The source-copy inference model is excluded
from those feature-tree totals and remains unaccepted.

This is not a new complete-flake result. The feature branch still contains
expected-red inferred-custody acceptance tests, and the candidate still fails
pure parent authoring. No signing, disk, MSRV, package or final programme gate
is waived by these focused passes. Idle task-owned candidate build caches were
removed to recover space; candidate sources, evidence and other AI work remain.

Finite child-permission hypothesis: factory availability must depend on the
actual direct child's send permissions, not on recursively proving the entire
created actor's execution. This is derived separation of pure authoring from
interpretation. The existing failing pure_parent declaration and the green
recursive/refusal journeys remain the pre-edit regressions; both must pass.
The ordinary-Rust probe rejects two overlapping context equalities (E0284),
but one equality inherited from the existing sealed trait preserves the exact
computed child context and its send bound. No public runtime type or arbitrary
extra context parameter is justified by that result.

The next source-copy substep adds ActorCapabilities to the existing private
sealing trait, bound to the exact existing CreatedActorCapabilities product.
Its only concrete implementation remains ApplicationCapabilities; obtaining
that associated value still requires the existing scoped creation contract.
Then the facade can require each authored child position to have its existing
CreationScope in that exact context, rather than gating on full CommitActions.
Caller syntax, original loan/request, creation ordering, interpreter kernel and
retirement data remain unchanged. No stored value, public type, runtime branch
or state alternative is added; expected owners are creation and facade, under
80 lines. Both birth declaration forms, generic/private/renamed authoring,
scope/creation denials, wrappers and real families remain gates. A cycle or
missing readiness at another child invalidates the hypothesis.

Pre-edit checkpoint: retirement/control/binding alternatives unchanged; no
subordinate state is proposed. The sole associated context is the existing
child runtime product: actual actor, resources, typed parent-report port, child
bindings and origin. Its value still belongs to the creation interpreter.
Cross-check existing creation freshness, explicit provenance, Driver custody
and source-correct facade laws. Disposition awaits the complete witnesses.

Declared-context disposition: reopen before facade lowering. The single
association works in the ordinary-Rust send probe, but putting the existing
CreatedActorCapabilities product in CreationScope's supertrait produces six
E0277 errors at ActorCreation's generic Birth equality. It prematurely
normalizes the closed child product and requests Behavior for Birth::Child;
that is false for legal heterogeneous sums. No suggested Behavior bound,
extra generic, capability widening or facade migration is accepted. The
source-copy sealing change is removed and both owner files restored.
Actor/control/custody sums, runtime branches and public spellings remain
unchanged. The trial changed one private sealing association, stored no new
value and touched no interpreter. The assumption that a scalar context
association preserves the richer birth/product equation was falsified.
Logs: /private/tmp/bombay-devx-declared-context-library.log and the three
ordinary-Rust context probes.

Native verification resource policy: committed Bombay a9b88dc's full flake
check fails during test linking with rustc-LLVM No space left on device,
starting first with 4.8 GB and again with 8.3 GB after completed task cache
cleanup. Both use the exact pinned toolchain; the retry also limits build cores
to one. Neither is a gate pass. Shared APFS space is confirmed for /nix and
the workspace; this is not a separate volume-quota assumption.

Nix checks produce verification artifacts rather than debugger sessions.
Disable development/test debug-symbol emission and incremental compilation
in the existing common check arguments. This is deliberate build policy;
debug assertions, optimization levels, target selection, test execution,
static denials and native failure payloads remain unchanged. Developer shells
keep normal Cargo debugger settings. No production Rust, type, dependency or
runtime contract changes. The two failed full checks are the pre-edit resource
witnesses. Expected owner: flake.nix, under ten lines; evaluate and run the
unchanged native gates. No source/check skip or failed gate is authorized.

Creation ownership hypothesis: the actual actor's BirthMode chooses leaf or
nominal launch; the nominal base owns the recursive future. ActorCreation's
separate generic Birth key currently repeats that scope-owned equation and
forces premature product normalization. Move the dispatch to the existing
NoBirths/Births/RetirementBirths algebra, remove the public factory's redundant
key parameter, and preserve each nominal implementation's exact actual
Actor/Base/Birth constraints. This changes no actor effect or accepted request.
Leaf launch calls the original kernel; nonempty launch calls the same nominal
factory. The interpreter selects the mode from Scope::Actor::Birth, never from
a value, flag, counter, address or caller hint. Private mode dispatch is not an
application capability. No public type, policy, callback or stored value is
added. Reuse the automatic leaf implementation, proxy factory and existing
complete custody kernel; delete the competing blanket factory on every Base.

Pre-edit regression: the declared-context trial's six generic-key E0277 errors,
pure_parent and all existing recursive/refusal journeys. Retain the single
context association only if the full factory contract accepts heterogeneous
products without new Behavior bounds. Expected owners: creation and facade,
under 100 net lines, one private dispatch trait, one fewer public generic.
Control/custody/binding sums and interpreter effects remain unchanged; the
three private dispatch cases are exactly the existing birth algebra and own
no state. Wrong actor/base/birth, forged scopes/loans and nonlocal execution
remain denied. Any recursive readiness cycle reopens this hypothesis.

Birth-launch disposition: reopen. Removing the redundant key lets the source
copy library compile, using the scope's declared retirement product for leaf
launch. Adding the direct-child scopes then rejects both required caller
classes: pure_parent still fails report ingress, and recursive declarations
fail E0271 for conflicting computed actor-context associations. No extra
associated context, parameter, projection, bound or structural alias repairs
those errors. Both production owners are restored; rejected source/logs stay
in /private/tmp/bombay-devx-birth-launch-rejected and
/private/tmp/bombay-devx-birth-launch-*.log. Trial delta: +76/-21/net
55 in two existing modules; one private dispatch trait/three cases,
one fewer public factory generic, zero stored values/public types. Actor,
application, binding and retirement sums and actual interpreter branches were
unchanged. The competing dispatch and context concepts are removed together.
The scalar association does not preserve distinct creator levels under the
real birth equation; both pure authoring and recursive custody stay required.

Compact-check evidence: the same full native check with development/test
debug symbols and incremental artifacts disabled completes dependency/build
phases and reaches the expected-red inferred-custody fixtures with over 5 GB
remaining. The recursive fixture then compiles for over nine minutes at
100% CPU, approximately 3.9 GB resident, without completing. The Nix client
was interrupted (exit 1); that is an incomplete gate, not a pass. The first
inferred counter fixture still fails on its missing custody API. No execution
or static denial is disabled. The shell probe confirms dev_debug, test_debug
and incremental remain unset in nix develop, preserving local debugger defaults.
Five unchanged native publication gates (build, fmt, Clippy, doc, doctest)
now pass together (native Nix build exits 0). Workspace release compilation
takes 11m 38s; Clippy 12.95s and documentation 7.12s. This certifies those
five gates with the compact artifact profile; it does not certify the
incomplete full flake check or the inferred API. Build policy is committed
in Bombay ec362f1.
Logs: /private/tmp/bombay-devx-current-native-gates*.log,
/private/tmp/bombay-devx-compact-native-gates.log,
/private/tmp/bombay-devx-compact-publication-gates.log and
/private/tmp/bombay-devx-check-profile-shell.log.

Build-policy delta: flake.nix +4/-0 and README +4/-0; zero production Rust,
test selection, public API, dependency or semantic state change. The native
resource falsifier is removed, while the missing inferred API and recursive
compile-cost failures remain open. The full programme is not verified.

Complete creator relationship hypothesis (derived, source-copy only): a direct
child scope is owned by the actual decorated actor that requested it, uses that
actor's event/birth protocol and the same allocation spaces, and selects the
exact declared child. These relationships are simultaneous; a child-only
association does not state the creator equation. The existing scope's Parent,
ParentEvent, ParentBirth, Spaces and Actor associated types own them. The
factory remains a nominal future cut; availability requires local child
interpretation, rather than recursive full CommitActions availability.

Caller syntax remains the existing actor declaration and inferred Application;
no parent label, structural alias or policy is added. Ownership equation:
creator of child = actual scoped actor; child namespace spaces = creator
spaces; parent reports enter that actual actor's Event under its actual Birth.
Re-use the existing sealed context, birth dispatch and complete creation kernel;
delete the separate redundant Birth factory key only if the required paired
witnesses pass. First regressions are the already failing pure_parent and
recursive tests (including full startup rejection and retirement). Run them
before retaining any production or migrating a second caller. Both must pass
together; suggested unrelated bounds remain forbidden.

Expected source-copy delta: two owners, under 100 net lines, no new public type,
one fewer public factory generic. Add the complete creator equation to the
existing direct-child scope guard. Actor control, binding (3), retirement (24),
subordinate custody sums, runtime branches and stored values remain unchanged.
Every surviving scope owns the exclusive creator borrow; every retirement
retains its original origin, actor and subtree. Check provenance against the
fresh-creation/binding law, Driver custody and both creation/report interpreter
paths. The prior scalar association and child-only guard are falsifiers, not
accepted evidence for this complete equation.

Complete-creator result: reopen. The paired consumer command exits 101:
recursive context E0271 errors are gone, but pure_parent still emits seven
E0277 report-ingress/EstablishChild errors. A definition-only caller cannot
be made conditional by the child-scope equality alone. No runtime pass is
claimed from the concurrently compiled recursive target. The two-owner trial
is not retained; the same control/binding/custody state sums and complete
kernel remain. Log: /private/tmp/bombay-devx-complete-creator-consumers.log.

Local-source availability probe: before another factory lowering, test ordinary
Rust's associated-type bound elaboration with a pure parent and a reporting
child. The law is conditional local execution, never mandatory executable
capability for declaration. An execution guard must explicitly carry the
existing InterpretSends and SourceSettlementCustody proofs for the actual
child's Sends on its created context; it cannot rely on recovering a where
clause from an empty tag or a concrete Actor equality. This is exactly the
existing scope's local send contract, not an additional effect, parent event,
policy, stored value or recursive CommitActions promise. If ordinary Rust
accepts the micro-probe, a source-copy factory experiment may spell those
existing local send proofs in its generated where clauses. Ordinary syntax
and all paired recursive/refusal/static-denial witnesses remain unchanged.
Two owners, under 140 net lines, zero new public type; no catalogue migration.
Any additional independent capability or recursive availability premise
reopens the design. State/control/custody and interpreter branches stay fixed.

Ordinary-Rust results: conditional_sources compiles. A supertrait containing
an associated send bound carries the usable proof; a blanket implementation's
where clause does not. conditional_context fails E0284 when another generic
repeats the sealed context equation. No context generic is added to production.

Creation-source port hypothesis: expose the existing local send interpretation
contract as one trait on Sends, with InterpretSends and SendSettlements /
SourceSettlementCustody as supertrait-associated bounds. Unlike the rejected
empty tag, its inherited operations and associated custody are the proof.
The owning creation module computes the exact child context privately.
CreationScope uses this port for its actor's Sends; nominal factories require
it for each actual direct child's Sends. This owns one interpreter seam,
substituted by NoSends, domain Sends and real ProxyEffects; it owns no runtime
state or actor policy. It replaces the two existing inline scope send bounds,
not the concrete interpreter or effect algebra. Generated guards preserve the
complete creator equation already recorded. No caller names a context, proof,
role path, callback or policy. Reuse the exact creation kernel and nominal
future cut. Pre-edit pure_parent, recursive and conditional_context failures
are the witnesses. Both real consumers must pass together before broadening.
Two owners plus export, under 180 net source-copy lines, one public trait and
one fewer factory generic; zero new actor state, custody alternatives,
interpreter branch, stored value or dependency. Cross-check same creation,
source-admission, prefix/rejection and Driver custody laws. Reopen rather than
add any unrelated bound, extra context generic or recursive readiness promise.

Creation-source port disposition: reopen. The library compiles, but the paired
consumer exits 101 with five E0277 errors in pure_parent. Associated supertrait
source proofs do not resolve the real scoped ingress/worker establishment
premises. No recursive runtime pass is claimed. The trait, export, dispatch,
association and generated guards are removed together and the prior nominal
source copy restored. No additional source/context/trait bound is accepted.
Logs and rejected owners: /private/tmp/bombay-devx-creation-sources-*.

Runner projection substep (DX05, derived Rust ownership): ordinary Application
already owns Members::Actor and Members::StagingFailure through its existing
ComposeApplication contract. These are not caller choices. Remove those two
independent generic slots from run_with, deriving its identical result and its
execute_with call from that one owner. Existing explicit terminal selection
and child-failure parameters remain until inferred custody is accepted; this
substep does not claim the final ordinary API or close DX03/DX05.
Desired intermediate syntax: Application::new(counter.stop_on_shutdown())
.run_with::<Never, _, _, _>(real_counter_work). The actual counter reply,
shutdown, joined retirement and independent work output stay unchanged.
Startup refusal returns the exact original actor and uninvoked non-Send work.
The two existing application_outcome caller tests are changed before production
and must fail on the prior six-slot interface. Reuse the private composition
contract and unchanged execute_with / execute_application_with kernel. No new
trait, bound, type, alias, lifecycle operation, disposal policy or visibility.
Expected semantic batch: one production owner, existing caller test, fewer than
15 net production lines; remove two public function generics, zero public
Types. Public result types remain exactly the same after substituting the
existing associated types. Any visibility/privacy defect reopens this substep,
rather than exporting composition machinery. Validate independently at the
real external integration-test crate before migrating other callers.

Aggregate checkpoint before lowering: outcome/work/retirement sums stay
6/5/24; actor and staging authority remain ComposeApplication; no subordinate
state is added. All current owned values and transitions survive verbatim.
Branch/module/effect counts remain unchanged. Residue scan requires no second
cause, arrival history, semantic boolean or actor engine. Cross-check cold
application/rejection ownership, caller-local work and Driver custody.

Runner projection disposition: reopen. The pre-edit two callers fail E0107
as intended. The four-slot source compiles internally, but both external
callers reject the private ComposeApplication::Actor at await; three private
interface warnings also expose Actor and StagingFailure. No visibility is
widened and no alias/result facade is invented. Production and both caller
edits are restored. Logs: /private/tmp/bombay-devx-runner-inference-{before,after}.log;
rejected patch: /private/tmp/bombay-devx-runner-inference-rejected.patch.

Retirement custody product substep (DX04, derived conservation law): source
inspection establishes that exactly 20 ActorRetirement alternatives own the
same 14-field product: behavior, unread control/user, descendants, child
failures, capability failures, additional Driver failures, received
interpretation/source, source index, acquired ingress, retirement failures,
terminal report and unread owner cancellation. Allocation refusal owns just
its original actor and allocation cause; ActorTaskFailed/Panicked/Cancelled
have no actor custody. Phase-specific initialization/active effects, source
progress, settlements, completion, primary errors and panic payloads remain
in their original alternatives. They are not equal to the common product.

Replace those 20 repeated field lists with ActorCustody, a named owned product;
retain all 24 alternatives and their phase-specific fields. Add borrowed
custody() on the existing retirement; consuming callers match its original
variant and move that same product. behavior() keeps its current semantics,
including allocation refusal. No outcome classification discharges a cause.
Target caller: inspect retirement.custody().user without matching every phase,
then consume Completed { custody, completion, .. } or the exact failed variant.
Smallest pre-edit witnesses extend the actual counter and startup-refusal
journeys to borrow the original product, preserve actor/allocation pointers,
and consume those same fields. The existing complete retirement owner tests
cover all 24 variants and original executor errors. No local-source/factory
prototype is part of this change.

Reuse current LocalResidual conversion and every existing retirement branch;
delete repeated common field declarations/reconstructions only. Expected
production owners: terminal plus its exact constructing/consuming interpreters,
under ten source files, net source reduction, one new public product. Stage
with the focused caller and smallest interpreter proof, then migrate callers
mechanically only after debug/optimized and inversion proofs pass. Generic
parameters are exactly the current BehaviorState/Root/ChildFailures, each
already instantiated by unrelated real actors and child topologies. No new
bound, event, effect, task, state, phase, flag, dependency or visibility.

Aggregate checkpoint before: terminal alternatives 24; common custody fields
14 repeated 20 times; branch/control sums unchanged. Each common value is
required for exact owned recovery or a future caller decision; each remaining
phase value owns its original incomplete effects/source or primary cause.
The owned product coexists with those causes and is never another transition
authority. Validate exact pointer recovery, accepted effect prefixes, source
cancellation and all cold/active paths against Driver and lifecycle custody
laws before retention. Preserve static authority denials. Missing field,
phase confusion or duplicated cause reopens the experiment.

Custody product focused checkpoint: actual counter and initialization-refusal
callers fail before production with two E0599 missing custody accessor errors.
After the product lowering all nine application_outcome tests pass in debug
and optimized builds, zero failures/ignored. An independent accessor inversion
reports no available product; exactly those two real journeys fail and seven
other tests pass. Original source is restored. Physical focused delta is
+633/-923/net -290 across three source owners and the existing caller test;
one public product, one borrowed method, zero new generic bound or visibility.
Terminal alternatives remain 24, common field declarations reduce from
14*20 to one 14-field product plus 20 product fields. Every phase-specific
value and branch is unchanged. The original ActorRetirement discriminant
remains the sole owner of the primary cause. Disposition: focused law pass.

Mechanical custody migration may now adapt the remaining owner tests, real
consumers, examples and rustdoc to that proven named product. It adds no new
semantic symbol, generic, callback, policy or route. Existing assertions,
independent failure traces, panic values, source lanes and static denials must
remain verbatim apart from grouping owned fields. Run full owner unit tests,
the real receiving/cancellation/outcome consumers, warning-clean all-target
compilation and docs; canonical inferred-API fixtures remain a separate open
gate. Logs: /private/tmp/bombay-devx-common-custody-{before,after,release,inversion}.log.

Facade rustdoc hygiene substep (DX07, derived static namespace law): the
executable custody rustdoc fails E0433/E0425 because actor generation selects
crate::behavior and crate::MailAddr when CARGO_CRATE_NAME equals bombay, even
though rustdoc compiles in its own crate root. Bombay already declares
extern crate self as bombay. The canonical ::bombay path resolves both that
owning alias and rustdoc's actual external dependency. Preserve renamed
external dependencies through the existing proc_macro_crate lookup.
Before production, /private/tmp/bombay-devx-common-custody-doctest.log contains
the failing actual actor/accessor snippet. Reuse the existing self alias;
delete the CARGO_CRATE_NAME branch from the owner resolver. One macro source
file, net source reduction, no new public symbol, alias, type, trait, bound,
event, effect, semantic state or visibility. Re-run executable rustdoc, full
facade denials, and real renamed downstream. No snippet-only imports or hidden
public re-exports are accepted. Actor/control/custody sums and generated
Behavior semantics stay unchanged; only canonical static crate resolution
changes. Failed rustdoc remains a real gate failure until fixed.

Owner-level hygiene gap: with the valid complete Actions signature and Bombay's
own resolver corrected, the same executable rustdoc still fails E0433 in
Behavior's generated crate::behavior path. This is a concrete downstream
consumer failure on locked macros 0.14.0, whose owning behavior_crate branch
matches current source. The failure is /private/tmp/bombay-devx-facade-doctest-after.log;
the restored valid pre-Bombay-fix witness is facade-doctest-before.log.
The initial short snippet omitted the already-required complete Actions return;
that authoring error was corrected before the retained hygiene proof. No
signature restriction was relaxed.

For Behavior's Bombay-package branch, use ::bombay::behavior consistently,
reusing Bombay's existing self alias. Delete the CARGO_CRATE_NAME guess; package
identity does not identify rustdoc's current target root. One existing macro
function, net reduction, no public symbol or actor semantics. The actual
consumer witness precedes this owner edit. Test via an explicitly patched
source copy because actual Bombay still locks the published old macro; do not
claim distribution from that source-only pass. Independent renamed/direct
paths and all owner gates remain required before delivery. No production
Cargo patch, snippet-only alias or hidden import is retained.

The owning facade crate-resolution fixture now registers a real rustdoc test:
Counter performs the complete pure transition, and a pool Worker declares the
same actual assignment/completion contract as the existing worker fixture.
Counter's restored old owner lookup fails, and the corrected behavior lookup
passes. The Worker snippet separately probes the shared Actors lookup before
editing its Bombay-package branch. Its canonical path is ::bombay through the
same existing alias; the actor-library own-package branch is unchanged without
its own witness. No semantic worker policy or second actor surface is added.
Both fixture snippets use fully qualified imports and existing complete Actions
contracts. This is a separate static lookup regression, not evidence for the
open factory/recursive application model.

Retirement migration checkpoint: 357 debug tests and 369 optimized tests pass,
including the owner, actual result receiving/cancellation, exact child custody,
real supervisor/FIFO recovery, facade and lifecycle denials. All 24 outcomes,
20 common products, every original phase-specific field/type and allocation
refusal equation compare unchanged against the baseline. Common fields are
stored once, and neither cause nor authority is inferred from product presence.
Clippy across the workspace/all targets/all features passes after preserving
one long complete ownership witness with a narrowly justified annotation.
The live counter passes and shrinks 147 -> 135 physical lines; the owner terminal
file shrinks 2630 -> 2323. Physical source delta (includes owner tests) is
+980/-1205/net -225; external tests/examples +1254/-857/net +397; docs +27.
One public product and one accessor; zero new trait/bound/actor state/runtime
branch/dependency. The counter retirement object measures 688 -> 696 bytes
on pinned Rust 1.99, an 8-byte alignment cost; no new allocation is introduced.
Residual scan finds no new arrival history, repeated cause, false cardinality,
semantic boolean, nested transition authority or structural user requirement.
Disposition: custody representation pass; publication/distribution still open.

Macro hygiene checkpoint: both valid owning doctest snippets fail with the old
library-root guess, individually for behavior and atomic paths. The corrected
owner passes six unit and six crate-resolution tests, including real doctest
execution. The paired Bombay source copy with only the explicit owner macro
patch passes its executable custody rustdoc; the actual renamed downstream
passes its real actor/ActorSpaces/TerminalProjection check. Actual Bombay's
Cargo manifest/lock remain unpatched. Published macros 0.14.0 still block that
new doctest, so source-only proof cannot close DX27 or full Bombay gates.
The native five-gate attempt captured the pre-fix failing doc snapshot and was
interrupted deliberately; exit 1 is not a pass. Owner full flake check first
fails formatting of the new fixture test; formatting is corrected and the same
full gate is restarted. No Darwin/disk failure is waived. Completed task build
caches are removed with source/logs preserved, restoring 7.5 GB free.

Retained commits: Bombay fe831e9 owns the custody product and mechanical field
migration; a431c2a owns facade rustdoc lookup. Behavior 2ea569a owns both actor
and worker lookup fixes and their registered owning regressions. Production
measurement for fe831e9, removing cfg(test) items before line comparison:
+570/-876/net -306. The physical source count above includes owner tests;
these are distinct measurements. Actor/control/phase alternatives and runtime
transition branches stay unchanged, modules add zero, public products add one.
Both old lookup inversions are killed; all six owner crate-resolution tests
pass. Owner full native verification completes with exit 0 under session 55587:
all ten native aarch64-darwin gates pass, including nextest, package,
Clippy, documentation, doctest, formatting, dependency audit and policy.
Other systems were not executed by this native check. Actual Bombay remains registry-unpatched, and its
new executable doc still needs distribution of the macro fix. Canonical
inferred application fixtures, recursive factory and the other open checklist
items remain required; no completion status is changed.

Relational creation-source hypothesis (derived, source-copy only): declaring
an actor does not certify a particular runtime context. A nominal creation
factory requires each actual direct child's existing send interpretation and
source custody for the actual creator's Event/Birth and shared Spaces. State
that local relationship on Sends, keyed by Parent/Child/Spaces, before testing
factory availability. Unlike the rejected scope-keyed CreationSources port,
this contract does not require CreationScope as a well-formedness premise;
that premise already required the proof the port was meant to supply.

Ordinary syntax stays the existing facade, named birth roles and inferred
Application. Exact rejection, prefix acceptance, binding, original actor and
retirement stay in the unchanged creation kernel. Reuse InterpretSends,
SendSettlements, SourceSettlementCustody, concrete ApplicationCapabilities,
LocalParentReports, NestedBindings and the existing nominal future cut. No
recursive CommitActions availability premise or scalar context equality is
added. One private context alias is parameterized directly by its actual
Parent/Child/Spaces; one public local-source port replaces access to that
private representation. All three parameters have unrelated real actor,
wrapper and hosting substitutions; none is a caller hint.

Pre-edit witness: paired pure_parent/recursive consumer, logged in
/private/tmp/bombay-devx-relational-sources-before.log. Expected source-copy
owners: creation, facade and existing exports; under 140 net lines, one public
trait, no new phase/state/value/runtime branch/visibility/dependency. Actor
retirement (24), creation binding (3) and all outcome sums stay unchanged.
Every surviving loan owns the original creator borrow and staged request.
Reopen for any new independent capability, inferred provenance, placeholder,
recursive proof cycle or second child-source implementation. Do not migrate
other callers until both required consumers pass together.

Relational source disposition: reopen. The named-child candidate passes 20
consumer tests, but valid `births = Services` authoring (where Services is
`Births<StableProxy<Worker, ImmediateActivation>>`) still fails twice with
E0277. The exact caller passes against retained Bombay. A heterogeneous-tail
ordinary-Rust probe also disproves the proposed recursive proof elaboration.
All four candidate source edits are removed; rejected source and logs remain
outside the repositories. No production API or catalogue migration is retained.
Control states, subordinate alternatives, runtime branches and modules remain
unchanged; retained production/public-surface delta is zero. The previous 20
passes do not close the source/factory law.

Caller-contract reinforcement (derived): pure facade declarations using existing
birth-mode aliases remain valid for both one child and heterogeneous closed
children, without inventing parent report ingress. Add both forms to the already
registered full-facade fixture before any replacement factory. Reuse Births,
ChildChoice, Never and the exact existing facade expansion. One test fixture,
zero production lines/types/bounds; complete initialization has no sends or
creations and Continue. The standalone alias witness fails with the rejected
factory and passes with retained Bombay.

Creation-spine isolation (derived, source-copy only): inferred terminal products
do not require changing the typed creation operation. Reuse the original
EstablishChild/CommitActions interpreter with child-local descendant custody;
remove nominal ActorCreation, ChildCreation and CreationScope from this probe.
The facade derives only static retirement metadata. Desired syntax remains
Application::new(root).child(role, actor).run_with(work), with no terminal enum
or runner arguments. Failure retains the complete root, work and child-local
retirement products. Counter/component and both birth-mode alias witnesses
precede this edit; named recursive execution remains a required separate proof,
not a deferral or a leaf-only completion claim. Reuse the original source-copy
creation kernel unchanged, preserving allocation-before-binding, initialization,
accepted prefix and complete rejection equations. Expected four source files,
negative production delta, three public ports removed, zero runtime states,
branches, capabilities or dependencies added. Reopen if the existing kernel
cannot execute the finite supervisor/pool tree without structural caller hints.

Isolation result: eight counter/component/pure-authoring tests and seven
namespace/supervision/FIFO/timer tests pass using the original creation
interpreter. A valid empty birth-mode alias also passes (one test); its first
probe omitted the existing mandatory settlement disposition and is not a
regression. Recursive execution fails at six calls with E0275 through
CommitActions -> InterpretCreations -> EstablishChild. Finite evidence cannot
close recursive acceptance. No source-copy change is lowered to either owning
repository. The nominal factory cut and pure birth-algebra compatibility must
still be proved together. No runtime control state, alternative, branch or
provenance changes occur in the isolation. This is a static-availability
falsifier, not an interpreter failure or a passing recursive check.

Registered caller reinforcement passes in retained Bombay: full-facade fixture
now includes single and heterogeneous existing birth modes; both owner tests
and all four original compile denials pass. Pinned formatting passes. No
production edit, public type, bound or module is introduced. Release preflight
on the exact retained Behavior source passes version/locked metadata across
all four Cargo workspaces plus extracted package-consumer verification; actual
versions remain Behavior/Actors 0.23.0 and macros 0.14.0, not new publications.
GitHub has no open owner PR; main CI and Release completed successfully at
d69f992b, which precedes these fixes and does not certify their delivery.

Caller reinforcement optimized gate: the same two retained Bombay owner tests
and four compile denials pass under --release. Fixture production delta is
zero; tests +26/-0, public API +0/-0. Isolation removes three candidate public
creation ports and measures +35/-351/net -316 source lines across four files,
relative to its previous unretained nominal-factory candidate. These deletions
are not measured against retained Bombay and are not a delivered code reduction.

Owner delivery: draft PR https://github.com/devrandom-labs/bombay-behavior/pull/89
is isolated on codex/devx-owner-contracts at 8c5a1b9. Its seven-file diff is
exactly the proven send-routing and macro-hygiene source/tests/law changes,
+335/-13 physical lines; no other programme model is included. It is labelled
semver-breaking because explicit owned-lane SendInput paths change. Fresh
exact-head native nix flake check exits 0: all ten gates, 927 tests passed and
zero skipped. Four-workspace release preflight and extracted archives pass.
The first invocation was deliberately interrupted after finding its two new
tests unstaged; only the restarted staged invocation is passing evidence.
Sources/logs were preserved while 1.4 GB of completed task build caches were
removed. No disk/signing waiver was used. The draft is not merged or published;
CI and review remain required. This source delivery uses the previously read
bombay-behavior-release skill and does not close the application inference gate.

Borrowed completion law (derived, DX04): ActorRetirement::completion() lends
the original Completion only for the Completed alternative. Allocation,
initialization, behavior, effects, cancellation and capability failures return
absence, with the complete original retirement still owned by the caller.
Stopped and Exhausted remain distinct; returned Completion is not a claim that
child or late-joined capability cleanup has no failures. Ownership equation:
&retirement -> Option<&original completion>, with retirement unchanged.

Inspect/reuse the existing 24-alternative ActorRetirement, Engine Completion,
from_local projection and original capability-retirement matrix. No new sum,
product, bound, state, authority, dependency or interpreter path. Desired caller:
match retirement.completion() while separately inspecting custody and retaining
all failure values. Pre-edit downstream counter/initialization/allocation/task
witnesses plus the five-cause projection matrix call the missing method;
expected E0599 before implementation. Expected production +40, owner/downstream
tests under +20, README/API/audit updates, one method and zero new types.
Control sum 24 -> 24; custody fields 14 -> 14; modules unchanged; no historical
state, repeated cause, false cardinality, nested authority, semantic boolean or
structural caller syntax added. Existing from_local projection remains the
sole authority; the accessor is exhaustive inspection only.

Owner PR #89 CI passes published API audit, Linux Nix, all eight mutation
shards, CodeQL and dependency policy; its first strict aggregate verdict fails
on one stale renamed function floor and two unaccounted replacement functions.
Downloaded immutable run 38057237529 reports prove both replacement emit-no-op
mutants were caught after successful builds. Preserve the old viable floor of
one under the owned path, add one for the new inner path, and remove only the
nonexistent old function key. No floor is lowered and no mutant is waived.
Rebasing all eight original artifact reports and running the pinned strict gate
passes: 205 viable / 415 total candidates. The three-line baseline correction
is applied to both owner feature and isolated delivery branches; new-head CI
remains required. This verification metadata edit adds no production surface.
