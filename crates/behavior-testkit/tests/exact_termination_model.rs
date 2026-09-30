//! Independent relationship-phase model for exact termination monitoring.

mod installed_control;

use std::marker::PhantomData;
use std::time::Instant;

use behavior_actors::{
    Activate as _, EstablishedObservation, EstablishedTerminationMonitor, Exit, ObservationId,
    ObservationOperation, ObservationRejection, TerminationMonitorError, TerminationObservation,
};

use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress, EstablishedRecipient,
    Never, NoBirths, Protocol, User,
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

    type Installed<B>
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: behavior_core::Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: behavior_core::Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
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

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn record_terminal(
    subject: &mut Subject,
    report: EstablishedObservation<Peer>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    assert!(matches!(report, EstablishedObservation::Stopped { .. }));
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

#[derive(Debug, Clone, Copy)]
enum ReportTarget {
    Selected,
    Foreign,
}

#[derive(Debug, Clone, Copy)]
enum ReportKind {
    Started,
    Cancelled,
    Rejected,
    Stopped,
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
        operations in prop::collection::vec((
            prop_oneof![Just(ReportTarget::Selected), Just(ReportTarget::Foreign)],
            prop_oneof![
                Just(ReportKind::Started),
                Just(ReportKind::Cancelled),
                Just(ReportKind::Rejected),
                Just(ReportKind::Stopped),
            ],
        ), 0..96),
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

        for (target, operation) in operations {
            let id = match target {
                ReportTarget::Selected => selected,
                ReportTarget::Foreign => ObservationId(8),
            };
            let report = match operation {
                ReportKind::Started => EstablishedObservation::started(id),
                ReportKind::Cancelled => EstablishedObservation::cancelled(id),
                ReportKind::Rejected => EstablishedObservation::rejected(
                    id,
                    ObservationOperation::Start,
                    ObservationRejection::IdAlreadyBound,
                ),
                ReportKind::Stopped => EstablishedObservation::stopped(id, Ok(Exit::Normal), timestamp),
            };

            let next_phase = match (target, model, operation) {
                (ReportTarget::Selected, ModelPhase::Requested, ReportKind::Started) => Some(ModelPhase::Observing),
                (ReportTarget::Selected, ModelPhase::Requested | ModelPhase::Observing, ReportKind::Rejected) => Some(ModelPhase::Rejected),
                (ReportTarget::Selected, ModelPhase::Observing, ReportKind::Cancelled) => Some(ModelPhase::Cancelled),
                (ReportTarget::Selected, ModelPhase::Observing, ReportKind::Stopped) => Some(ModelPhase::Observed),
                _ => None,
            };
            let before = subject.observation();
            match (subject.on_path(report), next_phase) {
                (Ok(_), Some(next)) => {
                    if matches!(next, ModelPhase::Observed) {
                        terminal_reactions += 1;
                    }
                    model = next;
                }
                (Err(TerminationMonitorError::UnexpectedReport { observation, report }), None) => {
                    prop_assert_eq!(observation, before);
                    prop_assert_eq!(report.id(), id);
                }
                (Err(TerminationMonitorError::Inner(never)), _) => match never {},
                _ => prop_assert!(false, "report settlement diverged from the model"),
            }

            prop_assert_eq!(actual_phase(subject.observation()), model);
            prop_assert_eq!(subject.base().terminal_reactions, terminal_reactions);
        }
    }
}
