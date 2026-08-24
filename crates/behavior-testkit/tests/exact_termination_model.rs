//! Independent relationship-phase model for exact termination monitoring.

use std::marker::PhantomData;
use std::time::Instant;

use behavior::{
    Actions, Activate as _, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress,
    EstablishedObservation, EstablishedRecipient, EstablishedTerminationMonitor, Exit, Never,
    NoBirths, ObservationId, ObservationOperation, ObservationRejection, Protocol,
    TerminationMonitorError, TerminationObservation, User,
};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P> {
    protocol: PhantomData<fn() -> P>,
}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self {
            protocol: PhantomData,
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct Peer;

impl Protocol for Peer {
    type Addr = RuntimeAddr;
    type Msg = ();
}

struct Subject {
    terminal_reactions: usize,
}

impl Protocol for Subject {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl BehaviorBase for Subject {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for Subject {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn record_terminal(
    subject: &mut Subject,
    fact: EstablishedObservation<Peer>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    assert!(matches!(fact, EstablishedObservation::Stopped { .. }));
    subject.terminal_reactions += 1;
    Actions::cont()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelPhase {
    Requested,
    Observing,
    Observed,
    Cancelled,
    Rejected,
}

fn actual_phase(observation: TerminationObservation) -> ModelPhase {
    match observation {
        TerminationObservation::Requested => ModelPhase::Requested,
        TerminationObservation::Observing => ModelPhase::Observing,
        TerminationObservation::Observed => ModelPhase::Observed,
        TerminationObservation::Cancelled => ModelPhase::Cancelled,
        TerminationObservation::Rejected { .. } => ModelPhase::Rejected,
    }
}

proptest! {
    #[test]
    fn exact_monitor_matches_the_independent_single_terminal_model(
        operations in prop::collection::vec((any::<bool>(), 0_u8..4), 0..96),
    ) {
        let selected = ObservationId(7);
        let recipient = EstablishedRecipient::issued(Endpoint::<Peer> {
            protocol: PhantomData,
        });
        let mut subject = EstablishedTerminationMonitor::established(
            Subject { terminal_reactions: 0 },
            selected,
            recipient,
            record_terminal,
        )
        .initialize()
        .unwrap()
        .behavior;
        let timestamp = Instant::now();
        let mut model = ModelPhase::Requested;
        let mut terminal_reactions = 0;

        for (matching, operation) in operations {
            let id = if matching { selected } else { ObservationId(8) };
            let fact = match operation {
                0 => EstablishedObservation::started(id),
                1 => EstablishedObservation::cancelled(id),
                2 => EstablishedObservation::rejected(
                    id,
                    ObservationOperation::Start,
                    ObservationRejection::IdAlreadyBound,
                ),
                _ => EstablishedObservation::stopped(id, Ok(Exit::Normal), timestamp),
            };

            let accepted = matching && matches!(
                (model, operation),
                (ModelPhase::Requested, 0 | 2)
                    | (ModelPhase::Observing, 1 | 2 | 3)
            );
            let before = subject.observation();
            match subject.on_path(fact) {
                Ok(_) => prop_assert!(accepted),
                Err(TerminationMonitorError::UnexpectedFact { observation, fact }) => {
                    prop_assert!(!accepted);
                    prop_assert_eq!(observation, before);
                    prop_assert_eq!(fact.id(), id);
                }
                Err(TerminationMonitorError::Inner(never)) => match never {},
            }

            if accepted {
                model = match operation {
                    0 => ModelPhase::Observing,
                    1 => ModelPhase::Cancelled,
                    2 => ModelPhase::Rejected,
                    _ => {
                        terminal_reactions += 1;
                        ModelPhase::Observed
                    }
                };
            }

            prop_assert_eq!(actual_phase(subject.observation()), model);
            prop_assert_eq!(subject.base().terminal_reactions, terminal_reactions);
        }
    }
}
