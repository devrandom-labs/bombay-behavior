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
target and duplicate-role cases. No competing actor-authoring macro is selected. The approved topology declaration owns application graph syntax only.

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

Deeper stable-language review (user requested): primary compiler/API sources
and40 executable language cases across pinned Rust1.95/1.99 and debug/optimized
profiles distinguish useful techniques from missing proof. These are feature
availability/falsifier tests, not actor acceptance or production changes.

| Technique | Executable result and responsibility |
| --- | --- |
| Nominal coinduction versus conditional native predicates | Recursive nominal custody is Send and preserves its original payload; an ordinary trait whose applicability requires itself denies E0275 on both compilers. The [compiler coinduction guide](https://rustc-dev-guide.rust-lang.org/solve/coinduction.html) explicitly separates recursive calls inside concrete bodies from cycles in where clauses. This supports the existing native-port cut; it supplies no custom execution auto trait. |
| Stable autoref selection | Concrete NoBirths selects its empty case; the same value passed through a generic function selects the generic alternative on both compilers. The [original pattern author's limitation](https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md#limitations) matches this witness. Concrete macro-site selection cannot supply generic recursive eligibility. |
| GAT families and precise opaque capture | A declared type family preserves affine input allocation; use<> lets a returned Send/static task avoid capturing its temporary non-Send owner on both compilers. [GAT RFC](https://rust-lang.github.io/rfcs/1598-generic_associated_types.html), [capture rules](https://doc.rust-lang.org/edition-guide/rust-2024/rpit-lifetime-capture.html). These select/loan known values rather than discover a graph or prove child interpretation. |
| Concrete boxed async recursion | A concrete recursive future passes Send/static checking without erased futures on both compilers; [stable since1.77](https://blog.rust-lang.org/2024/03/21/Rust-1.77.0/). This repairs future size, independently of trait applicability. |
| Opaque aliases/associated outputs and return-type notation | Type-alias impl Trait, opaque associated types and return-type notation each deny E0658 on1.95/1.99; root item fields spelled _ deny E0121. [TAIT](https://doc.rust-lang.org/unstable-book/language-features/type-alias-impl-trait.html), [opaque associated types](https://doc.rust-lang.org/unstable-book/language-features/impl-trait-in-assoc-type.html), [RTN](https://doc.rust-lang.org/unstable-book/language-features/return-type-notation.html). No nightly switch or visibility/bounds workaround is selected. |

Source/logs/results: /private/tmp/bombay-native-type-laws-1b_kv8bk. Parallel
read-only reviews agree that GATs/continuations carry existing evidence, while
body-local concrete native operations break the applicability cycle. A narrower
root-only inference candidate remains credible: infer the actual root value,
leave every recursive child class concrete, and require only its genuine parent
report admission. Actual LocalParentReports qualifies each ReportToParent payload
separately; SourceAdmission owns one named source input and does not summarize
an opaque child Sends product. Before any new lowering, prove that exact narrow
operation with the counter and two-order recursive reports. Do not substitute
the old whole-Commit guard, enumerate compiler-discovered payloads in application
code or claim automatic derivation from this read-only review.

## Completion and change record

### Requirement checklist

This is the single implementation checklist. `Open` includes unexecuted
acceptance; no source inventory or focused pass closes an entire requirement.
DX01–03 now accept the approved single explicit topology declaration; historical
inference probes below remain historical evidence, not current acceptance syntax.

| ID | Implementation and tests | Remaining work / disposition |
| --- | --- | --- |
| DX01 | One actual Application owns root/scope/members/families; App removed. Actual cold Account plus native families, staging recovery, unpolled cancellation and root refusal pass both profiles; supervisor/FIFO and all seven example packages use that owner | Open: generated topology/runner path, complete acquired-family-prefix and HTTP cancellation custody; full published gates |
| DX02 | Approved application-scoped topology declaration; private logical hosting, repeated same-type roles, replacement/address resolution and lease retirement pass both profiles; missing/ambiguous host denials retained | Open: public lowering, complete child/topology consistency and canonical migration |
| DX03 | Private declaration executes32 published-contract journey runs and12 static denials; root-only inference with parent-custody selection additionally executes16 recursive/refusal/counter journeys across both profiles and4 shutdown denials | Open: retain the proven sealed constructor/root declaration; cold and inferred standalone-root syntax, complete generated failure/cancellation custody and final gates |
| DX04 | Lossless work/cleanup, borrowed state/custody/completion and total owned terminal inclusion; all24 outcomes retained. Required Behavior/Actors0.24.0 and macros0.14.1 are published and selected | Open: inferred descendants, complete proportional recovery and final consumer gates |
| DX05 | Private run_with infers retirement schema from the exact original root and owned Members; counter, supervisor, FIFO and recursive journeys pass without runner type arguments | Open: retain API; infer remaining runners and complete cancellation/non-Send/host corpus |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Pure actor authoring and composed staged creation retained; actor/worker rustdoc and renamed consumer fixes published in macros0.14.1. Empty-birth alias/spoof regressions pass owner/downstream profiles and ten native gates | Open: empty-alias PR91 remains unmerged; public recursive execution applicability and complete generic/private corpus |
| DX08 | Unique composed send lanes, six owner regressions, OneShot/Periodic in both shutdown orders, complete Actions and static denials published in Behavior/Actors0.24.0; actual Bombay uses the registry contracts | Open: full named-lane catalogue evidence |
| DX09 | Existing ActorExt inspected | Open: prove missing real decorations and delete duplicate recipes |
| DX10 | Existing constructor owners retained | Open: measured policy syntax across all five families |
| DX11 | Real FIFO replacement-source refusal preserves original workers, source diagnostic, accepted job and independent caller work; owning debug/optimized and strict Clippy pass | Open: inferred template path, prepared prefix, shutdown and late completion |
| DX12 | Existing occurrence/creation owners inspected | Open: inferred roles, relational identity and rejection corpus |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Complete startup/provenance, raw/projected retirement and control/report rejection retained; actual301 owner tests pass both builds. Published-contract Bombay e4d2db1 nextest:571 pass, one topology-suite failure, one parent-invoked child skipped | Open: generated topology/custody and complete downstream gates |
| DX17 | Retained custody accessor/product and composed send changes have bounded interface provenance; DirectoryConfig owns immutable validated limits, removing impossible family/runtime error branches and preserving affine inputs | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected; existing thiserror owns the narrow configuration rejection with both original limits | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial executed | Open: complete pinned diagnostic corpus and renamed/minimal consumers |
| DX20 | Existing ActorInterface/ExternalActor retained | Open: correlated replies, cancellation and slow/closed customers |
| DX21 | Method-local inferred shutdown ingress; runners require only capabilities they execute. Both root wrapper orders, request replay, natural domain/HTTP completion and complete retirement pass7 focused tests per profile; absent/ambiguous lanes remain denied; old-source inverse denies only E0277 | Open: generated canonical access, child-derived phases/drain corpus and final repository gates |
| DX22 | One Application owner now composes ordinary/cold actors, HTTP and Entity; real combined cold Account plus hydrated/passivated/reactivated family and three independent failure/recovery journeys plus pre-transfer configuration rejection pass both profiles | Open: acquired family-prefix cancellation, complete HTTP/family/root failure corpus and generated assembly |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inference fixtures; original startup falsifier repaired in actual coupled-source runtime; owner tests and static denials preserved | Open: retain inferred API and complete downstream corpus/final gates |
| DX26 | Current counter135 lines retains terminal/runner ceremony; pinned-formatted inferred prototype114 lines removes both, with one domain topology declaration and complete success retirement. Both profile runs pass; warm rebuild+execution3.24/7.21s is not a clean-build comparison. Completed task caches reclaimed; native gates run without disk/signing failure | Open: canonical caller/compiler/runtime measurements and final resource gates |
| DX27 | Reviewed Behavior/Actors0.24.0, macros0.14.1 and Communication0.1.5 published/tag-verified; Bombay e4d2db1 consumes registry packages without patches. Exact owner release gates pass; downstream full Nix17 pass/four fail, nextest571 pass/one fail/one skip; compilation, formatting, fuzz check and Engine74 tests+1doctest per profile pass | Open: topology/custody fixture failures, PR91 delivery, retained topology and final consumers |
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
| Facade macro crate roots, Bombay `a431c2a` and Behavior `2ea569a` | Derived static namespace law: Bombay's existing `extern crate self as bombay` resolves its library, sibling targets and rustdoc. Valid real actor/worker doctests fail independently under the old CARGO_CRATE_NAME guesses. Reuse proc_macro_crate for renamed external crates. | Delete the guesses; Bombay source +2/-5, no new symbol/type/bound/alias. Six unit and six resolution tests including executable actor/worker doctests pass. Both original defects were restored independently and fail the intended witnesses. Bombay now selects the published macros0.14.1 containing the fix. |
| Shutdown capability locality, Bombay `2fb1601` | Derived: require existing typed ingress only at `request_shutdown`, inferred through genuine actor layers. Root execution/natural completion do not inject it. The prior reversed-root/natural/HTTP consumers fail E0277; pinned Axum proves the deleted serving-error arm unreachable. All original inputs, custody and authority remain. | Production+26/-30/net-4, tests+334/-0; zero types/modules/new methods, one inferred method parameter. Seven focused tests per profile,301 all-feature library tests, four HTTP recovery runs, source inverse, workspace check/fmt/Clippy pass. Fresh full Nix17 pass/four unfinished-topology failures; nextest578 pass/one same-suite failure/one skip. Programme acceptance remains open. |
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
The private sharing implementation now passes complete recursive/refusal and
cold-origin/missing-host recovery journeys in both profiles. Its sole projection
clones the existing inner Arc and moves the complete loan; it selects no new
operation. Logs: SHARED-{RECURSION,SCOPE}-{DEBUG,OPTIMIZED}.log in the scoped
application proof above. These remain private results, not Entity acceptance.

Before extending Entity execution, the unchanged real account journey adds a
real cold account (balance71) beside hydrated/passivated/reactivated balances40,2.
It fails solely with E0599: run_with_entities exists only for Members=().
Derived composition law: stage Members with existing ComposeApplication before
installing Families. Staging rejection returns original root, scope, uninstalled
families, Work and exact StagingFailure; prepared custody owns actual composed
Actor and the same shared scope. Root retirement uses RootOrigin<original Root>
and ActorRetirement<actual Actor>; independent family receiving and root-first
shutdown keep their existing equations. Reuse the existing execution kernel,
family installer and exact member Origins; add no engine, protocol or capability.
Expected private change: one existing execution module, roughly +20/-20 lines
plus existing associated actor proof substitutions; zero public types/states.
Focused proof: /private/tmp/bombay-scoped-application-p0fuke2_/ENTITY-{LAW.md,BASELINE.log}.
Complete rejection, cancellation, identity and family/root failure witnesses
remain required before retaining the combined model.
The private combined account journey, original staging rejection/recovery,
unpolled execution cancellation/recovery and root initialization refusal now
pass both profiles. Refusal retains the original root Box and unstarted child;
the installed zero-incarnation family shuts down independently. Restoring the
prior Entity method restriction fails solely with four E0599 diagnostics in
each build; removing Arc delegation independently fails six NativeChild E0277
diagnostics per build. Original source bytes are restored. Minimized library
Clippy passes; this is not the full workspace gate. Evidence remains in the
scoped proof's ENTITY-{STARTUP,REJECTION}-{DEBUG,OPTIMIZED}.log and
INVERSE-RESULTS.json. Acquired family-prefix cancellation and complete HTTP
runtime custody remain unexecuted on this composition.

Before the next graph comparison: existing timer/shutdown composition changes
the creator while concrete Base/Role still selects the same child. The unchanged
four-node/three-audit caller with a real root safety timer fails solely on the
missing decorated-creator NativeChild operation. Compare one operation generic
only in creator C for each concrete Base/Role: require its exact existing role,
event/report and send capability; keep concrete child Commit and all three
source-custody equations in the body. No arbitrary child generics or whole
Commit eligibility. Expected private edit: three scope implementations only,
zero runtime/type/state changes. Handwriting still names Trace: no automatic
report-payload extraction or public DSL is claimed. Root runner families need
their own application-selected owner; NativeChild cannot infer a leaf's root D.
Focused pre-edit law/source: scoped proof CREATOR-{LAW.md,BASELINE.log}.
The creator-generic Base-equality comparison is rejected: E0119 overlaps the
Root/Node factories and E0310 exposes the retained typed parent-report source
lifetime. Its three candidate implementations were removed; no bound was
invented to retain them. The corrected caller's baseline fails solely on
missing decorated-creator NativeChild. DEAD_ENDS.md records the falsifier.
Next pre-edit comparison: one application-scoped closed edge owner selects
source domain, concrete declared occurrence, exact child and its one target
retirement/failure family. A single operation on the concrete Scope delegates
to that edge; no universal blanket or global domain topology. Existing affine
PendingChild/native execution and exact ingress/source laws remain unchanged.
Same-type siblings, actual decorated occurrence resolution and stored-source
lifetime must be proven independently; metadata cannot assert execution.
Expected private comparison: one external consumer, one private edge contract
replacing three operation impls, roughly +60 lines, no runtime states or public
types. Pre-edit full checkpoint and corrected RED: scoped proof EDGE-LAW.md and
CREATOR-BASELINE.log. This is not a retained DSL or a caller-ceremony gain. The
current spawned interpreter retains LocalParentReports<Event, Child, C::Birth>;
its source tag is type qualification, not an owned Birth value. Restrict the
executed private edge's C::Birth to 'static for that existing native context,
without tightening pure authoring or substituting Base::Birth. This current
interpreter restriction is tested separately with lifetime-bearing pure birth
metadata and is not presented as an actor-model requirement.
Root-retirement inference, cancellation/failure custody,
HTTP/Entity combined composition, strict gates and wrapper/template proofs
remain separate required acceptance; field unification alone satisfies none.

Before actual Application retention, the same completed account/cold/failure
regression uses the actual Bombay feature branch, actual Core/Actors source and
Communication custody owner. Its cold Account has no births; remove the private
NativeChild implementation entirely. This independently proves the four-field
Application owner without retaining the unproven generated topology. Expected
actual production: application/mod.rs, execution.rs, http.rs and lib.rs only,
remove App, reuse exact kernel/Origins/family publication and existing launch
proof visibility; roughly net -560 lines. Focused witness/old-source inverse
must pass both profiles before mechanical App callers migrate. Existing counter,
heterogeneous, supervisor/FIFO, Entity and HTTP regressions remain independent
required gates. No new runtime type, native factory, state or protocol.
Evidence directory selected before editing:
/private/tmp/bombay-application-owner-i27iyp5f.

The actual four-file owner is now retained in the isolated Bombay diff. The
external consumer contains no NativeChild implementation. Both profiles pass
all four account/cold/failure journeys; byte-restoring the prior four owners
fails solely on five E0599 diagnostics in each profile. Proven supervisor and
FIFO callers pass before mechanical App migration. All seven example packages
compile/test through Application; the six terminating executables run. HTTP's
router and static protocol inversion tests pass. The complete actual execution
kernel, receiving and outcome suffix are token-identical to the prior source.
Public root borrowing and every rejection keep the same owned scope.

Checkpoint: ApplicationOutcome remains StagingRejected/Unstarted/Prepared/
NotInvoked/Completed/Interrupted (6→6); original Work custody5→5, bindings5→5,
startup2→2 and retirement24→24. Each surviving alternative owns current cold
inputs, staged actors, actual callable/result, task/cause or independent cleanup
needed by a later decision. No arrival history, duplicated cause, semantic
flag, cardinality assumption, nested transition authority or new structural
caller recovery is introduced. Production2720→2165 lines (+206/-761/net-555),
match expressions53→48 and match arms105→97; existing modules unchanged and
public method spellings23→17. Public types+0/-1 (App). Application's original
conditional Clone/Copy/equality are removed because it now owns an affine
ActorSpace. Members and Families coexist; family staging reuses exact Origins,
with no new generic bound beyond the existing selected-host contract.
Cross-checks: DX01/03/16/21/22, execution-ownership, Driver work/publication and
retirement laws, Entity root-first shutdown and normalized atomic source laws.
Disposition: pass for this independently bounded owner change, not generated
Application acceptance. Mechanical migration adds no semantic production.
Tests +1096/-194/net902 include four independent full custody journeys and
updated exact signature snapshots; examples+12/-11/net1. Source/default and
all-feature owner consumer checks are distinct from the registry contract.

Pinned source verification: workspace/all-target/all-feature compilation,
strict Clippy, formatting, two doctests, four focused owner cases plus fixed
supervisor/two FIFO cases in both profiles pass. Full all-feature nextest has
567 passed, one failed (the four unfinished generated-custody pass fixtures),
one skipped. Every existing compile denial remains active; only Application
signature text changed. Six terminating examples pass, including real Entity
reactivation; the HTTP example's deterministic package tests pass. Source
Communication emits a dependency deprecation warning under Rust1.99, while its
own pinned1.95 checks pass; no broad lint suppression or API workaround added.
The original registry lock is byte-restored. Exact laws, before/after inverse,
counts and commands: /private/tmp/bombay-application-owner-i27iyp5f/
{LAW.md,INVERSE-RESULTS.json,FINAL-COUNTS.json,BRANCH-COUNTS.json,
FINAL-VERIFICATION.json,WORKSPACE-DOCTEST.log,OWNING-TEMPLATES-OPTIMIZED.log}.
Fresh authoritative registry Nix exits1: formatting, panic-abort rejection,
Observe Loom and Driver-law evidence pass;17 remaining gates fail on unavailable
published Behavior APIs (including ChildOccurrenceMember and Unavailable).
Neither signing nor disk exhaustion occurred; no waiver applies. The immutable
run's REGISTRY-NIX.log and REGISTRY-NIX-RESULTS.json record all21 dispositions.
An independent post-run eval generated different unbuilt paths and is not used
as a substitute for that executable run. The registry lock remains byte-exact.


The bounded Application owner is committed as Bombay4529bc3. Current Behavior
also passes all ten pinned native Nix gates; this turn changes only its owning
PRD and rejected-model record, with no actual Core/Actors production edit.

Next native comparison, recorded before prototype edits: reuse the existing
finite InterpretSends and exact send-source custody against the actual child
host. Outer Terminal/Origin affect retirement projection only. Compare a public
ChildCommunications supertrait naming that operational contract, with the exact
associated custody equality, while keeping ApplicationCapabilities and its
namespace private. A blanket implementation contains no whole Commit/creation
guard. Concrete edge bodies retain all three complete source-custody equations.
It would delete caller-spelled report-payload ingress, without new metadata or a
surrogate interpreter. Expected private effects/reexport change: one public
trait, roughly80 lines, zero state/field/module changes. Foreign predicate
elaboration, two aliased report payloads plus timer, both orders, complete
rejection/prefix and missing ingress denials are required; a new unnameable
premise reopens. Full pre-edit law/checkpoint:
/private/tmp/bombay-scoped-application-p0fuke2_/COMMUNICATION-LAW.md.
The actual foreign-edge witness falsifies this public supertrait candidate:
three absent-port E0405 diagnostics become nine E0277 parent-report ingress
obligations at the edge's signature/return contract. Writing the already-selected
concrete Child/Descendants/Failures instead of Self projections still fails with
the same nine diagnostics in debug and optimization. A read-only reviewer
confirms this does not justify a generic Sends parameter or extra ingress bound.
The port and reexport are byte-restored; the two-report/timer extension was not
executed and no such success is claimed. Actual production/API/state deltas0;
private existing bindings/startup/retirement remain5/2/24, zero modules or event
alternatives. No custody, actor law or concrete host was changed. Disposition:
reopen; exact falsifier in DEAD_ENDS.md and scoped COMMUNICATION-RESULTS.json.

Before the next bounded configuration edit, actual Application::entity_family
and EntityRuntime::new still consume their affine inputs when raw DirectoryConfig
has an invalid shard count. Derived ownership law: validate configuration in
its owning value before transferring root, scope, children, family or runtime.
Local hash policy requires positive power-of-two shards and a u64-representable
mask; activation waiter capacity remains positive. DirectoryConfig gains private
immutable fields, new(shards, waiters) and read-only scalar accessors. Its own
DirectoryConfigError returns both original counts on invalid construction.
LocalDirectory::{new,with_hasher}, EntityRuntime::new and entity_family then
consume only a validated configuration and are infallible with respect to it.
Default remains valid. Do not merely move the loss into a result convenience
wrapper or add actor Debug/Clone bounds. Reuse actual shard/mask/table/slot and
native family/runtime composition, deleting duplicated validation, error
translation and impossible command-admission configuration branches.

Pre-edit record: /var/folders/3t/tds_sn397djg1g8d8c471g780000gn/T/
bombay-directory-configuration-rki61ud7/LAW.md. The current actual ownership
witness must fail because invalid configuration disposes original root/family;
the desired public constructor, complete scalar rejection and infallible
installation are separately red. Expected owning files: entity/directory.rs,
runtime.rs, family.rs, application/mod.rs and entity root export; one narrow
public configuration error, roughly net-neutral production, no module or
actor-state additions. Before/after checkpoint: config product2 fields remains2,
command DirectoryError3→2 plus configuration error1; SlotState Ready/Poisoned2,
dispatch Idle/Dispatching2 and all actor EntitySlot, startup, retirement and
source-custody alternatives unchanged. Every surviving command error owns its
original command; configuration error owns both rejected scalar inputs.
No arrival history, repeated cause, new cardinality, semantic flag, erasure,
second transition authority or structural caller parameter. Cross-checks:
actual directory hash/identity/retention, Entity admission, root-first family
retirement and DX16/17/18/21/22. Focused original failure/inverse and both
profiles precede mechanical migration; full consumer/gates and exact counts
remain mandatory. Retained result: both original-source ownership runs dispose
root/scope/family/cold child (1/1/1/0 instead of0/0/0/1); both constructor and
application syntax witnesses independently reject only missing owning API. The
inverse restores the original five owners, reproduces each defect in both
profiles and byte-restores the candidate. Candidate external configuration and
all four complete actual Application/Entity journeys pass both profiles.

Four owning configuration tests include390 independent table cases and complete
rejected counts, corrected waiter retention, valid Default and two active
forged/mutated-construction denials. Focused44 tests pass debug and optimization;
full source workspace check(all targets/features), strict Clippy, formatting,
two doctests and the migrated Entity binary pass. Full source nextest:572 run,
571 pass, one unfinished generated-custody fixture fails, one skipped. Six real
Entity directory Loom tests pass both preemption3 and the repository's unbounded
settings (1.91s model runtime). Fresh registry Nix completes in347.92s: four gates
pass, seventeen fail on missing published owner API, no disk/signing failure.
Cargo.lock is byte-identical to the committed registry lock. Exact commands and
results remain in CURRENT-RESULTS.json, SOURCE-RESULTS.json, LOOM-REPOSITORY.log,
INVERSE-RESULTS.json and REGISTRY-NIX-RESULTS.json in the evidence directory.

Measured production+89/-80/net+9 across five existing owning files; tests/benches
+199/-138/net+61 across16 paths, Entity example+6/-8/net-2. New production owns
validated construction and inspection; it is not a code-reduction claim. Types
+1/-0, existing public functions55→58, match expressions29→28/arms71→67 across
those owners; no modules. Complete aggregate state remains EntitySlot
Inactive/Activating/Active/Draining/Retiring5, SlotState Ready/Poisoned2 and dispatch
Idle/Dispatching2; native binding5/startup2/retirement24 unchanged. Both retained
configuration fields are current limits required by hashing/waiter admission.
The one configuration rejection retains both input limits; each of the two
command errors retains its original command. No new transition authority,
arrival history, repeated cause, cardinality assumption, semantic boolean,
erasure or structural caller syntax. The original directory hash/table/native
slot equations and kernel are unchanged. Cross-checked the documents/laws named
above and the normalized Entity/creation/custody contracts. Provenance for every
changed signature/export points to the pre-edit disposal and constructor
regressions; caller migration adds no policy, generic or bound. Checkpoint: pass
for this bounded law; whole-programme acceptance and registry delivery stay open.

Before the next private topology comparison, use the approved application-scoped
closed graph to name actual executed actor stacks, including decoration. Derived
law: each exact creator and authoritative occurrence selects one concrete child,
its complete descendant retirement and direct failure product. A changed stack
requires the one declaration to change; no universal decorator theorem or
application-global topology is claimed. Compare direct concrete NativeChild
operations with the private generic-C ChildEdge selection, deleting that trait
and its explicit report/lifetime predicates. Keep original PendingChild execution,
actual parent Event/Birth, complete Commit/source equalities inside the operation,
and generic terminal presentation only. A repeated child at a different position
is a distinct edge; identical normalized operation keys share one operation.
Pre-edit witness: the existing real recursive reports/refusal consumer without
ChildEdge must fail solely for missing native edge capabilities. Then prove two
aliased, distinct owned reports and meaningful child timer decoration in both
shutdown orders, including missing-ingress denial and accepted-prefix custody.
Scope before/after: binding5, startup2, retirement24, no subordinate state or
transition/module additions; actual production/API0. Only the private foreign
consumer changes, replacing handwritten generic selection with the finite exact
expansion. Finite selection deletes the ChildEdge trait and41 caller lines;
its missing-operation inverse fails solely on six E0277s in both profiles.
Two actual facade-generated named report lanes, aliased by the creator, pass
complete recursion/refusal with meaningful child timing in both shutdown orders.
Deleting either ingress produces eight independent denials, each solely on the
missing EventIngress capability. Closed-parent recovery keeps both original
reports, the readiness allocation and the accepted timer; replay preserves them,
and the original elapsed event stops the child. Restoring map-before-admission
reproduces the old closed-parent panic in both profiles and byte-restores the
owner. Refreshed six journey/custody runs pass against actual Behavior d9472a0
source in debug/optimization. Evidence: CURRENT-REPORT-RESULTS.json,
REPORT-DENIAL-RESULTS.json and REPORT-CUSTODY-INVERSE-RESULTS.json under
/private/tmp/bombay-scoped-application-p0fuke2_. The whole private lib-test
lowering still fails on62 unmigrated native fixtures; it is not an owner-gate pass.
These focused proofs add no state, authority, inferred provenance, repeated cause,
semantic flag, erasure or structural ordinary syntax. The eight ingress denials
and complete custody cross-check typed creation, initialization, wrapper and
retirement laws. Checkpoint: pass for finite operations and both report lanes;
native production retention, generator lowering, protocol sharing, whole birth-row
coverage, source lifetimes and catalogue migration remain open. The private
SendLayer/concrete SendProduct fixture setups were rejected before lowering;
neither their compiler errors nor an extra generic justifies a production API.

Before deriving recursive custody, test whether the existing owning child-failure
product can be named by actual actor type outside Bombay. Derived law: the direct
failures of an actor are exactly its authoritative Birth occurrence product,
with original child, origin and rejection; declaration order must not create a
second failure schema. Desired caller syntax is `ChildFailures<ManagedNode>`
inside the generated retirement frame, replacing the handwritten nested tuple.
Ownership remains the existing DirectChildFailures/ChildStartupFailures equation;
no new shape, dispatch trait, failure variant, actor bound or visibility widening
of its machinery is justified. Write the foreign alias regression first, expecting
only the missing public domain product; test a two-occurrence recursive creator
and a decorated zero-birth actor in both profiles. If the private shape prevents
lawful public use, reopen this candidate rather than exposing it to silence Rust.

This is a private experiment, not a retained export or topology API. Expected
private owner delta: two existing export lines, one public semantic spelling,
no states/modules/transitions. Binding5/startup2/retirement24 and every existing
failure alternative stay unchanged; the product owns current rejected child
requests, not arrival history. Reuse the complete interpreter and foreign report
journeys, including the original initialization refusal, both child wrapper
orders and accepted timer/report rejection prefix. Actual production/API0.
Cross-check authoritative occurrences, creation provenance, source custody and
DX01–03/16/18/22. Both baseline profiles fail solely on two missing ChildFailures
types. The private two-line export candidate then fails on17 private-shape
diagnostics. All consumer/owner bytes are restored; no export is retained.
Disposition: reopen. Evidence: FAILURE-PRODUCT-{RESULTS,RESTORED}.json in the
same private evidence directory; the falsifier is recorded in DEAD_ENDS.md.

Next private model, before any lowering: complete descendant custody selects a
closed direct-child failure sum; the runtime retains `Vec<Failure>` in its original
retirement traversal order. Each variant embeds one complete existing ChildFailure
for its actual creator/occurrence/child. This is a derived ownership construction,
not an actor-model ordering guarantee. Empty descendants select Never, so an empty
failure collection cannot contain a forged failure. A RetirementSchema interpreter
port associates that failure sum with the descendant family; it owns static
custody selection, not state transitions. Reuse the existing six failure variants,
total owned From inclusion, ProjectTerminal, NativeChild/PendingChild and the
unchanged retirement traversal. Delete DirectChildFailures/ChildStartupFailures
and their nested tuple schema rather than copying that schema into generation.
Native creation derives its failure collection from the selected descendant
schema, removing its independently selectable failure parameter. ProjectTerminal
likewise derives that collection from its associated descendant schema, so a
presentation cannot independently select a contradictory child failure family.
Expected owners additionally include the existing projected task and mechanical
projection derive. The generated ordinary From inclusions must be bare total
enum constructors, preserving the original head-then-tail traversal and outside
destination. Manual schema implementations are an explicit advanced ownership
port requiring conserving, non-unwinding inclusion; arbitrary conversion panic
has no custody guarantee and a catch cannot reconstruct a consumed failure.
Advanced empty presentations remain a separate veto: do not add a no-op schema,
unit implementation or default merely to satisfy a new bound.

Desired foreign witness uses named Next/Audit failures and the same complete
recursive two-report/refusal journeys, with no physical tuple order. Write it
before owner edits; require missing-schema/static-custody RED, then both profiles,
both wrappers, independent real task failure, initialization rejection and
cancellation. Generated callers will name only domain roles in one declaration.
Expected private owners: terminal schema, children, native creation, root execution
and root export; one public interpreter port, roughly net-neutral production,
no modules/control alternatives. Binding5/startup2/retirement24 and six failure
causes remain unchanged; faults are current owned evidence, not arrival labels.
Before/after counts, exact source equivalence and complete custody/composition
checkpoint are required before retention. Two real templates precede any broad
migration. Actual production/API0; disposition remains unretained.

Before runner-inference lowering, a selected application topology associates its
actual executed actor with one complete retirement schema. This is derived static
composition, not a second application owner or runtime choice. Desired syntax:
`Application::new(counter.stop_on_shutdown()).run_with(work)` and
`Application::with_topology(root, selected_topology).run_with(work)`, without
terminal/type holes. ApplicationTopology<Actor> is the explicit interpreter
metadata port for that selection; ActorSpace's builtin empty case requires the
actual Birth child to be Never and selects Never. No default generic, fallback
schema or unused policy. The declared graph implements the same port for its
exact root stacks. Reuse ComposeApplication's actual actor, RootInterpreter,
RetirementSchema, complete original work/root/scope returns and the identical
execution kernel. Remove run_with's independent Terminal generic, deriving it
from the selected scope. Other runner/HTTP/Entity methods remain unretained
follow-up until the owning port is proven.

Write counter and heterogeneous recursive missing-inference RED before edits;
then prove both report orders, initialization refusal and original cleanup.
Expected private owners: topology port, run_with and root export; one public
port, roughly +20 production lines, no runtime state, modules or transitions.
Binding5/startup2/retirement24 and every failure cause remain unchanged. The
port owns the selected static schema; all affine current inputs and failures
stay with their existing owners. Cross-check DX01–03/16, actual composition and
native schema law. Exact counts/provenance and empty-source denials are required
before retention; actual production/API0, disposition unretained.

Runner isolation uses an exact archive of Bombay fb7e89a, preserving its existing
native creation and failure tuples. The same inferred counter, real supervisor
and real FIFO journeys pass all six debug/optimized runs after changing only
topology metadata, run_with and its export. Counter baseline is solely E0283;
both template baselines are solely missing ApplicationTopology E0405 (the initial
pool fixture omitted its existing worker module and was corrected before the
intended RED). Evidence: /private/tmp/bombay-runner-topology-6cfbir4v.
This proves runner selection independently of the private failure-sum candidate.
Retention still requires original-value rejection, static denials and whole-source
verification. Cold-member callers reveal a further design obligation: their actual
ApplicationBehavior must be selected by the same declaration; do not invent a
new scope wrapper during mechanical migration. Actual production remains0.

Before private declaration lowering, compare ordinary Rust recursive aliases and
handwritten finite nominal custody. Functions, builders, extension traits and
associated selection solve value construction/inference but cannot declare a new
finite nominal recursive sum; the exact aliases must independently fail E0391.
The retained private handwritten exact graph already proves that the required
nominal types and native bodies work without erasure or whole-Commit guards.
Only that syntax derivation gap makes a function-like topology macro eligible.
It must lower to those exact owners, not define another actor/application algebra.

Desired single declaration names an application module, its actual actor classes,
child roles/targets and required logical protocol hosts. Root may list the two
already-proven executed decorator stacks sharing its semantic base; ordinary
classes name one exact executed type. A library's genuine single child-type edge
uses its authoritative head occurrence without a caller marker. Multiple edges
use existing semantic ChildRole types. The declaration derives local spaces,
finite native operations, actor-indexed nominal descendants, closed failure sums,
bare conserving From inclusions and ApplicationTopology selection. Zero births
derive Never; one failure lane uses the existing ChildFailure directly; several
lanes derive one exhaustive role sum. No public failure-shape projection or tuple
order, payload parsing, runtime registry, defaults, no-op policy or effect lane.

Validate real BirthNodeAt/ChildPosition relations and complete executable rows;
reject duplicate roles/operation keys, missing targets and unreachable classes.
Protocol sharing belongs only to required logical hosting; per-occurrence child
registration stays with the runtime. Preserve the complete existing initialization,
retirement, rejection, provenance and cancellation paths. Write recursive alias
denials and the desired declaration counter/heterogeneous regression before
macro code, then both wrapper/report orders, failure-prefix inverse, native
authority denials and actual supervisor/pool journeys before retention/migration.
Cold members remain an explicit unproven extension of this same declaration,
not a competing composition spelling.

Expected private owner: one existing macro entry/export and one parsing/expansion
module, no dependencies, roughly +250 production lines for derivation; no runtime
state/transition changes. Syntax products own declaration, actor class, role edge
and logical host respectively. Generated types own concrete namespace or exact
custody products/sums; no additional transition authority. Before/after counts,
hygiene/renaming/expansion and forbidden-erasure checks remain mandatory.
Cross-check actual birth metadata, typed host obligations, all six failure causes,
original native traversal and DX01–03/08/16/18/22. Actual production/API0;
disposition unretained pending executable comparison and all selected journeys.

Private declaration now passes the counter and both heterogeneous report/wrapper
orders in six debug/optimized whole journeys. Original recursive aliases deny
solely E0391; missing generation denies solely the missing macro/module. The
wrong Branch child denies already at the owning ChildRole::Position contract
in both profiles; it is not an accepted authority forgery. A missing declared
occurrence is accepted and needs classification against the existing resolver
law before adding a requirement. Execution still checks the complete actual
birth row; scope construction owns spaces and does not certify execution.
Evidence: /private/tmp/bombay-retirement-schema-kgazqjhv/DECLARATION-*-RESULTS.json.

Before topology visibility lowering: explicit declaration visibility promises
caller access to its actual owning scope and complete typed custody. This is a
derived Rust interface law, not lifecycle policy. Desired foreign caller:
`owner::counter_application::Topology::new()` followed by inferred
Application::with_topology(...).run_with(work); a pub(super) declaration must
also work from its original parent. Actual public Counter/protocol/error/send
products are already public; no actor capability is exposed by this fix.
The full public counter journey fails only E0603/E0624 in both profiles; a
sibling-module journey independently fails those codes. Existing default private
journeys retain parent-only access. Reuse syn Visibility, generated scope and
constructor, existing public ApplicationTopology port and typed custody; delete
hard-coded pub(super) on explicitly exported items. Translate relative restricted
visibility across the single generated module. Private caller-scope aliases,
registration fields, actor types and effects retain their original visibility.
Expected one macro owner, about +18/-6 lines, zero new types, runtime states,
transitions or bounds. Binding5/startup2/retirement24/native failure6 unchanged;
every surviving value remains original scope or exact retirement. Residue scan:
no history, duplicate cause, cardinality assumption, nested authority, bool or
structural caller path. Cross-check declared hosting, Entity Hosts ownership,
Rust caller reach, DX01–03/17/19/22. Private disposition unretained pending
foreign/sibling complete journeys, privacy denials, inverse and full counts.
Evidence: /private/tmp/bombay-retirement-schema-kgazqjhv/PUBLIC-SCOPE-*-RESULTS.json.

Private visibility checkpoint: production+26/-8/net18 in one macro module,
zero public types/spellings added or removed; one private caller_visibility
method owns declaration reach across the generated module. Four syntax-only
visibility alternatives replace hard-coded reach; actor transition branches
remain unchanged. Foreign and sibling counter journeys pass both profiles,
six privacy denials preserve E0603/E0624 or E0616. Restoring the original macro
fails only the recorded privacy codes in both profiles; source byte-restored.
Binding5/startup2/retirement24/failure6 and original owning values unchanged;
no history, duplicate cause, cardinality assumption, nested authority, semantic
bool or structural user syntax. Normalized creation/custody and interface laws
cross-checked. Disposition pass for this bounded private interface correction;
whole topology remains unretained pending the remaining checklist obligations.

Before class-name lowering: deliberate DSL syntax policy requires unique
Rust custody spellings for actor classes, and derived syntax must preserve
valid raw Rust identifiers. Root labels remain the declaration's existing
roles/classes, not runtime identity. The real recursive graph with root_node
and root__node currently fails only E0428 in both profiles, generating duplicate
RootNodeRetirements. The same complete graph with r#type panics while generating
R#typeRetirements; missing-module diagnostics are fallout from that macro panic.
Desired rejection names both colliding classes before expansion; desired valid
raw caller consumes garden::TypeRetirements through the original complete
recursive report/refusal/retirement journey. Reuse actor_name, existing class
validation and syn::Error; remove raw prefix before conventional Rust casing
and validate derived spelling uniqueness. No suffix ordinals, compatibility
aliases, new runtime modules, states, bounds or public types. Expected one macro
owner, about +9 production lines, one parser rejection branch. Binding5/startup2/
retirement24/failure6 unchanged; class declarations retain their exact executed
actor and roles, no new subordinate alternative. Residue scan and normalized
creation/custody laws show no history, repeated cause, cardinality policy,
nested authority, bool or structural caller syntax. Private unretained until
focused denial/raw journeys, inverse, composed witnesses and exact counts pass.
Evidence: /private/tmp/bombay-retirement-schema-kgazqjhv/{CLASS-COLLISION,RAW-CLASS}-*.

Private class-name checkpoint: macro production+8/-1/net7, rustdoc+17,
zero new public types, modules or spellings; one early syntax rejection branch,
zero actor transition branches. Original collision fails only E0428; corrected
syntax reports both labels with no unrelated Rust diagnostics, in both profiles.
Raw-class full recursive journeys pass both profiles; original macro panic is
independently restored and detected, then both sources byte-restored. Existing
literal duplicate-class diagnostic retains priority. Binding5/startup2/
retirement24/failure6 remain unchanged; original actor/role/custody values own
all decisions. Normalized creation/provenance, interface and macro hygiene laws
cross-checked; residue scan finds no history, repeated cause, false cardinality,
nested authority, semantic bool or structural user syntax. Disposition pass for
this bounded private syntax correction; no full-programme completion claimed.

Before hygiene lowering: supplied actor, role and logical protocol types must
resolve in the invocation's Rust scope, including nested generic arguments and
relative paths. This is syntax preservation, not actor semantics. A nested
counter declaration using super::counter::Counter fails solely E0433 in both
profiles on the candidate (the initial fixture used an incorrect library path,
corrected before this RED). Bind input types with generated private aliases in
the original scope, then reference those exact aliases from the generated
module. Remove its glob import. Reuse ordinary Rust aliases, syn/quote and the
same selected semantic types; no extra module, wrapper, dependency, state or
public interpreter port. Aliases own source-scope resolution, not runtime state.
Expected one macro owner, about +35 lines; measure complete generated aliases
and rerun relative paths, name collisions, crate renaming and all journeys.
Actual production remains0; disposition unretained until these witnesses pass.

Real generated supervisor and FIFO journeys each fail solely E0119 in both
profiles: the generated Hosts<Root::Protocol> overlaps their genuine worker and
reply hosts. Reopen automatic logical-root hosting. Registration and logical
delivery are distinct responsibilities: the root needs an exact registration
space; an established recipient uses its original capability and does not need
logical resolution. No actor-model law grants logical root delivery implicitly.
Desired unchanged caller is the single topology plus inferred run_with; declare
only genuine logical destinations. ApplicationTopology will own borrowing the
selected root registration space as well as selecting descendants. Reuse its
actual actor/protocol, ActorSpace, existing Arc delegation and the identical
launch/receiving kernel. Remove automatic generated Hosts<Root::Protocol> and
replace only root-registration lookup with the topology operation. Explicit
logical bindings remain Hosts; no effects, registry, namespace or erased path is
added. A genuine logical-root binding must share the original registered space,
not allocate a conflicting second namespace. Other interpreter paths require
the same audit and focused witnesses before retention.

Before lowering, existing real supervisor/pool desired consumers are RED solely
E0119. Prove both complete journeys, replacement provenance, missing genuine
logical binding, original refusal/cancellation and both report orders after the
change. Expected private topology/execution/HTTP/macro owners, one method on the
existing topology port, no new public type, about +15/-10 production lines.
All actor/retirement/failure sums and kernel ordering remain unchanged; the
surviving values are the original registered space and complete child custody.
Cross-check DX01–03/16/20, scoped hosting and Driver initialization/retirement.
Private candidate remains unretained until original-value, static and composition
checks pass. The directory configuration source in this private tree is refreshed
from actual fb7e89a; source-version drift is not evidence for current acceptance.

The generated real supervisor and FIFO now pass all four debug/optimized
journeys, including replacement and both retained worker incarnations. No root
protocol annotation or additional logical root host is required. This bounded
result does not certify all kernel paths or advanced empty presentations.

Before cold-member lowering: one ordered members section describes the existing
.child call order; actual values still transfer through those ordinary calls.
The existing owner alone stages them and selects ApplicationOrigins. Generate
the exact reversed ChildCons product and concrete ApplicationBehavior privately.
Original births retain ChildOrigin<Root::Base,Position>; appended members retain
ChildOrigin<original decorated Root,declared Role>. Never synthesize an origin
offset or a second sequence. One native operation per target class fixes the
actual composed creator and descendant family and varies only the sealed actual
ChildPosition. Its head/tail substitutions are real born/cold occurrences; its
complete child Commit proof remains body-local. Reuse BirthNodeAppend and the
existing complete-row execution/initialization/retirement ordering.

Desired witness has one genuine same-action born worker and two cold workers of
the same type under distinct roles, with three original non-Clone boxes and
fresh actual bindings returned on root retirement. Its first fixture mistakenly
used the multi-child Children representation for a single named birth; after
source correction, the pre-edit RED must be solely unsupported members syntax.
Then prove omitted/extra/reordered member denials, full failure/cancellation and
exact accepted-send custody in both profiles. Select metadata only for the actual
executed composed root; do not require unused uncomposed-root metadata on the
outer Application impl. No new runtime state, port, capability or public alias.
Cold members currently require one concrete executed root because their exact
origins include that decorated type; do not merge distinct root incarnations.
Expected private macro/composition-call-site ownership, about +100 derivation
lines and focused caller code; root5/startup2/retirement24/failure6 unchanged.
Cross-check DX01–03/12/16 and the actual ApplicationOrigins translation. Actual
production remains0; disposition unretained pending whole witnessed custody.

Cold whole journey passes both builds: three original boxes, distinct typed
role origins and fresh bindings, with the same-action send processed or returned
exactly once. Independent swapped/duplicated-role compile probes are wrongly
accepted in both profiles. ApplicationBehavior's ChildCons product drops role
types; Application retains them in Members and ApplicationOrigins. Thus the
actor-only selector cannot certify the declared cold slots. Reopen that selector.

Before lowering its replacement: ApplicationTopology<Root,Members> selects the
schema from the exact original root and existing ordered owned role/actor
product. Empty Members=() is actual absence, not a default/placeholder. Generate
the exact member tuple alongside the existing concrete composed native creator;
never make the creator generic. Separate RootRegistration<Protocol>, which
borrows the original physical space, from both this slot selection and logical
Hosts. Those three contracts change independently and have different laws.
ActorSpace, Arc and generated topologies reuse their exact existing spaces;
advanced ActorSpaces derives the same prior root-registration capability for
its explicit protocol fields. No extra allocation, wrapper, namespace, policy,
runtime flag or ownership clone. Kernel registration changes only its lookup
port; its actual source/origin and receiving machinery remain unchanged.

The preceding four wrongly accepted probes are the pre-edit falsifier. Prove
their exact role denials, the original born/cold journey, both template journeys,
root namespace identity, refusal and cancellation in both builds. A named input
constructor alone cannot prove this law: recovered scopes can enter existing
Application::with_topology again. Expected private topology/execution/HTTP and
macro owners, one additional public interpreter port, about +20 lines; no state,
failure alternative or transition authority change. Cross-check actual Members
ownership/Origins translation, DX01–03/16/20 and complete declaration coverage.
Disposition unretained until all checkpoints pass.

Before presentation lowering: TerminalProjection is a total owned embedding,
not a native execution policy. `(origin, retirement) -> presentation` preserves
the actor, descendant schema and every failure without creating a new schema.
The genuine counter/leaf presentation fails only E0277 for an unused schema in
both profiles (PRESENTATION-BASELINE). Reuse each existing From expansion and
ChildRole authority check; delete the generic native projection emitted by the
derive. The declared-child actor check must preserve its actual descendant
schema rather than replace it with Self. Native projection remains the explicit
interpreter port or generated topology operation. Expected one macro owner,
about -45 production lines, no new type/trait/module, states5/2/24/failures6
unchanged. Prove both actual owned retirements, wrong-actor/role denial, both
recursive wrapper orders and independent restoration before retention. This
is a derived ownership law; checkpoint remains unretained.

Before private binding correction: distinct topology declarations and distinct
role paths must remain distinct Rust bindings. Four pre-edit probes fail E0428:
blue_sky/blue__sky and Root.NodeAudit/RootNode.Audit normalize to identical names.
Keep source-scope aliases so relative types retain caller meaning, but name each
private binding by the original topology identifier and one declaration-local
ordinal. This ordinal only names generated Rust items; it is not a child route,
authority, nonce or runtime selection. No public spelling, state, module or
protocol changes. Expected one macro method, under +10 lines; reuse the existing
private aliases and delete their lossy concatenation. Prove four full journeys,
relative paths, both wrapper orders and restored original collisions before
retention. Source metadata and custody laws remain unchanged. Checkpoint pending.

Private focused presentation/hygiene results: actual counter and leaf retirement
inclusion passes both profiles; wrong actor is denied only E0308 after correcting
an omitted genuine timer function in the fixture. Original presentation restores
the schema failure in both builds and is byte-restored. Two lossy-name cases now
run four complete journeys; restoring their old bindings reproduces E0428 in all
four builds and is byte-restored. Presentation production +1/-41/net-40, no
state/type/module additions; binding naming changes four existing aliases only.
The independent reviewer found no retained decorator that changes a valid birth
head while retaining Base: all wrappers preserve Birth and Application append
preserves its prefix. The hypothetical changed-head case is withdrawn as a
blocker; no extra role enrollment or equality bound is added without a real
consumer falsifier. Full retained-batch counts and remaining custody/hosting
proofs are still required. Logs: PRESENTATION-*, ALIASES-* and SELECTED-* under
/private/tmp/bombay-retirement-schema-kgazqjhv.

Before logical-root binding: an explicit hosts.root declaration must borrow
that application's existing physical root space; it must not allocate another
namespace or add implicit Hosts projections that conflict for opaque protocols.
The genuine post-publication self-relay actor returns the original non-Clone
message through ordinary logical Delivery. Its existing ActorSpace journey
passes both builds; the identical topology declaration fails only the reserved
root-field diagnostic (ROOT-*-COMPLETE-BASELINE). Reuse the existing root field,
RootRegistration, Hosts and delivery kernel. Omit storage/construction for that
one host declaration and generate its ordinary explicit Hosts implementation.
No new public type, state, allocation or runtime branch; expected one macro
owner, under +10 lines. Prove namespace identity, original allocation and complete
root retirement, missing/mismatched protocol denials, both templates and restored
separate-namespace failure. Initial self-send before publication was an invalid
fixture expectation under the existing registration policy; no initialization
or publication ordering is changed. Classification: derived static binding;
checkpoint remains unretained.

Current private checkpoints: ROOT-DENIAL proves missing Hosts only E0277 and
wrong protocol only E0308 in both builds. ROOT-DELIVERY returns the original box,
normal actor retirement and the same physical/logical namespace. Independent
separate-space mutation fails both builds and restores exact production bytes;
all14 selected journeys then pass. Generation changes only three existing
expressions (root-name validation and two storage/constructor filters), no new
type/state/branch. The two template consumers delete unnecessary worker hosts
and pass all four journeys. Failure-custody probe now uses generated NodeChildFailure
with current normal modules: both accepted failures survive receiving cancellation
and replay; independent prefix deletion fails both profiles and restores bytes.
Eight renamed-dependency journeys pass with the same locked dependency graph.
First renamed attempt had an overbroad fixture rewrite, then disk exhaustion;
neither counts as a design result. Sources/evidence are retained; only obsolete
DevX inferred/creation-law build caches were removed. Public scope reuse and
class-name collision audit, complete counts, costs and owner gates remain open.
Evidence: GENERATED-FAILURE-*, FAILURE-CUSTODY-INVERSE-*, ROOT-*,
TEMPLATES-MINIMAL-HOSTS-*, RENAMED-* and SELECTED-ROOT-LOGICAL-HOST-* under the
same private evidence directory. Actual Bombay production remains unchanged.

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

Communication delivery: user authorized PR11 and its normal release on
2026-10-11. Exact af3e3f1 passes CI and merges02ee4c5. Official PR12 head5c2537d
changes only version/lock/changelog; its source diff against af3e3f1 is empty.
Seven fresh native gates pass,88 tests/zero skips,21-file package verification
and all five GitHub checks. PR12 merges24ad2b9; normal Release38161501036
succeeds. Registry0.1.5 is present/not yanked, checksum
`aa933d28c4af9a94c18494667eb0385aa88de2927d1ea78904453780afa5965d`,
with published tag/release. Source/logs:
/private/tmp/bombay-devx-communication-release-12.
The fresh Bombay0.24 check isolates sole E0599 for send_map absent from
Communication0.1.3; actual published0.1.4 inspection confirms it also lacks that
operation. Before pin edit, select published0.1.5 in the existing workspace
manifest/lock to realize the retained complete-report law. Zero Rust/types,
states or branches change. Do not claim the unfinished topology fixtures pass.

Registry adoption checkpoint (pre-edit): published Core/Actors0.24.0 and
macros0.14.1 are verified from release run38159701650, registry checksums and
annotated tags resolving049c88c. Deliberate delivery policy: both Bombay Cargo
workspaces consume those actual registry releases, with no owner source patch.
The prior exact fb7e89a registry build fails17 gates on missing owner contracts;
that retained failing downstream witness precedes this dependency edit. Change
only workspace and Driver-fuzz version declarations, their locks and coordination:
four dependency files, zero Rust production/test lines, zero public types or
control states. Existing runtime interpretation and retirement remain unchanged.
Reuse published packages; delete the stale0.23 selections. Fresh pinned checks
must report any remaining empty-alias, Communication or generated-custody gap
separately. No broad Cargo update and no publication of PR91/Communication11 is
authorized by this adoption. Provenance/composition/aggregate disposition: pass
for dependency selection; full downstream acceptance remains open.

Registry adoption results at Bombay e4d2db1: locked workspace/all-target/
all-feature compilation, formatting and Driver-fuzz compilation pass. Engine
tests pass74 cases plus one doctest in each profile. Fresh full native
`nix flake check --show-trace --keep-going --max-jobs 1 --cores 2` exits1:
17 gates pass; child/all-features, child/no-default-features, owner coverage
and workspace tests fail on the same four unfinished topology/custody
compile-pass fixtures. Workspace all-feature nextest runs572 tests:
571 pass, one fails, one is skipped (97.520s). The sole failed test is
`application_children_are_checked_from_caller_syntax`: four of six fixtures
fail on undeclared retirement-view syntax, inference or recursive proof.
Those proposed spellings are not architecture authority; complete custody
and static denial remain the acceptance laws. The skipped descriptor-limited
child test runs successfully through its passing parent regression, which
checks the child reports one pass and zero failures. There is no disk or
Darwin-signing failure and no waiver. Terminal successful-check records and
explicit failed derivations determine17/four, rather than post-run store-path
presence. Exact logs/results:
/private/tmp/bombay-devx-registry-delivered-{full-nix.log,full-nix-results.json,nextest.log}.
Dependency production/tests/API delta remains zero; the five-file adoption
commit is e4d2db1. Full acceptance remains open until the generated interface
is proven and retained against these published contracts.

### Empty-birth alias law and verification

Child logical-registration checkpoint (before private production edits): derived
namespace coherence requires a published child selected into a declared logical
family to resolve there at its actual allocated address. Selecting that family
is deliberate application topology policy, not an actor-model identity law.
Desired declaration: `worker: ProxyWorker in workers {}` with the existing
`hosts { workers: Worker; }` protocol host; exact-only classes need no host.
The initial focused witness uses the existing declaration without the new
binding syntax: an actual FixedSupervisor capability forwards an affine echo to
its actual worker. Receiving that worker's user reply proves publication and
supplies the address, without predicting allocation or treating initialization
reports as readiness. The declared host must resolve that exact address. Then
stop/replacement repeats the journey, retaining explicit incarnation provenance
and complete two-worker retirement. Require this runtime RED in both profiles
before adding class-selection syntax or changing construction.

Ownership equation: one application allocator establishes fresh addresses;
one explicitly selected Address family supplies reservation/publication/lease
custody; each creator/occurrence retains its own correlations, creation order,
controls, tasks, raw retirement and projected reports. Sharing the selected
Address family never merges occurrence rows or changes commitment timing.
Inspect root setup, nested creation and Entity setup before edits. Reuse the
existing NativeChild interpreter port, ActorSpace, Arc delegation and closed
ChildBinding product; replace scope-free Default construction rather than
allocate and overwrite unused spaces. No additional public selector, metadata
port or recursive whole-execution eligibility guard. Hosted native rows clone
the selected family; exact-only rows allocate their independent family once.

Expected private delta: existing native port, child bindings, three constructor
owners and declaration derivation; roughly +40/-20 production lines, zero new
public types/modules or runtime state alternatives. Binding states remain
Starting/RetiringStartup/Established/StartupRejected/Rejected (5), startup raw/
projection custody (2), retirement outcomes (24), six native failure causes;
all surviving values retain their current owned request, task, report or cause.
Before retention measure all symbols/branches/lines and test repeated same-type
roles, exact-only children, wrong/ambiguous hosts, invisible reservations,
initialization refusal, lease retirement and pending/raw/projected cancellation.
Cross-check DX02/03/16/21/22, runtime capability ownership, Driver ordering,
execution-ownership and normalized supervisor/pool laws. No production edit
exists yet; acceptance and aggregate disposition remain unretained pending RED
and the complete composition/custody proof.

Focused RED is now executable: private `hosted-supervision` runs the actual
fixed supervisor through affine echo, stop, fresh replacement echo and complete
native retirement. Both profiles fail only the post-publication first-worker
logical lookup assertion; both original payload allocations/bytes return,
replacement address differs and the old lease is absent. Logs:
/private/tmp/bombay-retirement-schema-kgazqjhv/HOSTED-SUPERVISION-BASELINE-{DEBUG,OPTIMIZED}.log.
Before lowering, the construction seam is named explicitly:
`NativeChild::registration(&self)` supplies the single selected ActorSpace;
private `ChildBindingsIn<Creator, Scope>::new_in(&Scope)` constructs the closed
empty/nonempty occurrence product. Two implementations own absence versus a
real occurrence row; its tail recurses only over that finite product. It has
no associated metadata, execution eligibility or additional transition state.
Delete both scope-free Default implementations and replace their three owning
production construction paths and existing corresponding signature bounds.
Declaration syntax adds an optional host identifier on an actual actor class;
its generated native method clones that exact field or allocates the existing
independent space for an exact-only class. Host protocol equality stays static
through the return type. Expected delta is roughly +60/-25 production lines,
one private construction trait/method and one method on the existing public
native interpreter port; zero new public types, actor states or modules.

First lowering is vetoed before consumer execution: it names bare Spaces in
the public constructor proof, but the existing prepared root publication owns
Arc<Spaces>. The two generic product predicates are not interchangeable.
Reopen the construction signature against the original ownership equation:
root entry points select construction in their existing Arc<Spaces>, matching
their actual RootInterpreter and prepared publication; the generic execution
kernel selects its own actual Spaces type. Entity already constructs in its
existing Arc<N>; nested creation borrows its actual N from Arc<N>. This replaces
the incorrect predicate rather than adding a second proof or an Arc adapter
trait. Existing NativeChild Arc delegation remains the registration identity
owner. No new semantic surface or caller annotation is justified by this veto.
The blanket root-entry substitution was also incorrect: only execute_on and
run_on prepare Arc<Spaces>; caller-local, background and HTTP paths prepare
bare Spaces. Their existing RootInterpreter signatures already state the exact
scope each constructs. Align each construction predicate with that existing
signature, without changing any scope representation or retaining both bounds.
This is a source-verified ownership correction, not permission to add generic
adapters. Both vetoed attempts are excluded from passing evidence.

Child-registration bounded results: the real single-worker and two-role fixed
supervisor journeys pass debug/optimized builds. Each affine echo preserves its
original allocation and bytes. Two actual worker addresses stay distinct despite
equal creator-local nonces; first-worker retirement removes its own lease while
the sibling remains resolvable; full shutdown removes all three leases and
returns each complete worker, including explicit replacement and prior-ID
provenance. Wrong protocol, missing family and duplicate-protocol families deny
in both profiles (E0308/E0277, declaration diagnostic, E0119 respectively).
Replacing only the selected-family clone with the old independent allocation
makes both journeys fail solely on logical lookup in both profiles; the macro
is byte-restored and both journeys pass again. The actual Entity example's
hydration/passivation/reactivation/root-first shutdown also passes both profiles
after its empty failure lane uses the already-selected Vec<Never> schema.
All14 counter/heterogeneous/two-order/supervision/FIFO/cold/relative-path runs
remain green. The generated head/tail real projection-task failure witness
preserves accepted head custody across receiving cancellation, then joins the
pending tail and rejects duplicate acquisition in both profiles.

Aggregate checkpoint: binding5/startup2/retirement24/native-failure6 remain
unchanged; there is no new actor transition, result alternative or state module.
The existing root, nested and Entity setup each constructs its current scope's
finite occurrence product exactly once. The optional syntax value retains only
the selected host identifier; occurrence rows retain the same correlation maps,
creation order, tasks and authority. Runtime matches/branches are unchanged;
syntax derivation adds one two-alternative family selection. Production
+78/-31/net+47 across six changed owners; rustdoc+3, public types+0/-0, one
existing native-port method and one private construction trait added. This is
added registration capability, not code reduction. No repeated cause, arrival
history, semantic boolean, inferred provenance, row deduplication, nested actor
engine, discarded namespace or caller positional syntax is introduced.
Cross-checks: DX02/03/16/21/22, Driver publication, Address reservation/leases,
execution-ownership and actual normalized supervisor/pool contracts.
Provenance/composition/aggregate disposition: pass for this bounded candidate;
production retention remains open for full hosted cancellation/refusal and
released-contract/interface minimization. Source/counts/results:
/private/tmp/bombay-retirement-schema-kgazqjhv/CHILD-REGISTRATION-*.

Before release-contract reruns, remove the prototype's historical Behavior and
Communication source patches and select published Core/Actors0.24.0,
macros0.14.1 and Communication0.1.5 in its three Cargo workspaces. This changes
only selected packages and locks, with zero Rust/types/states. The actual
Bombay e4d2db1 published-contract compilation and the verified releases above
establish availability. Re-run the same journeys and denials; any source-only
empty-alias support remains separately open under PR91, never supplied by an
unused policy. Prior private-source passes are not registry acceptance.

Published-contract reruns now pass:32 executed journey runs and12 expected
compile denials across debug/optimized profiles. This comprises14 selected
counter/recursive/two-order/supervision/FIFO/cold/relative-path runs,6 hosted
supervision/repeated-role/Entity runs,2 generated failure-prefix runs,2 public
counter-plus-sibling runs and8 renamed-consumer runs; hosted and visibility
rejections contribute6 denials each. Cargo metadata in all three workspaces
proves registry Core/Actors0.24.0, macros0.14.1 and Communication0.1.5, with
no source patches. These are bounded prototype results, not retained Bombay
production or full-suite acceptance. Empty-alias PR91 remains independent.
Exact provenance and terminal results:
/private/tmp/bombay-retirement-schema-kgazqjhv/PUBLISHED-CONTRACT-{PROVENANCE,JOURNEYS-RESULTS}.json.

### Inferred roots and the native constructor

The retained direction is one domain topology declaration and an actual root
value, with its genuine ActorExt decorations inferred. This is a derived static
composition law, not inference of arbitrary recursive Rust type equations.
The declaration is application-scoped. It must check the actual decorated birth
row, keep physical registration and logical hosting distinct, and preserve
original creator provenance and complete recursive custody.

Pre-edit caller syntax and observable law: `root: Root { Branch(Branch) => node; }`
plus `Application::with_topology(root_value, garden::Topology::new()).run_with(work)`.
No composed-root alias/list, runner argument, structural position or recursive
terminal declaration belongs to ordinary callers. Counter must execute real
commands/shutdown; recursive children must return both report lanes, original
initialization refusal and every descendant; supervision/pools must retire after
real policy execution. Cold child origins retain the actual original Root.

| Owner | Existing responsibility preserved by the candidate |
| --- | --- |
| NativeChild | Select declared occurrence, physical family and exact parent/child custody. Parent terminal is already fixed by its namespace loan. |
| PendingChild | Own the original routed request and affine creator-namespace loan; construction stays private. |
| NativeChildStartup | Private runtime operation owns the sole existing fresh-allocation, initialization, source, binding, projection and rejection kernel. |
| EstablishNativeChild | Sealed public callable operation hides the complete interpreter proof and delegates that same kernel. It is one interpreter seam, not an executable marker or second actor contract. |

The exact full child CommitActions predicate, including sends/creation/whole-action
source-custody equalities, remains mandatory. Put conditional execution
eligibility only at the inferred root; concrete recursive descendant operations
close nominally and have unconditional eligibility. Every generic parameter has
existing actual substitutions: the two root orders, Node/Leaf, root/node terminal
families, Node/Never descendants and original origin functions. No callback,
marker, report-summary trait, new origin or exposed private host is justified.

The finite parent-custody selection is
`NativeChild<Creator, Position, Child, ParentTerminal, ChildDescendants>`.
It moves the old method's universally substitutable terminal into the existing
port: arguments4→5, method substitutions3→2. Root and Node schemas have distinct
creator origins and nominal representations. This narrows advanced custom
terminal presentations to their corresponding native operation. Native/binding/
declaration delta+24/-19/net+5 (including5 rustdoc lines), types/methods/modules0,
branches unchanged. An unnecessary ParentTerminal:RetirementSchema hypothesis
was removed rather than propagated; its existing projection law is sufficient.

The public operational port replaces PendingChild::establish. Its private
method-bearing supertrait supplies implementation authority; the public default
operation and blanket hide that proof. Eligibility predicates and runtime body
occur once. This supersedes the initial unsealed/custom-standard-kernel
hypothesis. Basis: [sealed interfaces](https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed)
and [supertraits](https://doc.rust-lang.org/reference/items/traits.html#supertraits).
A separate empty marker repeating the full predicates is rejected. Preliminary
native/export/declaration delta+103/-60/net+43 plus restored panic rustdoc; public
operational types+1/private operational types+1, inherent constructors-1,
runtime bodies1/modules0. The complete kernel body is byte-identical after the
receiver rename, including all awaits, owned startup rows, returns and joins.

Focused pre-edit/inverse evidence, all under pinned registry contracts:

| Falsifier or witness | Exact result |
| --- | --- |
| Domain-only counter/recursive declarations before root lowering | Four fixture/profile cases fail solely E0277 on missing decorated-root topology/native capability. |
| Generic root plus concrete Node port before selecting parent custody | Both profiles fail solely E0119; Base equality alone does not separate implementation headers. No exclusion marker is retained. |
| Foreign consuming-constructor declaration before owner edits | Three absent-port E0432 imports per profile, without private imports or recursive proof errors. |
| Restored original constructor/exports/call | Both profiles fail solely E0432; candidate bytes restore exactly. |
| Removed Trace or Readiness root ingress | Four independent cases fail solely E0277; source bytes restore. |
| Counterfeit constructor implementation | E0119 in both profiles; it is not accepted as kernel authority. |
| Foreign recursive/refusal consumer with sealed operation | Three root forms, two complete journeys each, both profiles:12 journey executions pass. |
| Generated root declaration | Inferred counter and recursive/refusal:four binary runs/six journeys pass. Root scope remains nongeneric. |
| Generated root composition | Both timer/shutdown root orders, raw natural root, supervisor replacement and FIFO:10 binary runs/16 journeys pass across both profiles. |
| Restored old declaration generation | Inferred counter/recursive reject solely E0277 in all four cases; macro bytes restore. |

The generated-root sets overlap and are not summed as independent coverage.
The prior full parent-custody composition remains32 journey runs and12 static
denials against published Core/Actors0.24.0, macros0.14.1 and Communication0.1.5,
with no source patches. The private scoped root proof additionally executes16
recursive/refusal/counter journeys and4 shutdown denials across both profiles.
Those distinguish root-level orders from child-level orders.

Before retaining each lowering, the checkpoint required law/classification,
caller syntax, complete custody, failing witness, affected owners and expected
delta (parent selection~15/-15; consuming port~40/-10; root generation~30/-15),
then exact counts and both-profile downstream execution. The sealed interface
adds capability and must not be described as source cleanup. Initial misplaced
probe bodies, five missed multiline receiver renames, unsupported Tokio paused
clock setup and an overgeneral test assertion were corrected without adding
production bounds/dependencies; none supply passing evidence. DEAD_ENDS records
the independent coherence and impossible-HTTP falsifiers.

Aggregate checkpoint: binding5/startup2/retirement24/native-failure6 and actor
control states are unchanged. All subordinate alternatives retain their current
original request, actor, receipt, report, task, allocation or exact failure cause.
No arrival history, repeated cause, false cardinality, nested actor engine,
semantic boolean, erased protocol or new structural caller requirement survives.
Cross-check Driver/execution laws, initialization/freshness/provenance, normalized
atomic policies and DX02/03/12/16/21/25/26. Private bounded evidence passes;
production retention remains open. Child-class aliases, cold/generic-root
inference, complete generated failure/cancellation custody, hosting and final
cost/gate measurements remain prerequisites. No catalogue migration is claimed.

Caller measurement: current actual counter main135 formatted lines/5293 bytes;
private inferred counter114 formatted lines/4568 bytes, no manual terminal enum
or runner type arguments and one topology declaration. Reformatting with pinned
Rust1.99 and rerunning both profiles passes. Warm rebuild+execution is3.24s debug,
7.21s optimized; these are not isolated runtime or clean-build measurements.
INFERRED-COUNTER-MEASURED-RESULTS.json records source counts and exact limits.
Production counter remains unchanged; remaining error handling, clean build,
monomorphization and full-programme costs stay open.

Evidence: /private/tmp/bombay-retirement-schema-kgazqjhv/
{INFERRED-ROOT-BASELINE,INFERRED-ROOT-OWNER-COHERENCE,PARENT-CUSTODY-JOURNEYS,
INFERRED-ROOT-OWNER-SHUTDOWN-OPERATION,ROOT-SHUTDOWN-DENIAL,
NATIVE-ROOT-BASELINE,NATIVE-ROOT-SEALED,NATIVE-ROOT-SEALED-DENIALS,
NATIVE-CONSTRUCTOR-INVERSE,INFERRED-DECLARATION-ROOT-ONLY,ROOT-COMPOSITION,
INFERRED-DECLARATION-ROOT-INVERSE}-RESULTS.json;
NATIVE-CONSTRUCTOR-KERNEL-EQUALITY.json and ROOT-DECLARATION-INVERSE-RESTORED.json.

### Shutdown capability locality in actual Bombay

Pre-edit derived law: execution/termination observation require no shutdown
capability unless they actually inject shutdown. Unique request ingress belongs
only on ApplicationLifecycle::request_shutdown. Desired caller syntax remains
`.lifecycle().request_shutdown()` through both genuine wrapper orders, and
ordinary/HTTP runners must execute a naturally completing domain actor.

The actual1cc510c consumer first proves outer shutdown request/replay/full
retirement in both profiles; reversed ordering and naturally completing one-shot
counter deny solely E0277. Before production changes, owning regressions add
both counter orders, a finite CounterSession and absent/duplicate-lane denials.
The finite session's genuine Finish command returns the reply and stops through
Actions; no clock feature/dependency is added. Reuse request_actor_shutdown,
Ingress, existing EventLayer injection, the entire execution/receiving kernel
and every retirement/source/provenance bound. Only interface/execution/HTTP
production owners change; expected~15/-15 lines, zero types/modules. Here and
Inside<Here> are the two real method substitutions; no runner path parameter.

Pinned Axum0.8.9's [graceful contract](https://docs.rs/axum/0.8.9/axum/serve/struct.Serve.html#method.with_graceful_shutdown)
and [IntoFuture body](https://docs.rs/axum/0.8.9/src/axum/serve/mod.rs.html#334-350)
prove its serving-error branch unreachable despite io::Result. The provisional
extra HTTP path parameters are rejected, preserving both method arities.
Before final lowering, finite-session HTTP rejects that unused ingress solely
E0277. Return the original serving result, observe root termination and delete
only the impossible branch. Binding/staging refusals and independent actor/task/
receiving custody remain intact. A changed upstream serving contract would
require a fresh explicit retirement law, not an unused capability today.

Actual production+26/-30/net-4; tests+334/-0 including snapshots; public
methods/types/modules+0/-0. One existing request method gains an inferred type
parameter. Aggregate phases remain ApplicationOutcome6, work custody5, work
presence2, binding5/startup2/retirement24/native-failure6. Across the three owners,
match/arm/if counts53/104/6→52/103/5 delete only the impossible HTTP decision.
Incarnation/admission checks, private lifecycle construction, cancellation
authority and separate root/family joining are unchanged. All surviving values
retain their existing originals and causes; no new state, history, authority,
boolean, erasure or structural caller syntax. Independent read-only review
agrees. Cross-check lifecycle authority, Driver/execution and normalized atomic
laws, original HTTP/Entity failures and DX01/03/16/21/22/25. Bounded law: pass;
programme/repository acceptance: open.

Exact current verification:

- Seven focused tests per profile:four complete runtime journeys, two reused
  counter transition tests and a suite executing two compile denials. Replayed
  shutdown cannot be accepted twice; transfers occur before assertions.
- Three original owner files restored:both all-feature profile tests deny solely
  E0277, then byte-restore. Missing/ambiguous lane remain E0277/E0283.
- Four real HTTP bind-refusal/retry runs pass both wrapper orders/profiles;
  original application/router/address/io::Error and complete retirement survive.
- All301 all-feature owner tests, workspace/all-target/all-feature compilation,
  strict Clippy and formatting pass. A test assertion-style veto is corrected
  without a lint exemption.
- Native Nix exits1:17 gates pass; child/all-features, child/no-default, owner
  coverage and workspace tests fail on the same four unfinished topology/custody
  compile-pass fixtures. Nextest exits100:579 run,578 pass, one same-suite failure,
  one skipped child. No disk/signing failure or waiver.

Exact terminal logs establish completed checks. Observe Loom passes28 cases;
its unrooted output later ceases to be registered. An empty failed coverage
output is not success. The final coordination prose changes no tested Rust byte
or included rustdoc input. Five completed task build caches are reclaimed under
the existing “free up” authorization, preserving source/evidence/other AI
checkouts; free space is7.0GB after verification.

Evidence: /private/tmp/bombay-application-shutdown-pv2wsny6/
{BASELINE-RESULTS,OWNER-FINAL-INVERSE-RESULTS,OWNER-VERIFIED-RESULTS,
HTTP-FINAL-RESULTS,FINAL-COUNTS,FULL-NIX-RESULTS,NEXTEST-RESULTS,SPACE-RECOVERY}.json;
OWNER-FINAL-owner-lib.log. Source, capability and diagnostic denials stay active.

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
