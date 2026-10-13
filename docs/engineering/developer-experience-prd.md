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
topology path now has an actual public candidate with recursive/native failure
and custody witnesses. Broad catalogue migration and complete acceptance still
require the remaining consumers, inferred standalone roots and full gates.

## Evidence and scope

Current Bombay registry contract: Behavior/Actors0.25.0, macros0.14.1,
Address0.3.0, Communication0.1.5, Timers0.2.0, Tokio1.53.1 and Axum0.8.9,
with no repository patches. The selected published source is2bb4510;
all136 downloaded Core/Actors Rust files match the reviewed release. The later
correlation reason is locally committed as821380e and is not selected by that
registry contract. Native correlation experiments use an explicitly private
source copy/patch; they are not evidence of registry delivery.

The following table and observations preserve the initial review baseline,
not the current dependency selection. Claims come from source and test bodies
at immutable revisions, not repository prose or recorded completion marks:

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
Progressive disclosure is the caller contract: start with familiar actor,
message, queue, worker and supervisor terms, inferred types and explicit real
policy choices. Each lower layer exposes additional control over the same
contract. Owning constructors must not require users to learn internal staging,
lane-routing or custody vocabulary for ordinary configuration. Reserve those
terms for the advanced operations that actually transfer that authority;
preserve their complete rejection and retirement values at every layer.
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
User priority: peak resident memory, retained heap bytes and allocation counts
outrank binary size. Measure equivalent actor counts, mailbox capacities, work
and complete failure/retirement custody; report binary size as a secondary
diagnostic rather than a proxy for runtime memory.

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
DX01–03 now accept the approved single explicit topology declaration; rejected inference experiments do not define current acceptance syntax.

