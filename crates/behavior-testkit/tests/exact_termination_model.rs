//! Independent relationship-phase model for exact termination monitoring.

mod installed_control;

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

use behavior_actors::{
    CancelObservation, EstablishedObservation, EstablishedTerminationMonitor, Exit,
    InterpretEstablishedObservation, ObservationAuthority, ObservationId, ObservationOperation,
    ObservationRejection, ObservationSequence, ObserveEstablished, TerminationMonitorError,
    TerminationObservation,
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
    values: Arc<Vec<u64>>,
}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self {
            protocol: PhantomData,
            values: self.values.clone(),
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
    terminal_reports: Vec<EstablishedObservation<Peer>>,
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
    subject.terminal_reports.push(report);
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
            Subject { terminal_reports: Vec::new() },
            ObserveEstablished::new(ObservationSequence::issued(), id,
                EstablishedRecipient::issued(Endpoint::<Peer> { protocol: PhantomData, values: Arc::new(vec![7, 11]) })),
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
                        EstablishedRecipient::issued(Endpoint::<Peer> { protocol: PhantomData, values: Arc::new(vec![7, 11]) })))
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
            prop_assert_eq!(definition.base().terminal_reports.len(), 1 - remaining_reactions);
            let expected_phase = match remaining_reactions {
                0 => TerminationObservation::Observed,
                1 => TerminationObservation::Observing,
                _ => unreachable!("one affine terminal reaction"),
            };
            prop_assert_eq!(definition.observation(), expected_phase);
        }
    }
}

#[derive(Clone, Copy)]
enum ObservationTrace {
    RejectedStartRetried,
    Completion,
    Cancellation,
    CompletionThenRejection,
    RejectionThenCompletion,
}

// This host only exposes the original owning port's returned values. It does
// not register a member, interpret admission, or manufacture a grant.
struct ObservationInputs;

impl InterpretEstablishedObservation<Peer> for ObservationInputs {
    type Output = Result<(ObserveEstablished<Peer>, Endpoint<Peer>), CancelObservation<Peer>>;

    fn observe(
        &mut self,
        request: ObserveEstablished<Peer>,
        endpoint: Endpoint<Peer>,
    ) -> Self::Output {
        Ok((request, endpoint))
    }

    fn cancel(&mut self, request: CancelObservation<Peer>) -> Self::Output {
        Err(request)
    }
}

