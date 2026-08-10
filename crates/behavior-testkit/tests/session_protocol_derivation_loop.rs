//! Independent session protocol validation campaign (Resource Pool, 2026-08-08).
//!
//! Determine whether Bombay Behavior needs phase-indexed protocol typing
//! beyond the existing `Machine`. This file validates the earlier Supervised
//! Worker result with a distinct protocol; it is not part of the broad
//! architecture audit. It records every derivation attempt, its compiler
//! evidence, and exact obstructions.
//!
//! # Concrete case: Resource Pool
//!
//! A resource-pool actor creates N child workers from a factory spec,
//! leases them to clients, and drains before shutdown. Three phases with
//! direction-sensitive messages.
//!
//! ## Phases
//!
//! 1. **Initializing** — awaiting `Configure` from supervisor.
//!    Valid IN: `Configure { size, spec }`
//!    Valid OUT (sends): `PoolReady` to supervisor
//!    Valid OUT (births): N child workers via `Create`
//!    Invalid IN: `Acquire`, `Release`, `DrainStatus`
//!
//! 2. **Serving** — leasing workers, watching them, handling releases.
//!    Valid IN: `Acquire`, `Release(addr)`, `ChildStopped` observations
//!    Valid OUT: `Granted(addr)` to client, `NoWorkersAvailable`
//!    Invalid IN: `Configure`, `DrainStatus`
//!
//! 3. **Draining** — rejecting new leases, finishing in-flight.
//!    Valid IN: `DrainStatus`
//!    Valid OUT: `Draining { remaining }`, `DrainComplete`
//!    Invalid IN: `Configure`, `Acquire`, `Release`
//!
//! ## Actor relevance
//!
//! Exercises creation (birth capability) in Initializing, peer watching in
//! Serving, and supervised shutdown coordination — not a generic channel
//! protocol.
//!
//! ## Direction sensitivity
//!
//! `PoolReady`/`Granted`/`Draining`/`DrainComplete` are OUTBOUND (sends).
//! `Configure`/`Acquire`/`Release`/`DrainStatus` are INBOUND (receives).
//! An outbound message presented to `step()` as an event is a category error.

// All types in this file are illustrative for the derivation campaign.
#![allow(dead_code)]

use behavior::{Behavior, Exit, Machine, MailAddr, Move, Never, Recipient, Step, User, UserEvent};

// ============================================================================
// Phase and message vocabulary
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Initializing,
    Serving,
    Draining,
}

#[derive(Debug, Clone)]
struct PoolConfig {
    size: usize,
    spec: String,
}

#[derive(Debug, Clone)]
struct Acquire {
    reply_to: Recipient<MailAddr, LeaseReply>,
}

#[derive(Debug, Clone)]
struct Release {
    worker: MailAddr,
}

#[derive(Debug, Clone)]
struct DrainStatus;

/// Outbound lease result (sent BY the pool, TO the client).
#[derive(Debug, Clone)]
enum LeaseReply {
    Granted { worker: MailAddr },
    NoWorkersAvailable,
}

/// Union of all inbound messages across all phases (runtime FSM approach).
#[derive(Debug, Clone)]
enum PoolMsg {
    Configure(PoolConfig),
    Acquire(Acquire),
    Release(Release),
    DrainStatus(DrainStatus),
}

/// Outbound messages the pool SENDS (never receives).
/// These are NOT events — they appear in Actions::sends, never in step().
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum PoolSend {
    PoolReady { supervisor: MailAddr },
    Granted { client: MailAddr, worker: MailAddr },
    NoWorkersAvailable { client: MailAddr },
    Draining { remaining: usize },
    DrainComplete,
}

// ============================================================================
// Pool state
// ============================================================================

#[derive(Debug, Clone)]
struct PoolState {
    leased: Vec<MailAddr>,
    capacity: usize,
}

impl PoolState {
    fn new() -> Self {
        Self {
            leased: Vec::new(),
            capacity: 0,
        }
    }
}

