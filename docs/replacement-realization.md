# Replacement realization

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
