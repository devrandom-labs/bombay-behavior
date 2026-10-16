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

Current Bombay selected registry contract: Behavior/Actors0.26.0, macros0.14.2,
Address0.3.0, Communication0.1.5, Timers0.2.0, Tokio1.53.1 and Axum0.8.9,
with no repository patches. Published source98983b4 matches all207 downloaded
owner Rust files. The isolated runtime now returns complete duplicate-correlation
rejections through that published contract. Focused current/MSRV and both profile
proofs and all21 fresh native gates pass; remaining programme work stays open. Earlier
0.25 and private-patch results retain their revision limits.

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

For standalone families, use the owning `supervisor::roster::new`,
`supervisor::directory::new`, `pool::queue::new`, `pool::router::new` and
`proxy::StableProxy::new` constructors. Policy-heavy constructors take their
owning `Config`; automatic recovery takes the owning `RecoveryPolicy`. These
consume real choices into existing actor storage. No parallel builder or
same-actor extension method is selected for a standalone actor. Keep `MessageAdapterWithRoute::new` for the
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
require an unused source; immediate startup must not require a dummy plan;
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
prefix. Acceptance: immediate and async startup, source refusal, later
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

## Tooling

Use ordinary Rust and the existing selected crates. The bounded
[tooling review](developer-experience-evidence.md#tooling-review) preserves the
requested registry and stable-language comparisons, not another API proposal.

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
| DX10 | Published0.27.0 owns named Config/RecoveryPolicy and exact roster/FIFO replies. Factory-input candidate preserves ordinary unannotated factories:7 focused cases in both profiles and15 downstream cases on both compilers/profiles; complete refusal retained | Open: candidate full Nix/archive/release gates, remaining catalogue migration and final costs |
| DX11 | All11 original fixed-supervisor/FIFO runtime/recovery/source-refusal journeys use the inferred path and pass both compilers/profiles/features;246 original checks preserve prepared prefix, source panic/refusal, assigned payload, shutdown and full retirement. Keyed affinity/exact rejection/unbind/joined retirement adds56 assertions, passing all eight configurations | Open: dynamic supervision, independent late completion and remaining policy/failure/final gates |
| DX12 | Occurrence/correlation owners inspected; actual duplicate-correlation panic reproduced. Behavior821380e adds complete typed rejection; owner962 tests/ten Nix gates pass. Private native guard preserves accepted prefix/suffix, original birth/replacement/replay requests and both joined children on both compilers/profiles | Registry guard passes native birth/replacement/replay, prefix/suffix and joined retirement on both compilers/profiles; the registry adoption passes all21 native gates/646 optimized tests and complete package journeys. Open: remaining creation/occurrence acceptance. Exact-recipient comparison now passes real queue/dynamic/native Entity identity and closure witnesses |
| DX13 | Locked TimerQueue generation law inspected | Open: occurrence identity experiment and timer inversion corpus |
| DX14 | Stash/Machine restrictions retained | Open: bounds audit and bounded retention witnesses |
| DX15 | Existing ActionItem/InterpretItem and standard interpreter inspected | Open: two real external lanes without whole Environment replacement |
| DX16 | Complete startup/provenance, raw/projected retirement and control/report rejection retained; actual303 owner tests pass both compilers/features/profiles. Earlier e4d2db1 had a topology failure; current registry adoption passes all21 native gates/646 optimized tests, preserving the full denial corpus | Open: generated topology/custody and complete downstream gates |
| DX17 | Retained custody accessor/product and composed send changes have bounded interface provenance; DirectoryConfig owns immutable validated limits, removing impossible family/runtime error branches and preserving affine inputs | Open: consumer/invariant audit and competing spelling deletion |
| DX18 | No new dependencies selected; existing thiserror owns the narrow configuration rejection with both original limits | Open: error embeddings, bounds and costs measured |
| DX19 | Existing child declaration pass/denial; factory-input candidate has focused compile REDs, real downstream inverse and explicit original-caller compatibility failures | Open: complete pinned diagnostic corpus, renamed/minimal consumers and final candidate gates |
| DX20 | Application and exported interfaces establish clients/senders for exact recipients and stable Entity references. Root is the default; one shared ExternalActor can select multiple typed targets. Reply inference, original-target allocation rejection/retry, exact command custody, fresh origins, prefix draining and wrapper orders pass; eight denials preserve capabilities | Open: complete correlated/cancelled/slow-customer/late-reply corpus, capacity and final gates; pre-root scope retains exact endpoints through missing-host/prepared/staging/HTTP retry |
| DX21 | Method-local inferred shutdown ingress; runners require only capabilities they execute. Both root wrapper orders, request replay, natural domain/HTTP completion and complete retirement pass7 focused tests per profile; absent/ambiguous lanes remain denied; old-source inverse denies only E0277 | Open: generated canonical access, child-derived phases/drain corpus and final repository gates |
| DX22 | One Application owner composes ordinary/cold actors, HTTP and Entity. Declared family roots derive actual actor/child custody; finite two-family, recursive and all four original cold/family recovery journeys preserve their independent receiving, launch refusal and original heaps on both compilers/profiles | Open: remaining Entity/HTTP acceptance and full gates; admission-only clone/unused reply protocol/target wrapper removed; real closed admission preserves original order allocation, five example tests pass both compilers/profiles; all38 original receiving/failure journeys plus five HTTP runtime cases pass both compilers/profiles/features |
| DX23 | Downstream crosswalk remains in appendix | Deferred: networking, durability, identity and deployment belong to later PRDs |
| DX24 | Terminal evidence remains authoritative; FIFO role-rejection consumption now preserves the exact owned failure and lifecycle product through both projection orders, inverse, native consumer and ten owner gates | Open: bounded borrowed operational inspection and telemetry failures |
| DX25 | Lossless-result regressions/inversion and 65 custody tests; canonical inference fixtures; original startup falsifier repaired in actual coupled-source runtime; owner tests and static denials preserved | Open: retain inferred API and complete downstream corpus/final gates |
| DX26 | Actual counter main98 lines, FIFO184; Entity communication removes its unused reply protocol/interface and repeated target argument. Complete independent retirement oracles remain. Exact warm costs and full tracked/untracked deltas recorded | Open: broader comparable caller/compiler/runtime measurements and final resource gates. Five paired complete counter journeys use equal77 registry dependencies:74 allocation blocks each, peak heap122916→123644 bytes (+728), unchanged end heap; no memory reduction claimed. Scope adds another measured8 peak bytes with identical allocation blocks |
| DX27 | Published Core/Actors0.27.0 and macros0.15.0 atb8307d1: ten native gates,987 tests,118 doctests, four-workspace preflight, exact CI, normal release and registry source/consumer verification pass. Bombay selects registry packages without patches | Open: factory-input candidate breaking release review, current whole-catalogue adoption and final programme gates |
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
The test-only Clippy correction and original-boxed-refusal fixture are retained.
The identical-source retry passes all21 native gates in705.439 seconds; its failed
ENOSPC predecessor remains a failure. Current registry adoption and guide/package
checks are recorded below; local owner patches are not a registry release.

#### Policy construction and startup — DX10/11/19/24/27

Factory input (DX10/19/27), law recorded before production: ordinary
`queue::new(|_| WorkerSubmission::immediate(worker), roles, config).build()` and
`roster::new(|role| submission(role), roles, config).build()` must infer the
borrowed role at the owning constructor. This is a Rust API policy, not a new
actor-model law. Reuse the existing `FnMut(&Role) -> Preparation` contract;
direct submission and fallible `Result` are its two real forms. No preparation
runs before build. Refusal still owns the original factory, prepared prefix,
refused role/reason, untouched suffix and complete configuration.
Rust's closure parameter contract is primary:
https://doc.rust-lang.org/reference/expressions/closure-expr.html.

The real Bombay FIFO replacement caller falsifies published027 with uncoded
FnMut/FnOnce lifetime-generality diagnostics. The17-file mechanical migration
was archived and restored, without adding caller annotations. Focused queue/
roster REDs precede production (101,12.663/1.610s); all original43 assertions
survive, with three added deferred-call checks. Seven focused cases pass pinned
Rust1.95 debug/release (2.781/19.045s). Fifteen actual downstream cases pass on
Rust1.99/1.96 debug/release (8.260/106.170/28.033/122.175s),807 frozen inputs
conserved through the run. The byte-identical native FIFO caller against original
owner source fails only the intended call contract (101,9.512s). Source/oracle
and gate evidence each received two independent reviews.
Extracted candidate archives preserve all220 shipped Rust sources. Ten actual
native FIFO/source-refusal/replay cases pass both compilers/profiles (21.719/
79.374/24.426/90.027s), with659 frozen inputs and62 nonowner package records
conserved. Every row compiles all three exact extracted owners and a unique
consumer in a fresh target. This is not registry publication, remote support or
an additional static-denial claim; the initial bad archive-path setup fails before
tests and receives no credit. Archive source/evidence each receive two reviews.

Compatibility costs are explicit: constructor generic arity4→5, and an exact
original annotated caller reusing its returned factory without mutable access
fails E0596. Three existing rejection tests now bind the original rejection
mutably; a breaking0.x release is required. No stronger Fn bound, adapter,
default or ownership reconstruction is introduced. Initial full nextest attempts
failed on that third caller and an accidental test-edit typo; both raw failures
remain, neither earns passing credit. The corrected whole FIFO file equals its
baseline plus one mut keyword. All987 owner nextest tests then pass (31.260s).

Aggregate checkpoint: each five-way control sum is unchanged. FIFO retains
Constructed/Operating/Draining/Stopped/ForcedRetirement; fixed supervision retains
New/Operating/ShuttingDown/Terminating/Stopped. Every existing subordinate enum
payload and both constructor bodies are byte-identical; no actor transition,
state, result, field, module or public type is added. Source-inclusive inventories
(including cfg-tests): FIFO7 files,12 sums/59 variants,358 lexical match arrows,
6522→6528 lines; fixed20 files,47 sums/179 variants,748 arrows,11177→11183 lines.
Arrows are diagnostics, not independent semantic branch coverage. Selected public
type/function/trait declarations29/231 are unchanged; they exclude constants and
re-exports. Production+16/-4/net12; tests+27/-17/net10; public types+0/-0.
`FACTORY-INPUT-AGGREGATE-V3.json` retains exact unchanged sum payloads (V1's
where-bound miscount is superseded). No new arrival history, repeated cause,
cardinality assumption, transition authority, semantic bool or structural syntax.
Cross-check: FIFO/fixed actor laws, atomic architecture/features/DevX/verification,
construction custody and declaration-order preparation. Shape disposition: pass;
final retention awaits clean Nix gates, archive consumers and release review.
The first full Nix process reports ten outputs/exit0 (251.855s),987 optimized
tests and118 doctests, but its raw nextest fixup reports classifier crash35733.
The strict wrapper rejects it; this is not a clean gate. All827 frozen tracked
inputs remain unchanged through that run. A separate build-audit branch adopts
the already-proven byte-oriented scan; no actor change or waiver follows.

Scope limits: queue correlation alone does not prove distinct identities or every
role's order; the roster direct-form case intentionally switches to immediate
startup. The two native Stash/shutdown orders initially prove runtime compatibility,
not independent wrapper-policy execution. A deliberate test-route bypass passes
both old oracles (12.615s). Adding the same-worker Released assertion passes
all3 cases on both compilers/profiles (11.295/44.702/12.499/52.605s); bypassing
Stash then fails both cases at the intended state and custody assertions in debug/
release (11.634/45.000s). The first shared-cache probe is invalid and uncredited;
the corrected six-row run uses distinct package identities, verifies actual source
compilation, and conserves794 inputs. Inner shutdown is additionally supplied by
the proxy and is not independently proved by these native cases.


Owning modules expose supervisor/roster, supervisor/directory, pool/queue,
pool/router, proxy and worker. Parent modules curate public contracts; config
children and state/settlement machinery remain private. Folder context supplies
qualification, and worker preparation uses startup. Pure core Activate retains
its existing initialization law. All five owning constructors are proven; no
compatibility constructor, universal builder or extra trait is retained.

Named Config is simultaneous policy input, not actor state. Construction consumes
it into existing flat fields. Fixed RecoveryPolicy owns strategy/limit/delay;
pool RecoveryPolicy owns limit/delay/failure. Source and permanent/transient
eligibility remain separate; temporary needs no source and immediate needs no
unused plan. Dynamic construction is infallible. Keyed refusal returns workers,
original selector and original Config: factory, prepared prefix, refused role,
reason, untouched suffix and the original diagnostic route all survive.
StableProxy::new constructs Dormant; a submitted worker supplies its startup plan.

Provenance and bounded checkpoints (full pre-edit records remain in owning PRD
commit85ac933; retained code is Behaviorbc7718c):

| Change | Law, pre-edit witness and reused machinery | Retained measurement and disposition |
| --- | --- | --- |
| Fixed recovery | API policy; real registry E0432/E0061; pure complete construction and native replacement/refusal/wrapper trace. Reuse eligibility, validated release and flat RecoverySettings. | +40/-42/net-2 production, one public product. Nested storage344→352 is rejected; flat storage344. Root New/Operating/ShuttingDown/Terminating/Stopped and recovery/source alternatives unchanged. Pass. |
| Roster/queue Config and hierarchy | API policy; actual inferred fixed/FIFO construction and full prepared prefix/refusal/drain/join; existing ordered preparation and public worker ports. | Original proof and .26 replay preserved at677b1e5/15c0536/4f87c1e. All five families retain exact types and custody. No new transition authority, runtime policy copy or placeholder. Pass. |
| Directory/router Config | API policy; pre-edit native E0425/E0422 and pure router refusal/initialization RED. Reuse old construction and ordered preparation; remove positional constructors and seven repeated rejection fields. | Production+206/-148/net58; tests+674/-372/net302; two public products. Directory9→10 modules,4199→4239 lines,26→27 declarations,84 matches/249 arms unchanged. Router11→12 modules,9051→9069 lines,20→21 declarations,151 matches/363 arms unchanged. All18 other child files and actor fields/control sums byte-identical. Original selector/route pointers and all ten effect lanes checked; optimized Fail/Retry inverse catches policy loss. Pass. |
| Pool RecoveryPolicy | API policy; real native E0432/E0061; complete FIFO replacement/customer/shutdown/join. Reuse const constructors and flat enum fields, never fixed supervision's unused strategy. | Production+31/-13/net18; tests+210/-127/net83; one public product, no module.25 modules;19711→19729 lines,47→48 declarations,388 matches/936 arms unchanged. All23 other children and private recovery tail byte-identical. All343 affected tests pass both profiles. Optimized normal-stop inverse catches zero versus one source request; restored source passes. Pass. |
| Empty proxy/startup names | API policy; inferred StableProxy::new RED E0599 before production, five construction/custody cases and complete fixed/directory native journeys. Reuse dormant construction and existing startup settlement. | Production+90/-91/net-1; tests net0; one public method renamed, no type/trait/field/default/lint exception. Modules6/25/31/6; proxy8017→8018, pool19729→19727, supervisor14831 and worker2253 unchanged. Declarations16/48/56/27 and matches/arms176/438,388/936,391/997,26/48 unchanged. All686 Actors tests pass both profiles. Pass. |
| FIFO worker rejection | Derived consuming projection; original role/source/error pointers survive both projection orders; owned inverse and native refusal/join. | Production+30/-1/net29, tests+121, one method. No new type/trait/bound/state. Eleven→12 matches and25→27 arms belong only to the projection. Every other cause returns Err(original). Pass. |

Control/value audit: Dynamic availability Accepting/ShuttingDown owns the selected
shutdown policy or deadline. Its14 entry phases retain only current submission,
proxy, issued operation/witness, readiness or exact retirement. Service Ready/Empty
owns worker/readiness or previous attempt; change Committed/Cancelled owns current
disposition. Proxy shutdown AwaitingSettlement/Accepted/Rejected owns current
witness/operation or no authority. Keyed Constructed/Operating/Retiring/Stopped/
ForcedRetirement and FIFO Constructed/Operating/Draining/Stopped/ForcedRetirement
retain their prepared roster, current worker/queue/bindings, deadline and terminal
cause. Source Available/AwaitingReturn owns the capability or its outstanding
return; PrepareWorker/WaitForSource/RetireRole/StopPool is the existing decision.
Proxy Dormant/Starting/Ready/EmptyInitial/EmptyAfter/Replacing/ShuttingDown/Stopped
retains exact worker, attempt, predecessor, creation, shutdown and retirement.
WaitingForStart/Running owns the same startup grant; pending/returned departure
joins retain all affine outcomes. Existing subordinate alternatives and payloads
are source-identical. None becomes another Behavior or dispatcher.

All retained policy batches cross-check fixed/dynamic/keyed/FIFO/proxy laws,
preparation, total settlement, initialization and retirement. Residue scans find
no added arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, structural caller syntax or compiler-invented bound. Existing
complete bodies and hashes, rather than another state label, establish unchanged
subordinate values. All postcheck dispositions are pass; publication stays open.

#### Creation correlation and registry binding — DX12/16/21/25/27

Creation correlation is not identity or replacement provenance. Fresh allocation
is the research law (Agha et al.1997 pp19–20); creator-local correlation and
rejection before allocation are derived/project policies. Bombay3a80848 uses six
executable guard lines in the existing map. Duplicate correlation returns the
original RoutedCreation with CorrelationAlreadyRecorded, preserving ID, child,
kind and route. Accepted prefix and later sibling still execute and retire.
No failed replacement yields a successful restart.

RED0.25 lacks the typed reason; RED0.26 reproduces the occupied-map panic. Native
birth/replacement/replay and both joined children pass current/MSRV both profiles;
the optimized defect inverse restores that panic. CreationBinding stays Starting,
RetiringStartup, Established, Rejected, StartupRejected. Those own original task/
control, raw/projected retirement, installed endpoint/join, no child, or explicit
kind/address/route and failure lanes. StartupRetirementCustody Raw/Projection and
ChildAuthority Established/Unavailable retain their existing values. Driver and
all24 retirement alternatives remain unchanged; no subordinate transition engine.
Creation file2780→2789 lines,16 matches,8→9 conditionals,47 arms,3 enums and3 public
declarations; child file1066 lines/10 matches/18 conditionals/31 arms/3 enums/16
public declarations is byte-identical. Production+11/-2/net9; all adoption tests
+334/-5/net329; no new production module, retained field or public type. Pass.

Actual source207 owner Rust files match published98983b4 (Core/Actors0.26.0,
macros0.14.2), un-yanked with all tags and normal successful release. Workspace161,
fuzz29 and packaged140 registry identities change only selected owners. The
manifest binding regression first fails0.23 versus actual0.26, then checks both
Driver/template headers against locked offline Cargo metadata. Both manifests
retain the exact45-template/19-capability inventory. Ten manifest tests pass
current/MSRV both profiles; test-only+46/-5/net41, no production/API change.
The deliberately interrupted426.907-second stale-binding run is not a pass.

#### Verification, costs and delivery — DX19/25–28

Build-output audit, pre-edit: the actual factory-input owner full check reports
all ten outputs but its nextest fixup classifier crashes (PID35733); the wrapper
correctly rejects that nominal exit0 (251.855s). This is a build policy: every
output scan must finish and propagate enumeration/read/header/search/cleanup
failure while rejecting forbidden wrapper/RPATH references. No actor law changes.
Reuse Bombay's already-proven byte-oriented audit and50-control test; replace the
existing hook implementation without adding a second hook. First run that copied
test against this owner's exact pinned old hook and require the failed-enumeration
witness. Expected four files: flake, audit source, existing-style test script and
this owning record; about91 tooling lines/136 test lines, zero Rust or public type
changes. All actor control states, subordinate values, transitions, cardinality,
provenance and ownership stay byte-identical to publishedb8307d1. Recheck source
hashes and normalized architecture/feature/verification laws; aggregate disposition
awaits focused controls, integration, inversions and a fresh full eleven-check run.
No package publication or general Darwin crash root-cause claim follows.
Scope checkpoint before the next edit: commonArgs does not reach the four
existing fmt/TOML/audit/deny checks. The same owned audit must be selected for
all eleven workspace checks; upstream dependency hooks remain separate. First
assert exact evaluated preFixup source selection for every check, then reuse
commonArgs.preFixup in those four existing constructors (about four more lines).
The configuration regression first names exactly those four missing checks;
all eleven then select the byte-identical store audit. Pinned Crane73b9805
forwards preFixup through the four constructors to stdenv; no global hook is added.
The pinned old-hook test fails on enumeration37→0 (2.835s), while50 direct
controls pass (5.943s). Six copied-source inversions fail for their intended
status/header laws (matrix7.865s). Normal owner-pinned stdenv fixup accepts the
healthy output (1.743s) and rejects the exact forbidden wrapper (1.250s). These
are separate from the new focused Nix derivation and required eleven-check run.

Aggregate disposition: pass for this build-only representation. All323 Rust
files and all tracked locks match b8307d1. Every actor state/subordinate value,
transition branch, policy, protocol spelling, creation provenance and custody
representation is unchanged; zero actor-state or public-type additions/removals.
Tooling+96/-3/net93; tests+136/-0; four files. The serial scan owns only its
checked temporary file and exact process statuses; it retains no history and
registers no second hook. Each temporary file is removed on completed/error paths
unless cleanup itself fails, whose original failure is reported. Exact four-byte
headers intentionally fix the old character-read near-NUL/newline classification;
ELF port tests use mocked patchelf, not real ELF parsing. Original upstream and
other platforms remain outside any Darwin-root-cause claim. Cross-check actor
architecture/features/verification and the existing construction/retirement laws.
Raw sources/results: OWNER-REFERENCES-{COMPLETE,INTEGRATION}-*.json and
OWNER-REFERENCES-SELECTION-{RED,GREEN}.json. Two independent reviews inspect
source and raw controls; full-gate and CI receipts remain required for delivery.


| Exact source | Executed verification | Scope limit |
| --- | --- | --- |
| Bombay3a80848, registry owners98983b4 | All21 fresh native gates in2306.566 seconds;646 optimized tests plus one existing ignored subprocess case; all459 source hashes match. Current/MSRV debug/release packaged correlation/fixed/FIFO/guide journeys;61 production and195 other packaged Rust files match. | Engine and Bombay macros are extracted unpublished packages; not registry-only Bombay publication. No signing/disk waiver. |
| Behaviorbc7718c | Separate pinned nextest969 tests in40.893 seconds; all ten native Nix gates in253.184 seconds including969 optimized tests, denials/macros, rustdoc, Clippy, formatting, dependency and package checks. Actual archive preflight11.072 seconds; four-workspace release preflight6.140 seconds.143 packaged production files match. | Current candidate is draft PR98, semver-breaking; CI/review, policy publication and actual registry migration remain open. |
| Ownerbc7718c archives + isolated actual3a80848 consumer source |24 commands execute28 complete fixed/prefix/both-wrapper/FIFO/directory/keyed journeys on1.99/1.96 debug/release in305.341 seconds, with original independent refusal and retirement oracles. | Five worker-owner/startup consumer files adapt the actual runtime; Entity stays byte-identical. Local patches do not prove registry delivery or full migrated Bombay gates. |

Actual counter main98 lines, FIFO184; equal77-dependency paired counter journeys
use74 allocation blocks each, peak heap122916→123644 bytes (+728), unchanged end
heap. Scope adds8 measured peak bytes with equal blocks. Sample retained actors
fixed/directory/FIFO/keyed remain344/152/272/264 bytes. Directory constructor
arguments6→1, router10→4, automatic pool recovery4→2. No memory reduction, general
comparison with erased systems or throughput improvement is claimed. Broader
comparable compiler/runtime/resource measurements remain open. Memory is the
user's priority; binary size is secondary.

Portable evidence is the retained tests, exact source/package hashes and linked
CI; raw commands/counts/inverses are under
/private/tmp/bombay-retirement-schema-kgazqjhv/worker-factory-source/policies-026
and its parent. Historical full pre-edit evidence is preserved in commit85ac933,
not restored as chronological files. Rust always runs in pinned Nix. Linux and
x86_64-Darwin are unexecuted locally. Prior signing and ENOSPC failures remain
failures, never permanent waivers; preserve other AI/global caches.

Existing authorizations cover expanded cumulative budget (2026-10-09) and the
single topology declaration (2026-10-11); do not ask again for those scopes.
PR98 remains draft until existing16-document pruning and this concise owning
record are integrated and gates/CI pass on its new exact head. New policy merge/
publication needs concrete review and authorization; existing PR-specific release
approvals do not authorize it. No selected open criterion is closed by this
record. Networking, durability, global identity and deployment remain explicit
DX23 deferrals; all other open rows continue under the persistent active goal.
