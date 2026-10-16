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

Judge the ordinary path from a junior developer's perspective: meaningful
policy names, one clear entry point, and understandable failure recovery.
Use short, familiar member names when the receiver or module supplies the
context (`stash.take()`); add words only to distinguish real meanings.
Advanced access should let a caller peel one ownership layer at a time without
requiring that machinery for ordinary application setup.

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

The isolated Bombay registry branch at1a8f20b selects registry Behavior/Actors0.27.0,
macros0.15.0, Address0.3.0, Communication0.1.6, Timers0.2.0, Tokio1.53.1 and
Axum0.8.9, without repository patches. All220 shipped owner Rust files match
published sourceb8307d1. Its catalogue migration still needs the reviewed
constructor inference repair50348d7 published through the normal workflow;
source-selected consumer proofs are explicitly separate from registry delivery.
Bombay mainf25e4a0 and timer PR383 still select owners026/142. The timer
branch's25 fresh native gates pass; its scope does not certify the newer
catalogue branch. The single checklist below owns remaining acceptance.
Earlier registry/private-patch results retain their exact revision limits.

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
same-actor extension method is selected for a standalone actor. Use `MessageAdapter::new(destination, map)` for the standalone protocol hop,
with its concrete destination determining routing.
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
Zenoh is Bombay's sole internal network transport. Application authors create
actors, write business logic and compose typed actor systems; they need no
Session, Query, SDK configuration, transport selection or raw connection access.
Power-user layers expose Bombay capabilities. Mnesis owns persistence and
mnesis-bombay owns integration; these requirements are not claims of completed
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

