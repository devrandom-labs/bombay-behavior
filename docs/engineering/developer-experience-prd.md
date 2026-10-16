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
rejections through that published contract. Pre-merge current/MSRV proofs and all21 native gates pass at their recorded
revisions. The merged baseline passes all24 native gates ataf87ef4. Joined-result/HTTP commitsf0020c2/0011620 pass720 optimized all-feature tests, one existing skip. The later Entity retirement seam36af335 passes its focused current/MSRV witnesses and affected owner/consumer corpora; its final full gates remain open. Remaining programme work stays open. Earlier
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
| DX01 | One Application owns roots, members, scope, HTTP and Entity. Joined/split Node callbacks are actor-only; protected consumers adopt that contract at6c5c18a. Recursive, supervisor, pool and cold journeys execute without composed-type annotations. | Open: complete selected semantics and final gates; hide remaining SDK configuration/query surfaces. Delivery candidate: [Bombay PR375](https://github.com/devrandom-labs/bombay/pull/375). |
| DX02 | One topology declaration derives spaces, occurrences and custody for born/cold/recursive actors, including equal protocols in distinct roles. Missing/wrong hosts, roles and births remain denied. | Open: complete host/caller corpus, canonical catalogue reconciliation and final gates. |
| DX03 | Nominal role owners close the tested recursive retirement proofs. Native owner/topology tests, cancellation custody and static authority/occurrence denials run on supported compilers. Strict compiler-specific diagnostic replay retains source/coverage controls. | Open: latest-main reconciliation, all owner consumers, remaining cancellation/freshness acceptance and final gates. |
| DX04 | ApplicationOutcome joins work, native retirement, notifications and families; consuming completion returns value plus ApplicationRetirement or the original whole report. Native receiving migrations preserve original assertions; custody-loss inversions fail and restoration passes. | Open: proportional failure recovery, complete family-disposal/operational inspection, final-source minimum-compiler/archive/full gates. |
| DX05 | Runner types derive from root/members; current-thread/multithread, original-input recovery and independent receiving witnesses retain caller-local non-Send work. All38 original cancellation/host journeys pass their recorded matrices. | Open: final complete consumer and repository gates. |
| DX06 | Validated actor/client MailboxCapacity propagates through root, births, cold members and Entity. Saturation returns originals. Senders drop unused reply storage (176→48 bytes; three constructors each8→0 allocations). Capacity2 reduces measured client peak heap at equal work. | Open: complete resource accounting, final-source/archive gates and comparable package/compile/runtime measurements. Existing measurements cover their recorded consumer and host only. |
| DX07 | The existing actor/worker expansion remains authoritative. Pure births, aliases, renamed paths and a self-creating Entity actor execute against published macros0.14.2; empty-alias fix91/97 is delivered. | Open: complete generic/private/public, sibling-target and duplicate-role corpus. |
| DX08 | Published owners preserve selected send lanes through wrappers and static denials. PR104/a25afb7 extends FIFO/keyed construction to ordinary named completion/business products, with four retained denials, five native cases across supported compilers/profiles and actual packaged consumers. | Open: complete named-lane catalogue and unrelated-template/order evidence. |
| DX09 | ActorExt adds the exact owning FinalizeOnShutdown and EstablishedTerminationMonitor. Missing-method regressions, order/effect/refusal witnesses, archive consumers and recorded native gates pass. | Open: remaining decoration/catalogue audit and final combined-source reconciliation. |
| DX10 | Feature owners roster/directory/queue/router use Config and distinct recovery policies; direct/fallible factories infer types. Policy head186266e passes owner/package/archive proofs with unchanged actor layouts. Isolated reply refinement removes ordinary reply aliases and unused hosts; a deferred startup closure removes the worker alias, startup struct and associated declarations in copied real callers. Sixteen focused owner tests and979 whole-workspace tests pass; the latest ten native Nix checks return0 without a reported classifier crash, with earlier audit limitations retained below. Matched named/closure plans each occupy16bytes on the measured host. | Open: policy registry delivery, compact delivery docs, final caller/cost evidence and policy-name/necessity audit. Retry, recovery and drain choices stay explicit; no parallel builder or hidden defaults. |
| DX11 | Inferred fixed/FIFO journeys preserve immediate/async startup, source refusal/panic, prepared prefix, assigned payload and shutdown. Keyed tests preserve affinity, exact rejection, unbind and joined retirement. | Open: dynamic supervision, independent late completion, remaining preparation/policy/failure cases and final gates. |
| DX12 | Published0.26.0 returns complete duplicate-correlation rejection. Actual birth/replacement/replay witnesses retain accepted prefix/suffix and joined children; exact recipients preserve real application/Entity identity after closure. | Open: remaining creation/occurrence acceptance, including transitive/wrapped/rejected births and relational identity evidence. |
| DX13 | Distinct named timer wrappers run natively. No-ID syntax is RED; blanket occurrence scoping breaks named replacement and was removed. A disjoint-key probe passes both compilers/profiles and its collision inverse. | Open: prove automatic identity through actual typed requests/wrappers/interpreter without narrowing named ports; complete reorder/cancel/zero/stale/rejection/exhaustion corpus. Probe success is not an implemented API. |
| DX14 | Stash/Machine restrictions remain. PR99 head403a4f1 moves the sealed infallibility denial to construction; both valid error forms, owner runtime/model tests, supported compilers, archives and ten Nix gates pass. | Open: merge/publication authorization, registry adoption, bounds audit and bounded-retention/overflow witnesses. |
| DX15 | Native b1c6781 adds actor-owned interpreters through Application/topology using existing guarded startup. Real file/environment requests, wrapper orders, root/descendant/cold/Entity configuration, rejection/panic/cancellation and disposal witnesses pass recorded matrices and inversions. Independent owner diagnostics retain source/coverage controls. | Open: final integrated runtime/repository/archive gates, configured-service costs and delivery. Earlier child-oracle/work-entry falsifiers and their corrected evidence remain bounded; focused passes do not close the programme. |
| DX16 | Startup/provenance and complete source/interpretation custody remain. Draft PR100 head9948bdd supplies OwnedItem from existing settlement metadata; real IO/panic custody, mutations, privacy denials, owner gates and extracted consumers pass. PR101 head184354b fixes rejected reply custody:970 owner tests, ten native Nix gates, four-workspace preflight and locked real-runtime failure/retirement proofs pass. It merged as dda4a42 under explicit authorization; reviewed release PR103 merged as13951b8 and normal publication of Behavior/Actors0.26.1 is verified in both registries and release tags. | Open: owner delivery, two-template/wrapper composition, catalogue migration, native integration and final downstream/cost evidence. Different custody equations remain distinct. |
| DX17 | Shared retirement custody and semantic send access have bounded interface provenance. DirectoryConfig validates immutable limits once, deleting impossible downstream branches while retaining original limits. | Open: export/consumer/invariant and bounds audit; remove competing spellings only after comparing complete semantics. |
| DX18 | No new production dependencies selected. Existing thiserror, standard owned sums/products and validated limits remain the chosen tools; bounded registry/language comparisons are in the evidence appendix. | Open: error-embedding/source/visibility/bounds equivalence and dependency/MSRV/feature/compile-cost audit. |
| DX19 | Strict supported-compiler replay checks exact fixture names/modes, coverage and unchanged sources. Native owner denials add missing-interpreter/wrong-owner cases; damaged-expectation, empty-success and added-source controls reject. | Open: remaining diagnostic/renamed/minimal-feature corpus, owner diagnostic publication and final archive gates. Node/client denials keep their separate caller checks. |
| DX20 | Application directly establishes root/selected-recipient/Entity clients and senders. Nonwaiting exact admission preserves original Full/Closed commands; origin inversions fail. Shared external customers and pre-root scope retain target authority and original inputs on retry. | Open: complete correlated/cancelled/slow/closed-customer/late-reply corpus, capacity and final-source minimum-compiler/full gates. Admission remains distinct from business completion. |
| DX21 | Shutdown ingress is inferred at the operation; runners require only capabilities they execute. Root wrapper orders, request replay, natural completion and complete retirement pass focused tests; missing/ambiguous authority remains denied. | Open: generated canonical access, child-derived phases/drain/deadline/forced-retirement corpus and final repository gates. |
| DX22 | HTTP/Entity use the same Application and joined outcomes. Awaitable retire_entity preserves exact-slot completion, cancellation, reactivation and native/callback failures; HTTP removes unnecessary clones and receives ApplicationHandle directly. Original receiving/HTTP/family tests retain their assertions. | Open: callback/associated-type distillation, complete family/root cleanup coexistence and final complete consumer/repository gates. |
| DX23 | Zenoh is the sole internal network. Both Node Work callbacks receive only typed actor capabilities; native client/requester test drivers live under Node. Raw configuration/export/query surfaces remain unfinished. | Deferred: networking, durability, identity and deployment products stay with later PRDs. Open interface obligation: hide SDK types without a transport escape hatch; preserve typed extension and complete outcome laws. |
| DX24 | Terminal evidence stays authoritative. FIFO rejected-role projection preserves its exact failure/lifecycle product through both orders, inverse and native consumer. | Open: bounded borrowed operational inspection, telemetry-failure independence, sensitive-payload policy and bounded diagnostic dimensions/retention. |
| DX25 | Protected actor-only consumers retain complete custody. Provider repair cd77c8b conserves native actions/assertions and separates Rc cost windows into their existing isolated owner. Two reviews verify48 passing and12 intended failing executions over593 frozen/restored sources and supported compilers. | Open: latest PR375 head214d5b0 integrates main67a286c and is mergeable. All13 affected native cases pass debug/release; strict lint/format and two integration reviews pass. At06d6fd2 Nix reports24 successes/exit0 in4820.621s with620 unchanged hashes, but three unjoined classifier crashes prevent a clean-run claim. Checked/joined recovery verifies current exact-output references; no gate credit transfers to214d5b0; its exact native/GitHub checks are running. The earlier optimized retry hang remains unexplained. |
| DX26 | Recorded examples remove structural assembly; counter51→36 main lines, pool191→90, supervisor332→99 plus61 domain lines. Assertions move to owning tests. Frozen default-feature counter comparisons show equal allocation/heap products with the interpreter seam; sender/client measurements improve their bounded cases. | Open: full comparable caller ceremony, compiler/diagnostic/macro costs, layouts/binary metadata and runtime memory measurements; configured-service, remote and latency costs remain unproved. Memory outranks binary size. |
| DX27 | Published Behavior/Actors0.26.1 have verified tags and206 shipped Rust files. Six pure custody tests and an actual archived-Bombay failure/retirement journey pass both profiles against registry owners, without owner patches. Delivery still selects0.26.0/macros0.14.2; its earlier207-source proof remains scoped to that graph. | Open: policy/other owner publication, final programme consumers/features/renaming, delivery documentation and repository gates. No local patch or old-source pass certifies the final release. |
| DX28 | Historical pruning is retained. One compiled application guide runs in workspace/extracted packages across recorded configurations; local links resolve. This table is the sole DX01–DX28 disposition checklist. | Open: compact contract/evidence reconciliation, compiled sample/support tables, rendered/external links, final documentation gates and retirement of this PRD/appendix after obligations have owners. |

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

| Exact source | Executed verification | Scope limit |
| --- | --- | --- |
| Bombay3a80848, registry owners98983b4 | All21 fresh native gates in2306.566 seconds;646 optimized tests plus one existing ignored subprocess case; all459 source hashes match. Current/MSRV debug/release packaged correlation/fixed/FIFO/guide journeys;61 production and195 other packaged Rust files match. | Engine and Bombay macros are extracted unpublished packages; not registry-only Bombay publication. No signing/disk waiver. |
| Behaviorbc7718c | Separate pinned nextest969 tests in40.893 seconds; all ten native Nix gates in253.184 seconds including969 optimized tests, denials/macros, rustdoc, Clippy, formatting, dependency and package checks. Actual archive preflight11.072 seconds; four-workspace release preflight6.140 seconds.143 packaged production files match. | Current candidate is draft PR98, semver-breaking; CI/review, policy publication and actual registry migration remain open. |
| Ownerbc7718c archives + isolated actual3a80848 consumer source |24 commands execute28 complete fixed/prefix/both-wrapper/FIFO/directory/keyed journeys on1.99/1.96 debug/release in305.341 seconds, with original independent refusal and retirement oracles. | Five worker-owner/startup consumer files adapt the actual runtime; Entity stays byte-identical. Local patches do not prove registry delivery or full migrated Bombay gates. |
| Bombay2e21756 | All24 native Nix gates pass in4860.772 seconds, with every tracked source hash unchanged and clean Git. Raw log names all24 successful checks; delivery branch advances to this revision. | Later artifact inventory finds13 cached outputs and11 absent outputs; build success is established by the completed command and exact gate log, not continued cache availability. This run predates sender storage, diagnostic tooling and protected JSON integration. |
| Bombay6cb42ef | All26 joined-client/Node/JSON/sender tests pass on current/MSRV debug/release; nine client/shutdown denials and three Session fixtures pass. Final workspace strict Clippy, warning-denied rustdoc and fmt pass. | Frozen-source hashes match. Final integrated Nix/archive checks remain required; the2e21756 full-gate result is not transferred to this newer source. |
| Bombay ea2b5b4 | All24 native aarch64-Darwin Nix gates pass in4337.683 seconds; exact585 source hashes and clean Git are conserved. Independent review confirms the complete expected gate set and selected registry owners. Delivery advances ff-only toea2b5b4. | Includes sender storage, stash construction, supported compiler diagnostics, JSON and corrected resource environments. New DX15/16 design work is excluded; no all-systems, archive or continued-cache claim. |
| Bombay integration120cc2d | Isolated merge imports main8d809b3/PR370 into fully gated ea2b5b4. All363 selected native tests and both Node hosting journeys pass current/MSRV debug/release; all385 selected source hashes are conserved. Original assertions/effects are conserved. A pre-edit regression makes export clients inherit the existing configured reply capacity; its default-capacity inverse fails and restored control passes in both profiles. | All24 native aarch64-Darwin gates pass in4663.558 seconds at exact120cc2d, with all586 tracked hashes and clean Git conserved; optimized workspace tests749 passed/0 failed/1 intentional ignored subprocess fixture. Independent gate/source review passes and delivery advances ff-only to120cc2d. This excludes the separate native custom-service candidate and makes no all-systems or new archive claim. Static ActorRef/EstablishedRecipient layouts remain40 bytes ordinarily and grow40→48 with remote, in both profiles; no heap reduction claim. |

| Bombay release integration790fa08 | Fresh fetch selects main75c8db4/PR371. Isolated merge imports0.1.3 metadata and protected-counter allocation diagnostics/isolation; library production remains identical to verified120cc2d. | All24 native aarch64-Darwin gates pass in5404.589 seconds with586 conserved source hashes, unchanged HEAD and clean Git;104 workspace summaries report749 passed/0 failed/1 deliberate parent-invoked subprocess ignore. Both independent reviews and the exact completion audit pass. Delivery advances ff-only fromac1375c after the verification record. Current store20 available/four absent is recorded separately; two earlier exact output restorations and both cache-audit failures remain. No continued-cache, all-systems, new archive or custom-interpreter acceptance is claimed. |

| Owner 1104b95 | All ten native aarch64-Darwin Nix gates pass in177.111 seconds with817 unchanged tracked file hashes;964 workspace tests,26 external tests per debug/release profile,24 actual-archive runtime tests per profile and seven exact privacy denials pass. Seven IO plus eleven compatibility tests pass pinned1.99 debug/release; retention/corruption inversions are killed in both profiles. | Standard OwnedItem port only; no catalogue migration, registry publication or native custom-service integration claim. Initial doc-expectation and archive snapshot failures remain recorded. |

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
PR98 is ready at186266e: the16-document pruning is integrated, all ten native
gates and exact-head API/CI/mutation checks pass. Its merge/publication question
is pending; existing PR-specific release approvals do not authorize it. No selected open criterion is closed by this
record. Networking, durability, global identity and deployment remain explicit
DX23 deferrals; all other open rows continue under the persistent active goal.


#### Final shutdown decoration — DX09/21

Expose the existing FinalizeOnShutdown owner through
`actor.finalize_on_shutdown(reaction)`. This is API composition policy, not a
new actor law: the infallible pure reaction emits its complete sends/creations;
the existing wrapper forces normal stop. Reuse ShutdownReaction, ShutdownEvent,
FinalizeOnShutdown, event injection, native delivery and retirement. Preserve
call-order precedence: an outer stop wrapper consumes shutdown before inner
finalization; outer finalization executes its own reaction. No I/O in the fold,
new wrapper/type/trait/bound/default/dependency/state or interpreter change.
The existing real FinalizationProbe publishes Ready then Finalized; its complete
application/join test becomes the missing-method RED before production. Add
focused two-order effect/next-state comparison and real refusal/retirement
coverage before other callers migrate. Ordinary domain actor state and all
runtime/control sums stay unchanged. Expected actor_ext.rs plus focused tests/
compiled guidance, roughly net15 production lines, zero public types and one
method; source/before counts captured before edits. Cross-check pure transition,
shutdown precedence, initialization/effect ordering and exact terminal custody.
The pre-edit native journey fails E0599. The retained method returns that exact
registry owner: production+11/-2/net9, tests+233/-7/net226, public types+0/-0, methods6→7, owner module1→1 (79→88 lines).
There are no new control states, subordinate sums, transition branches or
runtime fields. Continue/Goto/Stop and all24 retirement alternatives remain at
their existing owners. Existing FinalizationProbe retains only its current
optional known observer, needed for the final notice; no arrival history,
repeated cause, false cardinality, semantic boolean or nested transition owner
is introduced. Cross-check shutdown.rs, the owner's complete sends/creation
regression, canonical transition/composition laws and native retirement.
Provenance and aggregate-drift disposition: pass for this bounded API addition.

The real journey and both timer orders retain final notices, shutdown replay
and complete joined retirement. Zero-duration expiry follows its own policy;
native reaction panic retains the original payload, actor and independent
work result without a fictional successful settlement. Ignoring the reaction
makes the optimized panic regression fail Normal versus Panicked; exact source
restoration passes. Two shutdown layers retain distinct occurrences and deny
inferred shutdown with E0283; no path annotation or capability widening is
retained. Current1.99 passes the complete native and six reviewed compile fixtures;
1.96 passes all20 runtime/pure cases in both profiles, and its six separate
compile cases preserve expected acceptance/error categories. Two existing1.96
snapshot renderings differ, so their exact-snapshot run remains failed; no
snapshot is blessed. Strict workspace/all-target/all-feature lint passes.
The pure two-order policy witness selects the outer typed event: an outer stop
returns complete empty effects and normal stop without invoking the reaction;
outer finalization retains its native panic. The ordinary Application still
denies ambiguous duplicate shutdown lanes. The final optimized workspace suite passes651 tests with one existing
subprocess-only skip in70.539 seconds; strict all-target/all-feature lint and
both MSRV profiles pass. The prior650-case cold run took620.293 seconds; these
differ in cache state and do not establish a performance improvement. Full
all21 native Nix gates pass at80348a6 in2358.777 seconds, with every captured
source file unchanged. No Darwin signing or disk waiver applies; incompatible
Linux/x86_64-Darwin systems are unexecuted. Extracted Engine/macros/runtime preserve
all61 production Rust files, the complete guide and run_with.rs byte-for-byte.
Their four current/MSRV debug/release runs pass21/21/20/20 cases in92.801 seconds;
current compiler runs all six exact fixture snapshots per profile. Only the two
unpublished extracted owner packages are patched; all144 other registry records
remain unchanged. No registry publication is claimed. DX09 retains its remaining
decoration audit.

#### Timer ownership — DX13 (pre-edit)

The desired ordinary chain supplies delay and reaction without coordinating
TimerId values. Independently declared wrapper occurrences must retain separate
reactions, while an explicit domain timer ID must retain its documented
replacement/cancellation meaning. This is derived composition and deliberate
Bombay timing policy, not an actor-model allocation law.

Inspect the registry OneShot/Periodic/TimerLease, path-indexed ingress and
LocalTimers: the interpreter currently keys one TimerQueue by TimerId alone.
The primitive queue accepts arbitrary concrete keys and replaces equal keys.
No type-name dispatch, global counter, function-address identity, reflection,
unused marker or caller structural path may manufacture occurrence identity.

Before production, add a real session journey with two zero-delay one-shots
(reminder and warning) plus a zero-delay periodic expiry. Actual external
recipients collect the complete accepted notice prefix after joined termination;
all original root retirement fields remain checked. Explicit distinct IDs must
deliver all three notices. Sharing an ID tests the falsifier of a hidden fixed
ID: it must reveal replacement, not be called a broken primitive contract.
A separate compile probe tests the desired no-ID syntax before any facade edit.
Only a complete scoped-key proof can justify owner changes and migration.

Budget: one focused test file and these records; zero production lines/types.
Existing actor facade, ActorExt, ApplicationScope, complete retirement oracle,
external close/drain, typed schedules/elapsed events and TimerQueue are reused.
Actor controls, timer leases, admission sums, queue algorithms and transition
authorities stay unchanged. Before checkpoint: no new production states,
branches, fields, modules or public spellings. Post counts, full custody,
reorder/stale/exhaustion/cancellation witnesses and disposition remain required.

Scoped-key candidate (before edit): reify only existing Here/Inside timer
ingress positions into a private actor-local depth, and key the unchanged
TimerQueue by (depth, TimerId). Reuse existing event injection and every
request/result/lease type. This derived key law has real outer/inner
substitutions and requires no equality/hash bound on user messages. The real
session tests precede the candidate: distinct IDs must keep all notices; equal
explicit IDs must still replace. Applying a lane namespace to every schedule
is rejected if it changes that explicit-ID equation. No facade migration or
default-ID constructor is retained. Expected temporary change: timers.rs only,
roughly20 production lines, one private proof trait, no public types, controls
or transition branches. Exact source restoration and a native semantic
falsifier are required before this experiment is recorded.

Timer checkpoint result: the real baseline passes both tests on1.99/1.96,
debug/release; strict all-feature focused lint and pinned formatting pass.
Production+0/-0/net0; tests+158/-0/net158; public types+0/-0. New test declarations
own one current session recipient and three emitted domain notices; they store
no arrival history or semantic flags. The existing root/custody oracle checks
all retirement fields and current source provenance. Domain actor controls,
TimerLease/admission alternatives, transition branches and public spellings
stay unchanged. No-ID compile RED is E0061/E0282, not an inferred API success.
The scoped candidate temporarily changes production+18/-2/net16, adds one
private routing proof and no control state/module/public type; explicit-ID
replacement fails Reminder/Warning/Expired versus Warning/Expired in both
profiles. Its private trait, bound and tuple-key representation are all removed.
Cross-check time/domain.rs, OneShot/Periodic, generation-safe TimerQueue, typed
event/effect composition, runtime timing and complete Driver retirement.
Disposition: baseline pass; blanket lane-key candidate reopen, recorded in
DEAD_ENDS. The restored all-feature optimized native journey also passes both
cases in27.294 seconds. Ordinary interpretation starts at Here and standard
SendLayer composition lifts through Inside; named requests remain an open
InterpretItem port. A future automatic-name proof must preserve that existing
named port, not impose a new private path requirement on every request. Broader
full gates and automatic owner semantics remain unproved.

Disjoint timer-key test (before lowering): an exhaustive private key sum keeps
Named(TimerId) separate from Owned(ingress position). Named replacement/cancel
selects only that name; an owned timer can replace/cancel only its own slot.
Generation tokens remain the primitive queue's exact authority. Test the two
owned slots alongside a repeated domain name using the unchanged registry
TimerQueue, including stale and foreign cancellation, equal deadlines and
original emitted values. This is a data-representation probe only: it must not
claim a new wrapper/API or impose a private path bound on existing named
requests. Expected one research-local crate, zero retained production changes.
The primitive generic key is reused; no scheduler, registry or counter is added.

Disjoint-key result: the unchanged published TimerQueue0.2.0 and actual
TimerId0.26.0 accept Named(TimerId) and Owned(usize) as a closed key sum.
Two tests pass on1.99/1.96 in debug/release, with strict lint and formatting;
the optimized collision inverse fails both cancellation and live-count checks,
and exact source restoration passes. The two keys retain distinct authority;
equal named keys still replace, equal deadlines retain the surviving insertion
order, stale/consumed tokens are inert, and a foreign queue cannot cancel either
namespace. Production/API/control states/branches/modules delta is zero. The
research-local test source is94 lines with two private sums (2 key alternatives,
4 emitted values); each surviving value is either the queue key required for
replacement/cancel or the non-cloned notice required for exact delivery. No
arrival-history state, semantic flag, repeated cause, nested transition authority
or caller structural syntax is retained. Cross-check the generation-branded
primitive queue, named schedule/elapsed contracts, timer leases and the native
session witnesses. Disposition: pass for the key equation only; no lowered
automatic request, wrapper identity, public API or programme completion follows.
Evidence: timers/disjoint/RESULTS.json, PROVEN.rs and SOURCE.json under the
existing temporary evidence root. Repository tests remain the native session
witnesses; this external probe is not a portable end-to-end acceptance fixture.


#### Automatic timer ownership — current experiment, DX13

Pre-edit policy: OneShot/Periodic/Deadline/ReceiveTimeout own their event occurrence
and generation; ordinary callers need no TimerId. Named ScheduleAt/ScheduleAfter
retain shared-name replacement and custom InjectEvent ports. Desired caller:
`actor.with_one_shot(delay, reaction).with_periodic(interval, reaction)`, through
both shutdown orders. Reuse TimerLease, initialization/settlement products and the
existing queue; compare separate owned requests against the same custody family
indexed by the two concrete targets TimerId/Here. No ID0, fallback, extra lifecycle
owner, caller marker or generic default is permitted. Receipts/rejections retain
the actual target, generation and deadline. Measure queue-key and deleted-field
bytes rather than assuming a memory gain.

The real no-ID consumer against published0.26.0 fails six E0061 diagnostics and
two inference consequences. Both oracle reviews accept only its actual three
ordered notices, actor origin and normal retirement. Periodic stops at its first
tick; there is no shutdown request, rearm/cancel/stale/rejection/exhaustion proof,
and the original retirement oracle does not inspect inner settlement contents.
The independent named-port regression uses one explicit ID through a genuine
custom Reminder event; its two cases pass before production lowering.

Initial bounded ledger: at most eight owner/two runtime production files, about
+200 lines, at most one target trait, no new actor states. The selected shared
family was archived and removed after source inspection found four existing
named calls whose port it made private. This visibility change had no law or
regression provenance. No candidate compiler/test run is claimed; the target
family itself is unproved. See DEAD_ENDS.md. Seven owner/two runtime production
files are restored byte-exact; only pre-edit regressions and coordination remain.
After checkpoint: production/API/state/branch/module delta0. Timer lease4,
absolute schedule2 and admission2 alternatives remain; durations, named IDs,
leases, reactions and accepted queue values keep their existing owners. There
is no retained arrival history, repeated cause, cardinality change, extra
transition authority, semantic boolean or structural caller syntax. Cross-check
named scheduling, TimedEvent, lease generation and LocalTimers contracts.
Disposition: reopen; do not retain the removed production candidate. Supervision
and pool caller types now have user priority. Full candidate patch/source hashes
and both pre-edit witnesses remain under the existing external evidence root.

#### Exact termination decoration — DX09 (pre-edit)

Derived composition law: a same-actor observation of one established recipient
uses the existing EstablishedTerminationMonitor, preserving its exact affine
ObserveEstablished request, grant/cancellation authority, original terminal
report, inner initialization effects, reaction Actions and independent joined
retirement. Endpoint termination and observation ordering are existing Bombay
policy, not actor-model allocation guarantees. Inspect the published0.26 target
states and native InterpretEstablishedObservation before lowering.

Desired caller syntax is actor.observe_termination(request, reaction), before
or after stop_on_shutdown; the exact existing owner is returned. Observation
IDs remain explicit domain correlation, not fabricated defaults. Use an actual
pre-root external endpoint and forward the entire Stopped relationship/outcome/
timestamp as an owned typed notice; ordinary callers need no event path, wrapper
type annotation or runtime lookup. First execute both native journeys through
the owning constructor, then prove the missing facade method fails before any
production edit. Add a shutdown-while-observing witness before broader migration.

Expected production: actor_ext.rs only, about20 net lines, zero new public
types, fields, control states, errors, modules or dependencies. Existing Protocol
equality and EndpointAddress bounds belong only to this operation because it
selects a real established endpoint of either unrelated message protocol.
Reuse the current target's complete state sum, typed observation interpreter,
ApplicationScope, exact established delivery and complete retirement oracle;
no competing monitor, request, receipt or cancellation representation is added.
Before drift checkpoint: ActorExt7 methods; owning target alternatives remain
the published exhaustive EstablishedTermination sum; one transition authority.
Post counts, complete failure custody and disposition remain required.

Exact-observation failure witness (before test lowering): close the genuine
report recipient, stop the actual watched endpoint and await root termination.
The reaction's rejected delivery must remain in complete joined retirement,
including its original recipient, relationship, peer outcome and timestamp.
Delivery rejection is independent of the observed peer's normal outcome and
the reaction's stop verdict. Reuse existing Completed, ActorCustody,
ActionSettlement, generated named WatcherSettlements and exact closed-delivery
rejection; add no production representation or bound. Inspect these owner
signatures before writing the exhaustive custody assertion.

The native refusal witness corrects an expectation rather than production: the
first debug/optimized assertion predicted EnvironmentFailed, but the actual
Driver returns Normal. Its classification rejects initialization refusal; a
completed transition's ordinary send refusal remains an owned settlement beside
the explicit stop. The corrected witness requires Completed/Stopped plus the
whole rejected delivery. No new error classification or actor policy is added.
The disproved expectation and optimized trace remain in observation evidence.

Exact-observation checkpoint: production+23/-3/net20; tests+346/-0/net346;
public types+0/-0, methods7→8, production modules1→1 and stored fields+0/-0.
The existing target has10 control alternatives before/after, five report
alternatives and18 accept_report match arms before/after; its573 pre-test
source/rustdoc lines remain unchanged. Facade transition branches stay0→0: the
method delegates construction to that owner. The new fixture stores only the
current real report recipient; generated named sends/settlements are the existing
authoring expansion, not a second interpreter or transition authority. The
existing target retains exactly its current request, correlation, grant or
rejected cancellation according to its unchanged phase; the delivery rejection
owns the whole report. No arrival-history field, repeated cause, cardinality
guess, semantic flag, caller path, no-op policy or erasure is added. Cross-check
established observation, TerminationMonitor and exact-delivery contracts, native
observation interpretation and Driver transition/initialization classification.
Disposition: pass for this facade; the broader decoration criterion stays open.

The final source at tests/observation.rs passes four native cases across1.99/1.96,
debug/release and default/all features (32 executions). All17 existing native
interpreter regressions pass both compilers/profiles (68 executions), including
original request rejection, stale/cancel races and wrapper orders. Strict
workspace/all-target/all-feature lint and formatting pass; the final focused
matrix takes276.749 seconds, including compilation. Initial zero-match filter
runs are not regression evidence; the corrected module selection executes17.
No memory reduction or throughput claim applies: the exact existing concrete
wrapper and its fields are returned. The source guide links the one-word
observation.rs consumer. Archive and final repository gates remain pending.

Actual archive verification atfcb85ca: Engine/macros/runtime archives preserve
all61 production Rust files, the complete guide and observation.rs byte-for-byte.
Only the extracted unpublished Engine and Bombay macros are patched; all144
registry package identities/checksums match the earlier verified archive graph.
Native cases pass4/4/4/4 and guide doctests pass3 per configuration on both
compilers/profiles, with the three existing schematic snippets still ignored.
The package/archive matrix takes37.734 seconds. This is local package verification,
not registry publication. The completed task-owned Cargo cache is removed
only after all processes exit; source, toolchains, archives and evidence survive.
Disk availability is5.3GiB before the new full native check; no previous disk or
signing failure is waived. All21 native aarch64-darwin repository gates for
fcb85ca passed in2504.413seconds, with all485 captured tracked-file hashes
unchanged. Other platforms were not run. Final nextest, main integration and
requirement reconciliation remain required; no selected requirement or
persistent goal is marked complete.

#### Whole-item receiving — DX16 (pre-edit)

Derived interface law over the selected 0.26 receiving contract: when an action
item is interpreted as one whole owned input plus its independent reply, request
authors should state accepted/rejection/prerequisite types and retention policy
once. Custody, input and reply shapes, prepare/input/finish then belong to that
one standard loan. Advanced item contracts retain their explicit ActionItem
implementation. No actor behavior, interpretation/source ordering, failure
vocabulary, request representation or accepted-value policy changes.

Smallest caller comparison: the actual OpenFile/ReadEnvironment consumer repeats
those same aliases and forwarding methods twice. First prove that metadata-only
ActionItem declarations fail on current source; then write the complete pending,
late/replay and OS-rejection consumer using `impl OwnedItem for OpenFile` and
`impl OwnedItem for ReadEnvironment`. OwnedItem declares only the existing three
settlement payload types and retain_accepted policy. Its one ActionItem blanket
implementation must delegate to prepare_item/finish_item, preserving the exact
existing whole-value custody equation and conditional input borrowing. Do not
introduce new generic slots, defaults, wrappers, dependencies, async consuming
callbacks, status flags, source ports or erased values. Coherence must be proved
against real owner packages and external implementations before retention.

Ordinary-Rust comparison: existing helper functions already own preparation and
finalization but do not supply nominal trait implementations; aliases and inherent
methods cannot do so either. A request wrapper changes the exact rejection/input
type without a new invariant. Default ActionItem methods with extra custody
bounds would constrain advanced implementations even when overridden. Test the
one explicit whole-item implementation port before considering any macro.
Expected production files: effects/sending.rs, effects/mod.rs and lib.rs; roughly
60 added lines and one public trait. First stage changes only two real consumer
requests and their existing custody regressions. Keep all different custody
equations explicit; broader migration follows only after complete comparison.
Baseline sums remain InterpretationProgress(3), Interpretation(2),
ItemSettlement(4), SettledItem(2); actor control states, transition branches,
creation provenance and all interpreter owners remain unchanged. Subordinate
values are the original pending request, outside reply and untouched siblings;
no history labels, duplicated causes or second transition authority are needed.
Disposition before the experiment: unproved; record the compiler falsifier and
remove the production candidate if the exact port cannot compose.

Focused port checkpoint (unpublished 1104b95 on `codex/devx-items`, based on
98983b4): the metadata-only ActionItem attempt fails E0046 for the six repeated
loan members; the intended OwnedItem consumer then fails E0432 before the new
port exists (with consequential ActionItem errors). Seven actual IO tests now
pass pinned 1.95 debug/release. They preserve pending originals, rejected path/name
allocations, advanced File receipts, String allocation through repeated source
offers, and an acquired reply through producer panic and repeated finalization.
Changing only the blanket retention delegation to discard its receipt kills
both source-offer tests in both profiles; restored controls pass. The owner
workspace passes 964 tests. The final external source suite passes 26
tests; the actual extracted archives pass 24 runtime tests per debug/release
profile plus all seven original privacy denials checked by exact compiler JSON
codes (not snapshot blessing). Strict focused Clippy passes. A corruption
inversion is independently killed in both profiles and restored controls pass.
The original mismatched diagnostic route still fails E0277 and its matching
String-route control compiles; all 69 annotated rustdoc denials pass after the
one-line annotation correction. On Bombay pinned 1.99, the seven IO plus eleven
existing source-custody/order tests pass debug/release. Final native Nix passes
all ten expected gates in 177.111 seconds, conserving all 817 tracked file hashes.
No native Application custom-service acceptance is claimed.

Full-gate falsifier: the first native Nix run exits 1 after 253.339 seconds.
The diagnostic-route compile-fail body still rejects the mismatched u8/String
protocol with E0277 (ActionItem not satisfied), and points at the existing
DiagnosticRoute bound; its annotation expected E0271. Preserve the complete
witness and verify the corresponding String route compiles before updating
only that diagnostic annotation. No capability, bound or valid construction is
to change. Archive runtime/denial verification is separate: the first complete
archive harness fails two trybuild text snapshots because extracted package
paths are printed literally; E0624 remains the actual privacy failure. Do not
bless the snapshots or count that failed command as passing.

Independent review requires the corruption injector to match the actual lawful
pending loan `(Some(input), None)` before writing its fault, so it cannot erase
an already-acquired reply. The test now enforces that precondition. Canonical
rustdoc links the standard OwnedItem path from ActionItem, includes the real
file-request declaration, and the crate README explains migration and retaining
receipt policy without implementing both ports. The preceding full native run
passes all ten gates in 187.457 seconds with frozen hashes; final strengthened
test/documentation gates pass independently on the final source.

Current measured ledger, including newly tracked test files:
production +105/-6/net+99 (including README/rustdoc); tests +647/-1/net+646;
public API +1/-0 types.
This adds an interpreter capability; it is not a net production deletion.
The two real request declarations shrink their shared request module 195→135
lines; the advanced ActionItem port remains available and unchanged.
InterpretationProgress 3, Interpretation 2, ItemSettlement 4, SettledItem 2,
SourceProgress 3 and SourceCustody 4 alternatives are unchanged. No actor control
state or transition authority changes. The affected sending owner is 2536→2627
lines, 57→58 match expressions and 33→34 public type spellings. The sole new match is the existing
whole-item loan gate (two alternatives), not another interpreter or source traversal. Three core
modules change plus the Core README and one Actors rustdoc annotation; no module,
request wrapper, marker, generic slot or state sum is added. Every retained
subordinate value still owns the exact pending request, acquired reply, or
untouched ordered sibling; no arrival history or duplicated cause is added.
Cross-check: selected receiving-loan source, adapter contract, atomic runtime
settlement, retained-core inventory and atomic essence/solution preserve their
existing ownership and initialization laws. Disposition: pass for the retained
port batch 1104b95. Mechanical catalogue migration remains gated on the remaining
two-template/wrapper and complete downstream witnesses; DX16 remains open.

Owner mutation-campaign gap, pre-edit: PR100 head1104b95 passes the Linux build,
API audit and each mutation shard, but aggregate CI38700008964 rejects five
surviving mutants across the new blanket methods and five unaccounted function
rows. The external IO regressions are not in that campaign's owner/testkit
selection. Run the same real IO regressions as an owner integration target too,
without duplicating their oracle; bind fixture contents to each compiling
package's actual manifest. Keep the external target to prove third-party use.
No production semantic changes are proposed. First kill the exact surviving
prepare/input/finish/retention mutations under the actual owner selection; then
add only the measured new function floors and the genuinely unviable default
method to the existing baseline. Do not lower any existing floor, exempt a
survivor, or count an unviable compiler error as a kill. Expected additions: one
owner test entrypoint, fixture-path correction and five baseline entries; API
and executable production +0/-0. All prior candidate gate receipts retain their
exact scope; fresh final gates and exact-head CI are required for this change.

The corrected campaign runs the same seven IO tests from the packaged Core
integration target; the external workspace includes that source rather than
duplicating its oracle. The request fixture is moved unchanged, and expected
file contents use the compiling package's actual manifest. The pinned targeted
campaign tests all nine candidates: five caught, four unviable, zero survivors
or timeouts in54.109 seconds. Only four new measured floors (1/2/1/1) and one
known-zero-viable default are added; every existing baseline value is unchanged.
Owner tests (7/profile), external tests (26/profile), extracted Core owner tests
(7/profile), extracted consumer tests (24/profile), and seven original privacy
denials pass. Both owner and external focused Clippy and workspace formatting
pass. The extracted Core test explicitly selects the extracted macros package;
its Cargo lock is regenerated for that paired archive graph. This proves source
archive self-containment, not an unpublished registry selection.

Preserved failures: two attempts selected a broken host cargo-mutants executable;
the valid run invokes the pinned Nix executable directly. An overlapping control
run shared the mutation build directory and observed a mutant artifact; after
the campaign ended, package-local artifact cleanup restored all seven tests.
Further mutation campaigns must use a separate target directory. The first
extracted owner run also rejected the packaged companion checksum; the paired
archive graph above is the explicit correction, not a passing original run.

Aggregate checkpoint: production/control states, subordinate alternatives,
transition branches, modules and public spellings are unchanged (all deltas0).
The test relocation adds30 net lines; baseline metadata adds5 net lines. No
arrival history, repeated cause, cardinality assumption, nested transition
authority, semantic boolean or structural user syntax is added. The same
receiving-loan and complete settlement laws remain authoritative. Disposition:
pass for test selection and measured baseline correction. All ten native
aarch64-Darwin Nix gates then pass in257.491 seconds, preserving all818 tracked
hashes and passing971 workspace tests. The sandbox audit cannot inspect registry
yank status; a separate fetched online `cargo audit --deny warnings` passes with
65 dependencies and1329 loaded advisories. Follow-up9948bdd commits only the
verified test relocation and measured baseline additions. Independent two-pass
review confirms package self-containment, unchanged oracle, all four genuine
unviable compiler denials, unchanged existing baseline entries and final hashes.
Fresh exact-head CI38704350167 then passes every job, including all eight
mutation shards and the strict aggregate; all other PR checks also pass.
`OWNED-ITEM-COVERAGE-*`, `OWNED-ITEM-DIRECT-MUTANTS.*` and
`OWNED-ITEM-ORACLE-CONSERVATION.json` preserve the raw commands and scope.

#### Typed interpreter ownership — DX15 (pre-edit probe)

Derived extension law: application-supplied requests must name their concrete
interpreter owner statically, while standard requests retain the native owner.
The proposed ownership equation is request::Interpreter = the supplied service;
it changes neither ActionItem's receiving loan nor complete settlement/result
custody. Desired application syntax is Application::new(actor).with_interpreter(
|| Services), supplying a constructor for each actor's owned instance, with
standard timers/delivery still available and unsupported leaves
denied. Existing native ActionItem/InterpretItem paths are inspected; there is
currently no application interpreter extension method.

Before lowering, test whether a local open request trait with an associated
interpreter can distinguish a blanket external route from the concrete standard
ScheduleAfter route under Rust coherence. Both routes directly forward the
existing receiving-loan method, without new request wrappers, path markers,
fallback, lookup or async erasure. This compiler-only ownership probe is not
a native journey, a proposed production surface or proof of task retirement.
Budget: an external port crate and a separate consumer crate; zero repository production changes, bounds,
states, fields or public types retained. Reuse actual0.26 ActionItem,
InterpretItem and ScheduleAfter. If coherence rejects it, record that falsifier
and reopen rather than repair it with caller tags or arbitrary bounds.

The independent consumer proves two genuine operations, OpenFile and
ReadEnvironment, using the same concrete Services owner beside an actual
ScheduleAfter/TimerQueue. Both tests pass on pinned1.99/1.96 in debug/release
(eight executions): acquired File/String/timer receipt survive dropping the
interpreter, cold cancellation preserves the original PathBuf allocation, and
OS rejection returns the same owned request. Each compiler separately rejects
a consumer claiming the standard ScheduleAfter owner (E0117) and supplying the
wrong external owner (E0271). Associated-owner coherence succeeds without tags
or new routing bounds. These are external port tests only: native Application
extension, wrapper orders, asynchronous task custody and retirement remain
unproved; DX15 remains open. Sources and exact commands/results are preserved
under /private/tmp/bombay-retirement-schema-kgazqjhv/extension.

The graph-ownership probe subsequently verifies separate non-Clone/non-Sync
instances from shared construction authority on both compilers/profiles, with
ordinary absence preserving the native120-byte probe layout. Arc<()> instead
measured128 bytes and is rejected. Fn + Sync permits interior state in the
constructor; it promises neither immutable captures nor isolated backing
resources. Only its returned service instance is actor-owned. The associated
request-owner trait must live beside the native dispatch implementation for
coherence. None of this proves native Application execution or heap cost.

Actual selected0.26 request loans now have three source-backed tests: suspended
File acquisition preserves accepted prefix, original permission-bearing request
and untouched suffix; resumption/completed replay preserve both advanced File
offsets; environment lookup and closed permission retain actual values and
complete original rejections. All three pass on1.99/1.96 debug/release, and strict
focused Clippy passes. Consuming the request before its await fails the intended
pending-custody assertion in both profiles; restored controls pass. Exact
commands, identities and source hashes are SERVICES-CUSTODY-MATRIX.json and
SERVICES-CUSTODY-INVERSION.json under the existing evidence root. The suggested
async-fn rewrite fails because it captures unrelated Event/Path lifetimes; keep
the existing receiving-loan signature rather than adding those bounds.

The native Application witness is deliberately failing before implementation:
separate receipts record missing with_interpreter (E0599) and missing associated
request-owner port (E0405). It specifies successful initialization, rejected
active timer-stop and independent native cancellation, including complete
retirement products. These native bodies remain unproved until the runtime
extension runs them. Both reviews accept only the executable request-loan
scope; native root/child/cold/Entity construction, source/retirement, host refusal,
constructor/destructor panic and wrapper orders remain open. No Behavior
production change is justified by these Bombay-local gaps.

Main integration selectedfb533b5 after the user requested current main in the
isolated Bombay branch. Main#359 squashed prior DevX throughc3b74ab; reconcile
that shared source before retaining later changes from both branches. Main's
independent work/native/notification retirement product takes precedence over
the older two-part consumer assumptions. No new semantics or public types are
selected by this merge. The pre-merge21-gate result above cannot stand in for
verification of the combined revision.

Combined Bombay source is committedbdaf904 (parentsfcb85ca andfb533b5),
with a clean isolated feature tree. Git confirmsfb533b5 is an ancestor. Resolving
the squashed overlap fromc3b74ab preserves main's independent retirement
receivers and all later DevX laws. The focused all-feature consumers pass39
cases plus their current protocol/factory compile denials; one existing
subprocess case remains ignored. Strict workspace/all-target/all-feature Clippy
and pinned formatting pass. Compared with selected main, retained DevX
production is+203/-8/net195 across4 files, with no merge-invented state, bound
or capability. Counter main shrinks86 to51 lines through upstream separation
of verification from ordinary execution; this is ceremony evidence, not a
memory or throughput claim.

The user requires upstream integrations to improve caller experience as well
as preserve features. Bombay AGENTS now states this rule. Independent
work/native/notification receiving is retained, but its exposed three-part
bookkeeping and stale cleanup inspection wording must be reconciled with the
ordinary Application path. DX01/05/17/19 remain open for that complete result
journey; no transparent tuple wrapper, discarded notification failure or
example-only helper is accepted. Fresh combined native Nix checks and optimized
workspace nextest are running against528 frozen tracked-file hashes. Pre-merge
passes do not satisfy these combined gates.

Native request-owner CI law, pre-edit DX15/19/25: preserve the already proven
absence/explicit-unit distinction as permanent caller compile denials. Reuse the
actual OpenFile ActionItem, real Services implementation and ordinary
Application::new(...).run() path. Omitting the constructor must fail even when
that request's declared interpreter is unit and unit implements its real IO;
supplying a constructor with a different output must fail the exact associated
owner equation. The existing supplied-unit native test is the positive control
and retains actual File, original OS rejection and every retirement lane. No
new production API, marker, callback, event, bound or fixture-only policy is
needed. Add two invalid caller programs and a checked compile harness after the
current frozen replay; keep their coverage separate from runtime test names.

#### Joined application result — DX04/17 (pre-edit comparison)

Interface policy: ordinary application results expose their acquired work,
native root retirement, notification receipts and Entity shutdowns by semantic
names. Derived custody law: these facts coexist independently; selection of
completed work must return the complete original application result on any
other phase. Borrowed inspection consumes none. Native receiving order remains
work publication, cleanup join, original root, notifications, then actual Entity
shutdowns. Reuse ApplicationOutcome's six work alternatives, the existing
independent receiving futures and ActorNotificationReceipts; add no lifecycle
engine, protocol erasure, allocation or clone.

Desired ordinary syntax is let outcome = Application::new(actor).run_with(...),
then outcome.work_output(), outcome.retirement() and outcome.notifications().
Retain deliberate owned peeling of every original value and paired execution.
Inspect the smallest complete native consumer before selecting a representation;
compare extending the existing sum against a named coexisting product. A product
is acceptable only if it owns the whole acquired result and hides positional
knowledge; a renamed tuple with no caller decision or complete rejection law
fails. Root-only, heterogeneous topology, Entity and HTTP must share the selected
ownership equation. Current run results are triples; Entity adds a fourth value.

Budget for this comparison: external compile/native consumer only, zero
repository production changes while combined gates remain frozen. Write the
missing-projection RED first. Baseline phases are6; fields and runtime effects
are unchanged; no new state or capability is accepted from compiler fallout.
Native failure, non-Send local work, cancellation and notification/retirement
independence must be proven before production lowering or catalogue migration.

The two native consumers pass using explicit tuple peeling, while their
position-free syntax fails E0599 for work_output/retirement/notifications and
whole-result completed selection. This isolates a Bombay result-owner gap,
without changing Behavior. The candidate comparison uses two plain products:
the whole acquired Outcome and owned Retirement accompanying selected work.
The existing six work alternatives remain unchanged; no new transition sum,
capability, trait, allocation or bound is selected. Product fields preserve
the original tuple's destruction order: work, root, notifications, families;
owned completed work moves cleanup into Retirement without cloning. On
selection refusal, every original component returns in the whole Outcome.
Both proposed public products remain external probes until native entrypoints,
Entity/HTTP and independent failure witnesses validate the complete design.

The merged optimized workspace passes717 all-feature tests (one existing skip)
in860.467seconds including compilation. Full native Nix fails after1001.945
seconds against unchanged528 hashes: the imported protected-counter isolated
lock vendors0.25 owners despite the runtime's0.26 requirement. Main adds
three native gates, so the combined scope is24, not the pre-merge21. Repair
only that lock's already selected owner versions/checksums, preserving its
independent native transport witnesses; zero production/state/API delta. The
actual failed vendor resolution is the pre-edit regression. No failure is
waived and cancelled gates are not counted as passes.

The native probe now covers six real journeys, including actor overflow and
destroying the selected host after caller-local work is Ready. The latter
retains the same Rc allocation plus independent cleanup, native receiving and
notification receiving errors. All six pass before any production edit.
Dependency identities/checksums for all62 registry packages match the merged
workspace; an initially floated Tokio patch is replaced by its exact graph.
The generic audit finds every native cleanup producer returns unit: remove
the proposed Retirement Cleanup parameter before retaining it. Root and family
payloads have multiple real substitutions; native cleanup does not. This
narrowing follows source ownership, not a compiler request.

Entity inspection exposes the remaining design gap: the candidate's families()
returns the original positional receiving product. Ordinary code must instead
select its exact declared role with outcome.family(CountersRole), retaining
the genuine receiving error and full shutdown/metrics/disposal values. This
is a derived role-projection law; reuse the existing sealed EntityFamilyAt
head/tail selection pattern and actual family protocol. Write its missing-method
RED before choosing any new trait, marker, product or bound. No generic
classifier or whole-Environment replacement is authorized by this gap.

Joined-result comparison verification: six native cases pass pinned1.99/1.96
debug/release (24 executions), strict current lint and formatting. Two independent
inverses fail in both profiles: classifying native receiving failure as unfinished
work erases Ready selection; manufacturing successful notifications loses the
cold cancellation's original receipt errors. Exact restoration passes four
focused reruns. Raw results, source hashes and62 dependency identities live in
results/VERIFIED.json, INVERSES.json and DEPENDENCIES.json below the existing
external evidence directory. Sizes of completed counter/startup refusal/Entity
results stay872/648/768 bytes, respectively; no heap or throughput gain claimed.

Aggregate checkpoint: application work alternatives6→6, producer custody5→5,
actor/runtime control sums unchanged; new transition authorities0. Candidate
subordinate sums0→0, data products0→2, selection branches0→2, source
modules0→1, production-like lines0→141, public type/method spellings0→14
plus two private constructors. Repository production/API delta is0. Each
surviving field owns actual work phase/output, cleanup error, root custody,
notification result or family receiving values needed by caller recovery.
Arrival history, duplicate cause, cardinality assumptions, semantic booleans,
new bounds and nested actor engines: none. Structural family inspection still
survives; outcome.family(CountersRole) fails E0599 in its actual native consumer.
Cross-checks: runtime authoritative-fact conservation, Driver ownership and
execution preservation contracts, atomic architecture protocol reuse and
fixed-supervisor query/retirement law. Disposition: reopen the complete API;
retain no candidate production representation. The custody equation is proven,
but family role projection and the HTTP journey must close before lowering.

The imported protected-counter gate passes ataf87ef4 after its isolated lock
selects the same0.26/0.14.2 owners:55 tests each in debug/release, formatting and
strict lint,291.608seconds. Fresh combined24 native gates pass ataf87ef4 in2786.262seconds,
with all528 frozen hashes unchanged. PR98 remains the only open owner PR, CLEAN at186266e
with every exact-head check successful; its previously requested merge/release
approval has no reply. No release or requirement completion is inferred.

Family role projection comparison (pre-edit): replace the existing coexisting
shutdown/metrics/disposal tuple with one owned FamilyRetirement<Id> product;
all three original values belong to that exact family and remain independently
inspectable. There is no new phase, callback or runtime authority. Borrowed
family(role) returns Result<&FamilyRetirement<Id>, &RecvError>; a closed
receiving channel never implies an empty or successfully retired family.
Use the existing Here/Inside inference and sealed head/tail selection law, with
a private FamilyAt<Role, Position, Id> port for two real positions. Id follows
the exact EntityShutdown identifier type. Existing native consumers use u64;
a separate String-keyed value witness must prove the meaningful substitution. Do not export a selector trait or require caller paths/type arguments.
The native producer will own named construction; temporary tuple packing in
the external witness is comparison evidence only. Public candidate products
become3, private selectors1, transitions/states/allocations unchanged. No
repository production edit precedes inferred head/tail, receiving refusal and
missing/duplicate-role denials.

Joined result native lowering (pre-edit): the named family projection passes
both real positions in two live same-protocol/same-ID families, retaining
distinct replies and each original actor. Native HTTP bind refusal returns
its original application/router/address for real retry; natural timer expiry
joins serving independently. Ten comparison cases pass both compilers/profiles
(40 executions); five denials pass on each compiler: missing/duplicate family,
lifecycle authority, moved custody and private-field access. The String-keyed
probe is a separate passive-data projection law, not a fabricated native birth.
Measured result sizes remain872/648/768/944 bytes. Protocol/actor state graphs
and receiving/destruction order stay unchanged; the raw families getter is
deleted. Independent receiving/disposal errors remain separately owned.

Now write the same native consumer without research tuple adapters before
production. Expected production owners: application/outcome.rs (whole acquired
ApplicationOutcome and owned ApplicationRetirement), application/work.rs
(existing six alternatives renamed WorkOutcome), existing execution/http
producers, entity/retirement.rs (FamilyRetirement and private role selector),
entity/family.rs (named publication), and curated module/crate exports. Expected
net production growth250–400 lines, three net public products, no new public
trait, marker, generic default, actor state, dependency or allocation. Preserve
existing bounds and port/callback contracts. The existing expanded cumulative
budget authorization applies; no mechanical catalogue migration precedes the
native owner witness and checkpoint. Current baselineall24 native gates pass
ataf87ef4 with528 hashes unchanged; no result API is yet claimed implemented.

Native joined-result evidence: tests/outcome.rs was written before production
and failed E0432/E0599 against the old public result. Its nine complete native
journeys now pass on Rust1.99 and1.96, debug and release (36 executions). The
actual native API passes ten compile denials across both compilers: absent or
duplicate family role, result lifecycle authority, moved ownership and private
custody construction. Strict focused Clippy passes; after documentation/lint
changes, current-debug repeats all nine successfully. Other final-source matrix
repeats and the complete catalogue/gates remain pending. The two-value completed
handoff remains a tuple; its retirement value already owns a named contract.
The owning work sum and private producer state retain six and five alternatives;
no actor transition authority, allocation, dependency or interpreter effect was
added. Ordinary result inspection no longer exposes family product positions.
Current lowering is uncommitted; do not infer catalogue compatibility, publication
or requirement completion from these focused results.

Joined-result lowering checkpoint before caller migration: production
+698/-483/net215; focused tests +909/-0; three net public
products and seventeen new public operations (the old phase sum is renamed,
not duplicated); affected production owner modules6→9. Source match arms
113→115, solely the complete owned selection; actor control states and
transition authorities unchanged. Work control alternatives6→6 and producer
custody alternatives5→5. Whole report owns acquired work/root/notifications/
families; selected retirement owns cleanup/root/notifications/families; family
report owns shutdown/metrics/the original native disposal payload. Each fact
is still needed for independent caller recovery; no arrival-history state,
duplicated cause, false cardinality, semantic flag or nested actor engine.
Private role selection has two real positions and distinct u64/String passive
products; ordinary syntax infers the position. Cross-checks remain Driver
execution conservation, native receiving/destruction order, initialization and
fixed-supervisor retirement laws. Disposition: pass this ownership/API batch,
with focused native/denial evidence above; complete catalogue and repository
gates are not implied.

Mechanical caller migration (pre-edit): rename authored work-phase matches to
WorkOutcome; native exhaustive tests peel the complete four-part report through
into_parts, while ordinary examples select completed value plus named retirement
and borrow its getters. Named family reports replace shutdown/metrics/disposal
patterns without losing any originals. This stage introduces no type, bound,
policy, callback, route or dependency. Existing expanded-budget authorization
applies to the full caller corpus. Remove example-only verification code into
existing owning test targets; do not hide result ceremony in example helpers.

Counter verification relocation transfers its existing futures-util0.3.34 dev
dependency from the example to the owning runtime tests. This dependency already
exists in the locked graph and supplies the same immediate-poll and catch-unwind
operations exercised by those tests; no runtime dependency or new crate is added.

Caller migration checkpoint: the current all-target/all-feature workspace check
passes. The first runtime batch stops at the topology denial snapshots: two
missing-lane E0277 diagnostics retain their law with changed owner-signature
formatting; swapped/duplicate-role and erased-cold-origin fixtures also acquire
an incidental old-tuple E0308. Before snapshot refresh, migrate only those
three fixtures' successful result peeling to the four-part report. Their
forbidden role/protocol/origin operations remain unchanged. Review the resulting
E0277/E0271/E0308 denials independently; do not accept the incidental tuple
mismatch as proof or count this failed batch as a passing runtime suite.

Joined-result migration verification: all-target/all-feature workspace check
and strict Clippy pass; seven native suites pass101 tests with one existing
ignored test. The canonical guide passes3 active doctests (3 schematic ignores).
Topology denial source changes touch only successful post-call peeling in three
fixtures; their E0271/E0308 witnesses remain identical. Only two E0277 snapshot
lines change their compiler truncation by one character. No incidental tuple
mismatch is accepted as a denial. Native inversions detect root-based work
selection, fabricated rejected notifications and hidden disposal failure in
both profiles; six restored controls pass and both owner source hashes match.

Whole-tree checkpoint including untracked source: production
+794/-1193/net-399; tests
+2511/-720/net1791; three net public products.
The apparent source deletion includes moving example verification into the
runtime test owner; counter main actually grows51→62 formatted lines. No
caller-line, heap or throughput reduction is claimed. Mechanical migration
adds no control state, transition authority, public product, bound, effect or
dependency identity beyond the previously proven result owners. Original
receiving errors, native panic payloads, metrics, actor values and notification
results remain required and retained. Arrival history, duplicate causes,
semantic flags and structural ordinary family selection remain absent.
Disposition: pass the measured native ownership/caller batch; ordinary examples
beyond counter still need oracle removal and canonical named-result consumption.
Final-source MSRV/archive/full24 native gates and coherent commits remain open.

Pool example distillation (pre-edit): retain the existing complete business,
role and retirement oracle as an owning fifo_pool_runtime test, reusing its
actual SearchWorker source and inferred pool topology. The ordinary example
will execute typed submission/reply/shutdown through Application and retain
one joined result. Cleanup is performed by execution/receiving; report
inspection does not perform it. Tokio's existing current-thread main macro
owns the caller host. This is caller/documentation policy, with no behavior
state, effect, public type, dependency identity, interpreter bound or runtime
policy change. Expected files: pool main/Cargo, existing runtime test and
coordination; net example code decreases, complete oracles move intact. The
expanded cumulative budget remains authorized. A moved actual-source oracle
passes before the ordinary entry is changed.

Direct retirement observation (pre-edit): the real pool caller fails E0599 at
application.retirement(). The user-selected single Application entry should
delegate the existing root lifecycle report, as request_shutdown/termination
already do. This is a derived interface composition over the unchanged native
report producer, not a new actor-model law or cleanup operation. Desired syntax
borrows application.retirement() while work remains active; cancelling one wait
cannot cancel another, and a waker fault retains the committed report plus the
original independent notification failure. Reuse ApplicationLifecycle::retirement
and its exact observation value. No new data owner, state, event/effect, generic
bound, trait, public type, dependency or lifecycle authority; add one method
with the same future capture pattern as termination. Change the existing two
focused native report regressions first and record their RED before production.
Aggregate before/after states, report alternatives and actor branches stay equal;
each report's two current assessments remain independently meaningful. The
expanded budget applies; broad callers remain unchanged until this witness passes.

Counter entry distillation (pre-edit): run_with already awaits its cleanup
producer and independent receipts after caller work. The actor receives the
explicit shutdown request; the caller need not also await termination before
returning its business value. Remove that redundant wait and imperative cleanup/
notification success checks from the basic entry. Retain the complete result
and show optional failure-bearing report inspection; no cleanup or original
error is performed, reconstructed or hidden by the example. The five relocated
counter laws and nine native complete-result cases remain its independent
verification. Existing current-thread Tokio main owns the host, with its macro
feature declared directly. No new type, bound, dependency identity or runtime
policy; actor state and action order remain unchanged.

Direct retirement delegate checkpoint: focused RED is four E0599 sites in
two native laws before production. Both laws pass Rust1.99/1.96 in debug/release
(8 executions), with the measured owner source hash unchanged. Production adds
10 lines and1 inherent operation; public types/traits/bounds/states/branches
remain unchanged. There is no second report producer: lifecycle owns the
shared fact and the application delegates it. Cancellation, pre-work-completion
publication, original actor/space custody and observer-panic notification
retention remain exercised. No arrival history, repeated cause, cardinality
assumption or nested authority. Cross-checks are native report conservation
and execution/receiving work barriers. Disposition: pass the delegate; full
repository and package gates remain required. Ordinary pool/counter entries
are now candidates using named results and automatic joining; their complete
source-backed runtime/executable matrices run next.

Counter/pool retained checkpoint: public direct-report witness passes8 native
executions across Rust1.99/1.96 debug/release. The ordinary counter and pool
execute eight binaries in those same configurations; expected business values
are1 and3. The three owning suites pass17 tests per configuration (68 total):
original processing/rejection/cancellation custody and the full moved pool
retirement oracle remain. Strict workspace all-target/all-feature Clippy and
three active guide doctests pass again; three schematic doctests stay ignored.
Counter main51→36 and source569→45; pool source191→90. Both have
zero assertions and explicit drops. The runtime owns cleanup; their report
printing is optional inspection, not a cleanup step. The pool retains its
actual source worker and all former retirement assertions in the runtime test.

This caller batch adds0 public types, traits, bounds or actor states and one
previously proven report delegate; no dependency identity changes. Tokio's
existing macro feature is explicitly declared by both examples. No boolean
state, duplicate cause, nested transition authority, new alias or structural
retirement pattern is introduced to ordinary code. Report fields still own
all independent errors; no lossless-selection law changes. Disposition: pass
these two source-backed examples. Supervisor, heterogeneous children, HTTP,
Entity and other templates still need equivalent distillation, and final
package/full Nix verification plus commits remain required.

Supervision example distillation (pre-edit): move the existing complete
replacement/tree-retirement oracle and pure initialization/correlation law
to one owning runtime supervision test. Verify the original witness before
changing the entry. Then share the actual worker command, actor and startup
plan through supervision/worker.rs, preserving their bodies; pub(super) exposes
only WorkerRole, WorkerCommand, Worker, its submitted startup product and the
existing worker_submission constructor required by the real parent factory.
Rename the private ActivationNotice to Startup in that owning module, matching
the user-selected vocabulary; the selected0.26 ActivationPlan trait stays
unchanged. No new domain type or wrapper is created. Parent topology stays
explicit once; the ordinary async current-thread main queries capabilities,
stops the first worker, awaits replacement, requests shutdown and retains one
ApplicationOutcome. Existing RootReport and optional retirement inspection
replace assertions and recursive enums in application code. No public crate
symbol, actor state, event/effect, generic bound, dependency identity, policy
or interpreter change. One worker owner module is added; original test oracles
move rather than disappear. Reuse existing published constructors and WorkerFactory;
the named policy release remains pending its specific approval.

Heterogeneous document example checkpoint (pre-edit): copy its existing runtime
and pure initialization/correlation oracles into an owning documents test
first, with no changed expectations. Verify the actual nonce and retirement
contract before distillation; source outranks legacy constants. Share domain
actors with the eventual application rather than retaining copied actor bodies.
The ordinary entry must construct both children, use phased shutdown and keep
the complete inferred tree without consuming recursive enums in main. This
is caller/module ownership policy, with no new core/runtime type, bound, effect
or allocation policy. Any discovered failing original oracle must be resolved
from executable allocation/creation evidence before moving the application.

The unchanged document oracle passes both runtime retirement and pure
initialization tests. Keep creation correlation IDs and creator-local child
nonces distinct; the observed expectations require no correction. Put the
three domain actors, their two concrete message protocols and one topology in
a private documents module. Its parent-owned contracts use pub(super) for
DocumentSystem, Indexer, Journal and their commands; actor state fields stay
private. These are the real authored child protocols and retained actor states
of the declared graph, available to deliberate deeper inspection. The parent
main names only DocumentSystem, its child roles and the one topology. Runtime
oracles include that exact source in their own document concern, keeping private
state inspection inside its owning scope. No test-only accessor or extra
constructor is added. Actor bodies, effect products and initialization ordering
stay byte-equivalent apart from ownership visibility. No new semantic type,
state, public crate spelling, bound or dependency. One coherent domain module
is added; initializer and all retired-tree assertions remain in runtime tests.

Supervisor/document distillation checkpoint: original native/pure oracles pass
before entry changes. The shared-source versions then pass Rust1.99/1.96
debug/release:16 oracle executions and8 real binaries, including replacement
and phased heterogeneous shutdown. Strict workspace all-target/all-feature
Clippy passes. All94 original assertion bodies remain identical and
in order in runtime tests; worker state is not duplicated and document private
state is inspected in its owning scope. Main has no assertion ledger, explicit
drops or recursive retirement enum matches.

Supervisor source332→99 for its entry plus61 in worker.rs; document source
383→34 plus95 in documents.rs. Those domain modules relocate actors/commands/
startup/topology rather than removing behavior. Existing Startup replaces the
private activation name; selected0.26 trait names stay authoritative. The
topology uses its already-supported pub(super) declaration for the real parent
consumer, with no broader module exposure. Public crate types/traits/bounds,
actor state sums, effect/transition branches and dependency identities remain
unchanged. Actor/current domain values remain the same; no arrival history,
semantic flags, duplicated causes or nested actor engine. Cross-checks remain
initialization ordering, fresh worker provenance, native report conservation
and complete custody. Disposition: pass these two source-backed entries;
HTTP/Entity/remaining templates, final package/full native gates and commits
still remain required.

Machine/timer example distillation (pre-edit): retain its two original native
journeys and complete retirement oracle as runtime processor tests against
the actual processor source. Prove them before changing the entry. The ordinary
current-thread entry runs a genuine receive-timeout policy after deferred work
is replayed, then an explicit shutdown case through the same Application path.
Use the existing joined report and lossless result selection; remove example
assertions, test deadlines, explicit drops and exhaustive terminal matches.
No policy values, Machine transition, cloning contract, timer identity, event/
effect, actor state, bound, public type or dependency identity changes. The
published Machine Clone constraint remains an open owner requirement, not a
claimed fix. Existing Tokio macro feature is declared directly; all original
assertions stay with their real native sources. Rename the old local boundary
variable to sender without changing its protocol or origin.

Machine/timer entry retained: both original runtime journeys passed before
editing the example. All 21 assertion expressions remain identical and ordered
in runtime processor tests against the actual domain source. The entry shrank
from 188 HEAD lines to 56; it contains no assertion, explicit drop, test deadline
or terminal enum match. Both native tests and the real binary pass on current
Rust 1.99 and MSRV 1.96 in debug/release: eight test executions and four binary
runs. Strict workspace/all-target/all-feature Clippy passes. Deferred work
still replays before the receive timeout stops the actor; the second application
requests shutdown and awaits its existing root report. Cleanup is already
joined when each outcome is returned; printing its result is optional inspection.

Aggregate checkpoint: Processor states Closed/Open, message alternatives
Work/Open, Machine transition branches and timeout/shutdown decisions are
unchanged; production domain modules, public spellings, bounds and registry
dependency identities are unchanged. Only the existing Tokio macro feature
is made explicit in the example manifest. The surviving report owns the
committed root report; retirement owns independent cleanup, native custody
and notification results. No arrival history, repeated cause, semantic flag,
false cardinality or subordinate transition authority was added. Cross-checks:
deferred-message replay, explicit timer identity, shutdown composition and
complete native retirement custody. Disposition: pass this entry; Machine
Clone/retention constraints, HTTP/Entity entries and final full gates remain
open.

HTTP application entry (pre-edit): the current example requires callers to
split one ApplicationHandle into an ActorInterface and ApplicationLifecycle,
then combines them again in LiveOrderCommands. A generic OrderCommands port
has one production implementation; ApiState only wraps that port. This is a
derived capability-composition change, not a new actor-model or HTTP policy.
Desired caller: `run_axum(address, http::router)`, where router receives the
actual application handle. Each request still allocates its own sender from
that application's allocator and admits the same typed OrderMessage; a closed
actor returns the complete original PlaceOrder, including its String allocation.
HTTP success echoes the submitted order; HTTP rejection returns the submitted
order with SERVICE_UNAVAILABLE. Shutdown uses the same root's existing lane.

Before replacing production syntax, move all four HTTP tests and the one pure
order-book test into the owning runtime test target, preserving their original
assertions, native root oracle and source-backed domain. Run that baseline.
Then write a failing caller regression that passes ApplicationHandle directly
to router and exercises actual HTTP admission, closed-root rejection, shutdown
and joined retirement; retain the original allocation witness. Reuse existing
ApplicationHandle sender/request_shutdown, ActorExt, Axum State, Communication
exact rejection, and run_axum serving/cleanup paths. Delete the example's
OrderCommands, LiveOrderCommands and ApiState; add no runtime type, trait,
bound, wrapper or dependency identity. Test mocks must be replaced by actual
actor/router journeys, preserving their independent success/rejection claims.

Expected files: example main/http/order_book/Cargo, runtime tests/orders.rs and
dev-dependency edges plus the lockfile and guide/coordination records. Expected
production delta negative, public crate types +0/-0; example-only trait and two
structs removed. Selected contracts remain registry Core/Actors0.26.0,
macros0.14.2, Communication0.1.5, Address0.3.0, Timers0.2.0, Tokio1.53.1 and
Axum0.8.9; no patches. Inspected the actual ApplicationHandle sender allocation,
shutdown delegation, typed endpoint rejection and HTTP serving path, plus the
selected StopOnShutdown and Communication contracts. Existing expanded change
budget applies; final full gates and catalogue migration remain separate.

HTTP application entry retained: all five original source-backed tests passed
after relocation and before replacing the API. Desired direct router syntax
then failed E0061; the original-allocation caller failed E0425 before its direct
application admission function existed. The actual application handle now
supplies Axum State. No runtime/library API changes: the example uses existing
sender allocation and shutdown delegation. Both former mock tests now execute
the actual router/root: the success case checks the retained order-book map,
and closed-root rejection checks the full returned HTTP order plus refused
shutdown. The original closed-admission String-allocation witness, live TCP
journey, pure transition and full native retirement oracle remain.

Five tests pass current/MSRV debug/release (20 executions). The actual example
binary also passes all four configurations with POST order and DELETE shutdown
returning202, followed by joined cleanup Ok. Strict workspace/all-target/
all-feature Clippy passes. Three native defect inversions—missing admission,
copied rejection and false shutdown acceptance—fail their intended assertions
in debug/release; six restored controls pass with exact source-hash restoration.
The binary harness initially rejected a completed listener's TIME_WAIT port;
no live listener remained. Its availability probe now uses socket reuse,
matching normal listener restart, and all four real runs pass. No test failure
or other process was waived or killed.

Aggregate checkpoint: OrderBook retains one BTreeMap and one Place command;
the action and admission/rejection equations, shutdown event, typed interpreter
and cleanup ordering are unchanged. Production example structs2→0 and traits
1→0; its three source files total584→110 lines, with tests relocated rather
than erased. The sole private alias names the actual order application used by
router and requests; it carries no new state, capability or generic parameter.
No semantic flag, arrival history, duplicate cause, cardinality assumption,
nested transition authority or structural caller syntax remains. Cross-checks:
exact communication rejection, explicit shutdown policy, pure actor state,
HTTP response policy and original joined custody. All165 full-workspace registry
identities/checksums are unchanged. Complete tracked/untracked Rust delta at
this checkpoint: production +1185/-2743/net-1558; tests +3840/-720/net3120;
public crate types +3/-0 for the previously proven joined-result batch, +0/-0
for this HTTP change. These figures include relocated example oracles and are
not a claim of equivalent code elimination. Disposition: pass this HTTP entry;
Entity, final full/package gates, documentation reconciliation and commits
remain open.

HTTP state cost: source-backed comparison compiles the original
LiveOrderCommands fields and the current OrderApplication against identical
selected registry dependencies. On Rust1.99/1.96 debug/release their stored
values measure104→64 bytes, removing40 bytes of repeated capability storage.
This is the size of the concrete order-router state, not total application
heap, transport buffers, request throughput or a general ApplicationHandle
layout guarantee. No runtime allocation/latency improvement is claimed.

Joined-result delivery checkpoint: Bombay feature commits f0020c2 (joined
results, direct root-report observation and source-backed application entries)
and0011620 (direct HTTP application state) preserve the staged semantic changes
separately. The full optimized workspace suite passes720 tests, one existing
skip, in840.301seconds including compilation and65.448seconds test runtime.
All539 captured tracked/untracked files remained unchanged during verification;
the final committed Rust/manifests match that capture. Only coordination prose
was reconciled afterward. Strict workspace/all-target/all-feature lint,
formatting and three active doctests pass; three schematic doctests stay ignored.
The Bombay worktree is clean. These are local feature commits, not publication
or final programme acceptance. Entity, final package/full native gates and all
other open checklist dispositions remain required.

Entity entry baseline (pre-edit): preserve the original complete account
journey as a runtime test before changing production example syntax. Its
observable law is existing project policy: admit40, fence/passivate the exact
incarnation, await its actual retirement, admit2 through the same stable
reference, then shut down root before family and retain both original account
states40/2 plus independent root/family/notification outcomes. The example's
test counters, semaphore and exhaustive custody oracle must live with that
test. This stage adds only the focused baseline, with no production shape or
policy change. Actual EntityDefinition still requires four consuming lifecycle
callbacks, and passivate_entity returns the immediate Passivation decision;
it is not a retirement future. Inspected actual admission, begin_drain,
retire_owned, native lease join, callback ordering and family result paths.
Do not disguise an immediate passivation request as completed cleanup or add
a no-op definition to shorten the example. The subsequent design must reuse
these owners or isolate a concrete missing consumer contract first.

Entity retirement completion candidate (pre-edit): current passivate_entity
returns only an immediate decision and forces the real caller to build a
callback/semaphore completion path. Desired ordinary syntax is
`application.retire_entity(AccountsRole, &id).await`; retain passivate_entity
for an explicit request-only operation. This is derived API composition over
existing Entity fencing/retirement policy, not a new actor-model operation.
The return value is the existing RetirementMode, proving that the captured
incarnation task ended and its slot was released, including a truthful forced
mode. It does not claim the native result or callbacks were error-free: their
complete original failures stay in the existing family outcome. No actor is
deleted from its stable domain namespace; later admission still creates fresh.

Use one optional generation-bound completion publication per existing Slot,
shared by independent observers. The existing Slot state remains the sole
lifecycle authority. Register under its existing synchronization, capture the
actual ActivationId, and publish only after retire_owned records the original
failure, commits Terminated and releases the slot. Dropping a caller wait neither cancels the directory-owned task nor consumes
another observer. Never wait for unrelated family tasks or infer completion
from termination, a counter or the user callback. Reuse existing Observe pair,
Publisher and Observation for the shared RetirementMode fact, exactly as root
and installed-child retirement do. Incomplete publication remains pending;
cancellation of the execution host cannot manufacture a completion. The
independent application/family result retains original task failures. The
paired execution/receiving path remains available for cancellation and recovery.
No extra dependency, trait, phase sum, port or public error is required.
Request refusal uses existing Passivation; EntityRetirementFailure still owns
native cleanup causes. A proposed closed-channel guard was rejected before
production because its closure is not proof the actor lease joined, and it
adds a second cancellation vocabulary instead of reusing current observation.
Test that missing completion stays pending while original family failure remains
available after cancelling the caller wait and shutting down separately.

Focused RED precedes production: the original native account journey awaits
the new operation without its semaphore. Follow with two independent observers,
cancelled waiting, an unrelated live account, immediate request-only retirement,
fresh reactivation, forced mode and callback failure retained independently in
the complete family result. Reject early publication by delaying the actual
retirement callback and checking that completion remains pending. Actor phases
remain Inactive/Activating/Active/Draining/Retiring; one current optional
publication owns an actual outstanding observation, not arrival history.
Expected owners: application interface, Entity family/runtime/directory and
curated exports; one optional publication product and no new public type.
No NativeEntityHost, EntityDefinition, LocalEntityRuntime, actor effect or
generic bound changes. Expected production growth below200 lines. Measure
slot/future memory and complete deltas; the existing expanded scope applies.

Retirement refusal sum correction (pre-edit): the candidate's Result error
used Passivation, which also contains successful Begun/AlreadyPassivating
decisions. Those are not possible refusal states for the awaited operation;
independent observers join an existing retirement. Require callers to handle
exactly NotActive, Superseded and ShuttingDown exhaustively. First add an
exhaustive native rejection match against the current candidate: it must fail
E0004 for the two extra positive decisions. Replace only this operation's
carrier with EntityRetirementError, a narrow three-case Error sum; leave the
existing request-only Passivation contract unchanged. This type owns an
independent early rejection contract, while EntityRetirementFailure retains
later native/callback failure. No new transition, state, bound or dependency.
This reopens the earlier no-new-public-type estimate: expected +1 Error type,
still covered by the recorded expanded programme scope.

Aggregate checkpoint correction before retaining the candidate: actual
directory code states a mapped Slot never acquires a second incarnation;
removal authority is its captured Arc identity. Copying ActivationId into the
observation product repeats the Slot's current fact, and the mismatch branch
has no lawful producer. Delete that duplicate identifier and its Superseded
alternative from this new operation. The existing locked state supplies the
exact ID for BeginDrain, and retire_owned captures the same original Slot
through join/removal/publication. The optional product owns only publication
authority and the shared observation. EntityRetirementError therefore has
exactly NotActive/ShuttingDown; the request-only Passivation still owns its
existing broader decision vocabulary. Measure this smaller representation and
prove fresh reactivation uses a distinct observation, not the previous fact.

Entity retirement candidate retained at the owner seam: the original native
journey fails E0599 before production and passes without its semaphore wait.
The exhaustive rejection witness fails E0004 before narrowing the carrier.
Five account tests plus one native-port interruption test pass Rust1.99/1.96
debug/release (24 test executions): callback barrier, independent/cancelled
observers, unrelated live entity, fresh reactivation, forced fence reason,
original callback panic and missing publication. Six existing/static denials
plus the ordinary error consumer pass all four configurations; message-only
interfaces cannot retire entities and a success cannot be a retirement error.
Three production defects fail in debug/release: premature completion, forged
graceful mode and erased callback failure. Six restored controls pass; source
restoration is checked by SHA. Strict all-target/all-feature owner lint and
formatting pass. Three active doctests pass, three existing schematic ignores.

Aggregate checkpoint: Entity control states5→5 (Inactive/Activating/Active/
Draining/Retiring), Slot readiness2→2 and dispatch2→2; family task/shutdown
state sums and actor transition authority are unchanged. The new Error sum
has two actual refusal alternatives; RetirementMode and all existing native/
family result alternatives are unchanged. The one optional product owns the
unique publication and shared observation only while a real completion is
awaited; its Slot identity already supplies the incarnation binding. No copied
ID, arrival history, repeated cause, semantic flag, false cardinality or nested
transition authority remains. Original actor and callback failures coexist
with a completed mode in their existing owners; incomplete publication cannot
invent either slot release or successful family shutdown. The unchanged pure
lifecycle/fencing, captured-slot removal, actor-subtree assessment and typed
family-retention laws are cross-checked. Production owner slice3687→3858 lines,
match arms79→83, modules9→9; public spelling adds one method and one Error type
with two variants. Complete delta at the measured point: production+199/-28/
net171, tests+1110/-0 (including the original example oracle moved into runtime).
This is added capability, not a claim of production reduction.

Memory: source-backed Observe comparison with identical registry identities
and Triomphe features passes both compilers/profiles. The optional publication
field occupies16 bytes; creating an observation pair allocates one72-byte
block, released after its owners leave scope. This measures the field/pair,
not complete process heap or end-to-end throughput. Actor creation, task lease,
generic bounds, effect lanes, initialization and dependency identities are
unchanged. Disposition: pass the bounded retirement seam; the example's
callback/associated-type surface and final package/full native gates remain
open, and broad caller migration has not begun.

Retained-seam follow-through: the affected Entity owner corpus passes23 tests
and its two existing downstream suites pass2+11 tests in debug/release, including
independent lifecycle traces, native wrapper/host retirement, exact endpoints,
passivation replay and forced fencing. The newly retained native interruption
case uses the real native port, original non-cloned panic allocation and
directory-owned task join; no alternate lifecycle engine is tested. Existing
full workspace720/24-Nix results retain their earlier revision limits. Only
the completed owned1.8GiB integration target was freed after confirming no
live command referenced it; source, evidence and other agents' caches remain.

Entity retirement owner seam is committed in Bombay36af335; the ordinary example has not yet migrated. No remaining requirement is closed by that commit.

Entity example distillation — DX22/26/28

Law (derived custody; application policy): ordinary Entity work uses the proven
`retire_entity` operation. Bombay owns completion ordering; the family definition
retains every original startup/admission/drain/native-retirement callback value.
An awaitable retirement mode never substitutes for independent native failure.
Pre-edit scope was main/account/Cargo/runtime tests/documentation, with no library
state, bound, dependency or public crate type. The family policy relocation to
accounts.rs was separately recorded before editing; it owns hydration, callback
custody and report formatting, not another lifecycle authority.

Provenance: the first report draft was reopened because its focused RED was
missing. Restoring the original main makes the unchanged proposed caller fail
E0560 (Accounts.reports absent) and E0433 (AccountReport absent). Reapplying the
candidate passes the same complete startup/admission/drain custody witness on
Rust1.99/1.96 debug/release. That witness now lives in the runtime accounts test
and includes the actual accounts.rs source. A second source-backed native trace
proves the actual policy retains both40/2 incarnation states, distinct attempts,
original reports and family settlement. The five original native oracles remain.
All seven tests and the real example pass all four compiler/profile builds;
source hashes stay frozen during that run. Strict workspace/all-target/all-feature
Clippy and formatting pass on the final source before commit. Evidence: ENTITY-REPORTS-RED.log,
ENTITY-REPORTS-VERIFIED.json and ENTITY-FINAL-VERIFIED.json in the retained results
artifact directory; earlier ENTITY-ENTRY results are superseded for this example.

Aggregate checkpoint: Root and Account retain the same pure control model and
existing StopOnShutdown/lifecycle owners; no library state or transition changes.
The previous retirement tuple retains native success/error only and three
callback ports discard inputs into a counter. One AccountReport now owns four
distinct callback equations: Startup owns id/attempt/original allocation or launch
failure; Admission owns id/original command and refusal; Drain owns id/attempt/
forced reason; Retirement owns id/attempt/original native result and assessment.
None dispatches commands or advances the family. Every retained value supplies
inspection or exact recovery; no duplicated cause or arrival-history label.

Example physical source lines401→297 (main95, account26, accounts176); main
body102→64. Source modules1→3, declared types6→7 (one report sum), methods9→8,
lexical matches2→4 and arms5→10, because reporting renders all retained callback
alternatives. Public crate spellings0→0; two family values have only parent
visibility for real assembly/inspection. Assertions58→0 in all example sources;
original oracles remain in runtime tests. No semantic booleans, nested transition
authority, explicit drop/barrier/semaphore, duplicated topology, structural alias
or recursive terminal enum remains in ordinary main. Shared actor and policy
source tests need no test getters. Cross-check: creation/lifecycle/initialization
laws, execution-ownership and DX04/22. Disposition: pass for this bounded example;
callback/associated-type ceremony, broader programme costs and final gates remain
open. Relocating the explicit family policy does not claim to remove it.

Client mailbox capacity — DX06, pre-edit

Law: deliberate bounded-resource policy plus derived affine admission custody.
Communication0.1.5 rounds a minimum up to a power of two with floor2. Validate
positive input and checked rounding before allocating any endpoint; store only
the effective capacity. Reuse Config::new, the existing external mailbox,
TrySendError, admission closure and accepted-prefix receiving. No actor algebra,
queue algorithm, aging policy, runtime phase or scheduling change.

Desired caller: Application::new(root).client_capacity(MailboxCapacity::new(3)?)
and ApplicationScope::new().client_capacity(capacity).external::<Replies>().
Scope settings configure future endpoints; existing endpoints retain their own
allocation. An exported ActorInterface inherits the application's client limit;
external(), client() and client_to() use that same limit. One-way senders retain
their current uninhabited reply lane. Constructor rejection returns the original
minimum; saturation/closure returns the exact original message, and closing
admission preserves the accepted prefix and originating address.

Smallest complete witness: an early endpoint requests3, admits exactly4 move-only
payloads, rejects the fifth by pointer identity, closes and drains all accepted
messages. Consume that scope into a real current-thread application; a later
endpoint must inherit4, saturate and still permit root shutdown and joined
retirement. A separate direct application witness verifies exported-interface
and bound-client propagation, fresh origins and rejected-message retry. Invalid0
and overflow return the original minimum. Run the focused native regression RED
before production. Then current/MSRV debug/release, independent saturation/default
inversions and existing scope/client/denial consumers.

Expected owners: application/mailbox.rs (validated MailboxCapacity and its error),
mod.rs, scope.rs, interface.rs, execution.rs/http.rs scope construction, crate
exports, focused tests and documentation. Reuse the existing optional scope as
configuration custody through untouched/prepared/staging failure; add no config
wrapper, generic parameter, trait, lifecycle state or capability. Default remains
bounded1024. Expected production growth below160 lines and two public value/error
types. Root/birth/Entity limits, complete resource accounting and final DX06 gates
remain required; this focused stage does not close DX06. Expanded cumulative
programme authorization remains the recorded2026-10-09 approval.

Client capacity retained checkpoint: the pre-edit native consumer fails E0432
(MailboxCapacity absent) and E0599 (both owner setters absent). Its protocol import
was corrected from Behavior's address to the actual native MailAddr; no production
bound or capability was added in response. All four focused laws now pass on
Rust1.99/1.96 debug/release, beside the existing actor-interface/client/scope corpus
(24 tests per build total) and five active rustdoc tests (three prior schematic
snippets remain ignored). Six existing authority/receiving denials and the client
protocol denials remain; new rustdoc denials prevent forged capacity and raw
integer selection. Three independent inversions (ignored limit, wrong rounding
floor, accepted zero) fail in debug/release, and all six restored controls pass
with exact original source hashes. Two actual timer/shutdown wrapper orders use
the selected small limit in their existing native consumer. Workspace all-target/
all-feature strict Clippy and formatting pass; both configured wrapper orders
pass the final current/MSRV debug/release matrix before commit. No dependency or Rust version changed.

Checkpoint: existing application work/control phases and subordinate alternatives
are unchanged; endpoint admission/closure and notification/retirement remain the
same owners. ApplicationScope retains the selected scalar through missing-host,
untouched, prepared and staging custody; ApplicationHandle and ActorInterface
need their immutable copies to construct future reply endpoints. No new authority,
state graph, historical label, correlated flags, runtime lookup or structural
caller path. The new validated scalar owns positivity/checked effective rounding;
its error owns the original invalid minimum. Six affected application modules:
physical lines3938→4060, lexical matches52→52/arms102→102, enums5→5, declared
types16→18, public methods52→56; modules5→6. Two public value/error types and two
owner setters replace the private external1024 constant. Existing private owner
fixtures supply the unchanged bounded default after the focused native proof;
there is no new public mandatory argument or placeholder policy. Cross-check:
DX06/20, Communication0.1.5 channel construction and native admission/retirement
laws. Disposition: pass for client settings; root/birth/Entity resource scope and
complete resource accounting remain open.

Actual allocation probe (not an example helper): equal two-message external
endpoint construction, admission, closure, prefix draining and disposal executes
on both compilers/profiles. Capacity1024→2 changes peak tracked heap35442→2738
bytes and total35570→2866, with equal25 allocation blocks and56 end bytes.
Scope and empty exported interface each measure16 bytes; the validated scalar
adds one usize of stored configuration to each existing owner. Results agree in
all four builds. No per-actor, throughput, RSS, leak-free or cross-framework claim.
Evidence artifacts: CLIENT-CAPACITY-RED/GREEN/CLIPPY.log,
CLIENT-CAPACITY-VERIFIED.json (eleven frozen source hashes),
CLIENT-INVERSES-VERIFIED.json, CLIENT-WRAPPERS-VERIFIED.json and
CLIENT-HEAP-VERIFIED.json in the retained results directory.

Full gate consumer correction (pre-edit): the frozen548-file c2c2abb Nix run fails
with four E0308 errors in the isolated transport counter's two old tuple borrows/
consumptions, plus E0765 from a malformed earlier mechanical migration line in
protected_counter/retirement.rs. The native gate did not pass; no disk/signing
waiver applies. Reuse the proven ApplicationOutcome::into_parts product and
WorkOutcome variants. Each existing fixture retains work, native result, both
notifications and the empty family product, plus independent transport-session
closure. Correct only those consumers and malformed syntax; preserve every
original custody/assertion/held-child oracle. Expected files: the two source
fixtures and owning/coordination evidence. No production library, abstraction,
policy, bound, protocol, transport feature or dependency change. Expected fixture
net delta below15 lines, public types0. RED is the exact captured native gate
failure. Prove the isolated protected-counter debug/release/fmt/lint gate and
rerun all24 native Nix gates on frozen final source. The additional mailbox
worktree remains clean at c2c2abb while this existing consumer blocker is fixed.

The isolated target exposed ten further E0308 errors once the parser defect was
removed: five native receiving acquisitions still passed ApplicationOutcome to
already-migrated complete four-part patterns. Add into_parts at those actual
acquisitions, not a new conversion/helper or changed oracle. The required isolated
formatter also normalizes old tuple formatting in mod.rs/transport.rs; these are
fixture-only changes. All original assertion counts remain540/149/53/87 across
the four affected fixture sources. No production surface or semantic change.

Isolated consumer checkpoint retained: protected_counter passes53 native cases
in debug and release; strict isolated Clippy and formatting pass. The two tests
responsible for the prior55 count were relocated from example Counter source;
both remain in the owning five-test counter target, which passes again. The four
fixture files retain all829 assertions (540/149/53/87), with identical normalized
bodies except two transport borrow patterns that now include the actual empty
family product. Evidence: CLIENT-PROTECTED-{DEBUG,RELEASE,CLIPPY,FMT}.log,
CLIENT-PROTECTED-ORACLES.json and CLIENT-COUNTER-ORACLES.log.

Drift/provenance: library production+0/-0/net0, fixture+124/-47/net77 (including
formatter expansion); public API+0/-0 types; four fixture modules remain four.
All aggregate/control states, subordinate alternatives, transition authority,
actor folds, policy choices, typed effect lanes and allocations remain unchanged.
Only existing result consumption and malformed syntax change. No bounds,
placeholder, semantic boolean, arrival-history label, structural user interface
or new abstraction. Original Work/native/notification/session-close custody
remains independently owned. Cross-check: DX04/25/27 and execution-ownership,
plus the existing protected-counter held-child/fault/native oracles. Disposition:
pass for the migration correction; all24 native Nix gates must still rerun on
frozen final source. The c2c2abb full run remains failed at281.057 seconds with
548 unchanged source hashes; cancelled checks are not passing evidence.

Optional HTTP consumer checkpoint: frozen c8f871c full Nix passes ten native
gates, fails remote-debug on unguarded optional Axum in outcome.rs, and cancels
thirteen remaining checks. All548 source hashes remained unchanged; runtime
861.140 seconds. No disk/signing waiver. The pre-edit coordination record
selects only existing axum feature conditions on four exclusive imports and
two HTTP tests. Non-HTTP outcome laws remain available without HTTP.

Seven no-default-feature outcome tests pass; all nine axum outcome tests pass
debug/release; remote-only all-target strict Clippy and workspace fmt pass
through pinned Nix. All131 assertions and all other source remain byte-identical
when the six conditions are removed. Production+0/-0/net0, tests+6/-0/net6,
public types0. Aggregate/control and subordinate states, branches, modules,
protocols and ownership equations are unchanged. No new bounds, placeholders,
structural user syntax, semantic booleans or duplicated causes. Cross-check:
DX04/25/27 and existing optional HTTP/outcome custody contracts. Disposition:
pass for test feature isolation; all24 final-source gates remain required.
Artifacts: CLIENT-FULL-NIX-c8f871c.log/json, OUTCOME-FEATURE-VERIFIED.json
and OUTCOME-FEATURE-ORACLES.json in the retained results directory.

Actor mailbox scope, pre-edit DX06: deliberate Bombay configuration policy.
Application::new(root).mailbox_capacity(MailboxCapacity::new(minimum)?)
and ApplicationScope::new().mailbox_capacity(validated) select one immutable
actor limit for root, fresh births/later generations, cold members and Entity
roots/descendants; existing client_capacity remains independent. Reuse current
Communication0.1.5 Config, ApplicationScope custody, native startup/creation,
ApplicationCapabilityInputs and Entity family installation. Delete the three
private actor-size constants after all paths are proven. Expected production
delta below180 lines, public types0, two existing owner setters; no builder,
wrapper, trait, generic, capability or bound. Control-state sums and subordinate
alternatives must remain unchanged; immutable configuration is retained only
where future mailbox construction requires it.

Separate codex/devx-mailboxes worktree now contains two focused root regressions:
minimum3 admits four original move-only documents; fifth Full returns identical
Box, retry follows the accepted prefix, actual count reply proves all five were
processed, and joined retirement preserves their exact allocations/order. Early
scope selection separately saturates two slots and proves shutdown remains
admissible. Use existing ExternalActor::try_send and exact root recipient;
ApplicationClient currently has no nonblocking method, and this stage adds none.
Pinned native RED yields only two E0599 errors for the missing setters.
Production remains unchanged. Root-only evidence does not close the stage: fresh
birth/Entity failure and retirement witnesses, wrapper orders, provenance and
aggregate measurements are still required before retaining an implementation.
Cross-check: DX06/20 and Communication admission plus native retirement contracts.
Disposition: design in progress. Expanded authorization remains2026-10-09.

Full verification resource evidence: frozen9c7e37a Nix run failed after113.984
seconds on an actual LLVM no-space error in isolated protected_counter;548 hashes
unchanged. Preserve CLIENT-FULL-NIX-9c7e37a-disk.log/json. Confirmed the owned
extension Cargo cache had no active builds, removed its disposable4.3GiB while
preserving all evidence, and restarted the identical full command/head with
5.0GiB available. No resource failure is accepted as a passing gate.

Actor capacity retained as f54f644 on codex/devx-mailboxes. Production+94/-16/net78;
public types0, existing owner setters+2. Tests+804/-0/net804. Immutable validated
configuration is retained only at future mailbox constructors: Scope, capability
inputs/owner, PendingChild and Entity runtime. Application work phases remain
Unstarted/StagingRejected/Prepared/NotInvoked/Completed; work presence remains
Supplied/Absent. All native creation and Entity sums and transition authority
remain unchanged. Owner-file matches148→148, alternatives300→300, enums22→22,
structs69→69 and modules24→24. No semantic flags, arrival history, duplicate
causes, extra generic/bound or structural caller syntax. Cross-check DX06/20,
Communication admission and native retirement contracts. Disposition: pass for
actor-limit propagation; cold canonical integration and complete resources open.

Frozen550-file final matrix: five actor consumers, one native Entity case,
eight client/wrapper cases and four client-capacity cases pass on Rust1.99/1.96
debug/release,72 executions. Strict workspace all-target/all-feature lint/fmt
pass. Rustdoc: six active tests pass, three existing ignored; raw actor/client
integers and forged validated capacity remain denied. Four ignored-limit defects
(root, born child, native Entity, family propagation) each fail the intended
saturation law in debug/release; four restored controls pass, source hashes
restored. Evidence: ACTOR-CAPACITY-VERIFIED.json, ACTOR-INVERSES-VERIFIED.json,
ACTOR-CAPACITY-LINT.json, ACTOR-CAPACITY-CHECKPOINT.json and rustdoc log.

Cold inference law DX03 is independent of mailbox configuration: unchanged
9c7e37a rejects real cold Branch→Worker with E0275 from projected child
self-equality. Ordinary actor_bounds already omit this equality for the exact
selected edge; cold custody/native bounds did not. codex/devx-topology applies
that established rule while retaining explicit targets, other-parent consistency,
role/position constraints and ExecutedRoot equality. Production+19/-2/net17,
public types0, no runtime/control-state change. Actual fresh creation, same-action
read/reply and joined original Box custody plus26 existing recursive, wrapper,
supervisor, pool and custody journeys pass current/MSRV debug/release (108
executions,549 frozen files). All24 current-compiler topology denials pass.
Each of the two original self-equality defects independently fails E0275 in
debug/release; restored controls pass. Two one-character diagnostic snippet
updates are proven identical on unchanged baseline, with complete diagnostic
bytes equal to the candidate. Renamed dependency tests pass. MSRV rejects all24 invalid programs, but eight
stored diagnostics differ from1.99; the unchanged baseline produces exactly
the same full diagnostics, so the MSRV snapshot harness remains a recorded
failure. Integration is333dddb, and the restored Library→Archive capacity witness passes.
Its original Full Box, processed accepted prefix and joined allocations remain
owned. Frozen552-file combined matrix passes80 test executions; independent
review then exposed that custody accessors alone do not certify retirement
success. Both new cold witnesses now require root Completed/Stopped, descendant
OwnerCancelled, successful original notifications and empty child/capability/
additional/retirement failure lanes. All eight strengthened executions pass
current/MSRV debug/release; the independent reviewer confirms the previous
false-positive is closed. Production changes0 for these amendments. Final integration
strict workspace/all-target/all-feature lint, formatting and all24 current
compiler topology denials pass with hashes unchanged. Combined consumer proof
is committed; latest main adds three new commits whose Node receiving interface
requires reconciliation before final gates. No full-gate claim. Artifacts: COLD-MATRIX-VERIFIED,
COLD-TOPOLOGY-INVERSES, COLD-TOPOLOGY-DIAGNOSTIC-BASELINE JSON and denial logs.

Latest full Nix retry on frozen9c7e37a failed at963.953 seconds from actual
no-space errors while trybuild compiled dependencies in owner coverage. Ten
gates passed, one failed, thirteen cancelled; all548 hashes remained unchanged.
Keep CLIENT-FULL-NIX-9c7e37a-coverage-disk.log/json. No static denial or resource
failure is waived, and the final full gate remains required.


Native-node/latest-main checkpoint (DX01/04/05,2026-10-14), superseded by the
internal-transport constraint below: the earlier SDK-parameter exception no
longer defines the selected public interface.
The isolated Bombay reconciliation uses one existing Application invocation spine,
original Work separate from Invoke, and a new joined Node.run_with for a business
result independent of the temporary SDK loan. Raw execute_with preserves borrowed
results and separate operations. Same-Node raw callback retry with only Session
typing remains a recorded E0499 limitation; rejected private-family and registry
probes are removed, with no new dependency/trait/public type. Current36-test remote
gate, two intended lifetime denials, complete64-test native corpus in both profiles,
strict owner/consumer lint and intended Host-restoration failure/restored control
pass. Custom library grows113 lines; no actor state or protocol added. Original
Work/payload, accepted reply, complete normal retirement, independent notifications,
caller-local non-Send work, canceled borrowed wait and refusal precedence are checked.
Fresh upstream81dc144 adds fairness, child-isolation evidence and Bombay0.1.2/SDK1.10.9;
next reconcile it after the c1d5b2b checkpoint. Final combined-source MSRV, optimized
inverse, TLS subprocess, packages and all native Nix gates remain open; no requirement
is closed by focused tests. Evidence lives in the existing Bombay coordination PRD
and NODE-CURRENT-VERIFIED/NODE-HOST-CAUSAL-VERIFIED artifacts, with both test reviews.

Stash construction, pre-edit (DX14/19): deliberate API composition policy over
an existing derived replay law. A release may emit accepted Actions for several
held inputs; those actions have no rollback. Existing Stash execution therefore
requires the sealed StaticallyInfallible error capability. Bare Stash::new and
ActorExt.with_stash currently admit fallible inner actors that cannot execute,
release anything, expose mutable state or recover a valid inner actor. Require
that SAME existing capability at construction, with a diagnostic at the caller's
composition expression. Preserve Never and MachineError<A,M,Never> witnesses,
closed phases, wrapper order and complete FIFO prefix/stop/tail custody.

Before production changes, prove owning construction with its existing u8 rejection and ordinary facade
construction with an owned Box<[u8]> rejection both unexpectedly compile. Change the
existing owning compile-fail rustdoc so bare construction is the oracle; add a
focused facade compile-fail fixture and retain the prior closed-phase denial.
Only then put Error:StaticallyInfallible on the two construction methods and
annotate the existing sealed trait with actionable diagnostic text. No broader
struct/inspection bound, new trait/type/default/dependency or transition change.
Reuse existing stash properties, actual worker/scope queue orders and Machine
rollback tests. Add a compile-pass infallible Machine caller beside an ordinary
infallible actor. Expected owner production+one trait diagnostic+one method bound;
Bombay+one method bound; test/guidance updates together. No catalogue migration.

Checkpoint before: control states unchanged (inner B plus owned FIFO queue),
subordinate state/results0 added; the queue owns exact deferred inputs needed
for replay/retirement. Existing StashRoute has3 alternatives, no new branch or
module. Public type/trait/spelling count unchanged. Count exact retained deltas
and replay tests after; scan for semantic flags/history/duplicated causes/nested
authority/structural caller syntax. Cross-check DX14/19, Actions ownership and
Stash statically-infallible replay documentation. Independent design review
finds no supported non-executable storage use lost; constructor eligibility is
the sole narrower promise. Bounded retention/overflow remains separate open work.

Both pre-production REDs now fail for the intended oracle: owner rustdoc says
construction compiled despite compile_fail; Bombay's sole new invalid-error
fixture likewise unexpectedly succeeds, while its prior four fixtures pass.
Preserve those original logs. Retain only the existing capability at the two
constructors; no runtime implementation or type declaration changes.

Source correction before retention: the first owner prototype used the old
0.23 documentation branch. Its stash and algebra source files are identical to
published0.26 apart from this patch, but those runs receive no selected-owner
acceptance. Rebase the patch on published98983b4 (Core/Actors0.26.0,
macros0.14.2; pinned owner Rust1.95), repeat bare-constructor RED before its
production edit, and repeat the complete proof matrix. Bombay's original RED
already uses locked registry0.26.0. The canonical checklist remains in this PRD;
feature source and exact verification are recorded by their own revisions.

Retained stash checkpoint: owner production+9/-1 (net8, including the existing
trait diagnostic), rustdoc+1/-2, tests+57/-0; public types/traits/modules/states/branches and
three StashRoute alternatives unchanged. Inner B, pure route and exact FIFO
inputs remain the only retained values. No history, duplicate cause, semantic
flag, nested authority, structural caller syntax or placeholder added. Both
test reviews and the independent paired-consumer review pass. Selected0.26
nextest runs965 tests without skips; all ten Nix gates pass in305.388 seconds
with unchanged source hashes. Four-workspace/archive preflight passes and all137
archived Rust files match. Bombay current debug/release passes10 tests and12
fixtures, plus strict lint/rustdoc/fmt. MSRV registry execution finds one prior
worker-factory golden mismatch (three "an"/"a" articles); new stash fixtures
pass, and its failed command remains recorded. Review/delivery/publication and
strict complete supported-compiler replay remain required. Bounded stash/Machine
retention and overflow remain open.

Native/latest-main integration law, pre-edit DX01/15/20/25/27: combine committed
native interpreterb1c6781 with verified delivery04104d8 and current maine2a4e7f
(PR372). This is conservation of two existing implementations, not a new actor
law or interface. Preserve actor-owned services and guarded initialization at
root, fresh child and Entity construction while retaining main's exact physical
application association for local/remote clients. Preserve every original input,
request/rejection, accepted prefix and independent retirement result. Inspect
all three native construction cuts and existing LocalEnvironment/endpoint/client
paths before resolving merge conflicts. New syntax remains the already proven
Application and topology constructor selection; no new bound, marker, wrapper,
field or policy may originate from compiler fallout. Expected changes are merge
reconciliation in existing construction sites and their owning test invocations;
reuse each side's original focused and full checks. All37/39 diagnostic and418
runtime receipts keep their pre-merge source limits. Fresh combined all-feature
checking, focused original native/client consumers, supported diagnostics and
full repository/archive gates must establish the combined revision.

Joined Node/main integration, DX01/20/25: nativeb1c6781 and delivery04104d8
are integrated with maine2a4e7f at42ba1dc in the isolated interpreter branch.
Seventeen incoming receiving consumers use the existing complete into_parts
product; all369 original assertions and effects remain, independently reviewed.
A focused real Counter journey through Node::run_with fails exact NodeAbsent on
both receipt profiles before its production correction. Mirror the existing
paired invocation's actual Session association in joined invocation; no new API,
bound, state, policy or authority. Both compilers' debug runs then pass the two
joined Counter and two original paired Ledger cases. Both independent source
reviews pass. The committed correctionfb295b6 passes all four construction cases
on both compilers in debug/release; the optimized original-defect replay fails
both joined cases with NodeAbsent and its restored control passes. Fresh39-case
immutable diagnostic checks pass both compilers with609 conserved hashes.
The corrected native/client/Node matrix passes465 listed outer tests and nine
exact client fixtures in each current/MSRV debug/release profile, conserving609
source hashes. Its first verifier incorrectly counted fixture paths as outer
tests and receives no passing-matrix credit. Three extracted archives contain75
byte-identical production files; renamed consumers pass18 journeys on all eight
compiler/profile/feature configurations. A separate completion audit conserves
609 repository and493 extracted/consumer files plus the three archive hashes.
These are15 native and3 pure-IO journeys, not18 native journeys; archive UI and
broader remote acceptance are excluded. Both independent evidence reviews pass.
The full24 native Nix attempt on cleanfb295b6 fails after4510.859 seconds:
20 gates pass and4 fail. Cold-role snapshot drift blocks test/coverage; a protected
consumer still expects the old tuple; remote debug lint rejects one long test.
No signing/disk waiver applies. No pre-merge or failed check proves final combined
acceptance or the later actor-only Node callback.

Adversarial test-review limits, DX01/20/25: the second independent review
confirms the joined Node correction only for actual association, declaration and
Session identity. The Counter construction cases issue no Query; their two
receipt types do not establish provider verification or request-key correctness.
Their shared assert_completed inspects terminal shape/empty failure lanes but
drops actor state and checks settlement count, so it does not prove exact actor
contents or settlement payloads. Incoming cancellation checks one stashed command
without inspecting B's exact recipient; incoming Ledger pressure checks accepted
prefix contents without proving every prefix Box allocation. These are explicit
coverage gaps, not newly demonstrated runtime defects. Seventeen receiving
migrations preserve every observed value/effect, but369 unchanged assertion calls
are not independent proof of those stronger laws. Keep full remote acceptance
open and retain the exact NodeAbsent original-defect/restoration evidence for the
narrow joined correction. No passing gate can erase these limits.

Transport interface constraint, DX01/20/23: the latest user instruction makes
Zenoh an internal Bombay implementation choice, with no transport/bus selection
or SDK access in ordinary business code. Existing public Node callback/config
exposure is therefore an open interface gap, not an accepted ordinary-user
exception. A caller must be able to compose actors and use typed application
clients without importing Session, Query or native SDK configuration. Internal
runtime and transport verification may still use their owning SDK directly.
Do not invent a second networking layer, expose an SDK escape hatch as the user
path, or conceal a still-required SDK argument behind a cosmetic wrapper.
Establish a real caller witness before changing this existing interface;
implementing new remote delivery, authentication, identity or deployment features
remains outside this selected local programme. The current source-frozen gates
verify the current implementation only and cannot close this new interface gap.


Actor-only Node callback, DX01/20: the isolateddevx-network branch first records
one real Counter caller that fails E0282 at its inferred one-argument callback.
Replacing only run_with's SDK callback with the existing Application FnOnce
contract removes the Session parameter and higher-ranked callback requirement;
no type, constructor, state, dependency, capability widening or new bound is
added. Work still returns intact on missing-host refusal. The focused current/
MSRV debug/release journey passes, retaining an actual non-Clone File, original
Box allocation, reply/origin1, retired Counter value1, both notifications and the
exact complete final Stop settlement. A real completion-owner settlement-loss
inverse fails0-versus1 and its restored control passes. Both invariant and
counterexample reviews accept this scope. The same actor-only callback preserves
export association: both receipt-profile construction cases pass, removing its
private association fails exactNodeAbsent, and restoration passes optimized.
Those cases issue no Query and prove no provider semantics. Committeda1da19d
migrates joined callers; all40 Node/client tests and9client denials pass in each
current/MSRV debug/release profile. Both independent evidence reviews validate
exact names and609 frozen source hashes. The identical Session-argument fixture
passes on originalfb295b6 and fails solely E0593 on the new API; dedicated-target
three-case suites pass both compilers. Shared targets had reused a control binary,
including after sequential commands; both failed attempts and the overlapping
control receive no credit. Final replays rebuild all three owners in dedicated
checkout/compiler targets and verify exact suite names and hashes.

The actual threea1da19d archives contain75 byte-identical production files.
Renamed external consumers pass the same4Node journeys on8compiler/profile/
feature combinations;609 repository/491 extracted-consumer hashes and all3crate
hashes remain conserved. Both independent reviews pass. Setup still names SDK
configuration and a separate paired test retains the old SDK loan; no whole SDK,
Query/provider or publication claim applies. The separate94f607a test-only change
uses one equivalent Endpoint destructure without deleting any assertion; its
focused unit passes current debug/MSRV release and complete all-target/all-feature
strict debug lint passes20.387 seconds. The earlier40-test matrix excludes that
later cfg(test) edit. Commit1a42e24 records internal transport as a standing AGENTS
rule. Paired SDK work, configuration and query custody remain unfinished.

Final gate repair preserves both protected receiving joins using existing complete
into_parts; apparent reference errors disappear without changing a borrow or bound.
The real recorded/scheduled requester journeys pass both compilers/profiles:
current debug2.562/release196.190seconds; minimum debug59.979/release223.962seconds. Add those2cases to both existing Nix gate profiles because
the former public_node_ filter compiled but did not execute them. Existing limits:
its generic native oracle accepts an empty accepted-settlement vector and does
not inspect the receiver's final Counter value; native reply1 and exact requester
state[ReadReply(1)] are independent observations. Both source reviews pass the
mechanical migration, not stronger unobserved guarantees.

Cold coverage adds existing4negative/1positive fixtures:44cases/9suites/39outer
tests through the existing immutable diagnostic tool. Current cold target passes7;
minimum generation identifies3additional compiler-specific snapshots. Every
fixture stays unchanged; distinct role/protocol/birth errors persist, with two
duplicate cascades omitted by the minimum compiler. Both reviews validate all13
canonical/minimum/source hash bindings and the unchanged earlier10records.
Generation supplies no acceptance credit. Fresh immutable44case replays pass
current117.091/minimum120.988seconds: all9suites/39outer tests,13snapshot
bindings and caller/copied source hashes are conserved. The610captured files
include one generated Python bytecode file, removed only after both terminal;
all609 repository files remain. Two prior runs failed the transcript parser
because pass-only trybuild omits `[should pass]`. A one-line format correction
follows a failing real-transcript control; all20controls accept both valid formats
and reject opposite modes, missing/duplicate/extra cases and incomplete suites.
Strict workspace/protected lint and Rust/Nix formatting pass.
The Engine prepared-ingress diagnostic mismatch is an intentional mutation KILL;
its baseline passes and no Engine correction is justified. Full24 checks must
be rerun on the corrected clean source.
Both final independent evidence reviews accept the44-case source/log/compiler
bindings. Bombay c6590d7 retains the bounded gate repairs; a fresh full24 native
Nix check is running on that clean frozen commit. No overall pass is claimed.

Paired actor-only work (DX01/20/23), isolated Bombay9ffba4f: the same existing
Application callback now serves run_with and execute_with. Missing-host retry
retains the original File/Box; paired wait cancellation retains actual Rc/&Cell
and both original futures. Exact reply/state0/1 and the full retained Stop row
remain independently checked. Three actual signature inversions fail for arity,
Send and 'static and restored controls pass. Both source/evidence reviews pass.
The5 runtime cases and initial2-denial harness pass8 compiler/profile/feature
configurations with614 frozen hashes; the final existing node_borrowing harness
adds a caller-borrow positive, preserves joined SDK denial, independently denies
paired SDK input and close overlap, and passes its4 fixtures on both compilers
in debug/release. Final optimized commands cover only that static harness;
production/runtime body hashes still match the8-way proof. Artifact row recovery
from a case-insensitive filename collision is explicitly reviewed, with no new
pass credit. Focused lint and format pass. No public type/state/module is added;
production+11/-9 includes comments, signature/body+4/-4/net0. SDK-dependent native
consumers remain unmigrated: this is a focused feature commit, excluded from the
clean c6590d7 full24 run and from whole-repository/SDK-hiding acceptance. No new
session accessor, callback family or dummy export is permitted to unblock them.


#### Supervision and pool reply capabilities — DX02/10/11/17/20/26

Law and source of authority: exact established reply recipients already contain
the destination authority. They require no logical namespace. Logical replies
retain their static hosting obligation. This is derived capability composition,
not a new actor-model guarantee. Current ReplyRoute deliberately accepts both
capabilities, so its metadata truthfully requires both even when the actual value
is exact. Do not delete that metadata, add a dummy host, widen capabilities or
infer a type-level proof from a runtime enum variant.

The pre-edit real6c5c18a example copies remove only reply aliases/host declarations
and the explicit supervisor external-client protocol. FIFO fails three missing
Hosts E0277 diagnostics; supervision fails six. Neither is an inference failure.
Worker business bodies and startup/replacement/admission/completion/shutdown work
remain unchanged. Printed main output alone supplies no full failure/custody
oracle. Raw REDs: POLICY-POOL-NO-ALIASES-RED and
POLICY-SUPERVISION-NO-ALIASES-RED under the existing external results directory.

Selected syntax:

```rust,ignore
let pool = queue::new(factory, roles, config).build()?;
let supervisor = roster::new(factory, roles, config).build()?;
let mut client = application.client()?;
client.send(FifoCommand::submit(submission, job, client.recipient())).await?;
```

Ordinary construction fixes exact reply endpoints. Advanced FIFO callers may
select logical_replies before build; supervisors select logical_status and
logical_capabilities independently, in either order. A shared all-query logical
choice was falsified by a real Status-only caller needing an unused Capability
host (three E0277 diagnostics). Its nine supervisor files were archived/restored
before the independent model was written. See DEAD_ENDS.md; do not restore that
choice or add the unused host.

Pre-edit ownership/model: reuse existing DeliveryRoute for both actual route
substitutions—EstablishedRecipient<P> and ReplyRoute<P>. The sealed ReplyRoutes
family selects only that metadata, not a second interpreter. It is needed because
Worker/its reply protocol are determined when the owning builder builds. ExactReplies
and MixedReplies are static choices with no runtime state or caller marker value.
FIFO stores the actual chosen customer with queued/assigned work and emits the
same chosen route's sends. Only accept_submission/Behavior require its existing
Clone operation (one admission copy while preserving the original); only Behavior
requires the selected SendsFor obligation. Supervisor status/capability fields and
send lanes use separate instances of the same family, giving four meaningful
combinations. No family-wide Clone/Send bound or generic default is added.

Builders and complete construction refusals preserve the same zero-sized protocol
index plus the original factory, prepared prefix, rejected role/reason, unvisited
suffix, Config and lifecycle route. New FIFO build defers its existing ordered
preparation until that explicit operation; Config performs no work. This adds one
ordinary build call. FixedRoster/member lifecycle storage is not parameterized;
only the six existing complete-error method seams and InitialProxyDecision retain
the two indexes needed by the complete rejected event/error/diagnostic. Existing
lower-order preparation, creation, observation, recovery, delivery and retirement
machinery is reused. CustomerDelivery and its distinct original-customer sidecar
belong to keyed/router paths and remain unchanged. No interpreter is broadened.

Smallest actual runtime: an exact git archive of Bombay6c5c18a, with only five
already-proven owner module/startup name adapters in lib.rs, worker_preparation.rs
and local/effects/{creation,delivery,mod}.rs. ReadyForStartup is included. All
shared registry package versions match the original lock; only the three explicit
Behavior path patches change their sources. This is a local native integration,
not a stub, published-package proof, or repository-wide Bombay migration.

Focused evidence on the corrected independent model (pinned Nix):

- Owner six focused tests pass debug/release (3.594/16.277seconds); strict focused
  Clippy and formatting pass. Two original queue tests preserve every preparation
  refusal/Config assertion, adding only build and removing unused mut; four command
  witnesses retain the existing broad/mixed contract. These are bounded proofs:
  initialization does not separately check distinct nonces; stateless preparation
  cannot detect an early Reserve call; command sinks do not inspect route payloads.
- Six actual native tests pass current1.99 debug/release (14.838/83.177seconds):
  FIFO queued admission, original rejected job allocation, search completion,
  supervisor tree retirement and both worker Stash/StopOnShutdown orders. Both
  independent test reviews distinguish laws from missing or vacuous observations.
- The wrapper oracle observes real Release acknowledgement, then authoritative
  Empty BEFORE external root shutdown, then retained Released worker, matching
  actual emitting address and zero stashed inputs. The earlier Retired expectation
  was a test-model error: temporary recovery lawfully leaves an Empty stable proxy.
  Dropping the actual held Stop in Stash makes both cases fail specifically at
  the bounded Empty observation (15.715seconds); byte-restored source passes all6.
- The native normal-retirement oracle exhausts ActorCustody fields and independently
  checks Work cleanup, both notifications, tree origins/cardinality and Stopped
  completion. One accepted final row is required, but exact inner receipt/effect
  payloads and pairwise fresh addresses are not fully inspected. No stronger
  accepted-prefix or cancellation claim follows. Cloning the test's submitted job
  makes the allocation oracle fail; restored input passes (4.398seconds). This is
  an input negative control, not an owning-production clone mutation.
- Status-only and capability-only callers host only their genuinely selected
  protocol; both-query callers emit both actual query kinds. All four combinations
  and both selector orders compile (3.620seconds). Missing selected Status or
  Capability hosts each fail three E0277 diagnostics (0.856/0.845seconds). Correct
  protocol control compiles; identical wrong-protocol input fails E0277, and its
  rustdoc denial runs exactly one case. The host probes are compile witnesses only; the separate native rejection
  proof below supplies bounded runtime custody evidence.
- The real replacement example executes original startup, business Stop,
  replacement startup and orderly retirement; cleanup/notifications succeed
  (1.196seconds). It retains the existing ManagedWorker alias in worker.rs.
- Actual native optimized pool/command inline layout is256/80 bytes for BOTH exact
  and mixed choices. No memory reduction, heap/latency or compile-speed advantage
  is claimed. Cached timings are receipts, not comparable compile-cost benchmarks.
  In the copied mains, pool90→90 and supervisor99→96 lines include Config adoption;
  reply aliases1/2→0/0 and reply host rows1/2→0/0. The worker alias remains open.

Static capability checkpoint before custody/startup: production+1302/-395/net907 across15 source files
including replies.rs; focused owner tests+8/-3/net5 across2files; public types+4/-0
(ReplyRoutes, ExactReplies, MixedReplies, FifoBuilder). This is net-positive
capability code, not code reduction. The recorded2026-10-09 expanded budget applies.
No new dependency, actor control state, result alternative or runtime branch is
introduced. The type propagation has a real source/review cost.

Aggregate checkpoint scope: the14 changed existing source modules plus replies.rs,
excluding five cfg(test) items. FIFO controls remain Constructed/Operating/Draining/
Stopped/ForcedRetirement (5→5); FixedRoster controls New/Operating/ShuttingDown/
Terminating/Stopped (5→5). All29 existing enum declarations retain142 named
alternatives. Transparent syntax counts are unchanged:271 match expressions,
658 arms,28 if tokens,2 loop,1 while,0 question-mark operators; these are not
semantic-transition counts. Compiled nonblank lines8353→9211; nonblank physical
production including comments8667→9549. Other owned submodules remain byte-exact.
Every existing variant keeps its original current owned product; only the actual
reply destination's static type is selected, never reconstructed from a second
cause. Queue, admitted assignment, current worker attempt, lifecycle/recovery
capabilities and complete rejected event retain their original authority. Source
reviews conserve135 supervisor/64FIFO non-constructor function bodies after only
removing the recorded generic indexes/trailing formatting commas. No arrival
history, repeated cause, false cardinality, nested actor engine, semantic boolean
or structural caller path is added. Cross-check the normalized roster/recovery,
FIFO ownership, worker preparation, creation/provenance and DeliveryRoute laws.

Final two-pass source review renews conservation of all135 supervisor/64FIFO
non-constructor bodies and all29 enum field products after only the recorded
route substitutions, generic indexes and optional trailing commas. The changed
variant payloads are FifoCommand::Submit, its event/unexpected-input carriers,
FifoDispatch's retained job/customer refusals and PoolState::Operating; supervisor
Status/Capability, their event/rejected-input/diagnostic carriers and the existing
InitialProxyDecision::Rejected. Each retains its original affine product.
Public spellings under this scope: structs18→21, enums17→17, traits0→1,
functions43→47, fields20→20, modules0→1; restricted declarations stay unchanged.
Re-export blocks16→16 add FifoBuilder once. Counting four new public types, four
qualified methods, the replies module and Route associated spelling gives+10/-0;
the private sealed trait is not an external implementation port. Disposition:
pass for the static capability/aggregate checkpoint only.

Final immutable replay conserves708 Rust/manifest files, with owner workspace
context matching186266e: six named owner tests per1.95 debug/release profile,
six native cases per1.99 profile, four genuine hosting combinations, three
selected-host denials and right/wrong-protocol controls. Every expected test name,
zero unexpected ignored/failing cases, expected E0277 counts and raw log hash are
checked. Receipts REPLIES-FROZEN-* and policy-applications/{final-replies-sources,
final-replies-results}.json bind the external evidence. The frozen replay timings
are0.519/1.280seconds owner and4.653/20.841native; they include cache effects.
These repeat executions establish final source binding, not additional laws.

Status: focused capability proof; whole-programme acceptance remains Open.
All original owner consumers, portable native fixtures, complete exact
settlements, comparable compile costs, package/archive proof for the reply API
and both full repository gates remain owed before its delivery. The bounded
logical failure and minimum-compiler evidence is recorded below. Do not call this published or merge the prototype. Timer
ownership and the separate worker-factory alias/associated-type question remain
separate open work; no placeholder or arbitrary bound may hide them.


Reply custody (DX16/20/25), derived ownership law recorded before lowering:
the Driver drops Exhausted custody; mixed ReplyDeliveries must retain exactly
what the existing Vec law retains. Original rejected, untouched and corrupt rows
stay in authored order; accepted receipts use ActionItem::retain_accepted.
prepare_source owns idempotent classification and finish_source delegates.
This adds no type, trait, bound, public operation or actor state. The borrowed
unit-pattern candidate is rejected in DEAD_ENDS.md; the owner port, not compiler
suggestions, supplies receipt retention.

The pre-edit native journey reaches an actual Ready proxy, sends Status to an
unbound logical recipient, then shuts down through the same mailbox. Normal
termination/FailuresFound still loses the required retained row (3.446seconds).
The exhaustive oracle owns the UnknownAddress request/snapshot/actual proxy,
Stop, Work cleanup, notifications and joined proxy/worker custody. Six owner
regressions check non-Clone invoice allocations, exact Vec control, both mixed
routes, accepted-unit exhaustion, interleaved residual order and original/
offering/completed/empty phases with repeated finishing. Original source fails
four with two controls passing. Both ownership/oracle and adversarial/vacuity
reviews pass. A consuming extraction caught inside assert was moved to a prior
let; no production law changed.

Custody drift:31 enums retain146 alternatives;293 matches,714→715 arms,
31→32 if,2loop/2while/0question marks. The added choice selects existing
Retained/Exhausted source custody, never another actor authority. FIFO5/roster5,
all subordinate owned products, provenance and initialization remain unchanged.
No history, repeated cause, false cardinality, flag, inferred provenance or
ordinary structural path appears. Cross-check source settlement, Driver disposal,
DeliveryRoute, creation and lifecycle laws. Disposition: pass for bounded custody.
Earlier715-source results remain bound in source-custody-{sources,results}.json;
subsequent startup and migration have separate final bindings below.

Independent correction on origin/main98983b4: draft PR101/184354b contains only
production+42/-9/net33 and347 test lines, public API0. All970 nextest tests pass
(38.030seconds), native Nix exits0 for all ten checks (62.202seconds), packaged
archive consumer passes (7.800seconds), four-workspace release preflight passes
(6.338seconds), and all14 exact-head GitHub checks pass. Authorization for merge/
publication is pending. Unchanged Bombay6c5c18a (all85 Rust source files identical,
all59 registry selections/checksums conserved) proves both genuinely hosted
logical Status/Capability UnknownAddress failures with original recipients and
payloads beside complete normal retirement. Locked debug/release pass
(10.323/24.958seconds); identical consumer on98983b4 fails rows1 versus3
(7.502seconds). No exact per-action row partition, independent proxy-to-child
identity correspondence, native closed-recipient or new panic/cancellation law
is claimed. These original-owner gates do not certify the larger prototype.

Latest-main integration remains open: Bombay mainc249044 adds provider result
conservation tests; isolated merge5548924 compiles red on old execute_with/
three-part outcome syntax and allocator-owner imports. Its Nix selector names a
removed external module. Reuse private session capture and complete existing
ApplicationOutcome::into_parts; keep allocation sampling in its isolated allocator
owner with the same concrete receipts. Preserve all observations. No public SDK
surface, new bound, global allocator or transport escape is justified. The later
40fbb61 pure regression commit does not certify the combined full remote gates.

Startup authoring (DX10/11/17), derived static adaptation recorded before lowering:
WorkerSubmission::with_startup(worker, move || async move { ... }) infers both
composed worker and startup Result without aliases, a Startup struct or repeated
associated declarations. The existing StartupPlan equation is Send FnOnce()->Work,
with Work a Send future returning Result<Send Ready,Send Rejection>. Invoke the
original closure once at start; return its original future without polling.
Original successful/rejected values and returned-future disposal keep ownership.
Reuse WorkerSubmission, StartWorker, initialization permits, proxy/pool state and
native startup tasks; no new state, type, operation, dependency or runtime port.
Named nonclosure plans retain their port; qualifying closures cannot separately
implement StartupPlan. Migration guidance documents that coherence restriction.

Before production, four owner cases and the actual factory fail missing
StartupPlan. The direct Future candidate was removed (DEAD_ENDS.md). Final
constructor-count cases fail17 E0277 against that candidate; the real closure
factory likewise fails. Calling plan() during submission construction is forbidden.
Matched optimized1.99 storage is16bytes named plan,32anonymous future,16deferred
closure; all inline pools256bytes. This supports deferred construction without the
measured suspension-state increase, not universal layout, heap or latency gains.
The actual initial/recovery factory shares its existing notification sender and
returns real SendError<Role> instead of the older example's panic. Notification
inside startup precedes committed readiness, so its consumer queries until Ready.
This is a deliberate failure-policy improvement, not identical failure semantics.

Four focused tests observe construction0→1 at start, body entry0→1 only at await,
complete non-Clone success/rejection Box identity, and once-only disposal of
unpolled/Pending returned futures. The counter is body entry, not every poll.
Both independent reviews pass. Disposal of an uninvoked closure and native panic/
cancellation/rejection custody remain outside these positive journeys. Production
runs start inside the native task and receives panic through Tokio JoinError;
the explicit catch_unwind(plan.start()) source inspected earlier is cfg(test).
No original-grant/permit panic custody is inferred from it.

Initial closure proof passes16 owner tests1.95 debug/release and7 actual native
journeys1.99 plus minimum1.96 in both profiles. Four native cases adopt startup
closures (two FIFO, two worker layouts), three controls remain byte-identical;
all original supervisor/FIFO assertions remain. An obsolete Pool spelling in a
test-only signature was removed by inlining its unchanged complete retirement
oracle, not by adding a generic premise. Their positive paths do not execute new
SendError/RecvError failures. Frozen730-source closure proof, names, denials,
commands and log hashes are in closure-{sources,results}.json; final migration
binding is below. None of these timings is a comparable compile-cost benchmark.

Startup drift:worker/mod.rs+32/-0 (16implementation/import,16rustdoc), public
named symbols0; no new actor branch or control sum. Worker StopKind2,
WorkerCreationRejection8 and WorkerCreationOutcome2 keep every original product.
FIFO5/roster5 and all prior static-capability checkpoint counts above remain.
No new match, if, phase, cause, history, flag, nested authority, widened capability
or ordinary structural syntax appears. Cross-check initialization/StartWorker,
worker preparation, FIFO, roster/proxy creation and retirement. Disposition: pass
for focused startup authoring/ownership; final failure/cost acceptance stays open.

Further caller simplification needs no owner change: proxy birth/replacement
already constructs StopOnShutdown<W> and targets that exact outer layer at Here.
Owned shutdown is consumed, not delegated to W. A copied main with raw Worker
still completes startup, replacement and clean retirement (7.677seconds), without
worker/startup aliases. Printed output is not a complete custody oracle; a normal-
retirement differential remains owed before that simplification's migration.
The older two Stash/shutdown worker layouts stop by business Stop/Release before
supervisor shutdown. They prove Stash work and forwarding, not genuine execution
of the inner shutdown policy. Do not retain redundant shutdown to count orders.

Composition (DX10/11/17), existing Bombay policy law recorded before tests:
compose Stash and OneShot in both fluent orders without naming the worker type.
Closed business state rejects an original invoice; Stash instead retains two
non-Clone invoices. Use the actual authored ScheduleAfter id/generation for one
reminder while both remain held; duplicate elapsed input emits nothing. Open
changes current business state, then releases original FIFO envelopes with exact
allocations, contents, origins and balances13→42. Stop returns empty effects and
termination. Observe all Actions lanes and current base state after every turn.
Reuse ActorExt, Activate and existing products; add no production symbol, bound,
forwarding method, structural path or interpreter capability. The common equation
has two actual wrapper substitutions. This proves pure effects, not native clock
acceptance, joint startup or nonempty inner initialization.

Both reviews pass after the independent destination oracle uses original
MailAddr59. Raw rejection plus both orders pass against real Bombay's registry
0.26.0 dependencies in debug/release (13.820/33.352seconds). Final lint-only edits
merge identical route arms and explain the complete trace's length; both reviews
remain valid. Final debug/release pass (1.161/1.187seconds), strict Clippy/format
pass (0.787/0.631seconds). Isolated copied owner inversions drop timer Actions or
reverse Stash FIFO: each fails both orders with the raw control passing. All702
copied source hashes restore exactly; a preserved-mtime restoration first reused
the mutant artifact and stayed red, then forced recompilation passes all3.
Bombay40fbb61 retains the266-line public regression and compact coordination record.
Drift:production/public API0; fixture Closed | Open(balance), normal Reminder |
Paid(original invoice,origin,balance) | Rejected(original invoice,origin). Every
field is a current business value or original obligation. No historical state,
repeated cause, false cardinality, nested authority or structural caller syntax.
Cross-check OneShot lease/init and Stash release/replay laws. Disposition: pass
for this pure composition; stronger native/failure obligations remain separate.

Proven-syntax migration changes no production law. Explicit advanced runtime
fixtures select their real existing mixed reply contracts; ordinary constructors
remain inferred exact endpoints. The first proxy ingress migration passes56 tests
(3.466seconds), supervisor fixtures99 (3.667seconds), FIFO94 (5.078seconds).
Both review passes reverse every migrated source to HEAD after removing only
recorded family indexes/imports, selectors/build calls, formatting and one unused
local mut. All assertions, requests, refusal matches, affine operations, model
transitions and exhaustive branches remain. Tests inside cfg(test) modules get
the same changes; their production prefix is byte-identical. Cascading ownership
diagnostics are not repaired. One initially misclassified worker-custody case
really sends to a logical customer later in the same test: its logical opt-in
preserves that original route instead of adding a host or forging an endpoint.
No new bound, placeholder, callback, compatibility spelling or capability is added.

Final owner workspace nextest passes979 tests,0skipped (40.616seconds). Final
native Nix invocation exits0 for all ten selected checks (158.508seconds), including
979 optimized tests; other architectures are not certified. External interpreter
contract passes19tests including7existing compile denials (20.450seconds), packaged
archive consumer passes (6.147seconds), and every fuzz binary compiles
(12.747seconds). Instrumented mixed FIFO and supervisor-correlation smoke runs
complete1000inputs each (29.059/10.685seconds), not a long fuzz campaign or an
exact-family corpus. Generated corpora are archived outside the source tree;
original tracked seeds remain byte-exact.

Nix verification: the first full invocation exposes Darwin's background
path-audit classifier segfault; Nix returns0 because the classifier is not joined.
A fresh rooted nextest build passes979/0skip (202.077seconds), then all ten checks
return0 with max-jobs1/cores2 (65.766seconds), with no reported classifier crash.
This does not establish the cause of the intermittent failure. The sandbox audit
still cannot query registry yanks. A separate pinned online cargo audit, using an
isolated Cargo home, updates the registry and scans all65 locked dependencies
successfully (3.755seconds). No signing/disk failure occurs in these runs.

Audit probes preserve that distinction: an injected classifier exit37 is masked
by the original routine, but returned by the copied routine with an explicit
producer join and top-level status check. It classifies the file headers of an actual98,120,993-byte compressed output
under all four C/en_US.UTF-8 and ordinary/strict-shell combinations; archived
contents are not inspected. Exact source/log
hashes are in REPLIES-NIX-CLASSIFIER-STATUS.details.json. An earlier locale probe
found absent old outputs and returned before inspecting files; it receives no
credit. These are observations of the tooling, not a repository patch or waiver
of the original subprocess failure.

Final migration binding freezes732 source/manifest/context files, changing19
existing test/benchmark files from the closure binding. All hashes remain equal
through7native startup/custody journeys plus3pure policy tests in current1.99
and minimum1.96 debug/release:10pass per profile (25.452/105.239/20.562/114.269seconds).
This remains an actual runtime archive with the five proved naming/startup adapters
and three owner path patches, not registry or real latest-main acceptance.
migration-{sources,native-results}.json bind commands, exact names and log hashes.

Final physical ledger relative to186266e: production+1376/-404/net972 in17files;
tests/benchmark+1156/-196/net960 in24files; documentation+22/-5/net17 in2files.
One source file contains both production and test changes, so42 distinct files.
Counting whole src paths gives+1550/-528/net1022 in18files:the extra50lines are
cfg(test) changes, not new production. Public types+4/-0 and public spellings+10/-0
remain as the static capability checkpoint. The2026-10-09 expanded budget applies.
Migration adds no state, alternative, branch, authority or public spelling; this
is net-positive capability work, not code reduction. Complete original-value
failure coverage, registry delivery, latest-main remote integration, comparable
costs, remaining catalogue paths and both repository acceptance gates remain open.

Delivery checkpoint: owner branch codex/devx-replies is clean at422e840 and
pushed as draft PR102, stacked on PR98/186266e. Commits a46cfbe (byte-identical
PR101 custody),1a22a9a (startup plus rustdoc/migration guidance) and422e840
(reply inference plus original consumer migration) preserve all732 frozen hashes.
Post-commit evaluation matches the exact ten gated Nix derivations. The owner
results binding is migration-owner-results.json. Explicit CI run38830157300
passes at422e840: Nix, published-API audit, eight mutation shards and their join
(11 jobs). Main-only CodeQL/dependency workflows do not trigger for this stacked
base and remain required at main integration. This is neither merge nor
publication approval.


Latest-main provider correction (DX20/23/25) is retained at Bombaycd77c8b. The
pre-edit compile fails13 errors. Existing private connection capture and the
complete four-part outcome repair its old callback/result contract; no public
SDK capability, dependency, production state or production Rust is added.
One shared provider source preserves both complete concrete families and existing
Requester Scope. Native tests retain actual Export sharing, buffers, proofs,
reference counts, duplicate/conflict/replay, pending work and full retirement.
Only Rc allocation counters move to the existing isolated allocator owner, using
public verify and the same source. Four original Rc::new windows and one clone
window exclude verification, SDK work and lookup. Recorded/scheduled blocks
measure376/392bytes on this host; each clone measures0. This is neither total
native memory nor measurement of the identical SDK-owned process objects.

Two independent source/evidence reviews pass. All18 required commands, exact
cases, key-derived exit modes and0ignored summaries are checked:48passing
executions and12 intended failures. Each failing case's own panic header points
to its frozen primitive assertion. Both original/restored593-file maps match.
Compiler versions are observed post-run; variant hashes are reconstructed from
the immutable driver/originals, not sampled per-run inventories. Actual commands,
timings and hashes remain in provider-sharing/{final-sources,results,verified,
verified-modes}.json. The first global-substring guard alone grants no attribution
credit. Rust1.99/1.96 debug/release pass6native+2sharing each; restored current
profiles add16passing executions. Omitted-window/extra-allocation calibrations
fail2cases per profile, and omitted Node.close fails both native cases at the
closed-connection assertion per profile. Final strict lint/format passes.
Production/API0; tests+551/-389/net162 in5files, gates+13/-3/net10 in1file;7files
including coordination documentation. Provider sums remain4enums/9alternatives;
Node declaration Some(Application)|None, independent connection, all production
branches/modules and public spellings are unchanged. No historical state,
repeated cause, semantic boolean, nested actor authority or structural caller
syntax is added. Cross-check Node-private close, Driver custody and authoritative
fact conservation. Disposition: pass for this test-only batch; programme open.

Main76dbd84 is merged at06d6fd2 after the sole dependency-sentence conflict.
Behavior/Actors0.26.0 and macros0.14.2 remain selected; Communication0.1.6 matches
its published checksum, VCS963067e and all21 packaged files. Both locks retain
all399/324 neighbors and every Rust file equals cd77c8b. Capacity observation
reserves no slot or authority; Consumer preserves the original across notification
panic. All24 native gates are running on this combined revision; old0.1.5 proof
is not its acceptance. No merge/publication authorization is inferred.

Pool completion composition — DX08/10/11/16. Pre-edit law/provenance was
recorded at9b03cc0: an ordinary worker consumes its actual Assignment and emits
both parent Completion and typed business Delivery in named Actions lanes.
This is derived explicit-effect composition, not a new pool policy. Baseline422e840
rejects that caller only because sealed CompletesAssignments recognizes fixed
completion batches/SendLayer; the unchanged completion-only control executes,
and missing completion remains denied. Baseline source/command hashes are in
worker-effects/baseline-{sources,results}.json; construction is not execution.

Implemented in234235d, with rendered-link correction a25afb7 and draft
[PR104](https://github.com/devrandom-labs/bombay-behavior/pull/104), stacked on102.
Both owning constructors reuse existing
SendInput<ReportToParent<Completion<R>>,Path> and R:Send. Path is inferred only
at construction; no worker alias, custom marker, stored route or actor generic
is added. Remove the sealed trait and nine repeated non-constructor premises.
Constructor refusal still returns its complete factory/prepared prefix/role/
reason/suffix/configuration product. No interpreter or transition is replaced.

Complete root sums remain FIFO Constructed(prepared workers), Operating(current
pool), Draining(members,deadline), Stopped and ForcedRetirement(members,cause);
Keyed has the same five alternatives with Retiring(workers,deadline) for drain.
The module-qualified source-AST checkpoint excludes inline cfg(test) modules
and their two explicitly pathed external files:59 sums/240 alternatives,56
products,320 function bodies,830 match arms and47 if branches are unchanged.
Every surviving alternative's complete fields and every body are token-identical.
Production is+25/-67/net-42 over eight files; selected source files stay23 and
declared modules fall23→22. These are source counts, not macro-expanded or
feature-evaluated production metrics. The application contract loses one trait
and its associated type; no public type is added. Unrestricted pub declarations
fall190→187, including the private sealing trait. The full25-file source
inventory, including tests, is63/248/78/416/926/55, with34→33 declared modules
and20047→20005 physical lines. Initial tools collided on three test struct names
and included two external test modules in their exclusion row; those figures
have no retained counting credit. See worker-effects/pool-census-v3/
ownership-{sources,results}.json (54 frozen inputs, locked pinned replay,
two independent review passes). Physical selected-file line counts still
include inline test text and are not production-only totals.

Current evidence: plain FIFO, Keyed, both exercised OneShot/Periodic orders and
closed-auditor rejection pass current1.99/minimum1.96 debug/release (five native
cases per profile, zero ignored). They preserve correlated completion, both
produced Box allocations, exact sender/auditor identity, independent cleanup/
notifications and joined children. Closing the real auditor retains its exact
EstablishedDelivery/ClosedRecipient/Continue row beside final Stop while parent
completion succeeds. Job input is u64, not an affine-input proof; Keyed still
names two actual domain reply protocols/hosts. These are old actual runtime
archives with their five recorded syntax adapters, not latest-main acceptance.
The unchanged five-case consumer against repaired Bombay214d5b0 first fails
four E0432 imports because delivery still uses published atomic module names.
The pre-recorded current-consumer stage repeats only the existing five-file
module/startup migration in an exact isolated214d5b0 archive: original files
match6c5c18a, replacements match the already-proven adapters, all615 other
tracked files match214d5b0, and every consumer assertion/oracle is unchanged.
This mechanical migration adds no law, state, bound, wrapper, protocol or
interpreter capability. All five native cases pass current1.99 debug/release
in8.289/36.157s, zero ignored, with614 frozen inputs conserved, including
inherited workspace context. Both independent reviews pass. Evidence:
worker-effects/current-runtime-adaptation.json and
current-adapted-bombay-{metadata,native-sources,native-results,REVIEW}.json.
This is214d5b0 plus five explicit adapters, not unchanged214d5b0, registry-only
owners, a minimum-compiler replay or full-runtime gate. The initial setup's
no-runtime-edit wording and final driver's exact-current shorthand do not widen
this scope; the unadapted E0432 compile remains retained separately.
Original FIFO assertions remain; absolute progress deadlines and nonempty final
Stop checks prevent the two reviewed false positives. Five v3 fixture controls
are independently reviewed; attempts1/2 have no acceptance credit. Their scope
is fixture counterexamples, not runtime-owner inversions.

Retained external contracts pass owner-pinned1.95 debug/release:22 tests per
profile, including both inferred constructors and four new missing/ambiguous
completion denials beside seven existing privacy denials. Both fixture reviews
pass. Final Nix a25afb7 reports ten successes/exit0 in189.626s with823 unchanged
hashes,979 nextest passes/zero skips and117 doctest passes. Its clippy classifier
crashes; prior244.809s run also failed the rendered link and a crane-utils audit.
The link is fixed, while original tooling failures remain explicit. Separate online
audit passes1.478s against1329 advisories/65 dependencies. Four-workspace
archive preflight passes6.403s. All11 manual exact-head CI jobs in38844092001 pass;
base-feature PR checks do not substitute for main integration's required checks.

Actual packaged owners match218 shipped Rust files and their a25afb7 VCS heads.
All five unchanged native cases pass debug/release in10.160/35.142s, resolving
exactly the three extracted owners. The589-file manifest binds package/runtime
subtrees; inherited runtime workspace manifests are bound separately by the
prior native-profile snapshot and remain equal. Both archive reviews pass.
See package-inputs-v2, packaged-native-{sources,results} and their metadata/logs.
Initial wrong-working-directory and malformed-path setup attempts earn no credit.
No minimum-compiler archive-native, registry pool, latest-main or comparable
runtime-cost claim is made.

Residue checkpoint: no new state, repeated cause, arrival history, false
cardinality, nested authority, semantic boolean or structural caller syntax.
Reuse Actions, SendInput, FIFO/keyed assignment joins, parent-report interpretation
and retirement. Cross-check FIFO/keyed actor laws and normalized atomic-actor
architecture/features/devx/verification. Disposition: pass for this bounded
composition model. Final-source consumers/costs, catalogue and publication
remain open; no release authorization for104 is inferred.

Authorized release103 selected Behavior/Actors0.26.1 and macros0.14.2. Reviewed
c00bd78 fixes three isolated lock versions (25/31/36 neighbors unchanged), passes
four-workspace preflight10.199s, ten native checks320.242s and970 tests/zero skips.
Separate online audit passes3.227s against1329 advisories/65 dependencies; sandbox
yank lookup remains unavailable. All14 exact-head checks pass. Merge13951b8's
verified main CI38840259711 triggers successful normal Release38841205660.
Both0.26.1 crates are available/not yanked; release tags and all206 shipped Rust
files match13951b8. Six original public custody tests pass registry1.95 debug/
release (initial debug resolves; release is locked/offline). The original actual
native failure/retirement journey passes locked current1.99 debug/release against
registry0.26.1 owners/macros0.14.2, with858 conserved inputs and no owner patches.
Both registry reviews pass. Evidence: worker-effects/registry-0261 and
release-0261-published.json. Bombay's delivery branch still selects0.26.0;
current-main adoption and the other owner releases remain separate obligations.

PR375 repair: capture main67a286c in a fresh isolated worktree, leaving the old
620-file run untouched. Merge cfaf86b resolves only the http-body-util lock entry
and export documentation. Incoming Export is byte-exact;107 moved assertions
survive with three existing DevX close checks. Both package graphs retain their
identities, verified owner versions and existing futures-util edge. Pushed214d5b0
is mergeable;13 native replay cases pass both profiles, strict lint/format pass
and two source reviews find no integration veto. All24 old06d6fd2 named checks
report success/exit0 (4820.621s,620 unchanged hashes), but three background
classifier crashes are status-masked. Exact214d5b0 native/GitHub gates remain
running and have their own audit failures; no old-source acceptance transfers.
No PR375 merge or broad remote-product acceptance is inferred.

Darwin audit recovery is a separate current-output reference check, not a clean
original run or permanent crash fix. Eighteen deterministic controls show that
joining alone still masks pipeline failures in an OR-list; checked pipeline plus
complete joins preserves producer/reader failures. Real pinned-shell scans fail
once among eight unchanged-environment trials; eight child-C trials pass, which
does not establish a root cause. Supplementary scans on the exact three06d and
two pool outputs preserve historical TMPDIR search prefixes and frozen inventories,
use owned scratch storage, join every subprocess and reject actual forbidden
wrapped-script controls. Both reviews pass, with no recovery crash. These outputs
contain no recognized ELF/script headers; compressed contents are not expanded.
Artifacts: audit-status-isolated-kdnsw_s8, audit-recovery-06d-rzsozb5a and
audit-recovery-pool-t8_bmjvk. No Nix pin, repository hook or check is weakened.
