# Worker preparation interleaving hypothesis

Status: historical experiment log; the integrated owner result and final
checkpoint are recorded at the end of this file. This branch was
based on the exact Bombay-locked Behavior Core/Actors 0.19.0 source revision
`e5c703e966eba4d2a15fe2c129594f57ae2270fc`. Its complete `AGENTS.md`
was read before this experiment. The independent proxy-diagnostic ingress fix
is in a separate local branch and is not part of this hypothesis.

## Law and caller trace

The actor-model law is one deterministic Behavior transition per admitted
communication. Engine's D-TURN-1 is Bombay's derived causal-turn policy. The
FIFO, keyed-pool, and fixed-supervisor decision to fold shutdown while an
external worker source is still preparing is a deliberate template policy,
already exercised by their pure transition tests. No actor-model source
requires this specific shutdown policy.

The smallest live witness is Bombay's
`docs/research-probes/fifo-shutdown-during-preparation.patch`. A permanent FIFO
pool loses its first worker, and its replacement source waits on a oneshot.
The caller sends shutdown and one distinct job while the source is held. The
job must return its exact payload with `ShuttingDown` before the source is
released. The selected source-action interpreter instead waits for complete
preparation inside `Environment::apply`; the live test times out at the
shutdown-fold witness in both the locked owner and the independent
proxy-diagnostic candidate. The ordinary recovery suite remains green.

The desired public sequence is: issue one `PrepareWorkers` with its private
ticket and affine source; consume it into one ticket-bearing start receipt and
a distinct `StartingWorkerPreparation` before awaiting the source; permit the
aggregate to process shutdown; then return one exact typed preparation
outcome. An accepted start is not a
completed worker preparation. Source rejection after start is a late outcome,
not retroactive rejection of the start. An unattempted or corrupt start keeps
the original request in the existing source-action settlement. A late result
for another ticket or a replay cannot discharge the outstanding preparation.

## Current and candidate products

`PrepareWorkers` currently implements `SourceAction` with
`ActionItem::Accepted = WorkerPreparation`. Its `WorkerPreparation` owns the
private ticket and one `WorkerPreparationOutcome` containing the returned
source and either an ordered prepared group or the first rejected role with
its prefix and suffix. `Rejected` returns the original request and real source
rejection; `Blocked` is impossible because its prerequisite is `Never`.
Therefore no existing `ItemSettlement` variant truthfully represents a start
whose source is still running.

The revised candidate uses the same `PrepareWorkers` request and ticket.
`PrepareWorkers::start(self)` would return `WorkerPreparationStarted` and a
`StartingWorkerPreparation`; the request cannot mint a second receipt.
The receipt owns only the exact ticket. The starting preparation owns the
affine source and selected roles with an empty prepared prefix. A successful
first submission either completes one role or moves to the existing
`PendingWorkerPreparation`, which owns a nonempty prepared prefix. The late
event carries the existing complete `WorkerPreparation`; its private outcome
distinguishes prepared workers, worker rejection, and source rejection. Only
`StartingWorkerPreparation` can construct the source-rejected outcome, which
statically denies that classification after later roles begin. These are distinct facts and phases, not second actor
contracts. This is a hypothesis:
test the caller syntax and complete trace before retaining either public type.

The actor-side preparation expectation needs two current phases: an issued
request can accept only its exact start settlement, and a started request can
accept only its exact late return. This is a shared worker-preparation
expectation used by both direct pools and fixed supervision. A duplicate start
or an early late result remains an unexpected typed input; neither can
discharge the request. This phase is a fact needed for a future decision, not
an arrival-history flag or a second aggregate state machine.

The candidate must preserve the current early `Corrupt` and `Unattempted`
diagnostics, the complete late source rejection, the fixed supervisor's
multi-role prefix/suffix, closed-ingress retirement custody, and the pool's
shutdown choice. A runtime task panic or abort must force an explicit terminal
path rather than leave the aggregate waiting for an impossible event. The
Bombay runtime currently joins delayed activation tasks only at retirement;
that storage alone cannot provide the required active task-failure path.

