# Distilled Architecture

This document records the architecture after the feature-complete distillation
of the behavior algebra. It separates accepted structure from deliberately
rejected or deferred abstractions. Future changes must still state their laws,
use concrete public types, analyze composition, and pass the repository's full
verification standard.

## Semantic floor

`Behavior` is the pure transition boundary. One typed event produces exactly
`Actions`: concrete sends, staged requests for fresh child creation, and the
next behavior or termination verdict. The trait does not expose substitute
`Effect` or `Done` associated types; implementations cannot escape the public
algebra while still claiming to implement `Behavior`.

This is Bombay's typed realization of actor transition effects, not a literal
transcription of Agha's surface syntax. In particular:

- `Create` packages fresh allocation with initialization;
- a creator-local nonce lets the fold refer to a requested child before the
  interpreter allocates and installs it;
- the interpreter commits all accepted creations before dependent sends; and
- termination, initialization effects, typed protocol products, and
  replacement provenance are explicit Bombay constructions or policies.

The nonce is not actor identity or proof of freshness. A creation becomes a
birth only after an interpreter establishes a fresh actor and commits the local
binding. `CreationKind::ReplacementIncarnation` records Behavior's semantic
designation; it does not replace an actor at an existing core address.

## Module structure

The crate has three dependency layers:

```text
core algebra
  behavior, verdict, exit

neutral protocol vocabulary
  protocol

concrete transformations
  deadlined, watching, supervising, shutdown, stashing, fsm, spec
```

`protocol` owns values and construction capabilities for interpreter-originated
lanes: time, peer termination, child termination, worker termination, and
shutdown. It owns no fold or wrapper. Concrete transformations own the closed
event sums that add those lanes and implement forwarding against the neutral
vocabulary.

This removes dependencies such as timing importing supervision solely to name
a forwarded event capability. It does not introduce a global envelope,
registry, `dyn Trait`, `Any`, runtime lookup, or erased dispatch. Every composed
protocol remains a concrete nested sum visible to the compiler.

### Composition law

> Wrapping a behavior must not silently remove, duplicate, reorder, relabel, or
> reinterpret any event or effect lane the wrapper claims to forward.

Wrapper order remains part of the concrete type. Tests must cover every
supported ordering and mixed-lane trace.

## Creation and lifecycle provenance

Initial birth, ordinary later birth, and replacement incarnation all request a
fresh actor. Mechanics alone cannot distinguish them. The causal law is:

```text
Behavior designation
    -> typed Create
    -> successful interpreter installation
    -> truthful lifecycle fact
```

A replacement request or supervision decision is not a completed restart.
An interpreter may report `Restarted` only after installing a
`ReplacementIncarnation`. Failed installation reports neither a successful
birth nor restart. Address, nonce shape, prior occupancy, and actor history are
not substitutes for the designation.

Concrete wrappers must preserve `CreationKind` when transforming a child type.
The stable supervision proxy marks its initial worker as `Birth` and every
successor emitted by `ProxyCommand::Replace` as `ReplacementIncarnation`,
whether creation is immediate or deferred until the old incarnation stops.

## Shutdown policy

Shutdown remains an ordinary typed input and behavior transition, not a fourth
actor-model effect. `FinalizeOnShutdown` retains the complete final `Actions`,
including creations, then forces `Stop(Normal)`. Narrowing that result without
a concrete use would add a policy restriction unsupported by the actor
algebra, so the current full result is retained.

Ingress closure, mailbox draining, request priority, cancellation, and handle
ownership remain interpreter concerns.

## Deliberately retained concrete structures

- `SendProduct` remains a concrete nested product. A generalized heterogeneous
  collection would erase lane types or require speculative machinery.
- Event wrappers remain concrete nested sums. A universal event envelope would
  violate static protocol composition.
- Function pointers remain the representation for non-capturing reactions.
  There is no demonstrated law requiring policy traits or erased closures.
- `Spec` remains a thin typestate composition facade because every method
  immediately constructs its concrete wrapper; it stores no runtime intent
  graph.
- `BirthMode`, `NoBirths`, and `Births<C>` remain because absence of creation is
  a compile-time capability, not an empty runtime convention.
- Stable restart remains proxy-derived. Reusing or overwriting an existing core
  actor address is not introduced as a primitive.

## Remaining boundary work

Some properties cross into an interpreter and cannot be completed inside this
crate:

- accepted creation must validate freshness and creator-local nonce binding;
- collision or installation failure needs a typed interpreter error and must
  never overwrite an incarnation;
- lifecycle diagnostics must be emitted only after the installation commit;
  and
- runtime identity must remain distinct from the nonce used by Behavior for
  local routing.

Behavior exposes the typed information needed for those laws but neither
performs installation nor reports runtime lifecycle completion.

Within this crate, resource-exhaustion panics and impossible interpreter-event
states must remain documented and tested. A panic must not stand in for a
controlled behavior error or a state that can be represented statically.

## Rejected distillation directions

The following changes were considered and rejected because they weaken the
algebra or add structure without a demonstrated law:

- trait objects or boxed erased futures for wrapper uniformity;
- a global event/effect enum shared by every behavior;
- runtime protocol registries or capability discovery;
- serialization between internal protocol layers;
- a second replacement effect lane separate from creation;
- inferring replacement from generation counters or child tables;
- literal allocator callbacks or continuations inside the pure fold; and
- splitting modules solely to reduce line counts without improving ownership.
