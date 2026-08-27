use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Births, ChildStopped, Crash,
    CreationKind, CreationResolved, Delivery, Exit, MailAddr, Never, Proxy, ProxyEvent, Recipient,
    ReplacementRequested, RestartPolicy, Step, Strategy, Supervise, SupervisionEvent, TimerId,
    User, UserEvent, WorkerStopped,
};
use behavior_testkit::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;
use tokio::runtime::Builder;

#[derive(Default)]
struct Echo;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: if message == u8::MAX {
                Step::Stop(behavior::Stopped)
            } else {
                Step::Continue
            },
        })
    }
}

type Child = Echo;

fn child(_index: usize) -> Child {
    Echo
}

struct SupervisorHarness;

impl behavior::Protocol for SupervisorHarness {
    type Addr = MailAddr;
    type Msg = ();
}

impl BehaviorBase for SupervisorHarness {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for SupervisorHarness {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

macro_rules! supervisor {
    ($strategy:expr, $count:expr) => {
        Supervise::new(
            SupervisorHarness,
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                $count,
                |index| Some(child(index)),
            ),
            behavior::RestartConfiguration::new(
                $strategy,
                RestartPolicy::Permanent,
                u32::MAX,
                Duration::MAX,
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap()
    };
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn fifo_driver_matches_a_prefix_model(messages in vec(any::<u8>(), 0..256)) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let events = messages
            .iter()
            .enumerate()
            .map(|(index, message)| User::user(MailAddr(u64::try_from(index).unwrap()), *message));
        let mut mailbox = Mailbox::new(events);
        let behavior = Echo;
        let trace = drive(behavior, &mut mailbox).unwrap();
        let expected_len = messages
            .iter()
            .position(|message| *message == u8::MAX)
            .map_or(messages.len(), |index| index + 1);

        prop_assert_eq!(trace.sends.len(), expected_len);
        prop_assert_eq!(trace.pending, messages.len() - expected_len);
        for (index, delivery) in trace.sends.iter().enumerate() {
            prop_assert_eq!(delivery.message, messages[index]);
            prop_assert_eq!(delivery.to.address(), MailAddr(u64::try_from(index).unwrap()));
        }
    }

    #[test]
    fn nested_time_initialization_is_a_lossless_product(
        offsets in vec(0_u64..1_000_000, 1..32)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(offsets[0]);
        let one = behavior::Deadline::new(Echo, TimerId(0), Some(first), |_| Step::Continue);
        let initialized = one.initialize().unwrap();
    let initial = initialized.actions;
    let _one = initialized.behavior;
        prop_assert_eq!(initial.sends.owned.len(), 1);
        prop_assert_eq!(initial.sends.owned[0].at, first);

        for offset in &offsets {
            let due = origin + Duration::from_nanos(*offset);
            let composed = behavior::Deadline::new(Echo, TimerId(0), Some(due), |_| Step::Continue);
            let initialized = composed.initialize().unwrap();
    let actions = initialized.actions;
    let _composed = initialized.behavior;
            prop_assert_eq!(actions.sends.owned[0].at, due);
        }
    }

    #[test]
    fn proxy_generation_model_matches_arbitrary_command_sequences(
        commands in vec(any::<bool>(), 0..256)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let proxy = Proxy::new(child(0));
        let initialized = proxy.initialize().unwrap();
    let initial = initialized.actions;
    let mut proxy = initialized.behavior;
        prop_assert_eq!(initial.creates[0].nonce, 0);
        prop_assert_eq!(initial.creates[0].kind, CreationKind::Birth);
        let installed = proxy
            .transition(ProxyEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(MailAddr(999)),
            }))
            .unwrap();
        prop_assert!(installed.sends.deliveries.is_empty());
        prop_assert!(installed.sends.unavailable_reports.is_empty());
        prop_assert!(installed.sends.child_observations.is_empty());
        prop_assert!(installed.sends.creation_observations.is_empty());
        prop_assert!(installed.sends.stopped_reports.is_empty());
        prop_assert_eq!(installed.sends.creation_reports.len(), 1);
        prop_assert!(installed.sends.shutdowns.is_empty());
        prop_assert!(installed.creates.is_empty());
        prop_assert!(matches!(installed.become_, Step::Continue));
        let mut generation = 0_u64;

