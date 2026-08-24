//! Independent timer-generation model for the stateful timing catalogue.

use std::time::Duration;

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, Lease, LeaseMessage, LeaseOutcome,
    LeaseRejection, LeaseRequest, LeaseState, MailAddr, Never, NoBirths, Recipient, TimerElapsed,
    TimerGeneration, TimerId, User,
};
use proptest::collection::vec;
use proptest::prelude::*;

struct Reply;
impl behavior::Protocol for Reply {
    type Addr = MailAddr;
    type Msg = LeaseOutcome<u8>;
}
impl Behavior for Reply {
    type Protocol = Self;
    type Event = User<MailAddr, LeaseOutcome<u8>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type Subject = Lease<MailAddr, u8, Reply, Recipient<Reply>>;

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, max_shrink_iters: 100_000, ..ProptestConfig::default() })]

    #[test]
    fn lease_matches_exclusive_generation_ownership_after_every_event(
        operations in vec((0_u8..4, 0_u8..5, 0_u64..24, any::<bool>()), 0..220),
    ) {
        let timer = TimerId(7);
        let mut actual = Subject::new(timer).initialize().unwrap().behavior;
        let mut held: Option<(u8, u64)> = None;
        let mut next = 0_u64;
        let reply = Recipient::global(MailAddr(1));

        for (operation, holder, observed, wrong_timer) in operations {
            let duration = Duration::from_nanos(1);
            let actions = match operation {
                0 => actual.receive(MailAddr(9), LeaseMessage::Acquire {
                    holder, duration, reply_to: reply,
                }).unwrap(),
                1 => actual.receive(MailAddr(9), LeaseMessage::Renew {
                    holder, generation: TimerGeneration(observed),
                    duration, reply_to: reply,
                }).unwrap(),
                2 => actual.receive(MailAddr(9), LeaseMessage::Release {
                    holder, generation: TimerGeneration(observed), reply_to: reply,
                }).unwrap(),
                _ => actual.on(TimerElapsed::new(
                    if wrong_timer { TimerId(8) } else { timer },
                    TimerGeneration(observed),
                )).unwrap(),
            };

            match operation {
                0 => match held {
                    Some((current, _)) => {
                        prop_assert_eq!(&actions.sends.outcomes[0].message,
                            &LeaseOutcome::Rejected {
                                request: LeaseRequest::Acquire { holder, duration },
                                reason: LeaseRejection::Occupied { current },
                            });
                        prop_assert!(actions.sends.schedules.is_empty());
                    }
                    None => {
                        let generation = next;
                        next += 1;
                        held = Some((holder, generation));
                        prop_assert_eq!(&actions.sends.outcomes[0].message,
                            &LeaseOutcome::Acquired { holder, generation: TimerGeneration(generation) });
                        prop_assert_eq!(actions.sends.schedules.as_slice()[0].generation, TimerGeneration(generation));
                    }
                },
                1 => match held {
                    None => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Renew { holder, generation: TimerGeneration(observed), duration },
                            reason: LeaseRejection::Vacant,
                        }),
                    Some((current, _)) if current != holder => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Renew { holder, generation: TimerGeneration(observed), duration },
                            reason: LeaseRejection::WrongHolder { current },
                        }),
                    Some((_, generation)) if generation != observed => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Renew { holder, generation: TimerGeneration(observed), duration },
                            reason: LeaseRejection::StaleGeneration {
                                observed: TimerGeneration(observed), current: TimerGeneration(generation),
                            },
                        }),
                    Some(_) => {
                        let generation = next;
                        next += 1;
                        held = Some((holder, generation));
                        prop_assert_eq!(&actions.sends.outcomes[0].message,
                            &LeaseOutcome::Renewed { holder, generation: TimerGeneration(generation) });
                        prop_assert_eq!(actions.sends.schedules.as_slice()[0].generation, TimerGeneration(generation));
                    }
                },
                2 => match held {
                    None => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Release { holder, generation: TimerGeneration(observed) },
                            reason: LeaseRejection::Vacant,
                        }),
                    Some((current, _)) if current != holder => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Release { holder, generation: TimerGeneration(observed) },
                            reason: LeaseRejection::WrongHolder { current },
                        }),
                    Some((_, generation)) if generation != observed => prop_assert_eq!(&actions.sends.outcomes[0].message,
                        &LeaseOutcome::Rejected {
                            request: LeaseRequest::Release { holder, generation: TimerGeneration(observed) },
                            reason: LeaseRejection::StaleGeneration {
                                observed: TimerGeneration(observed), current: TimerGeneration(generation),
                            },
                        }),
                    Some((current, generation)) => {
                        held = None;
                        prop_assert_eq!(&actions.sends.outcomes[0].message,
                            &LeaseOutcome::Released { holder: current, generation: TimerGeneration(generation) });
                    }
                },
                _ => match held {
                    Some((current, generation)) if !wrong_timer && observed == generation => {
                        held = None;
                        prop_assert_eq!(&actions.sends.outcomes[0].message,
                            &LeaseOutcome::Expired { holder: current, generation: TimerGeneration(generation) });
                    }
                    _ => {
                        prop_assert!(actions.sends.outcomes.is_empty());
                        prop_assert!(actions.sends.schedules.is_empty());
                    }
                },
            }

            match (actual.state(), held) {
                (LeaseState::Vacant { .. }, None) => {}
                (LeaseState::Held { holder: actual_holder, generation, .. }, Some((modeled_holder, modeled_generation))) => {
                    prop_assert_eq!(*actual_holder, modeled_holder);
                    prop_assert_eq!(*generation, TimerGeneration(modeled_generation));
                }
                _ => prop_assert!(false, "lease state diverged from independent model"),
            }
        }
    }
}