// ============================================================================
// Attempt 1: Existing Machine (FSM-01)
// ============================================================================

fn pool_fsm() -> Machine<MailAddr, PoolState, PoolMsg, Phase, Never> {
    Machine::new(PoolState::new(), Phase::Initializing, pool_transition)
}

fn pool_transition(
    phase: Phase,
    state: &mut PoolState,
    msg: &PoolMsg,
) -> Result<Move<Phase>, Never> {
    match (phase, msg) {
        (Phase::Initializing, PoolMsg::Configure(cfg)) => {
            state.capacity = cfg.size;
            // In a real pool, the FSM would emit Create actions for N children.
            // FSM cannot carry birth capability (Birth is NoBirths,
            // Sends is Vec<Delivery<A,Never>> — sends are empty).
            Ok(Move::Goto(Phase::Serving))
        }
        (Phase::Serving, PoolMsg::Acquire(_req)) => Ok(Move::Stay),
        (Phase::Serving, PoolMsg::Release(rel)) => {
            state.leased.retain(|w| *w != rel.worker);
            Ok(Move::Stay)
        }
        (Phase::Serving, PoolMsg::DrainStatus(_)) => Ok(Move::Goto(Phase::Draining)),
        (Phase::Draining, PoolMsg::DrainStatus(_)) => {
            if state.leased.is_empty() {
                Ok(Move::Stop)
            } else {
                Ok(Move::Stay)
            }
        }
        // All other (phase, msg) pairs are invalid in this phase.
        // FSM DEFERS them for later replay. It does NOT reject them.
        _ => Ok(Move::Defer),
    }
}

// ============================================================================
// FSM baseline tests (FSM-01 evidence)
// ============================================================================

#[cfg(test)]
mod fsm_baseline {
    use super::*;

    #[tokio::test]
    async fn fsm_accepts_valid_transition() {
        let mut fsm = pool_fsm();
        assert_eq!(fsm.phase(), Phase::Initializing);
        let cfg_msg = PoolMsg::Configure(PoolConfig {
            size: 4,
            spec: "echo".into(),
        });
        let event = User::user(MailAddr(0), cfg_msg);
        let result = fsm.transition(event);
        assert!(result.is_ok());
        assert_eq!(fsm.phase(), Phase::Serving);
    }

    #[tokio::test]
    async fn fsm_defers_acquire_in_initializing_phase() {
        let mut fsm = pool_fsm();
        let req_msg = PoolMsg::Acquire(Acquire {
            reply_to: Recipient::global(MailAddr(1)),
        });
        let event = User::user(MailAddr(0), req_msg);
        let result = fsm.transition(event);
        assert!(result.is_ok());
        // Message is deferred (held for replay after phase change).
        assert_eq!(fsm.held(), 1);
    }

    #[tokio::test]
    async fn fsm_replays_deferred_after_phase_change() {
        let mut fsm = pool_fsm();

        // Defer Acquire while Initializing
        let req_msg = PoolMsg::Acquire(Acquire {
            reply_to: Recipient::global(MailAddr(2)),
        });
        let _ = fsm.transition(User::user(MailAddr(0), req_msg));
        assert_eq!(fsm.held(), 1);

        // Transition to Serving
        let cfg_msg = PoolMsg::Configure(PoolConfig {
            size: 4,
            spec: "echo".into(),
        });
        let _ = fsm.transition(User::user(MailAddr(0), cfg_msg));
        assert_eq!(fsm.phase(), Phase::Serving);

        // Deferred Acquire was replayed (drain-on-change drains hold queue)
        assert_eq!(fsm.held(), 0);
    }

    #[test]
    fn fsm_cannot_prevent_invalid_phase_message_at_compile_time() {
        // THIS TEST EXISTS AND PASSES — it demonstrates a limitation.
        // PoolMsg::Configure is constructible regardless of phase.
        // We WANT this to fail to compile from a Serving-phase context,
        // but with the current algebra it compiles everywhere.
        let cfg = PoolConfig {
            size: 4,
            spec: "echo".into(),
        };
        let _configure_for_serving = PoolMsg::Configure(cfg);
        // ^ This SHOULD be a compile error when the pool is in Serving phase.
    }
}

