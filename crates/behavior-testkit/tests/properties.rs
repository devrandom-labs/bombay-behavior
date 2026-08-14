use std::time::Duration;

use behavior::{
    Acted, Actions, ChildStopped, Compose, Crash, CreationKind, CreationResolved, Delivery, Exit,
    MailAddr, Never, Proxy, ProxyCommand, ProxyEvent, Recipient, RestartPolicy, Step, Strategy,
    SupervisionEvent, Supervisor, TimerId, User, UserEvent, WorkerStopped,
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
                Step::Stop(Exit::Normal)
            } else {
                Step::Continue
            },
        })
    }
}

type Child = Echo;

struct Parent;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Echo
}

fn supervisor(strategy: Strategy, count: usize) -> Supervisor<Parent, Child> {
    Supervisor::new(
        Parent,
        |index| u64::try_from(index).unwrap(),
        count,
        |index| Some(child(index)),
        strategy,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    )
    .unwrap()
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
        let behavior = behavior::Compose::new(Echo);
        let trace = drive(behavior, &mut mailbox).unwrap();
        let expected_len = messages
            .iter()
            .position(|message| *message == u8::MAX)
            .map_or(messages.len(), |index| index + 1);

        prop_assert_eq!(trace.sends.len(), expected_len);
        prop_assert_eq!(trace.pending, messages.len() - expected_len);
        for (index, delivery) in trace.sends.iter().enumerate() {
            prop_assert_eq!(delivery.message, messages[index]);
            prop_assert_eq!(delivery.to.resolve(MailAddr(999)), MailAddr(u64::try_from(index).unwrap()));
        }
    }

    #[test]
    fn nested_time_initialization_is_a_lossless_product(
        offsets in vec(0_u64..1_000_000, 1..32)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(offsets[0]);
        let one = Compose::new(Echo).deadline(TimerId(0), Some(first), |_| Ok(Step::Continue));
        let initialized = one.initialize().unwrap();
    let initial = initialized.actions;
    let _one = initialized.behavior;
        prop_assert_eq!(initial.sends.schedules.len(), 1);
        prop_assert_eq!(initial.sends.schedules[0].at, first);

        for offset in &offsets {
            let due = origin + Duration::from_nanos(*offset);
            let composed = Compose::new(Echo).deadline(TimerId(0), Some(due), |_| Ok(Step::Continue));
            let initialized = composed.initialize().unwrap();
    let actions = initialized.actions;
    let _composed = initialized.behavior;
            prop_assert_eq!(actions.sends.schedules[0].at, due);
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
        proxy
            .transition(ProxyEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(()),
            }))
            .unwrap();
        let mut generation = 0_u64;

        for (index, replace) in commands.into_iter().enumerate() {
            if replace {
                generation += 1;
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(
                        MailAddr(0),
                        ProxyCommand::Replace(child(index)),
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
                proxy
                    .transition(ProxyEvent::CreationResolved(CreationResolved {
                        nonce: generation,
                        kind: CreationKind::ReplacementIncarnation {
                            replaces: generation - 1,
                        },
                        result: Ok(()),
                    }))
                    .unwrap();
            } else {
                let message = u8::try_from(index % 255).unwrap();
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(
                        MailAddr(0),
                        ProxyCommand::Forward(message),
                    )))
                    .unwrap();
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(
                    actions.sends.deliveries[0].to.resolve(MailAddr(17)),
                    behavior::Address::birth(MailAddr(17), generation)
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
        let initialized = supervisor(strategy, count).initialize().unwrap();
        let mut behavior = initialized.behavior;
        let event = SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: u64::try_from(dead).unwrap(),
            worker: u64::try_from(dead).unwrap(),
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let actions = behavior.transition(event).unwrap();

        prop_assert_eq!(actions.sends.replacement_commands.len(), expected);
        prop_assert!(actions.creates.is_empty());
        for delivery in actions.sends.replacement_commands {
            prop_assert_ne!(
                delivery.to.resolve(MailAddr(17)),
                delivery.to.resolve(MailAddr(18))
            );
        }
    }
}
use behavior_testkit::InitializeTest;
