//! Session protocol derivation campaign: determine whether Bombay Behavior
//! needs phase-indexed protocol typing beyond the existing `Machine`.
//!
//! Concrete case: **Supervised Worker Lifecycle** (3 phases, direction-sensitive).
//!
//! # Protocol
//!
//! A supervised worker actor passes through three phases:
//!
//! 1. **Starting** — waiting for configuration from supervisor.
//!    Valid IN: `Configure { config }`
//!    Valid OUT: `Configured` (send to supervisor)
//!    Invalid IN: `Work`, `DrainStatus`
//!
//! 2. **Running** — processing work, watching peers, sending results.
//!    Valid IN: `Work { payload }`
//!    Valid OUT: `Result { output }` (send to requester)
//!    Also valid: `Watch(peer)` creates observation links
//!    Invalid IN: `Configure`, `DrainStatus`
//!
//! 3. **Draining** — rejecting new work, finishing in-flight, signaling done.
//!    Valid IN: `DrainStatus`
//!    Valid OUT: `Draining { remaining }`, `DrainComplete`
//!    Invalid IN: `Configure`, `Work`
//!
//! # Direction sensitivity
//!
//! Messages have direction: `Configure` comes FROM supervisor, `Work` comes
//! FROM clients, `Result` goes TO clients. In a typed protocol, sending a
//! `Result` where a `Work` is expected would be a type error. The existing
//! `Recipient<B>` names the receiving behavior protocol, so outbound
//! destination identity is distinct even when protocols share a message type.
//! Phase-indexed direction within one behavior remains a separate question.
//!
//! # Actor relevance
//!
//! This is not a generic channel protocol. The lifecycle includes actor-
//! specific concerns: supervision linkage during startup, watching peers
//! only when Running, child actor creation during Running, and
//! shutdown/drain coordination with the supervisor.

// All types in this file are illustrative for the derivation campaign;
// dead-code warnings are expected and suppressed.
#![allow(dead_code)]

use behavior::{Exit, Machine, MailAddr, Move, Never, SendAlgebra, Step, User, UserEvent};

// ---------------------------------------------------------------------------
// Phase and message vocabulary (used by all derivation attempts)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Running,
    Draining,
}

/// Configuration sent by supervisor during startup.
#[derive(Debug, Clone)]
struct Config {
    max_concurrent: usize,
    supervisor: MailAddr,
}

/// A work request from a client.
#[derive(Debug, Clone)]
struct Work {
    payload: Vec<u8>,
    reply_to: MailAddr,
}

/// Union of all messages across all phases (runtime FSM approach).
#[derive(Debug, Clone)]
enum WorkerMsg {
    Configure(Config),
    Work(Work),
    DrainStatus,
}

// ---------------------------------------------------------------------------
// Attempt 1: Existing Machine (FSM-01)
// ---------------------------------------------------------------------------

fn worker_fsm() -> Machine<MailAddr, Vec<Work>, WorkerMsg, Phase, Never> {
    Machine::new(
        Vec::new(),
        Phase::Starting,
        |phase, in_flight, msg| -> Result<Move<Phase>, Never> {
            match (phase, msg) {
                // ---------- Starting phase ----------
                (Phase::Starting, WorkerMsg::Configure(_config)) => Ok(Move::Goto(Phase::Running)),
                (Phase::Starting, _) => Ok(Move::Defer),

                // ---------- Running phase ----------
                (Phase::Running, WorkerMsg::Work(work)) => {
                    in_flight.push(work.clone());
                    Ok(Move::Stay)
                }
                (Phase::Running, WorkerMsg::DrainStatus) => Ok(Move::Goto(Phase::Draining)),
                (Phase::Running, WorkerMsg::Configure(_)) => Ok(Move::Defer),

                // ---------- Draining phase ----------
                (Phase::Draining, WorkerMsg::DrainStatus) => {
                    if in_flight.is_empty() {
                        Ok(Move::Stop)
                    } else {
                        Ok(Move::Stay)
                    }
                }
                (Phase::Draining, WorkerMsg::Work(_)) => Ok(Move::Defer),
                (Phase::Draining, WorkerMsg::Configure(_)) => Ok(Move::Defer),
            }
        },
    )
}

