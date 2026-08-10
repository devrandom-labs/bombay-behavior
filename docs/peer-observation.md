# Peer observation

Peer observation is a derived Bombay protocol, not one of the actor-model
transition primitives. Agha's actor semantics supplies communication to known
addresses, fresh actor creation, and behavior replacement; an interpreter's
ability to retain and report lifecycle facts is an additional construction.
See Agha et al., *A Foundation for Actor Computation*, section 3:
<https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf>.

## Request and result law

`ObservePeer { peer }` is the pure request. `PeerStopped { peer, outcome }` is
its typed result. The interpreter selects the exact peer incarnation denoted
when it interprets the request:

- if that incarnation is live, `PeerStopped` is delivered if it later
  terminates;
- if authoritative retained terminal history identifies that incarnation,
  the same `PeerStopped` result may be delivered immediately.

No additional registration acknowledgement is required by behavior. Successful
selection of a live incarnation changes no behavior state; only its eventual
terminal observation is a semantic input to `Watch`.

Absence from a live-address table is not proof of termination. An interpreter
that can select neither a live incarnation nor authoritative retained terminal
history must return an interpreter error and must not fabricate `PeerStopped`.
Actorpass currently has no tombstone authority, so it implements only the live
selection leg.

Behaviorpass exposes only the peer address and normalized outcome. Exact
incarnation handles, generations, tombstone retention, and lookup are runtime
capabilities and do not enter the pure behavior algebra.

## Cancellation law

`UnwatchPeer { peer }` asks the local interpreter to cancel this actor's
current peer-observation relationship. Cancellation is idempotent, affects no
other observer, does not terminate the peer, and does not affect a later
`ObservePeer` request. It cannot retract `PeerStopped` already admitted to the
actor's mailbox.

Behaviorpass deliberately provides only this pure request vocabulary. Runtime
monitor ownership and cancellation remain interpreter concerns; a higher-level
dynamic watching transformation should be added only with a concrete behavior
whose transition laws demonstrate the need.
