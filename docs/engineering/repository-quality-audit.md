# Repository quality audit and completion checklist

Audit date: 2026-09-28. Baseline: `435560ce7bea8ad3330ee2d42e5034f837a80602`.
Scope: all five workspace crates, their test and benchmark targets, the nested
macro fixture and fuzz workspaces, public documentation, and verification gates.

The repository has a substantial typed algebra and meaningful adversarial tests.
It also has gaps between its documented contracts, current composition surface,
and what the tests prove. Address those gaps before expanding the catalogue.
The best initial reductions are redundant test interfaces and dependencies,
repeated effect-product machinery, and stale documentation. Aggregate state
must be reduced only after proving that the removed distinction is unnecessary.

This is an audit and work list, not a claim that every branch is verified. All
crates were inventoried; manual review covered public entry points, effect and
creation interpretation, every actor family, representative transitions and
test oracles, macros, and tooling. Source scans supplement that review. No line
or branch coverage measurement, complete mutation campaign, or downstream
Bombay Engine integration run was performed for this audit. Production code
and existing tests were not changed at the audited baseline. Repairs made on
the working branch are recorded below.

## Inventory and interpretation of the evidence

Counts below are physical Rust lines, including comments and inline tests.
They measure review surface, not essential complexity or production-only size.
Integration counts include Rust support modules and macro fixture sources.

| Crate | `src` files / lines | Integration and fixture files / lines | Responsibility and audit focus |
|---|---:|---:|---|
| `bombay-behavior` | 10 / 6,844 | 14 / 3,739 | Pure algebra, custody, structural composition, interpreter contracts |
| `bombay-behavior-actors` | 125 / 58,913 | 42 / 35,188 | Catalogue policies, lifecycle state, ownership, public construction |
| `bombay-behavior-macros` | 1 / 1,503 | 9 / 215 | Parsing, generated products, dependency resolution, diagnostics |
| `bombay-behavior-testkit` | 2 / 182 | 30 / 7,435 | Independent oracles, finite driver, properties and compositions |
| `behavior-mutants-gate` | 1 / 278 | 0 / 0 | Mutation verdict integrity; three inline unit tests |

The fuzz manifest declares 21 binaries. Its target directory has 28 Rust files;
the additional files include support modules and must not be counted as seven
missing campaigns. The scheduled workflow currently lists all 21 binaries.
There are also two benchmark programs and one core example.

Evidence labels used below:

- **Confirmed:** directly visible in source or reproduced by a focused probe.
- **Coverage gap:** the cited test or gate cannot establish the stated law;
  this does not by itself prove a production failure.
- **Design candidate:** a bounded investigation with explicit acceptance
  criteria; it is not authorization to introduce a new abstraction.

Priority: **P1** affects contract correctness or trust in verification;
**P2** affects maintainability, developer experience, or breadth of evidence;
**P3** is supporting cleanup. Check an item only after its completion evidence
is recorded against a revision.

## P1 — close contract and verification gaps

- [x] **A01 — Reconcile creation, installation, and initialization ordering.**
  **Confirmed document conflict.** `docs/actor-transition-algebra.md`, “Fresh
  creation,” orders initialization and its effects before installation.
  `docs/atomic-runtime-settlement.md`, “Initialization and activation order,”
  orders installation commit before the pure initialization fold, and describes
  `InitializeWorker` after `ChildCreationOutcome::Established`.
  `actor/creation.rs::ChildCreationOutcome` additionally represents initialization
  and host rejection before successful establishment. A runtime author cannot
  implement both descriptions as one unconditional order.
  **Complete when:** one normative contract distinguishes definition
  initialization, effect settlement, endpoint establishment, activation, and
  ordinary ingress; all canonical docs agree, and an interpreter trace proves
  the order plus each rejection and initialization-stop case. Any distinction
  between ordinary and atomic workers must be explicit and compositional.
  Classification: deliberate Bombay policy and derived composition, not a new
  actor-model guarantee. Resolve the law before changing production semantics.

- [x] **A02 — Support ordinary, unrenamed Cargo dependencies in both macros.**
  **Confirmed source defect; external probe recorded below.**
  `behavior-macros/src/lib.rs::crate_path` emits the name returned by
  `proc_macro_crate`. The manifests expose libraries named `behavior` and
  `behavior_actors`, while unrenamed package keys are `bombay-behavior` and
  `bombay-behavior-actors`. Facade resolution already compensates for this
  package/library distinction; direct dependency resolution does not. Current
  fixtures rename the direct dependencies, which avoids the failing case.
  **Complete when:** external consumers compile `#[behavior]` and
  `#[pool_worker]` with unrenamed dependencies, renamed dependencies, facade
  only, renamed facade, and direct plus facade dependencies. Preserve the
  existing missing-dependency diagnostic. Use a failing consumer fixture before
  editing expansion paths.

- [x] **A03 — Restore logical-host projection through atomic actor products.**
  **Confirmed missing composition.** `actors/src/atomic/requests.rs` derives
  send interpretation and settlement but not `LogicalDeliveryProtocols`.
  `FifoRequests`, `KeyedRequests`, `FixedSupervisorRequests`, and
  `DynamicSupervisorRequests` use this macro. The handwritten `ProxyEffects`
  also lacks this projection. There are no
  atomic implementations in `actors/src/requirements.rs` either. The current
  `behavior-testkit/tests/logical_host_requirements.rs` proves a synthetic
  `DeliveryOutcomes` tree, not these actual atomic families.
  **Complete when:** real FIFO, keyed, fixed, dynamic, and proxy compositions
  satisfy the documented projection, including transitive children and two
  wrapper orders. Assert exact ordered protocol products and duplicate
  occurrences. Audit `CustomerDelivery` and `DiagnosticAction` as part of the
  law: their logical routes must not disappear merely because they travel in
  `InterpreterRequests`, whose current projection is empty for every item.
  An empty blanket implementation is not a repair.

- [x] **A04 — Make compile-denial tests fail for the intended reason.**
  **Confirmed false-positive examples.** The `ChildRoute` compile-fail example
  in `actors/src/composition/message_adapter.rs` imports a removed type, so it
  does not prove rejection of a current creator-local capability. The
  `ActivationPermit` duplicate-move example in
  `actors/src/atomic/worker/initialization.rs` omits the required `W: Behavior`
  and endpoint bounds. It can fail before testing affine ownership. The
  incomplete-interpreter example in `behavior/src/actor/creation.rs` also used
  an implementation signature that obscured the missing child-host bound.
  **Complete when:** each safety fixture has a compiling lawful counterpart;
  one deliberate invalid operation produces the intended diagnostic. Exercise
  the counterfactual by removing that invalid operation or weakening the
  relevant invariant in an isolated probe. Keep removal-of-obsolete-name tests
  separate from capability-denial evidence. Rustdoc `compile_fail` alone proves
  that compilation fails, not why it fails.

- [x] **A05 — Make the mutation verdict reject malformed and incomplete runs.**
  **Confirmed source defects; adversarial probe recorded below.**
  `mutants-gate/src/main.rs::usable` rejects a failed baseline if present but
  does not require a successful baseline. `tallies` counts `Failure` and
  `Success` mutant outcomes toward completion while treating them as neither
  missed nor timed out. `check` matches counts per function, so duplicated
  outcomes can substitute for a different candidate in that same function.
  Three tests cover a clean run, a survivor, and a failed baseline only.
  **Complete when:** the gate verifies an explicitly supported baseline mode,
  exact candidate/outcome correspondence, valid outcome categories, missing and
  duplicate outcomes, unknown candidates, timeouts, viability collapse, stale
  baseline entries, malformed input, and command exit status. Include a case
  with a caught mutant plus a failed mutant at an otherwise satisfied floor.
  Reuse the report's actual candidate identity; function counts alone cannot
  prove campaign completeness.

- [x] **A06 — Strengthen partial property-test oracles to complete observations.**
  **Coverage gap.** In `behavior-testkit/tests/catalogue_models.rs`, the
  sequencer model checks delivery payloads and state, but not outcome messages,
  destinations, creation emptiness, or the next verdict. The order-gate model
  similarly checks selected deliveries. In `catalogue_invariants.rs`, successful
  configuration and readiness updates often check `result.is_ok()` and state,
  discarding successful `Actions`. Some deduplication checks inspect element
  zero without excluding extra elements. These tests are useful, but do not
  support the existing audit's claim of complete outputs after every step.
  **Complete when:** each generated step checks every declared effect lane,
  cardinality and order, destination, rejected owned input, and next verdict.
  Check initialization too. Use distinct owned payloads for custody-sensitive
  cases. Demonstrate failures for extra replies, wrong recipients, unexpected
  stopping, and dropped ownership; retain an independently structured oracle.

## P2 — improve verification reach and remove repeated machinery

- [ ] **A07 — Extend mutation evidence to the actor catalogue.**
  **Coverage gap.** Both mutation derivations in `flake.nix` select only
  `--package bombay-behavior`; test packages are core and testkit. They do not
  mutate `bombay-behavior-actors` or run its integration tests as a selected
  mutation test package. A strict core verdict says nothing about the largest
  crate's survivors. `actors/tests/mutation_contracts.rs` is an ordinary test
  file, not evidence of an actor mutation campaign.
  **Complete when:** independently reviewable actor-family campaigns exercise
  their relevant unit, integration, and model suites; survivors have a killed
  regression or a reviewed equivalence argument. Ratchet viability separately
  from mutant detection, and label coverage by crate and law. Fix A05 first.
  The current candidate listing for `bombay-behavior-actors` contains 3,125
  mutations; no actor-wide campaign verdict is claimed by this branch.

- [x] **A08 — Make external-consumer and packaging gates exercise their claims.**
  **Confirmed gate gap.**
  `behavior-macros/tests/crate_resolution.rs::facade_package_sibling_targets_resolve_the_library_crate`
  calls `cargo check -p bombay-rs` without `--examples` or `--all-targets`.
  The intended witness is `fixtures/facade/examples/sibling_target.rs`, which
  default `cargo check` does not select. The Nix package check only lists
  archive contents; the script's archive mode uses `--no-verify`.
  **Complete when:** explicit target selection checks the sibling example and
  both macro entry points; deliberately breaking that example breaks the gate.
  Build extracted published packages in a suitable release lane and compile a
  minimal consumer of the actual documented dependency declarations. Record
  package-content checks separately from package build verification.

- [x] **A09 — Remove test-only indirection and unused test dependencies.**
  **Confirmed simplification.** `behavior-testkit/src/lib.rs::InitializeTest`
  forwards unchanged to `behavior_actors::Activate` and adds a second spelling
  of the same operation. `criterion` is a dev dependency, but the benchmark is
  a handwritten `main` and no Rust source uses Criterion. Four testkit files
  contain 15 `#[tokio::test]` tests collectively and no `.await` at all:
  `stash_properties`, `fsm_properties`, `compositions`, and `error_paths`.
  Several destination-only fixtures implement an inert `Behavior` even though
  the route contract needs only `Protocol`.
  **Complete when:** callers import `Activate` directly, unused dependencies
  disappear from manifests and the lockfile where appropriate, synchronous
  tests use the ordinary harness, and destination fixtures keep only required
  capability implementations. Preserve fixtures whose actual purpose is to
  test behavior or creation. Report deleted surface and compile impact.

- [x] **A10 — Define what the finite test driver preserves on error and in time.**
  **Confirmed limitation.** `behavior-testkit/src/lib.rs::drive` returns only
  `B::Error` on a later failed transition, dropping accumulated successful
  effects and the active behavior. `Trace` also appends products lane by lane,
  losing the boundaries between turns. Interpreting that accumulated product
  afterward cannot establish per-turn runtime effect order. The error test
  fails on the first event and checks only the remaining mailbox length.
  **Complete when:** a success-success-error case exposes or explicitly
  documents the successful prefix's custody, and order-sensitive tests inspect
  per-turn actions or an independent interpreter trace. Do not describe the
  accumulation driver as a full runtime witness. Reuse existing action and
  settlement products if a richer error observation is needed.

