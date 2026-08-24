# Actor-template composition audit

## Scope and decision rule

This audit covers the complete `bombay-behavior-actors` catalogue as it exists,
not one commit or one pair of reported gaps. Git history is used only to identify
when a public name or implementation shape appeared.

A reusable actor template is retained only when its own fold implements a
distinct state-transition law or a distinct typed event/effect transformation.
Sharing data structures or similar branches is not evidence that two actors are
one template. Conversely, a constructor, alias, policy marker, or wrapper is not
a template merely because it gives a long concrete composition a shorter name.

The governing semantic classification is Bombay policy. The actor-model laws
remain serialized turns, finite acquaintance, fresh creation, and explicit
communications/creations/next behavior. This audit does not change those laws.
It changes how the library packages derived constructions around them.

## Pre-edit change ledger

Exact blocker: the existing H37 audit treats “uses one shared implementation”
as the success criterion. That criterion caused public policy markers, aliases,
builders, and wrappers to be counted as actor templates even when they only
select or nest existing folds. The smallest end-to-end regressions are the
existing pure-fold constructions that already exercise `StopOnShutdown`,
`FinalizeOnShutdown`, fixed supervision, delayed supervision, both pool laws,
both shutdown-coordinator products, and `Configuration<FeatureSet<_>>` without
requiring a second semantic template. Those constructions will remain and the
bespoke names will be removed around them.

Expected surface:

- 26 files directly name the redundant public surface; separating the pool,
  watch/monitor, and shutdown folds and updating exhaustive callers is expected
  to take the cumulative change above 15 files.
- Production delta is expected to be net-negative, primarily from deleting the
  child-shutdown planner, policy-parameterized fixed-supervisor forms, recipe
  forwarders, and name-only lifecycle/feature specializations.
- No new public type is expected. At least 20 public types/aliases/traits and
  seven forwarding functions are expected to be removed.
- The retained folds are `StopOnShutdown`, `FinalizeOnShutdown`, `Watch`,
  `TerminationMonitor`, `Supervisor`, `BackoffSupervisor`, `WorkerPool`,
  `KeyedWorkerPool`, `ShutdownCoordinator`, and
  `HeterogeneousShutdownCoordinator`. Their existing typed products and
  interpreter requests are reused.

## Complete catalogue classification

“Core” means a standalone actor fold. “Transformation” means a wrapper that
owns a distinct typed event/effect law. “Composition” means the public use case
must be written by nesting or connecting retained concrete actors. A truthful
structural alias such as a direct `Here` form may remain when it names the same
template rather than advertising another actor law.

