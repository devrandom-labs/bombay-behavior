//! Adversarial sequence invariants for ownership-carrying routing actors.

use core::num::NonZeroU64;
use std::collections::VecDeque;

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, Buffer, BufferConfiguration, BufferMessage,
    BufferOutcome, BufferRejection, MailAddr, MessageProtocol, Never, NoBirths, OverflowPolicy,
    PriorityQueue, PriorityQueueMessage, PriorityQueueOutcome, PriorityQueueRejection,
    RateLimitRejection, RateLimiter, RateLimiterMessage, RateLimiterOutcome, Recipient, RoundRobin,
    Router, RouterError, RouterMessage, TokenCount, User, WorkQueue, WorkQueueMessage,
    WorkQueueOutcome, WorkQueueRejection,
};
use proptest::collection::vec;
use proptest::prelude::*;

macro_rules! protocol {
    ($name:ident, $message:ty) => {
        struct $name;
        impl behavior::Protocol for $name {
            type Addr = MailAddr;
            type Msg = $message;
        }
        impl Behavior for $name {
            type Protocol = Self;
            type Event = User<MailAddr, $message>;
            type Sends = Vec<Never>;
            type Ph = Never;
            type Error = Never;
            type Birth = NoBirths;
            fn transition(
                &mut self,
                _: behavior::ActiveTurn,
                _: Self::Event,
            ) -> BehaviorActed<Self> {
                Ok(Actions::cont())
            }
        }
    };
}

protocol!(PriorityTarget, u8);
protocol!(PriorityReply, PriorityQueueOutcome<u8, u8>);
protocol!(RateTarget, u8);
protocol!(RateReply, RateLimiterOutcome<u8>);
protocol!(QueueWorker, u8);
protocol!(QueueReply, WorkQueueOutcome<u8>);

type TestBuffer = Buffer<
    MailAddr,
    u8,
    Recipient<MessageProtocol<MailAddr, u8>>,
    Recipient<MessageProtocol<MailAddr, BufferOutcome<u8>>>,
>;
type TestPriority =
    PriorityQueue<MailAddr, u8, u8, Recipient<PriorityTarget>, Recipient<PriorityReply>>;
type TestRate = RateLimiter<MailAddr, u8, Recipient<RateTarget>, Recipient<RateReply>>;
type TestQueue = WorkQueue<MailAddr, u8, Recipient<QueueWorker>, Recipient<QueueReply>>;
type TestRouter = Router<MailAddr, Recipient<PriorityTarget>, RoundRobin>;