- [ ] **A11 — Give named effect-product derivation one maintained implementation.**
  **Design candidate with observed drift.** The proc macro's generated sends,
  `atomic::request_product!`, and handwritten products such as `BufferSends`,
  `DeliveryOutcomes`, `LeaseSends`, and `PresenceSends` independently implement
  empty/append, interpretation, settlement classification, and custody.
  Logical-host projection is maintained separately in `requirements.rs`; A03
  shows a missed law in one derivation path.
  **Complete when:** a law table compares complete equations before selecting
  shared machinery. A retained derivation must preserve semantic field names,
  declared order, corruption suffixes, source admission, and logical-host
  projection, and delete repeated implementations. Prove two unrelated real
  products and both wrapper orders before catalogue migration. Do not merge
  products with different ownership or retirement laws, or introduce a public
  product framework merely to save typing.

- [ ] **A12 — Reassess aggregate decomposition using retained current values.**
  **Design candidate.** FIFO's root has 4,095 lines, stable proxy's root 3,509,
  fixed recovery 3,080, and dynamic supervisor's root 2,839. These counts include
  comments and tests; they are review triggers, not evidence of redundant
  states. FIFO keeps most transition concerns at its visibility root, while
  fixed supervision spreads substantial transition authority among roster,
  recovery, start, outcome, and shutdown implementations. Several roots use
  `mem::replace(..., Stopped)` or a dormant placeholder during a transition.
  **Complete when:** each family has the full AGENTS aggregate-drift record,
  including before/after states, alternatives, branches, production lines,
  modules, public spellings, and the future-needed value in every subordinate
  alternative. Investigate direct owned-data joins and one commit point;
  preserve distinctions needed for rejection, concurrency, or terminal custody.
  Reduce responsibilities only where a falsifying law proves redundancy.
  Keep FIFO assignment policy and keyed binding policy distinct.

- [ ] **A13 — Audit public bounds, hidden exports, and extension ownership.**
  **Confirmed surface requiring review.** The current source has 15
  `#[doc(hidden)]` annotation sites in core and 86 in actors, including
  members and re-exports. The
  [public-surface inventory](public-surface-inventory.md) classifies each site
  by contract owner. These annotations do not make an item private.
  Conversely, an associated type
  mentioning a value does not by itself justify exporting it. Some `Protocol`
  impls carried transition-related bounds: `Cache` required `K: Clone + Eq`
  and `V: Clone`; `Resolver` required `K: Clone + Eq` before the focused A13
  repair below. The four timer wrapper structs already have only one generic
  parameter, `B`; their effect signatures project address, phase, sends, and
  births from it, so they are a successful example rather than a gap.
  **Complete when:** classify each public item as application API, generated
  code obligation, or runtime port; identify each trait's lawful implementors.
  Move bounds only after caller-facing compile witnesses prove the narrower
  law. Measure diagnostics and compile cost. Document required runtime ports
  openly and keep representation private where Rust permits. Add no aliases,
  defaults, visibility, or generic parameters solely to silence the compiler.
  **Progress:** the [public-surface inventory](public-surface-inventory.md)
  now accounts for all 77 top-level public traits by lawful implementor role.
  The inventory confirms that `StashStatus` has multiple real wrapper
  implementations; its name alone is not grounds for deletion. External
  manually authored child roles and generic logical-host owners have compile
  witnesses for the newly visible types. A repeatable compile-cost comparison
  and review of remaining hidden runtime ports are still required.

  The creation settlement review found a narrower documentation defect:
  external caller suites name `CreationSettlement`, `CreationSettlements`, and
  `CreationsSettled` to retain or return exact child-creation custody, but none
  appears as an item in the generated crate-root Rustdoc index. Their public
  visibility and settlement equations already exist; showing them changes no
  actor transition. `InterpretCreations` is currently consumed only by the
  library's own traversal, so the same evidence does not justify exposing it.
  The three externally named settlement ports are now listed by Rustdoc.

- [x] **A14 — State the trust scope of initialization capabilities accurately.**
  **Confirmed documentation/API mismatch.** `InitializationTurn` says only the
  lifecycle boundary can issue initialization exactly once. However the public,
  doc-hidden `behavior::initialize(&mut B)` can be called repeatedly, and
  `delegate_transition` can be called before initialization. `Active<B>` provides
  a consuming application path, but it does not restrict those public wrapper
  ports. They are necessary seams to assess, not proof of universal enforcement.
  **Complete when:** specify which operations are trusted wrapper/runtime ports
  and which invariant the application-facing types actually enforce. Add a
  legal-user witness for that scope. If stronger enforcement is required,
  establish its composition law before changing the API; hiding rustdoc is
  insufficient.

- [x] **A15 — Make coding-rule enforcement agree with the actual repository.**
  **Confirmed local-rule deviations.** Timer-domain tests invoke mutable
  `accept` inside `assert!`; catalogue models carry readiness decisions as
  booleans. Many rustdoc fixtures have `use` sections despite the explicit
  fully-qualified-snippet rule. Private `Operating` types in both pools are
  ambiguous outside their immediate context. These are repository-policy
  issues; ordinary predicates are not automatically invalid semantic state.
  **Complete when:** move transitions outside assertions, express semantic
  alternatives using domain sums, update snippets and ambiguous domain names,
  and add focused checks for rules that can be enforced reliably. Keep
  assertions observational in debug and optimized tests. Audit nested
  `.sends.inner.inner` access in `testkit/tests/compositions.rs`: interpreter
  structure tests may inspect structure, while ordinary consumer tests must
  prove syntax that survives an unrelated layer.

- [x] **A16 — Restore developer-facing documentation accuracy.**
  **Confirmed drift.** The root README installs `0.14` while workspace packages
  are `0.17.0`; four distinct linked documents moved to `docs/engineering/`.
  It still names `InstallBirth` and claims stricter birth-algebra equality than
  the current role resolver documents. Historical engineering records describe
  removed templates and tests. Their historical status is useful, but an old
  “pass” cannot certify current code. Generated-HTML checking does not validate
  the repository README or compile all book examples.
  **Complete when:** root and crate entry points have working links and runnable
  current examples; current contracts use retained names; historical verdicts
  clearly identify revisions. Add README link and consumer-snippet checks.
  Prefer links to one normative law over copied explanations in several docs.

- [ ] **A17 — Validate the real downstream interpreter contract.**
  **Coverage gap explicitly acknowledged by existing docs.**
  `atomic-runtime-settlement.md` says Bombay will change to implement the
  architecture. Local tests include useful scripted interpreters, fabricated
  established endpoints, and a `ProxyRuntimeWitness` that returns corruption
  for every item. Such tests prove static composition and local custody, not a
  working host, fresh allocation, admission, or retirement transfer.
  **Complete when:** a recorded downstream revision runs an end-to-end witness
  for initial and replacement creation, collisions/exhaustion, initialization
  stop/rejection, activation, rejection followed by independent effects, closed
  emitter admission, and parent-to-root residual transfer. Prove that failed
  replacement establishment never reports restart success. Keep allocation,
  scheduling, and transport in the interpreter.
  A read-only inspection found a real `EstablishChild` implementation in the
  sibling Bombay checkout, but that checkout has substantial uncommitted work
  on a separate branch. This branch has not run the required downstream
  revision/witness, so the scripted testkit trace is not labeled integration
  evidence.

## P3 — make supporting evidence intentional

- [x] **A18 — Correct benchmark meaning before using performance results.**
  **Confirmed benchmark mismatch.** `protocol_matrix.rs::measure_fsm` claims
  alternating phase changes, but transitions `A -> B` once and then always
  stays in `B`. The base benchmark calls the inherent macro-authored receive
  method directly, whereas composed cases use activated behaviors. Observing
  only empty action sizes also weakens confidence that intended work survives
  optimization. Both benchmark programs report a single elapsed interval.
  **Complete when:** preflight traces prove the advertised workload, inputs
  and resulting state are observable, comparison paths and units are explicit,
  and repeated measurements report variability. Remove Criterion if these
  remain custom benchmarks. Measure `Machine` cloning with a real state and
  held queue before attempting to change its rollback design.

- [x] **A19 — Preserve truthful error sources and error ownership.**
  **Confirmed implementation gap.** `MachineError`, `TerminationMonitorError`,
  and `TerminationPropagationError` manually implement `std::error::Error`
  without forwarding an enclosed error as its source. Their fields retain the
  cause, but standard error-chain consumers cannot reach it. Some public
  aggregate failures have only their domain sum, so formatting expectations
  also need an explicit caller contract.
  **Complete when:** required display/source contracts use `thiserror` with
  truthful source relationships and appropriate narrow bounds; caller tests
  prove the cause and complete owned rejection survive. Do not reclassify
  ordinary rejections or settlements as fatal behavior errors.

- [ ] **A20 — Maintain a law-to-evidence ledger instead of a test-count claim.**
  **Coverage gap.** Existing model, fuzz, compile, and mutation evidence is
  spread across crate-local suites and historical audits. A filename or green
  suite does not identify which invalid implementation it rejects.
  **Complete when:** each law maps to a focused example, a complete trace or
  independent model, relevant wrapper orders, invalid-use fixture, boundary
  and ownership cases, and mutation/counterfactual evidence. Mark a layer
  inapplicable with a reason. Add coverage measurement to locate unexecuted
  code, but never use a percentage as proof of the law. Keep the workflow's
  fuzz-target list synchronized with the manifest; it is currently complete.

## Coverage map and what should remain distinct

This table locates the continuation work for every family. Existing evidence
is valuable even where A06 or A20 requires a stronger oracle.

| Area | Existing evidence inspected | Remaining audit obligation |
|---|---|---|
| Core actions, sending, source custody | `total_interpretation`, `action_interpretation`, source admission/custody and settlement tests | Complete-product assertions already exist; retain them across A11 and prove hostile source admission |
| Core addressing, creation, occurrences, event paths | creation/custody tests, child-occurrence product tests, generated creation tests, compile-fail rustdoc | A01/A14/A17; distinguish logical address, exact endpoint, and creator-local correlation |
| Machine and stash | `fsm_properties`, `error_paths`, `stash_properties`, `two_buffer`, fuzz sequences | Retain rollback and infallible replay laws; A09/A10/A18 |
| Composition and activation | `algebra`, `universal_layers`, `init_contract`, `compositions`, owner-scoped delivery | A03/A13/A14; prove consumer inference separately from structural interpretation |
| Stable proxy | `proxy`, `stable_proxy_shutdown_model`, operation-settlement tests, shutdown/replacement fuzz | A01/A07/A12/A17; preserve return/stop joins and retained affine activation data |
| Fixed supervision | initialization, recovery diagnostic, construction and protocol tests; four sequence fuzz targets | A03/A07/A12/A17; preserve ordered role policy, independent outcomes, and complete batch return |
| Dynamic supervision | `dynamic`, cancellation and shutdown fuzz targets | A03/A07/A12/A17; preserve key/generation/operation correlation and transferred cancellation |
| FIFO pool | split FIFO integration tests, independent admission-queue property, `fifo_pool_sequences` | A03/A06/A07/A12; prove fairness, retry placement, assignment custody, capacity, and termination independently |
| Keyed pool | binding, customer, assignment, lifecycle and compile tests; two keyed fuzz targets | A03/A06/A07/A12; preserve binding generations and per-role order separately from FIFO policy |
| Lifecycle and shutdown | shutdown models, heterogeneous shutdown, exact termination model, propagation sequences | A01/A17/A19; distinguish watch recurrence from exact-once monitoring and homogeneous from heterogeneous ownership |
| Time | receive-timeout model, timing invariants, init/composition tests, timer settlement tests | A06/A15; prove exhaustion and stale/duplicate input in both profiles; keep one-shot, periodic, deadline and inactivity policies distinct |
| Routing | catalogue models, routing/correlation invariants, exact reply tests | A06/A07/A11; distinguish sequencer gap closure from explicit watermark release and queue policy from delivery acceptance |
| Discovery | registry/topic models, presence fuzz, resolver/pub-sub unit tests | Assert snapshot order, stale versions, recipient identity, and complete rejected commands; retain read-only resolver authority |
| Operations and persistence | configuration/readiness/health/cache models | A06/A13; preserve health tombstones, fixed readiness membership, version conflicts, LRU ownership |
| Workflow | workflow invariants, barrier/latch tests, catalogue fuzz | Assert complete activations and terminal/stale cases; latch, reusable barrier, and dependency workflow have different laws |
| Macros and published consumers | parser permutations, behavior generation, facade fixture workspace | A02/A08; parser success and token text cannot substitute for consumer compilation |
| Testkit and quality tooling | driver properties, independent models, gate unit tests, Nix/CI definitions | A05–A10/A18/A20 |

