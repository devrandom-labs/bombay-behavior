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
generic send insertion and caller-name safety are delivered. Bombay's isolated
branch now selects these three registry packages without Behavior patches; all
actual Cargo-selected source bytes match the verified release. The canonical
shutdown migration and complete downstream/compiler/nested/repository gates
remain in progress. Older proofs retain their original version/revision limits.

Current owner delivery uses `codex/devx-delivery`, based on published55dbac2.
Its only changes are this PRD and its evidence appendix. The previous local and
remote `codex/developer-experience-docs` branch remains preserved at7318978;
already released implementations are not merged back as duplicate machinery.

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
| DX07 | The existing actor/worker expansion remains authoritative; empty birth aliases are delivered. Private send parameters avoid caller parameter/type names, raw spellings and occupied fallbacks. Pre-edit compile failures,19 added pure cuts, two-pass source/oracle reviews and three native failure/retirement journeys retain their recorded bounds. | Open: generic/private/public, renamed/sibling-target and duplicate-role reconciliation; common-owner fixes are published in029/016. |
| DX08 | Published owners preserve selected send lanes through wrappers and static denials. FIFO/keyed construction accepts ordinary named completion/business products. Published029/016 includes generic-product insertion and caller-name safety; owner gates, packaged consumers and complete missing/ambiguous controls pass. | Open: finish downstream retention, catalogue reconciliation and final delivery gates. Earlier native witnesses retain their recorded compiler/profile and source limits. |
| DX09 | ActorExt adds the exact owning FinalizeOnShutdown and EstablishedTerminationMonitor. Missing-method regressions, order/effect/refusal witnesses, archive consumers and recorded native gates pass. | Open: remaining decoration/catalogue audit and final combined-source reconciliation. |
| DX10 | Owning roster/directory/queue/router constructors use Config and explicit recovery. Published029/016 includes startup closures, reply-lane policy and constructor-local inference. All338 library tests pass both compilers/profiles with1013 original assertions;31 resumed caller tests and real examples have source-selected503 proofs. Startupf244566 removes its named plan/alias. | Open: inference repair is published in0.28.0; verify registry-only callers/nested locks, audit policy necessity/names and complete costs/full gates. Breaking source costs remain explicit. |
| DX11 | Inferred fixed/FIFO journeys preserve immediate/async startup, source refusal/panic, prepared prefix, assigned payload and shutdown. Keyed tests preserve affinity, exact rejection, unbind and joined retirement. | Open: dynamic supervision, independent late completion, remaining preparation/policy/failure cases and final gates. |
| DX12 | Published0.26.0 returns complete duplicate-correlation rejection. Actual birth/replacement/replay witnesses retain accepted prefix/suffix and joined children; exact recipients preserve real application/Entity identity after closure. | Open: remaining creation/occurrence acceptance, including transitive/wrapped/rejected births and relational identity evidence. |
| DX13 | Four no-ID timer wrappers and typed request custody are published; matching runtime and caller migration are on the isolated Bombay branch. PR109/6488965 passes11 fresh native gates and14 current CI checks; combined archives run112 consumer tests and16 examples across both compilers/profiles. Default Bombay workspace passes782 outer tests/132 UI fixtures; selected remote426 outer/20 UI per profile and both child-feature harnesses pass on1.99/1.96. Named replacement and memory costs remain explicit below. | Open: authoritative Bombay and protected nested gates, remaining acceptance cuts and final registry029 caller gates. Candidate/source/archive proofs do not certify the complete programme. |
| DX14 | Published029 retains finite admission, complete refusal and unwind-safe previously held custody; `take()` transfers originals. Both wrapper orders, initialization/replay/Stop, native open/closed/panic retirement, static denials, ranked/property/fuzz sequences and four independent defects pass their source-bound recorded matrices. All11 current owner gates pass with1,035 tests/123 doctests; registry-only compiler replay passes both supported compilers. | Open: final registry-only Bombay catalogue/nested/repository gates and Machine bounds/retention audit. Publication and authorized caller adoption are delivered; Clone-based rollback remains disproven. |
| DX15 | Native b1c6781 adds actor-owned interpreters through Application/topology using existing guarded startup. Real file/environment requests, wrapper orders, root/descendant/cold/Entity configuration, rejection/panic/cancellation and disposal witnesses pass recorded matrices and inversions. Independent owner diagnostics retain source/coverage controls. | Open: final integrated runtime/repository/archive gates, configured-service costs and delivery. Earlier child-oracle/work-entry falsifiers and their corrected evidence remain bounded; focused passes do not close the programme. |
| DX16 | Startup/provenance and complete source/interpretation custody remain. PR100 merged as18653a7 supplies OwnedItem from existing settlement metadata; real IO/panic custody, mutations, privacy denials, owner gates and extracted consumers pass. PR101 head184354b fixes rejected reply custody:970 owner tests, ten native Nix gates, four-workspace preflight and locked real-runtime failure/retirement proofs pass. It merged as dda4a42 under explicit authorization; reviewed release PR103 merged as13951b8 and normal publication of Behavior/Actors0.26.1 is verified in both registries and release tags. | Open: complete registry029 catalogue reconciliation, two-template/wrapper composition, native integration and final downstream/cost evidence. Different custody equations remain distinct. |
| DX17 | Shared retirement custody and semantic send access have bounded interface provenance. DirectoryConfig validates immutable limits once. PR110/7644735 removes the duplicate adapter spelling while preserving its implementation; complete native failure/retirement, both route alternatives, static denials, extracted packages and eleven owner Nix gates pass their recorded matrices. | Open: export/consumer/invariant and bounds audit; remove competing spellings only after comparing complete semantics. |
| DX18 | No new production dependencies selected. Existing thiserror, standard owned sums/products and validated limits remain the chosen tools; bounded registry/language comparisons are in the evidence appendix. | Open: error-embedding/source/visibility/bounds equivalence and dependency/MSRV/feature/compile-cost audit. |
| DX19 | Final Bombayeadea00 with registry029/016 passes immutable1.99/1.96 replay:136 cases (21 positive/115 negative),32 suites,28 targets,114 outer passes and the exact parent-invoked child. All633 source inputs remain exact;23 reviewed compiler-specific expectations preserve causes/codes. Both compilers independently reject the stale remote underline; the corrected four-case current borrowing suite passes. Caller migration retains255 assertions. | Open: final minimal/renamed/remote/archive coverage and programme gates. The earlier791-test source-selected workspace remains bounded evidence; failed/incomplete runs remain uncredited. |
| DX20 | Application directly establishes root/selected-recipient/Entity clients and senders. Nonwaiting exact admission preserves original Full/Closed commands; origin inversions fail. Shared external customers and pre-root scope retain target authority and original inputs on retry. | Open: complete correlated/cancelled/slow/closed-customer/late-reply corpus, capacity and final-source minimum-compiler/full gates. Admission remains distinct from business completion. |
| DX21 | Canonical `application.shutdown()` and advanced lifecycle access retain the original admission Result and inferred ingress. Both owners pass the pre-edit missing-method control and supported-compiler debug/optimized journeys. Root wrapper orders, request replay, natural completion and complete retirement retain their assertions; immutable compiler replay preserves missing/ambiguous authority denials. | Open: child-derived phases/drain/deadline/forced-retirement corpus and final repository gates. Admission does not imply joined cleanup. |
| DX22 | HTTP/Entity use the same Application and joined outcomes. Awaitable retire_entity preserves exact-slot completion, cancellation, reactivation and native/callback failures; HTTP removes unnecessary clones and receives ApplicationHandle directly. Original receiving/HTTP/family tests retain their assertions. | Open: callback/associated-type distillation, complete family/root cleanup coexistence and final complete consumer/repository gates. |
| DX23 | Zenoh is the sole internal network. Both Node Work callbacks receive only typed actor capabilities; native client/requester test drivers live under Node. Raw configuration/export/query surfaces remain unfinished. | Deferred: networking, durability, identity and deployment products stay with later PRDs. Open interface obligation: hide SDK types without a transport escape hatch; preserve typed extension and complete outcome laws. |
| DX24 | Terminal evidence stays authoritative. FIFO rejected-role projection preserves its exact failure/lifecycle product through both orders, inverse and native consumer. | Open: bounded borrowed operational inspection, telemetry-failure independence, sensitive-payload policy and bounded diagnostic dimensions/retention. |
| DX25 | Protected actor-only consumers conserve original custody; provider and current catalogue retirement matrices have source/effect reviews and independent failure controls. Freshb8 full-gate evidence is separate from earlier runs with masked classifier crashes. | Open: the observed optimized original-work-retry hang remains unexplained; complete final registry-only protected consumers and gates. Passing retries or repaired output headers do not explain that hang or establish compressed-content/permanent Darwin repair. |
| DX26 | Recorded examples remove structural assembly; counter51→36 main lines, pool191→90, supervisor332→96 main plus39 domain lines. Assertions move to owning tests. Frozen counter comparisons retain equal allocation/heap products; sender/client measurements improve their bounded cases. Fifty optimized stash runs account for held/refused originals; the no-overflow control has equal heap costs and16 additional actor bytes. | Open: full comparable caller ceremony, compiler/diagnostic/macro costs, layouts/binary metadata and runtime memory measurements; configured-service, remote and latency costs remain unproved. Memory outranks binary size. |
| DX27 | Published Behavior/Actors0.29.0 and macros0.16.0 match actual registry checksums, tags/releases and228 shipped Rust files at55dbac2. Reviewed PR115, all14 GitHub checks, all11 native gates, four-workspace preflight and normal main-CI/publication workflows pass. Root/fuzz/protected locks select these registry owners; Driver inventory, fuzz compilation and both immutable compiler corpora pass. | Open: complete registry-only nested runtime and repository gates, final feature/consumer/cost evidence and delivery docs. Older proofs retain their original source limits. |
| DX28 | Historical pruning is retained. One compiled application guide runs in workspace/extracted packages across recorded configurations; local links resolve. This table is the sole DX01–DX28 disposition checklist. | Open: compact contract/evidence reconciliation, compiled sample/support tables, rendered/external links, final documentation gates and retirement of this PRD/appendix after obligations have owners. |

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
missing publication cannot fabricate completion. Callback/associated-type
ceremony and independent family/root cleanup acceptance remain open.

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

