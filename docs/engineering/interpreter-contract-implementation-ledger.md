# Interpreter contract implementation ledger

This ledger records implementation evidence against the separately maintained
[ownership and startup PRD](creation-and-delivery-custody-prd.md). It does not
change that specification or treat a scripted host as a production interpreter.

## Merged Behavior candidate, 2026-09-30

The release branch merged current `main` at `835cf8d` into the repaired
Behavior candidate at `bd613ae`. The merge retained both ordinary direct
dependency and facade-first Actors macro witnesses. Its five macro-resolution
fixtures passed, followed by all ten local aarch64-darwin Nix checks and the
external interpreter fixture in debug and optimized builds. The user's revised
PRD was copied verbatim from the original worktree into this release branch;
the original uncommitted file remains untouched.

The external workspace now has a README and a Cargo-metadata guard. The guard
requires one local resolved package for each of core Behavior, Actors, and
macros, rejecting a duplicate or registry copy of these crates. It passed on
the merged candidate. It does not assert that the later Bombay/Address graph
is immutable or integrated. The retained-diagnostic witness now carries one
non-cloneable value across three later source-free offers, adding a discharged
diagnostic on each offer; each resulting residual contains only the original
terminal value. Debug and optimized tests pass. This strengthens the
Behavior-side T06/T07 law, while the real Driver retirement remains P5.
The precommit creation fixture also now rejects pure initialization with a
non-cloneable child and error, verifies that `into_actor` refuses to issue an
established capability, and returns the exact current child, route, ID, kind,
and error allocation. Debug and optimized tests pass. This is the Behavior
ownership shape for T10; the real reservation and publication trace remains
downstream.
The external mixed-product fixture now combines retirement creation custody,
an accepted terminal diagnostic, a live source request, and an independent
rejection. It exercises two `SendLayer` orders, requires source admission to
progress past retained siblings, and verifies that closed admission returns
every lane and the continuation verdict. The new focused tests pass in debug;
the optimized fixture and final gate follow after this batch is staged. This
is the Behavior-side composition portion of T08/T24, not a real Driver trace.
The canonical adapter contract was corrected to distinguish private host
commitment and `EstablishedCreation::Installed` from later effect settlement
and public resolution. It now states that ordinary rejection continues
independent effects while corruption owns an untouched suffix. This is a
documentation correction to the existing Behavior algebra and the PRD's
selected startup order; no runtime publication implementation is claimed.

Before/after for this evidence-only batch: aggregate control states and
subordinate alternatives unchanged; production transition branches and
production lines unchanged; one fixture README and one graph-check script
added; one CI step and one external test changed; public spellings unchanged.
The terminal value remains the sole future-needed residual. No arrival-history
state, duplicate cause, cardinality assumption, nested transition authority,
semantic boolean, or structural caller syntax was added. Cross-checks:
`actor-transition-algebra.md`, `atomic-runtime-settlement.md`,
`behavior-layer-laws.md`, and the five normalized actor-law documents.
Disposition: `pass` for this Behavior-side evidence batch. The PRD's real
runtime acceptance matrix is still open.

## P0 snapshot, 2026-09-29

Behavior is at `605d0634644a435f4bba31ce1768253abcafe23e` on
`codex/repository-quality-repairs`. The PRD is an uncommitted user edit and is
excluded from this snapshot. The isolated Bombay worktree is at
`f462b8ec25a4cc43aabf89fe448c09e3da9d12b7` with pending local changes;
the Address checkout is at `87f7af4fd67671bbcf05c59f5da687b40ce7ba98`.
Neither adjacent repository was edited for this ledger.