Retain the pure effect boundary, concrete protocol sums/products, owned
rejections, sealed structural authority, explicit restart provenance, and
deterministic time inputs. A source scan found no dynamic-dispatch/type-erasure
escape hatch in the reviewed Rust implementation. Heap allocation, an `Arc`
correlation witness, or a long type is not independently evidence of waste.

## Completion method

Work in this order: repair evidence integrity (A04/A05/A08), resolve current
contract blockers (A01–A03), strengthen the relevant oracles (A06/A07/A17),
then remove proven duplication (A09/A11–A16/A18/A19).

For every retained semantic batch, record the actor-model, derived, or Bombay
policy law before editing; write the failing caller or transition regression;
identify the existing compositions it reuses or deletes; and complete the
AGENTS provenance and aggregate-drift checkpoints. Do not count this audit as
those future experiments' approval or evidence. Respect the repository's
cumulative surface checkpoints.

Every proposed deletion must answer: what law or consumer needs this code,
what breaks if it is removed, and which independent test detects that break?
Every proposed abstraction must identify repeated semantics it deletes and
prove two real substitutions. Shared test setup may be reduced; independent
oracles must not be implemented by calling the production decision logic.

## Verification record

### A19 pre-edit error-chain law

Classification: Rust caller contract and deliberate Bombay ownership policy.
When an aggregate error encloses a causal error, `Error::source` exposes that
exact cause. An ordinary unexpected-report rejection has no causal source and
retains its report; a machine failure retains its exact user event. The focused
`actors/tests/error_sources.rs` caller checks all three error types and the
owned values. It failed three tests on the previous implementations because
every source was `None`. The existing error sums and fields remain the complete
model; the edit changes formatting and source forwarding only. No actor state,
effect, interpreter action, wrapper order, or public type shape changes. The
aggregate-drift checkpoint is therefore unchanged for control states,
subordinate alternatives, branches, modules, and public spellings, with no
new arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or positional user syntax. Disposition: `pass` for the
pre-edit model; source implementation and caller verification follow.

The three existing error shapes now derive `thiserror::Error` and identify only
their enclosed cause as a source. Their manual `Debug` implementations still
avoid a `Debug` requirement on the owned event or report. The focused caller
suite passes all three cases, including `None` for ordinary unexpected-report
rejections and direct inspection of the retained event/report. No source or
payload type was added; manual display/error implementations were deleted.
Post-edit aggregate states, alternatives, branches, modules, and public
spellings remain unchanged. The residue scan and law cross-check remain as
recorded before the edit. Disposition: `pass`.

### A08 published consumer and A16 entry-point evidence

The macro consumer test explicitly selects the facade sibling example. A
temporary `compile_error!` appended to that example made the focused gate
fail with the deliberate diagnostic; the file was restored. The release-lane
package script now assembles all three publishable archives, then extracts
their actual contents and runs `cargo check` on a fresh consumer. Its manifest
uses the root README's dependency declarations and patches only the three
local archives so unpublished sibling packages resolve. The consumer compiles
both `#[behavior]` and `#[pool_worker]` from the archived libraries. The
archive build and consumer check passed. The Nix package check remains a
separate content-list gate because its sandbox has no registry access; it does
not claim to build an extracted archive.

The root and three crate READMEs now have current entry points, working local
links, and runnable commands. The root consumer dependencies select the
packaged versions. Repository link tests cover the READMEs; the core example
and macro consumer passed. Historical engineering verdicts now identify the
last committed evidence revision (`1f20cc4`) and point to this current audit.
The current root README no longer describes the repaired A03 projection as
open. These are documentation and verification changes: no actor control
state, effect lane, or public Rust spelling changed. Disposition: `pass`.

### A10 finite-driver contract

`drive` now explicitly documents that its successful `Trace` appends send and
creation lanes separately and loses turn boundaries. On a later controlled
error it returns only the cause, dropping the active behavior and successful
prefix actions; the mailbox keeps the suffix after the rejected input. A
success-success-error caller test confirms two successful transitions occurred
before the error and that one unconsumed event remains. Order-sensitive A01
creation tests use their own per-operation interpreter trace rather than
accumulating `Trace` effects. The focused error-path suite passed five tests.
This is a documented limited driver, not a runtime-ordering witness. No actor
or testkit state shape, effect lane, public spelling, or transition branch
changed. Disposition: `pass`.

### A18 benchmark workload and measurement

The protocol-matrix benchmark now activates the base behavior through the
same owned path as its composed cases. A preflight trace checks three exact
FSM phases (`A -> B -> A -> B`), accumulated state `6`, and an empty held
queue; the timed workload changes phase on every event and observes its final
state and phase. The clone measurement uses a 64-value state and 128 held
messages before timing, so it measures the actual rollback copy. The FIFO
benchmark keeps its complete submit/assignment/settlement/completion cycle
and passes the full customer outcome slice to `black_box`. Both programs now
start each sample from a fresh definition and report minimum, median, maximum,
and sample count. Matrix rates are transitions per second except clone
operations per second; FIFO reports completed cycles per second. The custom
benchmark lane no longer depends on Criterion. Three-sample low-iteration
preflights ran successfully for both programs; those timings are correctness
smoke checks, not performance conclusions. No actor law or state type changed.
Disposition: `pass`.

### A11 pre-edit product-equivalence law

Classification: derived product law and Rust interface cleanup. Buffer's
released target deliveries and factual outcomes have exactly the same two
named lanes as the existing routing `DeliveryOutcomes`: deliveries before
outcomes for interpretation and append, each lane returned intact on
corruption, the same source-custody admission, and the same ordered logical
host projection. Neither product owns a Buffer-only invariant; Buffer's queue
and overflow policy live in `BufferState`. The caller-level syntax should
therefore be `Behavior::Sends = DeliveryOutcomes<TargetSends, ReplySends>` for
Buffer just as for Sequencer and OrderGate. A focused type assertion in
`routing/buffer.rs` is the pre-edit regression. The existing product,
`settle_in_order`, tuple source custody, and wrapper composition are reused;
the duplicate public `BufferSends` implementation and its duplicate logical
projection will be deleted. No transition, owned effect, interpreter
operation, ordering, or error semantics changes.

Aggregate-drift checkpoint: Buffer's control state remains its queue and
policy product before and after, with no subordinate state/result alternative
or transition branch changes. The exact future-needed values remain the
queued owned payloads and reply routes, positive capacity, and overflow
policy. The proposed edit deletes a transparent send-product name and its
implementations without adding modules, states, branches, public spellings,
arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or positional consumer syntax. The source law is the
ordered effect-product equation in `actor-transition-algebra.md`; the
normalized FIFO and routing laws remain distinct and are cross-checked.
Disposition: `pass` for the model, pending the focused test and edit. This
does not declare all A11 derivation paths unified.

The focused Buffer caller failed before the edit with E0271: its associated
`Sends` was `BufferSends`, not the required `DeliveryOutcomes`. It passed after
the edit. The two products had the same public field names and six identical
laws (`SendEffects`, `SendsFor`, settlement classification/unattempted,
source custody, interpretation, and logical-host projection). Buffer now
uses the existing routing product; Sequencer, OrderGate, and other routing
templates already use it. The former `BufferSends` spelling and its duplicate
projection are gone; there is no compatibility alias. In the affected source,
`routing/buffer.rs` fell from 591 to 498 lines despite the new focused test,
and `requirements.rs` deleted ten net lines. Modules, control states,
subordinate alternatives, and transition branches remain unchanged; the one
deleted public type is the only public spelling delta. Every surviving state
alternative still retains its prior current values. The residue scan and
law-document cross-check found no new history, duplicated cause, cardinality
assumption, nested authority, semantic boolean, or structural consumer syntax.
Disposition: `pass` for this duplicate deletion. Broader A11 derivation work
remains open.

The remaining product paths have different type equations. This inventory is
the input to the next A11 design experiment; matching method names alone does
not justify merging them.

| Product path | Product and settlement shape | Ordered lanes and shared law | Distinct obligation |
|---|---|---|---|
| Former handwritten `DeliveryOutcomes`, `LeaseSends`, `PresenceSends` | Two generic fields; settlement reuses the same product with settled field types | Empty/append, left-before-right interpretation, unattempted suffix on corruption, source custody, classification, and logical projection | Their public domain field names differ; all three now use the shared private derivation. |
| Private `send_product!`, formerly `atomic::request_product!` | One or more generic named fields; settlement reuses the product | The same ordered operations and projection are generated together | Source custody must preserve every earlier settlement and every unvisited owned field across any arity. |
| `#[behavior]` generated sends | Generated fields may have concrete types; a separate generated settlement struct holds associated settlement types | Generated lane order, corruption suffix, and source custody use the same transition equation | Separate settlement representation and caller lane methods are part of the generated API; this path currently has no generated logical-host projection. |

The named generic products now share the private derivation and retain their
public field names and both wrapper orders. The generated product still needs
a lawful public projection witness before its distinct settlement shape can
share an implementation. This inventory itself changed no product or API.

### A11 generated-send projection law, before implementation

Classification: derived typed-composition law. A `#[behavior]` send product's
logical destinations are exactly the ordered, duplicate-preserving append of
its declared lane projections. A lane with no logical destination contributes
`NoBirthProtocols`; a nested interpreter-request lane contributes only its
declared logical protocols. The caller syntax is
`<Bootstrap as LogicalHostRequirements>::LogicalHosts`, without a handwritten
`LogicalDeliveryProtocols for BootstrapSends` implementation. Existing
`behavior/tests/behavior_generation.rs` has precisely that handwritten
implementation, so removing it is the focused pre-edit compile regression.
The expected product is `FirstDestination` followed by `SecondDestination`.
The macro already generates the named send lanes and settlement interpretation;
the implementation should reuse each field's `LogicalDeliveryProtocols` and
the existing `BirthProtocolProduct::Append`. No new runtime operation,
transition, wrapper, or host lookup is required. The caller must still compose
through existing wrapper projections in either order.

Aggregate-drift checkpoint: the generated behavior's control states,
subordinate alternatives, transition branches, production modules, and
public type spellings are unchanged. Each lane's current value remains in its
existing generated field. The proposed implementation adds one derived trait
impl to the existing generated product and deletes the handwritten witness;
it stores no history, duplicates no cause, asserts no cardinality, creates no
nested transition authority or semantic boolean, and exposes no positional
consumer path. The law is cross-checked with `actor-transition-algebra.md` and
the normalized logical-host contracts in this audit. Disposition: `pass` for
the pre-edit model, pending the focused failing regression.

Removing the handwritten `BootstrapSends` projection produced E0277 in the
Nix-pinned `behavior_generation` caller: both `BootstrapSends` and a second
generated product with a request lane and repeated delivery destination lack
`LogicalDeliveryProtocols`. An unconditional generated impl using the field
projections then failed E0446 for existing private interpreter-request types.
That candidate was removed, the test fixture restored, and the experiment
recorded in the root `DEAD_ENDS.md`. A11 remains open; a public
interface law must be established before another generated projection edit.

### A11 named generic product derivation, before implementation