        for (index, replace) in commands.into_iter().enumerate() {
            if replace {
                generation += 1;
                let actions = proxy
                    .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
                        child(index),
                    )))
                    .unwrap();
                prop_assert!(actions.creates.is_empty());
                let actions = proxy.transition(ProxyEvent::ChildStopped(
                    ChildStopped {
                        nonce: generation - 1,
                        outcome: Ok(Exit::Normal),
                        at: Instant::now(),
                    },
                ))
                .unwrap();
                prop_assert_eq!(actions.creates.len(), 1);
                prop_assert_eq!(actions.creates[0].nonce, generation);
                prop_assert_eq!(
                    actions.creates[0].kind,
                    CreationKind::ReplacementIncarnation {
                        replaces: generation - 1,
                    }
                );
                prop_assert!(actions.sends.deliveries.is_empty());
                let installed = proxy
                    .transition(ProxyEvent::CreationResolved(CreationResolved {
                        nonce: generation,
                        kind: CreationKind::ReplacementIncarnation {
                            replaces: generation - 1,
                        },
                        result: Ok(MailAddr(999)),
                    }))
                    .unwrap();
                prop_assert!(installed.sends.deliveries.is_empty());
                prop_assert!(installed.sends.unavailable_reports.is_empty());
                prop_assert!(installed.sends.child_observations.is_empty());
                prop_assert!(installed.sends.creation_observations.is_empty());
                prop_assert!(installed.sends.stopped_reports.is_empty());
                prop_assert_eq!(installed.sends.creation_reports.len(), 1);
                prop_assert!(installed.sends.shutdowns.is_empty());
                prop_assert!(installed.creates.is_empty());
                prop_assert!(matches!(installed.become_, Step::Continue));
            } else {
                let message = u8::try_from(index % 255).unwrap();
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(MailAddr(0), message)))
                    .unwrap();
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(
                    actions.sends.deliveries[0].nonce,
                    generation
                );
                prop_assert_eq!(actions.sends.deliveries[0].message, message);
            }
        }
    }

    #[test]
    fn supervision_strategy_matches_independent_candidate_model(
        count in 1_usize..64,
        dead_seed in any::<usize>(),
        strategy_tag in 0_u8..3,
    ) {
        let dead = dead_seed % count;
        let strategy = match strategy_tag {
            0 => Strategy::OneForOne,
            1 => Strategy::OneForAll,
            _ => Strategy::RestForOne,
        };
        let expected = match strategy {
            Strategy::OneForOne => 1,
            Strategy::OneForAll => count,
            Strategy::RestForOne => count - dead,
        };
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = supervisor!(strategy, count).initialize().unwrap();
        let mut behavior = initialized.behavior;
        for proxy in 0..count {
            let proxy = u64::try_from(proxy).unwrap();
            let joined = behavior
                .transition(SupervisionEvent::WorkerCreationResolved(
                    behavior::WorkerCreationResolved::new(
                        proxy,
                        proxy,
                        CreationKind::Birth,
                        Ok(()),
                    ),
                ))
                .unwrap();
            prop_assert!(joined.sends.owned.child_observations.is_empty());
            prop_assert!(joined.sends.owned.creation_observations.is_empty());
            prop_assert!(joined.sends.owned.schedules.is_empty());
            prop_assert!(joined.sends.owned.replacement_inputs.is_empty());
            prop_assert!(joined.sends.owned.failure_reports.is_empty());
            prop_assert!(joined.sends.owned.shutdowns.is_empty());
            prop_assert!(joined.sends.inner.is_empty());
            prop_assert!(joined.creates.is_empty());
            prop_assert!(matches!(joined.become_, Step::Continue));
        }
        let event = SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: u64::try_from(dead).unwrap(),
            worker: u64::try_from(dead).unwrap(),
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let actions = behavior.transition(event).unwrap();

        prop_assert_eq!(actions.sends.owned.replacement_inputs.len(), expected);
        prop_assert!(actions.creates.is_empty());
        for delivery in actions.sends.owned.replacement_inputs {
            prop_assert!(delivery.nonce < u64::try_from(count).unwrap());
        }
    }
}
use behavior_testkit::InitializeTest;
