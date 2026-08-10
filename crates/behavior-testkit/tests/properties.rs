use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, ChildStopped, Crash, CreationKind, CreationResolved, Deadline,
    Delivery, Exit, Handler, MailAddr, Never, Proxy, ProxyCommand, ProxyEvent, Pure, Recipient,
    RestartPolicy, Route, Step, Strategy, SupervisionEvent, Supervisor, TimerId, User, UserEvent,
    WorkerStopped,
};
use behavior_testkit::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;
use tokio::time::Instant;

#[derive(Default)]
struct Echo;

impl Handler<u8, behavior::NoBirths, Never> for Echo {
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
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

type Child = Pure<Echo, u8>;

struct Parent;

impl Handler<Never, behavior::Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
    {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Pure::new(Echo)
}

fn supervisor(
    strategy: Strategy,
    count: usize,
) -> Supervisor<Pure<Parent, Never, behavior::Births<Child>>, Child> {
    Supervisor::new(
        Pure::new(Parent),
        |index| u64::try_from(index).unwrap(),
        count,
        child,
        strategy,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    )
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
        let mut behavior = Pure::new(Echo);
        let trace = drive(&mut behavior, &mut mailbox).unwrap();
        let expected_len = messages
            .iter()
            .position(|message| *message == u8::MAX)
            .map_or(messages.len(), |index| index + 1);

        prop_assert_eq!(trace.sends.len(), expected_len);
        prop_assert_eq!(trace.pending, messages.len() - expected_len);
        for (index, delivery) in trace.sends.iter().enumerate() {
            prop_assert_eq!(delivery.message, messages[index]);
            prop_assert_eq!(delivery.to.route(), Route::Global(MailAddr(u64::try_from(index).unwrap())));
        }
    }

    #[test]
    fn nested_time_initialization_is_a_lossless_product(
        offsets in vec(0_u64..1_000_000, 1..32)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(offsets[0]);
        let mut one = Deadline::new(Pure::new(Echo), TimerId(0), Some(first), |_| Ok(Step::Continue));
        let initial = one.init().unwrap();
        prop_assert_eq!(initial.sends.schedules.len(), 1);
        prop_assert_eq!(initial.sends.schedules[0].at, first);

        for offset in &offsets {
            let due = origin + Duration::from_nanos(*offset);
            let mut composed = Deadline::new(Pure::new(Echo), TimerId(0), Some(due), |_| Ok(Step::Continue));
            let actions = composed.init().unwrap();
            prop_assert_eq!(actions.sends.schedules[0].at, due);
        }
    }

    #[test]
    fn proxy_generation_model_matches_arbitrary_command_sequences(
        commands in vec(any::<bool>(), 0..256)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut proxy = Proxy::new(child(0));
        let initial = proxy.init().unwrap();
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
                    .transition(ProxyEvent::Inner(User::user(
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
                    .transition(ProxyEvent::Inner(User::user(
                        MailAddr(0),
                        ProxyCommand::Forward(message),
                    )))
                    .unwrap();
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(actions.sends.deliveries[0].to.route(), Route::Child(generation));
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
        let mut behavior = supervisor(strategy, count);
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
            prop_assert!(matches!(delivery.to.route(), Route::Child(_)));
        }
    }
}