Every new or changed test requires two explicit verifications. First map its
inputs, complete observable outcomes and retained ownership to the stated law;
check that its oracle is independent of the implementation. Second challenge
that mapping with counterexamples: a wrong implementation that could still pass,
missing phases or failures, circular assertions and vacuous success. Use an
independent reviewer when available. Prove an applicable original defect or
owning-source inversion fails for the intended invariant, then verify restoration.
Repeat execution on the required compilers/profiles, but do not count repeated
green runs, assertion totals or preserved syntax as either semantic review.
Record practical untested limits explicitly; a surviving counterexample reopens
the test and the acceptance claim it was meant to establish.

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
| DX01 | One Application owns roots, members, scope, HTTP and Entity. Joined/split Node callbacks are actor-only; protected consumers adopt that contract at6c5c18a. Recursive, supervisor, pool and cold journeys execute without composed-type annotations. | Open: complete selected semantics and final gates; hide remaining SDK configuration/query surfaces. [Bombay PR375](https://github.com/devrandom-labs/bombay/pull/375) merged as5e9bcd1 after all six checks passed; programme acceptance remains open. |
| DX02 | One topology declaration derives spaces, occurrences and custody for born/cold/recursive actors, including equal protocols in distinct roles. Missing/wrong hosts, roles and births remain denied. | Open: complete host/caller corpus, canonical catalogue reconciliation and final gates. |
| DX03 | Nominal role owners close the tested recursive retirement proofs. Native owner/topology tests, cancellation custody and static authority/occurrence denials run on supported compilers. Strict compiler-specific diagnostic replay retains source/coverage controls. | Open: latest-main reconciliation, all owner consumers, remaining cancellation/freshness acceptance and final gates. |
| DX04 | ApplicationOutcome joins work, native retirement, notifications and families; consuming completion returns value plus ApplicationRetirement or the original whole report. Native receiving migrations preserve original assertions; custody-loss inversions fail and restoration passes. | Open: proportional failure recovery, complete family-disposal/operational inspection, final-source minimum-compiler/archive/full gates. |
| DX05 | Runner types derive from root/members; current-thread/multithread, original-input recovery and independent receiving witnesses retain caller-local non-Send work. All38 original cancellation/host journeys pass their recorded matrices. | Open: final complete consumer and repository gates. |
| DX06 | Validated actor/client MailboxCapacity propagates through root, births, cold members and Entity. e629464 rejects native-unsupported sizes before allocation, retaining the original request; boundary/native tests pass both compilers/profiles. Saturation returns originals. Senders drop unused reply storage (176→48 bytes; three constructors each8→0 allocations). Capacity2 reduces measured client peak heap at equal work. | Open: complete resource accounting, final-source/archive gates and comparable package/compile/runtime measurements. Existing measurements cover their recorded consumer and host only. |
| DX07 | The existing actor/worker expansion remains authoritative. Pure births, aliases, renamed paths and a self-creating Entity actor execute against published macros0.14.2; empty-alias fix91/97 is delivered. | Open: complete generic/private/public, sibling-target and duplicate-role corpus. |
| DX08 | Published owners preserve selected send lanes through wrappers and static denials. PR104 reached main through merged PR102 (its a1772ab tree equals tested5b24f03) and extends FIFO/keyed construction to ordinary named completion/business products, with four retained denials, five native cases across supported compilers/profiles and actual packaged consumers. | Open: complete named-lane catalogue and unrelated-template/order evidence. |
| DX09 | ActorExt adds the exact owning FinalizeOnShutdown and EstablishedTerminationMonitor. Missing-method regressions, order/effect/refusal witnesses, archive consumers and recorded native gates pass. | Open: remaining decoration/catalogue audit and final combined-source reconciliation. |
| DX10 | Owning roster/directory/queue/router constructors use Config and explicit recovery. Registry270/150 supplies startup closures and reply-lane policy. All338 library tests pass both compilers/profiles with1013 original assertions;31 resumed caller tests and real examples have source-selected503 proofs. Startupf244566 removes its named plan/alias. | Open: publish the reviewed inference repair, verify registry-only callers/nested locks, audit policy necessity/names and complete costs/full gates. Breaking source costs remain explicit. |
| DX11 | Inferred fixed/FIFO journeys preserve immediate/async startup, source refusal/panic, prepared prefix, assigned payload and shutdown. Keyed tests preserve affinity, exact rejection, unbind and joined retirement. | Open: dynamic supervision, independent late completion, remaining preparation/policy/failure cases and final gates. |
| DX12 | Published0.26.0 returns complete duplicate-correlation rejection. Actual birth/replacement/replay witnesses retain accepted prefix/suffix and joined children; exact recipients preserve real application/Entity identity after closure. | Open: remaining creation/occurrence acceptance, including transitive/wrapped/rejected births and relational identity evidence. |
| DX13 | Four no-ID timer wrappers, typed request custody and matching runtime are implemented on feature branches. PR109/6488965 passes11 fresh native gates and14 current CI checks; combined archives run112 consumer tests and16 examples across both compilers/profiles. Default Bombay workspace passes782 outer tests/132 UI fixtures; selected remote426 outer/20 UI per profile and both child-feature harnesses pass on1.99/1.96. Named replacement and memory costs remain explicit below. | Open: authoritative Bombay and protected nested gates, remaining acceptance cuts and breaking registry delivery. Candidate/source/archive proofs do not certify the complete programme. |
| DX14 | Stash/Machine restrictions remain. PR99 head403a4f1 moves the sealed infallibility denial to construction. Owner 5068f6a adds `stash.take()` for original FIFO custody; focused owner/wrapper/independent-Machine tests and real Bombay shutdown pass, with isolated loss/order/sender inversions rejected. | Open: complete current Bombay catalogue adoption, bounds audit and bounded-retention/overflow witnesses. |
| DX15 | Native b1c6781 adds actor-owned interpreters through Application/topology using existing guarded startup. Real file/environment requests, wrapper orders, root/descendant/cold/Entity configuration, rejection/panic/cancellation and disposal witnesses pass recorded matrices and inversions. Independent owner diagnostics retain source/coverage controls. | Open: final integrated runtime/repository/archive gates, configured-service costs and delivery. Earlier child-oracle/work-entry falsifiers and their corrected evidence remain bounded; focused passes do not close the programme. |
| DX16 | Startup/provenance and complete source/interpretation custody remain. PR100 merged as18653a7 supplies OwnedItem from existing settlement metadata; real IO/panic custody, mutations, privacy denials, owner gates and extracted consumers pass. PR101 head184354b fixes rejected reply custody:970 owner tests, ten native Nix gates, four-workspace preflight and locked real-runtime failure/retirement proofs pass. It merged as dda4a42 under explicit authorization; reviewed release PR103 merged as13951b8 and normal publication of Behavior/Actors0.26.1 is verified in both registries and release tags. | Open: complete Bombay270 catalogue migration, two-template/wrapper composition, native integration and final downstream/cost evidence. Different custody equations remain distinct. |
| DX17 | Shared retirement custody and semantic send access have bounded interface provenance. DirectoryConfig validates immutable limits once. PR110/7644735 removes the duplicate adapter spelling while preserving its implementation; complete native failure/retirement, both route alternatives, static denials, extracted packages and eleven owner Nix gates pass their recorded matrices. | Open: export/consumer/invariant and bounds audit; remove competing spellings only after comparing complete semantics. |
| DX18 | No new production dependencies selected. Existing thiserror, standard owned sums/products and validated limits remain the chosen tools; bounded registry/language comparisons are in the evidence appendix. | Open: error-embedding/source/visibility/bounds equivalence and dependency/MSRV/feature/compile-cost audit. |
| DX19 | Strict124-fixture replay passes both compilers:19 positive/105 negative,29 suites,25 targets and107 outer tests plus the exact parent-invoked child. Canonical child denial now covers both feature configurations without duplicate source; unchanged verifier, compiler/fixture/source/patch controls and two independent reviews pass. | Open: remaining full minimal/renamed/remote/archive coverage and final programme gates. Earlier failed/incomplete runs remain failures; post-run patch equivalence has separate evidence. |
| DX20 | Application directly establishes root/selected-recipient/Entity clients and senders. Nonwaiting exact admission preserves original Full/Closed commands; origin inversions fail. Shared external customers and pre-root scope retain target authority and original inputs on retry. | Open: complete correlated/cancelled/slow/closed-customer/late-reply corpus, capacity and final-source minimum-compiler/full gates. Admission remains distinct from business completion. |
| DX21 | Shutdown ingress is inferred at the operation; runners require only capabilities they execute. Root wrapper orders, request replay, natural completion and complete retirement pass focused tests; missing/ambiguous authority remains denied. | Open: generated canonical access, child-derived phases/drain/deadline/forced-retirement corpus and final repository gates. |
| DX22 | HTTP/Entity use the same Application and joined outcomes. Awaitable retire_entity preserves exact-slot completion, cancellation, reactivation and native/callback failures; HTTP removes unnecessary clones and receives ApplicationHandle directly. Original receiving/HTTP/family tests retain their assertions. | Open: callback/associated-type distillation, complete family/root cleanup coexistence and final complete consumer/repository gates. |
| DX23 | Zenoh is the sole internal network. Both Node Work callbacks receive only typed actor capabilities; native client/requester test drivers live under Node. Raw configuration/export/query surfaces remain unfinished. | Deferred: networking, durability, identity and deployment products stay with later PRDs. Open interface obligation: hide SDK types without a transport escape hatch; preserve typed extension and complete outcome laws. |
| DX24 | Terminal evidence stays authoritative. FIFO rejected-role projection preserves its exact failure/lifecycle product through both orders, inverse and native consumer. | Open: bounded borrowed operational inspection, telemetry-failure independence, sensitive-payload policy and bounded diagnostic dimensions/retention. |
| DX25 | Protected actor-only consumers conserve original custody; provider and current catalogue retirement matrices have source/effect reviews and independent failure controls. Freshb8 full-gate evidence is separate from earlier runs with masked classifier crashes. | Open: the observed optimized original-work-retry hang remains unexplained; complete final registry-only protected consumers and gates. Passing retries or repaired output headers do not explain that hang or establish compressed-content/permanent Darwin repair. |
| DX26 | Recorded examples remove structural assembly; counter51→36 main lines, pool191→90, supervisor332→96 main plus39 domain lines. Assertions move to owning tests. Frozen default-feature counter comparisons show equal allocation/heap products with the interpreter seam; sender/client measurements improve their bounded cases. | Open: full comparable caller ceremony, compiler/diagnostic/macro costs, layouts/binary metadata and runtime memory measurements; configured-service, remote and latency costs remain unproved. Memory outranks binary size. |
| DX27 | Actual registry Behavior/Actors0.27.0 and macros0.15.0 match verified tags, checksums and220 shipped Rust files. Bombay05ca9db adopts them without changing397 root nonowner records;4086cbd integrates mainfbb5c74. Existing pool, supervisor, protected retirement and complete static consumers have source-bound evidence in the sections below. Timer branchb8c2e6f passes25 fresh native checks against its actual026/142 graph; candidate-selected catalogue tests prove their stated scope separately. | Open: owner inference-repair publication, current registry-only catalogue and nested-lock adoption, final consumers/features/costs, delivery docs and full programme gates. A local candidate or older graph cannot certify final delivery. |
| DX28 | Historical pruning is retained. One compiled application guide runs in workspace/extracted packages across recorded configurations; local links resolve. This table is the sole DX01–DX28 disposition checklist. | Open: compact contract/evidence reconciliation, compiled sample/support tables, rendered/external links, final documentation gates and retirement of this PRD/appendix after obligations have owners. |

### Retained contracts and evidence

The checklist above owns dispositions. This section states current contracts,
source-bound evidence and known limits once. Complete pre-edit laws, aggregate
measurements, inversions, failed runs and reviews remain in immutable
commitff4b4ba and the retained executable tests. They are not another current
ledger. Raw receipts are under `/private/tmp/bombay-retirement-schema-kgazqjhv/`;
tests, reviewed commits and CI provide portable evidence.

#### Application composition and topology — DX01–05/16/22

One Application owns ordinary roots, declared children, cold members, HTTP and
Entity families. One user-authorized topology declaration derives structural
spaces, occurrences, execution products and recursive retirement. Role owners
close recursive native proofs and distinguish equal protocols in different
roles. Actual decorated birth rows remain checked; no placeholder host, empty
alias, erased protocol or second actor algebra is accepted.

The declaration belongs to the application, allowing one actor under different
closed graphs. Initialization precedes mailbox work. Fresh allocation, committed
binding and explicit replacement provenance remain separate. A requested restart
is not a committed restart. Native startup, completion and cancellation retain
their original actor, capabilities, independent work and failure products.

#### Terminal custody and application capabilities — DX04/17/20/21

ApplicationOutcome owns independent work, native retirement, notification and
family reports. Borrowing consumes none; completed-work selection returns its
value and ApplicationRetirement, while every other phase returns the original
whole outcome. Cleanup is already performed by execution. Independent receiving
preserves publication/join ordering and caller-local borrowed or non-Send work.
No clone, second lifecycle engine or transparent tuple wrapper establishes this.

Application establishes clients and senders for its root, selected capabilities
and Entity targets. Receive authority remains affine; send-only customers have
no reply mailbox. Exact admission returns original Full/Closed commands and is
distinct from business completion or joined retirement. Cross-application target
association, cancellation, accepted prefixes and sender identity stay explicit.

Shutdown ingress is inferred at the operation; runners require only what they
execute. ActorExt exposes the existing finalization and termination-monitor
owners. Outer wrapper precedence remains observable: outer Stop consumes
shutdown; outer Finalize runs its reaction and forces stop. The reaction keeps
all sends and births, and rejection retains independent outcomes.

Coverage limits remain material: accepted settlement classification/count is not
exact receipt/payload custody. Older normal-retirement projections discard actor
state or settlement fields; the incoming stashed-command witness lacks exact
recipient inspection, and a ledger-prefix witness does not track every owned
Box. Pairwise fresh addresses, proxy/child correspondence and exact action-row
partition are not universally checked. The five pool cases use u64 jobs,
not affine job inputs; a closed auditor is not a closed parent-report recipient.
Keep original-work refusal, parent-report
closure and affine job input distinct from unrelated delivery failures.

#### Mailbox validation — DX06

Successful `MailboxCapacity::new(minimum)` must fit the selected native mailbox
before resource allocation; refusal retains the original minimum. This is
derived runtime validation policy, not an actor-model capacity law.
Communication0.1.6 requires the rounded power of two below2^31, so the largest
valid minimum is2^30. No public native validator/limit exists. Bombay now uses
one private maximum in its existing guard and actionable error text. The
checked rounding chain, NonZeroUsize product, default, public types and native
construction path remain unchanged. This is no available-memory guarantee.

The pre-edit law/budget at3bc6579 precedes production. At registry1a8f20b both
profiles incorrectly accept1073741825 as2147483648. Independent native
Scope→ExternalActor→mailbox_channel controls reach the primitive's exact panic
before ring allocation. Those two expected failures/two expected-panic controls
remain historical evidence only; eight supported pairs pass without allocating
huge mailboxes. Both independent audits bind1254 unchanged inputs and actual
registry Cargo files without patches. Receipts: `mailboxes/PREEDIT-*`.

Fixe629464 on isolated `codex/devx-mailbox-limits` preserves the complete original
requested minimum. Six client and six root/child/cold/Entity cases pass Rust1.99/
1.96 debug/release (48 executions). These retain the existing bounded payload,
refusal, retry and retirement assertions; they are not exhaustive all-field
custody evidence. Current strict Clippy and formatting pass. Doctests pass six
with three existing ignores, retaining denials for private construction and
raw-integer actor/client capacity selection. All628 bound inputs remain exact
through the matrix and both independent reviews pass. Sources/results:
`mailboxes/FINAL-*`, including the four-profile terminal checkpoint; an earlier
three-profile checkpoint is intermediate only.

Four Bombay paths change: production+10/-4/net6; tests+35/-0; public types+0/-0.
There remain one production module, two public types, no control/subordinate
state sum, the same two Result alternatives and one explicit validation if.
The raw substring count including rustdoc was corrected; it does not count
implicit compiler branches. No repeated cause, lifecycle authority, late check,
semantic flag or structural caller syntax is introduced. The primitive,
application capacity and Driver contracts cross-check unchanged; source/model
checkpoint:pass. The actual registry graph remains exact (270/270/150,
Address030, Communication016, Timers020; workspace Observe). The complete
Bombay Nix run fails on cleane629464; its exact disposition is recorded below.
Expanded resource/cost accounting and final combined-source delivery remain
with DX06/26. Ready owner PR heads remain unchanged.

#### Effects, preparation and worker factories — DX07–11/15

Behavior remains the pure transition owner; Actions carries named communications,
fresh creations and next behavior/termination. The actor facade delegates to the
owning macro. ActorExt is for same-actor decoration; owning constructors build
standalone policies. Named send selection survives unrelated wrapper orders.
OwnedItem exposes the standard whole-item receiving loan. Native IO, panic,
rejection and cancellation witnesses separately check outside custody; normal
settlements are not early actor errors.

Actor-owned native services reuse constructor authorization, existing requests
and interpreter composition. Absence grants no external IO, including a request
whose interpreter is unit; explicit `|| ()` is a meaningful positive. A shared
Fn+Sync constructor promises an actor-owned returned instance, not immutable
captures or exclusive backing resources. Original grant/permit and outside
custody remain authoritative in production; a cfg(test) panic catcher is not
proof of production cancellation custody.

Source preparation, worker startup and completed readiness are distinct.
Initialization settles before the startup permit. Owning startup closures defer
work to the same native task as named plans; exact Ready/Rejection types survive.
Positive closure counts prove construction/body entry, not every poll, arbitrary
uninvoked disposal or every panic/cancel/reject cut. Startup notice precedes
committed Ready; that notice alone cannot justify a one-query readiness promise.

#### Protocol adapters — DX17

Use `MessageAdapter::new(destination, map)` for logical, established and mixed
reply destinations. This derived API composition policy renames the existing
route-aware owner and deletes its logical-only alias; it adds no actor effect,
route lookup, bound, wrapper or capability. The mapper, destination, empty
initialization, NoBirths, Continue decision and interpreter remain unchanged.
Rejection owns the mapped message and original destination; unread original
inputs remain in retirement. A mapper cannot reconstruct an input it consumed.
Explicit logical types become `MessageAdapter<Input, Recipient<Protocol>>`.

The pre-edit law/change budget is committed at3e14ad0. At owner6488965 /
Bombaycf04f14 the short constructor fails with E0308 at both established
destinations; changing only three adapter identifier tokens makes the same two
native journeys pass debug/release. All2706 bound inputs stayed unchanged.
The strengthened oracle independently receives the original mapped allocation,
accounts for all three original allocations exactly once across rejected mapped
payments and unread invoices, and checks exact destination/reason/sender/value,
every native custody field, one empty terminal Stop, cleanup and both
notifications. Three admissions exceed the verified two-slot mailbox, requiring
the first input's processing before shutdown. Earlier mistaken test assumptions
about owned enums, accepted-unit discharge, shutdown and rounded capacity remain
failed controls. Both independent source/oracle audits pass; raw provenance is
`adapters/TERMINAL-PREEDIT-{SOURCES,RESULTS}.json`.

Owner7644735 / [PR110](https://github.com/devrandom-labs/bombay-behavior/pull/110)
passes all11 fresh aarch64-Darwin Nix gates:993 nextest,120 doctests, zero
skips/ignores and50 output-reference controls;832 owner inputs and HEAD remain
exact. A separate online audit reports65 dependencies, zero findings, database
479c888 of2026-10-16; sandbox yank-lookup failures are not credited as online
coverage. The two logical unit cases and43 complete integration cases pass
both profiles. Mixed-route tests prove constructor/dispatch and real interpreter
trace; `Interpretation::Complete` alone is not receipt-acceptance evidence.
Existing child-route and wrong-protocol compile denials remain executed.

The retained native pair passes Rust1.99/1.96 debug/release, with strict current
Clippy and formatting. The original Clippy run failed six style checks; narrow
item-local long-test expectations and four equivalent borrowed empty-slice
assertions preserve all actions and oracles. A fresh matrix binds those final
spellings;2082 inputs remain exact. Eleven unchanged Engine inventory checks
also pass each compiler/profile on the separately recorded pre-lint source;
these44 checks are not native custody journeys. Actual extracted7644735 archives
match all223 shipped Rust files and247 regular archive members. Both native
cases pass all four configurations, each freshly compiling all three extracted
owners and Bombay;2332 inputs remain exact. Both independent audits pass for
each source/receipt scope. Sources/results: `adapters/FINAL-V2-*`, `ARCHIVE-*`
and `results/ADAPTER-7644735-FULL-NIX.*`. None establishes registry delivery.

Owner delta across eight files: production+10/-17/net-7; tests+46/-5/net41;
public types+0/-1 (one alias removed, existing concrete owner renamed).
Three existing production modules remain; control/subordinate states and
transition branches stay zero; the only owned values remain destination plus
mapper. No arrival history, repeated cause, semantic flag, nested authority or
structural caller path is added. Existing composition/lifecycle/normalized
atomic laws remain unchanged; source/model checkpoint:pass. Two existing
integration targets migrate only names/type indices. The actual Bombay witness
adds340 test lines and one inventory constant update, with no production change.
Native tests exercise shutdown decoration only, not arbitrary wrapper orders.
PR107/108/109 retain their reviewed heads. Exact-head CI38931592499 passes all11
jobs through the normal dispatchable workflow;
PR-only checks still require the eventual main base. Normal publication,
registry adoption and the broader DX17 catalogue/bounds audit remain open.

#### Stashed-message recovery — DX04/14/17 pre-edit law

A retired stash owns original mailbox inputs, not just a queue length. The
smallest caller recovery should be `let pending = stash.take();`:
move every currently held `User` in FIFO order to the caller, leave the stash
empty, and preserve its inner behavior and routing policy. A second take is
empty. There is no replay, transition, effect, cloning or new lifecycle
authority. This is derived ownership access to existing state, not a new
actor-model operation. Machine's transactional Clone requirements and Stash's
sealed infallibility/phase restrictions stay unchanged.

Inspect the actual Stash/StopOnShutdown owners, pure initialize/delegate ports,
Driver retirement, ActorCustody and ApplicationRetirement. The current public
stash exposes counts only. A two-part `into_parts` would discard its route;
returning the function pointer as a third tuple element burdens this recovery
case. Standard `mem::take` of the authoritative VecDeque preserves all owners
without another type or queue vocabulary. The existing shutdown `into_inner`
supports deliberate ownership peeling. Do not add Active unwrapping or a
second actor interface to reach the stash.

Use isolated `codex/devx-stash-custody` worktrees from owner 7644735 and combined
Bombay 1bf7f328. Actual dependencies remain Behavior/Actors 0.27.0, macros 0.15.0, Address 0.3.0,
Communication 0.1.6, Timers 0.2.0 and workspace Observe. Private owner 764 selection is required only for the
already-retained owned-timer runtime; keep 397 nonowner records exact and do not
claim published candidate APIs. Before production, a focused real Application
consumer must fail at the missing method. Its native shutdown journey must
recover a nonempty held prefix plus unread runtime suffix, exact original
senders and move-only allocations, all retirement fields, cleanup, notifications
and terminal Actions. Pure cases cover both shutdown/stash orders, a second
unrelated existing template, empty/repeated takes and unchanged replay/stop.

Expected owner scope: stash.rs, one focused test, its existing recipe/rustdoc
and changelog; approximately 20 net production lines, one new method and no new
public type/trait. Bombay adds its native test and coordination only, no
production. Baseline stash.rs is 206 physical lines, one module, two public
types/two traits and 10 qualified public declarations. There is no stored control
sum; StashRoute's three transient choices, the inner actor and the original
FIFO remain the only values needed. `take` uses the stash receiver as its
context; it takes the held messages, not the wrapped actor or unread runtime
mailbox. The earlier `take_messages` compile failures remain valid evidence of
the ownership gap; repeat the pre-edit proof for the selected spelling.
There are three match, two if and one while
selection keywords. Retained representation must add no state, field, enum,
selection or generic parameter; only the recovery method changes public access.
Record measured drift, two independent reviews and source-bound failure
inversions before retention. Cross-check Behavior layer laws, all four normalized
atomic documents and existing replay/property/model tests. Bounded retention
and explicit overflow policy remain required separate work; this recovery API
does not satisfy them or the full DX14 programme.

Focused result at owner ee9516d: production+17/-0, tests+208/-0,
docs+21/-1 across four paths; public types/traits+0, one method added. The old
stash implementation is byte-identical after removing that method's impl.
Physical stash lines 206→223, public spellings 10→11, modules 1→1, stored control
states 0→0, route choices 3→3 and selection counts 3/2/1 remain. Every existing
field, constructor, transition/init/replay body and bound remains unchanged;
no new state, history, cause, nested authority, semantic flag or caller alias.
The selected receiver supplies context: one `stash.take()` returns the standard
VecDeque of original Users, with no additional retained field.

The source-bound pre-edit proof fails only at take: eight owner and two native
E0599 diagnostics per debug/release configuration. The focused positives pass
three pure cases in both owner profiles and one native journey on Rust 1.99/1.96
debug/release. That native case accounts for a nonempty held prefix and unread
suffix, three original move-only Box allocations, two independently allocated
senders and every retirement/notification/cleanup lane. A state-responsive
Machine case proves future outer replay after take, not only empty release.
Both independent reviews pass after strengthening that oracle. Isolated empty,
reverse and sender-corruption bodies fail in both profiles (14 intended
assertion failures; four unaffected Machine control passes). The initial wrong
import and too-literal diagnostic matcher remain uncredited; the first shared
build-directory inversion campaign also fails its harness when a release run
uses the baseline executable path, so only fresh distinct mutant targets earn
control credit. Do not infer a compiler/model defect from that first campaign.
The first owner full run at ee9516d fails 10/11 in 220.192s on a generated guide
link; the source file existed but its rendered relative URL was invalid.
Docs-only 5068f6a replaces that URL with the immutable source commit. Its full
pinned Nix invocation passes all 11 native gates in 178.859s (nine built, audit
and TOML cached): 996 nextest/0skipped,120 doctests/0ignored and 50 reference
controls. All 833 files and HEAD remain exact/clean; all 11 completion outputs
have nonnull narHash and the exact evaluated deriver. No reported signing,
classifier or disk failure; other three architectures are omitted. Do not
claim a fresh online audit from the cached audit gate.

Existing package preflight at 5068f6a passes in 6.146s with exactly two independent
consumer workspaces; its four existing dead-code fixture warnings and intended
incompatible-macro patch warnings remain. Three immutable packaged archives
contain 248 regular files/224 Rust files matching the committed source and clean
VCS metadata. The real Bombay 95499e2 journey passes the extracted packages on
Rust 1.99/1.96 debug/release in 123.988s, preserving 397 nonowner lock records and
all 2,342 frozen inputs, including tools and archive bytes. Current strict Clippy
passes in 8.210s; actual workspace formatting passes in 5.014s. These observed
build times are not a causal compile-performance comparison. Production field
storage is unchanged; no full runtime heap/latency improvement is claimed.
Both independent terminal reviews pass. Exact-head Linux CI38945141204 passes
all11 jobs. PR #111 merged into feature PR #110 as2d23f11; PR #110 merged into
timer PR #109 as5a29896. These feature-base merges do not establish main or
registry delivery. Actual Bombay adds 205 test and 22 coordination lines across
two paths and no production code. Receipts live under the existing evidence root's `stash/` directory;
`PREEDIT-TAKE-TERMINAL-AUDIT`, `FOCUSED-TERMINAL-CHECKPOINT` and
`POSTLOWERING-DRIFT` and `RETAINED-REVIEW` bind their exact sources, failures and dispositions. These
are bounded custody proofs, not bounded retention or full-programme acceptance.

#### Stash limits — DX06/14/17 pre-edit law

Selected policy: `actor.with_stash(8, refused, route)` or the owning
`Stash::new(actor, 8, refused, route)` starts with an empty FIFO. The limit counts
held logical inputs, not allocation bytes, Machine deferrals, runtime mailboxes
or outgoing effects. Zero is valid. Only a newly selected `Stash` input at the
limit is refused: its original `User` (sender and move-only message) moves to the
supplied typed recipient through Actions, and the actor continues. Holding owns
the original input; refusal owns it in an explicit send; release/take transfers
that same ownership. No duplication, eviction, crash or rollback is involved.
`Deliver`, `Release`, non-user events, initialization and stop keep their existing
laws, including successful effect prefixes and untouched replay suffixes.

This is Bombay resource policy, not an actor-model guarantee. Cross-check the
[actor computation reductions](https://www.cambridge.org/core/services/aop-cambridge-core/content/view/E9A5266BA5D37A1856D50C939679F31C/S095679689700261Xa.pdf/a-foundation-for-actor-computation.pdf),
Behavior layer laws L4–6/8/9 and all four normalized atomic documents.
`VecDeque::with_capacity` reserves storage; it does not enforce this limit.
Expected refusal is a normal typed effect, not transition failure. Use existing
DeliveryRoute logical/exact/mixed capabilities and SendProduct interpretation,
settlement and source custody. A same-event `StashSends { actor, refusals }`
preserves the inner effects first; add no synthetic ingress/event wrapper.
The repeatable route alone needs Clone because DeliveryRoute consumes it;
neither input nor inner actor acquires that bound.

Reuse the one existing queue/replay authority as explicit `UnlimitedStash` for
advanced unbounded use, including real Never/identity callers that need no
refusal capability. Finite Stash adds only an empty queue owner, limit and real
recipient. Do not convert populated queues without returning every original
input on refusal. `take()` leaves the limit, recipient and pure selector intact.
The finite constructor/generic-arity break is deliberate and requires migration
guidance; the ordinary facade infers the complete type. There is no optional
dummy port, policy trait, default generic, second queue or second replay engine.

Isolated owner/native `codex/devx-stash-limits` start at5068f6a/c8dfe3b.
Before production, execute focused owner and real Application REDs at the desired
syntax, with frozen original dependencies. Prove zero/full/refill/release, two
original senders and move-only allocations, both shutdown orders, MessageAdapter
and Machine, inner initialization/sends/creates/stop, accepted refusal and closed
exact-recipient custody. Review every test twice. Static denials must preserve
protocol/phase/infallibility and exact-only routing without a logical host.
Migration may begin only after the smallest complete downstream proof.

Expected owner scope: stash/{mod,queue,limit}.rs replacing stash.rs, crate exports,
one focused test, existing guide/changelog; about150 net production lines,
two additional public types (UnlimitedStash and StashSends), no trait/event/error.
Bombay scope: ActorExt, one native test and coordination; about20 production lines.
Baseline:223 stash lines, one module, two types/two traits,11 public spellings,
zero control-state sums, three transient route choices, three match/two if/one
while. Surviving values are inner behavior, pure selector, FIFO originals, finite
limit and actual refusal capability. Record measured drift and loss/order/sender/
limit inversions before retention. Two independent pre-edit design passes clear
only this Stash hypothesis. Machine bounded deferral remains open: Clone can
share state, so cloning then discarding a mutated candidate does not prove rollback.
Do not lower that reopened Machine hypothesis or weaken its existing bounds.

Pre-production result: owner debug/release REDs fail at missing StashSends and
the four-argument constructor (with the related untyped Machine selector); the
real native consumer fails only the missing product and three-argument facade.
All835 owner and1,504 native/archive inputs stay frozen. Five pure and two native
tests now specify the intended limits, original custody and lifecycle journeys;
this is compile-failure evidence, not runtime acceptance. Two source reviews
caught and corrected a zero-limit coverage gap, a waiting endpoint left open,
and an invalid assumption about retirement-settlement order. Nonempty inner
initialization/creation/stop-prefix and static-denial cuts still precede lowering.
Production remains byte-identical to the baseline. The initial missing private
native driver and pre-format check are setup failures, with no proof credit.

Complete the effect witnesses in the existing algebra test using its existing
ExplicitInitialization and StashRecording actors, including ReceiveTimeout's
real initialization/replay schedules. No new actor/protocol fixture family is
needed. This adds algebra.rs and the composition export file to the owner scope.
The same-event send law requires the chosen route's concrete SendsFor<B::Event>
proof at construction/Behavior and the facade; keep that bound at those ports,
not on stored queue values or borrowed observations. The queue exposes only
pure selection and receipt of an already selected original User to finite
admission; replay remains wholly at the old queue owner. Native static fixtures
must distinguish wrong refusal payload/address, fallible/open-phase replay,
and a real exact-only positive without a logical host. These are the remaining
focused test edits, not permission for catalogue migration before the native
accepted/closed-recipient journeys pass.

Before retention, also prove the existing DX08/L4 insertion law: adding a
same-event named effect product must preserve `Actions::cont().with_send(request)`
for a unique typed inner lane. A ReceiveTimeout outside the finite stash must
emit a real reminder to the MessageAdapter destination without naming the stash
product or a structural path. Initialize, hold one original input, refuse a second,
fire the actual latest schedule generation, and prove exactly one reminder plus
unchanged original custody; replaying elapsed must emit nothing. This is a derived
composition law, not new timer semantics. Generic SendProduct currently derives
interpretation/custody but no insertion delegation; inspect that common owner
and existing SendLayer/SendInput proofs before choosing a fix. A failing witness
reopens this unretained lowering; do not compensate in the callback or migrate
the catalogue. Any common fix needs a second real product and wrapper order,
missing/duplicate-lane denials, and unchanged interpreter ordering/custody.

The first finite lowering is reopened and removed: the real outer-timeout
callback fails only E0277 in both profiles at its unique adapter delivery.
The saved candidate's five pure and two native journeys remain bounded positive
evidence, not retention. Common-owner work starts separately at main e42088c6
in `codex/devx-send-products`. Generic SendProduct must delegate exactly one input
unchanged to its statically selected field and preserve every other lane. Each
field forwards the existing arbitrary inner SendInput path, using disjoint
existing Own/Inside markers solely for static selection; unique inference remains
canonical and duplicate/aliased capabilities stay ambiguous. Nominal actor sends
keep their existing semantic lane implementations without extra alternatives.
No new public type, trait, state, branch, storage or interpreter is proposed.
Scope: macros/src/lib.rs and its rustdoc/guide/changelog plus one focused actors
test and compile-denial docs; about35 production lines. Reuse all existing
SendEffects, SendInput, Actions, named interpretation and settlement machinery.
Before macro edits, a second real DeliveryOutcomes product with both nonempty
lanes and nested nominal actor sends must fail, while reverse wrapper placement
and missing/duplicate input cases specify the limits. Aggregate baseline: no
control states, no subordinate alternatives, one derive owner; the fix must
leave all interpretation/custody/initialization ordering code byte-identical.
This removes per-product caller-side insertion machinery rather than adding a
parallel constructor or callback adapter. Preserve the original stash regression
and prove it after integrating the shared fix before further stash lowering.

#### Policy construction and startup — DX10/11/19/24/27

Owning modules are supervisor/roster, supervisor/directory, pool/queue,
pool/router, proxy and worker. Parents curate contracts; configuration and
settlement details remain private. Config holds simultaneous explicit policies
and is consumed into existing flat fields. Roster recovery owns strategy, limit
and delay; pool recovery owns limit, delay and failure. Temporary needs no
replacement source; immediate workers need no unused startup plan. No universal
builder, parallel constructor, hidden default or semantic boolean is retained.

Construction preserves factory, prepared prefix, refused role, reason, untouched
suffix, selector and diagnostic route. Dynamic construction is infallible.
Exact reply capabilities already carry their destination and require no logical
host; logical/mixed replies retain real hosting. Do not infer a type proof from
a runtime reply variant. Directory/router still have meaningful mixed hosts.

Published270/150 supplies these policy and startup contracts. Constructor-input
repair50348d7 additionally makes borrowed-role factory inference local to the
constructor. Its extra public generic and mutable refused-factory reoffer are
real source breaks. Source-selected native proofs do not make this repair part
of registry270. Keep the proven unannotated examples; publish the reviewed repair
through a breaking0.x release before registry-only catalogue/nested-lock gates.

Caller migrationce12436 preserves all747 original assertions in order, adding
only the independent Released observation. The original Stash oracle passed a
bypass; the strengthened assertion rejects it in both profiles. Proxy-owned
outer shutdown means these layouts prove Stop/Release forwarding, not independent
inner Shutdown. Pure OneShot/Stash orders prove complete per-turn effects/FIFO
originals, not native clock, joint startup or nonempty inner initialization.
Removing a raw worker's shutdown wrapper still needs its normal-retirement
differential; omit no policy merely because a happy path passes.

Startup callerf244566 replaces Startup and ManagedWorker with the published
owning closure, preserving one cloned sender/role and the same deferred
send/expect. The exact Ready=()/Rejection=Never and Rust2024
[precise capture](https://doc.rust-lang.org/edition-guide/rust-2024/rpit-lifetime-capture.html)
contract retains no borrowed sender. Worker61→39lines (+9/-31/net-22), core/public
crate types+0/-0; Worker/Stop body, enums2, match arms1 and module1 stay unchanged.
No new state, authority, cause copy or structural path; cross-check StartupPlan,
StartWorker permit/grant, initialization and original retirement laws. Two
independent source/evidence reviews pass. The source-bound driver caption says
“two native tests”; actual coverage is one native and one pure test per profile.
No new readiness, cancellation, exhaustive custody or memory gain is claimed.

#### Creation correlation and registry binding — DX12/16/21/25/27

Fresh allocation is the actor-model law (Agha et al.1997 pp19–20). Creator-local
correlation and rejection before allocation are derived/Bombay policies.
Duplicate correlation returns the original RoutedCreation and typed reason;
accepted prefix, untouched suffix, later siblings and joined descendants survive.
It must not overwrite or report a successful birth/restart. CreationBinding's
Starting/RetiringStartup/Established/Rejected/StartupRejected retain the original
task/control, raw/projected retirement, endpoint/join, or explicit original
kind/address/route and failure lanes. Sequence arithmetic never supplies provenance.

Published270/150 matches verified checksums, tags and220 shipped Rust files.
The macro namespace change is breaking: candidate143 failed a valid published
Actors261 pool worker, while150 correctly leaves compatible registry142 selected.
The durable archive fixture/preflight must reject an unused *compatible* patch;
version labels, a locally compiled owner or a planned release do not prove
publication or downstream compatibility.

The fresh registry-only e629464 Nix run reproduces the protected consumer's
stale nested lock: its vendor contains026 while the path dependency requires027.
The pre-edit build-coherence law atd419486 precedes fixdbefc2b in a separate
`codex/devx-protected-lock` worktree. Only the nested lock's Behavior/Actors026→027
and macros142→150 records change, exactly matching the already published root
versions/checksums/dependencies. All322 nonowner records, root Cargo files, Rust
source and certificates remain unchanged; no patch or manifest edit is selected.
Scope: lock plus coordination, production/tests/public types+0; no state,
transition, module, capability or custody representation changes.

The private exact-source consumer matrix passes53 default and68 protected cases
on Rust1.99/1.96 debug/release:484 executions. Four reports of the existing query
ignore are retained. That exact ignored case then executes explicitly through
20 existing campaign modes:128 topology/layout/provider combinations and320
successful role-process runs. Router roles exercise their existing connection
fixture, not an additional business query. Four selected binaries are reused
from the completed consumer builds; process runs are new. All1253 bound sources,
seven certificates, selected binaries and child logs preserve their hashes.
Strict current Clippy passes both feature configurations. Both independent
reviews verify exact case sets, process provenance and bounded custody scope;
these unchanged tests do not prove universal custody or explain the old hang.
Receipts: `protected-registry/{RESULTS,TERMINAL-CHECKPOINT,NATIVE-AUDIT}.json`.
The protected Nix gate passes on cleandbefc2b in526.250s with624 source files and
HEAD conserved. Its132 outer executions exclude the two child-stop cases and
controlled TLS query case, covered separately above. Both strict Clippy phases
and formatting pass. Registered output, exact derivation and raw command results
pass both independent audits, with no reported signing/disk/classifier failure.
The original e629464 full run finishes with unchanged source and its dependency
failure intact. This one repaired gate does not certify the full programme.

Bombay mainf4aa3b3 lands our already-integrated timer-custody commits; merge6c0361
preserves the exact tree. The combined branch applies the focused mailbox and
lock repairs atd272d637 without reverting candidate timer/adapter/inventory or
root Cargo bytes. Only five paths differ; mailbox source, tests and nested lock
match the reviewed repairs. Both coordination records and current usage remain.
Actual packaged7644735 owners then pass108 checks on1.99/1.96 debug/release:
52 native journeys,12 pure capacity cases and44 inventory checks. All1498 bound
inputs remain exact through the matrix, formatting and strict current Clippy
for all five targets; both independent transfer/terminal reviews pass. Receipts:
`adapters/combined/{RESULTS,TERMINAL-CHECKPOINT,POSTLINT-CHECKPOINT}.json`.
This archive-selected integration is not publication or complete feature gates.

#### Verification, costs and delivery — DX19/25–28

| Exact source | Verified result | Limit |
| --- | --- | --- |
| Bombaye629464, actual registry270/150 | Fresh full `nix flake check --keep-going --max-jobs 1 --cores 2 --no-update-lock-file -L` exits1 in2598.452s:16 gates pass, nine fail; all624 source files and HEAD stay exact. Explicit25 Nix verdicts agree with nonnull registered outputs/exact derivations at the completion audit. | Eight fail on supervisor/pool factory inference awaiting PR107 publication/adoption: build, Clippy, supervisor/pool examples, owner coverage, panic-unwind, remote-debug and test. Protected-counter fails its stale nested lock; the separate dbef repair passes above. No reported signing/disk failure; no full pass, other-platform or permanent repair claim. Axum compiles zero tests and earns no runtime credit. `mailboxes/NATIVE-{VERDICTS,TERMINAL-AUDIT}.json` and nine raw failed-build logs retain exact failures; later store availability is separate. |
| Owner50348d7 | Eleven native Nix checks,987 nextest/118 doctests/50 audit controls, four-workspace archive preflight and all14 exact-head GitHub checks pass. Actual extracted owner consumers and external contracts retain their supported-compiler/profile evidence. | PR107 plus audit PR108 remain open; normal reviewed publication is pending. Sandbox yank lookups were unavailable; separate online audit succeeds. |
| Bombayb8c2e6f, registry026/142 | All25 fresh native aarch64-Darwin checks pass in4750.519s with623 conserved hashes:776 outer tests/132 UI fixtures, selected remote debug/release422 outer+20 UI, owner coverage and50 audit controls. Two independent audits pass. | PR383; no reported classifier crash. Not the270 catalogue graph, all-systems proof or automatic-timer implementation. Axum example gate compiles but executes zero tests. |
| Cataloguece12436 with source-selected503 |31 identical caller tests in seven targets pass1.99/1.96 debug/release; separate protected held-startup/sibling-progress retirement passes all four configurations;397 root and322 protected nonowner records remain exact. | Same747 original assertions plus Released; no final registry delivery, exhaustive custody or new readiness proof. Earlier inaccurate inverse matcher remains a failed matcher, separately attributed. |
| Startupf244566 with source-selected503 | Existing native+pure tests pass both compilers/profiles (eight test executions), four actual example runs pass;2080 inputs conserved. Strict whole-workspace/all-target Clippy passes14.476s. | Same published closure law; no new panic/cancellation policy. Ordinary initial-factory inference still needs PR107. |
| Bombay1cc0953 with mainf25e4a0, registry270/150 | Four supported-compiler/profile remote client runs each pass21 exact outer tests and nine UI denials; original declaration formatter fails the exact privacy assertion in current debug/release. Strict owner/client Clippy and workspace format pass;1876 inputs conserved before documentation updates,400 lock records exact; two independent reviews pass. | Normal Display/Debug privacy preserves original source/target. No full SDK hiding or execution proof for every Debug variant. Two stale-binary attempts were rejected and excluded; profile-specific rebuilds establish the six accepted comparisons. |
| Frozen V3 local corpus, registry270/150 | Strict replay passes1.99/1.96 in167.118/174.517s:124 unique fixtures (19 positive/105 negative),29 suites,25 targets,107 outer passes and one named parent-invoked child ignore;1248 inputs conserved. | Local axum-enabled coverage, not remote/minimal-feature/archive/full-programme acceptance. Post-run patch serialization has a separate equivalence proof. |
| Published270/150 archive/registry consumers | All220 shipped Rust files match reviewed release source. Five unchanged pool journeys execute against actual registry owners in debug/release without patches. | The runtime is adapted214d with five recorded naming/startup adapters; not current unadapted Bombay, minimum compiler or full catalogue gates. |

The retained verifier checks exact compiler identities, fixture names/modes,
per-target counts, one child/parent relationship, every summary/fixture outcome,
caller and scratch inventories, and immutable expectation/source/patch hashes.
Five unit methods and eight entry controls pass two independent reviews. Empty
Cargo0, failed-row Cargo0, added source, altered canonical/fixture/patch, unknown
compiler and overwrite mode reject for their own reasons. Raw exits stay
separate. Normalized minimum.patch preserves its original13-snapshot byte prefix;
applying eight added hunks reproduces all21 hashes from strict V3. It does not
retroactively change that run's frozen source. An outer Nix cache-busy message
is disclosed separately from completed compiler verification.

One unresolved optimized retry hang remains observed:
`public_node_missing_host_restores_the_declaration_for_original_work_retry`,
`NODE-PROTECTED-RETRY-HANG`. Later passing suites/nonreproduction do not explain
it; no wakeup, locale or SDK cause is established. Intentional mutation failures,
parent-invoked ignores and genuinely failed gates retain different dispositions.
Joined output recovery proves exact headers/reference properties, not compressed
contents or a permanent Darwin fix. Earlier signing, ENOSPC and masked classifier
runs remain failures; current passing runs do not erase them. Store availability
is not run acceptance. All Rust runs in pinned Nix; Linux/x86_64-Darwin remain
unexecuted locally unless the exact CI/source record says otherwise.

Memory outranks binary size. Recorded sender storage176→48bytes and three
constructors8→0 allocations are bounded results. Earlier equal77-dependency
counter work uses74 blocks, peak122916→123644bytes (+728), equal end heap; it
proves no memory reduction. Other layout/closure comparisons retain their
original source scope. Current comparable compiler, service, remote, latency,
heap and retention measurements remain with DX06/26; lexical/source counts are
review diagnostics, not semantic or performance proof.

Expanded programme budget (2026-10-09) and one topology declaration (2026-10-11)
are authorized. Release permission remains PR-specific; completed earlier
publications grant none for a later candidate. Networking/durability/identity/
deployment products remain explicit DX23 deferrals, while hiding SDK interfaces
is current work. Node business callbacks receive actor capabilities, preserving
borrowed/non-Send work, split receiving, cancellation and complete originalWork
refusal. Ordinary callers must not be required to name Session, Query, SDK configuration,
raw connections or an alternate transport; those remaining public surfaces
still need to be hidden. Query-free export association does not prove provider or
request-key semantics; native transport tests stay in private Node/SDK owners.

Entity's retire_entity proves joined slot release/mode, not an error-free native
or callback result. Cancelling its observer does not cancel the directory task;
missing publication cannot forge completion. Moving its report module did not
remove callback/associated-type ceremony. Machine Clone and bounded Stash overflow
remain the separate open changes already specified above.

#### Timer ownership — DX13/16/25–27

PR109 refresh c294687 merges main edf103e after PR107/108 landed. The two
conflicts affect the changelog and this record only; all327 Rust files and six
locks stay exact. All11 final native checks are realized and retained after a
132.599s rebuild; the authoritative rerun passes in2.196s with833 unchanged
inputs/HEAD,996 nextest,120 doctests and50 reference controls. The earlier
final-head wrapper correctly failed despite Nix exit0 when two cached outputs
were unavailable; its raw failure remains separate. Package preflight passes
9.409s across two workspaces, with existing fixture/patch warnings;248 archive
members/224 Rust files match clean c294 source. Separate online audit passes
1.537s with65 dependencies/no findings or warnings (database479c888); sandbox
yank lookup errors remain separate. Exact-head CI38950728786 and all14 GitHub checks pass at c294687.
PR #109 merged externally as e42088c6 on October16; this establishes main
delivery, not a new registry publication.
Two conflict reviews preserve the single28-row checklist and all change notices;
final gate reviews remain separate from earlier648 verification. No publication
or complete-programme acceptance follows from resolving this conflict.

Ordinary OneShot, Periodic, Deadline and ReceiveTimeout constructors now take
only the inner actor, delay/deadline and reaction. Bombay ActorExt exposes the
same no-ID operations. Each wrapper owns its exact typed event ingress and
current generation; reordering does not require caller names or paths. This is
Bombay composition policy, separate from actor freshness and lifecycle identity.

The ownership equation remains `(target,generation,time)` outside conversion
until exact receipt `(target,generation)` or rejection `(original request,reason)`.
Four shared timer products have closed TimerId/Here targets without generic
defaults. Existing root names and TimedEvent remain the named protocols; four
owned aliases select Here. Reuse the existing OwnedItem custody blanket and
SourceAction identity. Named absolute/relative methods retain their original
custom InjectEvent ports and actor-wide replacement law. A private destination
port shares the settlement algorithm; the queue uses Named(id)/Owned(occurrence),
with compile-time checked occurrence arithmetic, no registry or counter.

Inner-first initialization, user-only inactivity renewal, optional deadline,
current-generation acceptance, replay discharge, stopping/error behavior,
original rejection, accepted prefixes and independent later effects remain.
The two-state absolute schedule and four-state relative lease are unchanged
apart from deleting obsolete stored IDs. Their surviving values own only the
current deadline/generation, checked renewal history and inner behavior.

Pre-edit no-ID owner and actual registry-consumer witnesses failed at the
missing arguments and exact owned protocol contracts before lowering. Earlier
blanket path-scoping broke named replacement; a lowering that silently narrowed
existing named methods was removed. Initial stale-cache/zero-selector attempts,
an invalid test fixture, broken rendered links and an E0369 inversion earn no
runtime or gate credit. Complete prior laws, falsifiers, measurements and
receipts are preserved in [fe28b7a](https://github.com/devrandom-labs/bombay-behavior/blob/fe28b7a/docs/engineering/developer-experience-prd.md).
Retained source commits:87aedf1 tests,2e5e863 implementation,5e23384/78ca80a caller
adoption,7f4f0b3 independent oracles and7b0d013/c8c2104 migration guidance.

| Exact scope | Executed evidence | Limit |
| --- | --- | --- |
| Timer-only c8c2104 | All11 aarch64-Darwin Nix gates pass219.061s;832 hashes conserved,992 nextest/0skip,120 doctests/0ignore,50 reference controls; two independent audits. | Prior guide-link failure remains failed; no other platform certification. |
| Integrated e628018 (includes reviewed PR107/108) | Full Nix0/190.663s: eleven selected checks, nine new builds plus cached audit/TOML; same992/120/50, clean source conserved. Separate online audit0/2.440s,65dependencies/no findings or warnings, database479c888 from10-16. | Offline sandbox yank lookup failures are not online audit successes. Final20af935 adds only eight breaking-changelog lines: all eleven gates revalidate from cache in1.901s with clean conserved source. Publication remains PR-specific and pending. |
| Timer-only extracted owners | All223 shipped Rust files match archive-time feature source (145 under src). Four actual Bombay journeys pass1.99/1.96 debug/release:16 executions,878 frozen inputs, fresh three-owner compilation and397 unchanged nonowner records. | Unpublished archives and copied runtime; not the later factory+timer integration or registry delivery. Preflight passes with four existing fixture warnings and the expected unused incompatible macro patch. |
| Migrated Bombay source + e628 | Six targets pass17 cases each on both supported compilers/profiles:13 native/application plus four pure/type cases;2083 inputs conserved during runs.53 wrapper ID arguments removed; assertions and genuine named controls independently reviewed twice. | Source-selected scratch graph only; actual feature manifest/lock unchanged. Later metadata edits bind three reviewed UI sources without changing expected stderr. |
| Migrated Bombay strict corpus +20af935 |1.99/1.96 replay passes183.650/185.273s:124 fixtures (19 positive/105 negative),29 suites,25 targets,107 outer passes and the declared child ignore;1459 inputs conserved through both runs, two independent reviews. | Three exact owner-root path remaps preserve original trybuild cfg/verbose/width settings and unchanged snapshots; not registry execution. Unmapped and encoded-flag attempts remain failed. Later test-only lint amendments are outside that frozen source map. |
| Final owner PR109/6488965 | All11 native checks freshly build and pass229.044s,992 nextest/0skip,120 doctests/0ignore,50 reference controls;832 source hashes conserved. All14 exact-head current CI checks pass; primary workflow38910368134. External contracts pass29 outer tests+11 compile denials per debug/release profile. | Superseded CI was cancelled, not a passing historical run. Sandbox yank lookups remain unavailable; separate online audit above applies to identical dependency/Rust sources. Normal release remains pending. |
| Combined6488965 extracted packages | All223 shipped Rust files match clean648 source;247 regular tar members match the extracted package inputs. Eight complete targets run27 cases plus one selected keyed-pool case on1.99/1.96 debug/release:112 executions (96 native/application,16 pure), plus16 actual example runs.1501 inputs conserved;397 nonowner records unchanged. | Unpublished source-selected packages, not registry or remote/full-feature proof. Preflight checks two consumer workspaces, with four existing fixture warnings and an incompatible-macro patch warning; its unlocked README graph is separate. |
| Migrated Bombay default workspace +6488965 | Pinned1.99 formatting/build/strict Clippy/tests/doctests pass289.773s;2084 inputs conserved.782 outer passes,132 UI fixtures separately counted, two declared parent-invoked child ignores; seven doctests/three existing ignores. | Remote disabled;18 zero-case binaries earn no runtime credit. Four expected unused-patch warnings come from isolated macro probes. Prior lint/inventory failures remain failed. Not authoritative flake or minimum/remote coverage. |
| Reconciled Engine inventory | Registry1a8f20b passes all11 inventory checks on both compilers/profiles; four intended metadata inversions fail. All45 templates/19 capability families and24 historical98983 evidence records remain. Candidate241088d additionally binds Actors to6488965 and its shared timer family. | Presence scans are inventory, not complete type/interpreter proof or refreshed historical execution. Registry source still uses named wrappers; candidate remains unpublished. |
| Bombay4a8b531 protected downstream +6488965 | Pinned1.99 separate-workspace debug/release each pass53 plain tests and68 protected tests with one controlled-fixture ignore. The five TLS campaign modes per profile then execute64 complete combinations/160 exact role processes; all child logs, binaries, source and certificate hashes match. Nested322 nonowner records stay exact. Format and both strict Clippy modes pass; independent reviews distinguish all counts and custody limits. | Source-selected graph only; minimum compiler, registry and authoritative Nix remain open. The exact optimized retry also passes separately, without explaining its historical hang. Initial Clippy argument placement failed before Rust; corrected placement preserves locked/offline and deny-warnings checks. |
| Independent inactivity fuzz |20000 runs per nightly1.101 debug/release with sanitizer disabled; complete actual notices/targets, schedules, actor state, creations and verdict checked against unchanged InactivityModel. Eight intended assertion failures reject missing/duplicate/future notification and service rearm. | Evolving corpus, not identical comparative trials or sanitizer evidence; errors, terminal turns and exhaustion have separate tests. |

The final formatted Bombayd743f1f test source passes the pinned1.99 remote
sequence in101.644s:426 exact outer cases and20 separate UI fixtures per
debug/release profile, both child-feature harnesses with six fixtures each,
formatting, Bombay all-feature/all-target strict Clippy and all-feature docs.
All2084 inputs remain exact through nine commands; two independent reviews
confirm case/target sets, fixture modes, six required request regressions and
absence of warnings. Docs did not deny warnings; none were emitted. This does
not certify actual registry, protected nested consumers,
whole-workspace all-feature runtime or authoritative Bombay Nix gates.

The original optimized matrix still fails one startup before pressure work;
its panic hid the cause, and a same-source isolated pass does not explain it.
A controlled copy retains one parent IPv4/IPv6 listening pair across all four
commands: fixed-port startup fails native AddressInUse in both profiles, while
SDK-owned `localhost:0` passes the complete original Counter journey. Both
local-only pressure fixtures now select SDK-owned ports and reveal existing
startup sources on failure. Their Work bodies/all61 assertions and shared
two-node fixtures stay exact. The pre-edit law is preserved at
[566be5c](https://github.com/devrandom-labs/bombay-behavior/blob/566be5c/docs/engineering/developer-experience-prd.md).
First-runner, Darwin socket-option guard, initial formatting and draft audit
parser failures receive no gate credit. These controls establish a fixture
vulnerability and repair, not the hidden historical failure cause.

The first minimum feature run failed at existing child-denial snapshot drift
before later stages:1.96 emits oneE0277 where1.99 expects two. The retained
test-only consolidation4a8b531 keeps the original bare program/canonical stderr exact,
deletes its byte-identical feature twin, and renames four filename references in
the already reviewed minimum patch. The unconditional denial runs under both
feature configurations; production APIs, bounds and diagnostic parsers stay
unchanged. Its pre-edit law is preserved at
[cb9ad32](https://github.com/devrandom-labs/bombay-behavior/blob/cb9ad32/docs/engineering/developer-experience-prd.md).

Fresh1.96 feature verification passes seven commands in823.460s:426 exact outer
tests and20 separate UI fixtures per debug/release profile, both child harnesses
with five positive/one negative fixture, and all-feature docs. All2080 inputs
stay exact through execution;397 nonowner records and21 reviewed diagnostic
transformations remain unchanged. No minimum Clippy was installed or executed;
docs did not deny warnings and emitted none. Both independent reviews pass.

Final strict replay passes1.99/1.96 in170.998/176.449s:124 unique fixtures
(19 positive/105 negative),29 suites,25 targets,107 outer passes and the exact
parent-invoked child ignore. All2080 inputs are conserved through both runs;
the unchanged verifier checks all source/expectation/corpus controls. Its five
unit methods pass. Three exact owner-root remaps preserve original trybuild
arguments;143 fixture compiler invocations per compiler include19 additional
positive execution builds, not additional fixtures. Both independent audits
pass. The initial stale-index copy failure occurred before Rust; the failed
minimum run and a draft audit's omitted mode literal earn no acceptance credit.
The canonical batch is production/public-types0 and Rust tests0/-16; the
complete five-path change is+11/-80 including metadata and deleted stderr.
Source-selected proofs do not establish registry, protected nested or
authoritative flake acceptance, and do not explain the old optimized retry hang.

Two old oracles were repaired separately from mechanical migration. The service
test previously sent its alleged inner event to the outer ingress; it now
observes the inner state change, original outer timeout, exact98-to91 notice,
every action lane and replay. Fuzzing previously changed only local model state
with a no-op reaction; it now compares actual effects and state. Other retained
no-op timer callbacks prove construction/noninterference only, not admission.

The architecture checkpoint passes within the authorized experiment: baseline
11 selected production modules/2126 lines/56 arrows/8 enums/56 direct pub
spellings; retained12/2252/51/9/70. Production+491/-365/net+126, tests+1388/-506/net+882
across31 changed Rust files in owner plus minimal copied runtime, before broader
Bombay migration. Counts exclude true test tails and two test-only methods;
direct pub counts exclude reexports/restricted visibility and are not total API
counts. The extra enum owns a disjoint replacement namespace, not a lifecycle
phase. No arrival-history state, duplicated cause, semantic flag, second
transition authority, caller marker or structural path was added. Existing layer,
normalized aggregate and runtime settlement laws remain the cross-check.

Memory measurements remain bounded: actual three-ingress queues retain368bytes
and four allocations before/after on both supported optimized compilers.
Owned events16 versus24bytes, requests24 versus32, receipts8 versus16; queue120
unchanged. The disjoint key grows8→16:64 named timers retain7184 versus6672bytes
(+512),18 allocations each, total allocation14000 versus13008. Drain retains16,
full release zero. No full actor/application peak, throughput or latency gain
is established. Complete Bombay feature Rust delta againstbf1921b is
production+143/-39/net104, tests+623/-191/net432 across31 Rust files; public
types+0/-0. Four existing ActorExt signatures lose IDs; three private contracts
own timer key, occurrence proof and destination. There is no new production
module, lifecycle phase, nested transition authority, duplicated cause, semantic
flag or structural caller syntax. Disposition:pass against the same composition
and settlement laws; metadata/docs are separate from Rust counts. Protected nested/
authoritative runtime gates, remaining acceptance cuts and normal
breaking release remain open.
