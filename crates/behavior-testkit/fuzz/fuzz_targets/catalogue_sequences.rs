#![no_main]

use std::{
    marker::PhantomData,
    num::NonZeroU32,
    time::{Duration, Instant},
};

use behavior::{
    Actions, Activate, Address, Behavior, BehaviorActed, BehaviorBase, BreakerCompletion,
    BreakerError, BreakerMessage, BreakerOutcome, CircuitBreaker, EndpointAddress,
    EstablishedObservation, EstablishedRecipient, EstablishedTerminationMonitor, Exit, MailAddr,
    Never, NoBirths, ObservationId, ObservationOperation, ObservationRejection, Presence,
    PresenceMessage, PresenceReply, PresenceVersion, Protocol, Recipient, RoundRobin, Router,
    RouterError, RouterMessage, TerminationMonitorError, TimerElapsed, TimerGeneration, TimerId,
    User, Workflow, WorkflowDefinition, WorkflowError, WorkflowInput, WorkflowMessage,
    WorkflowOutcome,
};
use bombay_behavior_fuzz::TestRecipient;
use libfuzzer_sys::fuzz_target;

type BreakerReply = TestRecipient<BreakerOutcome>;
type PresenceReplyTarget = TestRecipient<PresenceReply<Vec<u8>>>;
type WorkflowReply = TestRecipient<WorkflowOutcome<u8>>;

fn timer(key: &Vec<u8>) -> TimerId {
    TimerId(key.first().copied().map_or(0, u64::from))
}

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

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn terminal(
    probe: &mut MonitorProbe,
    fact: EstablishedObservation<Peer>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    assert!(matches!(fact, EstablishedObservation::Stopped { .. }));
    probe.terminals += 1;
    Actions::cont()
}

fuzz_target!(|bytes: &[u8]| {
    let mut breaker =
        CircuitBreaker::<MailAddr, Recipient<BreakerReply>>::new(
        NonZeroU32::new(2).expect("constant is non-zero"),
        Duration::from_nanos(1),
        TimerId(1),
    )
    .expect("constant reset delay is positive")
    .initialize()
    .expect("breaker initialization is infallible")
    .behavior;
    let mut presence = (Presence::<
        MailAddr,
        Vec<u8>,
        Recipient<PresenceReplyTarget>,
    >::new(timer))
        .initialize()
        .expect("presence initialization is infallible")
        .behavior;
    let mut workflow = Workflow::<
        MailAddr,
        u8,
        Recipient<WorkflowReply>,
    >::new(WorkflowDefinition {
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
        selected_observation,
        EstablishedRecipient::issued(Endpoint::<Peer>(PhantomData)),
        terminal,
    )
    .initialize()
    .expect("monitor initialization is infallible")
    .behavior;
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
        let attempt = behavior::BreakerAttempt(u64::from(b));
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
            Ok(_) => {}
            Err(BreakerError::UnexpectedCompletion(returned)) => {
                assert_eq!(submitted_completion, Some(returned));
            }
        }

        let participant = vec![b];
        if a % 3 == 0 {
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
            Ok(_) => {}
            Err(WorkflowError::NotStarted(
                WorkflowInput::Complete { .. } | WorkflowInput::Fail { .. },
            )) => {}
        }

        let observation = if b & 1 == 0 {
            selected_observation
        } else {
            ObservationId(8)
        };
        let fact = match a % 4 {
            0 => EstablishedObservation::started(observation),
            1 => EstablishedObservation::cancelled(observation),
            2 => EstablishedObservation::rejected(
                observation,
                ObservationOperation::Start,
                ObservationRejection::IdAlreadyBound,
            ),
            _ => EstablishedObservation::stopped(observation, Ok(Exit::Normal), Instant::now()),
        };
        let fact_id = fact.id();
        let observation_before = monitor.observation();
        match monitor.on_path(fact) {
            Ok(_) => {}
            Err(TerminationMonitorError::UnexpectedFact { observation, fact }) => {
                assert_eq!(observation, observation_before);
                assert_eq!(fact.id(), fact_id);
            }
            Err(TerminationMonitorError::Inner(never)) => match never {},
        }
        assert!(monitor.base().terminals <= 1);

        let member = MailAddr(u64::from(b % 8));
        match a % 3 {
            0 => {
                router
                    .receive(
                        MailAddr(0),
                        RouterMessage::Add(Recipient::global(member)),
                    )
                    .expect("membership addition is infallible");
                if !eligible.contains(&member) {
                    eligible.push(member);
                }
            }
            1 => {
                router
                    .receive(
                        MailAddr(0),
                        RouterMessage::Remove(Recipient::global(member)),
                    )
                    .expect("membership removal is infallible");
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
                assert!(matches!(
                    router.receive(MailAddr(0), RouterMessage::Route(b)),
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
