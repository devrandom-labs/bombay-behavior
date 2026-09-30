# Interpreter contract implementation ledger

This ledger records implementation evidence against the separately maintained
[ownership and startup PRD](creation-and-delivery-custody-prd.md). It does not
change that specification or treat a scripted host as a production interpreter.

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

The remaining work is to establish the exact pre-commit versus post-commit
ownership equation with a production host witness, then cover PRD T10–T21 and
the catalogue composition cases. Any Behavior production-shape edit needs a
focused failing law regression and aggregate-drift record first. Real Bombay
changes are outside this Behavior branch while the instruction to leave that
repository untouched remains in force. A local or path-patched compile does
not close P5 or audit item A17.
