#![no_main]

#[path = "installed_control.rs"]
mod installed_control;

use std::{
    marker::PhantomData,
    num::NonZeroU32,
    time::{Duration, Instant},
};

use behavior_actors::{
    Activate, BreakerCompletion, BreakerError, BreakerMessage, BreakerOutcome, CircuitBreaker,
    EstablishedObservation, EstablishedTerminationMonitor, Exit, ObservationAuthority,
    ObservationId, ObservationRejection, ObservationSequence, ObserveEstablished, Presence,
    PresenceMessage, PresenceReply, PresenceVersion, RoundRobin, Router, RouterError,
    RouterMessage, TerminationMonitorError, TimerElapsed, TimerGeneration, TimerId, Workflow,
    WorkflowDefinition, WorkflowError, WorkflowInput, WorkflowMessage, WorkflowOutcome,
};

use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress, EstablishedRecipient,
    EventLayer, MailAddr, Never, NoBirths, Protocol, Recipient, User,
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

struct Endpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
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
    terminals: usize,
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
    probe.terminals += 1;
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
        MonitorProbe { terminals: 0 },
        ObserveEstablished::new(
            ObservationSequence::issued(),
            selected_observation,
            EstablishedRecipient::issued(Endpoint::<Peer>(PhantomData)),
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

        exercise_owned_observation_report(a, b);

        let relationship = match b & 1 {
            0 => selected_relationship.clone(),
            _ => ObservationAuthority::issued(ObserveEstablished::new(
                ObservationSequence::issued(),
                selected_observation,
                EstablishedRecipient::issued(Endpoint::<Peer>(PhantomData)),
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
        assert!(monitor.base().terminals <= 1);

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
});

#[derive(Clone, Copy)]
enum ObservationReportCase {
    ObserveRejected,
    Started,
    Stopped,
    Cancelled,
    CancelRejected,
}

// Every iteration starts from its actual one-shot emission so affine reports
// are fuzzed without cloning requests or manufacturing historical grants.
fn exercise_owned_observation_report(kind: u8, ordinal: u8) {
    let id = ObservationId(u64::from(ordinal));
    let mut monitor = EstablishedTerminationMonitor::established(
        MonitorProbe { terminals: 0 },
        ObserveEstablished::new(
            ObservationSequence::issued(),
            id,
            EstablishedRecipient::issued(Endpoint::<Peer>(PhantomData)),
        ),
        terminal,
    );
    let initialized = behavior_core::initialize(&mut monitor).expect("pure initialization");
    assert_eq!(initialized.sends.owned.len(), 1);
    assert_eq!(initialized.sends.inner.len(), 0);
    assert_eq!(initialized.creates.len(), 0);
    assert_eq!(initialized.become_, behavior_core::Step::Continue);
    let mut emitted = initialized.sends.owned.into_iter();
    let request = emitted.next().expect("actual original request");
    let extra = emitted.next();
    assert!(extra.is_none());
    let report_case = match kind % 5 {
        0 => ObservationReportCase::ObserveRejected,
        1 => ObservationReportCase::Started,
        2 => ObservationReportCase::Stopped,
        3 => ObservationReportCase::Cancelled,
        _ => ObservationReportCase::CancelRejected,
    };
    match report_case {
        ObservationReportCase::ObserveRejected => {
            let rejected = behavior_core::delegate_transition(
                &mut monitor,
                EventLayer::Owned(EstablishedObservation::observe_rejected(
                    request,
                    ObservationRejection::IdAlreadyBound,
                )),
            )
            .expect("exact start rejection");
            assert_eq!(rejected.sends.owned.len(), 0);
            assert_eq!(rejected.sends.inner.len(), 0);
            assert_eq!(rejected.creates.len(), 0);
            assert_eq!(rejected.become_, behavior_core::Step::Continue);
            let (probe, target) = monitor.into_parts();
            let Ok((request, reason)) = target.into_rejected_observe() else {
                panic!("owned start rejection");
            };
            let (original_id, _returned_recipient) = request.into_inputs();
            assert_eq!(original_id, id);
            assert_eq!(reason, ObservationRejection::IdAlreadyBound);
            assert_eq!(probe.terminals, 0);
        }
        ObservationReportCase::Started
        | ObservationReportCase::Stopped
        | ObservationReportCase::Cancelled
        | ObservationReportCase::CancelRejected => {
            let authority = ObservationAuthority::issued(request);
            let relationship = authority.relationship().clone();
            let started = behavior_core::delegate_transition(
                &mut monitor,
                EventLayer::Owned(EstablishedObservation::started(authority)),
            )
            .expect("exact start acceptance");
            assert_eq!(started.sends.owned.len(), 0);
            assert_eq!(started.sends.inner.len(), 0);
            assert_eq!(started.creates.len(), 0);
            assert_eq!(started.become_, behavior_core::Step::Continue);
            assert_eq!(monitor.base().terminals, 0);
            let at = Instant::now();
            let report = match report_case {
                ObservationReportCase::Started | ObservationReportCase::Stopped => {
                    EstablishedObservation::stopped(relationship.clone(), Ok(Exit::Normal), at)
                }
                ObservationReportCase::Cancelled => EstablishedObservation::cancelled(
                    monitor.take_cancellation().expect("sole original grant"),
                ),
                ObservationReportCase::CancelRejected => EstablishedObservation::cancel_rejected(
                    monitor.take_cancellation().expect("sole original grant"),
                    ObservationRejection::NotObserved,
                ),
                ObservationReportCase::ObserveRejected => {
                    unreachable!("separate original start rejection")
                }
            };
            let actions =
                behavior_core::delegate_transition(&mut monitor, EventLayer::Owned(report))
                    .expect("lawful exact report");
            assert_eq!(actions.sends.owned.len(), 0);
            assert_eq!(actions.sends.inner.len(), 0);
            assert_eq!(actions.creates.len(), 0);
            assert_eq!(actions.become_, behavior_core::Step::Continue);
            match report_case {
                ObservationReportCase::Started | ObservationReportCase::Stopped => {
                    assert_eq!(monitor.base().terminals, 1)
                }
                ObservationReportCase::Cancelled => assert_eq!(monitor.base().terminals, 0),
                ObservationReportCase::CancelRejected => {
                    let stopped = behavior_core::delegate_transition(
                        &mut monitor,
                        EventLayer::Owned(EstablishedObservation::stopped(
                            relationship.clone(),
                            Ok(Exit::Normal),
                            at,
                        )),
                    )
                    .expect("cancellation rejection cannot suppress termination");
                    assert_eq!(stopped.sends.owned.len(), 0);
                    assert_eq!(stopped.sends.inner.len(), 0);
                    assert_eq!(stopped.creates.len(), 0);
                    assert_eq!(stopped.become_, behavior_core::Step::Continue);
                    let (probe, target) = monitor.into_parts();
                    let Ok((request, reason)) = target.into_rejected_cancel() else {
                        panic!("whole rejected grant");
                    };
                    assert!(request.relationship() == &relationship);
                    assert_eq!(request.id(), id);
                    assert_eq!(reason, ObservationRejection::NotObserved);
                    assert_eq!(probe.terminals, 1);
                    drop(request);
                }
                ObservationReportCase::ObserveRejected => {
                    unreachable!("separate original start rejection")
                }
            }
        }
    }
}