Classification: derived ordered-product law and private implementation
consolidation. `LeaseSends<OutcomeSends, Schedules>` and
`PresenceSends<ReplySends, Schedules>` each own two named generic lanes. For
both, empty and append act lane by lane; interpretation visits the first lane
before the second, preserves the unattempted suffix on corruption, and
continues to the second lane after lawful rejection. Source admission visits
in the same order and returns the complete named product. Settlement status
combines both lanes, and logical-host projection appends their protocol
occurrences in order. The distinct public field names remain part of the
contract. This is the same law already derived by the private atomic
`request_product!` macro for generic named lanes. Moving that existing
derivation to the actor-crate root and giving it a domain-general send-product
name can delete the duplicate implementations; it adds no public framework.

Focused characterization before production edits passed in the Nix-pinned
toolchain: `requirements::tests::lease_and_presence_products_keep_both_wrapper_orders`
proves both logical orders through `SendLayer`; `total_interpretation`
checks Lease's corrupt suffix and Presence's rejection followed by its
independent schedule lane. These tests pass on the prior implementation
because this stage removes duplication without changing the observable law.
The existing four atomic request products and the core ordered settlement
and source-custody products remain the lower-order witnesses. The expected
design-stage files are the private derivation module, its four atomic import
sites, the actor crate root, Lease, Presence, their two projection impls in
`requirements.rs`, and the focused tests. The expected production delta is
roughly 200 fewer lines, with zero new or removed public types; later product
migration is a separate measured stage.

Aggregate-drift checkpoint: Lease and Presence control states, subordinate
alternatives, transition branches, modules by count, and public spellings
remain unchanged. The exact current values are Lease's owned outcomes and
schedule requests and Presence's owned replies and schedule requests. No
arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or positional consumer syntax is introduced. The ordered
send-product equation in `actor-transition-algebra.md` and the normalized
timing and presence contracts are cross-checked. Disposition: `pass` for the
pre-edit model and both focused caller witnesses.

The design stage moved the existing 273-line private macro from atomic
requests to the actor-crate send-product owner, changed its private name, and
retained field rustdoc. Lease and Presence now invoke it with their original
public field names. Their handwritten `SendEffects`, `SendsFor`, settlement,
source-custody, interpretation, and separately maintained logical-projection
impls were deleted. Both wrapper-order projections passed, all five focused
total-interpretation cases passed, and the actor crate's 158 unit tests passed
under `nix develop`. The retained production representation is 196 physical
lines smaller; focused tests add 56 lines, yielding 169 fewer actor `src`
lines including its new unit test. Public types and spellings, modules by
count, aggregate states, subordinate alternatives, and transition branches
are unchanged. Every surviving product field still owns its prior value.
The residue scan and law-document cross-check remain as recorded above.
Disposition: `pass` for the design stage. Other generic products are a
separate mechanical migration; the distinct proc-macro-generated settlement
shape remains an open A11 design question.

The measured mechanical stage applies only the proven syntax to
`DeliveryOutcomes`, `WorkQueueSends`, `BreakerSends`,
`TerminalPropagationSends`, and `ProxyEffects`, and removes their matching
separate projection impls. Each has generic named fields, the same ordered
interpretation/source-custody/settlement equation, and an existing real
consumer. Expected production delta is roughly 800 fewer lines, with zero
new or removed public types. No new effect lane, bound, state, or fixture is
authorized by this migration; a mismatch reopens the derivation instead of
adding a one-off branch.

The mechanical stage now uses the shared derivation at those five sites and
deletes their former implementations. `ProxyEffects` retains all seven public
field names and their declared order while losing its nested tuple source
plumbing. The actor `src` tree changed by +111/-1,104 physical lines, net
-993 including the focused unit test; the production representation alone is
about 1,020 lines smaller. No public type was added or removed. The actor
all-target Cargo check, formatter, 812 workspace Nextest cases, and current
workspace coverage run passed under the Nix toolchain. The full 21-check Nix
flake gate also passed from a clean worktree at signed commit `65caa65`.

Post-migration aggregate-drift checkpoint: the affected routing, timing,
discovery, lifecycle, and stable-proxy control states, subordinate result
alternatives, and transition branches are identical before and after; no
aggregate transition function changed. The current values in every surviving
send alternative are the same owned effect lanes, including all seven stable
proxy lanes. Modules remain constant by count because the derivation module
moved from `atomic/` to the actor root. Public spellings remain constant. The
residue scan found no new arrival history, repeated cause, false cardinality,
nested transition authority, semantic boolean, or positional consumer syntax.
The ordered product law in `actor-transition-algebra.md` and the normalized
atomic, routing, timing, discovery, and lifecycle contracts were cross-checked.
Disposition: `pass` for the retained representation.

### A13 pre-edit protocol-bound law

Classification: derived Rust protocol identity. A cache or resolver recipient
names an address and a command type without running a transition or copying a
binding definition. Thus `Protocol` for `Cache<K,V>` and `Resolver<K>` does
not require `K: Clone + Eq` or `V: Clone`; those laws are needed by the
respective transition or borrowed construction operations. The caller syntax
in `actors/tests/protocol_bounds.rs` names each actor protocol with non-Clone,
non-Eq payload types. It is the focused compile witness and must fail on the
prior bounds. The existing `MessageProtocol`, `Recipient`, `CacheMessage`, and
`ResolverMessage` products are reused. No new trait, alias, constructor,
runtime port, wrapper obligation, or effect lane is proposed.

Aggregate-drift checkpoint: cache and resolver state, protocol sums, results,
transition branches, production modules, and public spellings stay unchanged.
The future-needed cache values are its ordered entries, capacity, and exact
keys/values; the resolver needs its immutable key-recipient bindings. No
alternative is added or deleted. The residue scan finds no proposed arrival
history, duplicated cause, false cardinality, nested authority, semantic
boolean, or positional syntax. The persistence and discovery contracts and
the `Protocol`/`Behavior` separation in `actor-transition-algebra.md` are
cross-checked. Disposition: `pass` for the narrower protocol law, pending the
pre-edit regression and implementation.

The caller failed on the prior implementation with five E0277 diagnostics
requiring `Clone`/`Eq` on protocol-only payloads. The same caller passes after
removing those bounds from the two `Protocol` implementations; the bounds
remain on actual transition and borrowed-construction operations. No type,
module, aggregate control state, subordinate alternative, transition branch,
or public spelling changed. The residue scan and law cross-check remain as
recorded before the edit. Disposition: `pass` for these two bound repairs.
The wider hidden-export and extension-port inventory in A13 remains open.

The public `FifoError` and `KeyedError` sums are application-visible aggregate
failure contracts, and `FixedBuilder` is the inferred application
construction value. The trusted core `initialize` and `delegate_transition`
ports and the `RoutedCreation` child-host value are required by wrapper or
interpreter authors. Their existing rustdoc describes the relevant trust and
custody rules; the `#[doc(hidden)]` markers were removed so readers can find
those contracts. This changes documentation visibility only. Other hidden
generated obligations and runtime ports still need the full ownership
classification before A13 can close.

The follow-up inventory found a second hiding site for `KeyedError` and
`FixedBuilder`: their `atomic` re-exports still shared `#[doc(hidden)]` groups
with interpreter products. They now belong to the visible re-export groups.
The Nix-pinned Rustdoc build succeeded, and
`target/doc/behavior_actors/atomic/index.html` contains direct public links to
both items. The [public-surface inventory](public-surface-inventory.md)
classifies the remaining annotation sites. Trait implementor ownership and a
repeatable compile-cost comparison remain open before A13 can close.

### A13 FIFO aggregate error export, before implementation

Classification: deliberate Bombay public error policy, not an actor-model law.
`FifoError` is the FIFO aggregate's complete transition-failure sum, with
distinct `InitializationUnavailable` and `WorkerCreationsExhausted`
alternatives. An application that owns FIFO initialization must be able to
match those alternatives without naming the private `fifo_pool` module, just
as it can match the sibling `KeyedError`. The intended caller syntax is
`behavior_actors::atomic::FifoError` in an exhaustive match. The external
`actors/tests/protocol_bounds.rs` fixture will first require that path and
must fail before the re-export; then it will pass after the smallest
`atomic` visibility edit. The runtime-facing `Behavior::Error` remains the
same type and no failure or ownership transition changes.

Aggregate-drift checkpoint: FIFO control states remain `Constructed`,
`Operating`, `Draining`, `Stopped`, and `ForcedRetirement`. Subordinate states,
result alternatives, transition branches, and module count are unchanged by
the proposed re-export. The retained current values are the prepared workers,
operating members/backlog/cursor, draining workers/deadline, and terminal
forced-retirement members/cause in their existing variants. A12 still owns the
question of whether every terminal field is needed. Public
spellings increase by one, with zero public types added or removed. The edit
adds no arrival history, repeated cause, false cardinality, nested transition
authority, semantic boolean, or structural user syntax. It is cross-checked
with `actor-transition-algebra.md`, the FIFO law, and the existing aggregate
error vocabulary. Expected files: `atomic/mod.rs`, the external compile
fixture, and this audit; expected production line delta is zero or one.
The external caller failed before the edit with E0432 for the inaccessible
`behavior_actors::atomic::FifoError` path and passed after the one-spelling
re-export. The production diff is +1/-1 line; no type, state, alternative,
transition branch, or module changed. The existing two alternatives and their
error displays are unchanged. The Nix-pinned Rustdoc build passed and the
public `atomic` index links `FifoError` alongside `KeyedError` and
`FixedBuilder`. The residue scan and law cross-check remain as
recorded above. Disposition: `pass` for this export repair; A13's wider trait
ownership and compile-cost review remains open.

### A13 authored-role and logical-product documentation, before edit

Classification: Rust caller contract and derived typed-composition policy.
A manually authored direct-child role lawfully implements `ChildRole` and
`ChildOccurrence` with `DeclaredChildOccurrence`; an application host can
constrain the exact logical-host product with `BirthProtocolProduct`. Both
are existing external caller paths in `established_capabilities.rs` and
`logical_host_requirements.rs`. Their public proof names and the required
`ChildOccurrence::Resolution` associated type must be discoverable in
Rustdoc. Before editing, a Nix-pinned `cargo doc -p bombay-behavior --no-deps`
build succeeded but the crate index omitted `DeclaredChildOccurrence` and
`BirthProtocolProduct`, and the declared-occurrence page was absent. The
private `Recipient::new` has an ineffective `#[doc(hidden)]` marker to remove.
The intended edit changes documentation display only; it adds no alias,
default, bound, constructor, implementor, actor transition, or runtime port.

Aggregate-drift checkpoint: all actor control states, subordinate alternatives,
transition branches, production modules, and public spellings are identical
before and after. The future-needed role values remain its declared parent,
child behavior, and structural position; the logical product retains its
ordered protocol occurrences. Four ineffective or misleading documentation
markers are removed, with no arrival history, repeated cause, false
cardinality, nested authority, semantic boolean, or structural user syntax.
The actor-transition, creation, and logical-host laws are cross-checked.
Disposition: `pass` for the model; Rustdoc visibility and external caller
witnesses still need post-edit verification.

The four markers were removed as modeled. The core annotation count fell
from 22 to 18; no visibility modifier, trait bound, type, implementation,
aggregate state, branch, module, or public spelling changed. The Nix-pinned
Rustdoc index now links both `DeclaredChildOccurrence` and
`BirthProtocolProduct`, and both documentation pages exist. The
`established_capabilities` and `logical_host_requirements` external caller
suites passed 20 tests, including manual nominal occurrence and generic
logical-host use. The residue scan and law cross-check remain as recorded
above. Disposition: `pass` for the documentation repair. A13's wider trait
implementor and compile-cost review remains open.

### A15 coding-rule progress