| File under `crates/` | Current SHA-256 |
|---|---|
| `behavior/src/effects/sending.rs` | `4e8c578ae4a99383471fea3f7fc579f1c066d249505ae242fad9e47b952e09c2` |
| `behavior/src/effects/actions.rs` | `189cb846f51c0b07d38789cf1bca5b291b73e86c58b9428063ab9bf61db78ae9` |
| `behavior/src/actor/creation.rs` | `c47b085f147ce1443f634b3527a426030ec165923c0e833b8bb1cbb0d610172f` |
| `actors/src/atomic/pool/assignment.rs` | `308f3d4a78e06be1e027a1a3410ff374a72c2eb4897bcfbc278425cc1ecbbf8c` |
| `actors/src/atomic/stable_proxy/operation.rs` | `fda908e08c7a285d6b7e32555402ea9c8aac32f45f06a2dc631e1efd0b4220f3` |
| `actors/src/atomic/diagnostic.rs` | `8a36bdb2d6f28d5279c52a131fdf999f881ec7394f81a89982e686813538f8e6` |
| `actors/src/atomic/worker/initialization.rs` | `09487c21e7368911242aceaf202fa81f4627037bd6f21a9f76bb56e42a46a3f6` |

| PRD defect | Current-source result | Remaining proof |
|---|---|---|
| D1 assignment ownership | `AssignWorker::settle` consumes the request and returns its original receipt or complete request; decomposition and reconstruction are crate-private. External acceptance/rejection and compile-fail fixtures pass. | Real worker admission, two concurrent same-typed requests, and production source return. |
| D2 proxy ownership | `ProxyOperation::settle` uses `ProxyControlAdmission`; operation decomposition is test-private and the receipt constructor is private. External privacy fixtures pass. | Real start/replacement/shutdown admission and failure races. |
| D3 retained acceptance | Generic `ActionItem::retain_accepted` compacts discharged receipts; diagnostic ownership opts into retention. External custody fixtures pass in both profiles. | Real Driver continuation and retirement with the exact retained value. |
| D4 startup ownership | `HostRejected` still requires untouched initialization `Actions`. `InitializationPanicked` already exists and has an external pure-fold custody witness. | Host commitment and every post-commit failure outcome through the real runtime. |
| D5 child commitment | Isolated Bombay `establish_child` still awaits `spawn_owned_with`, which waits for publication. A pre-publication end can still reach a panic branch. | Private commitment acknowledgement and exact unpublished terminal return. |
| D6 hidden address reservation | Address now exposes `try_reserve` and consuming `Reservation::publish`; the isolated Bombay `LocalEnvironment::prepare` uses it. | Runtime interleavings, bounded nested progress, and immutable dependency graph. |
| D7 premature publication | The isolated Driver now calls `publish` only on the continuing initialization path after pending settlements progress; the early terminal branches no longer call it. | Production startup rejection/corruption/stop traces and public-resolution absence. |

The fixture at `tests/interpreter-contract` passed all 12 baseline test functions in
debug and optimized profiles. Its assignment host and startup panic witness
are local interpreters, so they do not close the production runtime matrix.
The fixture is already wired into `.github/workflows/checks.yml` in both
profiles. A clean Nix flake check at the Behavior snapshot passed eight
available `aarch64-darwin` checks, including 843 optimized Nextest tests.

## Next ownership proof

**T03 focused law before extending the fixture:** two independent FIFO pools
can have same-typed worker deliveries outstanding at once, even when their
creator-local child IDs are numerically equal. Settling those deliveries in
reverse order must return each opaque accepted receipt to its originating
pool. Both pools must continue without a foreign-receipt diagnostic, and each
later worker completion must produce exactly its own customer outcome. The
existing `AssignWorker::settle` and FIFO receipt correlation supply the
lower-order behavior; no new production representation is proposed.
The external regression now passes in debug and optimized profiles. It uses
two separate pool instances with equal numeric child IDs, settles their
same-typed requests in reverse order, rejects any diagnostic or early customer
outcome, and checks both later completions. This is a focused T03 witness,
not a production transport witness.

**T01/T02 pointer witness correction:** the previous fixture recorded the
address of a `Box<str>` handle and compared it with the address of its heap
payload. Those different addresses could not prove job custody. Requiring
equality first made both baseline assignment tests fail, exposing the weak
assertion. Source inspection then found `FifoPool` requires `Job: Clone` and
clones the queued payload into `Assignment`; the pool retains the original so
it can return it if delivery fails. The corrected fixture records the worker
payload's actual heap address, requires it to differ from the original, and
requires a rejected assignment to return the original address. The concurrent
test checks both copied worker payloads in reverse settlement order. No
production symbol or actor transition changed.