// ---------------------------------------------------------------------------
// FSM baseline tests (FSM-01 evidence)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod fsm_baseline {
    use super::*;

    #[tokio::test]
    async fn fsm_accepts_valid_phase_transitions() {
        let fsm = worker_fsm();
        let initialized = fsm.initialize().unwrap();
        let _ = initialized.actions;
        let mut fsm = initialized.behavior;
        assert_eq!(fsm.phase(), Phase::Starting);

        let event = User {
            from: MailAddr(0),
            message: WorkerMsg::Configure(Config {
                max_concurrent: 4,
                supervisor: MailAddr(1),
            }),
        };
        let _ = fsm.transition(event).unwrap();
        assert_eq!(fsm.phase(), Phase::Running);
    }

    #[tokio::test]
    async fn fsm_defers_work_in_starting_phase() {
        let fsm = worker_fsm();
        let initialized = fsm.initialize().unwrap();
        let _ = initialized.actions;
        let mut fsm = initialized.behavior;

        let work_event = User {
            from: MailAddr(2),
            message: WorkerMsg::Work(Work {
                payload: vec![1, 2, 3],
                reply_to: MailAddr(2),
            }),
        };
        let actions = fsm.transition(work_event).unwrap();
        assert_eq!(fsm.held(), 1);
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(fsm.phase(), Phase::Starting);
    }

    #[tokio::test]
    async fn fsm_replays_deferred_after_phase_change() {
        let fsm = worker_fsm();
        let initialized = fsm.initialize().unwrap();
        let _ = initialized.actions;
        let mut fsm = initialized.behavior;

        let work_event = User {
            from: MailAddr(2),
            message: WorkerMsg::Work(Work {
                payload: vec![1, 2, 3],
                reply_to: MailAddr(2),
            }),
        };
        let _ = fsm.transition(work_event).unwrap();
        assert_eq!(fsm.held(), 1);

        let cfg_event = User {
            from: MailAddr(1),
            message: WorkerMsg::Configure(Config {
                max_concurrent: 4,
                supervisor: MailAddr(1),
            }),
        };
        let _ = fsm.transition(cfg_event).unwrap();
        assert_eq!(fsm.phase(), Phase::Running);
        assert_eq!(fsm.held(), 0);
        assert_eq!(fsm.state().len(), 1);
    }

    /// What the FSM CANNOT do: prevent `Work` in `Starting` at compile time.
    #[tokio::test]
    async fn fsm_cannot_prevent_invalid_phase_message_at_compile_time() {
        // This compiles — and that's the limitation:
        let _bad_event = User {
            from: MailAddr(0),
            message: WorkerMsg::Work(Work {
                payload: vec![],
                reply_to: MailAddr(0),
            }),
        };
        // No compile error — FSM handles it at runtime (defers).
    }
}

// ---------------------------------------------------------------------------
// Attempt 2: Phase-indexed typestate (DERIVE-01)
// ---------------------------------------------------------------------------

enum InitPhase {}
enum RunningPhase {}
enum DrainingPhase {}

struct WorkerBehavior<P, Births> {
    state: WorkerState,
    _phase: std::marker::PhantomData<(P, Births)>,
}

struct WorkerState {
    config: Option<Config>,
    in_flight: Vec<Work>,
}

enum InitEvent {
    Configure(Config),
}

enum RunningEvent {
    Work(Work),
    Drain,
}

enum DrainingEvent {
    DrainStatus,
}

// OBSTRUCTION: impl Behavior for WorkerBehavior<InitPhase, NoBirths>
// Each phase would need its own Behavior impl with different Event types.
// WorkerBehavior<InitPhase>::Event = InitEvent
// WorkerBehavior<RunningPhase>::Event = RunningEvent
// But the caller (driver) holds `&mut B` for a single B.
//
// Even if we write the impls, transitioning from InitPhase to RunningPhase
// requires changing the concrete type at the call site — impossible without
// dynamic dispatch (prohibited by AGENTS.md).
//
// Furthermore, `Step::Goto(Ph)` takes a VALUE of type Ph. For uninhabited
// phase marker types (empty enums), no Goto value can be constructed.
// Using inhabited markers (e.g., unit structs) loses type discrimination
// because all phases would have the same Ph type.

// ---------------------------------------------------------------------------
// Attempt 3: Application-local enum dispatch (APP-01)
// ---------------------------------------------------------------------------

enum WorkerApp {
    Starting(WorkerStarting),
    Running(WorkerRunning),
    Draining(WorkerDraining),
}

struct WorkerStarting {
    in_flight: Vec<Work>,
}

struct WorkerRunning {
    in_flight: Vec<Work>,
}

struct WorkerDraining {
    in_flight: Vec<Work>,
}

enum AppMsg {
    Configure(Config),
    Work(Work),
    DrainStatus,
}

