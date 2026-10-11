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

Selected topology decision (user-authorized 2026-10-11): allow one explicit
application topology declaration. Derive its structural spaces, occurrence
products, runner projections and complete recursive retirement types behind the
canonical Application path. Fully inferred topology is no longer mandatory.
The declaration must use domain roles and actual child relationships, remain
statically checked against Behavior's birth/protocol contracts, and preserve
all failure custody. Ordinary callers still write no structural paths, managed
actor aliases, spaces or recursive terminal enums. Compare ordinary Rust with
a concise DSL before selecting macro syntax; syntax owns no runtime semantics.
The selection belongs to each Application, not globally to the root actor: two
applications may reuse an actor under different valid closed graphs. Validate
the complete actual decorated birth row, including singleton template births;
one space per graph vertex is insufficient when distinct actors share a protocol.

The first production work should be in **Bombay application assembly and terminal
consumption**, followed by demonstrated gaps in Behavior composition. The
initial evidence did not justify starting with a new Behavior trait, effect
language, application macro, or builder framework. The approved explicit
topology path has a private native recursive proof; public creation authority
and complete downstream execution remain prerequisites for a production DSL
or catalogue migration.

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
| Assemble and run an application | One explicit domain topology declaration feeding `Application`, on a caller-owned async host | Switching to `App`, spelling the final actor type, constructing spaces or terminal projections |
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
3. Declare application topology and roles once. Behavior-owned dynamic births remain
   explicit `Actions`; application startup children use `Application::child`.
   These differ in ownership and timing and must not create the same child twice.
4. Run `Application::new(actor).run_with(work).await` with type inference.
   This call shape is a **target**, not a claim that the current unannotated
   expression compiles. Derive hosting and complete result custody from the
   single explicit topology declaration; infer the resulting runner arguments.
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

**DX02 — Derive installation from one declared topology. P0, Bombay consuming Behavior proofs.**
Allow one explicit domain topology declaration and statically validate its
child relationships against declared births and logical-host requirements.
Derive internal occurrence products and standard local hosting. Eliminate mandatory
`ActorSpaces` structs, repeated protocol lists, `ChildHead`/`ChildTail` and nested
failure tuples in application source. Exact external recipients must not require
local hosting; a logical external destination needs an explicit domain binding,
not a fabricated local actor. Repeated occurrences must not create independent
conflicting logical namespaces or be deduplicated into one child identity.
Acceptance: add or reorder a child without editing spaces or terminal plumbing;
an absent genuine logical binding still fails with the protocol and role named.
Do not promise automatic Rust type-level deduplication before a concrete witness
proves the projection and sharing law using the existing owning primitives.

**DX03 — Generate complete terminal custody. P0, Bombay.** Applications receive
complete concrete results without defining recursive `TerminalProjection` enums
or naming composed actor types. Keep origin and child-role distinctions and the
complete failed child definition/actions. Eliminate projection boilerplate from
counter, topology, pool and supervision before declaring success. The owning
library must derive exact actor-indexed subtrees from the same explicit topology
and account for transitive and recursive birth structures, not only
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

