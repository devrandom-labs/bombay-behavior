# Replacement realization

Fresh incarnations may change behavior state or implementation without
changing a stable proxy's public protocol. This depends on the separation in
[Protocol, ingress, behavior, and effect algebras](protocol-algebra.md).

## Semantic classification

The actor-model law used here is fresh allocation. In Agha's actor semantics,
`newadr` chooses a fresh address, `initbeh` installs an initial behavior, and
`become` replaces the behavior of the actor processing the communication.
Fresh actor creation and behavior replacement are distinct operations. See
Agha et al., *A Foundation for Actor Computation*, section 3:
<https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf>.

Bombay's creator-local nonce, stable proxy construction, creation-before-send
interpretation order, and creation-result lane are derived constructions and
project policies. They are not guarantees made by the actor model.

Akka illustrates a different deliberate policy: restart replaces the behavior
instance behind the same actor reference and is normally invisible to external
lifecycle monitoring. That comparison supports keeping restart realization in
the supervision protocol rather than redefining exact-incarnation termination:
<https://doc.akka.io/libraries/akka-core/current/general/supervision.html>.

## Transition law

A replacement has three distinct phases:

1. Behavior emits a fresh `Create` whose provenance names the exact prior child
   nonce.
2. The interpreter attempts fresh allocation, initialization, and binding.
3. The interpreter reports `Installed` or a typed rejection through the
   requested creation-result lane.

A proxy may route to the successor only after step 3 succeeds. A mismatched or
stale result changes no state. A rejected creation binds no child and cannot
produce a successful restart report.

`ChildStopped` remains the terminal observation of one exact incarnation.
`CreationResolved` reports realization of one staged creation. Neither event
implies that supervision will or will not make a later attempt.

## Replacement-facing observation

The stable proxy reports `WorkerStopped` and `WorkerCreationResolved` to its
parent as separate typed facts. This separation is intentional:

- `WorkerStopped` owns the exact prior worker's terminal outcome and time;
- `WorkerCreationResolved` owns the fresh attempt, its explicit
  `CreationKind`, and the interpreter's installation result.

For `CreationKind::ReplacementIncarnation`, the kind names the exact prior
incarnation and the creation report names the fresh attempt. Therefore the
report already distinguishes successful fresh replacement, the incarnation it
replaces, and rejected replacement without address arithmetic, timing, or
allocation inference. `WorkerCreationResolved::into_replacement` exposes this
as the closed `ReplacementResolution::{Installed, Rejected}` sum.

`ReplacementResolution` is a derived projection, not a new interpreter event.
It deliberately does not copy the old worker's `Exit`/`Crash`: a consumer that
needs both facts retains the earlier `WorkerStopped` and correlates it with the
explicit `replaced` nonce. This prevents a combined convenience event from
duplicating or reinterpreting terminal provenance and requires no Bombay runtime
registry, lease, or additional observation lane.

## Rejected same-action observation

Creation resolution precedes service-send interpretation by Bombay policy. If
a creation is rejected, no child generation is installed at its nonce. A
same-action `ObserveChild` for that nonce is therefore consumed as inert: it
installs no observation, emits no `ChildStopped`, and is not retained for a
later creation. The same-action `ObserveCreation` still emits exactly one
rejected `CreationResolved`.

This is not an actor-model lifecycle guarantee. It is Bombay's interpretation
law preserving the distinction between a rejected creation request and the
termination of an actor that was actually installed.
