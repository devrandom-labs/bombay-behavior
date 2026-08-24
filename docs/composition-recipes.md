# Actor compositions

`bombay-behavior-actors` provides reusable actor arrangements where several
existing laws have to be connected in one exact order. They remain pure
behaviors: receiving one typed event produces typed `Actions` and the next
behavior decision. Scheduling, child creation, observation, and delivery are
still performed only by an interpreter.

These arrangements are derived Bombay policy, not laws of the actor model.
Their policies are explicit at construction and their failures remain typed.

## Supervised workers with restart delay

Use `supervised_backoff` when callers should send a worker command without
knowing about the stable proxy used to keep a worker slot constant:

```rust,ignore
let payments = supervised_backoff(
    ChildTopology::new([CARD_PAYMENTS, BANK_PAYMENTS], payment_worker),
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        3,
        Duration::from_secs(30),
    ),
    Backoff::exponential(Duration::from_millis(100), Duration::from_secs(5))?,
    |command: &PaymentCommand| command.payment_worker(),
)?;
```

The public message is `PaymentCommand`. The selector is the application policy
that chooses a configured worker nonce; the library does not invent round
robin or any other default. A successful command is delivered through the
creator-owned stable child route. Worker replacement changes the incarnation
behind that route, not the route selected by the command.

An unavailable command returns
`SupervisedWorkersError::CommandNotAccepted`. That value owns the selected
worker, the availability reason, and the complete original command together
with its sender. The supervisor does not drop, stash, redirect, or rebuild the
command. Unknown, starting, restarting, retired, and shutting-down workers use
the same law. Restart decisions, budget failure reports, replacement delay,
and coordinated shutdown use the same ownership and backoff folds as the
ownership-only supervisor. Restart delay applies only to replacement commands.

Topology construction can return `FleetError<A::Nonce>`. Backoff validation
returns `BackoffConfigError` before composition, while delay calculation can
return `BackoffError` during a transition. Keeping those stages separate makes
their recovery choices explicit.

`Supervise<B, C>` remains the right form when an existing application behavior
creates and adopts children through its own `Births<C>` lane. It already keeps
the application's public protocol. The fixed composition above owns the whole
configured worker set and therefore can safely expose direct worker commands.

## Shutdown phases from committed children

Use `shutdown_after_children` when an application's direct child roles define
its shutdown order:

```rust,ignore
let application = shutdown_after_children(Application::new(root))
    .shutdown_phase(ApplicationChild::Store)
    .shutdown_phase(ApplicationChild::Gateway)
    .finish();
```

Each phase names a role already declared by the application's child product.
The compiler rejects a duplicate role, a role from another application, a
value of the wrong type, or `finish` while a role is missing. Reversing the two
calls reverses the observable child termination order.

The composition observes every configured direct child created by application
initialization. Later application-created children remain ordinary application
effects and are not silently added to the declared shutdown plan. It records a
route only after that configured creation commits, preserves a rejected creation
as `ChildShutdownPlanError::CreationRejected`, and reports the completed
heterogeneous plan through ordinary `Actions`. The existing
`HeterogeneousShutdownCoordinator` installs and executes that plan. A shutdown
request received first is retained by the coordinator and starts immediately
when the plan arrives; a plan received first waits for shutdown. Both cases run
the same declared phases.

The report destination is inferred from the final composed actor type. It is
not fixed when `finish()` is called. The same spelling therefore remains valid
when a coordinated guardian or another statically typed outer composition is
added later. Framework and application code do not supply a structural path.
Unexpected, mismatched, rejected, duplicate, and post-plan creation facts are
never silently consumed; typed failures retain the complete authoritative
fact.

Application code does not construct event products, send products, structural
paths, child occurrences, or predicted addresses. The composition does not
reconstruct an address from a nonce and does not add a registry, callback,
runtime type, or another shutdown machine.

The lower-level `HeterogeneousShutdownPlan`, `shutdown_target`, and
`ReportShutdownPlan` APIs remain available for framework code that builds a
plan from a different statically known source. They are not needed for the
ordinary direct-child case.

## Actor-template audit

The two additions prompted a full pass over the reusable actor templates. Four
duplicated implementations were collapsed:

- `Guardian<B>` is now the existing direct `StopOnShutdown<B>` composition;
  only coordinated guardians retain a distinct behavior because they delegate
  shutdown instead of stopping themselves.
- Ownership-only fixed supervision and application-facing fixed supervision
  share one fleet state machine. A static command policy changes only the
  public protocol and command transition.
- Ownership-only and application-facing fixed restart delay share one backoff
  implementation. Their timer source differs, while delay, generation,
  cancellation, and release laws are identical.
- Homogeneous and heterogeneous shutdown coordinators share one ordered-phase
  state transition. Their concrete request products remain different because
  static child protocols differ.

The remaining actor compositions were checked against their state and effect
laws. They are intentionally distinct:

- `Deadline` owns an absolute optional deadline and may only change the next
  behavior decision; `OneShot` owns a relative one-time reaction with full
  actions; `Periodic` rearms after each matching generation; and
  `ReceiveTimeout` rearms only after successful application activity.
- `Watch` exposes a typed reaction for watched lifecycle facts, while
  `TerminationMonitor` owns exact-once terminal observation state.
  `PropagateTermination` additionally publishes or discharges a terminal
  result, so merging these would combine different capabilities.
- `Stash` owns FIFO replay state and is not a generic event wrapper.
- `Supervise<B, C>` adopts births produced by an inner behavior, fixed
  supervision owns a configured topology, dynamic supervision owns management
  commands, and worker pools additionally own job assignment. Those ownership
  laws are not interchangeable.

Observation and shutdown use their concrete compositions directly. Redundant
aliases for identical state machines were removed. Catalogue actors are
concrete protocols and were not treated as wrappers.

This is the stopping rule for the cleanup: share an implementation when the
state transition and effect order are the same; keep separate types when
combining them would weaken a protocol, mix ownership, or add a runtime choice.
