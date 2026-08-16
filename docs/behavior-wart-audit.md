# Behavior-owned wart audit

This ledger records the behavior-owned findings closed by the 2026-08-14
wart pass. It is an implementation audit, not a new actor-model authority.
Research-mandated laws, Bombay-derived constructions, and Bombay policy are
distinguished below. The sibling Bombay runtime is outside this workspace;
this list makes no claim that runtime-owned interpreter work was changed here.

| # | Closed wart | Classification | Current evidence |
|---:|---|---|---|
| 1 | Several overlapping behavior-definition abstractions (`State`, `Base`, `FnState`, handlers, and function adapters) obscured the actual fold. | Bombay API policy | `Behavior` is the only executable definition trait; `#[behavior]` generates that concrete implementation. `BehaviorBase` is only a static inspection projection through wrappers and cannot execute a fold. |
| 2 | Definition and execution methods were available on the same value. | Bombay typestate derivation | `Activate::initialize` consumes any concrete `Behavior` into `Initialized<B>` and `Active<B>`; only `Active<B>` accepts events. No definition container is required. |
| 3 | Pre-initialization transition was a runtime misuse state. | Bombay lifecycle policy | It is a compile error because concrete definitions expose composition and consuming activation, while mailbox folds exist only on `Active<B>`. |
| 4 | Repeated initialization was a runtime error/panic boundary. | Bombay lifecycle policy | Initialization consumes the definition; `Active<B>` does not implement `Behavior`. Compile-fail doctests pin both exclusions. |
| 5 | Initialization effects could be treated separately from the active behavior without a named product. | Bombay ordering policy | `Initialized { behavior, actions }` keeps both results together; interpreters must consume those actions before admitting mailbox input. |
| 6 | Wrapper effect lanes used positional `SendProduct { inner, own }` nesting. | Bombay typed-product derivation | Every wrapper exposes a named product with semantic fields such as `behavior`, `schedules`, `observations`, and `replacement_commands`. |
| 7 | Public wrapper inspection used generic `.inner()`/`.behavior()` chains whose meaning depended on nesting depth. | Bombay naming policy | `BehaviorBase::base` projects directly to the authored behavior at any wrapper depth. `StashStatus` names the one wrapper-local observation needed by tests. No public recursive wrapper accessor remains. |
| 8 | Protocol/error variants used the context-free name `Inner`. | Bombay naming policy | Variants name their actual lane, principally `Behavior`, `Command`, or `Fleet`. |
| 9 | Fleet members had competing `Crew` and `Worker` names. | Bombay vocabulary policy | Only `Worker` terminology remains in the public fleet API and generated sums. |
| 10 | Several production lifecycle paths panicked on ordinary rejection or exhaustion. | Typed-failure policy | Proxy, supervisor, pool, timer, creation, and worker-factory failures return concrete error variants. Production source contains no panic/expect site; the two checker matches are inside `#[cfg(test)]` modules. |
| 11 | Error enums were manually formatted and did not compose consistently. | Rust API policy | Concrete errors derive `thiserror::Error`; `thiserror` is configured without default features. |
| 12 | Generic supervised-behavior failures were called `Inner`, hiding their source. | Bombay naming policy | `SupervisorError::Behavior(E)` identifies the wrapped behavior failure. |
| 13 | Fleet and behavior errors were conflated at supervision boundaries. | Typed-sum derivation | `SupervisorError<E, N>` is an exhaustive sum of behavior, fleet, and factory rejection. Plain `?` is used for unambiguous conversions; explicit `map_err` selects the generic behavior lane where overlapping `From` implementations are forbidden by coherence. |
| 14 | Worker factories relied on indexing assumptions and panic paths. | Typed-rejection policy | Factories return `Option<Worker>` and absence becomes `FactoryIndex`/`WorkerFactoryIndex`. Generated out-of-range construction is absent rather than panicking. |
| 15 | Configured child-route collisions could reach initialization ambiguously. | Freshness law plus Bombay staging policy | Duplicate nonces are rejected as typed configuration/fleet errors before creations are emitted; collision never overwrites an actor. |
| 16 | Creation request and committed birth/restart provenance were insufficiently separated. | Fresh allocation law plus Bombay policy | `CreationKind`, creation-resolution protocols, and replacement-resolution types carry request, acceptance, and replacement provenance explicitly. |
| 17 | A requested replacement could be reported as a completed restart too early. | Bombay lifecycle policy | Proxy routing and restart reporting wait for the exact matching successful creation result; rejection remains vacant/unroutable. |
| 18 | Stable identity risked being confused with address replacement. | Derived proxy construction | Supervisors send replacement commands to stable proxies; proxies stage fresh worker incarnations. No replacement-at-address primitive exists. |
| 19 | Timer wrappers could conflate equal deadlines or nested timers. | Bombay typed-protocol derivation | `TimerId` and `TimerGeneration` are explicit; wrappers accept only their matching live timer observation. |
| 20 | Timer exhaustion and stale observations had implicit or lossy behavior. | Bombay timer policy | Exhaustion is represented in the timer state/result path, stale inputs are inert/consumed by the owning lane, and unrelated lanes are forwarded unchanged. |
| 21 | FSM replay dropped the unprocessed suffix when a replayed fold failed. | Bombay replay policy | `Machine::drain` restores the remaining batch before returning the controlled error; regression, exhaustive, property, and fuzz-target checks cover it. |
| 22 | FSM advancement encoded correlated state as `(Step, bool)`. | Rust algebraic-state discipline | A private exhaustive `Advance::{Continue, PhaseChanged, Stop}` sum now owns the legal outcomes. |
| 23 | Pool admission/retry cloning could partially commit ownership before a panic from application `Clone`. | Bombay transactional policy | Required clones are completed before admission/assignment state is committed; adversarial clone tests pin both admission and retry paths. |
| 24 | Pool worker phases and stale completions were inferred procedurally. | Typed-state derivation | Exhaustive slot states and typed errors distinguish installing, idle, assigned, retired, stale completion, and unavailable-event cases. |
| 25 | Test drivers and fuzz targets could bypass the initialization contract by folding raw definitions. | Testability and typestate policy | Drivers consume concrete `B` definitions and activate them into `Trace<B>` containing `Active<B>`; fuzz targets compile against the same lifecycle boundary. |
| 26 | Event acceptance and nested routing were duplicated across seven optional lane traits. | Typed-protocol derivation | `EventInput<T>` is the total acceptance proof and one lossless `RouteInput<T>` contract returns an unowned payload unchanged. The seven lane-specific traits and their `Option<Self>` constructors are gone. |
| 27 | A supervisor could exist while storing a rejected fleet configuration. | Illegal-state discipline | `Supervisor::new` is fallible and a constructed `Supervisor` owns a valid `Fleet`; duplicate topology rejection occurs before behavior existence. |
| 28 | `Active<B>` exposed unrestricted mutable access to the wrapped behavior. | Lifecycle capability policy | `DerefMut` is removed. Mailbox folds remain available only through `transition`, `receive`, and `on`. |
| 29 | `Compose::build` discarded the definition typestate without initializing it. | Lifecycle capability policy | The container and escape hatch are removed; concrete definitions initialize only through the consuming activation boundary. |
| 30 | The foundational crate contained a mailbox driver tied to `bombay-communication`. | Interpreter-boundary policy | The concrete driver and transcript are removed. The core has no executor, transport, or other Bombay-crate dependency. |
| 31 | One-use wrapper action aliases unnecessarily expanded the public vocabulary. | Bombay API policy | Deadline, receive-timeout, watch, proxy, and supervisor action aliases are crate-private; the concrete named send products and `Actions` remain public. |
| 32 | `Compose::machine` duplicated ordinary construction and privileged one behavior implementation. | Bombay API policy | `Machine::new` constructs the concrete behavior directly; `Compose` is now only a blanket wrapper-composition extension trait and has no constructor. |
| 33 | Child creation silently derived nonces from collection indexes and saturated on conversion failure. | Freshness staging policy | The composition trait's `children` transformation requires an explicit index-to-nonce function. The caller owns the routing policy; no hidden collision-producing fallback exists. |
| 34 | Raw initialization and composition exposed competing production lifecycle entry points. | Lifecycle capability policy | `Activate::initialize` is the single ordinary consuming activation API for both standalone and wrapped concrete behaviors; `Active<B>` cannot activate again. Interpreter-facing primitives remain component-level. |
| 35 | Concrete semantic-wrapper constructors let callers bypass the single authoring path. | Bombay API policy | Deadline, receive-timeout, watch, stash, and shutdown wrappers have crate-private constructors and are authored through the `Compose` extension trait; their concrete types remain public so the static composed protocol stays visible. |
| 36 | Exact wrapper types appeared necessary at adapter boundaries. | Static naming policy | Adapter and spawn boundaries are generic over `B: Behavior`, so ordinary compositions remain inferred. Rare internal storage may use ordinary Rust aliases or newtypes; a second behavior macro was removed. |
| 37 | Supervisor and pool constructors encoded topology, restart policy, capacity, and interruption as eight or nine positional arguments. | Rust product-type discipline | `ChildTopology`, `RestartConfiguration`, and `PoolConfiguration` name the coexisting semantic facts. Constructors accept those products and reject invalid topology before a behavior exists. |
| 38 | Independent adapter authors had to reconstruct the Driver contract from several architecture documents and runtime tests. | Interpreter-boundary policy | `docs/adapter-contract.md` specifies activation, one-event folds, action commitment ordering, named lane interpretation, event injection, terminal handling, and conformance tests without introducing a second runtime abstraction. |
| 39 | `workers!` generated a block-scoped heterogeneous behavior sum and factory through a second macro language. | Static protocol policy | The macro remains removed. Heterogeneous worker protocols use an explicit exhaustive `Behavior` enum with `ChildTopology`. Heterogeneous root births instead use the narrower `#[behavior::births]` creation-only sum: it generates installation dispatch, never message forwarding. |

