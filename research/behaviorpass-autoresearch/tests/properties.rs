use std::time::Duration;

use behaviorpass::{
    Acted, Actions, At, Base, Behavior, ChildStopped, Crash, Delivery, Exit, MailAddr, Never,
    Proxy, ProxyCommand, Recipient, RestartPolicy, Route, State, Step, Strategy, Supervising,
    SupervisionEvent, User, UserEvent,
};
use behaviorpass_autoresearch::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;
use tokio::time::Instant;

#[derive(Default)]
struct Echo;

impl State<u8, Never, Never> for Echo {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
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

type Child = Base<Echo, u8>;

struct Parent;

impl State<Never, Child, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Child, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Base::new(Echo)
}

fn supervisor(strategy: Strategy, count: usize) -> Supervising<Base<Parent, Never, Child>, Child> {
    Supervising::new(
        Base::new(Parent),
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
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let events = messages
            .iter()
            .enumerate()
            .map(|(index, message)| User::user(MailAddr(u64::try_from(index).unwrap()), *message));
        let mut mailbox = Mailbox::new(events);
        let mut behavior = Base::new(Echo);
        let trace = runtime.block_on(drive(&mut behavior, &mut mailbox)).unwrap();
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
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(offsets[0]);
        let mut one = At::new(Base::new(Echo), Some(first), |_| Ok(Step::Continue));
        let initial = runtime.block_on(one.init()).unwrap();
        prop_assert_eq!(initial.sends.own.len(), 1);
        prop_assert_eq!(initial.sends.own[0].message.at, first);

        for offset in &offsets {
            let due = origin + Duration::from_nanos(*offset);
            let mut composed = At::new(Base::new(Echo), Some(due), |_| Ok(Step::Continue));
            let actions = runtime.block_on(composed.init()).unwrap();
            prop_assert_eq!(actions.sends.own[0].message.at, due);
        }
    }

    #[test]
    fn proxy_generation_model_matches_arbitrary_command_sequences(
        commands in vec(any::<bool>(), 0..256)
    ) {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut proxy = Proxy::new(child(0));
        let initial = runtime.block_on(proxy.init()).unwrap();
        prop_assert_eq!(initial.creates[0].nonce, 0);
        let mut generation = 0_u64;

        for (index, replace) in commands.into_iter().enumerate() {
            if replace {
                generation += 1;
                let actions = runtime.block_on(proxy.step(User::user(
                    MailAddr(0),
                    ProxyCommand::Replace(child(index)),
                ))).unwrap();
                prop_assert_eq!(actions.creates.len(), 1);
                prop_assert_eq!(actions.creates[0].nonce, generation);
                prop_assert!(actions.sends.is_empty());
            } else {
                let message = u8::try_from(index % 255).unwrap();
                let actions = runtime.block_on(proxy.step(User::user(
                    MailAddr(0),
                    ProxyCommand::Forward(message),
                ))).unwrap();
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(actions.sends[0].to.route(), Route::Child(generation));
                prop_assert_eq!(actions.sends[0].message, message);
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
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut behavior = supervisor(strategy, count);
        let event = SupervisionEvent::ChildStopped(ChildStopped {
            nonce: u64::try_from(dead).unwrap(),
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let actions = runtime.block_on(behavior.step(event)).unwrap();

        prop_assert_eq!(actions.sends.own.own.len(), expected);
        prop_assert!(actions.creates.is_empty());
        for delivery in actions.sends.own.own {
            prop_assert!(matches!(delivery.to.route(), Route::Child(_)));
        }
    }
}