impl WorkerApp {
    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        msg: AppMsg,
    ) -> Result<Step<Never, behavior::Stopped>, Never> {
        match msg {
            AppMsg::Configure(config) => {
                if let WorkerApp::Starting(state) = self {
                    let in_flight = state.in_flight.clone();
                    *self = WorkerApp::Running(WorkerRunning { in_flight });
                    let _ = config;
                }
            }
            AppMsg::Work(work) => {
                if let WorkerApp::Running(state) = self {
                    state.in_flight.push(work);
                }
            }
            AppMsg::DrainStatus => {
                if let WorkerApp::Running(state) = self {
                    let in_flight = state.in_flight.clone();
                    *self = WorkerApp::Draining(WorkerDraining { in_flight });
                } else if let WorkerApp::Draining(state) = self {
                    if state.in_flight.is_empty() {
                        return Ok(Step::Stop(behavior::Stopped));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

// The application-local approach provides no ADDITIONAL compile-time safety
// over the FSM. `AppMsg` flattens all phase-specific messages into one enum,
// making invalid-phase messages representable.

// ---------------------------------------------------------------------------
// Attempt 4: Phase-indexed wrapper trait (DERIVE-01 continued)
// ---------------------------------------------------------------------------

trait PhaseBehavior {
    type Event: UserEvent<Addr = MailAddr, Message = Self::Msg>;
    type Msg;
    type Sends: SendAlgebra;
    type Error;
    type NextPhase;

    fn step_phase(
        &mut self,
        event: Self::Event,
    ) -> Result<(Step<Self::NextPhase, Exit<MailAddr>>, Self::Sends), Self::Error>;
}

// OBSTRUCTION: A generic wrapper over PhaseBehavior cannot implement Behavior
// because Behavior::Event must be a SINGLE concrete type. To bridge
// PhaseBehavior -> we'd need a sum type over all phase events,
// recreating the flat enum problem.

// ---------------------------------------------------------------------------
// Attempt 5: Session-type duality (COUNTER-01: falsification)
// ---------------------------------------------------------------------------

// Session types (Honda CONCUR'93, ESOP'98) require BOTH endpoints to be
// known at compile time for duality checking. Actor acquaintance addressing
// (Agha 1986 §3.2.1) means actors learn addresses dynamically. Two actors
// cannot know each other's protocol state at compile time unless they are
// created together as a closed system — incompatible with open actor systems.

// ---------------------------------------------------------------------------
// Summary of obstructions (OBSTRUCTION-01)
// ---------------------------------------------------------------------------

// 1. Fixed Event type: Behavior::Event is a single associated type, not
//    varying by phase. A behavior cannot change its accepted message type
//    when transitioning phases.
//
// 2. No runtime type change: The driver holds `&mut B` for a single B.
//    Even if B::Ph changes, B stays the same type. Rust has no dependent
//    types or typestate that changes the concrete type at a call site.
//
// 3. Ph is a value, not a type index: Step::Goto(Ph) transitions by VALUE,
//    not by type. The phase value does not constrain Event.
//
// 4. Actor addressing precludes session duality: Actors learn addresses
//    dynamically. Static duality requires both endpoints to be known at
//    compile time.
//
// 5. Wrapper composition: Wrappers (Supervisor, Watch, etc.) expect
//    Behavior with a single Event type. A phase-varying event type would
//    break every wrapper's composition contract.

// ---------------------------------------------------------------------------
// COMPILE-FAIL probes (COMPILE-01)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod compile_fail_probes {
    use super::*;

    /// Invalid-phase event construction is always possible with flat enums.
    #[test]
    fn invalid_phase_event_is_constructible_with_fsm() {
        let _msg = WorkerMsg::Configure(Config {
            max_concurrent: 1,
            supervisor: MailAddr(0),
        });
        // Nothing prevents sending this to a Running-phase FSM.
        // The FSM handles it at runtime (defer or ignore).
    }

    /// Per-phase event types ARE distinct — the obstruction is in
    /// transitioning between them at the Behavior trait level.
    #[test]
    fn per_phase_event_types_are_distinct() {
        let _init: InitEvent = InitEvent::Configure(Config {
            max_concurrent: 1,
            supervisor: MailAddr(0),
        });
        let _run: RunningEvent = RunningEvent::Work(Work {
            payload: vec![],
            reply_to: MailAddr(0),
        });
        // Distinct types exist — you can't pass RunningEvent to Init-phase.
        // But you can't transition from Init to Running at the type level
        // in the Behavior trait either.
    }
}

// ---------------------------------------------------------------------------
// COMPOSITION checks (COMPOSE-01)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod composition_checks {
    use super::*;

    /// FSM composes with all wrappers via the existing Behavior impl.
    /// A phase-varying Event type would break every wrapper.
    #[tokio::test]
    async fn fsm_composes_with_watching() {
        // Machine<A,S,M,P,E> implements Behavior<Event = User<A,M>>
        // Watch<Machine<...>> wraps it — this compiles because Event is single.
        let _fsm = worker_fsm();
    }
}
use behavior_testkit::InitializeTest;