| ID | Implementation and tests | Remaining work / disposition |
| --- | --- | --- |
| DX01 | One actual Application owns root/scope/members/families; App removed. Public topology candidate executes recursive, supervisor, FIFO and born/cold journeys in both profiles; counter uses inferred runners | Open: remaining selected semantics and full repository gates; complete receiving and HTTP consumer migrations now pass their focused matrices |
| DX02 | One declared topology derives spaces/custody and finite/born/cold actor types. Equal-type cold hosting, nested/cold recursive creation, both wrapper orders and original refusal custody pass both profiles/MSRV; recursive classes use nominal role owners. Wrong roles/hosts/births and missing owners remain denied | Open: canonical catalogue, complete host corpus and final gates |
| DX03 | Sealed startup closes native recursive proof. Actual303 owner tests plus19 runtime topology/cold/timer cases pass both compilers/features/profiles;23 static denials preserve report ingress, authority, role consistency, birth closure, hosting and origins. One obsolete cold restriction becomes an executable positive test | Open: all owner consumers, complete cancellation/freshness and final gates |
| DX04 | Lossless work/cleanup, borrowed state/custody/completion and total owned inclusion preserve24 outcomes. Actual generated custody retains original panic heap, rejected child, nested reports and later accepted sibling | Open: proportional recovery and remaining complete failure/cancellation consumer gates |
| DX05 | All run/paired/HTTP/Entity entry points select descendants from Root/Members metadata; six pre-edit inference witnesses and original-input/receiving journeys pass both profiles/MSRV. Supervisor/FIFO constructors infer actor types | Open: final full consumer/repository gates; all38 original cancellation/non-Send/custom-host journeys pass both compilers/profiles/features |
| DX06 | Locked Communication configuration inspected | Open: owner-scoped capacities and resource evidence |
| DX07 | Pure actor authoring and composed staged creation retained; actor/worker rustdoc and renamed consumer fixes published in macros0.14.1. Actual self-creating actor facade executes inside a native Entity family with original launch-refusal custody on both compilers/profiles. Empty-birth alias/spoof regressions pass owner/downstream profiles and ten native gates | Open: complete generic/private corpus; empty-alias fix91/97 is merged and published in macros0.14.2 |
| DX08 | Unique composed send lanes, six owner regressions, OneShot/Periodic in both shutdown orders, complete Actions and static denials published in Behavior/Actors0.24.0; actual Bombay uses the registry contracts | Open: full named-lane catalogue evidence |
| DX09 | Existing ActorExt inspected | Open: prove missing real decorations and delete duplicate recipes |
| DX10 | WorkerFactory has bounded inferred fixed/FIFO, failure/prefix, wrapper, memory-layout and denial evidence; named RecoveryPolicy is implemented locally:104 focused owner tests pass in both profiles, all16 real replacement/refusal/wrapper executions pass on1.99/1.96; all ten final owner gates pass963 tests. Registry delivery and other families remain open. Published0.25.0 from2bb4510 accepts direct or fallible initial factories through the existing fixed/FIFO/keyed constructors. Pure custody/inverse tests,963 pinned owner tests/ten native gates and five complete downstream journeys in eight configurations pass;765 fixed assertions preserved. Before-source denials prove the owner gap | Open: final registry consumer gates, dynamic/proxy construction and measured policy syntax across all five families |
| DX11 | All11 original fixed-supervisor/FIFO runtime/recovery/source-refusal journeys use the inferred path and pass both compilers/profiles/features;246 original checks preserve prepared prefix, source panic/refusal, assigned payload, shutdown and full retirement. Keyed affinity/exact rejection/unbind/joined retirement adds56 assertions, passing all eight configurations | Open: dynamic supervision, independent late completion and remaining policy/failure/final gates |
| DX12 | Occurrence/correlation owners inspected; actual duplicate-correlation panic reproduced. Behavior821380e adds complete typed rejection; owner962 tests/ten Nix gates pass. Private native guard preserves accepted prefix/suffix, original birth/replacement/replay requests and both joined children on both compilers/profiles | Open: native registry adoption of published Core0.26.0; exact-recipient comparison now passes real queue/dynamic/native Entity identity and closure witnesses |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Complete startup/provenance, raw/projected retirement and control/report rejection retained; actual303 owner tests pass both compilers/features/profiles. Published-contract Bombay e4d2db1 nextest:571 pass, one topology-suite failure, one parent-invoked child skipped | Open: generated topology/custody and complete downstream gates |
| DX17 | Retained custody accessor/product and composed send changes have bounded interface provenance; DirectoryConfig owns immutable validated limits, removing impossible family/runtime error branches and preserving affine inputs | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected; existing thiserror owns the narrow configuration rejection with both original limits | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial executed | Open: complete pinned diagnostic corpus and renamed/minimal consumers |
| DX20 | Application and exported interfaces establish clients/senders for exact recipients and stable Entity references. Root is the default; one shared ExternalActor can select multiple typed targets. Reply inference, original-target allocation rejection/retry, exact command custody, fresh origins, prefix draining and wrapper orders pass; eight denials preserve capabilities | Open: complete correlated/cancelled/slow-customer/late-reply corpus, capacity and final gates; pre-root scope retains exact endpoints through missing-host/prepared/staging/HTTP retry |
| DX21 | Method-local inferred shutdown ingress; runners require only capabilities they execute. Both root wrapper orders, request replay, natural domain/HTTP completion and complete retirement pass7 focused tests per profile; absent/ambiguous lanes remain denied; old-source inverse denies only E0277 | Open: generated canonical access, child-derived phases/drain corpus and final repository gates |
| DX22 | One Application owner composes ordinary/cold actors, HTTP and Entity. Declared family roots derive actual actor/child custody; finite two-family, recursive and all four original cold/family recovery journeys preserve their independent receiving, launch refusal and original heaps on both compilers/profiles | Open: remaining Entity/HTTP acceptance and full gates; admission-only clone/unused reply protocol/target wrapper removed; real closed admission preserves original order allocation, five example tests pass both compilers/profiles; all38 original receiving/failure journeys plus five HTTP runtime cases pass both compilers/profiles/features |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative; FIFO role-rejection consumption now preserves the exact owned failure and lifecycle product through both projection orders, inverse, native consumer and ten owner gates | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inference fixtures; original startup falsifier repaired in actual coupled-source runtime; owner tests and static denials preserved | Open: retain inferred API and complete downstream corpus/final gates |
| DX26 | Actual counter main98 lines, FIFO184; Entity communication removes its unused reply protocol/interface and repeated target argument. Complete independent retirement oracles remain. Exact warm costs and full tracked/untracked deltas recorded | Open: broader comparable caller/compiler/runtime measurements and final resource gates. Five paired complete counter journeys use equal77 registry dependencies:74 allocation blocks each, peak heap122916→123644 bytes (+728), unchanged end heap; no memory reduction claimed. Scope adds another measured8 peak bytes with identical allocation blocks |
| DX27 | Registry0.25 owners are published from2bb4510 without patches;963 owner tests, ten Nix gates, CI and preflight pass. Bombay e197d4d passes all21 native Nix gates in2177.895 seconds, including factory migration, complete caller migrations and compiled/package guide. The eight-configuration topology/family/cold, receiving38 and HTTP5 matrices remain | Open: PR91/new correlation owner delivery and remaining final consumer/package/MSRV gates. Correlation owner821380e passes962 tests/strict Clippy/ten Nix gates. Subsequent scope/equality passes its bounded composition/aggregate and package-guide checkpoints; its full native capture passes all21 gates in2244.019 seconds (runtime source and compiled guide match8011d49) |
| DX28 | Initial pruning preserved; one canonical guide is included through Cargo readme metadata. Complete leaf Application/joined cleanup executes in workspace and extracted runtime/engine/macros on all eight configurations;59 local links resolve | Open: compact reconciliation, rendered external links and final documentation gates. Scope guide executes in all eight workspace/archive configurations; stale normative version prose delegates to the current selection record |

### Retained contracts and evidence

This section records current laws and bounded evidence once. The requirement
checklist above owns remaining work. A passing local experiment is not a
registry release or whole-programme completion. Raw measurements and exact logs
are in `/private/tmp/bombay-retirement-schema-kgazqjhv/`; portable acceptance
comes from retained tests, selected commits and CI. Do not reconstruct the
removed chronological ledger.

#### Application composition and topology — DX01–05/16/22

