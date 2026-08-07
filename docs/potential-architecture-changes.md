# Potential Architecture Changes

This document records architectural pressure visible in the current behavior
algebra. It is not an accepted design change or a reason to weaken the current
static protocol guarantees. Any proposal listed here still requires a concrete
use, stated laws, public types, composition analysis, and the repository's full
verification standard.

## Separate protocol lanes from concrete wrappers

The behavior crate currently places each event-lane trait beside the concrete
wrapper that introduces that lane. For example, timing owns `TimeEvent`, peer
watching owns `PeerEvent`, supervision owns `ChildEvent` and `WorkerEvent`, and
shutdown owns `ShutdownEvent`.

This is locally coherent: a feature's event value, construction capability,
and fold live together. It also keeps every accepted protocol concrete and
statically known. As more lanes compose, however, each event sum must forward
the capabilities introduced by the other lanes. This creates peer-module
dependencies such as:

```text
deadlined   <-> shutdown
watching    <-> shutdown
supervising <-> shutdown
```

The same pressure already exists among timing, watching, and supervision.
Shutdown makes the growth more visible but does not originate it. The number
of forwarding implementations can approach the square of the number of event
lanes.

A possible future organization is:

```text
core algebra
  Behavior, Actions, Step, Exit

protocol-lane vocabulary
  user, time, peer, child, worker, shutdown

concrete transformations
  Base, At, Watching, Supervising, Stashing, StopOnShutdown
```

Under that organization, lane traits and their observation values would live
in a neutral protocol module. Concrete transformations would depend on that
vocabulary rather than on one another's feature modules. The public protocols
would remain closed generic sums and products; this is not an argument for a
global envelope, registry, `dyn Trait`, `Any`, or runtime capability lookup.

### Required law

Any reorganization must preserve the following composition law:

> Wrapping a behavior must not silently remove, duplicate, reorder, or
> reinterpret any event or effect lane the wrapper claims to forward.

Moving declarations alone is not valuable. A refactor belongs only if it
reduces dependency or implementation complexity while retaining the same
concrete public types and exhaustive routing behavior.

### When to reconsider

Do not perform this refactor solely for shutdown. Reconsider it when at least
one of these conditions holds:

- another independent event lane would require broad pairwise forwarding;
- wrapper permutations become materially difficult to inspect or test;
- peer-module dependency cycles obstruct otherwise truthful module boundaries;
- a concrete shared composition mechanism can be expressed without erasure or
  runtime dispatch.

Until then, explicit forwarding is verbose but honest: the compiler can see
the complete composed protocol, and tests can verify every supported wrapper
ordering.

## Shutdown API shape

The shutdown module currently follows the same feature-module pattern as the
other transformations:

```text
input value        ShutdownRequested
composed protocol  ShutdownProtocol<E>
capability trait   ShutdownEvent
stop policy        StopOnShutdown<B>
final-fold policy  FinalizeOnShutdown<B>
composition API    Spec::{stop_on_shutdown, finalize_on_shutdown}
```

This is a derived Bombay protocol, not a fourth primitive actor effect.
Shutdown transitions still return exactly `Actions`: sends, fresh creations,
and become/stop. Ingress closure, mailbox draining, request priority, handle
ownership, and cancellation remain interpreter concerns.

`ShutdownReaction<B>` deliberately remains a concrete function type rather
than a new policy trait. A trait should be introduced only if multiple concrete
uses demonstrate laws or composition needs that the function type cannot
express cleanly.

One policy question remains worth evaluating with concrete applications:
whether a final shutdown fold should retain the full `Actions` algebra,
including fresh creations, or expose a narrower statically typed result. The
actor algebra permits creation during a transition; forbidding it would be a
Bombay shutdown policy and should be encoded in the return type rather than as
a runtime check or undocumented convention.
