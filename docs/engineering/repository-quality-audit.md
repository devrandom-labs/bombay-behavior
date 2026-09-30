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

- [x] **A07 — Extend mutation evidence to the actor catalogue.**
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
  **Verified selected-slice gate:** `nix build .#mutants-actors --no-link
  --no-write-lock-file` passed four independently ratcheted actor campaigns.
  Health caught 7/7 viable mutations; WorkQueue caught 2/2 viable among four
  candidates; PubSub membership caught 6/6; publication caught 1/1 viable
  among five candidates. The remaining six whole-function replacements could
  not compile. All four reports had zero missed viable mutations and zero
  timeouts. `mutants/actors/*.json` records separate viability floors by
  function; the Nix command runs actor and testkit suites. Other actor laws
  retain their focused campaign evidence below, and A20 tracks their broader
  law coverage without claiming an actor-wide mutation verdict.

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

- [x] **A11 — Give named effect-product derivation one maintained implementation.**
  **Implemented and verified.** Eleven generic named actor products
  now use `SendProduct` and `#[behavior]` generated products use the same
  procedural generators for send effects, logical-host projection, ordered
  interpretation, settlement classification, and source custody. The two
  settlement representations remain explicit. The old `send_product!` module,
  `BufferSends`, and `atomic::request_product!` were removed. `requirements.rs`
  retains only the exceptional `ReplyDeliveries` and
  `HeterogeneousShutdownSends` projections.
  The unused public `behavior::settle_in_order` tuple helper was removed after
  a source search found no Rust caller in this workspace or adjacent
  Bombay/Address repositories. Named products generate their ordered traversal
  directly; the normative settlement document states that law.
  **Complete when:** a law table compares complete equations before selecting
  shared machinery. A retained derivation must preserve semantic field names,
  declared order, corruption suffixes, source admission, and logical-host
  projection, and delete repeated implementations. Prove two unrelated real
  products and both wrapper orders before catalogue migration. Do not merge
  products with different ownership or retirement laws, or introduce a public
  product framework merely to save typing.
  **Evidence:** the [equation inventory](#a11-named-send-equation-inventory),
  two unrelated actor products, source-custody tests in both `SendLayer`
  orders, and workspace `--all-targets` tests support the retained design.
  A clean Nix gate on `a8f15b7` passed all ten active aarch64-darwin checks,
  including 843/843 optimized Nextest tests.

- [ ] **A12 — Reassess aggregate decomposition using retained current values.**
  **Design candidate.** FIFO's root has 4,097 lines, stable proxy's root 3,506,
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
  **Reviewed distinction:** StableProxy's data-free `Dormant` and
  `EmptyInitial` states are not duplicate history labels. `Dormant` admits its
  one initial worker submission; `EmptyInitial` follows a rejected initial
  creation and rejects another submission with the observable `Overlap` and
  `ProxyPhase::EmptyInitial`. `proxy_command_recovery` and `proxy` exercise
  this path. Merging the states would admit a second initial worker or change
  the documented outcome. The other subordinate alternatives and family
  measurements still require the full A12 checkpoint.
  Dynamic supervision's `WorkerChangeDisposition::{Committed,Cancelled}`
  retains the answer to a future repeated cancellation. In
  `DynamicEntryPhase::Available`, the disposition selects the same committed
  or cancelled receipt again. `dynamic::ready_service_accepts_one_replacement_on_its_current_proxy`
  and `dynamic::accepted_start_cancellation_retires_before_fresh_key_reuse`
  exercise these answers. Deleting the disposition would change that later
  reply even though service availability is the same. This read-only
  distinction does not close the wider family audit.
  The stopped and forced-retirement alternatives in StableProxy, fixed
  supervision, FIFO, and keyed pooling still own workers, submissions,
  shutdown results, or rejection causes for terminal custody. Their fields
  have no local read path in some cases because the whole stopped behavior is
  retained for the interpreter. The downstream terminal-return witness in
  A17 and the interpreter ownership PRD must prove each transfer before any
  such alternative can be removed. This is a custody question left open by
  the source-only review.

  **Current decomposition baseline (read-only):** the figures below count
  physical lines in each family source tree, including comments and embedded
  tests. Production lines exclude whole spans under `#[cfg(test)]` but retain
  comments and blank lines. Match arrows are a search diagnostic, not semantic
  transition branch counts.

  | Family | Root control representation | Source modules | Physical lines | Production lines | Match arrows | Still required |
  |---|---|---:|---:|---:|---:|---|
  | Stable proxy | `ProxyState`: 8 alternatives | 6 | 5,946 | 5,076 | 398 | Enumerate each nested shutdown and retirement value against exact terminal custody. |
  | Fixed supervisor | `FixedRoster`: 5 alternatives; recovery owns a separate 2-way policy state | 19 | 10,104 | 9,512 | 718 | Check roster/recovery joins and every retained prepared worker. |
  | Dynamic supervisor | `SupervisorAvailability`: 2 alternatives; each keyed `DynamicEntryPhase` has 14 alternatives | 8 | 4,120 | 4,020 | 246 | Check whether per-key transitions remain an entity invariant rather than a second aggregate authority. |
  | FIFO pool | `PoolState`: 5 alternatives | 5 | 4,783 | 4,783 | 311 | Check backlog, cursor, per-member custody and forced retirement separately. |
  | Keyed pool | `KeyedPoolState`: 5 alternatives | 10 | 5,755 | 5,536 | 268 | Check binding generations and per-role order without importing FIFO policy. |

  These sums have no proposed deletion yet. The source scan found no semantic
  `bool` field in these families; the observed boolean signatures are
  membership or equality predicates. The `FifoDispatch` alternatives that
  return the same queued job still select different lawful actions: unavailable
  worker may enqueue it, while exhausted assignment correlation returns an
  explicit rejection. Fixed recovery's `LeaveEmpty` and `SourceUnavailable`
  both retain recovery state but select a lifecycle fact versus exact input
  rejection. Those alternatives cannot be merged by payload shape alone.
  This baseline does not replace the future-needed-value and production
  measurements for every subordinate alternative; A12 remains open.

  **A12 single-worker creation batch, pre-edit law:** StableProxy stages one
  worker creation per start. Its creation settlement therefore consumes exactly
  one matching result; a zero- or multi-item settlement is an unexpected input
  that must return the complete original ordered batch to the owner while the
  proxy remains in its creating phase. This is Bombay's derived staged-creation
  correlation policy, not an actor-model allocation law. The public caller
  syntax and observable transition stay unchanged. The focused
  `proxy_command_recovery` malformed-batch regression covers the complete
  returned batch. Existing lower-order products are `CreationSettlement`,
  `CreationsSettled`, `WorkerCreation::Unexpected`, and the owner diagnostic;
  no new type, bound, port, or interpreter operation is needed. The candidate
  implementation uses the owned vector's exact-one conversion so a failed
  cardinality check returns every item without cloning or reconstructing a
  different batch. This local simplification does not close the family-wide
  A12 inventory.

  **A12 single-worker creation batch, retained checkpoint:** Root
  `ProxyState` remains 8 alternatives before/after; the touched
  `WorkerCreation` sum remains 3 and `CreationSettlement` remains 3. Its three
  top-level settlement branches and five reachable cardinality paths are
  unchanged; two impossible `pop() == None` branches are gone. The touched
  production method is 83 → 70 lines; the six-module family is 5,959 → 5,946
  physical source lines, including unchanged embedded tests, and 5,089 →
  5,076 production lines after excluding the 870 lines under `#[cfg(test)]`.
  Public spellings and module count are unchanged.
  `WorkerCreation::Initializing` still owns the
  committed worker, activation, and possible prior stop;
  `WorkerCreation::Rejected` owns the rejected worker, activation, and possible
  prior stop; `WorkerCreation::Unexpected` owns the pending worker, possible
  prior stop, and the entire anomalous batch for the owner diagnostic. No
  arrival-history label, repeated cause, false cardinality, nested transition
  authority, semantic boolean, or structural caller syntax was added.
  Cross-checked `docs/actor-laws/proxy.md`,
  `docs/engineering/atomic-actor-essence.md`, and
  `docs/engineering/atomic-actor-retained-core.md`; the focused 53-test proxy
  recovery suite passes. Disposition: `pass` for this local simplification;
  A12 remains open for the complete family inventory and terminal custody.

  **A12 exact-one creation batch, pre-edit law:** Actor research requires fresh
  creation but does not prescribe a Rust batch API. Bombay's ordered
  `Creations<Item>` is a derived effect product. A single-child aggregate may
  consume exactly one result; if the batch has zero or multiple items, the
  conversion must return the complete original batch in order. The intended
  caller syntax is `creations.into_one()`, returning `Result<Item,
  Creations<Item>>` without `Clone`. A focused external caller test will require
  one move-only item to succeed and empty/two-item batches to return intact;
  the prior API fails that test because it has no such operation. The existing
  lower-order value is the privately owned `Vec<Item>` in `Creations`; its
  exact-one array conversion already appears twice in StableProxy, while
  DynamicSupervisor manually checks length then has an unreachable empty
  branch. The design stage adds only the batch operation and its focused test.
  Separate migration stages will apply that proven operation to those two
  aggregates, preserving their current `CreationSettlement` outcomes and
  interpreter requirements. Before editing, the core batch has no state sum;
  StableProxy has eight root states and six modules, DynamicSupervisor has two
  root availability states, fourteen entry phases, and eight modules. No new
  actor state, effect lane, trait bound, interpreter capability, or type is
  proposed. The public batch method count grows by one in the design stage;
  branch and line measurements follow each retained stage.
  The external caller initially failed with three `E0599` diagnostics at the
  intended `into_one` calls. After the core method was added, the focused
  Nix-pinned test passed for one, zero, and two move-only workers. The design
  stage adds 13 production source lines and 22 net test lines; it changes no
  aggregate state, subordinate alternative, transition branch, module, or
  interpreter path. On failure, the original `Vec<Item>` is returned inside
  the same ordered `Creations` value; there is no clone, dropped item, arrival
  history, repeated cause, false cardinality, nested authority, semantic
  boolean, or positional caller syntax. The actor transition and retained-core
  creation laws were cross-checked. Disposition: `pass` for the batch API;
  aggregate migrations and their separate checkpoints remain open.
  **A12 exact-one aggregate migration, retained checkpoint:** StableProxy now
  consumes rejected and settled worker batches through that operation. Its
  eight root states, three `WorkerCreation` alternatives, six modules, and
  public spellings are unchanged; the two cardinality success/failure choices
  are unchanged. Its production source falls by two lines, from 5,076 to
  5,074. DynamicSupervisor now consumes its proxy-settlement batch through
  the same operation. Its two availability states, fourteen entry phases,
  eight modules, and public spellings are unchanged. The same reachable
  success/failure choice remains, while the old impossible `next() == None`
  branch after `len() == 1` is gone. Its production source falls by five lines,
  from 4,020 to 4,015. The dynamic regression now checks empty and two-item
  rejection as the exact `ProxyCreationsSettled` event, including both untouched
  routes, creation IDs, kinds, item order, and the retained `CreatingProxy`
  phase; that test changes by `+47/-3` lines. All 29 dynamic and 53 proxy
  recovery tests pass with the Nix toolchain. The full malformed batch remains
  owned by the rejected event, with no arrival history, repeated cause, false
  cardinality, nested authority, semantic boolean, or structural caller path.
  `docs/actor-laws/proxy.md`, `docs/actor-laws/dynamic-supervisor.md`, and
  the normalized atomic-actor documents were cross-checked. Disposition:
  `pass` for the mechanical migrations; the remaining family inventories and
  interpreter terminal custody still keep A12 open. A clean detached-worktree
  `nix flake check -L --max-jobs 2` at `44922e2` passed all eight active
  aarch64-darwin checks, including 845/845 optimized Nextest tests, Rustdoc,
  Clippy, package, and formatting. The first run caught three test-only
  consuming calls inside assertions; those calls now execute before the
  assertions and the full rerun passes.

  **A12 StableProxy root values, read-only review:** the one control-state sum
  remains eight alternatives in six modules and 5,076 production lines. The
  retained values below are used by a later decision or returned as terminal
  custody; these rows do not claim that every nested sum has been reviewed.

  | `ProxyState` alternative | Exact current value required later |
  |---|---|
  | `Dormant` | No worker; the one initial submission is still admissible. |
  | `Starting` | Start kind and current creation, initialization, activation, or return value select correlation, rejection return, and the next effect. |
  | `Ready` | The exact current worker capability and attempts select service delivery, stop, and replacement. |
  | `EmptyInitial` | No worker; initial-start authority is spent, so another initial submission must return `Overlap`. |
  | `EmptyAfter` | The previous worker attempt is needed as replacement provenance. |
  | `Replacing` | The predecessor departure or successor result plus outstanding shutdown correlation must be joined before publication. |
  | `ShuttingDown` | Unresolved worker, creation, initialization, activation, and departure values must settle or transfer before retirement. |
  | `Stopped` | `ProxyRetirement` owns the exact residual values for the runtime custodian; A17/T16 must prove that transfer. |

  `WorkerStopping` is a direct independent join: the shutdown request is
  either awaiting its exact ID or has its resolution, while the exact worker
  stop is absent or present. The resolved-plus-stopped combination immediately
  becomes `StoppedWorker`, so no extra arrival-order state is stored.
  `proxy_command_recovery` exercises stop-first and resolution-first joins.
  The root table and this join match `docs/stable-proxy.md` and
  `docs/actor-laws/proxy.md`; they justify retention, not a new deletion.
  Nested result and retirement sums, other family inventories, and the
  interpreter's terminal transfer still keep A12 open.

  **A12 shared pool assignment join, read-only review:** FIFO and keyed pools
  both use the four-alternative private `AssignmentDelivery` sum. This is one
  current job obligation with an exact assignment correlation; it is not a
  second actor transition authority. The owning aggregate still selects the
  complete `Actions`. The shared assignment module has 811 production lines
  and 311 embedded-test lines; this review changes zero states, alternatives,
  transition branches, modules, production lines, or public spellings.

  | `AssignmentDelivery` alternative | Exact value needed by a future decision |
  |---|---|
  | `AwaitingReceipt` | No delivery receipt exists; completion or stop must be retained until acceptance or rejection settles the moved assignment. |
  | `Accepted` | The exact receipt has committed delivery; the next matching completion or stop can select the one customer disposition. |
  | `CompletionHasPriority` | The completion is authoritative if delivery is accepted; an optional later exact stop must still drive worker recovery once. |
  | `WorkerExitHasPriority` | The exact stop is authoritative if delivery is accepted; an optional later completion must remain available for stale or contradictory settlement. |

  `CompletionHasPriority` and `WorkerExitHasPriority` are observable order
  decisions, not duplicate arrival labels. Collapsing them into a product of
  two optional facts would lose which terminal event won. The two focused
  `atomic::pool::assignment` order tests and the FIFO/keyed pool law documents
  cross-check the retained values. `AssignmentReceiptOutcome`,
  `WorkerCompletionOutcome`, `AssignmentRejectionOutcome`, and
  `WorkerExitOutcome` return different complete values to their respective
  callers; they have not been merged by their common job fields. This review
  finds no semantic boolean, repeated cause, false cardinality, structural
  caller path, or redundant nested actor. Disposition: `pass` for this shared
  join only; both pool families' member, recovery, and retirement sums remain
  under A12 review, and the move-only FIFO law remains unresolved in the PRD.

  **A12 FIFO/keyed root values, read-only review:** each pool has one
  five-alternative root sum. This review changes zero states, subordinate
  alternatives, transition branches, production lines, modules, or public
  spellings. FIFO remains five modules and 4,783 production lines; keyed
  remains ten modules and 5,536 production lines. The exact future-needed
  values differ inside their operating products:

  | Root alternative | FIFO current value | Keyed current value |
  |---|---|---|
  | `Constructed` | The ordered prepared worker roster must be returned intact if initialization ID reservation fails. | The same prepared roster must survive failure before the per-role queues and binding table exist. |
  | `Operating` | `FifoOperating` owns the ordered members, admission-ordinal backlog, and next role cursor. These select a global FIFO dispatch. | `KeyedOperating` owns one queue per role, current worker cells, and the bounded key binding table with generations. These select exact key affinity. |
  | `Draining` / `Retiring` | The exact unresolved `RetiringWorker` vector and `ShutdownDeadline` settle worker results and a bounded drain. | The same shared worker-retirement values and deadline settle keyed drain; key/queue outcomes are emitted when retirement begins. |
  | `Stopped` | No worker or job remains in the actor root after ordinary retirement; later input is rejected. | No worker or binding remains in the actor root after ordinary retirement; later input is rejected. |
  | `ForcedRetirement` | The unresolved workers and exact cause (`WorkerShutdownIdsExhausted`, `DeadlineNotScheduled`, or `DeadlineElapsed`) must transfer to the runtime custodian. | The same kind of unresolved worker vector and exact cause must transfer; the per-role binding policy does not turn that into a FIFO backlog. |

  `Constructed` is a pre-initialization ownership phase and
  `ForcedRetirement` is terminal residual custody, not duplicates of the
  operational `Stopped` state. Both are source-level extensions to the
  three-state operational sketches in the normalized FIFO/keyed law documents;
  those documents now name the distinction. Existing FIFO forced-retirement
  tests and keyed lifecycle tests check retained workers locally. They do not
  prove the parent-to-root transfer, which remains A17/T16 work. The source
  scan found no arrival-history root alternative, repeated cause, false role
  cardinality, nested actor authority, semantic boolean, or positional caller
  syntax. Disposition: `pass` for these root sums only; member, recovery,
  deadline, and terminal-custody alternatives remain under A12 review.

  FIFO and keyed pools each move their root state out with `mem::replace(...,
  Stopped)` during initialization and transition; fixed supervision similarly
  substitutes `Stopped` or a temporary recovery value, and StableProxy
  substitutes `Dormant` during one consuming transition. Normal return commits
  the result once, but a caught pure-fold panic before that commit may leave
  only the placeholder in the actor while owned state unwinds. This is an
  inference from the source, not a proven runtime outcome. The PRD's T16 panic
  and root-custody witness must test it before these sites can be called safe
  or refactored. A passing ordinary transition suite cannot decide that law.

- [ ] **A13 — Audit public bounds, hidden exports, and extension ownership.**
  **Confirmed surface requiring review.** The current source has 10
  `#[doc(hidden)]` annotation sites in core and 56 in actors, including
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
  now accounts for all 78 top-level public traits by lawful implementor role,
  including the later `ProxyControlAdmission` interpreter port. Its actor-suite
  implementors and the isolated Bombay source identify its owner; the latter
  is not yet a successful A17 runtime witness.
  The inventory confirms that `StashStatus` has multiple real wrapper
  implementations; its name alone is not grounds for deletion. Externally
  authored child roles and generic logical-host owners have compile witnesses
  for the newly visible types.
  The inventory now classifies the hidden `ChildProduct::stage` method as a
  sealed structural conversion: only `NoChildren` and `ChildCons` implement
  the trait, `Children::into_creates` is its sole production caller, and the
  interpreter consumes the resulting `Creations` effect. Calling `stage` a
  runtime port had overstated its public contract. This correction changes
  zero Rust items, bounds, states, branches, modules, or public spellings.
  The cache/resolver protocol-bound comparison found no material compile-time
  difference in its measured pair; the caller diagnostics improved. The
  protocol-only caller now covers 20 catalogue actors, keeping construction
  and transition bounds at the
  operations that need them. A separate caller now proves that 14 unwrapped
  catalogue actors expose their read-only `BehaviorBase` projection without
  requiring the cloning, comparison, or ordering used only by construction or
  transition. This also includes keyed `Deduplicator` and `OrderGate` and a
  priority queue with opaque priority data. The proxy operation ID is now
  crate-private; focused external fixtures distinguish forbidden ID naming,
  receipt construction, and double settlement. Review of remaining hidden runtime
  ports and the rest of the public surface is still required.
  A post-repair source scan of catalogue `Protocol` and `BehaviorBase` impl
  headers found no remaining copying/comparison/ordering bounds except
  `Router`'s `Route: Clone + PartialEq`, which is also required by its current
  `RoutingStrategy<Route>` contract. Changing that contract would require a
  distinct membership and policy law, not a mechanical bound deletion.
  The read-only Router review found no legitimate transition for a route
  without identity comparison: `new` removes duplicates, `Add`/`Remove`
  select exact members, and selection clones the policy candidate so a
  failed route leaves it unchanged. A logical or established route whose
  endpoint lacks equality cannot currently satisfy that membership law.
  Narrowing only the protocol/base headers would create a nameable router
  that cannot be constructed or run; no bound edit was retained. Round-robin's
  cursor repair and the least-loaded/Rendezvous traces exercise the current
  comparable-route contract. This resolves the isolated header question,
  while A13's broader hidden-port review remains open.

  **Pre-edit A13 customer-delivery documentation law:** A keyed-pool
  interpreter must name the concrete `CustomerDelivery<P>` action to return
  both the attempted delivery and original customer route on rejection. An
  assignment interpreter must call `AssignWorker::target()` to obtain a clone of the
  exact worker recipient before consuming `settle`. These are existing
  derived ownership ports, not new actor powers. The public caller syntax is
  already exercised by the keyed-pool external compile suite and the
  interpreter-contract assignment fixture; the four customer alternatives
  retain their existing complete settlement equation. Before editing,
  Nix-pinned Rustdoc omits `CustomerDelivery` from the `atomic` index and has
  no item page, and omits `method.target` from the visible `AssignWorker`
  page. Removing their two declaration markers and separating the existing
  grouped re-export exposes only those already-public names. The focused
  regression is their Rustdoc visibility with the existing external callers
  still passing. No constructor, method, bound, trait, effect lane, control
  state, transition branch, or interpreter operation changes.
  Post-edit, Nix-pinned Rustdoc lists `CustomerDelivery` in the `atomic`
  index, generates its item page, and lists `method.target` on `AssignWorker`.
  The 23 keyed-pool tests and three external assignment-delivery tests pass.
  Actor hidden annotations fall from 58 to 56; the two declaration markers
  are gone while the sealed `CompletesAssignments` re-export remains hidden.
  The root aggregate states, subordinate alternatives, transition branches,
  source modules, and Rust public spellings are unchanged. The exact worker
  recipient capability remains inside `AssignWorker`, and the original
  customer route remains inside a rejected `CustomerDelivery`. No arrival
  history, repeated cause, false cardinality, nested authority, semantic
  boolean, or positional caller syntax was introduced. The keyed-customer
  rejection law in `docs/atomic-runtime-settlement.md` and the normalized
  pool law were cross-checked. Disposition: `pass` for these two visible ports;
  the wider A13 surface and compile-cost review remain open. A clean
  detached-worktree `nix flake check -L --max-jobs 2` at `19a38c2` passed all
  eight active aarch64-darwin checks, including Rustdoc and optimized Nextest.

  **Pre-edit A13 diagnostic-port visibility law:** An external interpreter
  must name the concrete `DiagnosticAction`, `DiagnosticAccepted`, and sealed
  `DiagnosticRoute` contract to settle a routed or terminal diagnostic. The
  accepted value either records delivery or transfers the exact diagnostic
  into terminal custody; it cannot silently discard the latter. This is a
  derived interpreter ownership port, not a new actor transition. The
  external `diagnostic_action` and interpreter-contract
  `source_free_custody` suites already use this syntax, including the
  constructor methods. Before editing, Nix-pinned Rustdoc builds but omits
  all three names from the `atomic` index and has no item pages for them.
  The focused regression is visible Rustdoc for these already-public names
  and methods while the existing caller suites continue to pass. The only
  proposed change removes documentation hiding annotations on the three
  declarations, their four methods, and grouped re-export. It adds no type,
  bound, capability, effect lane, or transition branch.
  Post-edit, Nix-pinned Rustdoc lists all three names in the `atomic` index,
  generates their item pages, and lists all four methods. The three
  `diagnostic_action` tests and four external source-free custody tests pass.
  Actor hidden annotations fall from 66 to 58. Aggregate control states,
  subordinate alternatives, transition branches, modules, and Rust public
  spellings are unchanged; the documentation-only edit removes eight source
  lines. The complete diagnostic remains owned by the terminal accepted
  variant until the interpreter transfers it. The residue scan finds no
  arrival history, duplicate cause, false cardinality, nested transition
  authority, semantic boolean, or structural caller syntax. The actor
  transition and retained-diagnostic laws were cross-checked against
  `docs/actor-transition-algebra.md` and
  `docs/engineering/atomic-actor-retained-core.md`. Disposition: `pass` for
  these diagnostic item pages; A13 remains open for other ports.

  The creation settlement review found a narrower documentation defect:
  external caller suites name `CreationSettlement`, `CreationSettlements`, and
  `CreationsSettled` to retain or return exact child-creation custody, but none
  appeared as an item in the generated crate-root Rustdoc index. Their public
  visibility and settlement equations already existed; showing them changes
  no actor transition. A later read-only downstream inspection found that
  Bombay's `ActionInterpreter` explicitly requires `InterpretCreations` in
  its `CommitActions` implementation. That is a real interpreter port and
  supplies the missing caller evidence to show the trait in Rustdoc as well.
  The three externally named settlement ports are now listed by Rustdoc:
  `cargo doc -p bombay-behavior --no-deps --locked` succeeded with the
  Nix-provided Rust 1.95 toolchain, and all three crate-root index links and
  item pages exist. The external `generated_creation_custody`, `custody`, and
  actor caller suites already name these products. Downstream compilation and
  runtime witnesses remain part of A17; this source inspection does not claim
  they pass. A second Nix-provided Rustdoc build after exposing
  `InterpretCreations` confirmed that all four names have crate-root index
  links and item pages. The `bombay-behavior-doc` Nix check passed on signed
  commit `4cd3b7f`, including Rustdoc, the book, and published-document checks.

  The next required runtime method is `CreateChild::into_parts`. The external
  `creation_initialization_order`, `creation_settlement_custody`, and
  `behavior_generation` caller suites consume it to retain the exact child,
  creator-local ID, and provenance. The real Bombay child host also names it
  after consuming a `RoutedCreation`. Its Rust visibility is already public,
  but `#[doc(hidden)]` conceals it from an interpreter author reading the API.
  This is a derived ownership port, not a new actor transition. The focused
  documentation repair removes that one marker and gives the method an
  explicit custody description; it adds no type, bound, effect lane, control
  state, or interpreter capability. The existing external callers are its
  compile witnesses. The post-edit check is that Rustdoc lists the method on
  `CreateChild` and those callers still compile.
  Post-edit, `cargo doc -p bombay-behavior --no-deps --locked` generated
  `struct.CreateChild.html` with `method.into_parts` and its custody text.
  `cargo check --locked` passed for the external
  `behavior-testkit/tests/creation_initialization_order` and
  `behavior/tests/creation_settlement_custody` callers; `mdbook build docs`,
  formatter, and diff checks passed. The source change is `+6/-1` physical
  lines of documentation only; tests and public type counts are unchanged.
  Aggregate control states, subordinate alternatives, transition branches,
  modules, and public spellings are unchanged. The owned child, ID, and kind
  remain the same complete product; no arrival history, duplicated cause,
  false cardinality, nested authority, semantic boolean, or positional caller
  syntax was introduced. The actor transition and creation-custody laws were
  cross-checked. Disposition: `pass` for this one documentation port. A13's
  wider surface review remains open.

  `CreationCorrelation<P, Occurrence>` is another required, already-public
  effect prerequisite. The external `behavior/tests/action_interpretation`
  and actor `interpreter_request_settlement` callers name it in `ActionItem`
  implementations; its compile-fail example rejects exchange of equal IDs
  at different occurrences. The derived law is typed correlation to exactly
  one creation settlement, without granting child-hosting authority. The
  focused visibility repair removes its Rustdoc hiding marker, retains its
  private representation and public constructor unchanged, and adds no transition
  or type. The pre-edit regression is the absence of its item in the generated
  crate-root Rustdoc index despite those external compile witnesses. The
  post-edit checks are the Rustdoc index and the occurrence-mismatch doctest.
  Post-edit Rustdoc has a crate-root link and item page, the focused E0308
  compile-fail doctest passed, and `cargo check --locked` passed for both
  `action_interpretation` and `interpreter_request_settlement` external tests.
  The source change deletes one hidden annotation; tests, public types,
  control states, subordinate alternatives, transition branches, modules,
  and public spellings are unchanged. No history, repeated cause, false
  cardinality, nested authority, semantic boolean, or positional syntax was
  introduced. The actor transition and occurrence laws were cross-checked.
  Disposition: `pass` for this visible prerequisite, with A13 still open for
  other items.

  `CreationId::get` is likewise an existing public runtime port concealed in
  Rustdoc. The actor catalogue derives shutdown and operation correlations
  from its occurrence-local number; external actor tests also inspect it.
  Exposing the existing method does not make the number an actor identity or
  a freshness proof, so its documentation now says that explicitly. The
  pre-edit symptom was its absence from generated Rustdoc despite external
  callers. The source edit removes one hidden marker and adds only custody
  text: zero types, bounds, lanes, states, branches, modules, or public
  spellings change. The current owned ID remains the sole value. The residue
  scan is clear for history, repeated cause, false cardinality, nested
  authority, semantic boolean, and structural syntax. Cross-checks: actor
  creation law and the exact-correlation consumers. Nix-pinned Rustdoc built
  `CreationId` with a visible `method.get` item, and `cargo check -p
  bombay-behavior-actors --tests --locked` passed. Disposition: `pass` for this
  documentation-only port; A13 remains open for the wider surface.

  The assignment custody repair narrowed `AssignWorker::receipt` and
  `AssignWorker::into_parts` from public to atomic-module-only after the
  external consuming settlement passed. Compile-fail fixtures now reject
  those two old assembly paths with `E0624` and reject double settlement with
  `E0382`; FIFO/keyed callers, benchmark, and fuzz campaigns use the actual
  exact-delivery result. The later proxy settlement stage also narrowed its
  operation ID, request decomposition, and receipt construction after external
  privacy and closed-control witnesses. The public-surface inventory reflects
  81 actor `#[doc(hidden)]` sites at that checkpoint. The wider port and bound review
  remains open.

  **Pre-edit A13 interpreter-name law:** an external interpreter implementing
  `InterpretItem<AssignWorker<Worker, Job>, ...>` or
  `InterpretItem<ProxyOperation<Source, Worker, Plan>, ...>` must name the
  corresponding opaque accepted receipt in its return type. The interpreter
  also names `ProxyControl` and the immediate proxy result when implementing
  the concrete `ProxyControlAdmission` port. These are derived ownership
  products, not additional actor powers; their fields and receipt constructors
  stay private. Existing external assignment and proxy caller suites, plus
  the inspected Bombay interpreter source, prove the naming syntax. Before
  editing, Nix-pinned `cargo doc -p bombay-behavior-actors --no-deps --locked`
  built successfully, but the generated `atomic` index and item files omitted
  `AssignWorker`, `AssignmentReceipt`, `ProxyControl`, `ProxyOperation`,
  `ProxyInputReceipt`, and `ProxyInputResult` because their declarations or
  grouped re-exports carried `#[doc(hidden)]`. The focused regression is that
  these existing names and their custody descriptions appear in Rustdoc while
  the external caller and compile-fail fixtures keep compiling unchanged.
  The edit is limited to documentation visibility and regrouping the already
  public re-exports; it introduces no trait, type, constructor, effect lane,
  bound, or actor transition. The ownership proof remains the consuming
  `settle` methods and the existing static interpreter port. Aggregate states,
  subordinate alternatives, branches, modules, production public spellings,
  and current values are unchanged by this visibility repair; there is no
  arrival history, repeated cause, false cardinality, nested authority,
  semantic boolean, or structural caller path to retain. Disposition will be
  `pass` only after the item pages, external fixtures, and Nix documentation
  gate are checked.
  The focused Rustdoc build now lists all six types in the `atomic` index and
  generates their item pages. The actor source has 75 hidden annotation sites,
  six fewer than the preceding checkpoint. All 13 external fixture test
  functions passed in debug and optimized profiles. Four compile-fail snapshots
  changed only the diagnostic's displayed type path; their E0382 and E0599
  failures remain the intended ownership and privacy errors. A clean Nix flake
  check at signed commit `b8b9842` passed all eight available macOS checks,
  including Rustdoc, published-document checks, and 843 optimized Nextest
  tests. Disposition: `pass` for these six runtime item pages; A13 remains open
  for the rest of the surface.

  **Pre-edit A13 worker-preparation port law:** a trusted worker-source
  interpreter consumes the owner-emitted `PrepareWorkers` request, borrows its
  current source and role, then consumes either an accepted submission or its
  exact rejection. A multi-role request continues through the owned
  `PendingWorkerPreparation` until the complete `WorkerPreparation` returns.
  The public methods already express that affine progression; the interpreter
  must be able to discover those types and methods without guessing hidden
  Rustdoc paths. This is a derived Bombay ownership port, not a new actor-model
  operation. External fixed/FIFO actor tests and the inspected Bombay
  `worker_preparation.rs` use this exact syntax; no replacement trait or
  constructor is proposed. Before editing, Nix-pinned Rustdoc omitted the
  three types from the `atomic` index. The focused regression is that the
  existing three types and six progression methods become visible while their
  constructors, tickets, fields, and owner settlement remain private. This
  visibility-only batch changes zero control states, subordinate alternatives,
  transition branches, modules, or Rust public spellings; the exact current
  source, role, prepared prefix, ticket, and remaining roles retain their one
  owners. The residue scan is clear for arrival history, duplicated cause,
  false cardinality, nested transition authority, semantic booleans, and
  structural caller syntax. Cross-checks are the worker-preparation custody
  law and normalized atomic pool/supervisor documents. Disposition requires
  Rustdoc visibility, the external actor suites, and the clean gate.
  Nix-pinned Rustdoc now lists the three structs in `atomic` and all six
  progression methods on their item pages. Actor hidden annotation sites fell
  from 75 to 66. The external `fifo_pool` and
  `fixed_supervisor_initialization` actor suites passed 91 and 98 tests,
  respectively. The first clean Nix attempt at signed commit `a74b287`
  stopped during optimized compilation when the machine ran out of disk space;
  it reported no Rust law failure. After cleaning this repository's generated
  Cargo target directory, the retry with two concurrent Nix builds passed all
  ten available macOS checks, including Rustdoc and 843 optimized Nextest
  tests. Disposition: `pass` for this documentation port; A13 remains open for
  the rest of the public surface.

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
  **Isolated downstream probe:** an unmodified snapshot of the sibling
  worktree compiled `bombay-rs --lib` under its Nix-pinned Rust 1.96 toolchain
  against published behavior crates. With Cargo patches pointing to this
  branch, compilation stopped at `E0046`: `EntityAdmission<D>` has not declared
  the newer `InterpreterRequest::LogicalProtocols` projection. Source review
  found another test-local `InitializationRequest` implementation with the
  same omission. The candidate law for a temporary compatibility probe is
  that both requests use already-owned capabilities and emit no logical actor
  `Delivery`, so their emitting actor's logical-host product is empty. This is
  an inference from the downstream source, not an actor-model guarantee or a
  successful A17 witness. No change was made to the sibling checkout.
  In a second isolated copy, those two requests declared the empty logical
  protocol product and Cargo patched both behavior crates to signed commit
  `4cd3b7f`. `bombay-rs --lib` then compiled, and five selected downstream
  targets passed 27 tests: `application_children` (1),
  `application_terminal_custody` (2), `entity_application` (1),
  `entity_runtime` (11), and `run_with` (12). The selected tests include real
  application admission, activation failure, passivation, terminal custody,
  and caller compile fixtures. The snapshot came from the dirty downstream
  worktree at `a7a66e391273`; it is not a recorded clean revision, and the
  temporary projections are not present in that repository. These tests do
  not prove replacement-establishment failure, every creation collision and
  exhaustion path, independent sends after rejection, or parent-to-root
  residual transfer. A17 stays open.

  Address `0.3.0` is now published. In the isolated Bombay worktree
  `codex/interpreter-ownership`, Cargo resolves that release while Behavior
  and Communication remain path-patched research inputs. Bombay's all-target
  compile passes against this mixed graph. Its current library run passes
  174 of 179 tests; five older startup assertions still expect publication
  after initialization stop or task unwind after a caught pure fold panic.
  This is neither an immutable integration revision nor an A17 verdict. The
  original Bombay checkout remains untouched.

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
  **Assignment custody evidence correction:** the external delivery fixture
  previously compared the original `Box<str>` payload allocation with the
  address of the delivered `Box` handle. A temporary equality assertion made
  both affected tests fail, proving that the old pointer comparison did not
  identify the payload. The retained fixture now records the delivered `str`
  allocation and contents. It verifies that FIFO's current `Job: Clone` policy
  sends a distinct copied allocation, while rejection returns the original
  allocation. Two same-typed requests settle in reverse order and return their
  own receipts. Debug and optimized external fixture runs pass. This evidence
  does not satisfy the PRD's non-`Clone` T02 requirement; the precise source
  conflict and current test scope are in the
  [implementation ledger](interpreter-contract-implementation-ledger.md).
  **Pre-commit host-refusal custody:** the external
  `tests/interpreter-contract/tests/startup_host_rejection.rs` caller runs a
  pure initialization with two ordered values whose types do not implement
  `Clone`. It then requires `HostRejected` to return the mutated current child,
  both original send allocations in order, the untouched staged nested child,
  the route, creation IDs, kinds, and continuation decision. Debug and
  optimized fixture tests pass. This proves
  the Behavior-side return shape; no production host or Address reservation
  participates, so it does not establish T11 or close A17/A20.

## Follow-on after the audit checklist

- [ ] **Complete the [Interpreter ownership and startup PRD](creation-and-delivery-custody-prd.md) after the Behavior-side audit repairs.**
  Follow its ordered work packages P0–P5 and prove every acceptance trace
  T01–T24 against the real Address and Bombay interpreters. Include complete
  rejection and terminal custody, external consumer compilation, release and
  downstream lock verification, and every required repository gate. Apply its
  architecture checkpoints and definition of done; the audit checklist does
  not narrow the PRD's scope. Its optional P6 consolidation follows the
  blocking contracts only where an independent law proves the deletion. P5's
  integrated witnesses are required to close A17 and the corresponding A20
  runtime-evidence rows; those items remain open until that proof exists.

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
| Discovery | registry/topic/pub-sub models, presence fuzz, resolver unit tests | Assert snapshot order, stale versions, recipient identity, and complete rejected commands; retain read-only resolver authority |
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

### A08 Nix test-runner ownership, before edit

The flake's `bombay-behavior-nextest` check already runs every workspace unit
and integration test in an optimized build; the separate
`bombay-behavior-doctest` check runs the workspace doctests. The
`bombay-behavior` package check also invokes Cargo's test phase, repeating
those executable tests and the slow external macro consumer builds. In the
committed hash-policy gate, Nextest passed 834/834 cases while the package
check separately ran the same `crate_resolution` cases. The derived gate law
is one owner for each test class plus an independent package build: keep
Nextest as the workspace executable test gate, keep the doctest check, and
make the package derivation build without its duplicate test phase. No source
test, target, profile, package artifact, dependency policy, or public API is
removed. The focused regression is a full Nix check whose log must show all
workspace cases under Nextest and the doctests under their own derivation,
while the package derivation completes without another unit/integration test
run. Expected files are `flake.nix` and this audit; production Rust delta and
public type delta are both zero.

At signed commit `326f1c2`, the clean-worktree `nix flake check -L` passed all
eight active `aarch64-darwin` checks. Nextest ran 836/836 workspace tests;
the doctest derivation passed separately. The package derivation built and
installed with `doCheck = false` and no `checkPhase`, so it did not repeat the
workspace executable suite. The check omitted incompatible systems as usual.
No actor state, public spelling, production Rust line, or test case changed.
Disposition: `pass` for the gate ownership repair.

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

### A11 unused tuple-settlement export, before deletion

Classification: derived ordered-effect product law. A named send product
itself owns its declared lane names, settlement shape, left-to-right
interpretation, and intact unattempted suffix. The public, doc-hidden
`settle_in_order` function offers only a two-product tuple settlement; it
establishes no additional actor capability or independent invariant. A source
search across the Behavior workspace and adjacent Bombay/Address repositories
found its declaration and two re-exports but no Rust caller. Existing
named-product tests and external interpretation callers exercise their own
generated traversal, so deleting the dead export changes no transition,
effect, or ownership return. Expected production touch: core
`effects/sending.rs`, `effects/mod.rs`, and `lib.rs`, about 29 deleted lines,
zero new public types, and one removed public function; update the two
documents that name it. Core behavior control states, subordinate
alternatives, branches, and modules remain unchanged. The future-needed
values are each product's named lanes and complete settlements, already
retained by its implementation. No arrival history, repeated cause, false
cardinality, nested authority, semantic boolean, or structural caller syntax
is proposed. Cross-check: actor transition algebra and atomic runtime
settlement law. Disposition: `pass` for this deletion design, pending gates
and final measurements.

The unused function and both re-exports are removed. The core production
delta is `+2/-31/net -29` physical lines, tests `+0/-0/net 0`, public types
`+0/-0`, and public functions `-1`; `effects/sending.rs` is
`1,505 → 1,477` lines. Control states, subordinate alternatives, transition
branches, and module counts are unchanged, and each named product still owns
the same fields and settlement return. The source scan found no remaining
Rust call site; the Behavior, actors, and testkit Nextest run passed 808/808
cases, with formatter, book build, and diff checks passing. The residue and
law-document cross-checks above still hold. Disposition: `pass` for the
unused-export deletion; A11 remains open for generated logical projection.

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
named-product interpretation, tuple source custody, and wrapper composition
are reused;
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
A later public wrapper probe avoided E0446 but lost the concrete structural
product needed for recursive host traversal. Its `reopen` record is also in
`DEAD_ENDS.md`; no generated projection was retained. The existing
`behavior_generation` caller now uses a direct `PhantomData` type check in
place of a one-implementation `Same` trait to keep the exact structural
product requirement visible with less test machinery. All 18 cases in that
test binary passed under the Nix-pinned toolchain.

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

### A11 named-send equation inventory

The two current general derivations preserve the same named field order and
the same complete settlement alternatives, but they have different authoring
inputs. This is a pre-design comparison, not approval for a shared public
product framework.

| Equation | `send_product!` in `actors` | `#[behavior]` proc macro |
|---|---|---|
| Authored product | A generic named struct with semantic fields. | A behavior declaration generates a named sends struct, a distinct settlement struct, lane selectors, and fluent action methods. |
| Empty and append | Delegates to each field in declaration order. | Delegates to each field in declaration order. |
| Interpretation | Traverses fields in declaration order; corruption preserves the committed prefix and makes the untouched suffix unattempted. | The same ordered traversal and corrupt suffix equation. |
| Settlement status | Combines every named field. | Combines every generated settlement field. |
| Source custody | Offers each field in order; admission or closure retains the complete remaining product. | The same ordered offer and remaining-product equation. |
| Event-lane routing | Composes the fields' `SendsFor<Event>` proofs; authored code selects a concrete field. | Composes the fields' `SendsFor<Event>` proofs and generates `SendInput` selectors and fluent methods. |
| Logical hosts | Appends every field's `LogicalDeliveryProtocols` in interpretation order. | No generated `LogicalDeliveryProtocols` implementation is present. A lawful addition must distinguish logical deliveries from exact and interpreter-request lanes. |

`ReplyDeliveries` has a logical and an established route with distinct target
laws; `HeterogeneousShutdownSends` has its own finite target shape. Their
manual projections in `requirements.rs` are therefore evidence of semantic
exceptions, not merely missed macro invocations. The next A11 hypothesis must
prove the logical-host equation through two unrelated proc-macro behaviors and
both wrapper orders before it can subsume either derivation. The aggregate
drift checkpoint for this inventory has no retained production change: control
states, subordinate alternatives, branches, production lines, modules, and
public spellings are unchanged; no new history, cause, cardinality, transition
authority, semantic boolean, or positional consumer syntax is introduced.
Disposition: `pass` for the evidence record only.

The next focused A11 law is a derived logical-host projection: a generated
named send product contributes each field's `LogicalDeliveryProtocols` in its
declared order, including duplicates, while exact delivery and interpreter
request fields contribute their existing empty projection. The user-level
syntax is a `LogicalHostRequirements` bound on two unrelated generated
behaviors and `LogicalDeliveryProtocols` bounds on both `SendLayer` orders;
the expected type product spells the actual protocol order. Before changing
the proc macro, caller tests in `behavior_attribute.rs` and `compositions.rs`
must fail solely because the generated named sends lack this trait. Existing
field projections, `BirthProtocolProductAppend`, and wrapper composition are
the lower-order laws. No actor state, effect lane, transition, generic
parameter, runtime port, or policy is added. Pre-edit aggregate drift:
control states, subordinate alternatives, transition branches, production
lines, modules, and public spellings stay unchanged until the focused witness
fails; the generated product retains only its authored fields. No arrival
history, repeated cause, false cardinality, nested authority, semantic
boolean, or positional caller syntax is proposed. Cross-checks:
`actor-transition-algebra.md`, `behavior-layer-laws.md`, and normalized
atomic documents. Disposition: `pass` for the proposed projection model,
pending the red caller witness.

The two caller tests failed before production with only `E0277`: generated
`PrinterSends` and `GeneratedBaseSends` lacked
`LogicalDeliveryProtocols`. `Printer`'s direct logical-host bound and both
`SendLayer` orders failed for that missing trait; `GeneratedBase` failed for
the same reason. The first draft of the wrapper type assertion expected the
outer lane before the inner lane and produced an unrelated `E0271`; it was
corrected to the existing inner-before-outer law in `SendLayer` before this
red result was accepted. A 24-line generated-impl candidate then reopened the
design. It conflicted with an existing handwritten `BootstrapSends` impl and
leaked private protocol/request types from public generated products (`E0446`).
The candidate and focused test edits were removed; the exact falsifier and
post-experiment drift checkpoint are in `DEAD_ENDS.md`. The missing projection
remains open and requires an explicit visibility/ownership model before code.

Next A11 visibility hypothesis, before implementation: a generated named send
product is part of a public actor's interface only when that actor exports it.
The `#[behavior]` attribute is applied to an `impl`, so it cannot inspect the
actor declaration's visibility. The current unconditional `pub` product makes
private actors' private request and destination types leak through the
generated `LogicalDeliveryProtocols::Protocols` associated type. A Rust 1.95
scratch compile confirmed that a private product may implement this public
trait with a private projected protocol, while a public product triggers
E0446 for the same private protocol. The candidate syntax keeps
`sends = { ... }` module-private and permits the ordinary Rust visibility
spelling `sends = pub { ... }` when a public actor deliberately exports the
product. Generated lane selectors, settlement product, and fluent trait must
have the same visibility as the named product; no consumer supplies a no-op
route, marker, or policy. The generated projection appends every field's
existing `LogicalDeliveryProtocols::Protocols` in declared order, retaining
duplicates and the empty projections of exact and request lanes. Private
`Bootstrap`, `Printer`, and `GeneratedBase` are real callers; a public actor
with public recipient protocol will prove the exported form. The prior
two-behavior E0277 regressions and both `SendLayer` orders are the focused red
witnesses, supplemented by a red parser witness for `sends = pub { ... }`.
The existing `BirthProtocolProduct::Append`, field projections, product
settlement, and wrapper laws are the lower-order contracts. No actor control
state, event, effect lane, interpreter operation, or policy is authorized to
change. Pre-edit drift: product fields and order, actor states, transition
branches, production modules, and public actor spellings remain fixed; the
generated product's public exposure is the one intentional interface change
for private actors. The values needed later are each declared send lane's
logical-host protocol product and its exact settlement custody. There is no
arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or positional caller syntax in the proposed model.
Cross-checks are the actor transition algebra, behavior layer law, recursive
host consumer in `logical_host_requirements`, and normalized atomic docs.
Disposition: `pass` for the hypothesis only, pending focused red witnesses
and a full generated-product visibility inventory.

The focused pre-edit callers now fail in the intended places. `compositions`
reports three E0277 diagnostics because `GeneratedBaseSends` lacks
`LogicalDeliveryProtocols`: one direct owner and each `SendLayer` order.
`behavior_attribute` reports the same three E0277 diagnostics for
`PrinterSends`; its deliberately public actor is rejected at `sends = pub`
by the old parser, so the later missing `Behavior` diagnostic is a consequence
of that parse failure. No route, birth, or settlement mismatch appears in the
red logs. The public actor emits a real reply, and neither private caller
supplies an inert policy or placeholder.

The design-stage candidate now derives `LogicalDeliveryProtocols` from the
named fields and makes generated send products and their companions private
by default, with an explicit Rust `pub` form for an exported actor product.
The private `BootstrapSends` handwritten projection was deleted. The two
focused testkit suites passed 6 and 11 tests, including their exact direct
and both-order host products; core generated-product and source-admission
suites passed 19 and 6 tests. The duplicate-lane test retains two occurrences
of the same destination after an empty interpreter-request lane, while a
request-only product projects the empty host product. An external fixture
successfully names both the public send and settlement products and checks
its exact logical host type. All workspace targets passed `cargo check`
under the Nix toolchain. The macro unit suite passed 6 tests. The full Nix
gate for this candidate remains pending.

Post-design aggregate-drift checkpoint: actor control states, subordinate
state alternatives, transition branches, send field order, settlement
alternatives, and runtime ports are unchanged. The macro parser still has
the same Existing/Generated send alternatives; Generated now retains the
explicit Rust visibility needed for its public interface. The macro source
grew from 1,499 to 1,534 physical lines, including the visibility parser and
one generated logical-host implementation template. The eight-line
handwritten `BootstrapSends` projection disappeared; production module count
is unchanged. Existing generated product names are retained, but products
for private actors cease to be unnecessarily public, and the new exported
form is public by declaration. The surviving values are each named send lane,
its exact ordered settlement, and its statically projected logical-host
protocols. The residue scan finds no arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or structural
consumer syntax. The actor transition, wrapper-order, and recursive host
contracts were cross-checked. Disposition: `pass` for this focused projection
and visibility stage, pending its full gate. The wider A11 one-implementation
derivation remains open.

Clean `24fe931` Nix gate: all ten active aarch64-darwin checks passed,
including the optimized Nextest campaign (841/841). The four macro fixture
checks took about three minutes each under the shared fixture lock; all passed.

Next A11 consolidation hypothesis, before implementation: an authored generic
named send product and a `#[behavior]` generated named product have the same
field-order, empty/append, logical-host, interpretation, source-custody, and
settlement-status equations. Their Rust settlement representations differ:
the authored product is generic over each lane and reuses its own struct with
settled lane parameters, while the behavior attribute generates a separate
nominal settlement struct for concrete authored lane types. A shared
procedural generator should own the traversal equations, with that
representation difference explicit. The caller syntax for an authored
product is a normal named generic struct with a `SendProduct` derive; its
public fields, generic parameters, settlement associated type, and ordered
`Actions` remain identical. The first focused regression will use a real
two-lane effect fixture and fail on the old code because this derive does not
exist; it must then prove accepted, retained, closed, and corrupt suffix
custody. Two unrelated existing products, `DeliveryOutcomes` and `LeaseSends`,
and both `SendLayer` orders must pass before any catalogue migration. The
lower-order contracts are the current `send_product!` equations, the
generated product just proved above, `ActionItem` settlement products, and
the recursive logical-host proof. No new actor state, event, effect lane,
runtime port, or no-op caller input is authorized. Pre-edit aggregate drift:
all actor control sums, subordinate alternatives, transition branches,
product fields, and public spellings remain fixed. The candidate may add one
proc-macro entry point while deleting the 273-line declarative derivation
after proof; source-line reduction is diagnostic, not acceptance. Each lane's
owned settlement and declared order remain the only future-needed values.
The residue scan finds no proposed arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
caller syntax. Cross-checks: actor transition algebra, A11 equation table,
source settlement law, wrapper law, and normalized routing/timing contracts.
Disposition: `pass` for the hypothesis only; implementation must reopen if a
third representation or caller placeholder is required.

The pre-edit `SourceAdmissionSends<ProxySends, AssignmentSends>` caller now
uses the proposed derive on two real source-action lanes. It checks the exact
generic settlement type; empty/append; accepted first admission; closed
second admission with complete residual custody; corrupt first result and
unattempted second suffix; and rejected-result retention. On the old code,
its first diagnostic is E0433 for the absent `behavior_macros::SendProduct`;
the remaining seven diagnostics are the missing trait implementations that
this derive is meant to produce. No unrelated interpreter or route mismatch
appears in the red log.

Retained A11 derivation batch: `SendProduct` now derives the named generic
product contract for authored lanes. The two unrelated actor products
`DeliveryOutcomes` and `LeaseSends` passed the 167 actor unit tests in debug
and optimized builds. The source-custody fixture passed eight tests, including
both orders around `SendLayer`; the generated-product fixture passed nineteen.
All eleven former `send_product!` declarations compile through the derive,
and workspace `--all-targets` tests passed. The 273-line declarative macro
and its module were deleted. One procedural settlement generator now owns
classification and ordered source custody for both authored and generated
products; the interpreter traversal is shared as well. The remaining duplicate
effect-product trait generation between the two procedural paths must be
consolidated before A11 closes.

Aggregate-drift checkpoint for this retained derivation batch: actor control
states, subordinate alternatives, transition branches, effect lanes, and
public product spellings are unchanged (zero added or removed). Eleven macro
invocations became eleven ordinary named structs with one derive each; one
actor source module was removed. Every surviving field owns the same current
lane value needed for ordered interpretation and exact settlement return.
The residue scan found no new arrival-history state, duplicated cause, false
cardinality, nested transition authority, semantic boolean, or positional
consumer syntax. Cross-checks: the actor transition algebra, A11 equation
table, source-settlement law, both wrapper orders, routing and timing
contracts. Disposition: `pass` for the already-proven representation migration;
the remaining common trait generation is still open.

Follow-up A11 consolidation: `named_send_contract` now generates all five
send-product traits for both authored and `#[behavior]` products, while
`named_settlement_contract` generates their classification and source custody.
Only their truthful settlement representations and generated fluent lanes
remain separate. This removes another 100 net production lines from the macro
crate. The focused generated and authored tests (19 and 8) and workspace
`--all-targets` tests passed after the consolidation. The first clean Nix gate
for the preceding migration commit reached the documentation checker, which
found six consuming calls inside new assertions. The test moved those calls
before the assertions; its focused tests and `check_assertion_effects.py` now
pass. A clean gate on the corrected consolidation is pending.

Aggregate-drift checkpoint for consolidation: actor control sums, subordinate
alternatives, transition branches, product fields, modules, and public
spellings all remain unchanged (zero delta). The macro crate replaces two
copies of the same five trait equations with one generator; no current actor
value is added or removed. The residue scan remains clear for arrival history,
repeated causes, false cardinality, nested transition authority, semantic
booleans, and structural caller syntax. Cross-checks: actor transition law,
A11 equations, source-custody law, generated behavior contract, both wrapper
orders, normalized routing and timing documents. Disposition: `pass`; the clean
Nix gate passed all ten checks with 843/843 optimized tests.

### A12/A13 least-loaded membership owner, before edit

Classification: deliberate Bombay routing policy and derived Rust ownership
law. `Router` owns the ordered, duplicate-free member list. `LeastLoaded`
needs exactly one current `LoadEvidence` per member position, not a second
copy of each recipient. On add it appends Unknown; on removal it deletes that
position; observations find the member in the Router list and update only
the aligned evidence; selection uses the first minimum observed load. An
unknown, stale, or conflicting observation still returns the exact request.
The caller syntax is `Router<A, Recipient<P>, LeastLoaded>` and
`router.load_evidence(&recipient)`; it borrows for observation and exposes
neither an index nor a
route parameter on the policy's stored state. Two different protocol routes
can use the same policy type. The existing independent least-loaded trace is
the pure transition oracle; the external `protocol_bounds` compile witness
must first fail solely on the old required generic argument.

Before-edit aggregate control is one Router membership list, with
`LeastLoaded` holding a parallel list of recipient/evidence pairs. After the
candidate, Router remains the only member identity owner and the policy holds
an aligned evidence list. The subordinate sum remains Unknown or Observed
(version, load); its current version/load are needed for every later
selection and stale/conflict decision. Neither the actor's control states nor
its effects, errors, interpreter path, transition branches, or module count
change. Expected files: `routing/router.rs`, the external protocol caller,
the routing model, and this audit; expected production delta is negative and
no public type is added. The public policy loses one generic parameter and
its direct recipient lookup; Router gains one semantic evidence lookup.
The current `Router::new` and membership transitions are the only legitimate
policy-hook callers; direct policy use with an unrelated member slice must
return typed unknown evidence, not index-panic. The residue scan finds no
arrival history, duplicated cause, false cardinality, nested authority,
semantic boolean, or structural user syntax. Cross-checks are the actor
transition algebra and normalized routing law. Disposition: `pass` for the
model, pending the focused red compile witness and implementation.

Post-edit checkpoint: `protocol_bounds` failed before the production edit with
only the two expected E0107 diagnostics for `LeastLoaded`'s unwanted route
parameter, then compiled with `Router<MailAddr, Recipient<Destination>,
LeastLoaded>`. The independent 384-case least-loaded trace passed in debug
and optimized builds. All eight focused router unit tests passed, including
the exact owned observation returned when a direct policy caller supplies a
member slice without matching policy evidence. `mdbook build docs` passed.

The Router control state remains one ordered, duplicate-free membership list;
the four message alternatives and three Router error alternatives remain four
and three. Policy evidence remains the same two-alternative sum, Unknown or
Observed(version, load), and its three typed rejection alternatives remain
three. There is no new transition authority, module, effect, or interpreter
path. The production portion of `routing/router.rs` fell from 879 to 863
physical lines; one private recipient/evidence product disappeared. The
public policy loses its route parameter and direct identity lookup, while the
Router gains the recipient-based evidence lookup. Router owns current member
identity and order; the aligned policy evidence owns the current load and
version needed for selection and stale/conflict decisions. Rechecking the
actor transition algebra and normalized routing laws found no arrival-history
state, repeated cause, false cardinality, nested transition authority,
semantic boolean, or positional syntax exposed to callers. Disposition:
`pass`.

### A12/A13 hash-token membership owner, before edit

Classification: deliberate Bombay hash-routing policy and derived Rust
ownership law. The Router already owns its ordered, duplicate-free recipient
list. Both hash policies need exactly one current Unknown or Observed(version,
token) evidence value at each member position; neither needs a second owned
copy of each recipient. Add appends Unknown, removal deletes the same position,
observation resolves the recipient in the Router snapshot and updates only
that position, and selection considers the observed tokens in membership
order. Unknown, stale, and conflicting observations must return their exact
owned request. The caller syntax is `ConsistentHash<K>` or
`RendezvousHash<K>` inside `Router<A, Route, _>`, with
`router.member_token_evidence(&recipient)` for current evidence. One policy
type must work with different eligible route types for the same key.

The focused pre-edit caller regression is the external keyed-routing test's
`Router<..., RendezvousHash<u64>>` and a corresponding `ConsistentHash<u64>`
type witness; they must fail only because the old policy demands an unrelated
route parameter. Existing rendezvous membership/permutation properties and
the consistent-hash removal test are the independent transition witnesses.
The direct policy boundary must return `UnknownRecipient` with the complete
observation when its passed member slice has no aligned evidence, without an
index panic. Existing `RoutingStrategy` hooks, `MemberTokenObservation`, and
the two concrete selectors are reused; no runtime or effect product changes.

Aggregate-drift precheck: Router retains one ordered member list before and
after. `HashMembership` changes from a second list of recipient/evidence
products to the aligned evidence list; its Unknown and Observed alternatives
stay two, and their current version/token remain necessary for later
selection and stale/conflict decisions. Router's four messages and three
errors, the hash policies' three rejection alternatives, all actor control
states, transition alternatives, modules, and interpreter paths stay the
same. Expected edit files are `routing/router.rs`, the keyed routing caller,
this audit, and any focused direct-policy regression. Expected production
delta is negative, with one private product and two public route parameters
removed, no public type added, and one Router evidence lookup added. The
residue scan finds no arrival history, repeated cause, false cardinality,
nested authority, semantic boolean, or structural caller syntax. Cross-checks
are the actor transition algebra and normalized routing law. Disposition:
`pass` for the proposed model, pending the red witness and implementation.

Post-edit checkpoint: the external keyed-routing caller failed on the old
policy shape with exactly two E0107 diagnostics, one for each unwanted route
parameter. The new caller syntax compiles. The nine focused router unit tests
pass, including complete typed return of an untracked observation from both
hash policies. All nine routing invariant tests pass in debug and optimized
builds; their rendezvous trace includes membership edits, evidence versions,
selection, and token-order permutation. `mdbook build docs` passes. The full
workspace gate for this batch is pending.

The Router control state and its four commands/three errors are unchanged.
The policy evidence still has two alternatives and the hash rejection still
has three. The production portion of `routing/router.rs` fell from 863 to 850
physical lines, despite adding the recipient-based Router lookup; the private
`HashMember<Route>` product and the second recipient list disappeared. Both
public policies lost only their route parameter and direct recipient lookup;
their key parameter remains because two key types and their hash functions
are valid substitutions. No new type, trait, module, effect, actor transition
branch, or interpreter path was introduced. Router retains member identity
and order; the policy retains only current version/token evidence. The shared
private member-index operation serves all three evidence lookups. Rechecking
the actor transition algebra and normalized routing law found no arrival
history, repeated cause, false cardinality, nested transition authority,
semantic boolean, or structural caller syntax. Disposition: `pass` for this
representation, subject to the pending full gate.

### A12/A13 router observation rejection custody, before edit

Classification: derived affine Rust ownership law; neither Agha's transition
effects nor hash-routing policy prescribes a Rust error representation. A
policy consumes one typed observation. If it cannot accept that observation,
it must return the same owned observation with one concrete reason, and
Router must retain exactly that returned value while discarding the mutated
policy candidate. Acceptance commits the candidate and returns empty
continuing `Actions`. There is no need for an observation to implement
`Clone`: it can own a move-only payload. The current trait returns only
`Self::Error`; Router therefore clones the input first, and the built-in
`LeastLoadedError<Route>` and `HashPolicyError<Route>` carry a second copy of
it. One semantic cause has two owners and every runnable policy observation
inherits a cloning bound.

The caller syntax is a running `Router<MailAddr, Recipient<Destination>,
ObservedSelection>` accepting the existing non-cloneable
`AcceptedPayloads(Vec<u8>)`, followed by a rejecting move-only observation
whose exact allocation returns through `RouterError::Policy`. The external
`protocol_bounds` caller must first fail only because the old Router behavior
requires `R::Observation: Clone`. A focused pure transition test will then
prove pointer identity, complete rejected ownership, policy-state rollback,
and one successful commit. The public policy seam should return one named
observation/reason product; the aggregate `RouterError::Policy` remains the
sole actor error owner. Once the observation moves to that product,
`LeastLoadedError` and `HashPolicyError` have the same three payload-free
reason alternatives and ownership equation. One shared
`MemberEvidenceError` should replace both, without a compatibility alias.
`RoundRobin` remains statically unable to observe. Existing `RoutingStrategy`
hooks, typed `RouterMessage`, policy candidate rollback, delivery effects,
and interpreter path are reused. No policy needs a no-op placeholder.

Pre-edit drift checkpoint: Router retains one ordered member list and the
same four message and three actor-error alternatives. Least-loaded evidence
remains Unknown/Observed(version, load); hash evidence remains
Unknown/Observed(version, token). The shared policy reason retains the same
three alternatives without duplicating the observation. The proposed public
surface adds one named rejection product and one shared reason sum, removes
two policy-specific reason types and the observation-clone bound from Router's
transition; no new actor state, effect, interpreter capability, module, or
transition branch is authorized. Expected production delta is negative after
removing redundant payloads, manual Debug implementations, and cloning.
Current membership/evidence and the actual rejected observation are exactly
the values needed for later decisions and ownership return. The residue scan
finds no arrival history, repeated cause, false cardinality, nested
authority, semantic boolean, or structural caller syntax in the proposed
model. Cross-checks are `actor-transition-algebra.md`, the routing law in
`atomic-actor-other-templates.md`, and existing wrapper composition tests.
Disposition: `pass` for the proposed model, pending red caller and pure
transition witnesses.

The external caller now constructs and advances the existing
`ObservedSelection` with non-cloneable `AcceptedPayloads`. Before any
production edit, its explicit Behavior witness fails with E0277 naming only
the missing `AcceptedPayloads: Clone` bound; the subsequent `initialize`
E0599 is the same unmet Behavior obligation. There is no unrelated route,
send, or address diagnostic. The acceptance action and committed policy state
are asserted in the caller test. Before production code, the rejecting
move-only custody regression failed because the named rejection product did
not exist and the Router still required `Observation: Clone`. After the
interface and built-in policy changes, all 11 focused router unit tests and
all four external protocol-bound tests pass. The pure rejection test checks
the returned `Box` allocation address, complete reason, discarded candidate
mutation, subsequent accepted observation, empty observation effects, and
the resulting delivery. LeastLoaded and the two hash policies use the same
rejection product; RoundRobin's uninhabited observation remains unchanged.

Post-design drift checkpoint: Router still has one control state, four
messages, three actor-error alternatives, and the same four transition arms.
Least-loaded and hash evidence each retain Unknown/Observed; their two
three-alternative reason sums became one three-alternative sum. The production
portion of `routing/router.rs` grew from 850 to 866 physical lines because
each rejection now explicitly returns its owned observation and reason; two
manual Debug implementations and the Router observation clone disappeared.
The two removed public error names were replaced by the shared reason and
named ownership product, so the public type count is unchanged. Modules,
effect lanes, and interpreter capabilities are unchanged. The current
member evidence is the only policy-local value needed for the next selection;
the returned observation is needed only for the exact rejection. The residue
scan finds no arrival history, repeated cause, false cardinality, nested
transition authority, semantic boolean, or structural caller syntax. The
actor transition, routing, and wrapper composition laws were cross-checked.
Disposition: `pass` for the design stage; testkit migration and the full gate
are separate follow-up work.

Mechanical caller migration at `f936dc8` changed only the routing invariant
test's three rejection patterns to inspect the one returned observation and
the shared reason. Its generic route witness no longer asks for an unrelated
observation-cloning bound. All 10 focused routing invariant tests pass in both
Nix-pinned debug and optimized builds. This stage adds no production type,
transition, branch, module, or policy.

The clean-worktree `nix flake check -L` at signed commit `26d2261` passed all
eight active `aarch64-darwin` checks, including optimized Nextest with
838/838 passing cases, doctests, Clippy, Rustdoc, documentation links,
formatting, package build, and deny. This closes the gate for the router
observation-custody design and its mechanical test migration; A12/A13 retain
their broader audits.

### A13 pre-edit base-projection law

Classification: derived, read-only composition law. `BehaviorBase` on an
unwrapped catalogue actor returns `&Self`; it neither constructs nor advances
that actor. Naming this projection must therefore require only the bounds that
make the actor type well formed. `Clone` and `Eq` on a command key or payload
belong to construction or transition when those operations actually copy or
compare values. The caller syntax is a `BehaviorBase<Base = Self>` bound on
`Acknowledgements`, `Resolver`, `Configuration`, and `Readiness` with opaque
non-`Clone`, non-`Eq` domain values. Its observable product is the same shared
reference, with no action or state transition. The focused external
`protocol_bounds` witness must fail only because the current base-projection
impls carry those operation bounds. Existing `Protocol` witnesses and the
runtime transition tests are the lower-order contracts; no new effect lane,
interpreter operation, wrapper, or actor state is proposed. Before-edit
control states, subordinate alternatives, transition branches, and public
spellings are unchanged. The current values in each actor remain its exact
records, bindings, versioned configuration, or dependency observations. The
residue scan finds no proposed arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
caller syntax. Cross-check: `BehaviorBase` in `transition.rs`, the actor
transition algebra, and the normalized catalogue contracts. Disposition:
`pass` for the proposed law. The external witness failed before production
edits with ten `E0277` diagnostics, all from the four named `BehaviorBase`
impls demanding `Clone` or `Eq` for opaque `Key`/`Value`. There was no route,
protocol, or associated-type failure. This is one repeated interface-bound
error, so the edit is limited to removing those operation bounds from the
four read-only impls.

The same derived read-only law applies to `Machine`, `Topic`, `PubSub`,
`Presence`, `Lease`, `Barrier`, `Workflow`, and `OrderGate`. Their base method
also returns `&Self`, while the current impls demand copying, equality, or
ordering used by other operations. The second focused caller stage names
`BehaviorBase<Base = Self>` for the already opaque payload/key/phase types,
including an `OrderGate` whose key has no `Ord`. It must fail before editing
these eight impls solely on their operation bounds. The existing actor
type/route bounds stay in place. No state or action changes; the ownership,
residue, and law cross-check above apply to this extension too.

The initial caller draft reused protocol-only topic aliases whose
`Subscription` deliberately is not a delivery route; that unrelated error
was corrected before accepting the red result. With lawful concrete routes,
the second stage failed with sixteen `E0277` diagnostics, all for the eight
impls' copying, equality, or ordering bounds. No route error remained.

After both stages, the focused external caller passes all three cases and
workspace Nextest passes 827/827. The twelve production files changed by
`+0/-16` physical lines (`1,517 → 1,501`); the external caller gained one
compile witness with twelve concrete substitutions. Aggregate control sums,
subordinate alternatives, transition branches, modules, public spellings,
stored values, effect lanes, and interpreter operations are unchanged before
and after. Every surviving state alternative still owns the same current
value. The residue scan found no arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
caller syntax. The `BehaviorBase` contract and catalogue laws remain the
cross-check. Disposition: `pass` for this bound-only batch; A13 remains open
for other public ports and protocol bounds.
The complete `nix flake check -L` passed all eight declared local checks at
signed commit `790d194`, including the optimized workspace Nextest and
release-test lanes. Later routing bound and test-only batches passed their
focused Nix tests; they still need the final flake run together.

### A13 pre-edit routing identity law

Classification: derived, typed-protocol identity law. `Deduplicator` and
`OrderGate` command types name a key and two concrete delivery routes;
identifying those messages neither compares nor copies the key. The
`Deduplicator` base projection also only borrows `Self`. The caller syntax is
an external `Protocol<Msg = ...>` bound for both actors and a
`BehaviorBase<Base = Self>` bound for `Deduplicator`, with an opaque key that
implements no `Clone`, `Eq`, or `Ord`. The complete observable product is the
same associated message type or shared reference, without a transition.
The focused protocol caller must fail on the old impl-only key bounds before
production changes. Existing route and actor type bounds, transition tests,
and the preceding base-projection witness are lower-order contracts. The
candidate changes no event, effect, state, runtime port, wrapper, or public
spelling. Baseline control sums, subordinate alternatives, transition
branches, and modules remain fixed; the deduplicator retains its FIFO key
window and the gate its watermark and held map. The residue scan finds no
proposed history, duplicate cause, false cardinality, nested authority,
semantic boolean, or positional caller syntax. Cross-check: actor transition
algebra and the routing catalogue contract. Disposition: `pass` for the
pre-edit law. The external caller failed before production with six `E0277`
diagnostics: `Clone`/`Eq` from the deduplicator's two impls and
`Clone`/`Ord` from the order gate's protocol impl. No route or message-shape
error occurred.

The focused external caller and the 827-case workspace Nextest suite pass
after removing those three impl-level lines. These two production modules
changed `+0/-3` physical lines (`752 → 749`); test syntax adds two protocol
substitutions and one base-projection substitution. Control states,
subordinate alternatives, transition branches, modules, public spellings,
effect lanes, interpreter operations, and retained values are unchanged.
The deduplicator still owns its FIFO key window; the order gate still owns its
watermark and held map. The residue scan and law cross-check above remain
clear. Disposition: `pass` for this interface-only batch; A13 remains open.

### A13 pre-edit priority-protocol law

Classification: derived, typed-protocol identity law. A priority-queue
command carries an application priority as owned data; only construction and
release ordering require `Ord`. `BinaryHeap<Entry<T, P>>` can be stored before
`Entry<T, P>: Ord` is available; the ordering proof is needed when operations
use the heap. The external syntax is a `Protocol<Msg =
PriorityQueueMessage<...>>` and `BehaviorBase<Base = Self>` bound with an
opaque priority type. Neither syntax constructs or transitions the queue.
The prior design is expected to fail this caller solely at the struct's
`P: Ord` bound. The existing priority selection/FIFO trace and construction
tests witness the lower-order operation contract. The candidate removes the
bound from the aggregate declaration and the two read-only impls, retaining
it on construction and `Behavior`. It adds no new state, effect, policy,
interpreter operation, or public spelling. Baseline control phases remain
Active and Exhausted; the current values remain capacity, next token, and
owned heap entries. Transition branches, subordinate alternatives, modules,
and wrapper products are unchanged. The residue scan finds no proposed
arrival-history state, repeated cause, false cardinality, nested authority,
semantic boolean, or positional caller syntax. Cross-check: the actor
transition algebra and routing priority law. Disposition: `pass` for the
pre-edit model. The external caller failed before production with exactly two
`E0277` diagnostics, both requiring `Value: Ord` at protocol identity and
base projection. No route, message, or heap-storage error appeared.

The focused caller and all 827 workspace Nextest cases pass after the bound
move. The production module changed `+1/-3`, net two fewer physical lines
(`446 → 444`); no transition branch changed. Its control phases remain
Active/Exhausted, the heap still owns the same entries, and all priority
comparison remains on construction and `Behavior`. Subordinate alternatives,
modules, public spellings, effect lanes, and interpreter operations are
unchanged. The residue scan found no arrival history, repeated cause, false
cardinality, nested authority, semantic boolean, or positional consumer
syntax. The actor transition and routing priority laws were cross-checked.
Disposition: `pass` for this bound-only stage; A13 remains open.

No-op and compiler-friction checkpoint for the three A13 bound batches:
they remove 21 net production lines of repeated read-only/identity bounds,
with zero new public types, traits, policies, adapters, or wrapper syntax.
An unrelated `BehaviorLayer` does not require a caller edit, and no public
name describes structural position. The external test file grew 47 net lines
of direct compile witnesses; eight aliases (11 physical lines) name existing
concrete routes and products, so test protocol plumbing did not exceed the
21-line production deletion. No test supplies a no-op policy or discarded
effect to satisfy the new surface. The remaining `Router` trait-level bound
is a separate design question rather than compiler fallout from these edits.

### A13 pre-edit protocol-bound law

The next A13 protocol-identity witness is `WorkQueue` with a
`ReplyRoute<ExactDestination>` worker route. `ReplyRoute` is a sealed, lawful
`DeliveryRoute` and can carry logical or exact recipients, but deliberately
does not claim `PartialEq` between them. The derived identity law requires
only the address and the two route protocol/message relationships to name
`WorkQueueMessage`; worker-route cloning and equality belong to queue state
inspection and availability transitions. The caller syntax is an external
`Protocol<Addr = ExactAddr, Msg = WorkQueueMessage<...>>` bound, with no
construction or transition. On the prior representation the intended focused
test must fail solely because the aggregate declaration and protocol impl
require `ReplyRoute<ExactDestination>: PartialEq`. The existing protocol-only
caller suite, `Recipient`, `ReplyRoute`, and `DeliveryRoute` are the lower-order
witnesses. The candidate moves the bound to existing methods/`Behavior`, adds
no semantic state, effect, public type, or runtime port. Its control states,
subordinate alternatives, branches, modules, and public spellings stay
unchanged; the route remains the same owned field. No history, repeated cause,
false cardinality, nested authority, semantic boolean, or positional syntax
is proposed. Cross-checks: `actor-transition-algebra.md`, `behavior-layer-laws.md`,
and the normalized FIFO law. Disposition: `pass` for the pre-edit model,
pending the red caller witness and measured implementation.

The first draft witness also failed because the ordinary `MailAddr` lacks an
exact-endpoint family; that failure was unrelated to route equality. The
corrected external caller uses an `EndpointAddress` and failed with only
`E0277`: `ReplyRoute<ExactDestination>` does not implement `PartialEq`, which
the previous `WorkQueue` declaration demanded. Moving `Clone + PartialEq`
from the aggregate declaration, `BehaviorBase`, and `Protocol` to the existing
construction/transition impls made that caller compile without changing queue
operation. The measured source change is production `+7/-4/net +3`, including
the public bound explanation, test `+35/-3/net +32`, and public API `+0/-0`
types. The one aggregate module grows from 263 to 266 production lines; the
direct state product remains capacity,
available workers, and waiting jobs, with no control-state enum or subordinate
sum. The five production `if` selections and three command arms remain eight
transition branches by the same count before and after. Every stored route is
still required for later dispatch or withdrawal. The residue scan found no
new arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or structural caller syntax. Disposition: `pass` for this
protocol-bound batch; the rest of A13 remains open. The focused debug and
optimized caller tests, two FIFO unit tests, workspace all-target compile,
all 824 workspace Nextest cases, and the documentation book build passed
through Nix. The complete `nix flake check -L` passed all ten declared checks
at signed commit `870f4b2`, including the optimized 824-case Nextest run,
release tests, doctests, Clippy, package, documentation, formatting,
dependency-audit, and dependency-policy gates.

The same derived protocol-identity law applies to `Topic` and `PubSub`:
their message sums contain owned publication, topic, and route values, and
name an address, without requiring those values to be cloned, compared, or
delivered. The intended caller syntax projects `Protocol::Msg` from each
actor with non-`Clone`, non-`Eq` publication and route types. The focused
`actors/tests/protocol_bounds.rs` compile witness must fail on the prior
impls. `Clone`, `Eq`, `PartialEq`, and `DeliveryRoute` remain necessary on the
actual transition impls. The lower-order `TopicMessage`, `PubSubMessage`,
`Protocol`, and `Behavior` contracts already express this distinction; no
new protocol, effect lane, wrapper, or interpreter capability is proposed.

Pre-edit aggregate-drift checkpoint: both actors have one membership state
(`Vec<Route>` for `Topic`, `Vec<TopicMembership<K, Route>>` for `PubSub`) and
the same control states, subordinate alternatives, transition branches,
production lines, modules, and public spellings before and after this proposed
bound change. The future-needed values are each ordered subscriber route,
and for `PubSub` each retained topic identity. No arrival-history state,
repeated cause, false cardinality, nested authority, semantic boolean, or
positional consumer syntax is proposed. The discovery contracts and the
`Protocol`/`Behavior` distinction in `actor-transition-algebra.md` are
cross-checked. Disposition: `pass` for the proposed narrower protocol law,
pending its pre-edit regression and implementation.

The pre-edit caller produced ten E0277 diagnostics for transition-only
`DeliveryRoute`, `Clone`, `Eq`, and `PartialEq` bounds. After narrowing only the
two `Protocol` impls, the caller passes; the `Behavior` impls retain their
transition bounds. Aggregate control states remain one for each actor;
subordinate state and result alternatives and transition branches remain
unchanged. Production lines in these two files changed from 323 and 184 to
317 and 179; modules and public spellings remain unchanged. The ordered
subscriber routes and retained topic identities remain the future-needed
state, with no residue from the pre-edit scan. The discovery and actor
transition law cross-check remains valid. Disposition: `pass` for this focused
bound repair; A13 remains open for the rest of the public surface.

The same identity equation also applies to `Machine<A,S,M,P,E>`: its protocol
is `(A, M)`, independent of phase copying and comparison. A caller must be
able to project `Protocol::Msg = M` while `P` lacks `Copy` and `PartialEq`;
execution still requires those bounds. The focused caller is added to
`actors/tests/protocol_bounds.rs` before production edit. Existing `Machine`,
`Protocol`, and `Behavior` are the only required layers, with no interpreter
effect or wrapper change. Pre-edit drift checkpoint: the machine's state,
held queue, phase, transition function, one control state, subordinate
`Advance` alternatives, branch count, modules, and public spellings are
unchanged by this bound proposal. Its future-needed values are the current
phase, owned state and held messages. No arrival history, repeated cause,
false cardinality, nested authority, semantic boolean, or positional syntax
is proposed. The machine and actor transition law documents were
cross-checked. Disposition: `pass` for the proposed bound repair, pending
the failing caller and implementation.

The pre-edit machine caller failed with E0277 for `Phase: Copy + PartialEq`.
After narrowing `Machine`'s `Protocol` impl, it passes while its constructor,
phase access, and `Behavior` impl retain the execution bounds. Production
lines in `machine.rs` changed from 198 to 194; all state alternatives,
transition branches, modules, and public spellings remain unchanged. The
future-needed values and residue scan are unchanged from the pre-edit
checkpoint. Disposition: `pass` for this protocol-only repair.

The next A13 protocol-bound batch applies that already-proven identity law to
`Configuration`, `Health`, `Readiness`, and `Registry`. Their command types
contain owned configuration or key values; naming those commands does not
clone or compare them. The route and destination bounds remain on each actor
struct because those types currently define its retained delivery capability;
the proposed edit removes only `C: Clone + Eq` or `K: Clone + Eq` from its
`Protocol` impl. A caller projects each exact command sum with a non-`Clone`,
non-`Eq` value and a lawful concrete result route. The compile regression in
`actors/tests/protocol_bounds.rs` precedes the four source edits. Existing
`Protocol`, command sums, result protocols, `DeliveryRoute`, and `Behavior`
are reused. No interpreter, wrapper, effect lane, or public spelling changes.

Pre-edit aggregate-drift checkpoint: configuration has one current
`ConfigurationState`; health retains ordered component states; readiness
retains ordered fixed dependencies; registry retains ordered key-recipient
bindings. Their control states, subordinate alternatives, transition
branches, modules, and public spellings do not change. The future-needed
values remain respectively the current version/value, component evidence,
dependency evidence, and key-recipient binding. There is no proposed arrival
history, repeated cause, false cardinality, nested authority, semantic
boolean, or positional syntax. The operations/discovery contracts and
`actor-transition-algebra.md` were cross-checked. Expected source change:
four `Protocol` impls, roughly four bound lines deleted; no new public type.
Disposition: `pass` for the proposed identity-law migration, pending its
failing caller and implementation.

The pre-edit caller failed with eight E0277 diagnostics, two for each actor's
unneeded `Clone + Eq` requirement. It passes after removing only those four
`Protocol` bound lines. Execution, construction, and retained route bounds
remain unchanged. The four aggregates retain all prior current values,
alternatives, transition branches, and modules; production source is four
lines smaller and public spellings are unchanged. The pre-edit residue scan
and law cross-check still hold. Disposition: `pass` for this identity-law
migration. A13 remains open for bounds on other protocols and the remaining
public-surface review.

The same derived identity law reaches `Correlator`, `Acknowledgements`, and
`Barrier`: their command sums can carry non-`Clone`, non-`Eq` correlation keys,
payloads, and barrier members while a lawful result route remains named.
`Clone`/`Eq` are execution bounds; their structs and message sums do not
require them. The caller projects the three message sums in
`actors/tests/protocol_bounds.rs` before production edits, using the existing
`Recipient<MessageProtocol<...>>` route. The proposed edit only removes
transition bounds from three `Protocol` impls, reusing their existing result
protocols, routes, and `Behavior` impls. No wrapper or interpreter path changes.

Pre-edit aggregate-drift checkpoint: correlator retains its ordered key
lifecycle states; acknowledgements retains ordered records; barrier retains
member order and its current generation/state. Their control states,
subordinate alternatives, transition branches, modules, and public spellings
are unchanged by this proposal. The future-needed values remain the keys,
reply recipients, pending acknowledgement payloads, members, and generation.
The residue scan finds no proposed arrival-history state, repeated cause,
false cardinality, nested authority, semantic boolean, or positional syntax.
The routing, workflow, and actor transition contracts were cross-checked.
Expected source delta is four deleted bound lines, no new public types.
Disposition: `pass` for the proposed identity-law migration, pending its
failing caller and implementation.

The pre-edit caller failed with eight E0277 diagnostics for `Key` and `Value`.
After removing four bound lines from the three `Protocol` impls, that caller
passes; the execution impls retain their bounds. Current states, subordinate
alternatives, branches, modules, and public spellings are unchanged. The
retained values, residue scan, and routing/workflow law cross-check remain as
recorded above. Disposition: `pass` for the focused identity-law migration.

`Presence`, `Lease`, and `Workflow` expose one further instance of the same
derived law. Each public struct requires `K: Clone + Eq` merely to name its
type, although its private fields can store a non-`Clone`, non-`Eq` `K` and its
message sum can own one. The constructor and `Behavior` impl genuinely need
those bounds for their policies; the protocol identity does not. The intended
caller projects `Protocol::Msg` for each actor with a non-`Clone`, non-`Eq`
key and a lawful result route. The compile witness is added to
`actors/tests/protocol_bounds.rs` before touching production. The change
removes the bound at each struct declaration and its `Protocol` impl only;
constructor and execution bounds remain, and no new trait or port is needed.

Pre-edit aggregate-drift checkpoint: presence retains its ordered records and
timer mapping, lease retains one vacant/held/exhausted state and timer ID, and
workflow retains its validated definition and current run state. Their
control states, subordinate alternatives, branches, modules, and public
spellings do not change. The future-needed values are respectively the
participant records, current lease holder/generation, and workflow steps and
result route. No arrival-history state, repeated cause, false cardinality,
nested authority, semantic boolean, or positional syntax is proposed. The
discovery, time, workflow, and actor transition contracts were cross-checked.
Expected production delta is six bounds removed across three files, three
physical source lines deleted, and no new public types. Disposition: `pass`
for the proposed bound relocation, pending the focused failing caller and
implementation.

The pre-edit caller failed with six E0277 diagnostics at the three struct
declarations. With their `K` parameters unbounded and the same bounds removed
from their `Protocol` impls, it passes. Constructors and transitions retain
their `Clone + Eq` requirements; the focused presence, lease, and workflow
transition tests pass. The three production files contain three fewer physical
lines, with unchanged states, alternatives, branches, modules, and public
spellings. The future-needed values, residue scan, and cross-checked laws
remain as recorded above. Disposition: `pass` for this bound relocation.

`Router` has a distinct protocol-only bound: its strategy's observation must
be `Clone` to execute the current rollback transition, but the public
`RouterMessage` can own an observation without cloning it. An application
strategy with a non-`Clone` observation must still be able to name the router
protocol. A focused compile caller in `actors/tests/protocol_bounds.rs`
implements a real membership-selection and observation-update policy, then
projects `Protocol::Msg`; it precedes the proposed one-line `Protocol` bound
removal. `RoutingStrategy`, `RouterMessage`, `Recipient`, and `Behavior` are
the existing pieces. No runtime, wrapper, or effect lane changes.

Pre-edit aggregate-drift checkpoint: router retains its ordered recipients
and one concrete strategy. Its control state, subordinate alternatives,
transition branches, modules, and public spellings do not change. Future
decisions need that recipient order and the strategy's current observation
state. No arrival-history state, repeated cause, false cardinality, nested
authority, semantic boolean, or positional syntax is proposed. The routing
and actor transition contracts were cross-checked. Expected production delta
is one deleted bound line, no new public type. Disposition: `pass` for the
proposed narrower identity law, pending the failing caller and implementation.

The pre-edit caller failed with E0277 for its owned, non-`Clone` observation.
After deleting `R::Observation: Clone` from only the router's `Protocol` impl,
it passes; the transition impl retains the rollback bound. The existing router
transition tests pass. One production line was removed; state, alternatives,
branches, modules, public spellings, future-needed values, and the residue
scan are unchanged. Disposition: `pass` for this identity-law repair.

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

Compile-cost comparison for those two bounds, on `aarch64-darwin` with the
Nix-provided Rust 1.95 toolchain: two detached checkouts at `1559a87` differed
only by restoring `K: Clone + Eq, V: Clone` to `Cache`'s `Protocol` impl and
`K: Clone + Eq` to `Resolver`'s `Protocol` impl. Both used the same warm
`CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, and
`cargo check -p bombay-behavior-actors --lib --locked`. Before each measured
run, `cargo clean -p bombay-behavior-actors` removed only the actor package's
artifacts. Both runs visibly checked that crate: restored bounds took 187.07
seconds wall time; narrower bounds took 187.84 seconds. The 0.77-second
difference is under 1% and does not establish a compile-time improvement.
The supported benefit is the narrower protocol law and the removal of five
misleading E0277 caller diagnostics. This pair does not measure every generic
bound or the full workspace build.

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

### A15 timer admission outcome, before implementation

The earlier A15 pass covered the listed fixtures, but a repository-wide scan
found a remaining semantic boolean: `TimerLease::accept` and
`OneShotSchedule::accept` mutate the schedule and return `bool`. Their callers
use that value to choose whether to run a timer reaction. Reopen A15 for this
specific policy violation; the prior evidence remains valid for its scope.

Classification: deliberate Bombay timer policy, not an actor-model guarantee.
An event matching the currently armed timer is admitted once and consumes that
schedule. A foreign, stale, duplicate, cancelled, or exhausted event is
ignored without changing schedule state. The complete local result is
`TimerAdmission::{Accepted, Ignored}`; the wrapper still emits the same
`Actions` and invokes the same reaction only on `Accepted`. The caller syntax
matches that sum instead of using a mutating boolean in a guard. The focused
domain regression checks exact, duplicate, cancelled, foreign-ID, and
foreign-generation arrivals, including the surviving schedule. Existing
timer-wrapper tests and both wrapper orders remain lower-order witnesses.

Pre-edit aggregate-drift checkpoint: `TimerLease` retains its four states
(`NeverIssued`, `Armed`, `Idle`, `Exhausted`), and `OneShotSchedule` retains its
two states (`Unscheduled`, `Scheduled`). The new return sum represents one
transition's disposition; it stores no future state. The future-needed values
remain the armed generation and the one-shot ID, generation, and deadline.
No actor aggregate state, public spelling, or effect lane changes. The edit is
expected to touch the domain and four wrappers, replacing two boolean returns
and four guard uses with one private sum and exhaustive matches. Record exact
line, branch, and module deltas after the edit. The residue scan found no
arrival history, repeated cause, false cardinality, nested transition
authority, or positional user syntax. Cross-check the explicit effect law in
`docs/actor-transition-algebra.md` and the timer wrapper Rustdoc.
Disposition: `pass` for the pre-edit model.

The pre-edit Nix-toolchain compile witness failed with seven `E0433`
diagnostics because `TimerAdmission` did not exist. After implementation,
the two mutating domain operations return the private `Accepted | Ignored` sum,
and Deadline, OneShot, Periodic, and ReceiveTimeout match it exhaustively.
The original ID guard and all action lanes remain in place. Domain tests
passed 3/3 in debug and optimized builds; all nine actor time unit tests
passed in both profiles. The actor timer-settlement integration target passed
4/4, including both wrapper orders, and the independent receive-timeout model
target passed 6/6. Full repository gates remain pending for this batch.

Post-edit aggregate-drift checkpoint: TimerLease remains four states and
OneShotSchedule two; no stored subordinate alternative or future-needed value
changed. The former boolean result is the new two-case local admission sum.
Across the four wrappers, top-level event arms changed from 12 to 11 because
Deadline now matches its owned arrival once; eight explicit admission arms
replace four boolean guard decisions. The five affected modules remain five,
and no public spelling changed. Measured production diff is +56/-34 (net +22)
lines; the focused tests are +22/-3 (net +19). This is a domain-model repair,
not a code-reduction claim. No arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or structural
caller syntax remains in this timer admission path. The actor effect law and
all four wrapper Rustdoc contracts were cross-checked. Disposition: `pass`
for this retained batch, pending the full Nix gate before A15 closes.

The same scan found `InactivityModel::notification` in the independent
testkit: it consumes a live token, returns a semantic `bool`, and three caller
assertions perform that mutation inside `assert!`. This is a second A15
checkpoint. The testkit policy is that one matching notification consumes and
returns its exact current token; a stale or duplicate notification returns
absence and leaves the live token unchanged. `Option<u64>` is the direct domain
representation of one accepted token or absence, with no new protocol type.
The caller must bind the outcome before asserting, so debug and optimized
builds execute the same transition. A focused testkit caller expecting
`Some(1)` versus `None` is the pre-edit compile regression. The lower-order
witnesses are the existing receive-timeout model trace, the actor timer-domain
test, and both timer wrapper orders. This changes no production actor state,
interpreter path, or effect lane. The model keeps its two optional tokens;
its future-needed value is still the current live token. Expected files are
the model and its one caller test. Record post-edit measurements and gates
before closing A15.

The standalone Nix-pinned `rustc` caller included the actual testkit model
module and required `notification` to return `Option<u64>`. Before the edit it
failed with the intended `E0308` (`bool` versus `Option<u64>`); afterward it
compiled. The 6-test independent receive-timeout trace passed in debug and
optimized builds. It now binds each admission before asserting, checks that a
stale notification leaves token 1 live, and checks that an accepted one returns
that same token. The model still uses two optional tokens and the same two
transition branches; it adds no public type, state, module, or effect lane.
Measured testkit model diff is +4/-5 (net -1) lines and its caller test is
+7/-3 (net +4). The retained current values remain the last issued token and
the one live token. The residue scan found no arrival-history alternative,
repeated cause, false cardinality, nested authority, semantic boolean, or
positional syntax in this path. Cross-check against the receive-timeout
Rustdoc and the actor transition effect law passed. Disposition: `pass` for
this batch; A15 awaits the full Nix gate on the signed revision.

A whole-crate assertion scan found additional test and fuzz assertions that
called state-changing methods inside the macro: actor `transition`, model
`initialize`/`activity`, collection `insert`/`remove`, mailbox `receive`,
sequence `issue`/`reserve`, and iterator `next`. The test-execution law is that
setup and transitions run before observation; assertions inspect retained
results and complete actions only. This is test discipline, not a new actor
transition. The calls were moved to named locals in their original order,
including four observational iterator calls, across 34 Rust files (+304/-258
lines, net +46). All changes are in test or fuzz code, including inline test
modules; no production type, state, effect lane, or branch changed. A repeated
whole-crate scan for these methods inside assertion macros found zero sites.
The Nix-pinned workspace Nextest run after this batch passed 819/819 tests,
including the macro dependency-resolution fixtures and the affected actor,
core, and testkit suites. The changed fuzz targets and clean-snapshot flake
gate remain to be checked.

The next A15 scan found five generated `bool` selectors in the independent
testkit models. Each selected a domain event or exact/foreign source; the
tuple generators also supplied values unused by some event alternatives.
This is a test-model policy issue, not a new actor law. Before editing, the
static scan `rg 'any::<bool>\(\)' crates --glob '*.rs'` found exactly these
five sites. The expected regression is that the same scan finds none after
the edit, while the three affected model targets still compare every generated
turn with their independent oracle in debug and optimized builds. The direct
test data model is a separate closed sum for buffer, priority, rate, lease,
and observation operations, with each variant owning only the data it uses.
No production state, effect product, interpreter path, or public spelling is
changed. The five-site static regression now finds no `any::<bool>()` under
`crates/`. The three edited test binaries passed 7/7 cases in both debug and
release mode with the Nix-pinned toolchain. Their measured test-only diff is
+219/-137 (net +82) lines; production is +0/-0, with no public API change.
Production control states, subordinate alternatives, transition branches, and
modules are unchanged. The independent models retain their prior observable
decisions. Five boolean event/source selectors and two numeric operation tags
became seven closed input sums with 18 alternatives. Each alternative owns its
current event data, such as an offered value, acquired cost, exact report
target, or elapsed timer source. The three test modules remain three. The
residue scan found no retained arrival history, repeated cause, false
cardinality, nested transition authority, semantic selector boolean, or
unused field on these generated operations. The actor transition and relevant
routing, timing, and observation laws were cross-checked. Disposition:
`pass` for this test-model batch; the complete A15 gate remains open.

A wider assertion scan then found mutating helper calls hidden behind local
names such as `query`, `put`, `acquire`, `release`, `hold`, and `offer`, plus
interpreter methods called inside assertions. The earlier method-name scan did
not catch these. Hoist those transitions and ownership transfers before
assertions, run the affected tests, and repeat the broader scan before A15
can close.

The follow-up batch hoisted local actor transitions, interpreter calls,
initialization, creation decomposition, source admission, and consuming
iteration before their assertions. It also replaced 91 assertion-side
`unattempted().into_inputs().len()/is_empty()`,
`into_requests().len()/is_empty()`, and
`into_deliveries().is_empty()` chains with existing borrowed `len`,
`is_empty`, or `as_slice().is_empty` observations. The direct forms inspect
the same ordered product without transferring custody merely to count it.
Across 25 Rust files the test-only diff is +206/-663 (net -457) lines;
production is +0/-0 and public API, actor control states, effect lanes,
transition branches, and module count are unchanged. All source-file edits
are inside `#[cfg(test)]`; the remaining files are tests or fuzz targets.
The surviving current values are the original actor state and exact owned
effect products; no arrival history, repeated cause, false cardinality,
nested authority, semantic boolean, or positional consumer path was added.
The actor transition and source-custody laws were cross-checked. The broader
assertion scan now finds no selected mutating or consuming method inside an
assertion; its ten remaining `&mut` matches compare a returned source with a
temporary source and do not invoke a transition. Workspace test compilation
and formatting passed. Local Nextest discovery and a direct actor test binary
then stalled in macOS `_dyld_start` before the Rust harness ran, so that run
is not test evidence. `scripts/check_assertion_effects.py` now enforces the
identified consuming-method and mutating-helper patterns in the Nix document
gate; five checker regression cases cover detection and allowed observation.
The local checker and its tests passed. A clean-snapshot Nix gate and changed
fuzz-target build remain required before this batch or A15 can close.

The clean detached snapshot at signed commit `9c9973e` passed
`nix flake check --max-jobs 1 --no-write-lock-file`, including package,
Nextest, Clippy, Rustdoc, doctests, formatting, deny, and the document gate
with both assertion-checker scripts in the flake source set. The Nix-provided
fuzz runner then built all manifest targets after the assertion edits with
`nix run .#fuzz -- build`; its separate lockfile was synchronized to the
workspace crate versions. These runs complete the pending A15 verification.
The later A20 priority-property edit has its own focused baseline and
counterfactual evidence and does not change the A15 assertion discipline.
Disposition: `pass`; A15 is closed.

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

### A07 actor mutation evidence: lease renewal authority

Classification: derived generation-correlation law. A held lease may renew only
when both the holder and observed generation match its current state. A stale
generation or different holder returns a typed rejection without scheduling a
new expiry. The independent model in `timing_invariants` compares the outcome,
scheduled generation, and state after each generated operation.

At signed revision `a334b0b`, a Nix-toolchain campaign selected all eight
mutations of `Lease::successor` with `cargo mutants --package
bombay-behavior-actors --test-package bombay-behavior-actors --test-package
bombay-behavior-testkit --test-tool nextest --no-shuffle
--minimum-test-timeout 180 -f crates/actors/src/time/lease.rs -F successor`.
The unmutated actor baseline passed 593 tests; a separate unmutated testkit
run passed 112 tests, including the independent lease model. Mutated commands
selected both packages (705 tests). Seven candidates compiled and failed
named tests; the eighth was unviable because `TimerGeneration` has no
`Default`. There were no survivors or timeouts. The strict mutation gate
accepted the complete report with a viability floor of seven
(`7 viable / 8 total`), keeping compiler rejection distinct from test
detection. The false stale-generation guard was caught by the independent
model; the other six viable changes were caught by lease unit tests. This is
one lease law slice, not an actor-wide verdict.

### A07 actor mutation evidence: termination observation and propagation

Classification: derived exact-correlation and terminal-custody laws. A
termination monitor consumes only the selected peer or observation report in
its lawful phase. A propagation target accepts one matching terminal report;
foreign or later reports return their complete value without changing state.

At signed revision `812fe86`, the Nix-toolchain monitor campaign selected all
ten `TerminationObservationTarget::react` candidates in
`lifecycle/termination_monitor.rs`. Its actor baseline passed 593 tests, and
mutated commands selected actor and testkit suites (705 tests). Eight viable
mutations failed named actor or independent-model tests; two whole-function
replacements could not compile because `Actions` has no `Default`. No survivor
or timeout occurred, and the strict gate accepted `8 viable / 10 total`.
Both lifecycle campaigns used `cargo mutants --package
bombay-behavior-actors --test-package bombay-behavior-actors --test-package
bombay-behavior-testkit --test-tool nextest --no-shuffle
--minimum-test-timeout 180`; reproduce the monitor selection with
`-f crates/actors/src/lifecycle/termination_monitor.rs -F '::react'`.

The matching propagation campaign selected nine correlation mutations in
`lifecycle/termination_propagation.rs`. Eight were caught, but replacing
`PeerTermination::matches` with unconditional acceptance survived: the peer
test exercised only a matching report. The strict gate correctly rejected
that original report. Signed commit `f0dce34` added a foreign-peer report
that must return intact while observation remains active, followed by an
accepted selected-peer report; it also made the independent child-sequence
model inspect initialization and every successful effect lane. Both focused
tests passed. A separate one-mutant rerun failed the new peer test, with no
timeout, and the strict gate accepted its complete `1 viable / 1 total`
report. The original eight results and the focused rerun are separate
revision-specific evidence, not a claim that all actor mutations were run.
The propagation selection is reproducible with
`-f crates/actors/src/lifecycle/termination_propagation.rs
-F '::matches|match guard self.state|replace && with'`.

### A07 actor mutation evidence: work admission and availability

Classification: deliberate Bombay FIFO and bounded-admission policy. A
submission consumes the oldest available worker or joins the bounded waiting
queue; at capacity its complete value is returned. An availability notice
dispatches the oldest waiting value or joins the unique worker queue.

At signed revision `69adab4`, the Nix-toolchain campaign selected four
`WorkQueue::submit` and `WorkQueue::announce` candidates in
`routing/work_queue.rs`. The actor baseline passed, and mutated commands ran
the actor and testkit suites. Both whole-function default replacements were
unviable because the complete `Actions` product has no `Default`. The two
viable admission and duplicate-availability guard changes compiled and failed
named work-queue unit tests. The strict gate accepted `2 viable / 4 total`,
with no survivor or timeout. Reproduce with `cargo mutants --package
bombay-behavior-actors --test-package bombay-behavior-actors --test-package
bombay-behavior-testkit --test-tool nextest --no-shuffle
--minimum-test-timeout 180 -f crates/actors/src/routing/work_queue.rs
-F '::submit|::announce'`. The independent two-FIFO property was subsequently
strengthened to inspect all action lanes with non-Clone work values and unique
reply destinations; this newer oracle passed, but was not part of the earlier
mutant verdict.

### A07 actor mutation evidence: keyed publication membership

Classification: deliberate Bombay keyed-membership and ordered-publication
policy. A topic is retained after its last subscriber leaves; a repeated
subscription is idempotent. Unsubscription of an unknown topic or absent
recipient returns the complete command without changing membership.

At signed revision `d7d0832`, a Nix-toolchain campaign selected all six
`PubSub::subscribe` and `PubSub::unsubscribe` candidates in
`discovery/pub_sub.rs`. The actor baseline passed, every candidate compiled,
and named pub-sub unit tests failed under each mutation. The strict gate
accepted `6 viable / 6 total`, with no survivor or timeout. Reproduce with
`cargo mutants --package bombay-behavior-actors --test-package
bombay-behavior-actors --test-package bombay-behavior-testkit --test-tool
nextest --no-shuffle --minimum-test-timeout 180
-f crates/actors/src/discovery/pub_sub.rs
-F 'PubSub<A, K, P, Route>::subscribe|PubSub<A, K, P, Route>::unsubscribe'`.
A later independent sequence model checks topic order, exact membership,
every successful action lane, and rejected publication custody with distinct
owned strings. It passed after the campaign and is not credited with the
earlier mutant kills. Publication-loop and downstream delivery laws remain
outside this mutation slice. Aggregate-drift checkpoint for the test-only
batch: control states `1 → 1` (active), named subordinate state sums `0 → 0`,
aggregate error variants `3 → 3`, message arms `3 → 3`, production lines
`234 → 234`, modules `1 → 1`, and public spellings unchanged. The current topic key,
retained membership list, and introduction order remain the only future-needed
values. The test's map and order list model the observable order; no production
arrival history, repeated cause, false cardinality, nested authority,
semantic boolean, or structural user syntax was added. Cross-checks were
`actor-transition-algebra.md`, `atomic-runtime-settlement.md`, and the PubSub
row in `engineering/atomic-actor-other-templates.md`. Disposition: `pass` for
this test-only evidence batch; capacity, retirement, and delivery settlement
remain independent future laws in that normalized catalogue record.

A second Nix-toolchain campaign at signed revision `888e78a` selected the
five `PubSub::transition` candidates. Four whole-function replacements were
unviable because they attempted invalid `BehaviorActed` construction; the
topic-equality inversion compiled and failed the existing publication unit
test. The strict gate accepted `1 viable / 5 total`, with no survivor or
timeout. Since that runner stopped after the unit failure, a separate
isolated-worktree counterfactual changed only publication's topic comparison
from `==` to `!=` and ran the new independent property alone. It failed and
shrunk to two commands: subscribe to topic 0, then publish to topic 0; the
actor incorrectly returned `NoSubscribers`. The temporary edit and worktree
were removed. This proves the independent oracle's topic-selection
sensitivity, not the publication clone-loop or transport settlement law.

### A07 actor mutation evidence: health observation versions

Classification: deliberate Bombay component-correlation and version-commit policy. A
health observation updates only its selected component. Older evidence returns
a stale error with its owned evidence; equal-version conflicting evidence
returns a conflict; equal-version identical evidence is idempotent. A removal
tombstone cannot be undone by an older observation.

The Nix-toolchain campaign selected all seven candidates in
`Health<A, K, Route>::commit` at revision `ab0d65f`. The actor baseline and
mutated actor and testkit suites passed or failed as expected. Every candidate
compiled, then a named health test failed; the relevant tests were
`stale_and_conflicting_evidence_preserve_committed_state` and
`tombstone_rejects_resurrection_and_report_aggregates_worst_status`. The strict
gate accepted `7 viable / 7 total`, with no survivor or timeout. Reproduce
the selection with `cargo mutants --package bombay-behavior-actors
--test-package bombay-behavior-actors --test-package bombay-behavior-testkit
--test-tool nextest --no-shuffle --minimum-test-timeout 180
-f crates/actors/src/operations/health.rs
-F 'Health<A, K, Route>::commit'`. This is one operation-law slice, not an
actor-wide mutation verdict.

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

### A20 pre-edit acknowledgement model law

Classification: deliberate Bombay participant-correlation policy. `Begin`
normalizes declaration order; each declared participant can acknowledge once;
the final acknowledgement completes; cancellation succeeds only while pending.
Unknown, duplicate, unexpected, completed, and cancelled operations each
return the exact rejected command or participant through the one reply lane,
without changing the retained lifecycle. Terminal records remain distinct.
The existing property checks begin/acknowledge in two separate batches and
never emits `Cancel`, so it cannot prove the cancellation or interleaving law.
The test-only replacement will generate mixed operations and varied reply
recipients, compare every ordered reply and complete outcome, the full record
order and current participant state, empty creations, and continuing verdict
after every step. Its independent pending model retains original declaration
and accepted participants, then derives remaining participants; terminal
states discard those lists. It does not perform the actor's in-place removal.
The existing concrete actor and `Actions` products remain the lower-order
contracts. No production symbol,
state, effect, runtime port, wrapper, or public spelling changes. Pre-edit
control phases Pending/Completed/Cancelled, subordinate error/outcome
alternatives, transition branches, production lines/modules, and public
spellings stay fixed. Every retained model value is needed for a future
acceptance, duplicate, or exact-order decision. No arrival-history residue is
added to production; the model's accepted order is observable and therefore
lawful. No repeated cause, false cardinality, nested authority, semantic
boolean, or positional caller syntax is proposed. Cross-check: actor
transition algebra and the routing correlation catalogue. Disposition:
`pass` for the test-only model before implementation.

The focused correlation suite passed all three cases in the Nix toolchain.
The previous no-op reply-actor macro was deleted; both model routes use the
existing concrete `MessageProtocol`. The new generated trace mixes Begin,
Acknowledge, and Cancel, and a deterministic trace reaches unknown,
unexpected, duplicate, completed, and cancelled replies. Every turn checks
the exact reply recipient/outcome, all current records and order, empty
creations, and `Continue`. In a disposable Behavior worktree, a one-line
counterfactual changed only a repeat-cancellation reply from `Cancelled` to
`Completed`. Both tests failed for that exact outcome; the generated trace
shrunk to Begin, Cancel, Cancel on one key. The mutation and worktree were
removed. This proves the new oracle detects wrong terminal rejection
classification, not every acknowledgement defect. Test source changed
`+298/-119/net +179` physical lines; production `+0/-0`, public API
`+0/-0`, control states, subordinate alternatives, transition branches, and
modules are unchanged. The surviving model alternatives own exactly their
future-needed current values; no arrival-history, repeated-cause, false
cardinality, nested-authority, semantic-boolean, or positional-syntax residue
remains. The actor transition and routing correlation contracts were
cross-checked. Disposition: `pass` for this test-only evidence batch; A20
remains open. The pure trace does not prove host delivery admission or a
wrapper-order interaction, and its isolated counterfactual covers one
terminal branch only.

### A20 pre-edit correlation reply custody

Classification: deliberate Bombay keyed-correlation policy. `Begin` retains
one exact reply recipient while pending. A matching Resolve or Cancel emits
one terminal result to that recipient, then removes its authority; a later
reply is rejected with complete key/value ownership. The current generated
property uses the same reply route for every Begin and checks only the first
send's payload on successful terminal transitions. It cannot detect a wrong
destination, duplicate send, creation, or stop verdict. The test-only
candidate varies reply recipients, retains their address in the independent
model only while pending, and compares all successful action lanes and the
exact retained state after every operation. Existing `CorrelatorError` cases
already check the owned rejected values. No production state, transition,
effect, interpreter operation, wrapper, public spelling, module, or line
changes are proposed. Control phases Pending/Completed/Cancelled and every
subordinate alternative remain fixed; the pending reply address is the exact
future-needed current value. The residue scan finds no proposed arrival
history, repeated cause, false cardinality, nested authority, semantic
boolean, or positional consumer syntax. Cross-check: actor transition
algebra and routing correlation law. Disposition: `pass` for the test-only
model before implementation.

The focused three-case correlation suite passes after the property varies
reply addresses for each Begin. Successful Begin asserts empty sends and
creations plus Continue; matching Resolve and Cancel each assert exactly one
send to the retained recipient, the complete result, empty creations, and
Continue. The record comparison now checks the retained pending recipient
alongside its key and phase. Every existing rejection still checks its exact
returned key, value, or submitted reply route. Test source changed
`+26/-10/net +16` physical lines (`393 → 409`); production `+0/-0`, public
types, control states, subordinate alternatives, transition branches, and
modules are unchanged. The pending recipient is discarded from the model
upon terminal settlement, matching the current-value law. No history,
repeated cause, false cardinality, nested authority, semantic boolean, or
positional consumer syntax was added. The actor transition and correlation
laws were cross-checked. Disposition: `pass` for this test-only batch; a
dedicated route mutation and real interpreter delivery remain outside it.

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

### A20 ledger entry: lease holder and generation correlation

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition | `time::lease::tests::acquire_renew_release_and_stale_elapsed_are_generation_safe` and `wrong_holder_and_matching_expiry_are_distinct` check renewal, wrong-holder rejection, stale expiry, release, and continued state. |
| Independent trace | `timing_invariants::lease_matches_exclusive_generation_ownership_after_every_event` compares each generated outcome, scheduled generation, and held/vacant state against its own model; it caught the false stale-generation guard. |
| Composition | `recursive_reply_protocols` and exact-reply template tests prove typed reply routes; this mutation slice does not establish a separate wrapper-order law for `Lease`. |
| Invalid construction and boundaries | Holder and generation are concrete typed inputs. `generation_exhaustion_is_terminal_and_never_wraps` checks the upper sequence boundary; the model's generated sequence stays below it. |
| Counterfactual | Seven viable `successor` mutations were caught; one replacement could not compile because `TimerGeneration` has no `Default`. Other lease transition branches remain outside this campaign. |

### A20 ledger entry: termination observation and propagation

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | Logical-monitor unit tests check matching reaction actions, duplicate rejection, exact foreign-report return, and continued user delegation. Established-monitor integration tests check requested, observing, cancelled, and observed phases with exact observation IDs; the independent model also covers rejection. The propagation peer regression checks foreign return before selected-peer publication and stop. |
| Independent trace | `exact_termination_model` compares generated exact-report phases and reaction count but does not inspect every action lane. `terminal_outcome_sequences` independently predicts selected-child acceptance, foreign return, discharge, publication, and later rejection; since `f0dce34` it also checks initialization and every effect lane on successful steps. It does not model `PeerTermination`. |
| Composition and invalid construction | The exact monitor composes inside `StopOnShutdown`, and the logical monitor appears in the universal-layer tests. No two-order wrapper proof is claimed for propagation. A child target requires a typed creation ID and protocol occurrence; a foreign report is a runtime input returned through the typed error. |
| Counterfactual | Eight viable monitor mutations were caught and two were unviable. The propagation campaign exposed an unconditional-peer-acceptance survivor; its isolated post-regression rerun was caught by the new focused test. The two campaign reports retain their separate baselines and verdicts. |

### A20 ledger entry: work admission and availability

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | `routing::work_queue` unit tests cover FIFO worker selection, queued dispatch, and zero-capacity rejection. The newer independent property also checks every emitted assignment and outcome, recipient, empty creation lane, and continuing verdict. |
| Independent trace | `routing_invariants::work_queue_matches_two_coupled_fifo_capabilities` tracks waiting work and available workers in separate deques, with unique reply recipients and a non-Clone work payload. It checks the complete observable state after every generated operation. It does not claim transport admission. |
| Composition and boundaries | Exact reply-route template tests cover logical and established customer routes. The property generates capacities including zero and repeated worker notices and withdrawals; it does not check downstream worker execution. |
| Counterfactual | Two viable guard mutants failed actor unit tests; two whole-function replacements were unviable. The strengthened independent property passed after this campaign, so the original verdict is not attributed to it. |

### A20 ledger entry: keyed publication membership

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | `discovery::pub_sub` unit tests check first subscription order, duplicate suppression, and publication rejection for known empty and unknown topics. The independent property checks the exact returned topic, recipient, and original publication allocation. |
| Independent trace | `catalogue_invariants::pub_sub_preserves_topic_membership_and_rejected_publications` tracks membership in a map plus introduction order, then compares every current topic, recipient order, successful action lane, and rejection after generated commands. Each publication has a distinct owned string. It does not interpret transport admission. |
| Composition and boundaries | Exact reply template tests exercise established publication routes. The property explores absent topics, empty retained topics, duplicate recipients, and re-subscription; it does not prove scheduling or downstream delivery. Topic/member capacity and topic retirement still need laws as recorded in `atomic-actor-other-templates.md`. |
| Counterfactual | All six selected membership mutants compiled and failed named unit tests. The separate topic-selection campaign caught one viable equality inversion and had four unviable replacements. An isolated rerun of that inversion against only the independent property failed with a two-command counterexample. Publication clone-loop and transport settlement evidence remain open. |

### A20 ledger entry: health observation versions

| Evidence layer | Current witness and limit |
|---|---|
| Focused transition and custody | The two `operations::health` tests check component selection, stale and equal-version conflicts, idempotence, tombstone retention, and report aggregation. Error variants retain the rejected component and evidence. |
| Independent trace | `behavior-testkit/tests/catalogue_invariants.rs::health_tombstones_and_versions_match_an_independent_map` compares generated health operations and complete rejected evidence with an independent ordered component map. It does not interpret host delivery admission. |
| Composition and boundaries | The health actor returns its report through a typed route. Version equality and ordering are explicit in the pure transition; the focused test includes a removed component and a later older observation. No wrapper-order claim is made for this standalone actor. |
| Counterfactual | All seven selected `Health::commit` mutations compiled and failed named health tests. The strict gate accepted `7 viable / 7 total`; other health methods remain outside this slice. |

### A20 ledger entries: ordered routing and workflow

The next A20 test-only experiment targets the circuit breaker's deliberate
single-flight and reset policy. A model built from the documented public
contract owns a free slot with consecutive failures, one accepted attempt,
a cooling generation, or a single trial. It predicts every reply recipient
and outcome, timer request, next phase, empty creation lane, and continuation
after each generated command. Successful admission must retain the same
attempt until its matching completion; unrelated completions return the exact
command; stale timer evidence changes nothing. The existing focused breaker
tests and typed `TimerElapsed` input are lower-order witnesses. No production
type, state, transition, or interpreter port is proposed. The pre-edit
aggregate control sum is Closed/Idle, Closed/Awaiting, Open,
Probing/Available, Probing/Awaiting, and Exhausted; the test adds no alternative
to it. The model's phase is an independent expected-value oracle, not another
production transition authority. The source has one aggregate module and its
existing branches remain unchanged. A mutation of the reset-generation guard
must fail this oracle before the test is retained. Cross-checks are the actor
transition algebra and normalized routing catalogue law. Disposition:
`pass` for the test design, pending the counterfactual and measurements.

The retained `circuit_breaker_model` compares generated command sequences
with an independently named single-flight/cooling/trial oracle, plus a
deterministic open/reopen trace. Each successful step compares ordered reply
recipients and complete outcomes, exact reset requests, phase and owned
attempt, empty creation lane, and continuing verdict; each invalid completion
checks its exact returned command. The generated instructions include current
and foreign completions, matching/stale timer generations, and a foreign
timer ID. Attempt-number and generation exhaustion remain in the focused
unit tests, since generated traces cannot approach `u64::MAX`. This is a pure
transition proof; it does not establish timer scheduling or delivery by a
real runtime. Both tests passed in debug and optimized Nix builds.

The isolated one-line counterfactual inverted the timer-generation equality
guard. Both tests failed; the generated oracle shrank to `Admit, Fail, Admit,
Fail, ElapsedCurrent`, where the actual phase remained open instead of
offering one trial. The temporary mutation and worktree were removed. This
proves the model detects incorrect matching-reset admission, not every
possible circuit-breaker defect. The test-only batch changes production
`+0/-0/net 0`, tests `+419/-0/net +419`, public API `+0/-0` types,
production lines `404 → 404`, production modules `1 → 1`, and production
transition branches unchanged. The control sum remains Closed/Idle,
Closed/Awaiting, Open, Probing/Available, Probing/Awaiting, Exhausted; its
owned values and future decisions remain those stated above. The residue
scan found no production arrival history, repeated cause, false cardinality,
nested authority, semantic boolean, or structural caller syntax. The
transition algebra and routing catalogue record were cross-checked.
Disposition: `pass` for this test-only evidence batch; A20 remains open. The
complete `nix flake check -L` passed all ten declared checks at signed commit
`ad4d198`, including optimized Nextest with 826 passing cases, release tests,
doctests, Clippy, package, documentation, formatting, dependency-audit, and
dependency-policy gates.

| Law | Focused transition and independent trace | Remaining proof boundary |
|---|---|---|
| Sequencer gap release | `routing::sequencer` tests missing, stale, duplicate, and maximum positions. `catalogue_models::sequencer_matches_an_independent_gap_map_after_every_offer` compares deliveries, outcomes, state, and empty creation lane after each generated offer. | Generated positions stay below exhaustion. The maximum-position unit test covers that separate boundary; no sequencer mutation slice is recorded. |
| Deduplicator retention | `routing::deduplicator` tests duplicate custody, eviction, and zero capacity. `catalogue_models::deduplicator_matches_an_independent_fifo_window_after_every_delivery` compares both send lanes and retained keys, and checks that the exact boxed value allocation travels through admission or rejection. | The model varies positive capacities; zero capacity is a constructor rejection in the focused test. Transport admission and a dedicated mutation slice remain unproved. |
| Order-gate watermark | `catalogue_models::order_gate_matches_an_independent_watermark_map_after_every_operation` compares ordered releases, duplicate and stale-open outcomes, watermark, held count, and empty creation lane after each generated operation. | The generated trace does not prove a runtime delivery receipt or a dedicated guard mutation slice. |
| Priority selection | `routing::priority_queue` tests stable priority/FIFO ties and full/empty outcomes. `routing_invariants::priority_queue_matches_stable_max_priority_selection` compares an independent ordered list after each offer or release, including exact delivery and reply recipients, reply depth, empty creation lane, and continuing verdict. An isolated `Released.remaining = queued.len() + 1` counterfactual compiled and failed at the new remaining-depth assertion on a one-offer, one-release trace. | The generated priorities cover 0–7 and positive capacities below eight; exhaustion and real host admission are outside this property. The recorded counterfactual covers the release-depth branch, not every routing branch. |
| Rate admission | `routing::rate_limiter` tests accepted/rejected ownership and saturating refill. `routing_invariants::rate_limiter_matches_saturating_token_arithmetic` compares capacity, available tokens, rejection reasons and returned value, and admitted delivery after each generated operation. | Its generated path constructs positive token costs and capacities; no host admission or dedicated mutation slice is recorded. |
| Round-robin membership cursor | `routing::router` tests cursor repair after removal. `routing_invariants::round_robin_keeps_the_same_next_recipient_across_membership_edits` tracks an independent member list and next recipient through generated edits and routes. | The pure trace does not prove transport admission. |
| Consistent-hash membership stability | `routing::router` tests one three-member removal over 128 keys. `routing_invariants::consistent_hash_membership_edits_preserve_unaffected_key_owners` checks complete actions, Unknown admission, varying tokens and keys, aligned evidence, addition, and removal in 128 generated cases. An isolated no-removal mutant failed at the exact surviving member token, shrinking to tokens `[0,1,2,3]`. | The relational property does not independently calculate every ring point or prove host delivery admission; the focused actor test covers same-version conflict but stale token evidence still needs a direct witness. |
| Least-loaded evidence | `routing::router` tests unknown, stale, conflicting, tied, and newly lower evidence. `routing_invariants::least_loaded_matches_versioned_membership_and_selection` models member order and latest evidence after mixed add, remove, observe, and route operations; a deterministic trace covers re-addition. | An isolated max-load selection counterfactual failed both new tests; pure routing still does not prove host delivery admission or the other policy families. |
| Rendezvous member stability | `routing::router` tests one keyed route, conflicting token, and exact stale-version return. `routing_invariants::rendezvous_membership_edits_only_move_keys_to_or_from_the_changed_member` checks exact route actions, token evidence, membership edits, and order-independent ownership for 128 generated distinct-token cases. An isolated index-dependent score counterfactual failed at the permutation assertion with a minimal `[0,1,2,3]` token set. | This relational law does not independently calculate each score or prove host delivery and every tie case. |
| Latch release | `workflow::latch` tests threshold order and zero-count startup. `workflow_invariants::latch_releases_each_accepted_route_exactly_once` compares an independent waiting list, release phase, and exact recipient order through generated arrivals. | The property does not interpret delivery admission or record a dedicated mutation slice. |
| Dependency workflow | `workflow_invariants::workflow_matches_an_independent_dependency_run` tracks step and run phases across start, completion, failure, and cancellation, including invalid early completion and failure. | The focused property and workflow unit tests do not provide a real interpreter trace or dedicated mutation slice for every branch. |

### A20 pre-edit consistent-hash membership law

Classification: deliberate Bombay ring-routing policy. With a fixed key hash,
stable member tokens, and a positive replica count, adding an Unknown member
changes no existing assignment; accepting its token can move a key only to
that member. Removing a member can change only keys it owned. These are
relational consequences of the stated clockwise ring selection law and do
not require a test to duplicate the point-mixing function. A generated test
will vary four distinct tokens and extra keys, compare complete route and
observation `Actions`, member order and token evidence, then check the two
before/after ownership relations. The focused three-member removal test and
the typed `MemberTokenObservation` are the lower-order witnesses. An isolated
mutation that leaves removed token evidence in the policy or changes the
selected eligible point must fail the generated law before retention.

The Router's one ordered membership control state, Unknown/Observed token
evidence, three hash rejections, transition branches, modules, production
lines, and public spellings remain unchanged. Current member order and each
latest token/version are the only values needed for later selection and
stale/conflict decisions. The proposed test adds no actor state, arrival
history, repeated cause, false cardinality, nested transition authority,
semantic boolean, or structural caller syntax. Cross-checks are the actor
transition algebra and normalized routing catalogue law. Disposition: `pass`
for the test model, pending the generated trace and counterfactual.

Post-edit, all ten routing invariant tests pass in debug and optimized Nix
builds. The new consistent-hash property checks every successful action lane
and continuation, exact keyed deliveries, member order and token evidence,
then the before/after key-owner relations. A shared typed route assertion now
serves the ring and rendezvous properties, deleting their duplicate action
check without adding a production bound. In an isolated Behavior worktree
with its own Cargo target, deleting only `HashMembership::removed` made the
new property fail at the exact survivor-token assertion; proptest shrank to
`tokens = [0,1,2,3]` with no extra keys. The mutant and worktree were removed.
This counterfactual proves detection of missing token-position repair, not
the ring mixer or runtime admission.

The test-only batch changes production `+0/-0/net 0`, tests
`+98/-16/net +82`, and public API `+0/-0` types; the routing invariant file
is 790 to 872 physical lines. Router control, hash evidence alternatives,
rejections, branches, production modules, and all future-needed member and
token values remain unchanged. The residue scan and law-document cross-check
above found no new production history, repeated cause, false cardinality,
nested authority, semantic boolean, or positional caller syntax. Disposition:
`pass` for this evidence batch; A20 remains open for other laws.

### A20 pre-edit hash-token version law

Classification: deliberate Bombay evidence policy, shared by consistent and
rendezvous selection. Once a member has Observed(version, token), an older
observation must return its exact owned value with `Stale` and leave the
committed evidence unchanged. The same version/token is idempotent, the same
version with a different token is a conflict, and a newer version replaces the
current token. The existing rendezvous example covers conflict only. A focused
pure Router test will prove stale ownership, idempotence, newer acceptance,
all action lanes, and evidence state. An isolated inversion of the version
ordering guard must fail the stale or newer assertion before retention.

No production type, branch, module, public spelling, or state alternative is
proposed. Router's one ordered membership list and each current token/version
remain exactly the values used by later selection and rejection. The test
adds no arrival-history state, repeated cause, false cardinality, nested
authority, semantic boolean, or structural caller syntax. Cross-checks are
the actor transition algebra and normalized routing catalogue law.
Disposition: `pass` for the test model, pending the focused witness and
counterfactual.

Post-edit, the focused router test passed in debug and optimized Nix builds.
It checks the exact stale observation in both returned policy fields, no
evidence mutation, idempotent same-version replay, a newer accepted token,
and empty send/create lanes with continuation. The existing conflicting-token
test now also checks the exact returned observation in both fields. In a
separate Behavior worktree and Cargo target, inverting only the hash evidence
version guard made the new test fail at the stale-return assertion. The
mutant and worktree were removed. This counterfactual tests ordering of
version admission; it does not prove the hash score or host delivery.

The actor source test section changes `+84/-14/net +70` physical lines;
production Rust, public types, control states, evidence alternatives,
transition branches, and modules stay unchanged. The current member order
and latest version/token remain the only future-needed values. No arrival
history, repeated cause, false cardinality, nested authority, semantic
boolean, or structural caller syntax was introduced. The actor transition
and normalized routing laws were cross-checked. Disposition: `pass` for this
focused evidence batch; A20 remains open elsewhere.

### A20 pre-edit least-loaded evidence law

Classification: deliberate Bombay routing policy. A member is ineligible
until it has versioned load evidence; the least load wins and declaration
order breaks ties. Unknown, stale, and same-version conflicting observations
return the exact evidence without changing membership or load. Removal
retires a member's evidence, so re-addition begins unknown. A source-only
unit test checks selected examples but no generated trace combines all of
these operations. The test-only candidate uses an ordered list of members
with optional latest `(version, load)` evidence and independently scans for
the first minimum, rather than calling the strategy's selection logic.
Mixed generated and deterministic traces will compare complete delivery or
rejection, every action lane, member order, and evidence after every step.
Existing `Router`/`LeastLoaded` types and actor unit tests are the
lower-order contracts. No production type, trait, state, effect, wrapper,
interpreter port, or public spelling changes. Baseline actor control remains
one router with an ordered membership list; subordinate evidence is Unknown
or Observed. Its latest version/load pair and membership order are the exact
future-needed values. Production branches, lines, and modules remain fixed.
The residue scan finds no proposed arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
consumer syntax. Cross-check: actor transition algebra and normalized
routing catalogue law. Disposition: `pass` for this test-only model before
implementation.

The retained independent model uses an ordered member list and each member's
optional latest version/load reading. It predicts the first minimum, exact
reply or returned value, all effect lanes, and membership and reading state
after every operation. The deterministic trace covers tied loads, stale and
conflicting observations, removal, and re-addition; 384 generated traces mix
those operations. The seven focused routing invariant tests passed after a
clean Nix development build; both new tests also passed in the optimized Nix
build. An isolated one-line counterfactual changed the
selection from minimum to maximum. Both new tests failed for the intended
recipient-selection law, and the generated trace shrank to two observed
members followed by one route. The counterfactual worktree was removed.

The isolated worktree temporarily shared the main checkout's Cargo target
directory. A later main-checkout test linked its mutated artifact and failed;
that failure is excluded from the baseline evidence. Cleaning both affected
packages and rebuilding the unchanged main actor source restored the seven
passing tests. Future worktree counterfactuals need separate Cargo targets.
This test-only batch changes production `+0/-0/net 0`, tests
`+235/-1/net +234`, and public API `+0/-0` types; the test file is
`414 → 648` lines. Production `router.rs` stays at 1,311 physical lines and
its modules, branches, and public spellings are unchanged. Router control
remains an ordered membership list with each member Unknown or Observed;
the optional latest reading and order are precisely the future-needed values.
The test model adds no production state or nested transition authority. The
residue scan found no arrival history, repeated cause, false cardinality,
semantic boolean, or structural caller syntax. The actor transition algebra
and normalized routing catalogue law were cross-checked. Disposition: `pass`
for this evidence batch; A20 remains open for the rest of the ledger.

### A20 ledger entries: catalogue versioning and membership

Next A20 routing candidate, before test changes: the Rendezvous policy's
stable member token is an explicit versioned fact, and adding an eligible
member can only keep an existing key assignment or move that key to the new
member. Removing a nonselected member cannot change an assignment. With
distinct tokens, reversing declaration order must preserve each key's owner
because the score depends on key and token, with no tie to break. This is a
deliberate Bombay highest-score policy, not an actor-model requirement. The
existing `rendezvous_hash_is_deterministic_and_rejects_conflicting_tokens`
unit test checks only one key before a conflicting observation; it does not
exercise membership edits across a key set. A generated relational trace
will compare the complete route `Actions`, member order, token evidence,
permutation ownership, and the before/after owner of each key through add and
removal without copying
the hash mixer. The existing router, `MemberTokenObservation`, and
`RoutingStrategy` are the lower-order contracts. No production state, type,
effect lane, public spelling, interpreter port, or policy changes. Router
control remains an ordered member list; the hash policy's only subordinate
alternatives are Unknown and Observed(version, token). Order and latest token
are precisely the future-needed values. Production branches, lines, and
modules remain unchanged. The proposed property introduces no arrival
history, repeated cause, false cardinality, nested authority, semantic
boolean, or positional caller syntax. Cross-check: actor transition algebra
and normalized routing catalogue law. Disposition: `pass` for the test
design, pending the counterfactual and measurements.

The retained test compares key owners before and after adding an unknown
member, accepting its token, reversing the fully observed membership order,
and removing one member. Every route checks the exact keyed payload and
recipient, empty creation lane, and continuation; membership operations check
all lanes and the policy's current evidence. The focused property passed in
debug and optimized Nix builds. A disposable worktree with a separate Cargo
target changed the Rendezvous score to depend on the member index. The
property failed at the order-independence assertion and shrank to distinct
tokens `[0, 1, 2, 3]` with no extra keys. The worktree and its generated seed
were removed. This proves the property rejects index-dependent selection;
the relation alone does not certify the exact mixer or every token branch.

This test-only batch changes production `+0/-0/net 0`, tests
`+145/-5/net +140`, public API `+0/-0` types, and the test file
`648 → 788` physical lines. Router production remains 1,311 lines, with
unchanged branches, modules, public spellings, and aggregate control state.
The surviving subordinate Unknown and Observed alternatives still own the
same latest version/token needed for future routing and rejection. The
residue scan found no production arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
consumer syntax. The actor transition and normalized routing laws were
cross-checked. Disposition: `pass` for the test-only evidence batch; A20
remains open for other catalogue and runtime laws.

| Law | Focused transition and independent trace | Remaining proof boundary |
|---|---|---|
| Configuration version | `operations::configuration` tests stale and equal-version conflicting candidates with complete value return. `catalogue_invariants::configuration_is_a_monotonic_atomic_register` compares each generated proposal with an independent optional version/value register, including idempotent equality and every action lane. | The recorded equality mutant covers one guard only; persistence and host delivery do not follow from this pure actor trace. |
| Readiness evidence | `catalogue_invariants::readiness_matches_per_dependency_version_registers` compares three independent version/status slots after generated known and unknown observations, including stale and equal-version conflicts with returned evidence. | The generated range cannot reach numeric version exhaustion, and no dedicated readiness mutation slice is recorded. |
| Registry identity | `discovery::registry` tests atomic stale unbind. `catalogue_invariants::registry_matches_atomic_compare_and_remove_bindings` compares a separate ordered binding list and exact bind, unbind, and lookup outcomes after generated commands. | The existing inversion covers exact-recipient comparison only; snapshot ordering and host delivery have no dedicated counterfactual here. |
| Topic membership | `catalogue_invariants::topic_is_an_ordered_idempotent_membership_snapshot` models first subscription order, duplicate subscription, unsubscribe, and publication to every current recipient or exact empty-topic rejection. | This standalone topic law differs from keyed `PubSub` membership; no topic-specific mutation slice or host admission is claimed. |

Next A20 test-only boundary hypothesis, before edit: readiness version evidence
is ordered by the full `u64` domain. At the maximum version, lower evidence is
stale, equal identical evidence is idempotent, and equal contradictory evidence
is rejected with the exact input; no arithmetic or wraparound manufactures a
newer observation. This is Bombay's version policy, not an actor-model law.
The existing independent three-dependency register model already checks the
complete state and action product after each generated command but samples
only small versions. Add `u64::MAX - 1` and `u64::MAX` to that model's input
domain, retaining its own comparison equation and exact error checks. No
production state, type, lane, branch, public spelling, module, or line changes.
The root readiness state remains a fixed ordered list of Unknown or
Observed(version,status); each observed pair is exactly the current value
needed by the next comparison and query. No arrival-history, repeated cause,
false cardinality, nested authority, semantic boolean, or structural syntax is
introduced. Cross-checks: actor transition law and normalized operations
catalogue. Disposition: `pass` for the test design, pending the run.

The widened register model passed in debug and optimized profiles. Each
generated trace still checks exact rejection data, the complete empty action
product on acceptance, and all three dependency slots after every command.
The change is test-only (`+0/-0` production lines, zero states, branches,
modules, and public spellings); the current evidence pair and fixed ordered
membership remain the only future-needed values. The residue scan and law
cross-check above remain unchanged. Disposition: `pass` for the boundary
evidence, with A20 still open for other catalogue and runtime laws.

A disposable worktree made readiness treat `u64::MAX` as stale even after
version zero. The widened independent model failed and shrank to two commands:
commit `(dependency 1, version 0, Ready)`, then offer the same dependency at
`u64::MAX`; the model required acceptance while the mutant returned the exact
wrong `Stale` error. This counterfactual demonstrates that the new boundary
input is exercised and the oracle rejects the bad ordering. The mutant and its
generated seed were removed; production remains unchanged.

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

The routing-invariant fixtures and work-queue unit tests also dropped their
destination-only inert `Behavior` implementations. Their routes require only
`Protocol`; the routing and queue tests still pass with those narrower
fixtures.
The pub-sub unit destination likewise now implements only `Protocol`.

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
The work-queue model now checks initialization and every step's full
assignment/outcome vectors, recipient, creation lane, and next verdict. It
tracks distinct non-Clone owned work and unique reply destinations through
both waiting and available FIFO queues; the focused property passed.
The keyed pub-sub model now independently checks topic introduction order,
recipient membership, complete successful actions, exact rejected commands,
and the original allocation of an undelivered owned publication. Its full
seven-test invariant suite passed.
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

At that checkpoint, thirteen checklist items were complete and seven remained
open. The source changes under `crates/*/src` were `+609/-430` physical
lines (net +179), including the stricter mutation verdict and logical-host
projection. The branch deletes the duplicate `BufferSends` and test-only
`InitializeTest` public names and adds one associated logical-protocol type
and one child-borrow method to existing ports. No actor control-state variant
was added. This scope measurement is diagnostic; it does not certify the seven
open design and integration items.

The later branch currently has fourteen complete items and six open items.
After `nix flake update`, `nix flake check` passed all ten applicable
`aarch64-darwin` checks on signed commit `d332af9`, including build, Nextest,
Clippy, docs, doctests, both formatting checks, audit, deny, and package
verification. These gates do not close the six remaining design, mutation,
and downstream integration criteria.

Against merge base `435560ce7bea8ad3330ee2d42e5034f837a80602`, the
current branch's source changes are net −78 lines in `behavior/src`, −1,089
in `actors/src`, −4 in `behavior-macros/src`, and −8 in
`behavior-testkit/src`: **1,179 fewer production and testkit source lines**.
The `crates/` tree as a whole is net +332 lines because focused tests and the
stricter mutation gate grew, alongside benchmark and supporting changes.
These `git diff --numstat` counts are diagnostics; the six
open checklist items still decide whether the remaining interfaces and
aggregate states are essential.

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