The approved assembly law is one Application and one domain topology declaration.
The declaration closes actual decorated birth rows, native hosts, ordinary
children, application-owned members and Entity family roots. Domain role owners
provide nominal recursive classes; sealed startup closes the native execution
proof. This is derived assembly, not a new actor transition or executor.

Unconditional factories rejected legitimate pure birth aliases; conditional
execution proofs recursed indefinitely. Neither candidate is retained. One
explicit declaration replaces manual spaces, composed actor aliases, structural
paths and recursive terminal enums. Family roots derive Behavior from their
EntityDefinition; explicit host choices remain actual logical-space policy.
Automatic family-host synthesis overlapped those existing proofs and was
rejected. Nonempty birth aliases still require their real settlement policy;
absence of birth capability needs no fabricated policy. The focused alias fix
is owner commit a1a06ef, refreshed7d5bb20/PR91; merged and published in macros0.14.2 through release97.

Initialization remains before mailbox events, creation commits before dependent
sends/observations, and nonce collisions never replace actors. Successful
creation reports preserve initial, ordinary and replacement provenance. Native
runner projection retains the original root, admitted sends, rejected requests,
accepted prefix, untouched suffix, actor-originated births and complete nested
retirement. Cold trees and hydrated Entity roots use the same native custody;
they do not create another lifecycle engine. Existing runtime control sums and
interpreter decisions remain unchanged; generated metadata has no transition
authority, historical state, boolean phase or erased protocol.

Bounded evidence:303 original owner tests; native recursive/cold/timer corpus;
23 retained topology/authority/host denials; original38 receiving/failure
journeys and five HTTP cases in both compilers/profiles/features. Complete
finite/recursive families include hydration/admission/initialization refusal,
passivation/reactivation and independent shutdown of both families. Generated
custody keeps original panic/refusal heaps and later accepted siblings. These
matrices do not close all remaining programme acceptance.

#### Terminal custody and application capabilities — DX04/17/20/21

Borrowing observes retained state/result; consumption transfers complete owned
products. `work_output`, `cleanup`, `behavior` and `completion` expose the
existing facts. `try_into_completed` includes only genuine completed work and
returns every other original outcome on rejection. The shared ActorCustody
product removes306 net production lines without collapsing24 outcomes.
Initialization panic, refused creation, interrupted work and cleanup failure
remain independently inspectable and owned; successful work never implies
successful cleanup.

Application is the entry point for clients, senders and shutdown. A client
binds one exact recipient; one external actor can select several typed targets.
Scope owns clients established before the root is supplied. It has no second
lifecycle authority. Missing hosting, preparation/staging refusal and HTTP
startup failure return the exact scope with its original endpoint and queued
reply prefix. Retry preserves that same custody. Final shutdown requirements
are method-local: a runner requires the shutdown ingress it actually executes.
No absent/ambiguous lane, forged actor control or weaker capability is admitted.

Exact endpoint equality is the existing typed Weak allocation identity, including
after closure; equal numeric addresses in distinct namespaces remain unequal.
WorkQueue's idempotent worker availability and dynamic supervisor's exact query/
lifecycle routes supply independent consumer witnesses. The real RED denied
those uses before ActorRef's equality implementation; no payload Clone bound,
untyped route registry or allocation-based provenance is introduced.

Scope/equality is retained at Bombay8011d49 with documentation c3b74ab:
production net184, one public capability, no new actor state/branch/trait.
Complete reply/refusal/retry/wrapper journeys and eight capability denials pass.
Package/workspace guide and real queue/dynamic/Entity identity corpus pass all
eight compiler/profile/feature configurations. Full native21 gates pass in
2244.019 seconds; runtime and compiled guide match that captured source.

#### Effects, preparation and worker factories — DX07–11/15

Actions remains the sole explicit send/create/become-or-termination boundary.
A uniquely typed send can be appended through timing and shutdown wrappers
without naming a positional lane. Six owner regressions and both OneShot/
Periodic shutdown orders preserve all independent effects. This and facade
rustdoc/renamed-dependency fixes are published in Behavior/Actors0.24.0 and
macros0.14.1. No erased sends, ambient effects or strengthened caller protocol
is selected.

Published0.25.0 accepts direct WorkerSubmission or exact fallible Result from
initial fixed/FIFO/keyed factories. Initial preparation owns its accepted prefix,
exact failed role/error, untouched suffix and original factory. Missing policy,
duplicate roles and rejected creation preserve complete construction custody.
The pre-edit owner/compiler witnesses, pure custody tests and inverse pass;
765 fixed assertions remain unchanged. The release's exact native ten gates
include963 tests; downstream complete journeys pass both compilers/profiles/
features. Dynamic/proxy and other catalogue construction remain open.

WorkerFactory is a Bombay runtime capability, not a behavior or source state
machine. It owns one FnMut callable plus a zero-sized role/preparation association.
Desired and implemented syntax is `WorkerFactory::new(|role| submission)`;
a fallible Result retains the caller's exact worker error. Whole-source refusal
remains the genuine advanced WorkerPreparationSource port. The factory's
SourceRejection is Never because one invocation can only refuse that worker.
Role:Sync remains local to the runtime preparation port. Constructor inference
adds no caller annotation, default, dummy callback or widened capability.

