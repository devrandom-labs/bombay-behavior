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

Published Behavior/Actors0.29.0 and macros0.16.0 are verified against the actual
registry index/checksums, downloaded archives, clean source55dbac2, tags and
releases. All228 shipped Rust files match the tested candidate. Finite holding,
generic send insertion and caller-name safety are delivered. Current Bombay
mainf09303c8 and the isolated recovery branch select registry Behavior/Actors0.30.0
and macros0.16.0 without Behavior patches. The030 source/registry verification
and full host gate below retain their exact source limits. Packaged runtime
dependencies and complete programme acceptance remain open; older proofs retain
their original version/revision limits.

Owner documentation PR116 merged asf9e5fdf; Bombay PR386 merged as531ded50
on2026-10-18. Their exact heads pass14 and six GitHub checks respectively.
The current owner branch is `codex/retirement-recovery`. Earlier
feature branches remain preserved; already delivered implementations are not
merged back as duplicate machinery. The programme checklist remains open.

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
| DX01 | One Application owns roots, members, scope, HTTP and Entity. Joined/split Node callbacks are actor-only; protected consumers adopt that contract at6c5c18a. Recursive, supervisor, pool and cold journeys execute without composed-type annotations. Current configuration/session/query owners are private; standard foreign-error spelling and rendered extraction signatures are verified atfa25bbd7. | Open: complete selected semantics, full interface acceptance and final gates. [Bombay PR375](https://github.com/devrandom-labs/bombay/pull/375) merged as5e9bcd1 after all six checks passed; programme acceptance remains open. |
| DX02 | One topology declaration derives spaces, occurrences and custody for born/cold/recursive actors, including equal protocols in distinct roles. Missing/wrong hosts, roles and births remain denied. | Open: complete host/caller corpus, canonical catalogue reconciliation and final gates. |
| DX03 | Nominal role owners close the tested recursive retirement proofs. Native owner/topology tests, cancellation custody and static authority/occurrence denials run on supported compilers. Strict compiler-specific diagnostic replay retains source/coverage controls. Mainf09303c8 is integrated with the recorded source conservation. | Open: all owner consumers, remaining cancellation/freshness acceptance and final gates. |
| DX04 | ApplicationOutcome joins work, optional root report, native retirement, notifications and families; consuming completion retains every axis. The owned actor-custody port preserves original causes and caller assertions. Unread commands, accepted/rejected effects, partial interpretation and failed-child/survivor consumers pass both compilers/profiles with intended loss inversions. Frozen030 source0da4c952 passes all28 host Nix checks and flake check; later consuming family, nonempty native source and joint Application cleanup witnesses retain their focused proofs. Mainf09303c8 is integrated with complete incoming schedule conservation and both-compiler/profile checks. Ata2ef6ce5, outcome() borrows the original typed primary cause without matching the outer runtime variant; complete subsequent consuming recovery and all four unavailable alternatives pass both compilers/profiles with the intended omission control. | Open: complete selected consumer/inspection audit, broader comparable costs and final combined-source/package/registry runtime delivery. Independent review, CI and other platforms remain open. Earlier archive/full-gate receipts retain their exact source limits. |
| DX05 | Runner types derive from root/members; current-thread/multithread, original-input recovery and independent receiving witnesses retain caller-local non-Send work. All38 original cancellation/host journeys pass their recorded matrices. Actual706810b8 archives replay the42-case receiving target on both compilers/profiles, including cancellation after work returns while cleanup is pending. | Open: direct non-Send root-actor compile denial with a valid transported-actor control, current combined-source gates/review and runtime registry delivery. The existing non-Send worker-factory denial does not prove the root-actor diagnostic. |
| DX06 | Validated actor/client MailboxCapacity propagates through root, births, cold members and Entity. e629464 rejects native-unsupported sizes before allocation, retaining the original request; boundary/native tests pass both compilers/profiles. Saturation returns originals. Senders drop unused reply storage (176→48 bytes; three constructors each8→0 allocations). Actual-archive FIFO measurements reproduce lower peak heap with capacity2 on both supported compilers, preserving the same work and complete cleanup. | Open: complete resource accounting and comparable package/compile/runtime measurements. Source41812fce passes all25 native gates and both-compiler archive consumers. Report retention adds8 peak heap bytes and no allocations/retained heap in the frozen pool workload on both compilers; broader costs remain open. |
| DX07 | The existing actor/worker expansion remains authoritative; empty birth aliases are delivered. Private send parameters avoid caller parameter/type names, raw spellings and occupied fallbacks. Pre-edit compile failures,19 added pure cuts, two-pass source/oracle reviews and three native failure/retirement journeys retain their recorded bounds. | Open: generic/private/public, renamed/sibling-target and duplicate-role reconciliation; common-owner fixes are published in029/016. |
| DX08 | Published owners preserve selected send lanes through wrappers and static denials. FIFO/keyed construction accepts ordinary named completion/business products. Published029/016 includes generic-product insertion and caller-name safety; owner gates, packaged consumers and complete missing/ambiguous controls pass. | Open: finish downstream retention, catalogue reconciliation and final delivery gates. Earlier native witnesses retain their recorded compiler/profile and source limits. |
| DX09 | ActorExt adds the exact owning FinalizeOnShutdown and EstablishedTerminationMonitor. Missing-method regressions, order/effect/refusal witnesses, archive consumers and recorded native gates pass. | Open: remaining decoration/catalogue audit and final combined-source reconciliation. |
| DX10 | Owning roster/directory/queue/router constructors use Config and explicit recovery. Published029/016 includes startup closures, reply-lane policy and constructor-local inference. All338 library tests pass both compilers/profiles with1013 original assertions;31 resumed caller tests and real examples have source-selected503 proofs. Startupf244566 removes its named plan/alias. | Open: inference repair is published; current/nested consumers select029/016. Audit policy necessity/names and complete costs and packaged runtime compatibility. Breaking source costs remain explicit. |
| DX11 | Inferred fixed/FIFO journeys preserve immediate/async startup, source refusal/panic, prepared prefix, assigned payload and shutdown. Keyed tests preserve affinity, exact rejection, unbind and joined retirement. | Open: dynamic supervision, independent late completion, remaining preparation/policy/failure cases and final gates. |
| DX12 | Published0.26.0 returns complete duplicate-correlation rejection. Actual birth/replacement/replay witnesses retain accepted prefix/suffix and joined children; exact recipients preserve real application/Entity identity after closure. | Open: remaining creation/occurrence acceptance, including transitive/wrapped/rejected births and relational identity evidence. |
| DX13 | Four no-ID timer wrappers and typed request custody are published; matching runtime and caller migration are on the isolated Bombay branch. PR109/6488965 passes11 fresh native gates and14 current CI checks; combined archives run112 consumer tests and16 examples across both compilers/profiles. Default Bombay workspace passes782 outer tests/132 UI fixtures; selected remote426 outer/20 UI per profile and both child-feature harnesses pass on1.99/1.96. Named replacement and memory costs remain explicit below. | Open: remaining acceptance cuts and coordinated runtime archives. Final registry029 native Bombay/protected gates pass; they do not certify the complete programme. |
| DX14 | Published030 includes finite Stash/Machine admission, complete refusals and the user-selected retained-progress law. PR117/119 and release PR118 pass their exact-head checks; main CI and normal publication succeed. Both registry archives match checksums and production source; tags identify628649d. All11 native owner gates pass with1,051 cases/zero skips; four sequence fuzzers pass10,000 cases each. Source/archive journeys and capacity inversions retain their recorded bounds. Bombay acknowledged the notice and merged registry030 adoption as PR413/main814c96d1; its six Machine cases also pass on the reconciled recovery branch on both compilers/profiles with all original source conserved. | Open: complete catalogue acceptance and packaged runtime delivery. The owner limits count held inputs, independently of outer queues, runtime mailboxes and allocation bytes. |
| DX15 | Native b1c6781 adds actor-owned interpreters through Application/topology using existing guarded startup. Real file/environment requests, wrapper orders, root/descendant/cold/Entity configuration, rejection/panic/cancellation and disposal witnesses pass recorded matrices and inversions. Independent owner diagnostics retain source/coverage controls. | Open: coordinated runtime archives, configured-service costs and complete delivery. Final integrated native runtime/repository gates pass. Earlier child-oracle/work-entry falsifiers and corrected evidence retain their limits. |
| DX16 | Startup/provenance and complete source/interpretation custody remain. PR100 merged as18653a7 supplies OwnedItem from existing settlement metadata; real IO/panic custody, mutations, privacy denials, owner gates and extracted consumers pass. PR101 head184354b fixes rejected reply custody:970 owner tests, ten native Nix gates, four-workspace preflight and locked real-runtime failure/retirement proofs pass. It merged as dda4a42 under explicit authorization; reviewed release PR103 merged as13951b8 and normal publication of Behavior/Actors0.26.1 is verified in both registries and release tags. | Open: complete registry029 catalogue reconciliation, two-template/wrapper composition, native integration and final downstream/cost evidence. Different custody equations remain distinct. |
| DX17 | Shared retirement custody and semantic send access have bounded interface provenance. DirectoryConfig validates immutable limits once. PR110/7644735 removes the duplicate adapter spelling while preserving its implementation; complete native failure/retirement, both route alternatives, static denials, extracted packages and eleven owner Nix gates pass their recorded matrices. | Open: export/consumer/invariant and bounds audit; remove competing spellings only after comparing complete semantics. |
| DX18 | No new production dependencies selected. Existing thiserror, standard owned sums/products and validated limits remain the chosen tools; bounded registry/language comparisons are in the evidence appendix. | Open: error-embedding/source/visibility/bounds equivalence and dependency/MSRV/feature/compile-cost audit. |
| DX19 | Final Bombayeadea00 and corrected41812fce with registry029/016 pass immutable1.99/1.96 replay:136 cases (21 positive/115 negative),32 suites,28 targets,114 outer passes and the exact parent-invoked child. All633 source inputs remain exact;23 reviewed compiler-specific expectations preserve causes/codes. Both compilers independently reject the stale remote underline; the corrected four-case current borrowing suite passes. Caller migration retains255 assertions. | Open: complete minimal/renamed/archive acceptance and programme gates. Final minimum workspace791/136 UI and both remote profiles426/20 UI pass; failed/incomplete runs remain uncredited. |
| DX20 | Application directly establishes root/selected-recipient/Entity clients and senders. Nonwaiting exact admission preserves original Full/Closed commands; origin inversions fail. Shared external customers and pre-root scope retain target authority and original inputs on retry. | Open: complete correlated/cancelled/slow/closed-customer/late-reply and capacity corpus plus packaged delivery. Final-source minimum/native full gates pass. Admission remains distinct from business completion. |
| DX21 | Canonical `application.shutdown()` and advanced lifecycle access retain the original admission Result and inferred ingress. Both owners pass the pre-edit missing-method control and supported-compiler debug/optimized journeys. Root wrapper orders, request replay, natural completion and complete retirement retain their assertions; immutable compiler replay preserves missing/ambiguous authority denials. | Open: complete child-derived phases/drain/deadline/forced-retirement acceptance and packaged delivery. Final native repository gates pass. Admission does not imply joined cleanup. |
| DX22 | HTTP/Entity use the same Application and joined outcomes. Awaitable retire_entity preserves exact-slot completion, cancellation, reactivation and native/callback failures; HTTP removes unnecessary clones and receives ApplicationHandle directly. Original receiving/HTTP/family tests retain their assertions. Public Application consuming recovery now preserves coexisting root-service/family disposal faults in all four fault combinations; entire42-case target passes both compilers/profiles with selective loss inversions. | Open: callback/associated-type distillation and final complete consumer/repository gates. Admitted/nonzero family work retains separate witnesses; zero-incarnation coexistence is not relabelled as that case. |
| DX23 | Zenoh is the sole internal network. Both Node Work callbacks receive only typed actor capabilities; native client/requester test drivers live under Node. Source and59 generated remote declaration pages confirm private configuration/session/query owners. Three public extraction signatures now spell their identical standard boxed error without SDK aliases; existing configuration/query and callback denials pass. | Deferred: networking, durability, identity and deployment products stay with later PRDs. Open: complete interface, typed-extension/outcome acceptance and final delivery gates. This bounded audit does not certify runtime/privacy policy or deferred products. |
| DX24 | Terminal evidence stays authoritative. Root/subtree report inspection survives completed selection, startup refusal, cancellation and a real notification panic without reclassifying actor failure; absent publication stays absent. FIFO rejected-role projection preserves its exact failure/lifecycle product through both orders, inverse and native consumer. | Open: remaining pressure/retry/drain inspection, sensitive-payload policy, bounded diagnostic dimensions/retention and final report delivery gates. |
| DX25 | Protected actor-only consumers conserve original custody; provider and current catalogue retirement matrices have source/effect reviews and independent failure controls. Freshb8 full-gate evidence is separate from earlier runs with masked classifier crashes. | Open: the observed optimized original-work-retry hang remains unexplained. Final native protected gates pass132 outer cases with two expected failed child controls and four zero-match controls; passing retries do not explain the hang or establish compressed-content/permanent Darwin repair. |
| DX26 | Recorded examples remove structural assembly; counter51→36 main lines, pool191→90, supervisor332→96 main plus39 domain lines. Assertions move to owning tests. Frozen counter comparisons retain equal allocation/heap products; sender/client measurements improve their bounded cases. Fifty optimized stash runs account for held/refused originals; the no-overflow control has equal heap costs and16 additional actor bytes. Comparable030 main814c96d1/recovery0da4c952 FIFO consumers now pass120 process controls/pairs on both compilers: identical heap traces, allocation counts and measured value sizes within each compiler/capacity, complete custody retained. | Open: full comparable caller ceremony, compiler/diagnostic/macro costs, binary/metadata diagnostics and remaining runtime paths; configured-service, remote and latency costs remain unproved. The bounded two-actor/one-job memory case is not full DX26. Memory outranks binary size. |
| DX27 | Published030 owners and unchanged macros016 retain exact registry/source/tag and downstream proofs. Mainf09303c8 is reconciled. Clean Native706810b8 joint Engine0.3/macros0.2/runtime0.2 all-feature packaging passes actual1.99/1.96 with identical VCS-bound archives. Actual extracted consumers on both compilers pass eight canonical examples/all-targets, native51 per profile, renamed3, six local example runs and ten doctests. All570 extracted files,533 native-copy source/test files,28 caller Rust files and prepared locks stay exact; only unpublished candidate manifest patches differ by design. | Open: remaining selected acceptance/costs, final combined-source Nix/CI/review/PR and runtime registry delivery. Candidate versions remain unpublished. Earlier release/native/compatibility-inversion receipts keep their original limits; package consumers do not replace complete programme gates. |
| DX28 | Historical pruning is retained. One compiled application guide runs in workspace/extracted packages across recorded configurations; local links resolve. Three partial ignored topology copies are replaced by complete compiled source links. The corrected guide teaches borrowed outcome inspection and complete consuming recovery; both supported current candidate archives execute ten all-feature doctests/zero ignores. This table is the sole DX01–DX28 disposition checklist. | Open: compiled sample/support tables, rendered/external links, final documentation gates and retirement of this PRD/appendix after obligations have owners. Current contract/evidence is compacted. |

### Retained contracts and evidence

This checklist owns dispositions. The following contracts are current; earlier
experiments, pre-edit regressions, complete aggregate measurements, rejected
candidates, reviews and command receipts are preserved in immutable
[45ab42d](https://github.com/devrandom-labs/bombay-behavior/blob/45ab42d/docs/engineering/developer-experience-prd.md)
and the retained executable tests. Historical source-selected, archive and
registry results retain their original source/compiler/profile limits. They do
not become certificates for the current tree. Raw receipts remain under
`/private/tmp/bombay-retirement-schema-kgazqjhv/`.

#### Application composition, custody and capabilities — DX01–06/16/20–22

One Application owns roots, declared/cold children, HTTP and Entity families.
One application-owned topology declaration derives structural spaces,
occurrences, execution products and recursive retirement. Nominal role owners
close native recursive proofs and distinguish equal protocols in different
roles. Actual decorated birth rows remain checked. No placeholder host, empty
alias, erased protocol or second actor algebra is accepted.

ApplicationOutcome retains independent work, native retirement, notifications
and families. Borrowing consumes none; completed-work selection returns the
value and ApplicationRetirement, while every other phase returns the original
whole outcome. Execution performs cleanup. Split receiving preserves actual
publication/join ordering and borrowed or non-Send caller work. Construction,
poll, cancellation and disposal remain distinct custody cuts.

DX05 source audit at Native0c9ef555 confirms the existing receiving tests use
original `Rc` allocations and borrowed work, retry the uninvoked callable after
startup cancellation, and retain the same receiver after cancelling its borrowed
wait. The selected-host test keeps a multithread actor host distinct from its
current-thread caller. The family-cleanup test cancels the real host only after
work has returned and the first family has retired, preserving output, root,
completed family and the unfinished cleanup error. Actual706810b8 archive runs
replay these receiving cases on1.99/1.96 in both profiles; their raw log hashes
were rechecked. The separate blocking-run test returns the original allocation
and both configured builders before worker startup and retries those inputs.
Its execution remains qualified to its own gates, not the archive42-case target.
The worker-factory `Rc` denial proves a different transfer contract; add a direct
root-actor denial and valid counterpart before claiming that diagnostic clause.
Receipt: execution-acceptance-audit.json, including all718 unchanged Native
inputs and counterexample review. This audit changes no runtime or test source.

Application establishes clients and senders for root, selected recipients and
Entity targets. Receive authority remains affine; send-only customers allocate
no reply mailbox. Exact admission returns original Full/Closed commands and is
distinct from business completion. Exact recipients retain their application
identity after closure; sharing a protocol does not grant cross-application
routing authority. Message-only interfaces acquire no lifecycle authority.

The user-selected application operation is `application.shutdown()`, on both
ApplicationHandle and ApplicationLifecycle. It retains the original synchronous
`Result<(), ShutdownRejection>`, borrowed receiver, static root authority and
inferred ingress. `termination()` and `retirement().await` retain their separate
meanings. InstalledActor keeps its lower-level operation; there is no alias.
Pre-edit missing-method controls, both wrapper orders, replay, natural completion
and complete native retirement run on1.99/1.96 debug/release. Admission never
claims joined cleanup.

MailboxCapacity validates before native allocation and retains the exact rejected
minimum. The selected Communication0.1.6 rounds to a power of two below2^31;
Bombay therefore accepts minima through2^30. This is a runtime-owner limit, not
an available-memory guarantee. Root, births, cold members, Entity and external
clients share the same validated product. Pool backlog, worker startup and
retained terminal storage remain separately owned resources.

Entity retire_entity proves joined slot release/mode, not an error-free native
or callback result. Cancelling the observer does not cancel its directory task;
missing publication cannot fabricate completion. Coexisting root/family disposal
faults have the four-combination witness recorded under DX22; admitted family
work has separate witnesses. Callback/associated-type distillation and final
consumer/delivery acceptance remain open.

#### Effects, policies and worker startup — DX07–12/15–18

Behavior remains the pure transition owner; Actions carries named sends, fresh
creations and next behavior/termination. The facade delegates to its owning
macro. ActorExt decorates the same actor; standalone policies use owning
constructors. Conditional SendProduct insertion reuses Own/Inside and SendInput:
unique lanes infer; missing, duplicate and aliased lanes remain E0277/E0283.
Private generated parameters avoid caller names, raw spellings and occupied
fallbacks without exporting more authority.

Owning modules are supervisor/roster, supervisor/directory, pool/queue,
pool/router, proxy and worker. Config groups simultaneous explicit policies
into existing storage; recovery owns real strategy/limit/delay choices.
Temporary recovery needs no replacement source; immediate workers need no
unused startup plan. Constructor-local borrowed-role inference is published;
its extra public generic and mutable refused-factory reoffer are acknowledged
source breaks. No universal builder or hidden policy default is selected.

Construction preserves the original factory, prepared prefix, refused role,
reason, untouched suffix, selector and diagnostic route. Dynamic construction
is infallible. Exact reply capabilities need no logical host; logical/mixed
capabilities retain their actual hosting proof. Initial source preparation,
startup permit and committed readiness remain distinct. Initialization settles
before the permit. A startup notice alone proves neither committed Ready nor
a one-query readiness promise.

Actor-owned native services reuse existing authorization, requests and
interpreter composition. Absence grants no external IO, including a request
whose interpreter is unit; explicit `|| ()` is a meaningful positive. Fn+Sync
constructors promise returned actor-owned instances, not immutable captures or
exclusive backing resources. Production grant/permit and acquired-result
custody remain authoritative; a test-only panic catcher proves neither native
cancellation nor arbitrary disposal safety.

MessageAdapter::new(destination, map) is the sole protocol-hop constructor for
logical, established and mixed destinations. Its mapper/destination, empty
initialization, NoBirths and Continue law remain unchanged. Rejection retains
the mapped message and destination, while unread originals remain in retirement;
a mapper cannot reconstruct the input it consumed. Explicit logical types use
`MessageAdapter<Input, Recipient<Protocol>>`.

Fresh allocation is the actor-model law; creator-local correlation and rejection
before allocation are derived/Bombay policies. Duplicate correlation returns
the original RoutedCreation and typed reason without overwrite. Accepted prefix,
untouched suffix, later siblings and joined descendants survive. Initial birth,
ordinary birth and replacement provenance stay explicit. A request or restart
decision is never a committed birth/restart diagnostic.

#### Stash and Machine custody — DX04/06/14/17

Stage1 owned-retirement investigation,2026-11-11, begins against Bombay2b7acf12,
selecting registry029/016 without Behavior patches; its separate030 adoption
remains with Bombay Codex in the existing step order. The source already owns
one ActorCustody product in20 of24 ActorRetirement alternatives. Two real native
controls pass current1.99 debug/release: completed work beside exact overflow
failure and actual shutdown returning the whole non-Clone/non-Default wrapper.
All payload allocations, report, remaining queues, cleanup and notification
facts are checked. The six consuming regressions fail before production for the
missing method/outcome, then pass1.96/1.99 debug/release. The focused native port
factors20 custody-bearing cases into Retained plus RetirementOutcome while
keeping all24 inhabited alternatives, exact original custody, failures and
progress. Two source inversions fail the intended case in both profiles; exact
restoration passes. The owning native test also passes both profiles, and
strict library Clippy passes. Production is +203/-167/net36 with one new public
type; all26 custody constructions and20 outcome payloads are conserved. The
paired Bombay PRD owns the complete pre-edit law, selected revisions, two test
reviews and measured checkpoint. The separately measured107-file/357-product
caller migration preserves original bindings, causes and assertions. Corrected
native source361d7728 now passes all28 host Nix checks and the authoritative
flake check, including both remote profiles, actual minimum1.96, protected
Counter and unchanged coverage/law gates. Exact manifests and logs remain in
the restart handoff; CI and other platforms are unclaimed. Nonempty consuming
progress/descendant/family evidence, costs and delivery remain open; the focused
port and local gate do not complete DX04. The additional test-only witness exercises
actual unread-command ownership through that existing consuming port, with its
law and falsifier recorded before editing in the paired native PRD. That witness
now passes with the six originals on1.96/1.99 debug/release; strict Clippy and
formatting pass. A lane-dropping source inversion fails only the new case in
both profiles, and exact restoration passes. Complete original six-case source
is conserved. Production/API +0/-0; tests +102/-2/net100. The full28-check receipt
stays tied to361d7728, before this additional test/record batch.
The existing real-file rejection case also consumes the port on1.96/1.99 in
both profiles: the usable accepted File, exact refused request/path allocation
and actual OS error coexist with complete actor custody. Settlement erasure
fails that case while all seven Counter controls pass; exact restoration and
strict Clippy/formatting pass. All original assertions/fixtures are conserved;
test +15/-14/net1, production/API +0/-0.

Two further existing consumers now use the same consuming port. Guarded native
interpreter panic preserves its actual panic carrier, acquired usable File,
pending request/provider and both untouched suffix allocations. All four service
cases pass1.96/1.99 debug/release. Interpretation erasure fails only that case;
three service and seven Counter controls pass each profile. Exact restoration
passes; all original source is conserved after reversing the pattern/iterator
edits. Tests +11/-11/net0, production/API +0/-0.

The failed-child journey consumes the parent and surviving sibling's complete
custody. Original failed creation/state, exact panic carrier, child identity/
provenance, report and payload lifetime stay checked. The joined root report
retains Established/FailuresFound beside its own Completed primary outcome.
Separate descendant and child-failure erasures fail their exact assertions in
both profiles; three terminal and seven Counter controls pass. Exact restoration,
both-compiler/profile positives, strict lint/format and complete original-source
conservation pass. Tests +53/-52/net1, production/API +0/-0. The native PRD owns
both pre-edit laws, full checkpoints, two author reviews and failed-command limits.

Main030 reconciliation preserves the029 branch at6cb916ac, integrates peer PR413
and reads exact628649d instructions/contracts. The sole backlog conflict keeps
the current upstream remote row and scoped DX row. Two fresh Machine patterns
receive only the proven representation/import migration: all579 original source
lines, fields, assertions and literals are conserved. Current1.99 passes23 outer
cases per profile, including both original interpreter denials; actual minimum
1.96 passes22 (its compiler harness is filtered). Both pass11 Driver inventory
checks; strict five-target Clippy, workspace and protected formatting pass.
Registry checksums/provenance remain exact. Native117/paired142 source paths,
Rust +2501/-1482/net1019, production net36, tests net983, public types +1/-0.
Frozen native0da4c952 passes all28 host Nix checks and authoritative flake
check. Exact roots, output hashes/derivers and all four unchanged coverage
floors verify; workspace execution reports990 passes/123 summaries, zero failed
summaries and six unchanged controlled child registrations. Actual minimum1.96
reports114 passes/136 compiler fixtures. This certifies that immutable source,
not subsequent edits or other platforms. Independent review, CI and feature
delivery remain open.

The following test-only family witness adds45 lines to two existing traces.
They consume every original family result and root custody after their borrowed
snapshots, preserving exact Work, roles, original panic carrier, acquired
prefix/tail, joined live account, nonzero metrics and final lifetime oracles.
All four cases pass actual1.99/1.96 debug/release, including restored runs;
strict current target lint/format pass. Erasing only the original disposal
failure in FamilyRetirement::into_parts kills both fault cases in both current
profiles, with both healthy controls passing. Production restores exactly;
removing only the bridges reproduces the original entire test byte-for-byte.
Two author reviews pass; no independent review is claimed. Existing consuming
ports suffice: no production/API/state/trait/wrapper or registration is added.
The native PRD owns the pre-edit law, complete pass checkpoint, corrected
evidence-locator failure and source-qualified receipts. Cumulative Rust is
+2546/-1482/net1064; production net36, tests net1028, public types +1/-0,
unchanged117 native/142 paired paths. Nonempty source progress, coexisting
root/family cleanup failures, comparable costs and whole programme acceptance
remain open. No new release or release notice belongs to this test stage.

Nativee0935e07 adds one inline test in an existing source interpreter module.
After actual timer receipt admission, a rejected observer causes a real native
offer panic. Consuming custody preserves the primary panic identity, source
index0, nonempty Completed(Admitted) remainder, unfolded state and distinct
admitted id1/gen1/untouched id2/gen1 receipts, with independent successful
notification and address retirement. New case/original cancellation control
pass actual1.99/1.96 debug/release, including exact restored runs. Source erasure
only in the HostExecutionPanicked projection fails the intended assertion in
both current profiles; cancellation and all seven Counter controls stay healthy.
Strict all-tests Clippy/format pass. Three new empty assertions needed equivalent
length0 syntax for lint; initial failed lint remains failed. Original whole
launch.rs is byte-identical after removing only the test/import. Two author
reviews and the full D-SETTLE-1/D-RETIRE-1/D-TERM-1 checkpoint pass; no independent
review is claimed. Tests +160/-0, production/API +0/-0, no new fixture or state.
Cumulative Rust110 paths +2706/-1482/net1224; production net36, tests net1188,
public types +1/-0; native117/paired142 paths unchanged. This witnesses one native
source phase, not every source phase or the Application callback path. Its
source-qualified receipts and remaining obligations are in the native PRD;
the full28/flake receipt still belongs to earlier0da4c952.

The next test-only Application witness consumes completed non-Send Work, native
root custody and independent family retirement in all four fault combinations.
It uses a real actor-owned service disposal failure and a separate definition
disposal failure: the shared address-space destructor cannot establish an
independent root fault. Both original boxed causes, Work/state/role allocations,
complete retirement fields, notifications and actual empty family shutdown/
metrics survive until explicit final discharge. Family faults do not change
the earlier root report. All42 owner cases pass actual1.99/1.96 debug/release
before and after selective inversions, strict target lint/format pass. Root
omission fails exactly two root-fault cases; family extraction omission fails
its two new cases and two existing consuming fault cases. All seven Counter
controls remain healthy in each profile for both mutations. Production restores
exactly; the entire original test source is conserved after reversing only the
added fixtures/trace/imports and pinned import formatting. Two author reviews
and the full unchanged-control-state checkpoint pass; no independent review.
Tests +352/-2/net350, three private fixtures/four registrations, production/API0.
Cumulative Rust110 +3058/-1484/net1574, production net36, tests net1538,
public types +1/-0; unchanged117 native/142 paired paths. This is a bounded
zero-incarnation family composition witness; nonzero admitted-family evidence
stays separate. Exact source-qualified receipts and remaining obligations are
in the native PRD and retirement-recovery-joint-* handoff evidence. Full28/flake
credit remains bound to0da4c952; final programme delivery/cost gates remain open.

Nativefa25bbd7 changes six zenoh::Error spellings to the identical standard
boxed Error/Send/Sync type, with one module import and no producer/consumer body,
test, expected diagnostic, visibility, lifetime or type-identity change. Selected
zenoh-result1.10.1's enabled std feature proves equality; no new conversion,
reboxing or erasure is introduced. Frozen717-input focused Nix checks pass:
current1.99 remote debug/release each524 outer cases/12 summaries/31 UI fixtures,
actual1.96 minimum remote3 outer cases/11 UI fixtures, strict all-target remote
Clippy and remote/workspace docs. All four output derivers/NAR hashes are rooted.
All three generated extraction signatures expand exactly their former unlinked
Error alias and link standard traits;59 remote declaration pages and relevant
private owners are inspected. The initial SDK-hyperlink assumption is rejected,
not counted as a failing regression. Two author reviews and unchanged-state
checkpoint pass; no independent review. Production +7/-6/net1, tests/API0;
cumulative native120/paired145 paths, Rust113 +3065/-1490/net1575, production
net37/tests net1538/public types +1/-0. Exact frozen source, conservation,
rendered signatures and gate receipts are sdk-privacy-* in the restart handoff.
Four focused checks do not replace the earlier0da4c952 full28 gate or final
programme/CI/delivery acceptance.

Mainf09303c8/PR414's incoming schedule campaign is merged into the recovery
branch as0e4851b2, preserving every upstream byte outside the previously proved
retirement pattern/import and the independent DX1 status row. Frozen718 inputs
pass current remote debug/release each528 outer cases/31 UI fixtures, strict
remote Clippy/docs and actual1.96 incoming five-case debug/release. Minimum
profiles each execute180 finite orders and256 generated successes plus one
persisted replay, with recorded seed/configuration. Two exact Nix outputs are
rooted; source/seed conservation and two author reviews pass. PR414's eight
hosted checks pass on its own51d8909a head, not this feature source. The original
28-check gate is not relabelled. Native8ccbc102 records this integration.

Nativea2ef6ce5 adds only the immutable ActorRetirement::outcome() view required
by DX04. Pre-edit inferred Counter syntax fails only E0599; original production
remains exact until that witness. The new getter borrows the existing primary
outcome beside custody; presence means availability and does not certify success.
Counter command and real-file rejection pointers survive inspection and subsequent
complete consuming recovery; normal wrapped completion is available, and all
four unavailable alternatives retain absence and exact original owned return.
Actual1.99/1.96 debug/release each pass nine existing owner cases: four live
Application journeys, one real executor, three pure unavailable cases and one
compile harness with two unchanged denials. Accessor-only absence inversion
fails three intended observations and preserves six controls in all four runs.
The initial classifier's five-control assumption and earlier eight-case reports
are corrected from original source/positive logs; no Rust oracle changes.
New-observer style corrections preserve the same pointer/byte expectations.
Production restores exactly; final reruns, strict lint/format/docs and rendered
const signature pass. Two author reviews, no independence. The five outer/20
retained/24 logical alternatives,14 custody fields and every runtime transition
stay unchanged; one public observation method/match, no type/bound/storage change.
Production +16/-0, tests +43/-0; all original bytes/oracles conserved. Cumulative
native120/paired145 paths, Rust113 +3124/-1490/net1634, production net53/tests
net1581/public types +1/-0. Native PRD and borrowed-outcome-* own exact receipts.
Final combined-source/CI/package delivery and the complete programme remain open.

Native706810b8 corrects the guide's stale outer Completed migration to the exact
borrowed outcome/owned custody pair and removes its redundant text-only Work
sketch. Compiled doctests and original assertions stay unchanged. Both actual
compilers verify joint all-feature Engine0.3.0/macros0.2.0/runtime0.2.0 archives;
all three sets are byte-identical with VCS706810b8. Archive inventories are28/7/
535 files, including3/2/108 src files (source-local tests included). The runtime
guide and all source match the frozen718 inputs. An earlier current-only e41da292
preview keeps its pre-guide qualification; no archive is a published release.

Actual archive consumers on1.99/1.96 each compile all eight canonical examples/
all-targets, pass native51 per profile (Application42/interpreter2/retirement7),
renamed3, six terminating local examples and ten all-feature doctests/zero ignores.
Root registry030/016 selection, all570 extracted files,533 native-copy source/
test/diagnostic/readme files,28 caller Rust files and prepared locks stay exact.
Only candidate path/version adapters and two unpublished Engine/macros manifest
patches support the isolated setup. The initial CLI-only patch failure is kept:
trybuild child Cargo lacked those unpublished versions. Existing manifest patch
propagation fixes setup without Rust/fixture/expected-diagnostic changes. The
minimum all-target check retains the original remote controller's unfulfilled
dead_code expectation warning; no warning-free minimum claim or suppression.
HTTP/remote examples compile only in this corpus. Two author reviews and
unchanged-model checkpoint pass; no independent review. Native96c249e2/0c9ef555
record exact results/current backlog. Source scope native121/paired146 paths,
same Rust113 +3124/-1490/net1634, production net53/tests1581/public types+1.
The native PRD and runtime-archive-*/runtime-consumer-* receipts retain exact
commands/inputs and limits. Final full-source Nix/CI/PR/registry runtime delivery,
broader acceptance/costs and whole DX01–DX28 remain open.

The isolated recovery cost comparison uses actual030 source main814c96d1 and
recovery0da4c952 with identical locked graphs, unchanged domain worker/work,
all18 borrowed root/worker field equations and complete owner retention through
join. Latest production source agrees; its later cfg(test) addition has exact
source conservation. Initial baseline compilation rejected an incorrect flat
custody pattern; source-derived existing ActorCustody nesting fixes only that
adapter, and the failed inputs/logs remain uncredited. Both supported compiler/
allocator/capacity functional matrices then pass. Wrong allocator and business
controls fail their exact0/4096 and3/4 assertions; known32MiB resident allocation
raises measured child RSS by33570816 bytes. Two author reviews pass.

Forty fresh controls and80 alternating paired processes pass all work/custody/
layout/hash oracles with no competing build workers in their snapshots. Both
sources have identical complete heap traces per compiler/capacity: current peak
161650/415106 bytes, minimum161874/415330 at effective2/1024; joined28656/current
or28880/minimum, after host56 bytes in one existing Tokio parker,176 total
allocations. Sizes also match: execution8936/current or9048/minimum; outcome3224,
application retirement2936, native2840, worker712. Page-scale RSS/footprint
variation and short-work duration outliers establish no general improvement or
latency result. Exact inputs, raw120 rows/CSV, controls, source/graph/binary proof,
reviews and limits are retirement-costs/* in the existing handoff and the native
PRD. This bounded healthy FIFO comparison changes no runtime/API/test source;
clean/incremental compile, binary/metadata, macro, other runtime/caller and final
programme delivery acceptance remain open. No independent review is claimed.

The matching compiler study passes220 checks on actual1.99/1.96: five clean
targets,25 unchanged noise controls and25 real business-input edits per source/
compiler. Cargo JSON verifies genuine rebuilds and restored source/lock hashes.
Current clean medians7.71→7.39s and minimum7.81→7.67s overlap sampled ranges;
edited medians0.20→0.19s/current or0.20→0.20/minimum, unchanged0.04s both.
CPU/RSS ranges and short-command precision establish no general speedup.
Snapshots show no unrelated build worker; coverage is before/during checks and
after campaign, not an immediate post frame for each short command. Both author
reviews retain those limits. All20 completed task-only targets are removed after
preserving raw evidence/metadata (2255316KiB); no global GC. Native metadata and
validated release binary sizes are secondary diagnostics with source-path
confounders; zero-byte consumer check metadata earns no size claim. The native
PRD and retirement-costs/check-*/artifact-sizes records own exact commands,
220 rows/CSV, sources/artifacts/toolchains, review and pass checkpoint. No code/
API change or independent review. Wrapper/child additions, macro expansion,
full-workspace/latest-test costs and remaining DX26/delivery stay open.

Canonical finite holding is `actor.with_stash(8, refused, route)`. The limit counts
held logical inputs, not allocation bytes, Machine deferrals, native mailboxes
or outgoing effects. Zero is valid. Only a newly hold-selected input at capacity
is refused: its complete original User/sender enters the typed refusal lane and
the actor continues. Deliver, Release, non-user events, initialization and Stop
retain their prior laws. StashSends interprets actor effects before refusals.
Only the repeatable consuming refusal route needs Clone; input/actor do not.
Closed phase and statically uninhabited inner error remain required.

`stash.take()` transfers FIFO original Users, leaves the stash empty and preserves
its inner actor and route. It performs no replay, transition, effect or cloning;
a second take is empty. Explicit UnlimitedStash retains deliberately unbounded
Never callers without an irrelevant refusal capability. Mechanical Bombay
migration preserves23 references and all255 original assertions.

Replay conservation is: pre-held originals = delegated inputs + reheld prefix
+ unread suffix. Selection borrows before removal. Selector unwind retains the
current input and unread tail; inner unwind retains only still-undelegated
originals. The private custody product reunites unread originals after the
reheld prefix on destruction; it owns no transition authority. Neither rollback
of inner state nor recovery of consumed inputs/unreturned Actions is promised.
Abort, allocator/destructor failure and arbitrary actor rollback are excluded.

Independent ranked/property/fuzz and original-source controls cover refusal loss,
extra capacity, reversed FIFO, changed sender, selector/actor unwind and both
shutdown/timeout orders. Real native journeys preserve held and unread originals,
exact senders/allocations, retirement, cleanup and notifications. The first
finite lowering was removed when an outer-timeout consumer rejected inferred
send insertion; the generic owner repair preceded catalogue migration.

Machine's Clone-based rollback hypothesis was rejected: shared cloned state
disproves rollback, so Clone cannot establish the claimed transactional law.
The explicit replacement below preserves original inputs and actual progress.
Static phase restrictions remain; finite Stash does not bound Machine retention.

Machine rejection decision (user-selected2026-11-11): retain completed work and
the exact refused/unprocessed inputs. This deliberately replaces whole-turn
rollback; it is Bombay policy, not an Agha guarantee. The unchanged saved native
consumer at Bombay9f542c18 reproduces one passing value-state control and the
shared-state rollback failure (balance7 rather than0). Rust's
[Clone contract](https://doc.rust-lang.org/std/clone/trait.Clone.html) explicitly
permits shared underlying storage; it cannot establish independent snapshots.

Before production edits: keep `Machine::new(state, phase, on)` and its existing
borrowed-message callback. Successful earlier decisions and actual state
mutations remain; on refusal `MachineError.event` owns the actual refused User,
not the earlier accepted drain trigger. For held inputs, the ownership equation
is originals = processed inputs + refused input + reheld prefix + unread suffix.
Only a genuine phase change retries deferred inputs; Stop ends that drain.
`machine.take()` transfers the remaining original Users, including senders, once.
No state/message Clone bound is needed to execute. Callback unwind retains held
inputs still borrowed by the callback and the unread suffix, without promising
state rollback or recovery of an incoming value consumed by an unwinding call.
Retention limits remain a separate unimplemented law; this batch does not bound
the queue or add an overflow policy.

The focused design stage changes only Machine, its tests, this contract and the
existing private held-input custody owner shared with Stash. Reuse Move's four
decisions, MachineError's event/cause and Stash's dispatch-free reunion on Drop;
delete cloned staging and Advance's three private alternatives. Expected scope:
six repository paths, at most100 net production lines, zero new public types,
one recovery method. No catalogue/testkit/example migration in this stage.
Held Users now own their addresses, so Machine's address-only PhantomData is
redundant and removed. The existing conditional explicit Clone implementation
remains ordinary duplication; execution no longer uses it as a transaction.
Pre-edit regressions cover the accepted prefix, exact refused sender/payload,
unread/reheld originals, Stop, unwind, non-Clone state/messages and real timeout/
shutdown policies in both orders. Native proof uses an isolated actual
Application consumer before any migration. The aggregate remains phase × state
× held FIFO with one transition authority; the shared custody product owns only
the reheld prefix and unread suffix needed for exact reunion. Complete measured
checkpoint and both oracle reviews are required before retention.

The design stage passes eight pure tests, including both timeout/shutdown orders,
on pinned1.95 in debug/release. Seventeen existing Stash/error-source controls
also pass both profiles. A real Application at Bombay9f542c18 passes on pinned
1.99 in both profiles, retaining the accepted deposit, refused allocation/sender,
unread allocation, failed-root report, notifications and cleanup. Removing only
the shared unread-input reunion fails four pure cases and that same native
journey in both profiles; exact source restoration returns every case to green.
The original two law tests failed on old source, before non-Clone inputs were
added; the latter independently expose the old Clone bounds. Both oracle review
passes check full Actions, allocation identity, real sender identity, nonvacuous
timer Stop, one-time recovery and unwind. No independent-agent review is claimed.

Aggregate checkpoint: phase × state × held FIFO remains the complete control
product, with user-defined phase cardinality unchanged. Move4→4; private
Advance3→0; MachineError's event/cause product and two Result alternatives stay.
The existing shared custody product retains exactly the reheld prefix/unread
suffix required for reunion; it has no callback or transition authority.
Machine match arms15→10. Affected production files627→644 lines,
production+89/-72/net17; Actors source modules136→137. Public types3→3,
public spellings13→14 (only `take`); focused tests+422/-0. Residue review finds
no arrival history, repeated cause, cardinality assumption, semantic boolean,
nested transition authority or new structural caller syntax. Synthetic timer
events in component tests name their existing interpreter-owned sum; the native
caller uses ordinary Application construction. Cross-checked transition algebra,
layer laws, Stash custody and all normalized atomic laws (no Machine clauses).
Disposition: pass for this design stage; old rollback-oracle migration, full
gates, release compatibility, bounds and broader DX14 acceptance remain open.
Receipts and the two oracle reviews: saved handoff
`restart-2026-11-11/machine-*`.

Mechanical adoption follows proven design9fb1ea4. The unchanged error-path
suite now fails exactly its three rollback oracles; their replacement follows
the user's retained-progress decision, not a green-build requirement. Update
only those expected failed inputs, actual state and remaining counts; retain
the two unrelated driver tests. The existing error-sequence fuzzer must compare
the actual refused input, and use input bytes to exercise deferred replay rather
than always opening on occurrence zero. Reuse its numeric protocol; no new
fixture type or production bound. Update the explicit-clone benchmark's obsolete
rollback description, the owning guide and Actors changelog. Expected cumulative
scope11 repository paths; production remains+89/-72/net17 and public types+0/-0.

Adoption review found an implementation regression before accepting that stage:
equal phase values can carry different observable fields. Goto must always
store its exact next value; equality determines replay only. Two independent
direct/replay probes pass on registry0.29 and fail on9fb1ea4 at revision1 versus2.
This repairs the existing designation law, without a new phase type, bound or
decision policy. Add those two owner regressions before replacing the stored
phase and comparing the previous value. Earlier1,043-test/11-Nix-check success
does not cover this counterexample. Repeat affected verification on the repair;
the model and cumulative11-path scope remain unchanged.

Both repaired designation cases and the complete native custody journey now
pass in debug/release; ten owner tests pass both profiles. Updated error-path,
FSM-property and two-buffer suites pass12 cases per profile. The range-based
fuzz oracle passes10,000 byte-driven cases (maximum256 bytes), including actual
deferred failures; its generated corpus is preserved outside the repository.
The existing Nix fuzz shell supplies the linker environment; the first package
runner attempt failed before fuzzing at missing libiconv and remains uncredited.
No setup or dependency change was made. The two added designation tests inspect
the carried revision directly, so PartialEq cannot conceal the rejected value.
Final checkpoint retains the same control/product, alternatives, match arms,
module count and public spellings; production is now+90/-72/net18, owner tests
+484/-0. The stored previous phase is a temporary comparison value, not retained
arrival history. The frozen repair passes1,045 workspace nextest cases/zero
skips and all11 native Nix checks. Its real Application journey and two facade
designation cases also pass debug/release on both1.99 and1.96 at current
Bombay937fecc3. Final source/oracle review preserves all original driver tests,
the three explicitly changed Machine error laws, phase fields and held custody.
Only this receipt/checklist text changes after those full checks; the final
documentation gate covers that update separately. Reviewed merge and package
delivery remain open. Retained inputs now store their senders as well as
messages; concrete layout and workload costs remain part of resource acceptance.
No whole-workload allocation or memory reduction is claimed from removing
execution clones.
The extracted Behavior/Actors/Macros archives also pass the real Application
custody journey and two pure designation cases on1.99/1.96, debug/release,
with patched Tokio1.53.3. Shipped Rust sources compare byte-for-byte with the
owner source. This establishes archive compatibility, not registry publication.

A bounded resource comparison reuses the shipped Processor domain source,
changing only its facade import. Identical harness sources and dependency
versions compare registry0.29 with the new Actors archive:64 processors each
defer one Work, then Open and prove that exact work completed. Five independent
optimized processes per selection/compiler give identical counts on1.99/1.96.
Actor layout remains72 bytes and completion live heap remains6656 bytes.
Allocations fall193→129, while total allocated bytes rise11776→12800, held
heap8704→10752 and peak heap8752→10784. Disposal leaves zero live allocations
and bytes. This pure sample uses MailAddr and includes the common actor vector;
it does not measure native mailboxes, scheduling, large payloads or latency.
Preserving senders has a measured cost; fewer clones do not prove less memory.
Sources, locks, import-only diff and raw twenty-process receipts are under
`restart-2026-11-11/processor-memory-*`; no source optimization follows from
this single workload. Broader resource acceptance remains open.

PR117 CI exposes a stale dependency prerequisite: Advisories and Licenses
run43017441450 rejects Tokio1.53.1 for
[RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html).
Before dependency edits: select the first patched compatible1.53.3 in the root,
interpreter-contract and fuzz locks; refresh only the existing advisory-db Nix
input so local audit sees the same published advisory. Existing manifest ranges
already permit this patch. No AsyncFd call exists in these owners, and no new
unsafe call, dependency, feature, API or actor-policy change is proposed.
Expected cumulative scope15 repository paths, unchanged production+90/-72/net18
and public types+0/-0. Retain the failed CI receipt and rerun audit, affected
owner/downstream cases and final gates; earlier1.53.1 proofs keep their original
dependency limits. Native fixture manifests/locks must select the patched Tokio
too. The aggregate checkpoint is unchanged because this batch changes only
existing dependency/advisory selections.
The refreshed database also reports rustix1.1.4 under
[RUSTSEC-2026-0486](https://github.com/RustSec/advisory-db/blob/2c684a5d6746d4508eff9e3cb529b3e33f56d438/crates/rustix/RUSTSEC-2026-0486.md);
its first patched version1.1.6 changes the same root lock, not another path.
Upstream package metadata gives Tokio1.53.3 MSRV1.71/MIT and rustix1.1.6
MSRV1.65 with an existing accepted license choice. Both fit pinned1.95.
The existing local audit returns0 while reporting these unsound warnings;
that result does not establish advisory clearance. Its recorded default differs
from CI's rejection policy and must not be described as equivalent.
Before changing the check, the same pinned database and original lock fail
`cargo audit --ignore yanked --deny unsound` with both named advisories;
the original default check had returned0. Use Crane's existing
`cargoAuditExtraArgs` port to reject unsound warnings while preserving the
existing yanked-package policy. No new check or tooling is needed. The user
explicitly approved the cumulative16-file scope, then gave standing approval
for the remaining goal work and necessary scope increases on2026-11-11.
This supersedes the earlier requirement to ask again for each scope increase;
pre-edit laws, bounded ledgers, review and required verification still apply.
The patched dependency source atc91177a passes all11 native Darwin Nix checks,
including1045 nextest cases with zero skips and the strict advisory check.
All three patched locks pass the same strict audit; the original fails for
both advisories. Patched native and archive matrices pass on both compilers
in both profiles, and the independent fuzz oracle passes another10000 cases.
Only this receipt text changes after the frozen full gate; its documentation
gate is checked separately. DraftPR117 carries the retained implementation;
exact-head CI and merge/publication remain delivery obligations.

Finite Machine deferral hypothesis, before production edits2026-11-11:
`Machine::bounded(state, phase, on, InputLimit::new(slots, refused))` starts an
empty finite queue. Slots count original deferred Users, not bytes, runtime
mailboxes or an outer Stash. Zero is valid. After the existing callback selects
Defer, a new original is either held within the limit or emitted intact to the
typed refusal recipient; the actor continues. Actual callback mutations stay,
as with the user-selected retained-progress law. The limit is not a transaction
around the callback. Stay, Goto, Stop and callback errors retain their laws even
at capacity. Replaying an already held input consumes no new slot. Its reheld
prefix plus unread suffix never exceeds the original admitted count. Take moves
remaining originals once, without changing the limit or recipient.

This is deliberate Bombay policy, reusing finite Stash's existing refusal
equation; it is not an Agha resource guarantee. Overflow is a normal typed send,
not actor failure. MachineError remains the exact callback refusal, so its
Never specialization stays statically infallible and lawful under Stash.
Before editing, review Agha et al.1997's communication/interface semantics,
the transition algebra, layer laws, existing logical/exact delivery interpreters
and normalized atomic laws. No normalized atomic document adds a Machine law.
No new runtime request, receipt, lifecycle outcome or interpretation port is
needed: logical and exact refusals use the same DeliveryRoute products as Stash.

Predeclared representation: one Machine authority still owns state × phase ×
original FIFO. Add one fixed retention configuration, with two real static
policies: unit retains without a bound; InputLimit owns slots and its refusal
capability. A sealed InputRetention law admits the complete original through
the authoritative queue and returns its concrete sends. These are the only two
new public contracts. Machine's additional policy parameter defaults to unit
to preserve existing valid five-parameter type spellings and three-argument
construction, not to repair an invalid caller. No caller supplies a unit marker,
empty callback, dummy recipient or placeholder. Finite construction requires
the real refusal capability; state/messages require no Clone. Only repeatable
delivery authority needs Clone, as in Stash. Reuse this same InputLimit in
finite Stash, deleting its separate limit/refusal fields and duplicate admission
branch rather than retaining two bounded-input algorithms. Keep existing Stash
construction and replay restrictions unchanged.

Before production, write the focused zero/full/release/custody regressions in
domain vocabulary, then capture their failure on6f90401's Machine API through
pinned Nix. Prove original sender/allocation/content, exact capacity, subsequent
nondeferred work, callback mutation, stopped/unread custody, and infallible
Stash composition. Two real templates, Stash and Machine, must use the shared
admission law and both shutdown/timeout orders must retain its refusal effects
before catalogue adoption. The smallest real Application witness uses the
existing pre-root scope and an exact external refusal endpoint. Wrong protocol
and forged policy denials must fail at construction. No broad example, testkit
or facade migration belongs in this design stage.

Pre-edit ledger: Machine, Actors exports, one shared retention module, Stash's
two existing owners, focused tests and this PRD, plus the separately requested
AGENTS release-coordination instruction. Expected Rust production at most120
net lines and two new public contracts. Across the resumed task this adds three
previously untouched paths to the prior16: AGENTS, retention and focused tests.
Standing user approval covers this19-path scope. Baseline affected production
is796 lines across Machine, Stash limit/queue, shared held custody and exports;
Actors has137 source modules. Move has4 alternatives, Machine has10 match arms,
and MachineError remains event × cause. The complete before/after checkpoint,
all surviving subordinate values, residue scan, source/oracle reviews and
defect inversions are required before retention. Any compiler diagnostic that
requires another unnamed policy, type, bound or route reopens this hypothesis.
The first four caller regressions are written before production and fail only
at missing InputLimit (E0432) and four missing bounded constructors (E0599),
exit101 under pinned1.95. State and boxed messages have no Clone implementation.
Receipt: `restart-2026-11-11/machine-limits-before.*`. InputRetention's sends
must satisfy the existing SendsFor<User<A,M>> law at Machine construction and
execution, as required by Behavior::Sends. Stash's finite owner will borrow its
existing child queue within the Stash aggregate via pub(super) visibility;
this grants no application or runtime mutable access and adds no forwarding
object or second queue. These are pre-edit interface decisions, not compiler
fallout. The native logical/exact delivery implementations retain complete
rejected requests, and Engine apply retains ordered independent settlements;
the finite send reuses those current ports unchanged.
The first production candidate compiles without another semantic symbol or
bound. Test compilation exposes two fixture mistakes: ambiguous Box conversion
before pointer inspection, and attempted mutable access through Active.
Use ordinary Vec::into_boxed_slice construction and the existing trusted
initialize/delegate_transition ports for the two pure recovery tests, just as
the retained Machine tests do. Add no DerefMut, escape hatch or caller plumbing.
The configured and replayed application syntax stays unchanged. Source is now
136 net physical Rust lines, including the two static-denial Rustdoc controls,
above the expected120 but below the standing-approved production budget; the
complete checkpoint must assess this surface before retaining it.
Three finite-owner cases pass. The composed oracle initially selects Stash for
Work in every phase, so the existing pure selector law correctly reholds all
three inputs instead of delegating any. Preserve the required two exact refusal
deliveries; make Poll establish the actual Admitting phase used by that selector.
This also exercises bounded replay's re-deferral without consuming another
slot. No production repair or weakened expected refusal count is justified.
All six capacity/order cases,24 existing Machine/Stash cases and63 Actors
doctests now pass. The normal facade witness first used the ActorExt-only
`bombay::actors` namespace incorrectly. Bombay already forwards the existing
`composition` semantic module, whose documented role is to curate these owning
construction contracts. The predeclared InputLimit/InputRetention contracts
must be curated there alongside Machine and Stash; crate-root exports remain
the same types. Record this completeness decision before adding those existing
contract re-exports, then import InputLimit through bombay::composition. Add
no facade wrapper, new constructor, dependency or generic to fix that error.
This adds one existing source path to the ledger: cumulative20 paths, still
two new public contracts under the user's standing approval. No runtime
production file or catalogue caller is migrated in this design stage.
The real Application now passes at Bombay937fecc3 with ordinary facade imports
and an exact pre-root refusal endpoint. Two independently allocated senders
distinguish admitted and refused inputs. Opening completes the admitted prefix;
closing admits new held originals, and Stop at capacity joins healthy retirement
with those exact allocations/senders recoverable once. No owner dependency or
runtime production edit was added to obtain that proof.

Before capacity-defect inversion, the aggregate product and Move4/Error product
remain unchanged; only the constant static retention policy is new. InputLimit
owns exactly slots and the refusal capability needed by later admission. Unit
owns no data and performs real unbounded holding. Shared HeldInputs still owns
only the reheld prefix/unread suffix. There is no new state/result sum or nested
transition authority. Current scope is20 cumulative paths and two public
contracts; affected production includes the existing composition curation,
812→952 physical Rust lines (+160/-20/net140), modules137→138. Machine match
arms remain10; the one capacity predicate moved from Stash into the shared
policy. Canonical public spellings32→38 and types9→11; composition re-exports
the same two new contracts. The new default policy parameter has two real
substitutions and adds no required caller marker. No history, repeated cause,
false cardinality, semantic boolean, runtime lookup or structural caller path
is introduced. Cross-check the transition algebra, layer laws, original Stash
contract and normalized atomic documents before retaining this candidate.
The planned inversion changes only `< slots` to `<= slots`: no types, states,
branches, public names or line counts change. It must falsify zero/full custody
cases and the real Application refusal, while the two-input Stop control stays
valid. Restore byte-exact source and repeat affected proofs before retention.
The inversion fails5/6 Machine cases (Stop control passes),7/9 existing Stash
cases and the exact native consumer in debug/release. Its minimized property
input is zero slots and one held-selected input, already covered by deterministic
cases. Preserve the generated seed outside the repository. Exact source restore
passes the whole candidate fingerprint,30 affected owner cases per profile and
the native journey per profile; minimum1.96's earlier positive remains exact.
The second oracle review moves three ownership extractions outside assertions,
preserving all expected values and assertion counts. No production shape changes
follow compiler feedback or a failing oracle; only fixture construction,
selector semantics and effectful-assertion placement were corrected.

Finite retention checkpoint,2026-11-11: **pass**. The complete control-state
sum is parametric in the caller's phase: before, each p:P owns S × original
FIFO; after, each p:P owns S × original FIFO × fixed R. No phase or transition
authority is added. Move remains4 alternatives, MachineError remains the
original event × cause product with Result's two alternatives, and Machine
match arms remain10. Neither admission nor HeldInputs owns a subordinate
control/result sum. InputLimit's slots and destination are the current values
needed by each later admission; unit owns no data and performs actual holding.
HeldInputs' reheld prefix and unread suffix remain necessary for ordered exact
custody on return/unwind. Stash's duplicate capacity branch and separate
configuration fields are replaced by this same admission law.

The complete retained source delta is production +160/-20/net140, tests
+433/-0/net433, public API +2/-0 types. Affected production is812→952 lines,
Actors source modules137→138, canonical public spellings32→38 and types9→11;
composition curates the same two contracts rather than inventing alternatives.
The cumulative resumed task touches20 paths under standing scope approval.
The residue scan finds no arrival-history state, repeated cause, false
cardinality, nested transition authority, semantic boolean, erased dispatch,
payload clone or structural application syntax. Cross-checked sources are the
transition algebra, behavior-layer laws L1–L8, Stash's existing admission/custody
contract, all five normalized aggregate laws and all four normalized atomic
engineering documents; none adds a conflicting Machine rule.

Two author source/oracle review passes are complete; no independent reviewer
is claimed. The first checks the entire static surface against the pre-edit
law and four failing callers, including sealed policy authority, exact sender
custody, callback-before-admission and unchanged replay. Its oracle review
corrected the fixture's phase selection without reducing the required refusals.
The second audits every new symbol/bound against that provenance and complete
Actions lanes in both wrapper orders and native retirement. It moves ownership
extractions outside assertions and checks the deliberate capacity defect and
exact restoration. All six new cases pass in both profiles; existing Stash
properties and exhaustive sequences exercise the shared admission algorithm.
No catalogue migration, new application protocol or placeholder is needed.

Final source passes all11 native Nix checks,1,051 nextest cases/zero skips and
78 checked compile-failure diagnostics, including the two new denials. Existing
stash_sequences, fsm_sequences, two_buffer_sequences and error_sequences each
complete10,000 fuzz inputs, with the zero-slot boundary seeded. The identical
real Application fixture passes path source and extracted packages on1.99 and
minimum1.96 in both profiles. It checks two distinct senders, the exact refused
allocation, completed work, later FIFO recovery and joined healthy retirement.
This is one native journey across eight selections, not eight domain journeys.
Archived Actors contains207 files; all production src trees match their owning
checkout. These0.29.0/0.16.0 candidate archives are not new registry releases.
Receipts, archives and source fingerprints remain under
`restart-2026-11-11/machine-limits-*`. The archive matrix's outer receipt writer
initially used zsh's read-only status variable after all four successful tests;
the four terminal exit receipts were checked individually before recording the
aggregate result. No test failure was suppressed or rerun for that shell issue.

The added public policy parameter has two real substitutions and preserves
the existing five-parameter annotation and inferred constructor. It adds one
statically selected admission/send implementation per used policy/route, no
dynamic dispatch or runtime policy registry. Wrong refusal protocol fails with
E0271 and forged policy with E0277. The existing frozen Processor probe compares
PR117's archive with this candidate using identical source/locks and fresh
targets on both compilers. Five processes per selection/compiler agree:
default actor size72 bytes,129 allocations/12,800 allocated bytes, held heap
10,752 bytes, peak10,784 bytes, completed heap6,656 bytes and disposed heap0.
Release executable sizes are854,608→854,592 bytes on1.99 and796,240→796,240
on1.96. Cold builds take16.71→14.76s and17.36→15.02s respectively; each is one
sample with dependency work, so no compile-speed improvement is established.
Cargo timing reports and binaries' sizes support this bounded generic-cost
audit, not a catalogue monomorphization or whole-runtime performance claim.
The first measurement wrapper used BSD stat syntax inside GNU Nix; its saved
build timing was reused and only size/measurement collection was resumed.
The finite policy's real Application and shared quota proofs establish behavior;
no finite-Machine allocation advantage is claimed. Broader DX06/26 costs remain
open. Only receipt/documentation edits follow the frozen full source gate.

Release030 preparation, before metadata fixes2026-11-11: PR119 merged as
c9a8408 after all14 exact7d0afaa feature checks passed. Main CI43053551760 and
the normal Release workflow43055494445 pass. The latter refreshes existing
PR118 at a5ef0c0, based on c9a8408, proposing Behavior/Actors0.30.0 and retaining
Macros0.16.0. Both Machine changes and their breaking migration are named in
Actors' generated changelog. No new versioned release has been established.

The required external release-skill preflight fails before editing: three
secondary Cargo locks still select029, and both README installation entries
still select0.29. Root offline metadata additionally lacks the cached
zerocopy-derive0.8.56 archive. Receipt `release030-preflight-before.log` records
the exact findings and terminal exit1. Fetch that locked archive graph, update
only workspace-owned version entries through Cargo, align the two README
entries and add Core's shared-version changelog entry. The proposed version is
owned by Cargo.toml; no new version policy or dependency upgrade is introduced.
Then rerun the same preflight, packaged consumer and full pinned Nix gate at
the final candidate. Preserve the existing four workspace boundaries.

Release ledger: nine expected release-PR paths including its generated three,
the three secondary locks, README, Core changelog and this existing task record;
zero Rust production/test/API changes. Recounting the complete resumed task
against f9e5fdf gives21 paths before the generated release and25 after these
planned fixes. The earlier20 count omitted stash/limit.rs from the cumulative
paths; its measured production delta was already included. Standing user scope
approval covers the corrected total. This receipt is not registry or downstream
adoption proof, and the larger programme remains open.

Release030 delivery,2026-11-11: all14 exact33a7da5 release-PR checks pass.
Previously enabled squash auto-merge commits PR118 as628649da014a527d47f21567a859c5c78494df13
at16:41:32UTC. Main CI43060819819 passes, and its normal publication workflow
43062619719 succeeds without dispatch. Behavior0.30.0 is published at17:00:14UTC
and Actors0.30.0 at17:00:22UTC; Macros remains0.16.0. Both registry entries are
available and unyanked. Downloaded archives match registry SHA-256 values:
Core a053cef32c83bfd0df2fb146d2163a9d78a5e71534febfbd5601087040776b27;
Actors af2978fe3eda8157844a8929b82df5c0454bc4946478993a5c0740fdf4259641.
Their production source trees equal checkout, and each archive's VCS metadata
and annotated release tag identify628649d. Exact receipts and actual registry
archives are preserved under `restart-2026-11-11/release030-*`.
[Core release](https://github.com/devrandom-labs/bombay-behavior/releases/tag/bombay-behavior-v0.30.0),
[Actors release](https://github.com/devrandom-labs/bombay-behavior/releases/tag/bombay-behavior-actors-v0.30.0).

The release correction passes the required four-workspace preflight, archive
consumer and all11 native Nix checks, including1,051 nextest cases/zero skips
and78 checked negative diagnostics. Its complete diff contains nine version,
lock, installation-documentation, changelog and existing-record paths, with no
Rust implementation or test changes. Existing packaged-consumer dead-code and
deliberate unused compatibility-patch warnings remain explicit. No whole-runtime
or fresh registry-based Bombay upgrade is credited from these owner gates.

The user-requested notice is queued to the rediscovered Bombay remote-actor
session01a1ca41-7633-7a11-bc73-53ad7779597e as01a1cb90-7e3d-7992-b0fc-2ab36dafa134.
It names exact published versions/links, retained-progress and original-input
error migration, bounded Machine syntax and the verification scope. Bombay
acknowledges receipt at17:04:07UTC and17:06:01UTC, retaining those details and its
existing remote-actor goal. It preserves PR412's dependency set while its Nix
gate is open, then will qualify registry adoption in the existing step order.
Delivery and acknowledgment are established independently; complete notice and
receipts remain in the existing `bombay-release-notices.md` handoff record.
This closes owner030 publication and its release notice. Runtime adoption,
proportional terminal recovery, remaining catalogue/privacy work and broader
resource/compile/runtime measurements remain open under the same DX01–DX28 goal.

#### Timer ownership — DX13/16

Ordinary OneShot, Periodic, Deadline and ReceiveTimeout construction takes the
inner actor, time policy and reaction, without caller IDs. Each wrapper owns
its exact typed event ingress and current generation. Reordering needs no caller
names or structural paths. Named domain-addressed timer methods remain available
with their original custom InjectEvent ports and actor-wide replacement law.

The ownership equation is `(target,generation,time)` outside conversion until
exact receipt `(target,generation)` or rejection `(original request,reason)`.
Shared products have explicit closed TimerId/Here targets, without generic
defaults; owned aliases select Here. A private destination port reuses settlement;
the queue distinguishes Named(id)/Owned(occurrence) with statically checked
occurrence arithmetic, no registry or global counter. Blanket path-scoping that
broke named replacement and a lowering that narrowed named methods were rejected.

Inner-first initialization, user-only inactivity renewal, optional deadlines,
current-generation acceptance, stale replay discharge, Stop/error decisions,
original rejection, accepted prefixes and independent later effects remain.
The absolute two-state and relative four-state protocols keep their existing
current deadline/generation, checked renewal history and inner actor. Earlier
service and fuzz oracles were independently strengthened; no-op callbacks prove
construction/noninterference only. Complete source/model measurements and
falsifiers remain in45ab42d and the retained timer tests.

#### Verification, measurements and delivery — DX19/25–28

| Exact scope | Executed result | Limit |
| --- | --- | --- |
| Published owner628649d, Behavior/Actors0.30.0 and unchanged Macros0.16.0 | All14 exact-head release checks,11 native Nix gates,1,051 nextest/zero skips and four-workspace release preflight pass. Main CI43060819819 and normal publication43062619719 succeed. Both actual registry checksums, production source trees, VCS records and annotated tags agree with the verified source. Bombay explicitly acknowledges the release/migration notice. | Full registry-based runtime adoption and remaining programme criteria stay open. Packaged-consumer dead-code and deliberate compatibility-patch warnings are unchanged. |
| Published owner55dbac2, Behavior/Actors0.29.0 and macros0.16.0 | All14 GitHub checks,11 native Nix gates,1,035 nextest/zero skips,123 doctests/zero ignores and four-workspace package preflight pass. Main CI38992917401 and normal publication38993686976 succeed. All228 shipped Rust files match actual Cargo-selected registry archives/checksums, tags and releases. | Owner delivery is complete; it does not close downstream acceptance. Offline audit has60 unavailable yank checks, six unused license allowances and two duplicate groups. Four package-fixture and four intended unused-patch warnings remain. |
| Actual Bombay4fcf68d registry029/016 | Immutable1.99/1.96 replay passes128 fixtures:20 positive/108 negative,30 suites,26 targets and111 outer passes plus the controlled child. All633 caller inputs are unchanged. | Superseded full Nix run was interrupted after21 valid outputs when both compilers independently reproduced the stale remote borrow underline. Full run exit1 is not a pass. Logs, hashes and individual outputs remain preserved. |
| Prepared immutable workspace extension | Both compilers pass136 existing cases:21 positive/115 negative,32 suites,28 targets,114 outer passes plus the controlled child; all633 inputs remain exact. Five verifier controls pass. | Final actualeadea00 replay also passes both compilers in225.176/223.750s, preserving all633 inputs and exact named coverage. The136-case default/Axum corpus does not include four remote borrowing cases. |
| Isolated minimum workspace with reviewed diagnostics |791 default/Axum outer passes, two existing parent-invoked child skips; the separate corrected remote debug matrix passes. | No remote credit comes from zero-case feature-disabled targets. Final-source minimum debug/release remote matrices each pass426 outer cases/20 UI fixtures with all633 prepared inputs unchanged. The final-source default/Axum workspace also passes791 outer cases/136 UI fixtures in87.305s, retaining both parent-invoked child skips. |
| Actual4fcf68d sanitizer/coverage gates | Driver2,048 bounded cases; Observe1,024 for each of four targets. Owner coverage passes:737/818 Driver,962/1001 actor execution,95/97 actor outcome,502/525 Observe lines. | Existing floors90/93/90/90% are unchanged. Bounded campaigns and line coverage are not exhaustive invariant or runtime-performance proof. |
| Final Bombayeadea00 | All25 native Darwin Nix gates pass in3459.500s on the unchanged633-input tree. Every output has its expected deriver and content hash. Workspace:791 outer successes/136 UI cases; remote debug/release:426 outer successes/20 UI each. Protected consumers:132 outer successes; two deliberate failed child controls and four zero-match controls are checked by their parents. Registry adoption preserves all396 complete unrelated records and the354-node native graph. | Both minimum remote profiles pass426 outer cases/20 UI; minimum workspace passes791/136 UI. Two existing parent-invoked workspace child skips remain explicit. Windows metadata is not execution. Draft [Bombay PR386](https://github.com/devrandom-labs/bombay/pull/386) runs separate Linux CI. Workspace success does not establish package correctness: actual runtime archive verification fails below. |
| Corrected Bombay41812fce | All25 native Nix gates pass in3185.671s on the unchanged633-input tree. Strict1.99/1.96 replay passes136 cases/32 suites/28 targets. Clean-commit three-package/all-feature verification passes both compilers. Actual extracted consumers pass24 selected native cases per profile/compiler, three renamed tests and six executable examples each; all seven examples compile. All501 archive files and19 caller Rust files remain unchanged. Archived doctests each pass six cases with three existing ignored excerpts. | Engine0.3.0/macros0.2.0/runtime0.2.0 are prepared, not published. Exact-head Linux Nix CI remains pending. Eight successful outputs were unavailable during initial post-run hash inspection; exact re-realization passes in584.829s, and all25 expected derivers/content hashes are verified and retained with local GC roots. Owner-relative compiler snapshots remain separate from the native archive selection. Historical compatibility inversions prove why smaller release bumps are invalid. |

Historical full failures retain their exact outcomes: initial Darwin signing and
disk exhaustion; e629464 exits1 with16 passing/nine failing gates; the later
b8c2e6f25-gate pass applies only to its registry026/142 tree. Intentional mutation
failures, malformed audit/parser attempts, stale/shared-target artefacts and
parent-invoked ignores retain separate dispositions. Store availability alone
never proves a completed run. Other platforms are unexecuted locally unless an
exact CI/source receipt establishes otherwise.

The immutable verifier binds exact compiler identities, patch/fixture/snapshot
hashes, complete names/modes/targets/summaries and controlled-child ownership.
Five unit methods and eight entry controls reject empty Cargo0, failed rows with
Cargo0, added source, changed canonical/fixture/patch, unknown compiler and
TRYBUILD overwrite for their own reasons. Verification changes neither checkout
nor scratch inputs except the reviewed minimum expectations. All Rust, Cargo,
example, formatting and lint commands run through pinned Nix.

One observed optimized retry hang remains unexplained:
`public_node_missing_host_restores_the_declaration_for_original_work_retry`,
receipt NODE-PROTECTED-RETRY-HANG. Passing retries do not establish a wakeup,
locale or SDK cause. Joined output recovery proves headers/reference properties,
not compressed contents or a permanent Darwin repair. A separate controlled
fixed-port AddressInUse failure versus SDK-owned `localhost:0` establishes a
fixture vulnerability and repair, not the hidden historical failure cause.

Acceptance still needs stronger custody witnesses: settlement classification
or count is not exact receipt/payload ownership; some older normal-retirement
projections discard actor state or settlement fields. Pairwise fresh identities,
proxy/child correspondence and exact action-row partition are not universally
checked. Five pool cases use u64 jobs rather than affine job inputs; a closed
auditor is distinct from a closed parent-report recipient. Original-work refusal,
parent closure and affine jobs must remain independently covered. Construction/
body-entry counters prove neither every poll nor arbitrary uninvoked disposal.

Memory outranks binary size. Sender storage176→48bytes and three constructors
8→0 allocations are bounded gains. A frozen equal77-dependency counter comparison
uses74 blocks, peak122916→123644bytes (+728) and equal end heap; it proves no
memory reduction. Timer queues retain368bytes/four allocations before/after;
owned events16→24bytes, requests24→32, receipts8→16, queue120 unchanged. A disjoint
key grows8→16:64 named timers retain6672→7184bytes (+512),18 allocations each,
total allocated13008→14000. No whole-application RSS, throughput or latency gain
follows.

An isolated optimized FIFO search journey from the actual41812fce archives uses
one root, one worker and one typed customer. Five interleaved processes per
capacity and allocator mode preserve the same job, complete retirement, cleanup
and notification results. Configuring both actor/client mailboxes to minimum1
(effective2), instead of1024, reduces measured peak heap396226→142770bytes and
System-allocator median child RSS4161536→3883008bytes. These are buffer-capacity
tradeoffs for one workload, not historical implementation or framework-wide
performance gains. Joined retained heap is10768bytes in both cases. After the
caller host drops, one56-byte allocation remains; an independent profile traces
it to Tokio's thread-local CURRENT_PARKER during blocking-pool shutdown.
The consumer selects Tokio1.53.3, separately from workspace1.53.1.

The4096-byte allocation/release calibration, independent OS RSS-unit calibration,
strict result classifier and eight missing/conflicting-result controls are
reviewed. A System-for-dhat inversion fails before actor work. That inversion
first exposed same-name Cargo cache reuse; the initial samples are superseded.
Accepted samples use separate source-specific targets and pinned1.99. Pinned1.96
independently reproduces every exact heap count in20 further process samples;
its System RSS medians are4128768→3850240bytes. A mistaken
1.95-shell attempt remains uncredited and is repeated correctly. Receipts:
shutdown-spelling/package-preview/POOL-MEMORY-SUPPORTED-COMPILERS-VERIFIED.json
and its40 raw per-process results. Portable memory and latency
remain open; retained heap is not claimed to be zero.

The repeated optimized stash probe uses64 move-only512-byte messages, five
repetitions/policy/compiler (50 runs), ten empty controls and payload calibration.
Two source/oracle reviews and a System-for-dhat inversion check instrumentation:

| Holding policy | All originals retained, bytes | After refusal disposal, bytes | Peak heap bytes | Allocations |
| --- | --- | --- | --- | --- |
| Limit0 |34816|0|34848|133|
| Limit1 |34912|608|34944|133|
| Limit8 |35008|4288|35040|127|
| Limit64 |34304|34304|34304|69|
| Explicit unlimited |34304|34304|34304|69|

The limit bounds stash storage, not a consumer retaining refusals. Taking inputs
transfers the same live allocations; dropping all custody reaches zero. Equal64
accepted inputs without overflow have equal finite/unlimited heap costs and
72/56-byte actor storage. Different refusal policies are not equal-work performance
comparisons. Portable measurements, process RSS, allocator overhead, configured
services, remote work, compile/incremental/diagnostic/macro costs and latency
remain open. No percentage threshold is invented.

The final example recount includes every Rust module, rather than moving code
out of main and crediting that as removal. Physical lines include comments,
blanks and the original embedded tests; assertion macro sites are counted
separately. These are source diagnostics, not executable-statement measurements
or a claim that every removed line was framework ceremony:

| Application | All Rust lines, initial→current | Assertion macro sites, initial→current |
| --- | --- | --- |
| Counter |226→125|27→0|
| Heterogeneous topology |406→127|68→0|
| Templates |252→107|20→0|
| Supervision |389→135|25→0|
| Worker pool |273→124|15→0|
| HTTP |521→119|26→0|
| Entity |399→295|53→0|

Baselinec3afb301 and currenteadea00 are exact. Assertion removal from examples
is distinct from preserving the owning tests. Two structural annotation seams
remain: HTTP's decorated Event projection and Entity's explicit retirement/
Terminal/ChildFailures declarations. Entity's callback storage/reporting remains
substantial; zero ordinary structural plumbing is not yet established across
all seven applications. Receipts: shutdown-spelling/FINAL-EXAMPLE-SURFACE.json.

#### Current change record and scope

The actual runtime archive at clean Bombayeadea00 fails pinned
`cargo package --locked -p bombay-rs` verification with305 errors. Its normalized
manifest selects registry Engine0.2.3 and macros0.1.3; its lock contains both
Behavior0.25.0 (Engine's public contract) and0.29.0 (runtime). Cargo reports the
distinct Behavior traits. Archive creation with `--no-verify` passes, but is not
package acceptance. Receipt: DEVX-029-RUNTIME-PACKAGE-VERIFY; archive/hash/VCS and
resolved packages: shutdown-spelling/RUNTIME-PACKAGE-FAILED-VERIFY.json.

Release-boundary law (deliberate package policy): Driver, Environment and runtime
must share the same nominal Behavior contract; changing that public dependency
must not silently enter an existing Cargo-compatible release line. Desired caller
syntax remains ordinary `Driver::new(actor, environment)` and the canonical
Application, without dependency patches or additional bounds. Before changing
production manifests, compare an unchanged historical Engine/Behavior0.25
consumer against the registry package and a deliberately misversioned current
Engine; prepare a coordinated archive preview separately from the tested branch.
Reuse the unchanged Driver/Environment and current macros/runtime. Expected
preview production Rust delta:0; public types added/removed:0. Candidate release
paths are the three package manifests/changelogs, root manifest/lock and any
exact nested locks naming their changed versions. Official release PR381's
Engine0.2.4 proposal needs independent compatibility verification. The existing
148-path approval does not authorize additional production paths automatically.

The unchanged historical Engine consumer passes with registry0.2.3/Behavior0.25,
fails with three E0277 errors when current Engine is labelled0.2.4, then passes
with Behavior0.29/Engine0.3.0. All three use identical Rust source. A second
historical topology consumer passes with registry runtime/macros0.1.3, but fails
with E0405/E0425/E0433 when current macros are labelled0.1.4: generated NativeHost
and Runtime do not exist in the old runtime. Thus the first coordinated archive
preview (Engine0.3/macros0.1.4/runtime0.2) compiles with all features, but its
macro version boundary is rejected. The corrected proposal is Engine0.3.0,
macros0.2.0 and runtime0.2.0. These are executable compatibility witnesses,
consistent with [Cargo's pre-1.0 version rules](https://doc.rust-lang.org/cargo/reference/semver.html),
not speculative API changes. No production Rust or bounds change. The isolated
ten-path preview expands the approved native scope from148 to153 paths;
five extra paths are the Engine/macros manifests and three changelogs. It is
not published. The user authorizes the153-path source scope on2026-10-17;
merging and publication require separate authorization. The retained patch uses
the corrected version boundaries and preserves all396 Rust files. Bombay commit
41812fce applies that correction and its coordination record. Three-package
all-feature preview verification passes. All seven examples and the renamed
consumer compile against extracted archives; six terminating local examples run.
Selected native archive journeys pass24 tests in each debug/release profile and
three renamed tests; all19 caller Rust files and498 extracted files are unchanged.
Both historical consumers retain their old registry versions when the honest
incompatible candidate versions are available. Runtime-only archive tests require
the existing external example files. Their owner-relative compiler snapshots
also differ when the runtime is an external dependency; the attempted snapshot
run remains failed, without overwrite. The selected native replay explicitly
excludes that compiler harness. Strict source-bound136-case replay is separate:
Rust1.99/1.96 pass on41812fce in203.362/208.448s with all633 inputs and exact
136/32/28 coverage conserved. All-feature native/current and Windows/minimum
metadata pass; the354-node native graph has identical edges, kinds and features
after normalizing only the three intentional workspace version identities.
Windows execution remains unclaimed; all25 resulting-source native Nix gates
pass at41812fce without changing any of633 inputs. Actual clean
41812fce three-package/all-feature verification also passes in8.655s: all82
production Rust files and VCS paths match, normalized manifests retain no paths
or patches, and the runtime lock's Engine/macro checksums match the exact tested
archives. Its separate actual archive journeys pass24 native cases/profile on each
supported compiler, three renamed tests and six terminating examples each,
conserving501 archive files and19 caller Rust files.
Pinned1.96 all-feature package verification passes in68.860s too. Both archive
doctest runs pass six cases; three pre-existing partial topology excerpts remain
ignored at41812fce. The guide correction removes these duplicate excerpts and
links their complete compiled examples/tests. Its separate archive previews pass
all six doctests with zero ignores on both supported compilers; all Rust tests
and assertions remain unchanged. All53 local guide links resolve. Actual native
all-feature doctests also pass all six with zero ignores in74.320s, including
their dependency rebuild. Original archives stay unchanged.

Bombayb4b21f9 now retains the runtime-issued joined root report for DX04/24 for ordinary post-run
inspection: `outcome.root_report()` and `retirement.root_report()`. Work returns
its business value; callers need not return the report merely to inspect actor
status. Before production edits, the complete real FIFO archive consumer failed
with exactly two E0599 errors; its unchanged-journey control ran successfully.
This is a derived inspection/custody law, not another actor-model effect or
lifecycle authority. An unpublished report stays absent, never fabricated success.

The complete ownership product is Work × optional root report × native receiving
× notification receiving × families. Completed selection transfers every axis;
other phases return the complete original outcome. `into_parts` returns all five
axes, with the report second. Reuse the existing Observe pair, cleanup publication
and join: the native cleanup handle carries a clone of its existing observation;
family-only cleanup carries none. After joining, `try_get` reads the immutable
snapshot without waiting for a publication that may never happen. Cold execution
allocates no observation earlier. No new public type, trait, bound, channel,
module, actor control state, effect lane or transition authority is introduced.

The user authorized179,184 and186 cumulative paths on2026-10-17. The additional
five existing test files contain twelve report-slot binding changes missed by
the first scan. All7,639 original assertion sites survive; six native regressions
and two product laws add55. Mechanical migration inserts only the report slot;
three compiler-fixture hashes change and expected diagnostic bytes remain exact.
Native success, initialization refusal, cold/running cancellation, both wrapper
orders and a real notification panic pass before catalogue migration. Erasing
only the acquired report causes five retention regressions to fail in each
profile while the cold/absent control passes; exact source is restored afterward.
Two independent test reviews inspect observables, custody and counterexamples.
Actual owner debug/release each pass344 library and19 outcome tests; the six
isolated native regressions also pass both profiles on1.96. Real FIFO and fixed
supervision replacement/drain journeys pass debug/release. Workspace/Axum
all-target checks and strict Clippy pass. Immutable1.99/1.96 replay passes all136
cases with the same32 suites,28 targets and633 unchanged source inputs. Fresh
full repository verification remains required; earlier41812fce gates do not
certify this source. Actualb4b21f9 archives verify all features on both supported
compilers, with82 exact production Rust files, clean VCS identity and exact
candidate dependency checksums. Extracted consumers pass32 native cases per
debug/release profile, three renamed tests and six running examples, conserving
501 archive files and284 caller Rust files. The isolated host's initial HTTP
feature-forwarding/count-classifier failures stay failed. HTTP lifecycle and
bind-refusal cases execute; the HTTP example is compiled. Cleanf0fdae3 archives
also verify all features (74.198/81.473s); its final minimum immutable replay
passes all136 cases with633 exact inputs in239.240s.

Linux CI onb4b21f9 exposed two missed `matches!` products and separately rooted
formatting. f0fdae3 inserts only their new report slot and fixes whitespace in
five already-approved files; removing those slots restores every original token
apart from whitespace. No assertion, production behavior, bound or public type
changes. Nested fmt, native-bin/protected all-target strict Clippy and whole
remote/Axum workspace all-target strict Clippy pass. The two native full runs
are interrupted, not credited as full passes:707.143s to correct that consumer,
then593.909s to include the newly authorized examples before the final freeze.

Bombaybb832b0 migrates pool, supervision, heterogeneous topology and Machine
examples. All five Work callbacks return only business results or unit; five
explicit retirement waits disappear. Existing run_with already joins actual
actors and cleanup, including natural timer completion. A separate unchanged
Machine/domain/policy consumer without the extra wait proves native Stopped
completion and retained completed(7) business state in debug/release. No no-op
binding or ignored must-use report is added. Four final examples run in both
profiles and strict whole-workspace remote/Axum Clippy/fmt pass. Main sizes are
pool90→93, supervisor96→98, topology32→34 and templates56→60 lines:20 added,
nine removed, no new alias/type/policy. Report inspection moves outside Work;
this is simpler ownership, not a line-count reduction. bb832b0 is frozen for
fresh25-gate native Nix and Linux CI. Its completed native run passes all25
gates in3308.872s, withbb832b0 unchanged and all633 source inputs identical.
Its six exact-head GitHub checks pass; PR386 merged as531ded50. Publication
is not established by that merge. Retained output hashes and exact outer/child
test counts still require their separate review; later main is not certified
by this run. Durable receipts: the saved handoff's
`results/DEVX-ROOT-REPORT-EXAMPLES-FULL-NIX.{json,log}`.

The actual two-family failure/retirement journey also preserves report inspection
on both compilers/profiles. All original test tokens and assertions remain;
only final root-report presence/classification checks are added in the isolated
consumer. Hydration, launch and capacity refusals, forced retirement, exact
callback/native/family custody and clean-root assessment coexist. Erasing only
the producer's report fails this same journey in both current profiles; every
one of501 extracted archive files is restored exactly afterward. No new law
implementation or production/test file is added for this downstream proof.
Ownerf336ec9 separately passes all11 native Nix gates in219.391s,1,035 nextest
cases with no skips, and all14 GitHub checks. The documentation branch retains
its own source limits; these owner results do not certify Bombay's running gates.

Aggregate checkpoint: application work presence/custody and all native control
sums remain unchanged; subordinate result alternatives and74 match arms remain
unchanged. Three production owners grow182→228,1732→1753 and389→395 lines
(+101/-28, net+73); this is a new inspection capability, not code reduction.
Public types added/removed0/0; two getters are added. Surviving values are the
same affine work, cleanup, native, notification and family products plus the
existing Copy report. Scans find no arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean or structural caller
syntax. Ownership/runtime/Driver laws cross-checked; disposition pass. Exact
source, two reviews, inversion and counts: shutdown-spelling/ROOT-REPORT-*.json.

The frozen-source FIFO cost comparison repeats five processes per capacity and
allocator mode on1.99 and1.96. Report preservation adds8bytes of peak heap
(396226→396234 at capacity1024;142770→142778 at effective capacity2), no extra
allocations and no retained heap: joined10768bytes, after host56bytes in one
Tokio parker allocation,169 blocks through join/174 after host. Identical business
source, lock, work and complete retirement are checked. System RSS is recorded
without claiming improvement from its noisy variation; these results cover one
root/worker/customer workload, not framework-wide costs.

Official release PR381 at5d486b8 remains unmodified. Its exact Engine source and
`^0.26.0` dependency (freshly resolving0.26.1) reject the unchanged0.25 historical
caller with the same three E0277 errors. Its macro source equals the independently
rejected0.1.4 candidate. Automated API-compatible labels therefore do not establish
package compatibility; the corrected version proposal has independent witnesses.

The expanded programme and single explicit topology are authorized; release
permission remains PR-specific. The earlier Bombay approval covers148 cumulative
paths: prior146 plus README.md/tools/diagnostics.py. Its applied seven-file
batch restores four original Cargo.lock edges, adds eight existing Engine/Observe
cases and two compiler-only diagnostic hunks, corrects one shared remote underline,
updates README spelling and compacts coordination. No production Rust, invalid
Rust fixture, public type, bound, actor state or runtime behavior changes.

Both compilers first reject the shared remote snapshot at its stale underline;
the complete E0499 denial and all other diagnostic bytes agree. Correcting only
eight marker columns passes the four cases. The original failed runs remain
failed. Scope/provenance and both source/oracle reviews are recorded before
retention in shutdown-spelling/{WORKSPACE-VERIFIER-SCOPE,LOCK-PRESERVATION-DESIGN,
REMOTE-BORROWING-VERIFICATION-DESIGN,FINAL-ADOPTION-SOURCE}.json. Aggregate checkpoint:
no new control/subordinate alternatives, transition branches, modules, authority,
arrival history, repeated cause, semantic flag or structural caller syntax;
disposition pass. Earlier semantic batches retain their complete before/after
measurements and surviving-value equations at45ab42d and in their law tests.

At1bef166 the compacted contract passes the owner documentation gate: generated links,
76 rustdoc failure diagnostics and existing documentation/assertion controls.
Its first standalone book build exposed an unquoted generic type; quoting that
inline code removes the HTML warning without changing the contract.

The owning PRD/evidence resumes on codex/devx-resume from mergedf9e5fdf;
the prior codex/developer-experience-docs branch is preserved at7318978. Historical
ledgers remain deleted. The sole checklist stays open until every selected
acceptance criterion is implemented and verified or explicitly transferred with
user agreement. Deferred networking/durability/identity/deployment products do
not defer SDK privacy or the selected local developer-experience programme.