Timer-domain tests now call the mutating `accept` operation before assertions,
so optimized and debug test profiles execute the same transition. Generated
readiness cases use the closed `ReadinessStatus` sum rather than a semantic
boolean. The pool-private values formerly named only `Operating` are now
`FifoOperating` and `KeyedOperating`; each remains under its existing root
state variant with no state or branch change. The nested `.sends.inner.inner`
uses in `testkit/tests/compositions.rs` are structural composition tests that
explicitly assert nesting order. All 92 rustdoc import lines across 26 source
files now use qualified paths or were removed where the negative example
tested only a missing alias. `scripts/check_rustdoc_imports.py` enforces that
rule in the Nix documentation gate; a hidden-import counterexample fails the
check. Core delivery compile-fail examples now require only `Protocol`, the
capability they actually test, rather than an inert `Behavior` fixture.
Direct `rustc --error-format=json` probes of the three logical-delivery
examples produced only `E0308`; replacing the wrong protocol, address, or
payload with the declared one compiled in each case. Rustdoc's
`compile_fail,E0308` tag alone does not enforce that error code, so this
counterfactual check supplies the failure-reason evidence. A direct compiler
check now validates the expected diagnostic for all 45 tagged compile-fail
snippets in the Nix documentation gate; it found and corrected five stale
tags. The optimized timer-domain tests passed both cases, and
`nix flake check` passed all ten checks on `aarch64-darwin`, including the
documentation and doctest gates. The post-check edits in this audit batch
only add the A11 inventory and this verification result; its Rustdoc examples,
scripts, and Nix gate definition are the checked snapshot. Disposition: `pass`.

### A07 actor mutation evidence: routing-buffer capacity

At revision `0274dab`, a focused campaign mutated the bounded buffer's
below-capacity guard (`routing/buffer.rs:261`). The law is Bombay's declared
overflow policy: an offer below capacity is retained, while an offer at
capacity follows the configured rejection or eviction branch and preserves
ownership of every value. The actor unit tests and the independent FIFO and
overflow property in `behavior-testkit/tests/routing_invariants.rs` are the
relevant witnesses.

The command selected `bombay-behavior-actors`, filtered to the guard and its
comparison operators, and set `--test-workspace true --test-tool nextest
--no-shuffle --minimum-test-timeout 180 -- --profile mutants` with
`PROPTEST_CASES=32`. The mutation log confirmed `test_packages=All` for each
mutant. The unmutated baseline passed; all five selected mutants built and
were caught by actor buffer tests, primarily
`every_overflow_policy_preserves_or_returns_all_owned_values`. This is a
five-mutant routing-law result, not an actor-wide mutation verdict. A07 still
needs independently reviewable campaigns for the other actor families and a
separate viability ratchet.

### A07 stable-proxy activation admission experiment, before simplification

Classification: derived exact-correlation law. `BeginActivation` consumes one
`ActivationPermit` and issues an `ActivationAttempt` containing that permit's
`WorkerAttempt`. Every `WorkerActivation` constructor in
`atomic/worker/activation.rs` carries that same worker evidence alongside the
attempt; its fields are private, and the permit cannot be duplicated. Thus a
matching activation attempt already proves the matching worker for every
constructible input. The public transition still accepts only its exact
activation and returns a foreign input intact through the diagnostic lane.
The caller syntax is a normal `StableProxy::on(request.started())`, including
two independently constructed requests with equal worker payload and endpoint
values but different non-forgeable attempt tokens.

At signed revision `855458a`, the Nix-pinned campaign first selected the only
mutant in `stable_proxy/state.rs`; it was unviable because `ProxyPhase` has no
`Default`, so it supplied no mutation verdict. Its unmutated baseline ran 590
actor tests. The separate activation-guard campaign selected all five
mutations at `stable_proxy/mod.rs:314` and ran 702 actor/testkit tests per
mutant; it skipped a second baseline, while the full 812-test workspace run
had passed at this revision. Four were caught; changing `&&` to `||` survived.
The survivor falsified the claim that both comparisons are independently
necessary; existing tests already reject foreign activation. The focused
caller regression distinguished exact
attempt tokens despite equal payload and endpoint values. It passed before
and after the edit; the complete 51-test proxy recovery suite and optimized
focused witness also passed after it. The private guard now retains only the
exact activation comparison and deletes the redundant worker input. No public
type, effect lane, interpreter operation, wrapper, or actor-model law changed.
The existing activation and shutdown models remain the lower-order transition
witnesses.

Aggregate-drift checkpoint before the edit: stable proxy's control states are
`Dormant`, `Starting`, `Ready`, `EmptyInitial`, `EmptyAfter`, `Replacing`,
`ShuttingDown`, and `Stopped`; they remain the same. No subordinate state or
result alternative, production module, or public spelling changes. The only
transition branch affected is the exact activation admission predicate. Every
surviving state keeps the same current worker, attempt, activation plan, and
pending custody. The proposal stores no arrival history or repeated cause,
assumes no cardinality, adds no nested transition authority or semantic
boolean, and exposes no positional syntax. It is cross-checked with the
stable-proxy and atomic actor laws and the complete-effect rule in
`actor-transition-algebra.md`. Expected and actual files: this audit, one proxy
caller test, and `stable_proxy/mod.rs`. The production diff is +7/-11 lines,
net -4; the test adds 47 lines. Public types added/removed: 0/0. Control states,
subordinate alternatives, transition branch count, modules, and public
spellings remain unchanged. The exact retained current values and residue
scan remain as recorded before the edit. At signed revision `7163ec3`, the
Nix-pinned post-edit campaign ran a 591-test actor baseline and selected the
remaining viable equality mutation. The mutated build ran 703 actor/testkit
tests; six actor tests failed, so the mutant was caught. The unselected
function-wide replacement proposes `Ok(Default::default())`, which cannot
construct the returned affine activation. This is a stable-proxy
activation-law verdict, not an actor-wide result. The full Nix flake gate
passed from a clean worktree at `7163ec3`, including workspace Nextest,
Clippy, docs, doctests, formatting, dependency policy, and packaging.
Disposition: `pass` for the retained representation and focused mutation
slice.

### A07 actor mutation evidence: FIFO completion correlation

Classification: Bombay's creator-local correlation and assignment-custody
policy. A worker completion has exact assignment authority, while its outer
`ChildReport` separately names the child creation. The pool accepts a
completion only when both identify the current busy worker. A foreign child
report must not release the assignment or send a completed customer outcome.
At signed revision `0a40bdc`, a Nix-pinned campaign selected the three
guard mutations at `fifo_pool/mod.rs:276`: unconditional true, unconditional
false, and inverted child-ID equality. The baseline ran 591 actor tests; the
full Nix gate had passed on the same production code at `7163ec3`. Every
mutated command selected the 813-test workspace suite. All three mutants
built and were caught. `fifo_pool::delivery_and_completion_orders_complete_once`
caught unconditional acceptance; the FIFO correlation suite caught rejection
and inversion. No production edit was needed. This is one FIFO correlation-law
slice, not a full FIFO mutation campaign or an actor-wide verdict.

### A07 actor mutation evidence: keyed binding expectations

Classification: Bombay's keyed-binding compare-and-set policy. A binding
command carries the expectation it observed; a stale generation returns the
complete command and current expectation without changing placement. A
rebalance to the already bound role returns the existing binding. At signed
revision `0a40bdc`, a separate Nix-pinned campaign inverted the two guards
at `keyed_pool/mod.rs:1069` and `:1146`. Its baseline ran 591 actor tests;
mutated commands selected the 813-test workspace suite. Both mutants built
and were caught by the keyed construction-and-commands integration test,
which checks generations, owned rejection, and an independent directory
placement. No production edit was needed. This is a keyed binding-law slice;
other keyed and actor-family laws still require mutation review.

### A07 actor mutation evidence: dynamic cancellation authority

Classification: Bombay's keyed-operation correlation and affine authority
policy. Cancellation of a live service requires the exact current operation;
the actor returns a stale authority unchanged and retains the active entry.
At signed revision `f9f39dc`, a Nix-pinned campaign inverted the operation
comparison at `dynamic_supervisor/mod.rs:885`. Its baseline passed 591 actor
tests; the mutated build selected the 814-test workspace suite. The mutant
built and was caught by four dynamic integration tests, including
`accepted_start_cancellation_retires_before_fresh_key_reuse`. A separate
isolated counterfactual changed `!=` to `>`: current authority still passed
the guard, but an older authority for the same reused key was accepted. The
focused integration test failed exactly at its stale-after-reuse assertion
(`dynamic.rs:3309`). The counterfactual was reverted. No branch production
edit was needed. This is one dynamic-supervision law slice, not a full
family campaign. The full Nix flake gate passed all ten checks on `f9f39dc`
before the counterfactual; the latter ran in a separate detached worktree.

### A07 actor mutation evidence: fixed replacement correlation

Classification: Bombay's exact predecessor-correlation and returned-custody
policy. A fixed supervisor accepts a replacement outcome only while that
member awaits an outcome for the same predecessor worker. A foreign outcome
returns unchanged with the pending member; a matching outcome remains held
until the independent predecessor-stop leg resolves. At signed revision
`ebcfa9d`, a Nix-pinned campaign selected three mutations at
`fixed_supervisor/recovery/mod.rs:1876`: unconditional guard acceptance,
unconditional refusal, and inverted predecessor equality. Its baseline
passed 591 actor tests; each mutated build selected the 814-test workspace
suite. All three built and were caught. The foreign-outcome integration test
caught unconditional acceptance; valid replacement and coordinated-restart
tests caught refusal and inversion. No production edit was needed. This is
one fixed-supervision replacement-law slice, not a full family campaign.

### A07 child creation resolution regression, before test edit

Classification: Bombay's exact staged-creation correlation policy. A
`ChildShutdownPlan` may mark a declared child established only when both the
reported creation ID and creation kind equal the values stored in that child's
`Awaiting` state. A report with exactly one mismatched component returns the
complete report, leaves the child awaiting, and permits the later exact report.
The existing test used a replacement report with both components mismatched,
so replacing the `||` admission rejection with `&&` passed every actor test;
the workspace mutant was mislabeled caught only when unrelated macro fixture
tests timed out. A focused test will present wrong-kind/same-ID and
right-kind/wrong-ID reports independently before the exact birth. No
production type, bound, wrapper, interpreter port, transition branch, or
public spelling changes.

Aggregate-drift checkpoint: `Planning` remains `Collecting` or `Reported`;
each child remains `NotRequested`, `Awaiting { creation, kind }`, or
`Established { creation }`. The future-needed values are the declared
position, exact staged ID and kind while awaiting, and committed ID for plan
construction. Production states, subordinate alternatives, branches, lines,
modules, and public spellings are unchanged before and after this test-only
experiment. The residue scan finds no arrival-history state, repeated cause,
false cardinality, nested authority, semantic boolean, or structural user
syntax. This is cross-checked with `actor-transition-algebra.md` and
`atomic-runtime-settlement.md`. Disposition: `pass` for the regression model;
the focused counterfactual and baseline still need verification.

### A05 nested test-timeout verdict, before implementation

Classification: deliberate Bombay verification policy. A `CaughtMutant`
summary is not proof that a law assertion failed when the selected test runner
itself timed out a test and returned failure. The verdict must reject any
selected mutant whose Nextest log contains a timed-out test, even if another
test failed, and must require inspectable log evidence for every claimed
caught mutant. A focused gate fixture will submit a complete, otherwise valid
campaign with `CaughtMutant` and a Nextest `TIMEOUT` line; the prior gate
accepts it. The implementation will reuse cargo-mutants' `log_path` and
existing `Outcome`/candidate identity, and the Nix mutation profile will let
the outer cargo-mutants timeout classify genuinely stalled commands. No actor
algebra, aggregate state, transition, interpreter effect, public Rust
spelling, or wrapper changes.

Aggregate-drift checkpoint: actor control states, subordinate alternatives,
branches, production lines, modules, and public spellings are unchanged; the
gate adds only report validation and removes the runner's premature timeout.
No arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or structural user syntax enters actor code. The gate law
is cross-checked with the A05 report-identity law above and the Nix mutation
derivation. Disposition: `pass` for the pre-edit model; the failing fixture
and retained verdict still need verification.