## Direct-composition comparison

| Existing construction | Result against the live law |
| --- | --- |
| Await the source in the existing `SourceAction` interpreter | The Driver cannot receive shutdown until the complete action settles. |
| Spawn the existing request, then return `Accepted` | `Accepted` requires the complete `WorkerPreparation`, which does not exist at start. |
| Return `Rejected`, `Blocked`, `Corrupt`, or `Unattempted` at start | Each variant asserts a different false fact or requires an uninhabited prerequisite. |
| Read shutdown inside the interpreter | Reenters the Behavior fold before the current action settles, violating D-TURN-1. |
| Typed start receipt and later typed return | Expresses the two facts; needs an owner API and a Bombay task-failure path. |

No macro, dynamic source map, actor facade, second Behavior algebra, or
application adapter is justified by this comparison.

## Aggregate-drift checkpoint before the experiment

- FIFO `PoolState`: `Constructed`, `Operating`, `Draining`, `Stopped`,
  `ForcedRetirement` (five alternatives). Keyed `KeyedPoolState` has the same
  five semantic phases under its own policy. `FixedRoster`: `New`,
  `Operating`, `ShuttingDown`, `Terminating`, `Stopped` (five alternatives).
  The fixed recovery source custody is `Available`, `PreparingWorkers`, or
  `Retirement` (three alternatives). No candidate adds an aggregate phase.
- Existing worker preparation has three public progress products
  (`PrepareWorkers`, `PendingWorkerPreparation`, `WorkerPreparation`) and two
  private outcome alternatives (`Prepared`, `WorkerRejected`). The revised
  candidate adds two public products with distinct timing and one private
  source-rejected outcome. It also replaces each actor's bare
  ticket expectation with a private `Issued | Started` sum. No FIFO, keyed,
  or fixed aggregate root phase is added. After counts and residue scans
  remain pending until a focused experiment exists.
- Each surviving value has a future decision: the request carries source and
  selected roles; the pending request carries prepared prefix and next role;
  the complete preparation carries the source and selected result; the start
  receipt proves the exact ticket began; the starting preparation alone can
  reject the source before the first submission; the existing complete product supplies the
  exact terminal source result. None may store a separate arrival-history
  label.
- No semantic boolean, false cardinality, nested transition authority, or
  structural user syntax is proposed. The candidate preserves one aggregate
  transition authority for each template. Cross-checks are
  `docs/atomic-runtime-settlement.md`, `docs/fixed-supervisor.md`,
  `docs/keyed-pool.md`, the atomic actor architecture documents, and the
  selected FIFO/keyed/fixed pure transition tests. Disposition: **pending**;
  no production representation has been retained.

## Change ledger before production

- Exact blocker: the complete accepted source-action result cannot be
  constructed before its asynchronous source finishes. Smallest end-to-end
  regression: the held-source FIFO shutdown/job witness above. The first
  owner-local probe will use a one-role FIFO request, start it, fold shutdown,
  then return its exact prepared worker; it must fail against the old API.
- First focused stage forecast: `crates/actors/src/atomic/worker/preparation.rs`,
  the worker and atomic re-exports, FIFO event and aggregate modules, the
  shared direct-pool worker owner, one focused FIFO test, and this ledger;
  production about `+260/-165/net +95`, tests about `+75/-10/net +65`, two new
  public types and zero removed. This forecast does not authorize migration of
  keyed and fixed callers. If the focused API cannot serve those two consumers
  and both relevant wrapper orders without placeholders, reopen the model.
- Reuse `PreparationTicket`, `WorkerSource`, `SourceAction`, `SourceActions`,
  `ItemSettlement`, `EventIngress`, each aggregate's existing preparation
  expectation, and the generic Driver source-admission order. Replace the
  single-stage accepted result and its direct event meaning; delete no
  unrelated capability. Bombay's eventual interpreter must reuse its own
  actor task hierarchy and typed control ingress, and explicitly monitor task
  failure while active.