Machine's Clone-based rollback hypothesis remains reopened: shared cloned state
disproves rollback, so Clone cannot establish the claimed transactional law.
Do not remove existing bounds by dropping deferred inputs, weaken static phase
restrictions or credit finite Stash as bounded Machine retention. The Machine
bounds/deferred-custody model needs its own focused pre-edit witness and complete
failure equation before production changes.

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
| Published owner55dbac2, Behavior/Actors0.29.0 and macros0.16.0 | All14 GitHub checks,11 native Nix gates,1,035 nextest/zero skips,123 doctests/zero ignores and four-workspace package preflight pass. Main CI38992917401 and normal publication38993686976 succeed. All228 shipped Rust files match actual Cargo-selected registry archives/checksums, tags and releases. | Owner delivery is complete; it does not close downstream acceptance. Offline audit has60 unavailable yank checks, six unused license allowances and two duplicate groups. Four package-fixture and four intended unused-patch warnings remain. |
| Actual Bombay4fcf68d registry029/016 | Immutable1.99/1.96 replay passes128 fixtures:20 positive/108 negative,30 suites,26 targets and111 outer passes plus the controlled child. All633 caller inputs are unchanged. | Superseded full Nix run was interrupted after21 valid outputs when both compilers independently reproduced the stale remote borrow underline. Full run exit1 is not a pass. Logs, hashes and individual outputs remain preserved. |
| Prepared immutable workspace extension | Both compilers pass136 existing cases:21 positive/115 negative,32 suites,28 targets,114 outer passes plus the controlled child; all633 inputs remain exact. Five verifier controls pass. | Final actualeadea00 replay also passes both compilers in225.176/223.750s, preserving all633 inputs and exact named coverage. The136-case default/Axum corpus does not include four remote borrowing cases. |
| Isolated minimum workspace with reviewed diagnostics |791 default/Axum outer passes, two existing parent-invoked child skips; the separate corrected remote debug matrix passes. | No remote credit comes from zero-case feature-disabled targets. Final-source minimum debug/release remote matrices each pass426 outer cases/20 UI fixtures with all633 prepared inputs unchanged. The final-source default/Axum workspace also passes791 outer cases/136 UI fixtures in87.305s, retaining both parent-invoked child skips. |
| Actual4fcf68d sanitizer/coverage gates | Driver2,048 bounded cases; Observe1,024 for each of four targets. Owner coverage passes:737/818 Driver,962/1001 actor execution,95/97 actor outcome,502/525 Observe lines. | Existing floors90/93/90/90% are unchanged. Bounded campaigns and line coverage are not exhaustive invariant or runtime-performance proof. |
| Final Bombayeadea00 | Registry adoption87c602e restores all396 complete unrelated root package records; 8bc055b extends existing verification; eadea00 compacts coordination. Current remote borrowing passes all four fixtures. Current native and minimum Windows locked all-feature metadata pass; native354-node graph is conserved. | Immutable supported-compiler replays pass on this frozen633-input tree; full25-gate Nix is running; both minimum remote profiles pass426 outer cases/20 UI fixtures. Draft [Bombay PR386](https://github.com/devrandom-labs/bombay/pull/386) runs separate Linux CI. No full-gate pass is claimed. |

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

The expanded programme and single explicit topology are authorized; release
permission remains PR-specific. The latest Bombay approval covers148 cumulative
paths: prior146 plus README.md/tools/diagnostics.py. The applied final seven-file
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

The compacted contract passes the owner documentation gate: generated links,
76 rustdoc failure diagnostics and existing documentation/assertion controls.
Its first standalone book build exposed an unquoted generic type; quoting that
inline code removes the HTML warning without changing the contract.

The owning PRD/evidence branch is codex/devx-delivery based on published55dbac2;
the prior codex/developer-experience-docs branch is preserved at7318978. Historical
ledgers remain deleted. The sole checklist stays open until every selected
acceptance criterion is implemented and verified or explicitly transferred with
user agreement. Deferred networking/durability/identity/deployment products do
not defer SDK privacy or the selected local developer-experience programme.