Creation of the factory and dropping an unpolled first/later preparation future
do not invoke the callable. `into_factory` returns its original callable/current
captures without invocation or cloning. The eager-invocation inverse fails the
unpolled-future oracle. Six retained denials cover non-callables, wrong output,
non-Send captures, clone, reuse and fabricated whole-source refusal. Current/MSRV
categories agree; seeded trybuild resolution has94 registry identities and no
drift. Latest unpinned initial trybuild resolution was rejected as gate evidence.

Complete fixed/FIFO native replacement and retirement, three-role preparation
with accepted prefix/exact refusal/untouched suffix, and held Stop released
through stash/shutdown in both orders pass16 failure/wrapper executions across
1.99/1.96 and debug/release. The retained24-command repository matrix passes.
Function item/factory sizes are0/0 bytes; captured closure/factory8/8; first
future24. Those two layouts show no extra factory storage, not total runtime
memory or speed. Capability production is+101/-2/net99; supervision example
migration is+3/-36/net−33 with all25 assertion bodies/order preserved. One public
type/two inherent methods; no new production module/trait/dependency, actor
control alternative or interpreter. WorkerPreparationStart remains Submitted |
WorkerRejected | SourceRejected. The added first-call match selects only these
existing outcomes. No arrival history, repeated cause, false cardinality, nested
engine, semantic boolean, structural user type or placeholder survives.
Full native Factory capture fails Clippy at an unnecessary drop_non_drop in
the new unit test:18 gates pass, test/doc gates are cancelled. Replace that
explicit drop with an owned unused binding. Separately retain the already-proven
into_factory original-boxed-refusal test in the existing unit module, using its
real one-role worker/activation fixture. No production/API/state change is
selected; original pointer/data ownership and preparation timing laws remain
the regressions. Final full gates, packaging and publication remain pending.

#### Named configuration and naming — DX09/10/28

