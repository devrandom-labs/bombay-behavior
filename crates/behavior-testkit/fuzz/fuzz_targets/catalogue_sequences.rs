#![no_main]

#[path = "installed_control.rs"]
mod installed_control;

use std::{
    marker::PhantomData,
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};

use behavior_actors::{
    Activate, BreakerCompletion, BreakerError, BreakerMessage, BreakerOutcome, CancelObservation,
    CircuitBreaker, EstablishedObservation, EstablishedTerminationMonitor, Exit,
    InterpretEstablishedObservation, ObservationAuthority, ObservationId, ObservationOperation,
    ObservationRejection, ObservationSequence, ObserveEstablished, Presence, PresenceMessage,
    PresenceReply, PresenceVersion, RoundRobin, Router, RouterError, RouterMessage,
    TerminationMonitorError, TerminationObservation, TimerElapsed, TimerGeneration, TimerId,
    Workflow, WorkflowDefinition, WorkflowError, WorkflowInput, WorkflowMessage, WorkflowOutcome,
};

use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress, EstablishedRecipient,
    EventLayer, MailAddr, Never, NoBirths, Protocol, Recipient, Step, User,
};
use bombay_behavior_fuzz::TestRecipient;
use libfuzzer_sys::fuzz_target;

type BreakerReply = TestRecipient<BreakerOutcome>;
type PresenceReplyTarget = TestRecipient<PresenceReply<Vec<u8>>>;
type WorkflowReply = TestRecipient<WorkflowOutcome<u8>>;

#[derive(Clone, Copy, PartialEq, Eq)]
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
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
}

struct Peer;

impl Protocol for Peer {
    type Addr = RuntimeAddr;
    type Msg = ();
}

struct RouteTarget;

impl Protocol for RouteTarget {
    type Addr = MailAddr;
    type Msg = u8;
}

struct MonitorProbe {
    terminal_reports: Vec<EstablishedObservation<Peer>>,
}

impl Protocol for MonitorProbe {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl BehaviorBase for MonitorProbe {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for MonitorProbe {
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

fn terminal(
    probe: &mut MonitorProbe,
    report: EstablishedObservation<Peer>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    assert!(matches!(report, EstablishedObservation::Stopped { .. }));
    probe.terminal_reports.push(report);
    Actions::cont()
}

fuzz_target!(|bytes: &[u8]| {
    let mut breaker = CircuitBreaker::<MailAddr, Recipient<BreakerReply>>::new(
        NonZeroU32::new(2).expect("constant is non-zero"),
        Duration::from_nanos(1),
        TimerId(1),
    )
    .expect("constant reset delay is positive")
    .initialize()
    .expect("breaker initialization is infallible")
    .behavior;
    let mut presence =
        (Presence::<MailAddr, Vec<u8>, Recipient<PresenceReplyTarget>>::new(|key| {
            TimerId(key.first().copied().map_or(0, u64::from))
        }))
        .initialize()
        .expect("presence initialization is infallible")
        .behavior;
    let mut workflow =
        Workflow::<MailAddr, u8, Recipient<WorkflowReply>>::new(WorkflowDefinition {
            steps: vec![0, 1, 2],
            dependencies: vec![(0, 2), (1, 2)],
        })
        .expect("constant graph is acyclic")
        .initialize()
        .expect("workflow initialization is infallible")
        .behavior;
    let selected_observation = ObservationId(7);
    let mut monitor = EstablishedTerminationMonitor::established(
        MonitorProbe {
            terminal_reports: Vec::new(),
        },
        ObserveEstablished::new(
            ObservationSequence::issued(),
            selected_observation,
            EstablishedRecipient::issued(Endpoint::<Peer> {
                protocol: PhantomData,
                values: Arc::new(vec![7, 11]),
            }),
        ),
        terminal,
    );
    let initialization =
        behavior_core::initialize(&mut monitor).expect("monitor initialization is infallible");
    assert_eq!(initialization.sends.owned.len(), 1);
    assert_eq!(initialization.sends.inner.len(), 0);
    assert_eq!(initialization.creates.len(), 0);
    assert_eq!(initialization.become_, behavior_core::Step::Continue);
    let mut emitted = initialization.sends.owned.into_iter();
    let authority = ObservationAuthority::issued(emitted.next().expect("actual emitted request"));
    let extra = emitted.next();
    assert!(extra.is_none());
    let selected_relationship = authority.relationship().clone();
    let started = behavior_core::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::started(authority)),
    )
    .expect("matching acceptance");
    assert_eq!(started.sends.owned.len(), 0);
    assert_eq!(started.sends.inner.len(), 0);
    assert_eq!(started.creates.len(), 0);
    assert_eq!(started.become_, behavior_core::Step::Continue);
    let mut router =
        Router::<MailAddr, Recipient<RouteTarget>, _>::new(Vec::new(), RoundRobin::default())
            .initialize()
            .expect("router initialization is infallible")
            .behavior;
    let mut eligible = Vec::<MailAddr>::new();
    let mut next = 0usize;