The earlier PRD's T02 requirement for a **move-only job** is not proved by this FIFO
fixture: `Box<str>` implements `Clone`, and the current FIFO bound rules out a
non-`Clone` job. The full ownership equation must be revisited before claiming
T02 complete; a test using a cloneable value cannot certify a move-only law.
This is a real policy conflict, not just a missing test: FIFO's dispatch
clones `queued.customer.payload` into `Assignment` while retaining the original
in `AssignedJob`. On accepted delivery followed by a worker stop,
`Interruption::Fail` returns that original as `ReturnedAssigned`, and
`Interruption::Retry` requeues it. A single non-`Clone` job cannot be both
transferred to an independently executing worker and retained by the pool for
those later paths. The public pool is the only current producer of this exact
assignment request besides the similarly clone-bound keyed pool. A future
move-only witness therefore needs a changed post-acceptance return/retry law
or a distinct lawful producer; merely changing the `Job: Clone` bound would
make the existing transition impossible. No such policy change was made here.

The revised T02 preserves FIFO's existing clone-and-retry law. It requires a
rejected delivery to return the pool's exact original job and affine
completion authority without another settlement-time clone. The pointer
witness proves this Behavior-side distinction; the real close-after-resolution
runtime trace remains open for Bombay integration.

The remaining work is to establish the exact pre-commit versus post-commit
ownership equation with a production host witness, then cover PRD T10–T21 and
the catalogue composition cases. Any Behavior production-shape edit needs a
focused failing law regression and aggregate-drift record first. Real Bombay
changes are outside this Behavior branch while the instruction to leave that
repository untouched remains in force. A local or path-patched compile does
not close P5 or audit item A17.

## T11 Behavior-side host-refusal custody, before fixture edit

The derived Bombay pre-commit law says a host refusal after a successful pure
initialization returns the current child and the complete untouched `Actions`;
no request has crossed the interpreter boundary. The existing public
`ChildCreationOutcome::HostRejected` already carries those values, but the
`action_interpretation` host-rejection case supplies `Actions::cont()` and
cannot detect loss of a pending send. The next external fixture will run a
move-only child's pure initialization, retain two ordered move-only sends, then
construct and decompose the public host-rejection outcome. It will check the
original allocations, route, creation ID, kind, send order, and continuation
verdict. No production type, bound, host operation, or state change is
proposed. This proves the Behavior-side ownership equation only; a real
reservation refusal and non-interpretation trace remain mandatory for T11.

The external `startup_host_rejection` fixture now passes in debug and
optimized profiles. `OwnedText` has no `Clone` implementation. The test checks
that pure initialization moves both pending sends into `Actions`, then a host
refusal returns the mutated current child, both exact send allocations in
order, the original route, creation ID, birth kind, and `Continue` decision. The
first version adds 116 test lines and zero production lines, types, states, branches,
modules, or public spellings. No arrival history, repeated cause, false
cardinality, nested transition authority, semantic boolean, or positional
caller syntax is introduced. Cross-checks: the actor transition algebra and
PRD section 6.3. Disposition: `pass` for the external Behavior-side custody
witness. T11 remains open because the fixture does not own an Address
reservation or a production child host and therefore cannot prove that a real
host attempts no effects after refusing commitment.

**Creation-lane extension, before edit:** the first fixture observes two sends
and `Continue` but leaves the creation leg empty. A host refusal must also
return a staged nested creation untouched. Extend the same fixture with one
move-only nested child, check its exact allocation, ID, and birth kind after
decomposing `HostRejected`, and keep the existing ordered-send assertions.
This is a test-only strengthening of the same derived law; no actor or
interpreter type changes.

The extension passes in debug and optimized profiles. `Grandchild` owns a
separate non-`Clone` value; its staged request stays in the returned creation
leg with the exact allocation, nested creation ID, and birth kind. The
fixture now has 157 test lines and checks sends, creation, and next-behavior
decision together. Production state, branches, modules, and public API remain
unchanged. The nested child is current data required for a possible later
commit, not arrival history; no duplicated cause, false cardinality, nested
aggregate authority, semantic boolean, or positional caller syntax is added.
Cross-checks remain the actor transition algebra and PRD section 6.3.
Disposition: `pass` for the Behavior-side complete-actions witness; T11 still
requires the real host refusal and absence of effect attempts.