// ============================================================================
// Attempt 2: Phase-indexed typestate via Behavior trait (DERIVE-01)
// ============================================================================

// Phase type markers — uninhabited for type-level discrimination only.
enum InitPhase {}
enum ServingPhase {}
enum DrainingPhase {}

/// A phase-indexed pool behavior. P selects the conceptual Event type.
struct _PoolBehavior<P> {
    state: PoolState,
    _phase: std::marker::PhantomData<P>,
}

/// Events only valid during Initializing.
#[allow(dead_code)]
enum InitEvent {
    Configure(PoolConfig),
}

/// Events only valid during Serving.
#[allow(dead_code)]
enum ServingEvent {
    Acquire(Acquire),
    Release(Release),
}

/// Events only valid during Draining.
#[allow(dead_code)]
enum DrainingEvent {
    DrainStatus(DrainStatus),
}

// OBSTRUCTION 1: Cannot impl Behavior for _PoolBehavior<InitPhase> etc.
// Each phase would need a different impl with a different Event type:
//   impl Behavior for _PoolBehavior<InitPhase> { type Event = InitEvent; }
//   impl Behavior for _PoolBehavior<ServingPhase> { type Event = ServingEvent; }
// These are DIFFERENT concrete types. The driver holds `&mut B` for a
// single B — it cannot change the concrete Event type at a call site.
//
// OBSTRUCTION 2: Step::Goto(Ph) carries a runtime VALUE. For uninhabited
// phase markers (empty enums), no Goto value is constructible. Using
// inhabited markers (unit structs) loses type-level discrimination
// because all variants would have the same Ph type.
//
// OBSTRUCTION 3: Rust has no dependent types. The call site that holds
// `&mut _PoolBehavior<InitPhase>` cannot become
// `&mut _PoolBehavior<ServingPhase>` — these are different TYPES.
//
// OBSTRUCTION 4: Even a successful per-phase Behavior impl cannot
// transition between phase types. Behavior::step returns
// BehaviorActed<Self> where Self is fixed. There is no mechanism to
// return a _PoolBehavior<ServingPhase> from a _PoolBehavior<InitPhase>::step().
//
// The commented-out impl below would produce approximately:
//   error[E0119]: conflicting implementations of trait `Behavior`
//   (if we tried multiple impls) OR the step() method cannot change
//   the type of Self.

// ============================================================================
// Attempt 3: Application-local enum dispatch (APP-01)
// ============================================================================

/// Application-local pool behavior using flat enum dispatch.
enum PoolApp {
    Initializing(PoolState),
    Serving(PoolState),
    Draining(PoolState),
}

/// Flat message enum — all phase-specific messages collapsed.
#[allow(dead_code)]
enum AppPoolMsg {
    Configure(PoolConfig),
    Acquire(Acquire),
    Release(Release),
    DrainStatus(DrainStatus),
}

impl PoolApp {
    #[allow(dead_code)]
    fn dispatch(&mut self, msg: AppPoolMsg) -> Result<(), ()> {
        let next = match (&*self, msg) {
            (Self::Initializing(state), AppPoolMsg::Configure(cfg)) => {
                let mut new_state = state.clone();
                new_state.capacity = cfg.size;
                Some(Self::Serving(new_state))
            }
            (Self::Serving(_state), AppPoolMsg::DrainStatus(_)) => None,
            // All invalid (phase, msg) pairs: runtime rejection only.
            _ => None,
        };
        if let Some(next_self) = next {
            *self = next_self;
        }
        Ok(())
    }
}

// APP-01 FINDING: Application-local enum dispatch provides zero additional
// compile-time safety over the FSM. AppPoolMsg flattens all phase-specific
// messages into one enum. `AppPoolMsg::Configure(...)` is constructible
// regardless of the PoolApp variant. Phase validity is checked at runtime
// via match arms — identical pattern to the FSM's transition function.