The requested crates.io/[Awesome Rust](https://github.com/rust-unofficial/awesome-rust)
review inspected current registry metadata and downloaded published sources.
No dependency was added. The bounded comparisons are:

| Candidate | Responsibility and disposition |
| --- | --- |
| [`frunk_core` 0.5.0](https://docs.rs/frunk_core/0.5.0/frunk_core/) | Typed heterogeneous products/sums. Substituting actual HCons/Coproduct into the descendant eligibility equation still produces E0275 in both profiles; it does not prove recursive metadata. Core already owns the closed occurrence product. |
| [`recursion` 0.5.4](https://docs.rs/recursion/0.5.4/recursion/) | Stack-safe traversal of a defined recursive data family. Traversal is distinct from proving the family's types and child interpreter eligibility; no owner machinery deletion is demonstrated. |
| [`recursion-schemes` 0.1.3](https://docs.rs/recursion-schemes/0.1.3/src/recursion_schemes/recursive.rs.html) | Fix stores Box of F::Layer<Fix<F>>. Keeping exact current child payloads retains the eligibility cycle; replacing recursive positions by X requires a different, proven closed actor frame. It supplies no child interpreter proof. Partial expansion uses an erased callback. No adoption. |
| [`enum_dispatch` 0.3.13](https://docs.rs/enum_dispatch/0.3.13/enum_dispatch/) | Generates static delegation, but its generated TryInto rejection returns a string and drops the other variant. An independent original-worker Drop witness fails in debug and optimized builds. Rejected for custody-bearing sums. |
| [`enum-as-inner` 0.7.0](https://docs.rs/enum-as-inner/0.7.0/enum_as_inner/) | Checked consuming access returns Err(original), but the derive also generates unchecked unsafe accessors. It expands the public surface and does not select recursive custody. No adoption. |
| [`auto_enums` 0.8.10](https://docs.rs/auto_enums/0.8.10/auto_enums/) | Generates closed Future/Iterator delegation. It does not discharge child-host capability requirements; no repeated owner implementation has been shown to justify another macro dependency. |
| [`derive_more` 2.1.1](https://docs.rs/derive_more/2.1.1/derive_more/) | Derives standard conversions and checked variant extraction. The three existing u64 newtypes MailAddr, TimerId and TimerGeneration have six five-line conversion impls; these are bounded boilerplate candidates, not an assembly fix. No dependency/build-cost or deletion experiment is retained; thiserror remains the error owner. |
| [`async-recursion` 1.2.0](https://crates.io/crates/async-recursion/1.2.0) | Fresh registry/source recheck: src/expand.rs still emits Pin<Box<dyn Future>>; excluded by the static-dispatch law. Ordinary Box allocation alone is not excluded. |
| [`bon` 3.10.2](https://docs.rs/bon/3.10.2/bon/) | Awesome Rust lists its checked builders. Named field construction does not establish recursive child custody or interpreter eligibility; adding a required-field typestate builder would conflict with the selected owning constructors and ordinary Application spine. No adoption. |
| [`lru`](https://docs.rs/lru/latest/lru/struct.LruCache.html) / [`priority-queue`](https://github.com/garro95/priority-queue) | Catalogue source check: Cache deliberately supports Eq keys and exposes a recency slice; lru requires Hash and would change that contract. PriorityQueue already uses BinaryHeap for immutable priorities and checked FIFO ties; priority changes are not its law. Reconsider only with a measured scale or mutable-priority requirement. Neither addresses Application assembly. |

Published-source comparisons and pinned 1.99 debug/optimized falsifiers are in
/private/tmp/bombay-devx-crate-review/. These are library evaluations, not
Bombay acceptance tests. Existing thiserror, trybuild, Tokio and standard owned
sums/products remain the selected tools.

## Completion and change record

### Requirement checklist

This is the single implementation checklist. `Open` includes unexecuted
acceptance; no source inventory or focused pass closes an entire requirement.
DX01–03 now accept the approved single explicit topology declaration; historical
inference probes below remain historical evidence, not current acceptance syntax.

| ID | Implementation and tests | Remaining work / disposition |
| --- | --- | --- |
| DX01 | Private unified Application owns root/scope/members/families; actual public cold-child/origin/late-report and missing-host work recovery pass both profiles; supervisor/FIFO and namespace-refusal evidence retained | Open: retain the public model, unify App, all seven journeys, combined families and complete cold recovery |
| DX02 | Child bindings, logical Hosts and exact delivery inspected; one explicit topology declaration approved | Open: derived hosting, repeated roles, child/topology consistency and missing-binding denials |
| DX03 | Private declared graph metadata and genuine external scoped recursion pass both profiles, with complete refusal/retirement, nine authority denials and startup cancellation | Open: retain minimized public port and generated schema; Application scope/cold-report custody, wrappers/templates and complete downstream journey before migration |
| DX04 | Lossless work/cleanup, borrowed state/custody/completion and total owned terminal inclusion; all 24 outcomes retained; native retirement passes 300 library plus eight journey tests debug/optimized and all eight terminal denials | Open: published macro dependency and final package gates; inferred descendants and complete proportional recovery |
| DX05 | Existing execute/run work and caller-host contracts inspected | Open: infer associated runner types; cancellation and host corpus |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Pure actor authoring retained, including single/heterogeneous birth-mode aliases and owned nested staged creation; ordinary and pool-worker rustdoc fixes, renamed consumer and original denials pass | Open: publish macro fix; resolve factory availability with recursive creation; complete generic/private corpus; empty-alias/spoof law now passes owner/downstream profiles and all ten native gates |
| DX08 | Composed SendInput routes unique lanes; six owner regressions, OneShot/Periodic in both shutdown orders, complete Actions and two static denials pass | Open: downstream dependency delivery and full named-lane catalogue evidence |
| DX09 | Existing ActorExt inspected | Open: prove missing real decorations and delete duplicate recipes |
| DX10 | Existing constructor owners retained | Open: measured policy syntax across all five families |
| DX11 | Real FIFO replacement-source refusal preserves original workers, source diagnostic, accepted job and independent caller work; owning debug/optimized and strict Clippy pass | Open: inferred template path, prepared prefix, shutdown and late completion |
| DX12 | Existing occurrence/creation owners inspected | Open: inferred roles, relational identity and rejection corpus |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Startup/native failure, complete provenance, independent raw/projected retirement, read custody, template rejection and closed-control/report custody implemented; actual301 tests pass both builds and563 downstream tests pass with one inference failure | Open: full downstream gates and released dependency delivery |
| DX17 | Retained custody accessor/product and composed send changes have bounded interface provenance | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial executed | Open: complete pinned diagnostic corpus and renamed/minimal consumers |
| DX20 | Existing ActorInterface/ExternalActor retained | Open: correlated replies, cancellation and slow/closed customers |
| DX21 | Existing lifecycle and child retirement retained | Open: inferred access with complete shutdown/retirement evidence |
| DX22 | Existing Entity/HTTP owners retained | Open: same Application spine, exact rejection and family/root failures |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inference fixtures; original startup falsifier repaired in actual coupled-source runtime; owner tests and static denials preserved | Open: retain inferred API and complete downstream corpus/final gates |
| DX26 | Counter baseline 144 lines; native linking exhausts 4.8/8.3 GB; compact Nix profiles execute finite witnesses with space remaining; current counter main is 135 lines, with terminal/runner ceremony still present | Open: canonical caller/compiler/runtime measurements and final resource gates |
| DX27 | Fresh Bombay 102fc2d Nix: four pass, seventeen fail on locked owner API mismatch; Timers 0.2 released/pinned; owner PR #89 passes native ten-gate/927-test, package preflight and all CI; draft #90 at 66de825 and tool-generated version preview 3501be1 pass native ten-gate/961-test, 112 doctests, package preflight and all eleven exact-head CI jobs | Open: owner review and release/pin; public declared-topology API and downstream gates |
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
laws; public declared-topology assembly remains open.

The expanded selected-programme surface was explicitly authorized on
2026-10-09. That authorization does not waive per-stage law, provenance,
composition, aggregate-drift or verification gates. Stage 1 must finish before
catalogue migration or later programme stages.

### Earlier inference experiments and current declared-topology proof

The desired caller is one `Application` with one domain topology, derived hosts and
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

Native projection has an independently demonstrated owner gap: raw child
retirement and the projected report are different ownership roles, but the
retained interpreter forces their descendant types to agree. The original
`application_terminal_custody` journeys reject the finite inferred candidate
with 17 diagnostics. Selecting a projection after launch cannot change its
already-owned raw task. Pure total inclusion uses `(origin, retirement).into()`;
native projection consumes that same exact origin and complete retirement once
inside the existing task, preserving cancellation, native panic and later siblings.

Two selection models are rejected in `DEAD_ENDS.md`: an unused occurrence
requires ordinary callers to invent an annotation, and per-variant associated
inputs reject original-valid pure child variants sharing one origin alias but
accepting different payloads. No production from either is retained. The private
replacement selects descendants using the actor and finite direct failures known
before launch, with one native implementation per derived enum reusing exact
`From` inclusions. Its counter main is 109 lines; all 300 library tests pass
in debug and optimized builds, including native projector panic, startup raw/
projection retry, cancellation and later-sibling custody. Four complete terminal
journeys, real FIFO source refusal and supervisor activation/retirement pass both
profiles; pure shared-origin inclusion, finite inference and timer orders pass.
This still is not retained: handwritten template metadata and recursive execution
remain open. Strict library Clippy rejects an oversized derive function; no
lint waiver has been applied. Source/logs: /private/tmp/bombay-native-input._k_nomqj.

Before extracting a narrower native-only batch, the derived law is:
`raw retirement<Actor, Descendants, Failures> + exact Origin -> native report`.
`Descendants` is selected before launch; it need not equal the report. Preserve
all five creation-binding states, two startup-custody alternatives, six child
failures, receiving slots, initialization/commit ordering and original task
causes. Reuse the existing creation and retirement interpreters, closed occurrence
products, `ActorRetirement`, `ProjectTerminal` and projection task; delete only
the forced raw/projected descendant equality. The focused native leaf regression
first implements the existing port for `ActorRetirement<Worker, Never, ()>`:
actual creation/retirement rejects it with E0277, requiring the report instead of
`Never`. Its two original worker allocations, actual creation IDs, distinct fresh
addresses, complete custody and later sibling must survive. The owned-conversion
regression independently fails actual source with missing-From E0277.

Scope before edits: five existing runtime/macro owners, approximately +75/-50
production lines, zero new public types/modules/control alternatives. Reshape
the existing native port with `Origin`, `Actor`, finite direct failures and
associated descendants; no unused occurrence parameter in this narrower batch.
No inferred metadata, facade construction, Application inference or catalogue
migration belongs to it. The original 300 native tests and both unrelated real
templates must remain valid, along with pure aliases/shared-origin payloads and
static denials. Full inference/recursive acceptance stays open independently;
this extraction cannot close those requirements. Native failure and raw/projection
retry need fresh debug/optimized evidence on the extracted source. Raw data must
remain readable without acquiring execution or projection authority. Aggregate
checkpoint before edits: control sums 5/2/6, no new state/branch/module, every
surviving alternative retains its original pending/acquired task or exact cause;
no historical state, repeated cause, inferred provenance or semantic boolean.
Final measurements and residue scan are required before retention.

The native-only extraction passes 300 library tests in debug and optimization,
plus the leaf witness, four existing terminal journeys, real FIFO source refusal,
supervisor activation, and real timer/shutdown orders with replay rejection and
original allocations. Strict focused Clippy passes without a production waiver.
Pure shared-origin payloads and all eight terminal denials pass; only the
duplicate-pair and forged-role diagnostic carriers change from the old port to
`From`. Full workspace/all-target compilation passes on coupled source. Native
failure/retry controllers retain their original bodies and semantic oracles.

Checkpoint before retention: production +187/-118/net69 (five semantic owners
and mechanical example conversions); tests +1084/-154/net930 (Rust and both diagnostic fixtures), including all
untracked fixtures; zero new public types, actor states, alternatives or modules.
The existing native port gains associated raw descendants and replaces its
arbitrary terminal input with the actual actor/failure inputs. One private macro
function separates native projection from pure inclusion. Actor control branches
and the 5/2/6 custody sums are unchanged; each alternative still owns exactly its
existing pending/acquired task, report or native cause. Read-only capabilities
retain raw storage independently of projection authority. Residue scan finds no
new arrival history, repeated cause, inferred provenance, false cardinality,
nested authority, semantic boolean or ordinary occurrence placeholder. Cross-check:
DX03/04/16/21/25, execution-ownership DG-API/DG-PROJECTION/DG-WORK, Driver
D-RETIRE-1 and current atomic creation/replacement laws. Disposition: pass for
this bounded native law; inferred/recursive acceptance and full package gates
remain open. Source, exact counts and logs:
/private/tmp/bombay-native-retirement-xeszwlu8. Mechanical migration now applies
only the proven standard inclusion and native signature; it changes no policies,
constructor choices, topology, wrapper order or effect interpretation.

Retained Bombay commit `102fc2d` implements this bounded native law.
Fresh actual worktree verification uses pinned Rust 1.99, compact debug/release
profiles and the exact paired Behavior source, with no committed manifest/lock
patch. Native law tests: 300 library + eight integration journeys pass in both
profiles; terminal tests pass three tests including one pure fixture and all
eight denials. Workspace/all-target strict Clippy, formatting and the counter
executable pass. Actual counter remains 135 lines with explicit terminal/runner
ceremony; the 109-line inferred version remains private. Commands/results:
/private/tmp/bombay-devx-native-retirement-actual-results.json and
/private/tmp/bombay-devx-native-retirement-actual-{debug-final,optimized-final,static,clippy-final,counter-final,format}.log.
An initial static run in an external target directory resolved released Core
instead of the paired unpublished source and failed; it is not passing evidence.
The target now sits under the actual worktree so nested trybuild inherits the
same temporary patch configuration, removed with the original lock restored.
Full inference, released dependency delivery and authoritative repository gates
remain unaccepted. No Darwin signing or disk failure is waived.

Before native signature migration, the owning real FIFO source-refusal test
passes debug, optimized and strict Clippy, preserving first/replacement worker allocations, accepted job,
actual source diagnostic/provenance and independent boxed caller work. This
substep is tests +385/-0, production/API/control-state delta zero. Format check
passes. Each source snapshot uses its own target directory: a shared-target check
reused an experimental interface and is not passing evidence. Fresh owning logs:
/private/tmp/bombay-devx-source-refusal-owning-{test-fresh,optimized-final,clippy-final}.log.

The metadata obligation is independently red before execution: the actual
facade Node with a StopOnShutdown<Node> child cannot even name its inferred
Descendants (E0275). A conditional uniform birth-family projection admits pure
foreign/generic/empty/nested aliases and real FIFO/fixed-supervisor metadata in
both timer/shutdown orders, but retains that same recursive proof cycle.
Unconditional nominal storage cannot postpone the exact field's eligibility
(E0277). No generated metadata or template-specific exception is retained.
The precise equation and disposition are in DEAD_ENDS.md; private logs are in
/private/tmp/bombay-retirement-metadata.o4obk6bk/. This is distinct from the
remaining interpreter CommitActions/EstablishChild cycle.

Read-only follow-up separates two narrower premises. Manually supplied Node/Leaf
schemas guarded only by each actor's own native settlement contract can name
complete recursive boxed retirement data (pinned 1.99 pass); this does not infer
the schemas. Existing public DriverRetirement also carries a pure actor's
original Actions and error without a settlement guard, preserving all owned
allocations. It cannot manufacture accepted receipts or acquired-source prefixes.
A valid pure child emitting LedgerDispatch lacks ActionItem; its receipt-ready
parent's metadata compiles, but the child's full native ActorRetirement fails
E0277. Parent readiness therefore does not imply child retirement readiness.
Actual child launch replaces its immediate parent context; fixed Self recursion
repeats the same interpreter type rather than growing host ancestry. The loop
remains operational CommitActions→creation dispatch→child establishment→
CommitActions. These reviews add zero production, tests, public types, states,
branches or modules. Existing control/subordinate sums remain 5/2/2/3/6/24,
ActorCustody retains fourteen fields, and original task, receipt, prefix, error
and projection custody remain their only causes. No flags, erased values,
placeholder capability, arrival history or caller structural syntax is added.
Cross-checks: actor-transition-algebra, atomic-runtime-settlement,
execution-ownership DG-PROJECTION and normalized atomic laws. Disposition:
reopen automatic family and factory ownership. Evidence:
/private/tmp/bombay-retirement-data.l5tbw4o8/ and
/private/tmp/bombay-occurrence-factory-w5qkcqq9/HOST_IDENTITY_REVIEW.md.
The independent context review identifies existing ChildCreationAt as a real
lower-order selector: it exposes a known Child/Descendants without CommitActions
or ProjectTerminal. Automatic construction of that binding from the report
being inferred still depends on the same report's ProjectTerminal. It retrieves
an already-selected family rather than supplying an independent nominal one.
No source candidate or new context parameter is retained; the preceding zero
checkpoint remains unchanged. Exact equations:
/private/tmp/bombay-retirement-data.l5tbw4o8/CONTEXT-REVIEW.md and
/private/tmp/bombay-occurrence-factory-w5qkcqq9/RETIREMENT_OWNER_REVIEW.md.

The joint factory obligation remains open: pure nested birth aliases and actual
recursive execution must work together, with operational child-source
interpretation and exact settlement custody. Nominal Self/NoSends execution
passes three generations, but adding immediate-child source premises rejects
valid pure Nursery→Parent staging. Whole-product execution premises recurse
(E0275); projected source markers do not prove heterogeneous leaf operations.
No extra depth bound, ingress placeholder, erased future or duplicate source
sum is retained. The repository-root `DEAD_ENDS.md` records their falsifiers.

The routed receiving candidate was also removed. Its correlated input equation
passed four static forgery denials, non-Send standalone error custody, complete
heterogeneous rejection and later effects, replay, receiving cancellation,
projection failure and both shutdown orders. It nevertheless lost a child on
pending startup disposal. The same witness through ORIGINAL
Births::InterpretCreations fails debug/optimized against actual locked registry
dependencies. This proves a runtime owner gap independently of the proposed
interface. Repair parent-owned startup custody before revisiting inferred
creation. No candidate receiving port is part of the actual public API.

The next factory proof must reuse actual LocalParentReports, SourceSettlementCustody,
fresh allocation/init/binding, the existing closed child products and selected
occurrence, parent-owned joins and create-before-dependent-send ordering.
Pure construction cannot certify execution; a child task cannot take or borrow
its parent's binding row. Empty/custom creation products and non-Send standalone
initialization errors remain legal. The finite witnesses and recursive veto are
source-only results, not package or full-gate acceptance.

Before retaining the original consumer regression: scope one Bombay owning
creation test module, expected test-only +163/-5, zero production/state/module/API delta.
The test requires exactly one original definition, its allocation/content and
available ID/kind/route through request, exact reply or joined retirement;
unrelated replies/children cannot satisfy it. This is deliberately red evidence
for DX03/DX16/DX21/DX25, not acceptance. Reuse the current batch, child bindings,
startup cancellation and parent retirement; no new bound or runtime operation.
Current control sum CreationBinding is Established | Rejected | StartupRejected;
all three remain, as do existing progress/result alternatives and one transition
authority. Residue scan: no new history, semantic flags, inferred provenance,
false cardinality, nested actor or caller structural syntax. Cross-check:
creation/lifecycle custody and Driver retirement laws. Exact private original
source/patch/logs: /private/tmp/bombay-original-creation-cancellation.znm6zpay.
The next production model must put original startup authority/receipt/control/
join in parent-owned custody BEFORE the first await, reuse spawn_local_execution
and ordered child retirement, and prove complete rejection/replay/retirement.
It must not report a birth/restart before fresh startup and binding commit.

Retained original regression (`d3b1363`) measurement: Bombay tests +205/-5/net +200,
coordination +13/-0; production/public types/modules +0/-0. Actual unpatched
locked registry debug and optimized executions both fail the intended exact
custody assertion (one test, 290 filtered). Strict `cargo clippy --locked -p
bombay-rs --lib --tests -- -D warnings` and `cargo fmt --all -- --check` pass.
Commands run in pinned Bombay Nix; logs original-cancellation-actual-*.log under
/private/tmp/bombay-devx-. This adds evidence, not a passing completion claim. The final witness reads the
route from actual batch reservation before polling child startup; it predicts no
nonce. Its name is pending_creation_cancellation_retains_child_custody.

#### Startup and creation custody

The concrete failing consumer requires these bounded laws. They are derived
ownership/composition laws or deliberate Bombay failure policy; none is an
Agha executor or restart guarantee. Application syntax remains unchanged,
except borrowed creation provenance on the existing terminal origin.

| Owner and law recorded before editing | Complete ownership and machinery reused |
| --- | --- |
| Core: storage eligibility belongs to the interpreter. | Split the old universal shape GAT into Empty and conditional ChildOccurrenceMember, using the same sealed three-clause occurrence walk. Existing universal shapes retain their Behavior-only contract; raw task storage states its real payload premises. Direct child-failure metadata stays independent of recursive execution eligibility. Pure authoring gains no runtime obligation. |
| Bombay: startup custody enters the parent before await. | Store the actual startup receipt, control and raw OwnedTask in the existing binding. Initialization rejection acquires raw output and returns the current child/error/actions before one-way terminal projection. Reuse spawn_local_execution, existing receiving slots, settlement forwarding and ordered retirement. Request every child's retirement before joining; preserve an acquired value across disposal and retry. |
| Bombay: uncommitted native failure grants no actor authority. | Existing ChildFailure gains StartupActorTaskFailed and StartupProjectionTaskFailed, owning original JoinError and typed origin. Startup retirement owns Raw or Projection: the original raw task/result or the actual terminal projection join/result. Project complete raw cleanup once; fabricate neither an OwnedTask nor cancellation authority after acquisition. |
| Core/Actors: a destroyed accepted definition remains unavailable. | ChildCreationOutcome::Unavailable { id, kind } retains exact correlation and explicit provenance. Native cause and allocated origin remain in parent custody. Runtime borrows Err and leaves its row intact; only Ok transfers the old initialization classification. WorkerCreationRejection gains the same unavailable facts, retaining activation and any prior stop independently. Observe/Born/Restarted still require genuine commitment. |
| Bombay: authority reads hide startup payload storage. | ChildBindingAt and its cursor return borrowed endpoint/control/kind or the exact unavailable prerequisite ID; absence means unknown binding. Separate private ChildCreationAt/cursor owns raw mutation/removal/recording. Reuse one registry and occurrence cursor. Ordinary readers require only their actual local Behavior contract; remove their raw-state matches and unrelated settlement/base/failure bounds. |
| Bombay: terminal origin owns complete creator provenance. | Existing ChildOrigin adds exact CreationId and CreationKind, including replacement predecessor, with creation_id()/creation_kind() accessors. Existing maps and role conversion preserve address, actual reserved route and both facts. Remove duplicate ID/kind fields from every ChildFailure; RootOrigin remains separate. No provenance inference, new type, bound, capability or caller topology spelling. |
| KeyedPool: correlated creation failure follows configured policy. | Reuse Member::admit_creation, creation_positions, retire_unrecovered_worker, retire_role and begin_retirement. RetireRole conserves sibling progress; StopPool transfers remaining original receipts through existing retirement admission, preserving emitted prefixes. Validate the whole batch before admitting any member; retain workers, activation, jobs, customers, bindings and prior stop. Delete the success-only admission. |
| FIFO/KeyedPool: whole-batch rejection preserves each original request. | Namespace exhaustion and interpreter corruption use the same configured reaction. Move the existing two-alternative CreationBatchRejection once to the direct-worker owner; both pools reuse it. Foreign ID, wrong kind and replay return the whole unchanged input before any partial admission. No fake routed receipt, worker stop, source call or successful restart. |
| Testkit: observation preserves the new receipt's exact facts. | Existing test-only ObservedCreation adds Unavailable { id, kind } for both heterogeneous alternatives. ModelHost admission, decisions and trace remain unchanged; no fabricated native-failure simulator or false establishment/corruption classification. |

The original regressions precede their respective production edits:

- Restricted original GAT fails E0271/E0277 for actual startup payload storage.
  Conditional selection passes for empty and repeated children; missing head
  and tail independently deny E0277, as does a sealed foreign node. Native
  Core-only tests use Rust 1.95; Bombay's Rust 1.96 minimum is never bypassed.
- Pending startup disposal loses the original child on the old interpreter in
  both profiles. The repaired actual-source test reports exactly one joined
  original definition, its allocation/content and replacement provenance.
- Native executor shutdown leaves the creator future alive but destroys the
  child definition. The old kernel panics and takes its JoinError row; old Core
  and Worker sums reject the required receipt syntax with E0599. Repaired
  interpretation yields Unavailable and retirement returns the original native
  error, task ID and allocated origin once.
- Generic read projection originally fails five storage-only E0271/E0277
  premises. The retained read/exclusive-custody split avoids an identity
  extractor and a second registry; rejected projections are in DEAD_ENDS.md.
- Exact joined replacement provenance initially fails E0599 for both absent
  origin accessors. The witness uses an actual reserved route distinct from
  its creation ID. Fresh births, replacement predecessor, role conversion,
  projection failure, receiving retry and later siblings retain the full origin.
- Keyed initialization-panic policy tests initially return zero queued jobs
  instead of one/two; namespace/corruption tests initially emit one Unexpected
  instead of two complete creation rejections. Unavailable policy tests show
  the same defect before integration. Foreign/wrong-kind second items, both
  roster orders, committed sibling progress, accepted prefixes and exact replay
  preserve all independent custody after repair.
- Testkit's two unavailable observations initially fail E0599/E0004. The
  corrected oracle preserves ID/kind without changing the host model.

Complete aggregate checkpoint: one original actor transition authority.
CreationBinding changes 3→5: Starting owns receipt/control/raw task;
RetiringStartup owns pending/acquired raw or projection custody; Established
owns genuine endpoint/control/projected join; Rejected retains no invented
actor; StartupRejected owns its original classified error/cleanup. Raw and
Projection are two different currently owned payload equations, not an event
engine. The read sum contains established borrowed authority or one exact
prerequisite, not a second lifecycle state. ChildFailure changes 4→6, with
origin as sole provenance owner. Core creation outcome changes 4→5 and worker
rejection 7→8. Keyed pool control states remain five, Member states four,
creation admission and retirement custody three each. Existing two-arm batch
cause moves once. No policy, generic execution bound or public runtime type is
invented. Remaining values are precisely the original request/reply/task/ACK,
current actor/error/actions, activation, prior stop, job/customer/binding or
borrowed authority required for settlement and retirement.

Residue scan: no new arrival history, repeated cause, false cardinality,
semantic flag, nested actor transition authority, type erasure, placeholder,
compiler-origin bound or structural caller requirement. Cross-checks cover
creation freshness/provenance, commit-before-dependent-send policy,
initialization/source custody, Driver retirement/receiving laws and normalized
proxy, FIFO and keyed pool laws. Disposition: pass for the bounded storage,
startup, unavailable receipt, template rejection, read and provenance laws;
reopen the complete programme for the joint pure-authoring/recursive factory
proof. The later inverse and truthful failure-reporting batches preserve these
control sums, retirement order and rejection tombstones.

Measured concrete layouts agree in both profiles: ChildOrigin 16→32 B,
RootOrigin 8→8, ChildFailure 128→128, actual complete child report 512→528,
binding 600→616 and ActorRetirement 496→496, all aligned to 8 B. The earlier
startup-only scalar Root u64 probe grows binding 128→536 and map entry
136→544 B (+408 each); raw acquired result is 488 B. These substitutions do not
support a universal storage-cost claim. No boxing is selected from a lint.

Retained measurements against Behavior 9ca633e and Bombay d3b1363:

| Tree/batch | Production physical lines | Tests | Public type change |
| --- | --- | --- | --- |
| Behavior current, 23 Rust files | +525/-274/net251; rustdoc +106/-6/net100; nonblank compiled code +408/-263/net145 | +3141/-12/net3129 | +1 conditional storage trait; two existing-sum Unavailable alternatives; one existing-trait inverse method |
| Bombay startup/provenance 4fd7ad6, 7 source modules plus 1 integration test | +655/-175/net480, including documentation +5/-2/net3 | +922/-44/net878 | +0 types; two existing failure alternatives and two origin accessors |
| Bombay inverse delivery 29da7a4, 1 existing source module | +22/-10/net12 | +504 | +0 types/states |
| Bombay truthful startup report 3c34bee, 1 registry method | +6/-3/net3 | +150 | +0 types/states |

CFG-scoped measurements include interspersed test functions/modules and
`cfg(all(test, tokio_unstable))`; first-test-module counts are superseded.
Later runtime batches add 15 production lines to the 480-line startup batch.
Owner production arm tokens across current changed modules are 811→821;
runtime remains 216→231. Public runtime nominal declarations remain 11,
ActorRetirement alternatives 24 and SpawnError 9. Increased code owns proven
custody/rejection responsibilities; it is not described as code reduction.

Actual Behavior 0dedb69 passes all ten native pinned Nix gates with
`nix flake check --show-trace --keep-going --max-jobs 1 --cores 2`: build,
nextest (956 passed, zero skipped), strict Clippy, docs, doctests, package,
Rust/TOML formatting, audit and deny. Doctests total 112, including exact
head/tail/foreign denials; seven conditional-storage tests pass optimized.
The first fresh check failed unpublished research links; the next exposed nine
ownership transfers inside assertions. Corrected links and pre-assertion
transfers pass the authoritative documentation/ownership scan. No Darwin
signing or disk waiver is used. Log:
/private/tmp/bombay-devx-owner-full-nix-assertions.log.

Pinned fuzz-shell runs pass 1,000 inputs each for keyed assignment, keyed
binding and FIFO sequences, with scratch corpora. The initial `nix run .#fuzz`
invocation failed Darwin libiconv linking; the pinned `nix develop .#fuzz`
environment supplies the required linker environment. These runs do not claim
coverage of native task loss, which has independent runtime witnesses.

Actual paired-source Bombay passes 300 library tests and strict library/test
Clippy. The original 297-test library passes optimization; focused new inverse
and truthful startup-rejection journeys pass debug and optimized. They retain
original allocations, complete error/request values, real commit/shutdown,
replay, provenance and both meaningful wrapper orders. Temporary CLI owner
patches are development evidence; every committed registry lockfile is restored.
Full Bombay/optimized consumer/package gates and released owners remain open.

Source, pre-edit vetoes and detailed counters remain in the bounded scratch
proofs: /private/tmp/bombay-pending-storage.0zBK6I,
/private/tmp/bombay-startup-binding-proof,
/private/tmp/bombay-startup-authority-proof,
/private/tmp/bombay-child-origin.tbw14s8e and
/private/tmp/bombay-keyed-creation-custody.xzfotjq0. The owning checklist above
remains the sole completion disposition; no new permanent audit series is added.

The selected native loan now passes a genuine foreign caller in debug and
optimization: four recursive nodes, three audit children, real reports, original
boxed initialization refusal and complete actor-indexed retirement. Nine
independent invalid callers are denied in both profiles, including wrong failure
product, occurrence swapping, forged loan, missing edges and separate report
ingresses. Startup cancellation and host-shutdown tests preserve original rows,
retirement order, allocation/provenance and the exact native task failure.
This remains private evidence, not a retained Application or DSL.

The first public alias candidate failed E0446 and was removed. Its replacement
stores the existing complete raw `(Vec<Descendants>, Failures)` product, matching
ActorRetirement/OwnedTask; the private constructor and unchanged Commit require
the exact child failure product. The loan fixes occurrence and failures while
preserving the actual origin function item; no private mapper/host is exported.
Compared with the prior private native cut, the selected-loan delta (before
removing superseded cursor methods) is production +290/-287/net3 across four
existing modules/reexports, public types2, new modules0. This is not the delta
from the actual production branch; the earlier native cut adds a further net2.
Creation/startup/retirement alternatives remain5/2/24; native match/if/else counts
remain3/2/6. No historical label, semantic flag, duplicate registry, source
weakening or extra transition authority. Evidence:
/private/tmp/bombay-selected-custody-jyhj820f,
/private/tmp/bombay-selected-denials-ne0mc1bu and
/private/tmp/bombay-selected-startup-ic0qhkg6.

The private cold-origin journey passes both profiles with actual ordinary
ChildOrigin<RootBase, ChildHead> and cold ChildOrigin<ActualManagedRoot, Role>
kept distinct; the actual origin function item survives creation and joined
retirement. Actual public Application cold-child execution still fails E0277:
it hardcodes ActorSpace instead of the selected graph scope. The owning probe
also exposes an early-stopping parent panic on a sibling's later report; the
original closed report is consumed. Preserve this independent failure witness
at /private/tmp/bombay-cold-origin-iz73ahf3. The existing Core reply already
supports complete rejection with ClosedParentControlLane; forward-only
EventIngress cannot recover the report after failed send. Before any production
repair, compare admission-before-mapping in the Communication owner with inverse
ingress: the former must return the original non-Clone input without invoking
mapping on closed admission, preserve current accepted-tail semantics and never
reserve a queue slot before a panicking conversion. No Core capability widening
or structural recovery path is selected. Expected bounded owner/native changes:
existing ControlSender admission plus ParentReporting and its interpreter;
zero new types, about +15/-9 production, focused owner and real sibling-report
regressions in both profiles. Communication owner af3e3f1 implements the admission law in
[draft PR #11](https://github.com/devrandom-labs/bombay-communication/pull/11):
all seven native Nix gates, 88 tests (zero skipped), strict all-target Clippy,
four focused tests in both profiles and twelve bounded Loom models pass.
Both original-source inverses fail solely on five missing-method diagnostics.
Exact-head Linux CI run38124396727 passes; current actual Bombay still selects
registry0.1.3. Owner production is +21/-2 including docs; one public method,
no types/dependencies/versions. Its native repair is +15/-15 in reports.rs,
with existing typed rejection and no new bounds. The original single-report-stop
journey passes both profiles; restoring the original interpreter reproduces
the lane panic in both. The later complete request remains in EffectsFailed
with ClosedParentControlLane, and the earlier report remains in the root result.
The actual owning regression was applied before production and fails at the
original lane panic in both builds/orders. The actual +16/-15 native fix then
passes all301 library tests debug/optimized (5.34s/5.28s), workspace/all-target
strict Clippy and formatting. Complete Box fields/allocation, actual timers,
replay and retained source are covered. With inherited task-local owner source
selection, workspace/all-features nextest has563 passed, one failed, one skipped
in71.432s; the sole failure is the still-open inferred-assembly compile corpus.
The first CLI-only run stopped with compile-suite failures because spawned
Cargo selected registry0.23; the inherited-source control and full rerun retain
that failure honestly. Native report source, all three custody equations,
creation/storage/terminal shapes remain unchanged apart from the stated repair.
Logs: /private/tmp/bombay-parent-report-actual-{baseline-debug,baseline-optimized,
library-debug,library-optimized,clippy,workspace-inherited-nextest}.log. The original
registry lock is byte-restored. Fresh authoritative Nix exits1: four gates pass,
seventeen fail on the unpublished owner API mismatch; no signing or disk
exception is observed or waived. Exact log:
/private/tmp/bombay-parent-report-actual-registry-nix.log. The report aggregate
adds no control/subordinate alternatives, production242→243, public types0,
modules0 and new bounds0; the original report-send two-arm branch moves to
its interpreter settlement, with complete request ownership as the sole cause.
Creation/startup/retirement remain5/2/24 and all three source-custody equations
are unchanged. No arrival history, semantic flag, structural user recovery,
second transition authority or duplicate error is added. Cross-checks: existing
Communication closure/accepted-tail contract, Core ParentReportReason/
NoReturnEmitter source laws and the original early-stop retirement witness.
Disposition: pass for the bounded repair; registry release/gates and complete
Application acceptance remain open. The owning report test is one existing-module cfg(test)
seam, 451 formatted lines: actual report/timer interpreters, pure child Actions,
accepted and rejected source custody, replay, exact boxed fields and both
OneShot/shutdown orders. Its empty hosting/binding inputs perform no unused
host or creation operation; it proves item/correlation laws, while the separate
early-stop journey proves genuine establishment and origin custody. No new
runtime type, bound, path, visibility or control state is selected.
Public scope selection, cold recovery, wrapper/templates,
full gates and minimization remain required. Disposition:
reopen complete Application model; retain only the independently proven laws.
The repository-root DEAD_ENDS.md records older removed factory candidates.

The next bounded Application experiment follows the actual scoped caller's
`.child(role, actor)` E0599 in both profiles, recorded before edits at
/private/tmp/bombay-application-scope-5s0do3ul. Derived ownership law: one
Application owns `(root, scope, members, families)`. Scope is the actual local
hosting capability selected per application; two applications may select
different scopes for the same root. Ordinary comparison syntax is
`Application::with_topology(root, scope).child(role, actor)`; generated graph
construction may later supply that scope. Root-only construction selects the
existing actual ActorSpace internally, without a caller marker or discarded
transient space. No global Root::Scope association is selected.

Missing-host/unstarted return owns Self and original work. Member rejection
owns original root, scope, uninstalled families, work and existing complete
StagingFailure; HTTP additionally retains its acquired listener. Prepared
custody owns the actual composed actor and exact scope. Existing family joins
remain independent original results; this does not reclassify the currently
consuming invalid-directory policy. Reuse ComposeApplication/its exact Origins,
ApplicationWorkPublication, execute_application_with and existing Entity setup;
delete the competing App owner and hardcoded Application host construction only
after the scoped cold-child journey is proven. Expected private comparison:
three existing Application modules/reexports, one owner removed, no additional
control phases or bounds beyond the existing selected-host execution contract.
Checkpoint baseline is ApplicationOutcome6, WorkCustody5, child binding5 and
startup custody2. The private four-field owner now compiles and executes
through public run_with: actual ordinary/cold origins and receipt IDs/kinds/
addresses, accepted result and late rejection survive in both profiles. Missing
host returns the exact selected scope, untouched root/member and original work;
re-entering executes that same callable and returns its original Box allocation.
HTTP-feature library compilation passes, but its runtime/family composition is
not proven. Private source delta +76/-674/net-598 in four existing files removes
the parallel App owner; this is pre-minimization/private evidence, not retained
production or a final caller-ceremony claim. Evidence:
/private/tmp/bombay-scoped-application-p0fuke2_/{LAW.md,COUNTS.json,
CANDIDATE-DEBUG.log,CANDIDATE-OPTIMIZED.log,MISSING-HOST-DEBUG.log,
MISSING-HOST-OPTIMIZED.log}. Existing Entity execution prepares Arc<Scope> and the current kernel holds
Arc<Arc<Scope>>; Hosts already delegates that shared scope. The exact recursive
consumer with Arc<Scope> fails solely on missing NativeChild in both profiles,
while the bare-scope baseline passes:
/private/tmp/bombay-shared-native-scope-lrt82aa9. Before edits, derived inclusion
law: shared ownership of the same hosted registration scope preserves the same
native child capability. Compare a direct Arc<Scope> delegation with changing
all kernel prepared payloads. Select the narrower existing-trait implementation:
clone only the existing inner Arc, move the exact selected map/order/actor space,
allocations/control/request/origin/position loan unchanged, and return the
original Scope::establish future. No public extractor, new type, Clone<Scope>,
Commit guard, extra actor state or child/source proof is selected. Expected
private delta: one existing creation module, roughly40 lines, zero types/states.
Independent denials, original Box refusal, full recursive retirement, actual
shared registration identity and both profiles must pass before retention.
Root-retirement inference, cancellation/failure custody,
HTTP/Entity combined composition, strict gates and wrapper/template proofs
remain separate required acceptance; field unification alone satisfies none.

### Dependency delivery and exact gates

The fresh released-dependency failure selects a bounded delivery action:
prepare the already-retained Core/Actors custody changes after owner PR #89,
without changing their semantics. Reuse the recorded conditional occurrence,
Unavailable receipt, pool rejection and ingress inverse laws and their original
falsifiers. Expected extraction: 29 tracked code/test/contract-document paths (actual 31,
including the two ingress files from the later inverse batch; current 33 with
the direct owner-test path and reviewed mutation baseline),
zero new production beyond the retained +251-line owner batch; one new public
trait relative to #89, unchanged source relative to the retained feature branch.
Exclude the large DevX research documents from the delivery branch and link the
single owning checklist. Create a separate stacked draft for review, verify exact
source equivalence, pinned gates and packaged consumers before requesting any
merge/publication authorization. Existing #89 and both working feature branches
remain intact. This is delivery preparation, not inferred-assembly acceptance.

Delivery review identified missing direct owner coverage for the retained inverse.
Derived law: recover(child_input(original)) returns the original complete proxy
control; any owned outer lane or foreign inner proxy event returns the exact
original event. Focused tests use the actual StableProxy/ProxyControl/ProxyEvent,
original boxed worker and activation data, and two EventLayer orders. Reuse the
existing RuntimeAddr, OwnedActivation and protocol visibility in the proxy's
owning unit tests; no placeholder actor, source, plan, executor or public accessor.
Expected test-only +150 lines in one existing cfg(test) module; production,
control-state, transition-branch, public API and dependency deltas zero.
An independent inversion will stop inner recovery and must fail the focused
round trip in both profiles; restore exact production bytes before fresh gates.
Do not retain an inverse representation or constructor merely for these tests.
The three owner regressions pass in pinned debug and optimized builds.
Stopping inner recovery independently fails the intended ownership assertion in
both profiles; the production bytes were restored exactly. Actual test delta is
+138/-5 (net +133); all production/API/state/module/dependency deltas are zero.
Delivery head 66de825 exactly matches the feature branch crates. Its fresh
pinned native Nix check exits 0: all ten gates pass, 961 nextest tests pass with
zero skipped, and 112 doctests pass. Release preflight passes all four Cargo
workspace locks, README versions and the packaged archive consumer. No signing
or disk exception is needed. Production is unchanged from extracted 322c3db;
its real Bombay packaged-consumer run has 562 passed, one failed and one skipped
(563 executed, 61 binaries). The sole failure is the four still-unimplemented
inferred-assembly fixtures; workspace/all-target Clippy and the downstream
doctest pass. The attempted nextest --all-targets command failed test listing
on the custom benchmark and is not counted as a passing check. Registry-based
Bombay full gates and publication remain open. The extracted delivery is stacked
draft [PR #90](https://github.com/devrandom-labs/bombay-behavior/pull/90), labeled
semver-breaking, after #89. Exact-head CI was dispatched on its feature branch
because ordinary pull-request CI filters for main. At 7a85639 the published API
audit, Linux Nix/external-interpreter/fuzz-build job and all eight mutation
shards pass; strict aggregation fails on unaccounted EventLayer child_input and
recover_child_input functions. Both exact candidates are Unviable, not survivors:
Default::default() fails E0277 because EventLayer has no Default; returning a
default Input fails E0277 because ChildInputIngress promises no Input:Default.
Retain both typed vetoes and classify these two known-zero-viable entries without
altering production bounds, existing mutation floors or exclusions. Original
shard 2/3 candidate, outcome, diff and compiler logs are downloaded under
/private/tmp/bombay-devx-custody-mutations-38114649392.
Separately extend the existing Core creation tests and their genuine ChildHead/u8
private-input port through two EventLayer orders: injection retains the exact
user lane, recovery returns the complete input, and each foreign owned lane
returns its original allocation unchanged. Reuse TestAddr, Child and that input
implementation; introduce no actor/source/plan fixture or production port.
Pre-edit budget: +90 test lines and two baseline entries, zero production/state/
API/dependency changes. Retained tests at owner 66de825 (feature e7a8bbb) add +76/-1 test lines and
two known-zero-viable classifications (+3/-1 JSON); no production/API/state
change. Both Core tests pass debug/optimized; injection interruption and stopped
recovery fail for the intended law in both profiles, four independent failures,
with original production bytes restored. Complete control/subordinate sums,
branches, production lines, modules and public spellings remain unchanged;
foreign owned allocations, the exact input and its complete lane are retained.
Residue scan and the preceding custody/composition laws pass. The strict gate
reproduces the original aggregation failure and passes the reviewed baseline on
all eight original reports: 205 viable / 417 total mutants. Existing floors and
exclusions are unchanged; a future viable mutant in either classified function
still fails the gate. Exact-head native and package logs:
/private/tmp/bombay-devx-custody-delivery-66de825-{nix,nextest,doctest,preflight}.log.
New exact-head CI [run 38116165593](https://github.com/devrandom-labs/bombay-behavior/actions/runs/38116165593)
passes all eleven jobs, including Linux Nix/external-interpreter/fuzz build,
published API audit, all eight mutation shards and strict aggregation
(205 viable / 417 total). Exact logs/terminal status:
/private/tmp/bombay-devx-custody-ci-66de825.{log,json}. Before the subsequent empty-birth alias batch, feature and delivery crate sources
and mutation baseline were byte-equivalent. Public declared-topology assembly remains open.

The isolated version preview `3501be1` on `codex/devx-release-preview` was
produced by pinned release-plz 0.3.169 with the verified stack-safe API checker:
Behavior/Actors 0.24.0 and macros 0.14.1. Reviewed changelogs name the breaking
contracts. The first preflight rejected stale secondary locks/README; all four
locks now retain identical external packages and the exact-head preflight passes.
Metadata/docs delta: ten files +56/-23; production Rust, tests and API delta zero.
Native `nix flake check --show-trace --keep-going` passes all ten gates,
961 nextest tests (zero skipped) and 112 doctests. Exact-head
[CI run 38117186479](https://github.com/devrandom-labs/bombay-behavior/actions/runs/38117186479)
passes all eleven jobs. The separate API check accepts the 0.24 major-change
classification, skipping compatibility checks; it is not a compatibility proof.
Logs: /private/tmp/bombay-devx-release-preview-{full-nix,nextest,doctest,api-audit}.log,
/private/tmp/bombay-devx-release-preview-3501be1-preflight.log and
/private/tmp/bombay-devx-release-preview-ci-3501be1.{json,log}. No merge or
publication is claimed; Bombay still selects registry 0.23/0.14.

The user-approved explicit graph is being proved separately before production
lowering. Ordinary Rust schema data/space proof passes against exact owning
roles (including repeated Node roles and an actor-indexed leaf `Never` subtree).
The full native recursive probe first fails E0275 under the original establishment
blanket, then passes debug/optimized by selecting nominal closed-edge operations
whose bodies reuse the unchanged establishment kernel and all three exact
source-custody equations. Actual reports establish four Nodes/three audit actors;
the initialization refusal returns its original Box and joined retirement owns
complete native subtrees. These are owner-internal probes, not public DSL proof:
foreign private hosts cannot be implemented from downstream code. Whole-row
validation, shared-protocol spaces, singleton supervisor/proxy roles, finite
source denials, public authority and ordinary-Rust/macro comparison remain open.
Pre-edit law and logs: /private/tmp/bombay-closed-creation-du62dgu1/LAW.md,
BASELINE-FOCUSED.log and CANDIDATE-OPTIMIZED.log; data/space proof:
/private/tmp/bombay-declared-topology.g8fpfq6k/ordinary.log. No catalogue migration
or new production representation is retained from these private probes. Further
scope tests reject missing/wrong edges and preserve shared actual protocol space.
A deeper decorated child originally passed a base-only check with an undeclared
birth; the same complete row equation on each resolved actual Child now vetoes
it with E0277 in both profiles. Data/coverage artifact totals 302 lines, zero real
production edits; cold copied actors still have six unused-field warnings.
Singleton template role proof reuses the sealed owning product and existing role
resolver: StableProxy, fixed/dynamic supervisors pass both profiles; seven invalid
rows/targets/protocols fail, preserving replacement ID and original allocation.
Private proof records: /private/tmp/bombay-declared-topology.g8fpfq6k/RESULTS.md and
/private/tmp/bombay-single-worker-edge.fJtD35/RESULTS.md. Operational native cut
retains all three source-custody equations and the byte-identical establishment
body; five capability/edge denials and original E0275 inverse are checked in both
profiles. Existing states remain 5/2/24, branch arms 12/12, native kernel delta
+74/-72/net2, three generated operations 79 lines. Private authority data cannot
be silently equated with actual ApplicationOrigins; foreign consumer fails
privacy/availability. Public factory integration remains open.


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
Fresh authoritative Bombay `102fc2d` check:
`nix flake check --show-trace --keep-going` exits 1, with four native checks
passing (formatting, Driver-law evidence, Observe loom and panic-abort denial)
and seventeen failing. The locked published Behavior/Actors 0.23.0 contract
lacks ChildOccurrenceMember, recover_child_input and ChildCreationOutcome::Unavailable;
the original ChildOccurrenceShape::Member requirement also differs. The build
reports seven E0432/E0576/E0046/E0599 diagnostics. These failures precede the
inferred caller witnesses; paired-source focused passes cannot certify this
released build. No signing or disk-exhaustion failure occurred; 35 GB remained.
Exact full/build logs: /private/tmp/bombay-devx-full-nix-102fc2d{,-build}.log.
Full gates remain open until actual dependency delivery and inferred interface
retention, including the published macro rustdoc path. Package final canonical
syntax and all authority denials against actual released dependencies before DX27 closes.

### Empty-birth alias law and verification

Concrete Bombay facade consumer exposes a spelling-based owner gap: an alias
for NoBirths is rejected unless supplied an irrelevant creation-settlement
policy, while a nonempty mode named NoBirths is accepted. Derived law: absence
of fresh-child capability needs no settlement policy; eligibility follows
BirthMode::Child = Never, independently of Rust spelling. Preserve pure authoring,
exact initialization and both timer/shutdown orders without adding runtime ingress.
Original empty consumer fails the parser, and the named nonempty consumer runs;
private candidate accepts the alias and rejects the spoof with E0271. Complete
wrapper initialization Actions match the literal NoBirths reference in debug and
optimized builds. Pre-edit law, source and logs:
/private/tmp/bombay-empty-birth-policy-769jvzr0/.

Before retaining production, extend the existing owning FirstChild declaration
with its empty alias and prove the original parser failure; the existing generic
message actor tests the same alias without losing its type parameters. Reuse existing
BirthMode/Behavior generics and exact Never, deleting BirthsSpec::is_no_births.
Only a policy-free existing mode receives that semantic constraint; generated
nonempty rows retain their explicit-policy requirement. Expected one semantic production
module under +12/-15 lines, zero new public types/methods, one existing test
module and Core/macro rustdoc. No state, branch, error, creation-source equation,
factory capability or interpreter changes. Aggregate alternatives, production
transition authority and owned values remain unchanged; pure, initialization,
creation provenance and facade hygiene laws cross-checked. The original parser and an independent missing-Never-guard inverse falsify the
pass and denial respectively in both profiles. Current owning generation tests
pass all 19 cases in debug/optimized; the parser passes six tests. Actual Bombay
consumer compares complete timer/shutdown initialization Actions in both orders
and profiles, with the same exact owner sources. Final native
`nix flake check --show-trace --keep-going` passes all ten gates, 961 nextest
tests (zero skipped, 6.742s) and 114 doctests; diagnostic
checker passes 68 annotated denials. Exact nextest/doctest counts and logs:
/private/tmp/bombay-empty-birth-policy-769jvzr0/ACTUAL-{FULL-NIX-FINAL,NIX-NEXTEST,NIX-DOCTEST}.log.
No catalogue migration belongs to this bounded owner law.
The first full owner Nix run passes nine checks; the doc gate fails because its
standalone rustc diagnostic probe lacks Cargo package/manifest context for the
new macro denial. Cargo doctests pass all 44 Core cases. Before changing that
checker, retained its E0271 assertion and reproduced the context failure; now each
probe receives the actual owning manifest/name with both library artifacts
unchanged. All 68 annotated denials pass. Verification edit +10/-1; no actor
capability or dependency. Final batch: semantic macro +13/-14/net -1;
Core rustdoc +37, tests +5/-3/net2; zero public types/modules or runtime
branches/states changed. BirthsSpec and CreationSettlementDisposition remain
2/2 alternatives; they own the same declaration and explicit disposition values.
The removed spelling predicate has no independent domain responsibility.
Residue scan: no no-op caller policy, erased protocol, arrival history, repeated
cause, source or factory premise added. Provenance remains the pre-edit actual
consumer plus owning FirstChild parser failure. Disposition: pass for the bounded
alias law and native verification. Stacked draft [PR #91](https://github.com/devrandom-labs/bombay-behavior/pull/91), a1a06ef, has byte-identical crate sources and passes its own ten native checks, 961 tests (6.963s, zero skipped), 114 doctests, 68 diagnostics and package preflight across four workspace locks. Exact-head [CI run 38122047497](https://github.com/devrandom-labs/bombay-behavior/actions/runs/38122047497) passes all eleven jobs, including strict mutation aggregation (205 viable / 417 total). Exact status/logs: /private/tmp/bombay-devx-empty-birth-a1a06ef-ci-run.json and -ci.log. Earlier version preview3501be1 excludes this additional batch. Release delivery remains open.


### Remaining completion obligations

The generic private-child input witness exposes another concrete runtime gap:
an established parent binding can outlive its child's control receiver. The
actual retained-control regression passes after genuine commit, delivery,
shutdown and endpoint termination; the claimed unreachable closed-send branch
is false. Direct public StableProxy input fails only an unrelated shutdown
bound, and its shutdown wrapper fails missing child-input forwarding. Neither
failure may be repaired with a placeholder event or widened protocol.

Next bounded law, recorded before production: input embedding must permit exact
owned recovery when admission closes. This is the existing affine rejection
law, not a new actor capability. Desired caller remains ordinary
ChildInput::after(id, input), with wrapping inferred. Extend the existing
ChildInputIngress ownership port with recovery returning Input or the unchanged
foreign event; forward both directions through EventLayer. Reuse ProxyEvent's
existing RecoverEvent and ControlClosed, preserving source qualification and
the whole ChildInput, including its creation ID. Remove the generic
interpreter's unrelated shutdown premise and its false unreachable branch.
The read-only binding remains the same owner; no new source, path parameter,
trait, phase, error or registry is selected. Before editing, record the absent
recovery-operation compile veto and the complete non-Clone proxy request test;
prove missing/pending/closed bindings, replay and both meaningful wrapper orders
before migration. Expected scope Core user_event and its one existing input
fixture, proxy protocol, runtime delivery and focused tests; about +30 Core,
+5 Actors and +15 runtime production lines. Counts and full composition audit
remain required. Evidence: /private/tmp/bombay-child-input-custody.qq6n6tna.
The complete actual test module fails before production with E0576 for absent
recovery and E0277 for the unrelated shutdown/forwarding requirements; no
other error codes. Selected inverse batch: three existing production modules,
+48/-10/net38 production, +507 test lines, zero new types/traits/paths/states,
one method on the existing ingress. Final production is +51/-10/net41
including three additional rustdoc lines. Reuse the existing proxy inverse and
remove its separate runtime recovery bound. Existing EventLayer alternatives
remain Inner | Owned; two inverse match arms preserve each original value.
Original five binding phases, two startup-custody alternatives and six native
failure alternatives are unchanged. Pre-edit checkpoint: pass for this scoped
model. Actual focused native/composition tests pass debug and optimized,
as does the existing Core custody/replay test; all 299 runtime library tests
and strict library/test Clippy pass with paired command-line owner patches.
Owner gates pass as recorded above; full downstream gates and published
dependencies remain required.
Logs/source: /private/tmp/bombay-devx-child-input-actual-before.log and
/private/tmp/bombay-child-input-recovery.o0d5i77j.

Independent startup reporting falsifier: a joined child initialization refusal
returns its original definition/error but also emits an empty
StartupRetirementFailed. Starting already placed its ID in creation order;
recording a later fact-free row does not remove that entry. Derived law:
retirement reports actual remaining failures only; complete returned refusal
is not a second independent failure. The retained registry reconciles that
same ordered collection with its current row, keeping rejection tombstones and
order for owned tasks/facts. No new state/type/port, one selection branch;
existing 5/2/6 state/failure counts remain. Residue scan finds no history,
duplicated cause, false cardinality, new transition authority or semantic flag.
Cross-check: creation/rejection ownership and Driver retirement/receiving laws.
Disposition: pass for this bounded law. Pre-edit estimate was one registry
method/test module under +10 production lines; actual +6/-3/net3 and tests +150.
Actual original fails only the final phantom assertion, then passes debug and
optimized; 300 library tests and strict Clippy pass. Original RED:
/private/tmp/bombay-inferred-application.qm31zb9h/PHANTOM-ORIGINAL-DEBUG-COMPLETE.log.

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