The user approved named configuration for all policy-heavy templates, familiar
domain language, progressive access to lower layers and renaming existing
awkward APIs. Prefer short concrete nouns; owning modules provide context.
Follow [Rust naming](https://rust-lang.github.io/api-guidelines/naming.html):
files/modules snake_case, types/variants UpperCamelCase, normal constructors
new and ownership conversions into_*. The [module reference](https://doc.rust-lang.org/reference/items/modules.html)
and [Tokio runtime::Builder](https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html)
show contextual naming. Preserve this repository's aggregate mod.rs convention.
[Builder guidance](https://rust-lang.github.io/api-guidelines/type-safety.html)
supports builders for complex incremental construction; it does not require a
builder for an ordinary product of required choices. The plain-noun preference
is project API policy, not a Rust language rule. A rename changes source,
consumers, rustdoc, fixtures and migration guidance together; retain one spelling.

Inventory from actual current constructors: fixed7 arguments, dynamic6, FIFO8,
keyed10 are the primary positional policy owners. Factories, rosters, selectors
and concrete typed routes remain distinct from immutable policy. PoolRecovery
already exposes named Permanent/Transient variants; buffer/cache use validated
configuration and workflow/barrier definition/membership products. Queue,
latch, sequencer and rate-limiter need at most two ordinary values. CircuitBreaker
needs a separate threshold/reset/timer policy audit. This inventory is remaining
work, not acceptance for the whole catalogue.

RecoveryPolicy owns strategy/limit/delay, independent of the affine source and
permanent/transient eligibility. This is deliberate API policy preserving the
existing fixed-supervisor selection, admission, timing and retirement law.
The intended owning syntax is now implemented locally:

```rust
Recovery::permanent(
    source,
    RecoveryPolicy {
        strategy: Strategy::OneForOne,
        limit: RestartLimit::new(1, Duration::from_secs(30)),
        delay: RestartRelease::immediate(),
    },
)
```

Transient takes the same product; temporary remains source-free and needs no
unused choices. RestartRelease remains the validated timing owner. No builder,
default, extra validation, generic parameter, trait, bound or source capability
is introduced. The real registry0.25 consumer RED has only E0432/E0061, and the
focused owner construction RED has the same missing product/signature causes.

The policy replaces two repeated public-construction bundles. At runtime it is
consumed into the existing flat RecoverySettings plus eligibility. Keeping the
public product nested there increased the actual same supervisor344→352 bytes;
that lowering is rejected in DEAD_ENDS.md. Unpacking restores344 bytes with the
same complete native journey. There is no second stored policy copy. This is a
retained-size measurement on1.99/aarch64-Darwin, not an allocation/throughput claim.

FixedRoster stays New | Operating | ShuttingDown | Terminating | Stopped;
RecoveryDecision stays Permanent | Transient | Temporary; SupervisorRecoveryState
stays Automatic | Temporary; source custody stays Available | PreparingWorkers |
Retirement. Every existing owned worker/source/attempt/readiness/budget/
correlation remains at its original owner. Match and arm counts are unchanged;
no history, repeated cause, false cardinality, nested actor, boolean phase or
structural caller syntax is added. Cross-check fixed eligibility/selection,
restart/timer admission, initialization, full settlement and actor-graph drain.

After the bounded design proof,38 old calls in initialization and three existing
fuzz traces mechanically adopt the product; all839 assertions are unchanged.
Canonical syntax documents migrate together. The104 focused owner tests pass
in debug/release on pinned1.95, and all eight real replacement/refusal/wrapper
journeys pass1.99. All eight equivalent pinned1.96 journeys also pass; fuzz binaries compile.
Final production+40/-42/net−2, tests+212/-131/net81, docs+4/-4/net0; one
public product, no new module/trait/bound/dependency or transition branch. All ten
final native gates pass in191.790 seconds, including963 tests and68 rustdoc
denial diagnostics; all ten captured files match final source. No signing/disk
waiver. The earlier flat gate passes in266.296 seconds; final capture also
includes formatting of the three independent fuzz crates.
No four-argument compatibility constructor is retained. The rejected nested
candidate's interrupted Nix run is not a passing final gate. Bombay's working
source is unchanged; its final WorkerFactory full check failed in1906.182 seconds from disk exhaustion during optimized workspace compilation. This is not a passing gate. Completed task-owned build caches are being removed before retry;
private source patches do not count as registry delivery. The identical-source WorkerFactory retry passes all21 native Nix gates in705.439 seconds; all18 captured files match. The preceding1906.182-second ENOSPC attempt remains a failed gate, not a waiver.

Pending constructor configuration, before production (DX10): use the owning
module's short Config name for extensive simultaneous choices. The real fixed
replacement/refusal/wrapper consumer now requests
`fixed(factory, roles, fixed::Config { activation, recovery, failure, shutdown,
diagnostics }).build()`. Source and diagnostic route are the two real generic
substitutions; Config introduces no bounds or defaults. Classify as deliberate
API policy, preserving all existing pure initialization, selection, budget,
delay, shutdown and complete refusal custody. Keep the final running supervisor's
existing flat storage; consume the construction product before runtime. The
existing builder's optional typed lifecycle publication remains its own genuine
capability choice. Config replaces repeated constructor/builder/rejection policy
bundles, and rejection returns the complete original product. Move the actual
fixed owner to the matching fixed module rather than adding a forwarding module.
Expected bounded design: one product, no trait/dependency/actor state/branch;
roughly net±50 production lines plus file moves. First RED is the unchanged
complete private fixed journey requesting the absent owning Config/three-argument
constructor. Prove pure complete rejection, prefix, retirement, both wrapper
orders and unchanged retained memory before any catalogue migration. Extend the
same named-field standard independently to dynamic/FIFO/keyed where their actual
policy equations require it; no one-role pool receives fixed roster strategy.
Pre-edit native consumer RED rejects only the absent module/Config and seven-to-three argument change; the five owner construction regressions also reject those and the absent complete `config` return (E0433/E0061/E0609). Baseline fixed owner source (including internal tests):10,598 Rust lines,19 source files,28 public declarations,307 lexical matches and748 arms; control sum remains New | Operating | ShuttingDown | Terminating | Stopped. No subordinate sum or result alternatives change: all existing roster/member/recovery/retirement data remains owned by the same current values. Expected Config replaces5 builder fields and5 rejection fields, with the same worker-source and diagnostic capabilities; no new actor state or branch. Cross-check fixed-supervisor, worker preparation, proxy, Driver initialization/settlement and normalized atomic laws. Before checkpoint passes; post-proof measurements remain pending.

FIFO configuration proof, before production (DX10): the unrelated direct-worker
pool uses `fifo(factory, roles, fifo::Config { activation, recovery, backlog,
interruption, shutdown, diagnostics })`. Deliberate caller API policy; all six
simultaneous choices preserve their existing values and laws. The existing
`PoolRecovery` supplies real source eligibility; no fixed-roster strategy or
placeholder is added. Construction prepares the ordered roster once, returning
`workers + original Config` on refusal; success destructures Config into the
existing flat FifoPool fields. Replace the six repeated constructor/rejection
fields; move the actual owning fifo_pool directory to fifo and expose only its
existing public contracts plus Config. One public product, no bound, trait,
dependency, state, transition branch or interpreter change; expected net±30
production lines plus ownership-preserving file moves. REDs: focused pure
complete construction rejection/initial creation and the real native FIFO
replacement/customer completion/retirement journey. Preserve heap identity of
an owned diagnostic route on rejection. Baseline FIFO owner source (including internal tests/unreferenced historical custody file):6,055 Rust lines,6 source files,13 public declarations,143 lexical matches and352 arms. Control state remains Constructed | Operating | Draining | Stopped | ForcedRetirement; all subordinate phases and payload ownership remain unchanged. Corrected owner RED fails only E0061/E0433/E0609; native RED only E0061/E0433. The first owner test fixture used MailAddr without its required endpoint contract and is not accepted as RED evidence; the corrected test reuses the existing FIFO interpreter fixture. Fixed five owner tests and all eight
native replacement/refusal/wrapper journeys already pass; broad catalogue
migration waits for this second template and its complete journey.

Named configuration design checkpoint: both pure construction suites pass in
both profiles (5 fixed +2 FIFO). All8 fixed native replacement/refusal/wrapper
runs pass; the complete FIFO native customer/replacement/retirement journey
passes debug/release. Actual retained sizes are fixed344 bytes and FIFO272
bytes; fixed matches its recorded pre-change size. No runtime fields change.
Both aggregate control sums and every subordinate sum/result payload remain
unchanged. Fixed source including internal tests:10,598→10,557 lines,
19→19 files,28→29 public declarations,307→307 lexical matches,748→748 arms;
FIFO6,055→6,061 lines,6→6 files,13→14 public declarations,143→143 matches,
352→352 arms. Source counts include internal tests; production-only deltas
will be measured separately. The obsolete private builder test with a non-worker
factory is deleted; real initialization and complete rejection tests replace it.
Only Config, the two owning module spellings and the already-proven constructor/
rejection fields change; runtime transition authority, initialization order,
creation provenance, source/route capabilities and retirement ownership do not.
Residue scan: no arrival history, repeated cause, false cardinality, nested
transition authority, semantic boolean, default, placeholder, widened bound,
new trait or structural caller path. Cross-check fixed/FIFO/proxy/preparation
and Driver initialization/settlement laws and normalized atomic documents.
Disposition pass for the bounded design. Begin mechanical fixed/FIFO caller
migration with assertion counts, existing rejection values and API names
preserved; no new semantic surface may be added during migration. Other
policy-heavy families remain independent follow-up work.

Source hierarchy and startup vocabulary, reopened by user (DX10/19/28):
`atomic` and bare `fixed`/`fifo` are rejected as source ownership names. The user
requires a tree of noun-named concerns whose parents curate public/private
contracts. Replace the catch-all group with a private `templates` catalogue,
curating public supervisor, pool, proxy and worker owners. Supervisor owns
`roster` (fixed ordered membership) and `directory` (dynamic membership); pool
owns `queue` (global FIFO admission) and `router` (keyed affinity). Move existing
owners intact; configuration/construction moves into each owner's private
`config.rs`, re-exporting Config and its sole constructor at that owner.
Desired syntax is `supervisor::roster::new(factory, roles, roster::Config { … })`
and `pool::queue::new(factory, roles, queue::Config { … })`. No parallel atomic
or fixed/fifo namespace remains. Preserve qualified domain types; single-word
filesystem nouns acquire meaning from their parents. Rust API naming and module
privacy guidance support the convention; this is deliberate API/ownership
policy, not an actor-model law. No lifecycle state, effect, source capability,
visibility of authority constructors or interpreter proof may be widened.
`startup` replaces application-worker activation terminology: StartupPlan,
StartupLimit and ImmediateStartup, with WorkerSubmission::with_startup;
creation, pure initialization and resulting readiness remain distinct.
Pre-edit REDs must use the actual complete fixed/FIFO native journeys with the
new owner paths and vocabulary, followed by the focused owner construction,
macro facade/rename and denied-authority corpus. Existing transitions,
preparation, proxy start, retirement and wrapper equations are reused unchanged.
The focused owner RED uses the existing complete ordered initialization and
original-config rejection tests; it rejects the absent public owners. Pool
assignment/macro contracts belong to pool, proxy controls to proxy, worker
startup/preparation/roles/restart choices to worker, drain policy to lifecycle,
and diagnostic dispositions to diagnostic. Private templates retains the same
internal authority scope as the old private children of atomic; public group
re-exports introduce no access to issued grants/tickets. Startup vocabulary maps
ActivationPlan→StartupPlan, ActivationPolicy→StartupLimit,
ImmediateActivation→ImmediateStartup, BeginActivation→StartWorker,
ActivationPermit→StartupPermit, ActivationStartRejection→StartupRejection,
WorkerActivation/Grant/Outcome→WorkerStartup/Grant/Outcome. The owned plan's
interpreter operation is start; WorkerSubmission::with_startup carries it without
claiming readiness. Named runtime startup lanes remain in their existing order. The original
startup grant and the optional pending startup custody are different values:
retirement returns workers + role + original worker + original grant + outcome,
while independently retaining the pending custody required by that outcome.
A blanket local-variable rename collided with existing `startup` custody and
fails E0308 in the exact return branch; preserve the grant as `grant`, without
adding a type, bound, state or reconstructing its authority. The original
retirement return equation remains authoritative.
Macro-generated pool completion paths must target the real public pool owner,
including facade precedence and renamed dependencies. No equivalent shim remains.
Expected file moves plus curated group modules and configuration extraction;
no new semantic types or traits beyond the already-proven two Config products.
The previous short-path configuration gates pass all ten in287.363 seconds,
but do not accept the rejected naming or satisfy the final hierarchy gates.
The seven focused new-owner construction tests now pass. Before curation: delete
the obsolete catch-all public forwarding exports; retain only crate-private
aliases consumed by existing transitions. The shared validated-capacity error
keeps one canonical public spelling, `behavior_actors::ZeroCapacity`. Move pool
worker macro documentation to the pool owner and update its executable paths.
Extract the existing Config, sole constructor, complete rejection and fixed
builder into each owner’s private config child; parent re-exports preserve the
proven caller surface. Child construction consumes the same factory + roles +
Config and returns the exact same original product on refusal. No runtime field,
state, branch, bound, authority visibility, trait or dependency changes.
The focused new-owner suites pass7 tests in both profiles, warning-clean.
All20 complete native journeys pass on1.99/1.96 in both profiles: replacement,
owned prefix plus role refusal, both stash/shutdown orders, and FIFO customer
completion/replacement/drain. Retained layouts remain344/272 bytes. These use
an isolated copy of actual Bombay52a7005 with six source files mechanically
adapting owner paths and the same startup interpreter; no tracked Bombay edit
or registry delivery is claimed. All6 macro resolution/hygiene tests pass,
including renamed/direct/facade precedence and missing-dependency rejection;
all56 Actors rustdoc denials plus12 pass examples pass. The attempted whole
macro-fixture workspace run included its deliberately failing missing-dependency
member; it fails as expected and is not counted as a passing gate.
Before mechanical migration: aggregate control sums and subordinate payloads,
transition matches/arms and authority constructors are unchanged. Config stays
construction-only; no runtime storage or second transition authority is added.
Roster19→20 and queue6→7 source files reflect the two owned construction children;
two supervisor/pool grouping modules curate existing owners. Delete catch-all
flattened exports and obsolete duplicate construction fixtures; every surviving
subordinate value retains its prior decision/return purpose. No semantic boolean,
arrival history, repeated cause, false cardinality, structural caller path or
new bound/trait/dependency is accepted. Cross-check fixed/FIFO/proxy/preparation,
Driver settlement and normalized actor laws. Bounded design disposition pass;
mechanical migration may apply this proven surface only. The migrated workspace
passes964 nextest tests with zero skipped. Before final curation, the existing
zero-capacity regression requests `directory::EntryCapacity`: retained directory
entries are a supervisor policy, not worker startup. Keep `worker::StartupLimit`
in a private worker limit child, move EntryCapacity into a private directory
capacity child, reuse the exact validated positive-capacity value and typed
ZeroCapacity error, and delete the mixed limit leaf. This is ownership/naming
policy only: fields, validation, alternatives, branches and authority unchanged.
Finish the same worker-startup vocabulary in worker-specific phase labels
(Starting, ReadyForStartup and StartupRejected), with one-to-one label mapping;
keep the unrelated pure Activate/Active initialization contract unchanged.
In particular, emitted-but-unacknowledged startup is StartupRequested; actual
accepted startup is Starting. Request refusal is StartupRefused and still owns
the original StartWorker request + reason; plan rejection is StartupRejected
and owns the plan’s exact rejection. These remain different existing alternatives,
not a new phase or inferred acceptance. The directory-capacity RED fails E0603
because the intended owning public path is not yet exported.
The blanket Starting label is rejected: ProxyShutdown already uses Starting
for pending fresh creation, while established-worker startup owns a different
product (E0428). Before correction, its control alternatives are pending creation,
worker return, failed-start return, predecessor return, successor return,
initialization, startup and predecessor join. Preserve all eight exact products:
rename only the pending-creation alternative WaitingForCreation; the established
startup alternative remains Starting. Root ProxyState’s eight alternatives and
WorkerStartPhase’s four alternatives stay intact. WorkerStartRetirement::Startup
owns any WorkerStartupRetirement, including readiness or either refusal; it is
not a rejection-only alternative. No type, bound, flag or alternative is added.
All corresponding transition/late-shutdown/refusal assertions remain authoritative.
The corrected phase names pass964 workspace tests and all20 native journeys
on both supported compilers/profiles. Full final gates remain pending. Source
(including internal tests/rustdoc) measures70→74 files,45,892→45,796 lines,
154→156 public declarations,1,010→1,010 lexical matches,2,477→2,477 arms and
969→969 internal assertions. Integration assertions3,435→3,454 add the focused
configuration witnesses; existing transition assertions are preserved. Only the
two Config products add domain types. Before final public curation, delete the
proxy/roster re-exports of shared worker ports: WorkerSource, WorkerAttempt,
startup and preparation contracts have one canonical worker owner; each family
keeps only private imports of the contracts it consumes. This deletes duplicate
public spellings without changing any type, authority visibility, transition,
interpreter requirement or consumer syntax. Final production/test deltas and
post-curation verification passes: all ten native Nix gates in89.598 seconds,
with all captured source hashes unchanged. Nextest reports964 tests/zero skips;
rustdoc preserves43 Core +56 Actors compile denials. All20 final native journeys
pass on1.99/1.96 debug/release with retained sizes344/272. The independent
interpreter workspace passes19 tests plus7 compile-denial fixtures; five
snapshots changed only owner paths, private-scope text and source spans after
inspection of their unchanged E0624/E0599/E0382/E0603 causes. No denial is weakened.
The preceding51.806-second Nix failure exposed old shared-worker imports still
using supervisor forwarding paths; those imports now name the actual worker
owner. That failure is retained and not waived. Physical production-path diff
(including internal tests/rustdoc) is+2619/-2715/net-96; tests/fuzz/benchmarks
+2849/-2356/net493 before final snapshot metadata. One canonical worker owner
replaces the forwarded public ports; only2 Config domain products are added,
with no new state, trait, bound, dependency or actor transition. Both full
control-state sums and every subordinate owned value remain as above. Cross-check
fixed/FIFO/proxy/preparation and Driver settlement laws: final bounded
configuration/naming disposition pass. Source-patched consumer proof does not
claim publication or registry adoption; dynamic/keyed extensive-policy products,
latest-main delivery and the remaining DX rows stay open.

#### Owner failures and delivery — DX12/24/27

Creation correlation is not actor identity or replacement provenance. Duplicate
correlation returns the original request with a typed reason; accepted creation
prefix and later suffix remain independent. Owner821380e/PR93 passes962 tests,
strict Clippy and ten native gates. Complete native initial/replacement/replay
failures and both joined children pass private source guards on both compilers/
profiles. PR93 merged externally as6eb4f73 on2026-10-13; exact main CI38346109051 passes.
PR91 merged under explicit authorization as79cae26 after fresh ten native gates,
964 tests, archive preflight and exact-head CI38346558064. The generated release
PR97 at8064f22 repairs all four workspace locks, README and changelogs without
production changes; secondary registry identities17/29/34 have zero drift.
Archive preflight and ten native gates pass in287.305 seconds; exact-head
CI38350983446 passes. Under the existing authorization PR97 merged as98983b4;
main CI38352087285 and normal Release38353101159 both pass. Registry publication
is independently confirmed: Core/Actors0.26.0 and macros0.14.2 are available,
un-yanked, with all three GitHub release tags. The exhaustive
CreationRejection::CorrelationAlreadyRecorded break is documented. Bombay’s
committed registry graph still uses0.25.0/0.14.1; downstream adoption is separate.

FIFO diagnostic consumption at owner d287f85 is a derived ownership projection.
`into_worker_rejection` returns original Arc role, previous WorkerAttempt,
complete ChildStopped, optional source and exact worker refusal. All other
causes return Err(original). Both worker/source projection orders preserve
original role/source/error heap identities; the inverse fails the ownership
oracle. PoolState remains Constructed | Operating | Draining | Stopped |
ForcedRetirement, with all ten original diagnostic causes unchanged. Production
+30/-1/net29; focused tests+121; one method, no new type/trait/bound/module or
actor transition. Lexical matches11→12 and arms25→27 reflect only the consuming
projection. No cause duplication, inferred provenance, historical state or
second lifecycle authority survives. Cross-check FIFO/preparation, Driver
settlement and exact retirement laws. Native ten owner gates pass965 tests in89
binaries plus68 rustdoc denial diagnostics; no signing/disk waiver. The full
private native FIFO refusal consumer owns the error and independently joins the
root/worker. This API is committed but not published; there is no registry claim.

Published owner changes were merged only under the user’s concrete authorization:
PRs89/90, Communication11 (complete-input send_map), Behavior94/95 and91/97.
FIFO projection and named policy releases still need concrete review/delivery.
Feature work remains isolated from main and the other Bombay AI. Expanded
cumulative change-budget authorization was given2026-10-09; the topology
exception2026-10-11. Do not request those already-granted permissions again.

#### Verification and costs — DX19/25–28

Use pinned Nix only for Rust, both supported Bombay compilers1.99/1.96, both
profiles and applicable feature choices. Required authoritative native gates
are ten in Behavior and21 in Bombay. Prior Darwin signing and exhausted disk
failures are neither passes nor permanent waivers. Latest retained pre-factory
Bombay8011d49 capture passes21 gates in2244.019 seconds; WorkerFactory's separate
live capture is pending. The final named-policy corrected capture passes all ten gates in191.790 seconds.
Linux/x86-Darwin gates have not been executed locally. Exact source hashes/logs
are retained; checks for an earlier or rejected candidate do not certify later
working source. Package/archive consumers and minimal/renamed/diagnostic corpus
remain explicit checklist obligations. Preserve other AI and global Nix caches.

Actual ceremony measurements include counter main98 lines and FIFO184. Five
paired complete counter journeys use identical77 registry dependencies and74
allocation blocks each: peak122916→123644 bytes, unchanged end heap. Scope adds
another measured8 peak bytes with equal blocks. No memory reduction or general
comparison with erased/untyped systems is claimed. WorkerFactory's example
removes33 lines and its two sampled callable layouts add no storage; named
configuration restores the sample supervisor's original344-byte retained size.
Broader compiler/runtime/resource comparisons remain open.

Distillation preserves the normative requirements, selected directions, one
DX01–DX28 checklist, current semantic contracts, source/consumer evidence,
measured checkpoints, actual approval limits and explicit deferrals. Historical
failed candidates remain in DEAD_ENDS.md only where their falsifier matters.
The evidence appendix owns initial source/catalogue coverage; canonical user
syntax belongs to Bombay's compiled user-facing guide. Network transport,
durability, stable global identity and deployment are separately deferred.