The adversarial gate fixture failed on the prior implementation: `check`
returned `Ok(())` for a complete `CaughtMutant` report whose only Nextest
evidence was `TIMEOUT`. The repaired gate requires a safe relative log path
and a `FAIL` test line for each caught mutant, rejects any `TIMEOUT` line even
alongside a failure, and applies the same validation when seeding a baseline.
The mutation Nextest profile no longer terminates individual tests after ten
seconds; cargo-mutants owns the command timeout and reports `Timeout` to the
strict verdict. All 12 gate tests, targeted Clippy, and formatting passed.
The actual three-mutant child-creation campaign passed the repaired verdict
(`3 viable / 3 total`) with three test failures and zero timeouts. The gate's
production portion grew from 252 to 294 lines; its test portion added 62 net
lines. The actor file added 46 test lines and no production lines; the Nextest
profile shrank from six to four lines. Aggregate states, alternatives,
branches, modules, and public spellings remain unchanged. The residue scan
and law cross-check remain as recorded above. Disposition: `pass` for this
verification repair. The full Nix flake gate passed all ten
`aarch64-darwin` checks on signed revision `81bda41`.

### A07 actor mutation evidence: catalogue correlation and admission

At signed revision `337bc2e`, one Nix-pinned campaign selected ten mutations
across registry unbinding, child creation settlement, configuration version
conflict, one-shot timer admission, and barrier duplicate arrival. The baseline
passed 591 actor tests; mutated commands selected the 814-test workspace.
Every mutant built and cargo-mutants labeled every one caught, but one label
was false evidence: the child-creation `||` to `&&` mutant passed all actor
tests, while four unrelated macro fixture tests hit the ten-second Nextest
timeout. The other nine mutants had named actor test failures. The focused
child-creation regression on `81bda41` passed unmutated and failed under
`||` to `&&` at the wrong-kind/same-ID assertion. A separate Nix-pinned,
actor-only rerun selected all three current creation-correlation mutations;
its 593-test baseline passed, all three mutants built and failed actor
assertions, and the strengthened gate accepted its complete report with no
timeout, miss, or unviable candidate. This closes the false-positive slice;
other catalogue laws still need review, so A07 remains open.

| Family and law | Original mutation result with a real actor oracle |
|---|---|
| Registry: exact recipient required to unbind | One inversion caught by `discovery::registry::tests::mutations_are_atomic_and_stale_unbind_is_typed`. |
| Child shutdown: exact creation ID and kind | Two comparison inversions caught by child-shutdown tests; the initially false `||` to `&&` verdict was corrected by the independent-component test and actor-only rerun. |
| Configuration: same-version equality | One inversion caught by `operations::configuration::tests::stale_and_conflicting_candidates_return_ownership_atomically`. |
| One-shot timer: ID and generation admission | Guard-true, guard-false, `&&` to `||`, and ID-equality inversion all caught by `time::one_shot::tests::initialization_schedules_then_matching_generation_fires_once`. |
| Barrier: duplicate participant arrival | One inversion caught by `workflow::barrier::tests::generation_releases_exact_membership_in_arrival_order`. |

### A07 actor mutation evidence: state and ownership

Classification: derived state-transition and returned-custody laws. A machine
replays held messages when `Goto` changes phase, a stash releases its held
FIFO when its route admits delivery, and a cache evicts the oldest entry only
when an absent key enters a full cache. Replacing an existing key returns its
old value without evicting another entry. At signed revision `c3a54b3`, a
Nix-pinned actor-only campaign selected four mutations: machine phase equality,
stash drain removal, cache `&&` to `||`, and cache capacity equality. Its
baseline passed 593 actor tests. All four mutants built and failed named
actor tests, with zero misses, timeouts, or unviable candidates. Machine and
stash failures came from `algebra`; both cache mutations failed its recency
and replacement-custody tests. The repaired mutation verdict accepted the
complete report (`4 viable / 4 total`). No production edit was needed. These
are three law slices; the actor-wide 3,125-candidate inventory is not claimed
as fully tested.

### A20 ledger entry: stable-proxy activation correlation

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | `proxy_command_recovery::equal_worker_values_and_endpoints_do_not_share_activation_authority` checks every effect lane, unchanged phase, exact diagnostic return, admission by the original owner, and admission of the target's own activation. |
| Independent trace and composition | `stable_proxy_shutdown_model` explores activation, return, stop, and shutdown order; `stable_proxy_owner_composition` checks owner projection. These models do not themselves forge inconsistent worker and activation evidence. |
| Invalid construction | `ActivationPermit` is affine and has a compile-fail duplication example; `WorkerActivation` has private fields and constructors that couple worker and activation evidence. The application cannot synthesize the inconsistent pair required by the surviving pre-edit `&&` to `||` mutation. |
| Counterfactual | Four of five pre-edit guard mutants were caught. The `&&` to `||` survivor exposed a redundant worker comparison, which was removed. The post-edit equality inversion was caught by six actor tests; the function-wide default replacement was not selected. |

### A20 ledger entry: FIFO completion child correlation

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition | `fifo_pool::delivery_and_completion_orders_complete_once` places completion before and after the assignment receipt, then wraps an authorized completion with a foreign child ID. It checks no customer completion, one diagnostic, and later return of the still-assigned job during shutdown. |
| Independent trace | `fifo_pool::correlation::completed_work_advances_and_wraps_worker_selection` checks completed work and worker selection across roles. The FIFO model and fuzz targets cover broader ordering; this mutation slice does not certify them. |
| Composition and invalid construction | The completion authority is issued only from an assignment, but a `ChildReport` can carry a different creator-local child ID. The pool must check both. This is an aggregate ingress decision, not a wrapper-order transformation. |
| Ownership limit | The public `FifoDiagnostic` is intentionally opaque. The test checks diagnostic cardinality and that shutdown returns the still-assigned job, but it cannot inspect the exact foreign completion inside the terminal diagnostic. That custody seam remains for A17/A20. |
| Counterfactual | All three guard mutants at `fifo_pool/mod.rs:276` built and were caught; viable selection and detection are recorded separately. |

### A20 ledger entry: keyed binding expectations

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition | `keyed_pool::compile::keyed_construction_and_commands_need_only_domain_types` exercises absent, exact, stale, same-role, cross-role, and removed bindings with distinct request IDs and generation values. |
| Independent trace | The test compares placement to a separate account directory; keyed pool assignment and lifecycle tests and keyed fuzz targets cover other transitions, not every binding counterfactual. |
| Composition and invalid construction | `BindingExpectation` is an exhaustive absent-or-exact sum; stale rejection returns the complete command and current generation. This is an aggregate binding decision, not a wrapper-order transformation. |
| Counterfactual | Both selected guard inversions in `keyed_pool/mod.rs` built and were caught by the binding integration test. The remaining keyed aggregate laws still need mutation slices. |

### A20 ledger entry: dynamic cancellation authority

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | `dynamic::accepted_start_cancellation_retires_before_fresh_key_reuse` checks current cancellation, returned authority and submission, exact retirement before key reuse, and the old authority's stale reply after reuse. It checks all send lanes and the continuing verdict for replayed and stale cancellation. |
| Independent trace | Dynamic cancellation and shutdown fuzz targets explore orderings. The focused integration trace has concrete expected replies but is not an independent state model. |
| Composition and invalid construction | `CancelAuthority<Key>` carries the key and operation; the actor compares it with the entry's current operation. This is a keyed aggregate decision, not a wrapper-order law. The authority is returned by start or replacement receipt and retained in cancellation replies. |
| Counterfactual | Inverting `!=` to `==` built and failed four dynamic integration tests. Replacing `!=` with `>` in an isolated worktree left current-token cancellation intact but made the old token after same-key reuse pass; the focused test failed at the stale assertion. These checks do not certify all dynamic entry phases. |

### A20 ledger entry: fixed replacement correlation

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | `fixed_supervisor_initialization::replacement_rejects_an_outcome_for_another_predecessor` checks the complete foreign outcome returned through the diagnostic path. Valid receipt and coordinated-restart tests check a matching outcome waits for the independent predecessor stop. |
| Independent trace | `fixed_supervisor_initialization` explores 90 lawful replacement arrival orders across recovery strategies. This is a local typed interpreter trace, not the downstream host required by A17. |
| Composition and invalid construction | The outcome and pending replacement each carry exact predecessor evidence; the guard compares those values after the input settlement advanced to outcome-pending. A foreign predecessor can be reported by a runtime, so this is an aggregate ingress check rather than a static invalid-construction case. |
| Counterfactual | Guard-true, guard-false, and equality-inversion mutants at `recovery/mod.rs:1876` all built and were caught by fixed-supervision integration tests. This slice does not cover scheduling, policy selection, or terminal retirement. |

### A20 ledger entries: catalogue admission laws

| Law | Focused transition and broader witness | Boundary and counterfactual limit |
|---|---|---|
| Registry stale unbind | The registry unit test returns the stale recipient and keeps its binding; discovery model tests cover broader bind/lookup sequences. | Recipient equality is a runtime command condition, not a compile-denial capability. One comparison inversion was caught; snapshot ordering is outside this slice. |
| Child creation ID and kind | `creation_resolution_requires_matching_id_and_kind_independently` returns each single-component mismatch with exact expected values and then accepts the lawful birth. The surrounding shutdown plan exercises nested event paths. | Runtime reports can carry mismatched typed facts. The `||` to `&&` counterfactual failed this test, and all three actor-only mutants were caught; the earlier workspace timeout was rejected as evidence. |
| Configuration version conflict | `stale_and_conflicting_candidates_return_ownership_atomically` checks stale version and same-version value conflict; catalogue models cover update sequences. | This is a runtime version decision. The same-version comparison inversion was caught; no downstream persistence claim is made. |
| One-shot timer admission | The one-shot unit trace checks initialization scheduling, matching generation, and exact-once firing; timing composition tests check wrapper use. | A timer ID or generation mismatch is a typed runtime input. Four guard mutants built and failed the focused test; broader periodic and deadline laws are separate. |
| Barrier duplicate arrival | The barrier unit trace checks exact membership and arrival-order release; workflow invariant tests cover reusable generations. | A repeat arrival is an ordinary command rejection. One equality inversion was caught; it does not certify the future/stale-generation branches. |

### A20 ledger entries: machine, stash, and cache

| Law | Focused transition and broader witness | Boundary and counterfactual limit |
|---|---|---|
| Machine phase replay | `algebra::fsm_is_receive_plus_become_policy` observes phase change and held-message replay; `fsm_properties` separately exercises self-`Goto`, generated sequences, and rollback. | Inverting phase equality built and failed the changed-phase algebra test. The self-`Goto` oracle is separate; this mutation slice does not test every error rollback branch. |
| Stash FIFO release | `algebra::stash_release_delivers_the_trigger_then_drains_the_held_fifo` checks trigger-first delivery and held FIFO order; `stash_properties` covers generated hold/release sequences. | Replacing `drain_into` with a no-op built and failed the algebra test. The statically infallible inner behavior is the applicable compile-time constraint; runtime route choices remain typed. |
| Cache replacement and eviction | Cache unit tests check recency, full-capacity eviction, and complete replacement/removal custody; `catalogue_invariants` compares generated operations with an independent ordered-entry model. | Zero capacity is rejected at construction. Both capacity-condition mutations built and failed cache tests; this slice does not certify other persistence mechanisms. |

### A20 ledger entry: bounded-buffer capacity and ownership

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition | `routing/buffer.rs` tests zero capacity, FIFO release and empty outcomes, and every overflow policy with distinct values. |
| Independent trace | `behavior-testkit/tests/routing_invariants.rs::buffer_preserves_fifo_and_returns_every_unaccepted_value` compares retained FIFO values and returned custody after each generated command; it does not claim transport admission. |
| Composition | The buffer is a standalone `Behavior` whose destination lanes are concrete `DeliveryRoute`s. There is no wrapper-order law for its capacity decision; route typing is checked separately in `composition/delivery_route.rs`. |
| Invalid construction and boundaries | `BufferConfiguration::new(0, ..)` returns `ZeroCapacity`; a runtime numeric capacity is validated at construction. The full-queue tests distinguish `Reject`, `DropNewest`, and `DropOldest` ownership. |
| Counterfactual | The five guard/operator mutants in the A07 slice were all caught by actor buffer tests. This evidence covers this capacity branch only. |