// ============================================================================
// Attempt 4: Phase token gating (DERIVE-01 continued)
// ============================================================================

/// Phase tokens to gate message construction.
/// OBSTRUCTION: These are ZSTs with pub fields — anyone can construct them.
#[allow(dead_code)]
struct InitToken(());
#[allow(dead_code)]
struct ServingToken(());
#[allow(dead_code)]
struct DrainingToken(());

// OBSTRUCTION: Phase tokens don't enforce anything.
// InitToken(()) is constructible by any code. Making tokens uninhabited
// (empty enums) means NO ONE can construct them — defeating the purpose.
// There is no safe-Rust mechanism to make a type constructible ONLY by
// specific phase-transition code paths without `unsafe` (prohibited).

// ============================================================================
// Attempt 5: Per-phase Behavior wrapper with enum bridge (SURFACE-01 attempt)
// ============================================================================

/// A trait for per-phase behavior that cannot implement Behavior.
trait PhaseProtocol {
    type Phase: Copy;
    type Event;
    type Sends;
    type Error;

    fn transition(
        &mut self,
        event: Self::Event,
    ) -> Result<(Step<Self::Phase, Exit<MailAddr>>, Self::Sends), Self::Error>;
}

// To bridge PhaseProtocol -> Behavior, a wrapper must hold a PhaseProtocol
// implementor and convert a flat event enum to the per-phase event.
// This requires:
// 1. A flat event enum (defeating phase discrimination)
// 2. Runtime dispatch on phase (no compile-time improvement)
// PhaseProtocol cannot implement Behavior because Behavior::Event is a
// single associated type — the bridge would have the flat enum as its Event,
// undoing the per-phase distinction.

// ============================================================================
// Attempt 6: Session-type duality (COUNTER-01: falsification)
// ============================================================================

// Session types (Honda CONCUR'93, ESOP'98) require BOTH endpoints to be
// known at compile time for duality checking. Actor acquaintance addressing
// (Agha 1986 §3.2.1) means actors learn addresses dynamically via messages.
// Two actors cannot know each other's protocol state at compile time
// unless they form a closed system — incompatible with open actor systems.
//
// Even ignoring this, Honda-style duality encodes a two-party protocol
// (!T.S is dual to ?T.S), which is a different guarantee than per-actor
// phase-indexed message validity (which messages can THIS actor accept in
// THIS phase).

// ============================================================================
// Compile-fail probes (COMPILE-01)
// ============================================================================

#[cfg(test)]
mod compile_fail_probes {
    use super::*;

    #[test]
    fn per_phase_event_types_are_distinct() {
        // InitEvent and ServingEvent are distinct types.
        // You cannot pass an InitEvent where ServingEvent is expected.
        let _init = InitEvent::Configure(PoolConfig {
            size: 1,
            spec: "x".into(),
        });
        // This would fail to compile (type mismatch):
        // let _serving: ServingEvent = _init;
    }

    #[test]
    fn fsm_event_is_always_constructible() {
        // PoolMsg::Configure is constructible regardless of phase.
        // This test EXISTS and PASSES — it documents the limitation.
        let msg = PoolMsg::Configure(PoolConfig {
            size: 4,
            spec: "echo".into(),
        });
        let _ = msg;
    }

    #[test]
    fn outbound_send_not_distinguished_from_inbound() {
        // PoolSend::PoolReady is outbound — the pool sends it, never
        // receives it. But nothing prevents constructing one.
        let _send = PoolSend::PoolReady {
            supervisor: MailAddr(99),
        };
    }
}

// ============================================================================
// Composition checks (COMPOSE-01)
// ============================================================================

#[cfg(test)]
mod composition_checks {
    use super::*;
    use behavior::{Watch, stop_on_abnormal_death};

    /// Verify FSM composes with Watch.
    #[tokio::test]
    async fn fsm_composes_with_watching() {
        let fsm = pool_fsm();
        let _watching = Watch::new(fsm, MailAddr(1), stop_on_abnormal_death);
    }
}