fn overflow(tag: u8) -> OverflowPolicy {
    match tag % 3 {
        0 => OverflowPolicy::Reject,
        1 => OverflowPolicy::DropOldest,
        _ => OverflowPolicy::DropNewest,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 384,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn buffer_preserves_fifo_and_returns_every_unaccepted_value(
        capacity in 1_usize..8,
        policy_tag in any::<u8>(),
        operations in vec((any::<bool>(), any::<u8>()), 0..200),
    ) {
        let policy = overflow(policy_tag);
        let mut actual = TestBuffer::new(BufferConfiguration::new(capacity, policy).unwrap())
            .initialize().unwrap().behavior;
        let mut expected = VecDeque::new();
        let outcome = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for (offer, value) in operations {
            let actions = if offer {
                actual.receive(MailAddr(9), BufferMessage::Offer { value, reply_to: outcome }).unwrap()
            } else {
                actual.receive(MailAddr(9), BufferMessage::Release { to: target, reply_to: outcome }).unwrap()
            };
            let mut returned = Vec::new();
            for delivery in &actions.sends.outcomes {
                match &delivery.message {
                    BufferOutcome::Rejected { value, .. } | BufferOutcome::Evicted { value } => returned.push(*value),
                    _ => {}
                }
            }

            if offer {
                if expected.len() < capacity {
                    expected.push_back(value);
                    prop_assert!(returned.is_empty());
                } else {
                    match policy {
                        OverflowPolicy::Reject => {
                            prop_assert_eq!(returned, vec![value]);
                            let matched = matches!(actions.sends.outcomes[0].message,
                                BufferOutcome::Rejected { reason: BufferRejection::Full, .. });
                            prop_assert!(matched);
                        }
                        OverflowPolicy::DropNewest => {
                            prop_assert_eq!(returned, vec![value]);
                            let matched = matches!(actions.sends.outcomes[0].message,
                                BufferOutcome::Rejected { reason: BufferRejection::DroppedNewest, .. });
                            prop_assert!(matched);
                        }
                        OverflowPolicy::DropOldest => {
                            let evicted = expected.pop_front().unwrap();
                            expected.push_back(value);
                            prop_assert_eq!(returned, vec![evicted]);
                            prop_assert_eq!(actions.sends.outcomes.len(), 2);
                        }
                    }
                }
                prop_assert!(actions.sends.deliveries.is_empty());
            } else if let Some(released) = expected.pop_front() {
                prop_assert_eq!(actions.sends.deliveries.len(), 1);
                prop_assert_eq!(actions.sends.deliveries[0].message, released);
                prop_assert!(returned.is_empty());
            } else {
                prop_assert!(actions.sends.deliveries.is_empty());
                prop_assert!(matches!(actions.sends.outcomes[0].message, BufferOutcome::Empty));
            }

            let retained = actual.state().queued().map(|entry| entry.value).collect::<Vec<_>>();
            prop_assert_eq!(retained, expected.iter().copied().collect::<Vec<_>>());
            prop_assert!(actual.state().len() <= capacity);
        }
    }

    #[test]
    fn priority_queue_matches_stable_max_priority_selection(
        capacity in 1_usize..8,
        operations in vec((any::<bool>(), any::<u8>(), 0_u8..8), 0..200),
    ) {
        let mut actual = TestPriority::new(capacity).unwrap().initialize().unwrap().behavior;
        let mut expected: Vec<(u8, u8, u64)> = Vec::new();
        let mut order = 0_u64;
        let reply = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for (offer, value, priority) in operations {
            let actions = if offer {
                actual.receive(MailAddr(9), PriorityQueueMessage::Offer { value, priority, reply_to: reply }).unwrap()
            } else {
                actual.receive(MailAddr(9), PriorityQueueMessage::Release { to: target, reply_to: reply }).unwrap()
            };
            if offer {
                if expected.len() == capacity {
                    prop_assert!(actions.sends.deliveries.is_empty());
                    let matched = matches!(actions.sends.outcomes[0].message,
                        PriorityQueueOutcome::Rejected { value: returned, priority: returned_priority, reason: PriorityQueueRejection::Full }
                            if returned == value && returned_priority == priority);
                    prop_assert!(matched);
                } else {
                    expected.push((value, priority, order));
                    order += 1;
                    let matched = matches!(actions.sends.outcomes[0].message, PriorityQueueOutcome::Accepted { .. });
                    prop_assert!(matched);
                }
            } else if expected.is_empty() {
                prop_assert!(actions.sends.deliveries.is_empty());
                prop_assert!(matches!(actions.sends.outcomes[0].message, PriorityQueueOutcome::Empty));
            } else {
                let selected = expected.iter().enumerate().max_by(|(_, left), (_, right)| {
                    left.1.cmp(&right.1).then_with(|| right.2.cmp(&left.2))
                }).unwrap().0;
                let released = expected.remove(selected).0;
                prop_assert_eq!(actions.sends.deliveries[0].message, released);
            }
            let queued = match actual.state() {
                behavior::PriorityQueueState::Active { queued, .. }
                | behavior::PriorityQueueState::Exhausted { queued } => queued,
            };
            prop_assert_eq!(queued, expected.len());
        }
    }

    #[test]
    fn rate_limiter_matches_saturating_token_arithmetic(
        capacity in 1_u64..32,
        initial_seed in 0_u64..64,
        operations in vec((any::<bool>(), 1_u64..48, any::<u8>()), 0..200),
    ) {
        let initial = initial_seed % (capacity + 1);
        let capacity_tokens = TokenCount::new(NonZeroU64::new(capacity).unwrap());
        let mut actual = TestRate::new(capacity_tokens, initial).unwrap().initialize().unwrap().behavior;
        let mut available = initial;
        let reply = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for (acquire, amount, value) in operations {
            let tokens = TokenCount::new(NonZeroU64::new(amount).unwrap());
            if acquire {
                let actions = actual.receive(MailAddr(9), RateLimiterMessage::Acquire {
                    cost: tokens, value, to: target, reply_to: reply,
                }).unwrap();
                if amount > capacity {
                    let matched = matches!(actions.sends.outcomes[0].message,
                        RateLimiterOutcome::Rejected { cost: returned_cost, value: returned, reason: RateLimitRejection::ExceedsCapacity }
                            if returned == value && returned_cost == tokens);
                    prop_assert!(matched);
                    prop_assert!(actions.sends.deliveries.is_empty());
                } else if amount > available {
                    let matched = matches!(actions.sends.outcomes[0].message,
                        RateLimiterOutcome::Rejected { cost: returned_cost, value: returned, reason: RateLimitRejection::InsufficientTokens }
                            if returned == value && returned_cost == tokens);
                    prop_assert!(matched);
                    prop_assert!(actions.sends.deliveries.is_empty());
                } else {
                    available -= amount;
                    prop_assert_eq!(actions.sends.deliveries[0].message, value);
                    prop_assert_eq!(&actions.sends.outcomes[0].message, &RateLimiterOutcome::Admitted { remaining: available });
                }
            } else {
                actual.receive(MailAddr(9), RateLimiterMessage::Refill { tokens }).unwrap();
                available = available.saturating_add(amount).min(capacity);
            }
            prop_assert_eq!(actual.state().available(), available);
            prop_assert!(available <= capacity);
        }
    }

    #[test]
    fn work_queue_matches_two_coupled_fifo_capabilities(
        capacity in 0_usize..7,
        operations in vec((0_u8..3, any::<u8>(), 0_u8..8), 0..220),
    ) {
        let mut actual = TestQueue::new(capacity).initialize().unwrap().behavior;
        let mut waiting = VecDeque::new();
        let mut available: VecDeque<Recipient<QueueWorker>> = VecDeque::new();
        let reply = Recipient::global(MailAddr(1));

        for (operation, value, worker_seed) in operations {
            let worker = Recipient::global(MailAddr(u64::from(worker_seed)));
            let actions = match operation {
                0 => actual.receive(MailAddr(9), WorkQueueMessage::Submit { value, reply_to: reply }).unwrap(),
                1 => actual.receive(MailAddr(9), WorkQueueMessage::Available { worker }).unwrap(),
                _ => actual.receive(MailAddr(9), WorkQueueMessage::Withdraw { worker }).unwrap(),
            };
            match operation {
                0 if !available.is_empty() => {
                    let selected = available.pop_front().unwrap();
                    prop_assert_eq!(actions.sends.assignments[0].to, selected);
                    prop_assert_eq!(actions.sends.assignments[0].message, value);
                }
                0 if waiting.len() < capacity => {
                    waiting.push_back(value);
                    prop_assert!(actions.sends.assignments.is_empty());
                    prop_assert_eq!(&actions.sends.outcomes[0].message, &WorkQueueOutcome::Queued { depth: waiting.len() });
                }
                0 => {
                    let matched = matches!(actions.sends.outcomes[0].message,
                        WorkQueueOutcome::Rejected { value: returned, reason: WorkQueueRejection::Full }
                            if returned == value);
                    prop_assert!(matched);
                }
                1 if !waiting.is_empty() => {
                    let assigned = waiting.pop_front().unwrap();
                    prop_assert_eq!(actions.sends.assignments[0].to, worker);
                    prop_assert_eq!(actions.sends.assignments[0].message, assigned);
                }
                1 => {
                    if !available.contains(&worker) { available.push_back(worker); }
                    prop_assert!(actions.sends.assignments.is_empty());
                }
                _ => available.retain(|candidate| *candidate != worker),
            }
            let state = actual.state();
            prop_assert_eq!(state.available(), available.make_contiguous());
            prop_assert_eq!(state.queued(), waiting.len());
            prop_assert!(waiting.len() <= capacity);
        }
    }

    #[test]
    fn round_robin_keeps_the_same_next_recipient_across_membership_edits(
        operations in vec((0_u8..3, 0_u8..10, any::<u8>()), 0..220),
    ) {
        let mut actual = TestRouter::new(Vec::new(), RoundRobin::default())
            .initialize().unwrap().behavior;
        let mut members: Vec<Recipient<PriorityTarget>> = Vec::new();
        let mut next: Option<Recipient<PriorityTarget>> = None;

        for (operation, address, value) in operations {
            let recipient = Recipient::global(MailAddr(u64::from(address)));
            match operation {
                0 => {
                    actual.receive(MailAddr(9), RouterMessage::Add(recipient)).unwrap();
                    if !members.contains(&recipient) {
                        members.push(recipient);
                        if next.is_none() { next = Some(recipient); }
                    }
                }
                1 => {
                    actual.receive(MailAddr(9), RouterMessage::Remove(recipient)).unwrap();
                    if let Some(index) = members.iter().position(|candidate| *candidate == recipient) {
                        let removed_was_next = next == Some(recipient);
                        members.remove(index);
                        if members.is_empty() {
                            next = None;
                        } else if removed_was_next {
                            next = Some(members[index % members.len()]);
                        }
                    }
                }
                _ if members.is_empty() => {
                    let result = actual.receive(MailAddr(9), RouterMessage::Route(value));
                    prop_assert!(matches!(result, Err(RouterError::NoEligibleRecipients(returned)) if returned == value));
                }
                _ => {
                    let selected = next.unwrap();
                    let actions = actual.receive(MailAddr(9), RouterMessage::Route(value)).unwrap();
                    prop_assert_eq!(actions.sends.len(), 1);
                    prop_assert_eq!(actions.sends[0].to, selected);
                    prop_assert_eq!(actions.sends[0].message, value);
                    let index = members.iter().position(|candidate| *candidate == selected).unwrap();
                    next = Some(members[(index + 1) % members.len()]);
                }
            }
            prop_assert_eq!(actual.recipients(), members.as_slice());
        }
    }
}
