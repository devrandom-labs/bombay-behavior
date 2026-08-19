# Template capability ownership

This audit records which layer truthfully owns every capability exposed by a
reusable actor template. It is normative for additions to
`bombay-behavior-actors`.

## Law

A concrete template declares the creation and send capabilities required by
its own transition law. A transparent wrapper preserves the inner lanes and
adds only the lanes owned by the wrapper. A composition may require an inner
capability only when an inner initialization or mailbox transition can
actually emit that capability.

An inert behavior whose only purpose is to make an associated capability type
available is not a valid composition component. Such a witness assigns
semantic ownership to a fold that does not own the operation and makes the
public construction depend on meaningless application code.

This is an Actors-layer API law. Actor research requires fresh allocation for
actor creation and distinguishes it from behavior replacement, but does not
prescribe this Rust ownership decomposition.

## Audit

| Family | Capability result | Ownership finding |
|---|---|---|
| `Machine` and concrete routing, discovery, workflow, persistence, and operations templates | Their declared named sends and `NoBirths` | Sound: each standalone fold owns its protocol and products. |
| Timing, shutdown, stash, watch, activation, guardian-root, termination-monitor, and shutdown-coordinator wrappers | Added named sends with the inner birth lane preserved | Sound: these are structural transformations and do not fabricate authority. |
| `Proxy` | `Births<C>` for fresh worker incarnations | Sound: the stable proxy owns incarnation installation and explicit replacement provenance. |
| `DynamicSupervisor` | `Births<DynamicProxy<C>>` | Sound: the template owns admission, stable routes, installation, replacement, and retirement. |
| `Supervisor<A, C>` | `Births<Proxy<C>>` | Sound: the standalone template owns fixed topology, proxy creation, observation, replacement and draining directly. |
| `Supervise<B, C>` | `Births<Proxy<C>>` plus a real inner `Births<C>` lane | Sound as a composition: the shared ownership core owns configured topology while `B` is required only for additional child creations that a real inner transition may emit. Use `Supervisor` when no such application behavior exists. |
| `BackoffSupervisor<A, C>` / `BackoffSupervise<B, C>` | The corresponding supervisor birth lane plus timer schedules | Sound: both adapters share one pending-delay state machine; standalone use has no inner capability witness. |
| `WorkerPool` and `KeyedWorkerPool` | Supervised proxy births | Sound: both compose the shared ownership core directly; the former inert `PoolKernel` has been removed. |

## Required construction

Fixed-fleet ownership is one state machine shared by:

- a standalone fixed supervisor with its own nominal protocol;
- supervision composed around an application behavior that can independently
  create additional supervised children;
- generation-safe backoff over either form; and
- fixed and keyed pools.

The ownership core retains configured routes, proxy-installation phases,
restart budget, replacement provenance, shutdown-drain state, and factory. It
produces the existing `SupervisorSends` and proxy birth product. Application
events and sends are a separate composition dimension, not the source of the
core's creation authority.

## Review rule

Every new template or wrapper must answer all of the following in its rustdoc
and tests:

1. Which fold owns each send and birth lane?
2. Can each required inner capability be emitted by a real inner transition?
3. Does initialization preserve the documented ordering of inner and wrapper
   effects?
4. Can standalone use be expressed without an inert behavior, sentinel
   protocol, erased type, or runtime registry?
5. Do all wrapper orders preserve every lane without positional knowledge in
   application code?