    let breaker_reply = Recipient::global(MailAddr(1));
    let presence_reply = Recipient::global(MailAddr(2));
    let workflow_reply = Recipient::global(MailAddr(3));
    for chunk in bytes.chunks(4) {
        let a = chunk.first().copied().unwrap_or(0);
        let b = chunk.get(1).copied().unwrap_or(0);
        let generation = TimerGeneration(u64::from(chunk.get(2).copied().unwrap_or(0)));
        let attempt = behavior_actors::BreakerAttempt(u64::from(b));
        let submitted_completion = match a % 4 {
            1 => Some(BreakerCompletion::Succeeded { attempt }),
            2 => Some(BreakerCompletion::Failed { attempt }),
            _ => None,
        };
        let breaker_result = match a % 4 {
            0 => breaker.receive(
                MailAddr(0),
                BreakerMessage::Admit {
                    reply_to: breaker_reply,
                },
            ),
            1 => breaker.receive(MailAddr(0), BreakerMessage::Succeeded { attempt }),
            2 => breaker.receive(MailAddr(0), BreakerMessage::Failed { attempt }),
            _ => breaker.on_path(TimerElapsed::new(TimerId(1), generation)),
        };
        match breaker_result {
            Ok(actions) => {
                assert!(actions.sends.replies.len() <= 1);
                assert!(actions.sends.schedules.len() <= 1);
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, behavior_core::Step::Continue));
            }
            Err(BreakerError::UnexpectedCompletion(returned)) => {
                assert_eq!(submitted_completion, Some(returned));
            }
        }

        let participant = vec![b];
        let presence_actions = if a % 3 == 0 {
            presence.on_path(TimerElapsed::new(TimerId(u64::from(b)), generation))
        } else {
            presence.receive(
                MailAddr(0),
                PresenceMessage::Announce {
                    participant,
                    version: PresenceVersion(u64::from(generation.0 as u8)),
                    lifetime: Duration::from_nanos(1),
                    reply_to: presence_reply,
                },
            )
        }
        .expect("presence fold is infallible");
        if a % 3 == 0 {
            assert!(presence_actions.sends.replies.len() <= 1);
            assert!(presence_actions.sends.schedules.is_empty());
        } else {
            assert_eq!(presence_actions.sends.replies.len(), 1);
            assert!(presence_actions.sends.schedules.len() <= 1);
        }
        assert!(presence_actions.creates.is_empty());
        assert!(matches!(
            presence_actions.become_,
            behavior_core::Step::Continue
        ));

