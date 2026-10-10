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
| DX01 | Existing Application declaration/composition inspected | Open: unify App, all seven journeys, cold recovery |
| DX02 | Child bindings, logical Hosts and exact delivery inspected | Open: inferred hosting, repeated roles, missing-binding denial |
| DX03 | Inferred/recursive regressions registered; source-copy factory/facade and role custody pass seven downstream debug/release tests | Open: selected public representation, own-birth role access, supervision/pools and complete owner gates |
| DX04 | Retained lossless work/cleanup access; source-copy role selection preserves exact borrowed and unmatched ownership | Open: retain complete inferred descendant consumption and audit actor retirement custody |
| DX05 | Existing execute/run work and caller-host contracts inspected | Open: infer associated runner types; cancellation and host corpus |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Facade sender-independent regression retained in ec79d9f; original authoring and denials pass | Open: preserve this law through inferred custody; full expansion/hygiene/private/generic corpus |
| DX08 | Existing named Actions lanes retained | Open: unrelated templates, both orders, complete effects |
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
| DX25 | Pre-edit missing-method regression, seven debug/release tests, killed cleanup inversion, 65 custody/runner tests | Open: inferred syntax and complete downstream corpus |
| DX26 | Counter baseline remains 144 physical main lines | Open: comparable caller/compiler/runtime measurements |
| DX27 | Timers 0.2 released/pinned; owner and Bombay gates, unpatched extracted/MSRV and renamed facade evidence pass | Open: package the final inferred API and execute its downstream denials and journeys |
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

Complete current task delta from the initial documentation commits: Bombay
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

The user explicitly authorized the expanded selected-programme surface on
2026-10-09. The cumulative task currently touches 21 repository paths, including
five Timers release paths and three uncommitted Bombay regression paths.
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