| Catalogue surface | Owned transition law | Verdict |
|---|---|---|
| `Machine` | finite-state receive/become transition and staged drain atomicity | retain core |
| `MessageAdapterWithRoute` | one pure protocol map followed by one typed delivery | retain core |
| `Stash` | bounded hold/replay order over an inner fold | retain transformation |
| `StopOnShutdown` | direct shutdown-to-stop event transformation | retain transformation |
| `FinalizeOnShutdown` | shutdown reaction returning complete inner `Actions` | retain transformation |
| `Guardian`, `CoordinatedGuardian` | no state beyond the two shutdown transformations above | delete; compose the retained transformations directly |
| `Watch` | recurring logical-name observation across later incarnations | retain as its own concrete transformation |
| `TerminationMonitor`, exact monitor form | one correlated observation lifecycle with terminal consumption | retain one monitor implementation with typed target form |
| `EstablishedWatch` and watch target policy aliases | restrict or parameterize the monitor reaction without another lifecycle law | delete; use the exact monitor and an ordinary reaction |
| `PropagateTermination` | one correlated source fact and explicit propagation disposition | retain transformation |
| `Task` | pending-to-completed/failed one-result terminal lifecycle | retain core |
| `ShutdownCoordinator` | ordered homogeneous phase shutdown | retain transformation |
| `HeterogeneousShutdownCoordinator` | ordered phase shutdown over a closed heterogeneous effect product | retain separately; its typed selection/effect transition is distinct |
| `ChildShutdownPlan`, its typestate builder, and `shutdown_after_children` | joins committed direct-child creations, rejects mismatched facts, and reports one typed heterogeneous plan | retain transformation; generic consumers use associated-output operation traits instead of copying its hidden proof vocabulary |
| `ProxyWithParent` | stable slot and fresh-incarnation replacement lifecycle | retain core |
| `SuperviseWithParent` | adopt application creations into fixed proxy ownership while preserving inner actions | retain transformation |
| `SupervisorWithParent` | standalone fixed proxy-fleet ownership | retain core, with a concrete implementation rather than a command-policy engine |
| `SupervisedWorkers` and selector policy types | parameterize fixed ownership with one routing selector | delete; route commands with an ordinary typed routing actor/application behavior |
| `BackoffSuperviseWithParent` | delays accepted replacement effects from a supervised application | retain transformation |
| `BackoffSupervisorWithParent` | delays accepted replacement effects from standalone fixed ownership | retain transformation |
| `FixedBackoff`, `BackoffWorkers` | implementation/policy parameterizations of the two retained delayed folds | delete |
| `DynamicSupervisorWithParent` | changing stable-child membership and replacement lifecycle | retain core |
| `WorkerPoolWithParent` | bounded FIFO admission, assignment, completion, and interruption | retain core and its direct `Behavior` fold |
| `KeyedWorkerPoolWithParent` | the FIFO law plus persistent key-to-slot affinity and rebalance transitions | retain separately and restore its direct `Behavior` fold |
| `Deadline` | absolute one-shot schedule and single matching-generation reaction | retain transformation |
| `OneShot` | relative one-shot schedule and single matching-generation reaction | retain transformation |
| `Periodic` | matching-generation reaction followed by explicit rearm | retain transformation |
| `ReceiveTimeout` | activity-driven generation reset and one notification per idle period | retain transformation |
| `Lease` | exclusive ownership, generation-safe expiry, release, and rejection | retain core |
| `Router` with its strategies | membership plus strategy-specific selection/evidence transitions | retain core; strategies are values of this law, not actors |
| `WorkQueue` | bounded FIFO work plus worker-availability transitions | retain core |
| `Buffer` | bounded FIFO buffering and overflow ownership policy | retain core |
| `PriorityQueue` | stable immutable-priority admission/release | retain core |
| `OrderGate` | monotonic keyed release | retain core |
| `Sequencer` | sequence-gap buffering and ordered release | retain core |
| `Deduplicator` | bounded first-seen admission | retain core |
| `RateLimiter` | explicit token consumption/refill | retain core |
| `CircuitBreaker` | closed/open/probing single-flight admission | retain core |
| `Correlator` | keyed request/result ownership | retain core |
| `Acknowledgements` | multi-participant acknowledgement lifecycle | retain core |
| `Registry` | mutable typed binding ownership and lookup | retain core |
| `Resolver` | immutable definition and read-only resolution capability | retain core; mutation is unrepresentable in its protocol |
| `Topic` | one ordered subscription set and snapshot publication | retain core |
| `PubSub` | keyed topic introduction, known-empty retention, and per-topic membership | retain core; these keyed states are not actor nesting hidden in a wrapper |
| `Presence` | versioned presence evidence and generation-safe expiry | retain core |
| `Configuration` | versioned atomic configuration acceptance/query | retain core |
| `FeatureSet` | a domain product invariant, not an actor | retain product |
| `Features`, `FeaturesState` | name-only aliases of `Configuration<FeatureSet<_>>` | delete; write that concrete composition |
| `Health` | versioned component evidence and aggregate health | retain core |
| `Readiness` | fixed dependency evidence and aggregate readiness | retain core |
| `Cache` | bounded deterministic LRU ownership | retain core |
| `Latch` | one-generation countdown and single release | retain core |
| `Barrier` | cyclic fixed-membership generations | retain core |
| `Workflow` | dependency activation and terminal run lifecycle | retain core |
| composition recipe functions | forward to constructors while inferring structural parameters | delete; construct and nest the concrete types directly |

## Composition law for the removals

The replacements must preserve complete `Actions`; they may not intercept,
drop, duplicate, reorder, or reinterpret an effect lane. Actor-to-actor
composition uses ordinary typed recipients and sends. Wrapper composition uses
the retained concrete event layers and named send products. A topology owner,
not a generic planner wrapper, remains responsible for correlating its own
creation results and reporting any creation-dependent shutdown plan. These are
derived Bombay constructions and policy choices, not additional Agha laws.

## Implementation checkpoint

The cumulative catalogue rewrite remains production-negative even though the
retained pool and shutdown actors now expose their own folds directly. These
figures include all tracked and untracked files; documentation is reported
separately:

```text
production: +1204 / -2117 / net -913
tests:      +1366 / -581 / net +785
docs:       +331 / -175 / net +156
public API: +5 types / -19 types
```

Seven forwarding functions are also removed. The three apparent public type
additions in the textual diff replace existing aliases with concrete structs
of the same name (`Watch`, `SupervisorWithParent`, and
`BackoffSupervisorWithParent`); they add no public type name. The five additions
are `ProxyUnavailable`, `CommandSupervisionEvent`, `DeclareShutdownPhase`,
`FinishShutdownPhases`, and `LogicalHostRequirements`. Eight constructors or
methods were also removed, for 34 removed public names in total. No public
wrapper or policy-marker type was added.

The later semantic regressions retain this classification while adding no
actor wrapper. Dynamic supervision now joins its two initial creation facts in
either order. Proxy commands carry an explicit typed logical unavailability
recipient, and pools join that return with worker-stop/replacement facts.
`DeclareShutdownPhase` and `FinishShutdownPhases` expose the retained builder to
generic consumers through associated outputs. `LogicalHostRequirements` is an
owner-authored, duplicate-preserving closed product of all transitive logical
destinations; exact-only endpoints remain absent.

## Verification

- `cargo check --workspace --all-targets`: pass without Rust warnings.
- `cargo nextest run --workspace --no-fail-fast`: 513 passed, none skipped.
  Nextest marked one macro parser test leaky but returned success.
- Actor rustdoc and compile-fail tests: 24 passed.
- `supervision_sequences`, `pool_sequences`, and
  `shutdown_plan_sequences`: 5,000 fuzz executions each under the locked Fenix
  input's nightly sanitizer toolchain.
- `nix flake check`: passes all seven checks, including build, optimized
  nextest, docs, Rust/TOML formatting, dependency audit, and dependency policy.