fn exercise_observation_trace(selected: u64, trace: ObservationTrace, reports: &[u8]) {
    let id = ObservationId(selected);
    let values = Arc::new(vec![selected, 9, 13]);
    let original_allocation = values.as_ptr();
    let original_values = values.clone();
    let mut sequence = ObservationSequence::issued();
    let scope = sequence.branch().expect("first owned ordinal exists");
    let mut monitor = EstablishedTerminationMonitor::established(
        Subject {
            terminal_reports: Vec::new(),
        },
        ObserveEstablished::new(
            scope,
            id,
            EstablishedRecipient::issued(Endpoint::<Peer> {
                protocol: PhantomData,
                values,
            }),
        ),
        record_terminal,
    );
    let initial = behavior_core::initialize(&mut monitor).unwrap();
    assert_eq!(initial.sends.owned.len(), 1);
    assert_eq!(initial.sends.inner.len(), 0);
    assert_eq!(initial.creates.len(), 0);
    assert_eq!(initial.become_, Step::Continue);
    let mut emitted = initial.sends.owned.into_iter();
    let mut request = emitted.next().expect("actual emitted request");
    let extra = emitted.next();
    assert!(extra.is_none());
    let repeated = behavior_core::initialize(&mut monitor).unwrap();
    assert_eq!(repeated.sends.owned.len(), 0);
    assert_eq!(repeated.sends.inner.len(), 0);
    assert_eq!(repeated.creates.len(), 0);
    assert_eq!(repeated.become_, Step::Continue);

    // A rejected original has never been accepted. Its public owning effect
    // may be serially retried, without reconstructing its correlation.
    if matches!(trace, ObservationTrace::RejectedStartRetried) {
        let rejected = behavior_core::delegate_transition(
            &mut monitor,
            EventLayer::Owned(EstablishedObservation::observe_rejected(
                request,
                ObservationRejection::IdAlreadyBound,
            )),
        )
        .unwrap();
        assert_eq!(rejected.sends.owned.len(), 0);
        assert_eq!(rejected.sends.inner.len(), 0);
        assert_eq!(rejected.creates.len(), 0);
        assert_eq!(rejected.become_, Step::Continue);
        assert_eq!(
            monitor.observation(),
            TerminationObservation::Rejected {
                operation: ObservationOperation::Start,
                reason: ObservationRejection::IdAlreadyBound
            }
        );
        let (subject, target) = monitor.into_parts();
        assert_eq!(subject.terminal_reports.len(), 0);
        let Err(target) = target.into_rejected_cancel() else {
            panic!("wrong getter cannot consume the observe original");
        };
        let Ok((original, reason)) = target.into_rejected_observe() else {
            panic!("whole observe rejection remains owned");
        };
        assert_eq!(reason, ObservationRejection::IdAlreadyBound);
        let Ok((original, endpoint)) = original.interpret(&mut ObservationInputs) else {
            panic!("actual observe port returns its original request and endpoint");
        };
        assert_eq!(original.id(), id);
        assert_eq!(endpoint.values.as_ptr(), original_allocation);
        assert_eq!(endpoint.values.as_slice(), original_values.as_slice());
        monitor = EstablishedTerminationMonitor::established(subject, original, record_terminal);
        let retried = behavior_core::initialize(&mut monitor).unwrap();
        assert_eq!(retried.sends.owned.len(), 1);
        assert_eq!(retried.sends.inner.len(), 0);
        assert_eq!(retried.creates.len(), 0);
        assert_eq!(retried.become_, Step::Continue);
        let mut emitted = retried.sends.owned.into_iter();
        request = emitted.next().expect("same whole never-accepted original");
        let extra = emitted.next();
        assert!(extra.is_none());
    }
    let Ok((request, endpoint)) = request.interpret(&mut ObservationInputs) else {
        panic!("whole request is inspected without replacing it");
    };
    assert_eq!(endpoint.values.as_ptr(), original_allocation);
    assert_eq!(endpoint.values.as_slice(), original_values.as_slice());

    // This fresh branch has the SAME numeric ID and endpoint, but cannot
    // satisfy the requested correlation. Whole returned authority is checked.
    let foreign_scope = sequence.branch().expect("next bounded ordinal exists");
    let foreign_authority = ObservationAuthority::issued(ObserveEstablished::new(
        foreign_scope,
        id,
        EstablishedRecipient::issued(endpoint.clone()),
    ));
    let foreign = foreign_authority.relationship().clone();
    let returned = behavior_core::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::started(foreign_authority)),
    );
    let Err(TerminationMonitorError::UnexpectedReport {
        observation,
        report,
    }) = returned
    else {
        panic!("same-ID foreign Started must be returned whole");
    };
    assert_eq!(observation, TerminationObservation::Requested);
    let EstablishedObservation::Started { authority } = report else {
        panic!("original foreign Started returned");
    };
    assert!(authority.relationship() == &foreign);
    let foreign = authority.into_relationship();
    assert_eq!(monitor.observation(), TerminationObservation::Requested);

    let authority = ObservationAuthority::issued(request);
    let relationship = authority.relationship().clone();
    let started = behavior_core::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::started(authority)),
    )
    .unwrap();
    assert_eq!(started.sends.owned.len(), 0);
    assert_eq!(started.sends.inner.len(), 0);
    assert_eq!(started.creates.len(), 0);
    assert_eq!(started.become_, Step::Continue);
    assert_eq!(monitor.observation(), TerminationObservation::Observing);
    let at = Instant::now();
    // Each generated foreign report passes through this SAME live monitor.
    // The expected fact is derived from its foreign origin, not control state.
    for ordinal in reports.iter().take(64) {
        let reported_at = at
            .checked_add(Duration::from_nanos(u64::from(*ordinal)))
            .expect("bounded timestamp");
        let returned = behavior_core::delegate_transition(
            &mut monitor,
            EventLayer::Owned(EstablishedObservation::stopped(
                foreign.clone(),
                Ok(Exit::Normal),
                reported_at,
            )),
        );
        let Err(TerminationMonitorError::UnexpectedReport {
            observation,
            report,
        }) = returned
        else {
            panic!("foreign completion never belongs to this original");
        };
        assert_eq!(observation, TerminationObservation::Observing);
        let EstablishedObservation::Stopped {
            relationship: original,
            outcome,
            at: original_at,
        } = report
        else {
            panic!("whole foreign terminal returned");
        };
        assert!(original == foreign);
        assert!(outcome == Ok(Exit::Normal));
        assert_eq!(original_at, reported_at);
        assert_eq!(monitor.base().terminal_reports.len(), 0);
    }
    match trace {
        ObservationTrace::Cancellation => {
            let request = monitor.take_cancellation().expect("one live grant");
            let repeated = monitor.take_cancellation();
            assert!(repeated.is_none());
            assert!(request.relationship() == &relationship);
            assert_eq!(request.id(), id);
            assert_eq!(monitor.observation(), TerminationObservation::Observing);
            let cancelled = behavior_core::delegate_transition(
                &mut monitor,
                EventLayer::Owned(EstablishedObservation::cancelled(request)),
            )
            .unwrap();
            assert_eq!(cancelled.sends.owned.len(), 0);
            assert_eq!(cancelled.sends.inner.len(), 0);
            assert_eq!(cancelled.creates.len(), 0);
            assert_eq!(cancelled.become_, Step::Continue);
            assert_eq!(monitor.observation(), TerminationObservation::Cancelled);
            assert_eq!(monitor.base().terminal_reports.len(), 0);
            // Cancelled+Stopped is not a lawful winning producer trace. This
            // defensive whole receipt is returned without invoking callback.
        }
        ObservationTrace::CompletionThenRejection | ObservationTrace::RejectionThenCompletion => {
            let request = monitor.take_cancellation().expect("one live grant");
            let repeated = monitor.take_cancellation();
            assert!(repeated.is_none());
            assert!(request.relationship() == &relationship);
            assert_eq!(request.id(), id);
            assert_eq!(monitor.observation(), TerminationObservation::Observing);
            let rejection =
                EstablishedObservation::cancel_rejected(request, ObservationRejection::NotObserved);
            let stopped =
                EstablishedObservation::stopped(relationship.clone(), Ok(Exit::Normal), at);
            let ordered = match trace {
                ObservationTrace::CompletionThenRejection => [stopped, rejection],
                ObservationTrace::RejectionThenCompletion => [rejection, stopped],
                _ => unreachable!("two lawful completion-winning notification orders"),
            };
            for (position, report) in ordered.into_iter().enumerate() {
                let actions =
                    behavior_core::delegate_transition(&mut monitor, EventLayer::Owned(report))
                        .unwrap();
                assert_eq!(actions.sends.owned.len(), 0);
                assert_eq!(actions.sends.inner.len(), 0);
                assert_eq!(actions.creates.len(), 0);
                assert_eq!(actions.become_, Step::Continue);
                // Expectations come from the independently supplied fact order,
                // not from the target's private control-state variants.
                let (expected_phase, expected_terminals) = match (trace, position) {
                    (ObservationTrace::RejectionThenCompletion, 0) => {
                        (TerminationObservation::Observing, 0)
                    }
                    _ => (TerminationObservation::Observed, 1),
                };
                assert_eq!(monitor.observation(), expected_phase);
                assert_eq!(monitor.base().terminal_reports.len(), expected_terminals);
                let permission = monitor.take_cancellation();
                assert!(permission.is_none());
            }
            assert_eq!(monitor.observation(), TerminationObservation::Observed);
        }
        ObservationTrace::Completion | ObservationTrace::RejectedStartRetried => {
            let actions = behavior_core::delegate_transition(
                &mut monitor,
                EventLayer::Owned(EstablishedObservation::stopped(
                    relationship.clone(),
                    Ok(Exit::Normal),
                    at,
                )),
            )
            .unwrap();
            assert_eq!(actions.sends.owned.len(), 0);
            assert_eq!(actions.sends.inner.len(), 0);
            assert_eq!(actions.creates.len(), 0);
            assert_eq!(actions.become_, Step::Continue);
            let permission = monitor.take_cancellation();
            assert!(permission.is_none());
            assert_eq!(monitor.observation(), TerminationObservation::Observed);
        }
    }
    let expected = match trace {
        ObservationTrace::Cancellation => 0,
        _ => 1,
    };
    assert_eq!(monitor.base().terminal_reports.len(), expected);
    for original in &monitor.base().terminal_reports {
        let EstablishedObservation::Stopped {
            relationship: original,
            outcome,
            at: original_at,
        } = original
        else {
            panic!("callback owns the whole terminal fact");
        };
        assert!(original == &relationship);
        assert!(*outcome == Ok(Exit::Normal));
        assert_eq!(*original_at, at);
    }
    // Read-only terminal identity can be replayed, never an affine grant.
    // The same monitor returns every replay whole and reacts no second time.
    for _ in 0..reports.len().clamp(1, 64) {
        let before = monitor.observation();
        let returned = behavior_core::delegate_transition(
            &mut monitor,
            EventLayer::Owned(EstablishedObservation::stopped(
                relationship.clone(),
                Ok(Exit::Normal),
                at,
            )),
        );
        let Err(TerminationMonitorError::UnexpectedReport {
            observation,
            report,
        }) = returned
        else {
            panic!("whole replay cannot trigger another terminal reaction");
        };
        assert_eq!(observation, before);
        let EstablishedObservation::Stopped {
            relationship: original,
            outcome,
            at: original_at,
        } = report
        else {
            panic!("whole replay is returned");
        };
        assert!(original == relationship);
        assert!(outcome == Ok(Exit::Normal));
        assert_eq!(original_at, at);
        assert_eq!(monitor.base().terminal_reports.len(), expected);
    }
    let (subject, target) = monitor.into_parts();
    let Err(target) = target.into_rejected_observe() else {
        panic!("completed or cancelled original is never reconstructed as rejected Observe");
    };
    match trace {
        ObservationTrace::CompletionThenRejection | ObservationTrace::RejectionThenCompletion => {
            let Ok((original, reason)) = target.into_rejected_cancel() else {
                panic!("whole rejected grant coexists with completed terminal");
            };
            assert_eq!(original.id(), id);
            assert!(original.relationship() == &relationship);
            assert_eq!(reason, ObservationRejection::NotObserved);
            let relation = original.into_relationship();
            assert!(relation == relationship);
        }
        _ => {
            let Err(target) = target.into_rejected_cancel() else {
                panic!("no rejected cancellation was acquired");
            };
            drop(target);
        }
    }
    assert_eq!(subject.terminal_reports.len(), expected);
    assert_eq!(endpoint.values.as_ptr(), original_allocation);
    assert_eq!(endpoint.values.as_slice(), original_values.as_slice());
}

proptest! {
    #[test]
    fn continuous_observation_lifetimes_conserve_every_original_fact(
        selected in any::<u64>(),
        reports in prop::collection::vec(any::<u8>(), 0..65),
    ) {
        for trace in [ObservationTrace::RejectedStartRetried, ObservationTrace::Completion,
            ObservationTrace::Cancellation, ObservationTrace::CompletionThenRejection,
            ObservationTrace::RejectionThenCompletion] {
            exercise_observation_trace(selected, trace, &reports);
        }
    }
}
