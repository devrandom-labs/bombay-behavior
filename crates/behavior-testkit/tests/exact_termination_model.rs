//! Independent relationship-phase model for exact termination monitoring.

mod installed_control;

use std::marker::PhantomData;
use std::time::Instant;

use behavior_actors::{
    EstablishedObservation, EstablishedTerminationMonitor, Exit, ObservationAuthority,
    ObservationId, ObservationRejection, ObservationSequence, ObserveEstablished,
    TerminationMonitorError, TerminationObservation,
};

use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress, EstablishedRecipient,
    EventLayer, Never, NoBirths, Protocol, Step, User,
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

#[derive(Debug, Clone, Copy)]
enum ReportTarget {
    Selected,
    Foreign,
}

proptest! {
    #[test]
    fn exact_monitor_delivers_one_terminal_and_denies_foreign_same_numeric_id(
        selected in any::<u64>(),
        targets in prop::collection::vec(
            prop_oneof![Just(ReportTarget::Selected), Just(ReportTarget::Foreign)], 0..96),
    ) {
        let id = ObservationId(selected);
        let mut definition = EstablishedTerminationMonitor::established(
            Subject { terminal_reactions: 0 },
            ObserveEstablished::new(ObservationSequence::issued(), id,
                EstablishedRecipient::issued(Endpoint::<Peer> { protocol: PhantomData })),
            record_terminal,
        );
        let initialization = behavior_core::initialize(&mut definition).unwrap();
        prop_assert_eq!(initialization.sends.owned.len(), 1);
        prop_assert_eq!(initialization.sends.inner.len(), 0);
        prop_assert_eq!(initialization.creates.len(), 0);
        prop_assert_eq!(initialization.become_, Step::Continue);
        let mut emitted = initialization.sends.owned.into_iter();
        let authority = ObservationAuthority::issued(emitted.next().expect("actual original request"));
        let extra = emitted.next();
        prop_assert!(extra.is_none());
        let relationship = authority.relationship().clone();
        let started = behavior_core::delegate_transition(&mut definition,
            EventLayer::Owned(EstablishedObservation::started(authority))).unwrap();
        prop_assert_eq!(started.sends.owned.len(), 0);
        prop_assert_eq!(started.sends.inner.len(), 0);
        prop_assert_eq!(started.creates.len(), 0);
        prop_assert_eq!(started.become_, Step::Continue);
        let timestamp = Instant::now();
        let mut remaining_reactions = 1usize;
        for target in targets {
            let reported = match target {
                ReportTarget::Selected => relationship.clone(),
                ReportTarget::Foreign => ObservationAuthority::issued(
                    ObserveEstablished::new(ObservationSequence::issued(), id,
                        EstablishedRecipient::issued(Endpoint::<Peer> { protocol: PhantomData })))
                    .into_relationship(),
            };
            let expected = match target {
                ReportTarget::Selected => remaining_reactions,
                ReportTarget::Foreign => 0,
            };
            let original = reported.clone();
            let report = EstablishedObservation::stopped(reported, Ok(Exit::Normal), timestamp);
            let before = definition.observation();
            let returned = behavior_core::delegate_transition(&mut definition, EventLayer::Owned(report));
            match returned {
                Ok(actions) => {
                    prop_assert_eq!(expected, 1);
                    remaining_reactions = 0;
                    prop_assert_eq!(actions.sends.owned.len(), 0);
                    prop_assert_eq!(actions.sends.inner.len(), 0);
                    prop_assert_eq!(actions.creates.len(), 0);
                    prop_assert_eq!(actions.become_, Step::Continue);
                }
                Err(TerminationMonitorError::UnexpectedReport { observation, report }) => {
                    prop_assert_eq!(expected, 0);
                    prop_assert_eq!(observation, before);
                    let EstablishedObservation::Stopped { relationship: returned, outcome, at } = report else {
                        panic!("whole exact terminal report is returned");
                    };
                    prop_assert!(returned == original);
                    prop_assert_eq!(outcome, Ok(Exit::Normal));
                    prop_assert_eq!(at, timestamp);
                }
                Err(TerminationMonitorError::Inner(never)) => match never {},
            }
            prop_assert_eq!(definition.base().terminal_reactions, 1 - remaining_reactions);
            let expected_phase = match remaining_reactions {
                0 => TerminationObservation::Observed,
                1 => TerminationObservation::Observing,
                _ => unreachable!("one affine terminal reaction"),
            };
            prop_assert_eq!(definition.observation(), expected_phase);
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ObservationReportKind {
    Started,
    Stopped,
    Cancelled,
    ObserveRejected,
    CancelRejected,
}

proptest! {
    #[test]
    fn every_owned_report_has_complete_actions_and_consumable_rejection_custody(
        selected in any::<u64>(),
        kinds in prop::collection::vec(prop_oneof![
            Just(ObservationReportKind::Started), Just(ObservationReportKind::Stopped),
            Just(ObservationReportKind::Cancelled), Just(ObservationReportKind::ObserveRejected),
            Just(ObservationReportKind::CancelRejected),
        ], 1..32),
    ) {
        for kind in kinds {
            let id = ObservationId(selected);
            let mut monitor = EstablishedTerminationMonitor::established(
                Subject { terminal_reactions: 0 },
                ObserveEstablished::new(ObservationSequence::issued(), id,
                    EstablishedRecipient::issued(Endpoint::<Peer> { protocol: PhantomData })),
                record_terminal,
            );
            let initialized = behavior_core::initialize(&mut monitor).unwrap();
            prop_assert_eq!(initialized.sends.owned.len(), 1);
            prop_assert_eq!(initialized.sends.inner.len(), 0);
            prop_assert_eq!(initialized.creates.len(), 0);
            prop_assert_eq!(initialized.become_, Step::Continue);
            let mut emitted = initialized.sends.owned.into_iter();
            let request = emitted.next().expect("actual emitted original");
            let extra = emitted.next();
            prop_assert!(extra.is_none());
            let at = Instant::now();
            match kind {
                ObservationReportKind::ObserveRejected => {
                    let actions = behavior_core::delegate_transition(&mut monitor,
                        EventLayer::Owned(EstablishedObservation::observe_rejected(request, ObservationRejection::IdAlreadyBound))).unwrap();
                    prop_assert_eq!(actions.sends.owned.len(), 0);
                    prop_assert_eq!(actions.sends.inner.len(), 0);
                    prop_assert_eq!(actions.creates.len(), 0);
                    prop_assert_eq!(actions.become_, Step::Continue);
                    prop_assert_eq!(monitor.base().terminal_reactions, 0);
                    let (subject, target) = monitor.into_parts();
                    let Ok((original, reason)) = target.into_rejected_observe() else { panic!("whole start rejection is consumingly available"); };
                    let (returned_id, _returned_recipient) = original.into_inputs();
                    prop_assert_eq!(returned_id, id);
                    prop_assert_eq!(reason, ObservationRejection::IdAlreadyBound);
                    prop_assert_eq!(subject.terminal_reactions, 0);
                }
                ObservationReportKind::Started | ObservationReportKind::Stopped |
                ObservationReportKind::Cancelled | ObservationReportKind::CancelRejected => {
                    let authority = ObservationAuthority::issued(request);
                    let relationship = authority.relationship().clone();
                    let started = behavior_core::delegate_transition(&mut monitor,
                        EventLayer::Owned(EstablishedObservation::started(authority))).unwrap();
                    prop_assert_eq!(started.sends.owned.len(), 0);
                    prop_assert_eq!(started.sends.inner.len(), 0);
                    prop_assert_eq!(started.creates.len(), 0);
                    prop_assert_eq!(started.become_, Step::Continue);
                    prop_assert_eq!(monitor.observation(), TerminationObservation::Observing);
                    prop_assert_eq!(monitor.base().terminal_reactions, 0);
                    let report = match kind {
                        ObservationReportKind::Started | ObservationReportKind::Stopped =>
                            EstablishedObservation::stopped(relationship.clone(), Ok(Exit::Normal), at),
                        ObservationReportKind::Cancelled => EstablishedObservation::cancelled(
                            monitor.take_cancellation().expect("one affine cancellation")),
                        ObservationReportKind::CancelRejected => EstablishedObservation::cancel_rejected(
                            monitor.take_cancellation().expect("one affine cancellation"), ObservationRejection::NotObserved),
                        ObservationReportKind::ObserveRejected => unreachable!("start rejection owns its separate original"),
                    };
                    let actions = behavior_core::delegate_transition(&mut monitor, EventLayer::Owned(report)).unwrap();
                    prop_assert_eq!(actions.sends.owned.len(), 0);
                    prop_assert_eq!(actions.sends.inner.len(), 0);
                    prop_assert_eq!(actions.creates.len(), 0);
                    prop_assert_eq!(actions.become_, Step::Continue);
                    match kind {
                        ObservationReportKind::Started | ObservationReportKind::Stopped => {
                            prop_assert_eq!(monitor.observation(), TerminationObservation::Observed);
                            prop_assert_eq!(monitor.base().terminal_reactions, 1);
                        }
                        ObservationReportKind::Cancelled => {
                            prop_assert_eq!(monitor.observation(), TerminationObservation::Cancelled);
                            prop_assert_eq!(monitor.base().terminal_reactions, 0);
                        }
                        ObservationReportKind::CancelRejected => {
                            prop_assert_eq!(monitor.observation(), TerminationObservation::Observing);
                            let stopped = behavior_core::delegate_transition(&mut monitor,
                                EventLayer::Owned(EstablishedObservation::stopped(relationship.clone(), Ok(Exit::Normal), at))).unwrap();
                            prop_assert_eq!(stopped.sends.owned.len(), 0);
                            prop_assert_eq!(stopped.sends.inner.len(), 0);
                            prop_assert_eq!(stopped.creates.len(), 0);
                            prop_assert_eq!(stopped.become_, Step::Continue);
                            prop_assert_eq!(monitor.base().terminal_reactions, 1);
                            let (subject, target) = monitor.into_parts();
                            let Ok((original, reason)) = target.into_rejected_cancel() else { panic!("whole rejected grant coexists with terminal reaction"); };
                            prop_assert!(original.relationship() == &relationship);
                            prop_assert_eq!(original.id(), id);
                            prop_assert_eq!(reason, ObservationRejection::NotObserved);
                            prop_assert_eq!(subject.terminal_reactions, 1);
                            drop(original);
                        }
                        ObservationReportKind::ObserveRejected => unreachable!("separate start-rejection path"),
                    }
                }
            }
        }
    }
}