        let workflow_message = match a % 4 {
            0 => WorkflowMessage::Start {
                reply_to: workflow_reply,
            },
            1 => WorkflowMessage::Complete { step: b % 4 },
            2 => WorkflowMessage::Fail { step: b % 4 },
            _ => WorkflowMessage::Cancel {
                reply_to: workflow_reply,
            },
        };
        match workflow.receive(MailAddr(0), workflow_message) {
            Ok(actions) => {
                assert!(actions.sends.len() <= 1);
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, behavior_core::Step::Continue));
            }
            Err(WorkflowError::NotStarted(
                WorkflowInput::Complete { .. } | WorkflowInput::Fail { .. },
            )) => {}
        }

        let relationship = match b & 1 {
            0 => selected_relationship.clone(),
            _ => ObservationAuthority::issued(ObserveEstablished::new(
                ObservationSequence::issued(),
                selected_observation,
                EstablishedRecipient::issued(Endpoint::<Peer> {
                    protocol: PhantomData,
                    values: Arc::new(vec![7, 11]),
                }),
            ))
            .into_relationship(),
        };
        let original_relationship = relationship.clone();
        let at = Instant::now();
        let report = EstablishedObservation::stopped(relationship, Ok(Exit::Normal), at);
        let observation_before = monitor.observation();
        match behavior_core::delegate_transition(&mut monitor, EventLayer::Owned(report)) {
            Ok(actions) => {
                assert_eq!(actions.sends.owned.len(), 0);
                assert_eq!(actions.sends.inner.len(), 0);
                assert_eq!(actions.creates.len(), 0);
                assert_eq!(actions.become_, behavior_core::Step::Continue);
            }
            Err(TerminationMonitorError::UnexpectedReport {
                observation,
                report,
            }) => {
                assert_eq!(observation, observation_before);
                let EstablishedObservation::Stopped {
                    relationship,
                    outcome,
                    at: retained_at,
                } = report
                else {
                    panic!("fuzzed terminal report is retained whole");
                };
                assert!(relationship == original_relationship);
                assert!(outcome == Ok(Exit::Normal));
                assert_eq!(retained_at, at);
            }
            Err(TerminationMonitorError::Inner(never)) => match never {},
        }
        assert!(monitor.base().terminal_reports.len() <= 1);

        let member = MailAddr(u64::from(b % 8));
        match a % 3 {
            0 => {
                let actions = router
                    .receive(MailAddr(0), RouterMessage::Add(Recipient::global(member)))
                    .expect("membership addition is infallible");
                assert!(actions.sends.is_empty());
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, behavior_core::Step::Continue));
                if !eligible.contains(&member) {
                    eligible.push(member);
                }
            }
            1 => {
                let actions = router
                    .receive(
                        MailAddr(0),
                        RouterMessage::Remove(Recipient::global(member)),
                    )
                    .expect("membership removal is infallible");
                assert!(actions.sends.is_empty());
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, behavior_core::Step::Continue));
                if let Some(index) = eligible.iter().position(|candidate| *candidate == member) {
                    eligible.remove(index);
                    if eligible.is_empty() {
                        next = 0;
                    } else {
                        if index < next {
                            next -= 1;
                        }
                        next %= eligible.len();
                    }
                }
            }
            _ if eligible.is_empty() => {
                let rejection = router.receive(MailAddr(0), RouterMessage::Route(b));
                assert!(matches!(
                    rejection,
                    Err(RouterError::NoEligibleRecipients(returned)) if returned == b
                ));
            }
            _ => {
                let expected = eligible[next % eligible.len()];
                next = (next % eligible.len() + 1) % eligible.len();
                let actions = router
                    .receive(MailAddr(0), RouterMessage::Route(b))
                    .expect("non-empty round-robin membership selects one route");
                assert_eq!(actions.sends.len(), 1);
                assert_eq!(actions.sends[0].to.address(), expected);
                assert_eq!(actions.sends[0].message, b);
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, behavior_core::Step::Continue));
            }
        }
        assert_eq!(
            router
                .recipients()
                .iter()
                .map(|recipient| recipient.address())
                .collect::<Vec<_>>(),
            eligible
        );
    }
    let selected = bytes.iter().take(8).fold(0u64, |id, byte| {
        id.wrapping_mul(256).wrapping_add(u64::from(*byte))
    });
    for trace in [
        ObservationTrace::RejectedStartRetried,
        ObservationTrace::Completion,
        ObservationTrace::Cancellation,
        ObservationTrace::CompletionThenRejection,
        ObservationTrace::RejectionThenCompletion,
    ] {
        exercise_observation_trace(selected, trace, bytes);
    }
});

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
        MonitorProbe {
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
        terminal,
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
        monitor = EstablishedTerminationMonitor::established(subject, original, terminal);
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