## Reviewed and deliberately retained

- `InitializationTurn` and `ActiveTurn` remain crate-minted lifecycle
  capabilities. Removing them would let manual `Behavior` implementations call
  lifecycle-specific folds without the typestate proof; `#[behavior]` hides
  their construction from ordinary authoring.
- `Proxy::new` remains public because a proxy is itself a concrete derived
  behavior, including the vacant/installation state that cannot be expressed as
  a transparent wrapper. `Supervisor::new` remains fallible because custom
  strategies and factories need a typed configuration boundary. Its function
  pointer factory remains static and non-capturing: no concrete catalogue use
  demonstrates that another factory generic would improve the semantic model.
- `<A as Address>::Nonce: From<u64>` remains required by proxy incarnation
  allocation, which derives distinct attempt nonces from a checked monotonic
  sequence. It is a concrete creation dependency, not an Environment service
  or convenience conversion.
- Reaction function pointers remain statically dispatched. Captured application
  state belongs in the concrete behavior value; making every reaction a new
  generic parameter would multiply wrapper and protocol types without adding a
  semantic capability.
- Worker-pool types and the other reusable actor implementations live in
  `bombay-behavior-actors`, which depends one-way on `bombay-behavior` and
  preserves their concrete static protocols. `PoolActions` remains the one
  named alias used across both pool variants.

## Static and verification evidence

- Zero `dyn Trait`, `Any`, `TypeId`, `unsafe`, registry, erased future, or
  untyped global envelope exists in `crates/behavior/src` or
  `crates/actors/src`.
- `cargo nextest run --workspace`: 283/283 passed.
- `cargo test -p bombay-behavior --doc`: 10 passed, including lifecycle
  compile-fail proofs.
- All ten fuzz targets compile. The pinned shell does not install `cargo-fuzz`,
  so no timed libFuzzer campaign was claimed.
- `nix flake check`: all seven aarch64-darwin checks passed.
- `research/architecture-critical-review-loop/check.sh`: 67/67 obligations,
  53/53 capability rows, static-escape counts all zero, and authoritative gates
  passed.