The remaining catalogue laws need equally specific entries, so A20 remains
open.

### A20 Nix coverage measurement

The flake now provides `nix run .#coverage -- --lcov --output-path
target/coverage.lcov`, using the pinned Rust toolchain, its LLVM tools, and
Nix-provided `cargo-llvm-cov`. The default development shell also includes
`cargo-llvm-cov`. A full `--workspace --lib --tests --locked` run passed after
the A11 migration and produced `target/coverage.lcov`; the macro fixture
integration tests ran too. Instrumented source-line observations were: core
1,584/1,834 (86.4%), actors 25,438/30,848 (82.5%), macros 727/899 (80.9%),
testkit 70/73 (95.9%), and mutation gate 309/348 (88.8%). The actor files
with the least measured execution among files of at least 30 instrumented
lines include stable-proxy state (15/71), the shared send-product derivation
(21/46), dynamic-supervisor event (21/43), dynamic-supervisor entry retirement
(43/85), stable-proxy protocol (46/87), and delivery-route composition
(81/149). Some generic code depends on which concrete products are
instantiated, so these counts identify review targets, not necessarily missing
runtime transitions. A20 remains open until the law ledger and
counterfactuals cover the full catalogue.

### Repair ledger: dependency resolution and mutation verdicts

Before production edits, the blockers are A02 and A05. The macro law is a
derived Cargo/Rust naming contract: generated paths must name the actual extern
crate for ordinary package declarations and explicit renames. An external
consumer with both direct dependencies and the facade is the focused witness;
its unrenamed `#[behavior]` and `#[pool_worker]` uses must compile, while the
existing renamed and missing-dependency witnesses retain their outcomes. The
smallest expected production change is in `behavior-macros/src/lib.rs` (roughly
10 lines), with a fixture manifest/source, its lockfile, and the fixture test
harness. No public types are added or removed; generated behavior and actor
products and the existing facade resolution are reused.

The mutation-gate law is deliberate Bombay verification policy: exactly one
successful baseline and exactly one terminal, valid outcome for each discovered
mutant identity are required before a coverage verdict. A report missing its
baseline, duplicating an outcome, or reporting `Failure` must fail even if its
function-level viability floor is met. Focused adversarial report tests will
fail on the current gate before changing it. The expected production edit is
`mutants-gate/src/main.rs` (roughly 45 lines); the report's own mutant name and
existing function-floor products are reused. No public types are added or
removed. Neither repair changes actor transitions or interpreter effects.

Initial cumulative scope includes this audit and its two index links: three
changed documentation files. The macro fixture required its own package because
Cargo rejects the same package under renamed and unrenamed keys in one manifest;
the repair adds seven macro and gate files rather than the estimated five.
This ledger will be updated with measured
production/test/API deltas at the next checkpoint.

### Repair batch result on `codex/repository-quality-repairs`

- **A02 complete.** Both unrenamed direct package declarations compile the
  ordinary `#[behavior]` and `#[pool_worker]` expansions in an external
  consumer. The direct-plus-facade fixture now also uses a renamed direct
  actors dependency and exercises both entry points. Facade-only,
  renamed-facade, facade sibling example, and missing-dependency fixtures
  retain their outcomes. All four `crate_resolution` tests passed after the
  fixture lockfile was regenerated offline. A later Nix run exposed that the
  nested lockfile had selected four registry versions absent from Nix's
  vendored root lockfile. The nested `indexmap`, `syn`, `toml_edit`, and
  `unicode-ident` versions now match the root lock; all four macro consumer
  tests passed again under `--offline --locked`.
- **A04 complete.** The adapter denial now supplies a current `ChildDelivery`
  and a compiling `Recipient` counterpart. The activation permit denial has
  the required generic bounds and a compiling single-transfer counterpart.
  The heterogeneous birth denial has the interpreter's real future signature;
  an isolated compiler probe produced E0277, and the same source compiled
  after adding the missing concrete child host. Filtered doctests passed.
- **A05 complete.** The gate reads the cargo-mutants `name` field as candidate
  identity, requires exactly one successful baseline and one valid outcome per
  selected identity, rejects unknown or duplicate identities and malformed
  reports, and keeps function-level viability floors. Debug and release unit
  tests passed. Direct CLI probes returned exit 0 for a complete clean report
  and exit 1 for missing baseline, failed mutant, duplicate outcome, and an
  invalid `Success` mutant; `emit-baseline` produced the expected viability
  inventory.
- **A08 and A16 partially repaired.** The facade sibling example is explicitly
  selected by the macro consumer gate. The root README now has current local
  links, package versions, and interpreter trait names. Its code example
  compiled as an external offline path-dependent consumer, and a repository
  test checks its local document targets. Published package verification and
  the remaining entry-point and historical-document review stay open.

Scope checkpoint after this batch: 15 changed files; production
`+87 / -67 / net +20` lines; tests, fixtures, and doctests
`+295 / -43 / net +252` lines; public API `+0 / -0` types. These counts use
line comparisons with inline test modules separated from production. The
positive production delta is new verdict validation, not a code reduction.
No actor aggregate control state, subordinate alternative, transition branch,
module, or public spelling changed; the aggregate-drift checkpoint has the
same before and after representation. The residue scan found no new arrival
history, repeated cause, cardinality assumption, nested transition authority,
semantic boolean, or positional user syntax in this batch. Its disposition is
`pass` for these tooling and fixture repairs; it makes no claim about A01/A03
or the catalogue simplification candidates. Actor-transition and normalized
atomic laws were cross-checked for scope, with no actor transition changed.

### A03 pre-production projection law and failing caller

Classification: this is Bombay's derived static hosting requirement, not an
actor-model transition law. A concrete send product must expose, in declared
interpretation order, every protocol that one of its values can address
logically. The type-level product must retain repeated occurrences. An
`InterpreterRequests<CustomerDelivery<P>>` lane can address `P` logically even
when a particular value selects its exact variant, so its projection contains
one `P`. A `DiagnosticAction<Recipient<P>, Diagnostic>` can similarly target
`P`; an established-only or terminal-only route adds none. A wrapper appends
its inner requirements before its owned requirements, and transitive child
requirements follow the actor's sends. This projection is evidence only; it
cannot perform routing, allocation, or any `Actions` effect.

The pre-edit external consumer uses the public
`LogicalDeliveryProtocols::Protocols` syntax. It requires
`InterpreterRequests<CustomerDelivery<CustomerProtocol>>` to equal
`BirthProtocol<CustomerProtocol, NoBirthProtocols>` and requires
`FifoRequests<NoSends, ...>` to implement `LogicalDeliveryProtocols`.
`cargo check --offline` failed for the two intended reasons: the first product
is currently `NoBirthProtocols`, and the FIFO product has no implementation.
An independent diagnostic caller required
`InterpreterRequests<DiagnosticAction<Recipient<DiagnosticProtocol>, ()>>`
to contain that protocol while its `EstablishedRecipient` counterpart remained
empty. It failed only the logical-route assertion, confirming that the defect
is the blanket empty request projection rather than the exact-route case.
Focused assertions now live in
`behavior-testkit/tests/logical_host_requirements.rs`. Before production edits,
`cargo test -p bombay-behavior-testkit --test logical_host_requirements --no-run`
fails on the customer request, logical diagnostic request, and both wrapper
orders; the exact diagnostic counterpart remains the empty control case.

The existing lower-order pieces are `LogicalDeliveryProtocols`,
`BirthProtocolProduct::Append`, `SendLayer`'s inner-to-outer ordering,
`BirthNodeLogicalHosts`, `InterpreterRequests`, the four atomic request
products, and `ProxyEffects`. Customer delivery and diagnostic routing are
already interpreted through typed request lanes. No interpreter action or
acceptance contract needs to change. A candidate design would give request
items a separately owned logical-protocol projection, then make
`InterpreterRequests<M>` project `M` rather than always returning empty. It
must prove two real nonempty request families, all five atomic families,
transitive children, and two wrapper orders without mandatory placeholder
inputs.

The selected public contract extends the existing `InterpreterRequest` seam
with an associated logical-protocol product. Each request already owns its
return continuation and possible recipient; a separate public trait would
split one request law across two implementations. The caller writes the same
`InterpreterRequests<Request>` syntax; only static
`LogicalHostRequirements::LogicalHosts` evidence changes. Host-free schedule,
observation, exact lifecycle, and parent-report requests select
`NoBirthProtocols`. `CustomerDelivery<P>` selects `P`; diagnostic route
capability selects logical `P` or empty. The request-product derivation appends
authored field products in interpretation order; `ProxyEffects` does the same.
Existing `BirthProtocolProduct::Append`, wrapper ordering, transitive birth
projection, request interpretation, and all runtime ports are reused. No
transition, rejection, effect, or custody value changes.

Pre-edit shape checkpoint: the five atomic actor control-state sums and their
subordinate alternatives remain exactly as declared; this batch changes no
state or result variant, transition branch, or source module. The affected
implementation files currently total 6,619 physical lines across 11 files;
the five atomic aggregate trees have 37 module declarations and 76 enum or
interpreter-request declarations by source scan. Public type and trait names
remain unchanged; one associated type is added to the existing request port.
Every surviving alternative retains precisely its prior current values.
The residue scan found no proposed arrival-history state, duplicate cause,
cardinality assumption, nested transition authority, semantic boolean, or
positional caller syntax. Cross-check `docs/actor-transition-algebra.md`,
`docs/behavior-layer-laws.md`, `docs/atomic-runtime-settlement.md`, and the
normalized atomic-family laws after the projection is compiled. Disposition
is `pass`: the post-edit implementation files total 6,707 physical lines,
net +88 for this type-level repair. Actor control states, subordinate
alternatives, transition branches, and source modules remain unchanged; the
same residue scan found no new historical state, duplicate cause, false
cardinality, nested transition authority, semantic boolean, or positional
caller syntax. The only public shape change is
`InterpreterRequest::LogicalProtocols`; no new public type or trait name was
added. The normalized actor laws and the transition and layer documents were
cross-checked; the stale claim that every interpreter request is host-free was
corrected in the core and composition documentation. Focused regressions
assert exact logical customer and diagnostic projections, exact-route
exclusion, two wrapper orders, and duplicate occurrence preservation. Real
FIFO, keyed, fixed, dynamic, and stable-proxy behavior compositions compile
with their expected protocol products or positions, including a transitive
worker host. The full workspace gate passed 797/797 tests.

### A01 creation-order policy before documentation repair