- Automatic checkpoint: stop before any further production edit once the
  cumulative task exceeds 15 changed files, 500 net new production lines, or
  three new public types, unless the user explicitly authorizes that expanded
  surface. A green compile cannot override the model or the checkpoint.

## Focused prior-contract result

The first owner-local caller test is
`preparation_start_commits_before_shutdown_and_late_worker_return` in
`crates/actors/tests/fifo_pool.rs`. It starts the exact request, folds FIFO
shutdown while the source request is still owned outside the aggregate,
retires the remaining live worker, then returns one prepared worker and
requires a stopped state with no replacement creation or new preparation.
Against the selected 0.19.0 API, pinned
`nix develop -c cargo test --locked -p bombay-behavior-actors --test fifo_pool
preparation_start_commits_before_shutdown_and_late_worker_return --no-run`
fails E0599 for the absent `WorkerPreparationStarted` event and absent
`PrepareWorkers::start` method; the optimized `--release --no-run` command
was first run against the borrowed-receipt spelling and failed for the same
missing start and event capabilities. The consuming-start test must be rerun
in both profiles before production resumes. These are the proposed start facts, not
compiler-invented architecture; the complete live Bombay timing failure is
the independent behavioral falsifier. No owner production file has changed.

The public names follow the [official Rust API naming guidance](https://rust-lang.github.io/api-guidelines/naming.html):
domain types and event variants use `UpperCamelCase`, and the consuming
`start()` method names the exact custody transfer.
The owner event sums should carry the two semantic inputs directly; a shared
wrapper around those variants would add public surface without owning another
decision. The candidate's two public products own a distinct receipt and
first-attempt authority. The existing complete product owns the late result.
This naming review is not approval of the semantic model.

Complete branch checkpoint before this record: two changed paths including
one untracked document; production `+0/-0/net 0`, tests `+72/-0/net +72`,
docs `+128/-0/net +128`, public API `+0/-0`. The aggregate control-state and
subordinate-product counts remain exactly the baseline above. Disposition:
**pending**, because the design has not yet passed its cross-template or
runtime-custody proof; it is not retained as an owner contract.

## Consuming-start source stage

Two source-only hypotheses were reopened and removed; their falsifiers are in
`DEAD_ENDS.md`. The current third hypothesis uses the existing request as an
affine pre-start action, a consuming `start()` transfer, and an initial
`StartingWorkerPreparation` value. This value alone can return a source
rejection. Its first accepted submission either completes the group or moves
to the existing `PendingWorkerPreparation`, whose prepared prefix is then
nonempty. `WorkerPreparationStarted` is the exact start receipt, while
the existing `WorkerPreparation` owns the distinct later
prepared/source-rejected outcomes. The prior `PrepareWorkers` first-attempt methods are deleted; this
is a phase split, not a stored wrapper or a second transition authority.

The source and re-export changes are deliberately incomplete until the three
aggregate consumers and Bombay runtime path are checked. Pinned owner
`cargo check --locked -p bombay-behavior-actors --lib` currently reports one
cluster: fixed recovery and direct pools still expect the old complete
`ActionItem::Accepted` shape or import its old correlation predicate. No
bound, alias, default generic, or placeholder has been added in response.
The compiler is vetoing the old consumers; it did not originate the new
types. The first stage forecast was low: current source under `src/` is
`+165/-43/net +122` before any aggregate migration. This is new capability
code, not code reduction. Complete branch checkpoint before this record: six
changed paths including one untracked document; tests `+72/-0/net +72`,
`DEAD_ENDS.md` `+64/-0`, design document `+172/-0`, and three then-proposed new
public types. No aggregate control-state or module count has changed yet.
Disposition: **pending**; no broad migration or Bombay production edit is
eligible until the focused owner surface and runtime failure path are proven.

The unrelated fixed-supervisor caller now has the same test-first shape in
`late_preparation_and_proxy_exit_close_shutdown_in_either_order`: it consumes
one request into a start receipt and first-attempt authority, folds shutdown,
then returns the exact prepared worker while proxy retirement is still
outstanding. In a separate untouched 0.19.0 worktree containing only the two
test edits, pinned debug and optimized compile checks for that fixed test
both fail E0599 at the missing `PrepareWorkers::start` and
`FixedSupervisorEvent::WorkerPreparationStarted`. The FIFO caller failed the
same two symbols against the untouched source. These are two real aggregate
families, not fixture-only aliases. The pure tests do not yet prove Bombay's
active task-failure path or that either post-change trace passes.

Complete branch checkpoint before this record: seven changed paths including
one untracked design document; production `+165/-43/net +122`, tests
`+89/-8/net +81`, `DEAD_ENDS.md` `+64/-0`, design document `+201/-0`;
public types then proposed `+3/-0`. The source candidate retained the
  original FIFO/keyed/fixed aggregate control-state counts and introduces one
  first-attempt progress phase plus one shared actor-side expectation phase,
  not another aggregate. The incomplete owner
library check still fails at the known old-consumer cluster. Disposition:
**pending** until the full ownership path compiles and the focused laws pass.

## Shared direct-worker stage checkpoint

The partial owner migration now puts the issued/started expectation in the
shared direct-worker state. A start settlement can move an exact expectation
from `Issued` to `Started`; only a started expectation admits a later complete
return. The shared direct-worker code also distinguishes early source-action
faults from late source rejection. The FIFO event sum has distinct start and
return variants. FIFO, keyed, and fixed aggregate consumers still contain the
old one-stage spelling, so this is **not** a passing implementation or an
accepted contract. Pinned `cargo check --locked -p bombay-behavior-actors --lib`
reports those old-consumer errors; it reports no independent error in the new
direct-worker module. The unfiltered command exits nonzero.

The forecast underestimated the cross-template production surface. Complete
branch checkpoint here: nine changed paths including the untracked design
document; production `+580/-123/net +457`, tests `+89/-8/net +81`,
`DEAD_ENDS.md` `+64/-0`, public types then proposed `+3/-0`. Rustfmt ran through the owner
Nix shell and `git diff --check` passes. The aggregate root phase counts remain
unchanged. The proposed new subordinate `Issued | Started` expectation is
retained provisionally because a duplicate start must not be able to discharge
the later source result. The three-public-type ceiling was then reached;
the separate late-result wrapper was subsequently reopened and removed.
The +500 net production-line ceiling is near, so the next
logical change must either reduce the representation or stop for the same
authorization before exceeding it. Disposition: **pending**.

The complete owner reference scan finds the old single-stage result in the
FIFO, keyed-pool, and fixed-supervisor event, aggregate, recovery, shutdown,
and diagnostic modules, plus their own tests and fixed-supervisor fuzz
targets. The prospective migration necessarily touches more than the
original eight-path focused forecast. The independent proxy-diagnostic branch
cannot supply the missing timing contract. The first additional production
paths are FIFO `mod.rs` and `protocol.rs`; completing all three actor families
requires keyed `event.rs`, `mod.rs`, `protocol.rs`, `recovery.rs`, and
`shutdown.rs`, and fixed `event.rs`, `mod.rs`, `recovery/mod.rs`, and
`shutdown/member.rs` at minimum. The selected Bombay interpreter and active
task-failure observation are separate downstream work. This path inventory
is the concrete reason the repository's cumulative 15-path and +500-line
authorization checkpoint is expected to be reached; splitting commits would
not change the count.

## Result-surface minimization checkpoint

The proposed public `WorkerPreparationReturn` was a forwarding wrapper around
the existing complete `WorkerPreparation`. It is removed, with the exact
falsifier recorded in `DEAD_ENDS.md`. The revised private outcome sum adds
`SourceRejected { source, failed_role, reason, remaining }`; only the public
`StartingWorkerPreparation`, which has no prepared prefix, can construct it.
The late event carries `WorkerPreparation` directly, so this candidate has
two new public types, not three. The FIFO and fixed caller tests now spell the
direct return; they remain negative controls until their aggregate consumers
are migrated.

Complete branch checkpoint: nine changed paths including this untracked
document; production `+606/-164/net +442`, tests `+88/-11/net +77`,
public API `+2/-0` types. The aggregate root control states remain FIFO five,
keyed five, and fixed five; the subordinate issued/started expectation is the
only new actor-side phase. No arrival-history field, semantic boolean,
runtime lookup, or second source action was added. Pinned rustfmt and
`git diff --check` pass. The owner library check still fails at the known
unmigrated consumer cluster, and the Bombay live timing witness remains red.
Disposition: **pending**.

Bombay task-custody cross-check: its current `ActivationTasks<E>` owns a
`JoinSet<Result<(), E>>` inside `ApplicationCapabilities`, while
`ActiveLocalEnvironment::next` polls its mailbox, facts, timers, and owner
cancellation, but not interpreter-owned tasks. `ActivationTasks::settle`
joins and resumes panic only after retirement. A new delayed source task
that panics before sending its typed late result would therefore leave a
live pool waiting indefinitely unless the active environment observes task
failure. The existing `BeginActivation` pattern proves typed return through
the control lane and exact closed-lane recovery, but does not prove live panic
observation. A Bombay implementation must show one active failure path and
one retirement path using the existing actor-owned task hierarchy; merely
spawning into the current set is insufficient. No runtime source has changed
in this owner experiment.

The revised caller tests were copied into an otherwise untouched selected
0.19.0 worktree. Pinned debug and optimized `--no-run` checks for each of
`fifo_pool::preparation_start_commits_before_shutdown_and_late_worker_return`
and `fixed_supervisor_initialization::late_preparation_and_proxy_exit_close_shutdown_in_either_order`
all exit 101 with E0599 at the missing consuming `start()`, start event, and
late-return event. This confirms that the minimized two-public-type syntax is
still a genuine prior-contract negative control in two unrelated templates.

Before the next direct-pool edit, the early-start helper exposed a modeling
fault: reusing `WorkerRecoveryPreparation::Ready | Failed` as its break value
would force each caller to handle an impossible ready worker at source-action
start. An early start fault uniquely owns the source, role, prior worker,
recovery count, stop fact, and exact interpreter error. A private named
`FailedWorkerPreparation` can carry only that case; it transforms no event
and removes an impossible `Ready` match from FIFO and keyed start admission.
The concrete use is a corrupt or unattempted preparation start during pool
drain. The candidate should add that private product before a FIFO start
handler; it adds no public type or aggregate phase. The current source still
has the overbroad return and is not retained. The next edit must keep the
complete task below the +500-line checkpoint or stop for authorization.

Checkpoint after adding the FIFO correlation scan and concrete event type
arguments: eleven changed paths including this untracked document;
production `+646/-176/net +470`, tests `+88/-11/net +77`, public API
`+2/-0` types. The FIFO start handler and all keyed/fixed consumers remain
unmigrated. The source representation is intentionally incomplete, and the
overbroad early-failure type above must be corrected before retaining it.

The early-start failure value is now a private `FailedWorkerPreparation` with
an exact `InterpreterCorrupt | InterpretationSkipped` fault sum. It does not
carry a `Ready` alternative or a plan marker without owned plan state. The
first check briefly rejected an unused `Plan` parameter with E0392; removing
that redundant parameter cleared the new helper's diagnostics. The remaining
owner library errors are in the unconverted FIFO, keyed, and fixed consumers.
Complete branch checkpoint: eleven changed paths including this untracked
document; production `+664/-176/net +488`, tests `+88/-11/net +77`, public
API `+2/-0` types. Pinned rustfmt and `git diff --check` pass. The next
coherent production edit is the FIFO start handler, which would cross the
automatic +500 net-line checkpoint. Expanded-surface authorization was
requested; no further production edit is eligible without it.

## Integrated owner result and retained checkpoint

The user explicitly authorized the expanded cumulative surface on 2026-10-01.
The diagnostic ingress batch and the preparation timing batch now coexist on
`codex/arc010-owner-contract`. The two public progress products are
`WorkerPreparationStarted` and `StartingWorkerPreparation`; the existing
`WorkerPreparation` carries the later result. `PrepareWorkers::start` consumes
the issued request and returns one exact start receipt plus first-attempt
authority. An accepted start is never treated as a prepared worker. The later
result carries the source and exactly one of a prepared group, worker rejection,
or source rejection. Corrupt and unattempted starts retain the original
request. FIFO, keyed, and fixed supervisors admit the start and later result
through distinct typed event variants. Their shutdown paths retain the exact
outstanding ticket, never create a replacement after shutdown, and finish
retirement after the late result.

The actor-model law is still one transition per admitted communication and
fresh creation. The two-stage source timing, exact ticket correlation, and
shutdown admission order are Bombay policy, not guarantees from Agha et al.
The full user trace requires a runtime to commit the start receipt before
awaiting source work and to inject the later result through a typed control
lane. The isolated Bombay adoption patch at
`docs/research-probes/bombay-worker-preparation-runtime.patch` does exactly
that without a dynamic envelope or template-specific Driver branch. It also
observes actor-owned source task failure while the actor remains active and
preserves completed task custody during retirement. The original Bombay
working tree was not edited.

Final aggregate-drift checkpoint, measured from retained diagnostic commit
`31f2fd1`: FIFO `PoolState` remains five alternatives; keyed
`KeyedPoolState` remains five; fixed `FixedRoster` remains five. The
`WorkerSourceCustody` alternatives remain `Available`, `PreparingWorkers`, and
`Retirement`. The prior bare preparation ticket becomes one private
`Issued | Started` expectation: `Issued` owns the exact ticket needed to
admit one start settlement, and `Started` owns that same ticket needed to
admit one late result. `WorkerPreparationOutcome` grows from `Prepared |
WorkerRejected` to those two plus `SourceRejected`; each alternative owns the
source and the exact prepared prefix, failed role, reason, or untouched suffix
needed by the aggregate's recovery/retirement choice. The fixed shutdown
custody adds `Start | Returned` so an early start fault and a completed late
source result remain distinguishable without a second cause label. The direct
worker early-fault value owns the source, selected role, prior worker, stop,
recovery count, and `InterpreterCorrupt | InterpretationSkipped` fault; it has
no impossible ready-worker branch.

Across the 22 changed production source files, measured `=>` arms change
1303 to 1389, production lines 20585 to 21840 (net +1255), reachable `pub`
declarations 128 to 134, and the module count is unchanged. The complete
staged preparation batch is production `+1682/-427/net +1255`, tests
`+528/-434/net +94`, and documents/research artifacts `+1008/-55/net
+953`. The cumulative branch since the selected 0.19.0 base `e5c703e` is
production `+1786/-421/net +1365`, tests `+648/-443/net +205`, and
documents/research artifacts `+1086/-49/net +1037`. Public API adds two
types and removes none; no production source module was added. The public
surface adds two progress types, their start/source-rejection methods, and
separate start/return event variants for each of FIFO, keyed, and fixed
supervision. The residue scan found no arrival-history field, repeated cause,
false cardinality assumption, nested transition authority, semantic boolean,
or positional wrapper path. The source request, receipt, starting cursor,
pending cursor, and completed result each own a different current custody or
phase needed by a future decision.

Cross-checks: `docs/atomic-runtime-settlement.md`,
`docs/actor-laws/{fifo-pool,keyed-pool,fixed-supervisor}.md`, the corresponding
template docs, and Bombay's ARC-010 live probe. `cargo test --locked
--workspace` passes in this owner worktree. In an isolated Bombay copy pinned
to this owner source, the held-source FIFO runtime probe passes: shutdown and
the exact `ShuttingDown` job reply occur before source release; the eventual
source result closes the tree without another worker. The active source-task
panic probe also passes, as do all 186 Bombay library tests and all five FIFO
recovery integration tests. Three affected fixed-supervisor fuzz targets
compile and pass crafted seed inputs; coverage-guided fuzzing requires a
nightly toolchain unavailable here. The downstream patch passes `git apply --unidiff-zero --check`
against the current Bombay working tree. Disposition: `pass` for the owner
contract; downstream Bombay adoption is a separate repository change.