Agha et al., [“A Foundation for Actor Computation,” section 3.1](https://doi.org/10.1017/S095679689700261X),
separate fresh address allocation (`newadr`) from behavior initialization
(`initbeh`). They do not prescribe Bombay's host commit, initialization-action
settlement, or activation sequence. The contradictory orders in the two local
documents are the pre-edit failing contract witness.

Bombay's selected policy is: reserve a fresh address without publishing a live
endpoint; run the child's pure definition initialization; install the endpoint
and commit the creator-local nonce binding only after that fold succeeds; then
settle all initialization actions before ordinary ingress. Reservation failure
returns the whole staged request. A pure initialization error returns the
current child and exact error. An installation failure after a successful fold
returns the current child and uninterpreted initialization actions. After
commit, an effect rejection or interpreter fault belongs to the installed
child's drain and cannot retroactively become a creation rejection.
Initialization `Stop` still settles its final actions and never enables
ordinary ingress. For atomic workers, `InitializeWorker` is a later exact-host
request that reports the committed worker's initialization-effect outcome; it
does not redo the pure fold or establish a second birth. Activation follows a
successful ready report. This is a policy and derived composition law, not an
additional actor-model guarantee. The remaining A01 proof obligation is an
interpreter trace for each success, rejection, and stop path.

The interpreter port currently cannot realize this custody law directly:
`RoutedCreation` exposes the child only by shared borrow or by consuming the
complete request. A host that calls `initialize(&mut child)` before commit
would have to dismantle and rebuild the rejected request, obscuring exact
ownership. The pre-production regression in
`behavior-testkit/tests/creation_initialization_order.rs` calls
`initialize(creation.child_mut())` through the staged request and fails with
E0599 because that borrow is absent. The intended syntax leaves the same
`RoutedCreation` owned by the host for `InitializationRejected` or
`HostRejected`, and permits `Established` only after successful installation.
The smallest public repair is one mutable-child borrow on the existing
`RoutedCreation` runtime port. It reuses `initialize`, `CreateChild`,
`ChildCreationOutcome`, and `EstablishChild`; it adds no transition type,
effect lane, interpreter trait, or actor state. The aggregate-drift checkpoint
is unchanged before and after: no aggregate control sum, subordinate
alternative, branch, module, or public type name changes. The new method owns
no arrival history, repeated cause, cardinality assumption, nested authority,
semantic boolean, or positional syntax. The retained disposition is recorded
below.

**A01 retained result:** `RoutedCreation::child_mut` is the single new runtime
borrow port. It lets an `EstablishChild` host call the real pure initialization
fold while retaining the complete request for typed rejection. The six-case
testkit host trace checks reservation, fold, install/commit, effect settlement,
and ingress authorization, including allocation failure, pure fold failure,
host refusal with uninterpreted actions, initialization stop, and a post-commit
effect failure that cannot undo birth. The focused tests and the 804-test
workspace run passed. `docs/actor-transition-algebra.md`,
`docs/atomic-runtime-settlement.md`, and `docs/adapter-contract.md` now state
the same order; the atomic worker `InitializeWorker` request is explicitly
later effect-settlement observation, not a second pure fold. Post-edit drift
disposition is `pass`: no control-state sum, subordinate alternative,
transition branch, source module, or public type name changed; one public
borrow method was added and no historical state, false cardinality, nested
authority, semantic boolean, or positional syntax was introduced. The testkit
host witnesses that the typed port can realize this policy; the downstream
Bombay Engine implementation still needs its own integration verification.

The isolated probes used Rust 1.95.0 and local path dependencies on the audited
crates. No production mutation was made. Their relevant results are:

| Probe | Observation |
|---|---|
| Unrenamed `bombay-behavior` dependency; ordinary `#[behavior::behavior]` counter | `cargo check --offline` fails with E0433: cannot find `bombay_behavior`; the identical source compiles after renaming the dependency to `behavior` |
| Renamed core plus unrenamed `bombay-behavior-actors`; ordinary `#[pool_worker]` assignment completion | Fails with E0433: cannot find `bombay_behavior_actors`; renaming the dependency and corresponding authored path to `actors` compiles |
| `require_projection::<FifoRequests<NoSends, NoSends, NoSends, NoSends, NoSends, NoSends, NoSends, NoSends, NoSends>>()`, with `T: LogicalDeliveryProtocols` | Fails with E0277, establishing the missing product law even when every lane already has a projection |
| `fn duplicate<W>(permit: actors::atomic::ActivationPermit<W>) { let _accepted = permit; }` | Still fails with E0277 after removing the second move; the existing negative example is not specific evidence of affine ownership |
| Mutation report with no baseline, one caught and one unviable mutant | Gate exits 0 and prints `mutation coverage: 1 viable / 2 total` |
| Mutation report with a successful baseline, one caught and one `Failure` mutant | Gate again exits 0 with the same coverage line |
| External caller invokes `delegate_transition` before initialization, then calls `initialize` twice on the same owned counter | Test passes and observes all three state changes; the public composition ports rely on caller discipline |

The mutation probes use two candidates with file `a.rs` and function name `f`,
and baseline `{"floors":{"a.rs::f":1},"known_zero_viable":[]}`. Outcomes use
the gate's existing `summary` and `scenario` schema; the tested command is
`behavior-mutants-gate check OUT OUT/baseline.json`. These are acceptance bugs
in the verdict tool, not results of a real mutation campaign.

### A14 trusted-port evidence

The public `InitializationTurn` constructor remains private. The documentation
now states the narrower truth: `initialize(&mut B)` and
`delegate_transition(&mut B, event)` are trusted wrapper/runtime ports that
can issue turns repeatedly or out of lifecycle order; the consuming
`Activate::initialize` application path owns the once-per-definition rule.
`behavior/tests/trusted_ports.rs` is a legal external caller witness: it
delegates an event before initialization, then initializes the same definition
twice, inspects all three complete empty `Actions`, and observes the three
state changes. The focused test passed. This repair changes no actor effect,
state type, or turn constructor visibility.

### A09 testkit simplification

The testkit now imports the existing `behavior_actors::Activate` trait directly
in its six callers; the forwarding `InitializeTest` trait and its public
spelling are gone. The unused Criterion declaration and its 458 lockfile
lines were removed without updating unrelated dependency versions. Fifteen
tests with no `.await` now use the synchronous harness. Destination-only
fixtures in the catalogue model files implement `Protocol` alone; they no
longer pretend to own an inert `Behavior` lifecycle. The testkit bench check
and all migrated test targets passed. This deletes one duplicate public trait,
one dev dependency graph, and nine generated inert behavior implementations;
it changes no actor transition or runtime effect law. The existing consuming
`Activate` path and its tests are the caller-facing proof, so no replacement
interface or no-op fixture was introduced.

### A06 property-oracle evidence

The sequencer, deduplicator, and order-gate models now compare complete
ordered delivery and reply vectors, including destination addresses, reply
alternatives, rejected payloads, creation emptiness, and next verdict after
each generated step. Their initialization actions are checked too. The
deduplicator uses a non-`Clone` boxed payload and checks its original pointer
through accepted delivery or duplicate rejection, so equal reconstructed
values cannot masquerade as returned ownership. Configuration, readiness,
health, cache, registry, and topic models now inspect successful actions,
reply cardinality and destinations, and initialization rather than discarding
those observations. Readiness generation carries the domain status enum
instead of a semantic boolean; stale errors check the committed version.
The independent map, deque, and list oracles remain separate from the
production transition methods. Focused model and invariant suites passed.
Four isolated temporary counterfactuals each caused the targeted property to
fail: an extra reply, a wrong destination, a stop verdict, and a lost original
boxed payload. The temporary test target and its regression artifact were
removed afterward. No production actor state or effect type changed.

Source evidence at the audited revision:

- A02/A08: [macro resolution](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-macros/src/lib.rs#L20)
  and [consumer harness](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-macros/tests/crate_resolution.rs).
- A03/A11: [atomic product derivation](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/actors/src/atomic/requests.rs)
  and [current projection tests](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-testkit/tests/logical_host_requirements.rs).
- A04: [obsolete adapter fixture](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/actors/src/composition/message_adapter.rs#L29)
  and [activation permit fixture](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/actors/src/atomic/worker/initialization.rs#L50).
- A05/A07: [verdict implementation](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/mutants-gate/src/main.rs)
  and [gate definitions](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/flake.nix).
- A06: [catalogue models](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-testkit/tests/catalogue_models.rs)
  and [catalogue invariants](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-testkit/tests/catalogue_invariants.rs).
- A09/A10: [testkit API and driver](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-testkit/src/lib.rs).
- A18: [benchmark workload](https://github.com/devrandom-labs/bombay-behavior/blob/435560ce7bea8ad3330ee2d42e5034f837a80602/crates/behavior-testkit/benches/protocol_matrix.rs#L66).

- `cargo nextest run --workspace`: **783 passed, 0 skipped**. Three nested
  Cargo fixture tests were reported slow; compilation and execution overlapped
  other audit builds, so these timings are not a performance baseline.
- `mdbook build docs`: passed with this audit included in the book.
- `python3 -m unittest scripts/test_check_published_docs.py`: three passed.
- `python3 scripts/check_published_docs.py target/doc`: passed after building
  the updated guide. All local links in this audit also resolve in the source
  tree.
- `git diff --check`: passed.
- `nix flake check`: **passed** against the clean audited baseline, including
  its optimized nextest run (**783 passed, 0 skipped**), build, Clippy,
  documentation, doctests, formatting, dependency checks, and package-content
  check. The added documentation is checked separately by the book build and
  link checks above. The mutation campaign is a separate Nix package and was
  not run; its focused verdict probes are recorded above.

Change scope: one audit document and links from the engineering index and book
contents. Production `+0 / -0 / net 0`; tests `+0 / -0 / net 0`; public API
`+0 types / -0 types`. Every checklist item remains open; documenting a finding
does not implement its repair. This paragraph records the audited baseline;
the branch repair scope is measured above.

Repair verification on `codex/repository-quality-repairs`:

- `cargo nextest run --workspace`: **790 passed, 0 skipped** before the final
  gate-only test for viability collapse; that additional test passed in both
  debug and release, making 791 current workspace tests.
- `cargo test -p behavior-mutants-gate --bin behavior-mutants-gate` and the
  matching `--release` invocation: **10 passed** each.
- `cargo test -p bombay-behavior-macros --test crate_resolution`: **4 passed**.
- Filtered actor adapter and activation-permit doctests and the core
  heterogeneous-birth doctest passed. An isolated `rustc` counterfactual
  confirmed E0277 for the missing host and successful compilation after adding
  its exact implementation.
- The README example compiled in an offline external path-dependent consumer;
  `python3 -m unittest scripts/test_check_published_docs.py` passed four tests.
- `nix flake check`: **passed all 13 checks** with the new fixture and audit
  document included in the Git source. The mutation campaign itself is a
  separate package and was not run.
- `mdbook build docs`, `python3 scripts/check_published_docs.py target/doc`,
  and `git diff --check HEAD`: passed after the repair record was written.

Final verification of this repair batch before its branch commit:

- `cargo nextest run --workspace --no-fail-fast --test-threads 4`: **810 passed,
  0 skipped**. The three external macro consumers were slow while sharing the
  fixture target; no test failed.
- `cargo test -p bombay-behavior-macros --test crate_resolution --offline`:
  **4 passed** with the nested lockfile aligned to the root vendored set.
- `nix flake check`: **passed all applicable aarch64-darwin checks**, including
  optimized nextest, build, Clippy, docs, doctests, formatting, audit, deny,
  and package-content verification. The first run exposed the nested fixture
  lockfile mismatch; the corrected run passed.
- `bash scripts/check_published_packages.sh`: passed. It assembled the three
  archives and compiled an extracted-package consumer using the README
  declarations and both public macro entry points.
- `cargo bench --profile dev` preflights for `protocol_matrix` and `fifo_pool`
  passed with three short samples each. These are workload checks, not
  performance claims. `mdbook build docs`, README link tests, `cargo fmt
  --all -- --check`, and `git diff --check HEAD` passed.

Thirteen checklist items are complete; seven remain open with their explicit
criteria above. The source changes under `crates/*/src` are `+609/-430` physical
lines (net +179), including the stricter mutation verdict and logical-host
projection. The branch deletes the duplicate `BufferSends` and test-only
`InitializeTest` public names and adds one associated logical-protocol type
and one child-borrow method to existing ports. No actor control-state variant
was added. This scope measurement is diagnostic; it does not certify the seven
open design and integration items.

## References

Repository acceptance criteria are in `AGENTS.md`. Current semantic references
are [actor transition algebra](../actor-transition-algebra.md),
[behavior layer laws](../behavior-layer-laws.md),
[atomic runtime settlement](../atomic-runtime-settlement.md), and the normalized
[proxy](../actor-laws/proxy.md), [fixed supervisor](../actor-laws/fixed-supervisor.md),
[dynamic supervisor](../actor-laws/dynamic-supervisor.md),
[FIFO pool](../actor-laws/fifo-pool.md), and [keyed pool](../actor-laws/keyed-pool.md)
laws. Their ordering conflict
is tracked by A01; this audit does not silently choose a new semantic law.

Rust interface guidance comes from the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
The [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html)
describes compile-fail tests; the
[Cargo check documentation](https://doc.rust-lang.org/cargo/commands/cargo-check.html)
describes target selection. Rust guidance is distinct from Bombay's stricter
local policies on naming, semantic booleans, imports, and static dispatch.
