//! Adversarial sequence invariants for ownership-carrying routing actors.

use core::num::NonZeroU64;
use std::collections::VecDeque;

use behavior_actors::{
    Activate as _, Active, Buffer, BufferConfiguration, BufferMessage, BufferOutcome,
    BufferRejection, LeastLoaded, LeastLoadedError, Load, LoadEvidence, LoadObservation,
    LoadVersion, MemberToken, MemberTokenEvidence, MemberTokenObservation, MemberTokenVersion,
    OverflowPolicy, PriorityQueue, PriorityQueueMessage, PriorityQueueOutcome,
    PriorityQueueRejection, RateLimitRejection, RateLimiter, RateLimiterMessage,
    RateLimiterOutcome, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    TokenCount, WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection,
};

use behavior_core::{MailAddr, MessageProtocol, Recipient, Step};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestCaseResult};

macro_rules! protocol {
    ($name:ident, $message:ty) => {
        struct $name;
        impl behavior_core::Protocol for $name {
            type Addr = MailAddr;
            type Msg = $message;
        }
    };
}

protocol!(PriorityTarget, u8);
protocol!(PriorityReply, PriorityQueueOutcome<u8, u8>);
protocol!(RateTarget, u8);
protocol!(RateReply, RateLimiterOutcome<u8>);

#[derive(Debug, Eq, PartialEq)]
struct KeyedRoutingMessage {
    key: u64,
    value: u8,
}

impl RouteKey<u64> for KeyedRoutingMessage {
    fn route_key(&self) -> &u64 {
        &self.key
    }
}

protocol!(KeyedRoutingTarget, KeyedRoutingMessage);

#[derive(Debug, Eq, PartialEq)]
struct OwnedQueueWork(String);

protocol!(QueueWorker, OwnedQueueWork);
protocol!(QueueReply, WorkQueueOutcome<OwnedQueueWork>);

type TestBuffer = Buffer<
    MailAddr,
    u8,
    Recipient<MessageProtocol<MailAddr, u8>>,
    Recipient<MessageProtocol<MailAddr, BufferOutcome<u8>>>,
>;
type TestPriority =
    PriorityQueue<MailAddr, u8, u8, Recipient<PriorityTarget>, Recipient<PriorityReply>>;
type TestRate = RateLimiter<MailAddr, u8, Recipient<RateTarget>, Recipient<RateReply>>;
type TestQueue = WorkQueue<MailAddr, OwnedQueueWork, Recipient<QueueWorker>, Recipient<QueueReply>>;
type TestRouter = Router<MailAddr, Recipient<PriorityTarget>, RoundRobin>;
type TestLeastLoaded =
    Router<MailAddr, Recipient<PriorityTarget>, LeastLoaded<Recipient<PriorityTarget>>>;
type TestRendezvous = Router<
    MailAddr,
    Recipient<KeyedRoutingTarget>,
    RendezvousHash<Recipient<KeyedRoutingTarget>, u64>,
>;

#[derive(Clone, Copy, Debug)]
enum BufferTurn {
    Offer { value: u8 },
    Release,
}

#[derive(Clone, Copy, Debug)]
enum PriorityTurn {
    Offer { value: u8, priority: u8 },
    Release,
}

#[derive(Clone, Copy, Debug)]
enum RateTurn {
    Acquire { amount: u64, value: u8 },
    Refill { amount: u64 },
}

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
        operations in vec(prop_oneof![
            any::<u8>().prop_map(|value| BufferTurn::Offer { value }),
            Just(BufferTurn::Release),
        ], 0..200),
    ) {
        let policy = overflow(policy_tag);
        let mut actual = TestBuffer::new(BufferConfiguration::new(capacity, policy).unwrap())
            .initialize().unwrap().behavior;
        let mut expected = VecDeque::new();
        let outcome = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for turn in operations {
            let actions = match turn {
                BufferTurn::Offer { value } => actual.receive(MailAddr(9), BufferMessage::Offer { value, reply_to: outcome }).unwrap(),
                BufferTurn::Release => actual.receive(MailAddr(9), BufferMessage::Release { to: target, reply_to: outcome }).unwrap(),
            };
            let mut returned = Vec::new();
            for delivery in &actions.sends.outcomes {
                match &delivery.message {
                    BufferOutcome::Rejected { value, .. } | BufferOutcome::Evicted { value } => returned.push(*value),
                    _ => {}
                }
            }

            match turn {
                BufferTurn::Offer { value } => {
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
                }
                BufferTurn::Release => {
                    if let Some(released) = expected.pop_front() {
                        prop_assert_eq!(actions.sends.deliveries.len(), 1);
                        prop_assert_eq!(actions.sends.deliveries[0].message, released);
                        prop_assert!(returned.is_empty());
                    } else {
                        prop_assert!(actions.sends.deliveries.is_empty());
                        prop_assert!(matches!(actions.sends.outcomes[0].message, BufferOutcome::Empty));
                    }
                }
            }

            let retained = actual.state().queued().map(|entry| entry.value).collect::<Vec<_>>();
            prop_assert_eq!(retained, expected.iter().copied().collect::<Vec<_>>());
            prop_assert!(actual.state().len() <= capacity);
        }
    }

    #[test]
    fn priority_queue_matches_stable_max_priority_selection(
        capacity in 1_usize..8,
        operations in vec(prop_oneof![
            (any::<u8>(), 0_u8..8).prop_map(|(value, priority)| PriorityTurn::Offer { value, priority }),
            Just(PriorityTurn::Release),
        ], 0..200),
    ) {
        let mut actual = TestPriority::new(capacity).unwrap().initialize().unwrap().behavior;
        let mut expected: Vec<(u8, u8, u64)> = Vec::new();
        let mut order = 0_u64;
        let reply = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for turn in operations {
            let actions = match turn {
                PriorityTurn::Offer { value, priority } => actual.receive(MailAddr(9), PriorityQueueMessage::Offer { value, priority, reply_to: reply }).unwrap(),
                PriorityTurn::Release => actual.receive(MailAddr(9), PriorityQueueMessage::Release { to: target, reply_to: reply }).unwrap(),
            };
            match turn {
                PriorityTurn::Offer { value, priority } => {
                    prop_assert!(actions.sends.deliveries.is_empty());
                    if expected.len() == capacity {
                        let matched = matches!(actions.sends.outcomes[0].message,
                            PriorityQueueOutcome::Rejected { value: returned, priority: returned_priority, reason: PriorityQueueRejection::Full }
                                if returned == value && returned_priority == priority);
                        prop_assert!(matched);
                    } else {
                        expected.push((value, priority, order));
                        order += 1;
                        prop_assert_eq!(&actions.sends.outcomes[0].message, &PriorityQueueOutcome::Accepted { depth: expected.len() });
                    }
                }
                PriorityTurn::Release => {
                    if expected.is_empty() {
                        prop_assert!(actions.sends.deliveries.is_empty());
                        prop_assert!(matches!(actions.sends.outcomes[0].message, PriorityQueueOutcome::Empty));
                    } else {
                        let selected = expected.iter().enumerate().max_by(|(_, left), (_, right)| {
                            left.1.cmp(&right.1).then_with(|| right.2.cmp(&left.2))
                        }).unwrap().0;
                        let released = expected.remove(selected).0;
                        prop_assert_eq!(actions.sends.deliveries.len(), 1);
                        prop_assert_eq!(actions.sends.deliveries[0].to, target);
                        prop_assert_eq!(actions.sends.deliveries[0].message, released);
                        prop_assert_eq!(&actions.sends.outcomes[0].message, &PriorityQueueOutcome::Released { remaining: expected.len() });
                    }
                }
            }
            prop_assert_eq!(actions.sends.outcomes.len(), 1);
            prop_assert_eq!(actions.sends.outcomes[0].to, reply);
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, behavior_core::Step::Continue));
            let queued = match actual.state() {
                behavior_actors::PriorityQueueState::Active { queued, .. }
                | behavior_actors::PriorityQueueState::Exhausted { queued } => queued,
            };
            prop_assert_eq!(queued, expected.len());
        }
    }

    #[test]
    fn rate_limiter_matches_saturating_token_arithmetic(
        capacity in 1_u64..32,
        initial_seed in 0_u64..64,
        operations in vec(prop_oneof![
            (1_u64..48, any::<u8>()).prop_map(|(amount, value)| RateTurn::Acquire { amount, value }),
            (1_u64..48).prop_map(|amount| RateTurn::Refill { amount }),
        ], 0..200),
    ) {
        let initial = initial_seed % (capacity + 1);
        let capacity_tokens = TokenCount::new(NonZeroU64::new(capacity).unwrap());
        let mut actual = TestRate::new(capacity_tokens, initial).unwrap().initialize().unwrap().behavior;
        let mut available = initial;
        let reply = Recipient::global(MailAddr(1));
        let target = Recipient::global(MailAddr(2));

        for turn in operations {
            match turn {
                RateTurn::Acquire { amount, value } => {
                    let tokens = TokenCount::new(NonZeroU64::new(amount).unwrap());
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
                }
                RateTurn::Refill { amount } => {
                    let tokens = TokenCount::new(NonZeroU64::new(amount).unwrap());
                    let refilled = actual
                        .receive(MailAddr(9), RateLimiterMessage::Refill { tokens })
                        .unwrap();
                    prop_assert!(refilled.sends.deliveries.is_empty());
                    prop_assert!(refilled.sends.outcomes.is_empty());
                    prop_assert!(refilled.creates.is_empty());
                    prop_assert!(matches!(refilled.become_, behavior_core::Step::Continue));
                    available = available.saturating_add(amount).min(capacity);
                }
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
        let initialized = TestQueue::new(capacity).initialize().unwrap();
        prop_assert!(initialized.actions.sends.assignments.is_empty());
        prop_assert!(initialized.actions.sends.outcomes.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, behavior_core::Step::Continue));
        let mut actual = initialized.behavior;
        let mut waiting: VecDeque<(OwnedQueueWork, Recipient<QueueReply>)> = VecDeque::new();
        let mut available: VecDeque<Recipient<QueueWorker>> = VecDeque::new();

        for (turn, (operation, value, worker_seed)) in operations.into_iter().enumerate() {
            let worker = Recipient::global(MailAddr(u64::from(worker_seed)));
            let reply = Recipient::global(MailAddr(
                u64::try_from(turn).expect("generated sequence length fits a u64") + 100,
            ));
            let submitted = OwnedQueueWork(format!("work-{turn}-{value}"));
            let expected_value = OwnedQueueWork(format!("work-{turn}-{value}"));
            let actions = match operation {
                0 => actual.receive(MailAddr(9), WorkQueueMessage::Submit { value: submitted, reply_to: reply }).unwrap(),
                1 => actual.receive(MailAddr(9), WorkQueueMessage::Available { worker }).unwrap(),
                _ => actual.receive(MailAddr(9), WorkQueueMessage::Withdraw { worker }).unwrap(),
            };
            let mut expected_assignments = Vec::new();
            let mut expected_outcomes = Vec::new();
            match operation {
                0 if !available.is_empty() => {
                    let selected = available.pop_front().unwrap();
                    expected_assignments.push((selected, expected_value));
                    expected_outcomes.push((reply, WorkQueueOutcome::Dispatched { queued: waiting.len() }));
                }
                0 if waiting.len() < capacity => {
                    waiting.push_back((expected_value, reply));
                    expected_outcomes.push((reply, WorkQueueOutcome::Queued { depth: waiting.len() }));
                }
                0 => {
                    expected_outcomes.push((reply, WorkQueueOutcome::Rejected {
                        value: expected_value,
                        reason: WorkQueueRejection::Full,
                    }));
                }
                1 if !waiting.is_empty() => {
                    let (assigned, waiting_reply) = waiting.pop_front().unwrap();
                    expected_assignments.push((worker, assigned));
                    expected_outcomes.push((waiting_reply, WorkQueueOutcome::Dispatched { queued: waiting.len() }));
                }
                1 => {
                    if !available.contains(&worker) { available.push_back(worker); }
                }
                _ => available.retain(|candidate| *candidate != worker),
            }
            prop_assert_eq!(actions.sends.assignments.len(), expected_assignments.len());
            for (assignment, (recipient, assigned)) in
                actions.sends.assignments.iter().zip(expected_assignments)
            {
                prop_assert_eq!(assignment.to, recipient);
                prop_assert_eq!(&assignment.message, &assigned);
            }
            prop_assert_eq!(actions.sends.outcomes.len(), expected_outcomes.len());
            for (outcome, (recipient, expected)) in
                actions.sends.outcomes.iter().zip(expected_outcomes)
            {
                prop_assert_eq!(outcome.to, recipient);
                prop_assert_eq!(&outcome.message, &expected);
            }
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, behavior_core::Step::Continue));
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
                    let added = actual
                        .receive(MailAddr(9), RouterMessage::Add(recipient))
                        .unwrap();
                    prop_assert!(added.sends.is_empty());
                    prop_assert!(added.creates.is_empty());
                    prop_assert!(matches!(added.become_, behavior_core::Step::Continue));
                    if !members.contains(&recipient) {
                        members.push(recipient);
                        if next.is_none() { next = Some(recipient); }
                    }
                }
                1 => {
                    let removed = actual
                        .receive(MailAddr(9), RouterMessage::Remove(recipient))
                        .unwrap();
                    prop_assert!(removed.sends.is_empty());
                    prop_assert!(removed.creates.is_empty());
                    prop_assert!(matches!(removed.become_, behavior_core::Step::Continue));
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

#[derive(Clone, Copy, Debug)]
enum LoadInstruction {
    AddMember(u8),
    RemoveMember(u8),
    ReportLoad { member: u8, version: u8, load: u8 },
    RouteMessage(u8),
}

#[derive(Clone, Copy)]
struct ModeledReading {
    version: u8,
    load: u8,
}

struct ModeledMember {
    id: u8,
    evidence: Option<ModeledReading>,
}

fn load_recipient(member: u8) -> Recipient<PriorityTarget> {
    Recipient::global(MailAddr(u64::from(member) + 1))
}

fn same_load_observation(
    observation: &LoadObservation<Recipient<PriorityTarget>>,
    member: u8,
    version: u8,
    load: u8,
) -> bool {
    observation.recipient == load_recipient(member)
        && observation.version == LoadVersion(u64::from(version))
        && observation.load == Load(u64::from(load))
}

fn check_least_loaded_trace(turns: impl IntoIterator<Item = LoadInstruction>) -> TestCaseResult {
    let mut router = TestLeastLoaded::new(Vec::new(), LeastLoaded::new())
        .initialize()
        .unwrap()
        .behavior;
    let mut members: Vec<ModeledMember> = Vec::new();

    for turn in turns {
        match turn {
            LoadInstruction::AddMember(member) => {
                let actions = router
                    .receive(MailAddr(9), RouterMessage::Add(load_recipient(member)))
                    .unwrap();
                prop_assert!(actions.sends.is_empty());
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(actions.become_, Step::Continue);
                if !members.iter().any(|current| current.id == member) {
                    members.push(ModeledMember {
                        id: member,
                        evidence: None,
                    });
                }
            }
            LoadInstruction::RemoveMember(member) => {
                let actions = router
                    .receive(MailAddr(9), RouterMessage::Remove(load_recipient(member)))
                    .unwrap();
                prop_assert!(actions.sends.is_empty());
                prop_assert!(actions.creates.is_empty());
                prop_assert_eq!(actions.become_, Step::Continue);
                if let Some(position) = members.iter().position(|current| current.id == member) {
                    members.remove(position);
                }
            }
            LoadInstruction::ReportLoad {
                member,
                version,
                load,
            } => {
                let observation = LoadObservation {
                    recipient: load_recipient(member),
                    version: LoadVersion(u64::from(version)),
                    load: Load(u64::from(load)),
                };
                let result = router.receive(MailAddr(9), RouterMessage::Observe(observation));
                match members.iter().position(|current| current.id == member) {
                    None => {
                        let exact = matches!(result,
                            Err(RouterError::Policy { observation, error: LeastLoadedError::UnknownRecipient(returned) })
                            if same_load_observation(&observation, member, version, load)
                                && same_load_observation(&returned, member, version, load));
                        prop_assert!(exact);
                    }
                    Some(position) => match members[position].evidence {
                        Some(ModeledReading {
                            version: committed, ..
                        }) if version < committed => {
                            let exact = matches!(result,
                                Err(RouterError::Policy { observation, error: LeastLoadedError::Stale(returned) })
                                if same_load_observation(&observation, member, version, load)
                                    && same_load_observation(&returned, member, version, load));
                            prop_assert!(exact);
                        }
                        Some(ModeledReading {
                            version: committed,
                            load: prior_load,
                        }) if version == committed && load != prior_load => {
                            let exact = matches!(result,
                                Err(RouterError::Policy { observation, error: LeastLoadedError::ConflictingVersion(returned) })
                                if same_load_observation(&observation, member, version, load)
                                    && same_load_observation(&returned, member, version, load));
                            prop_assert!(exact);
                        }
                        _ => {
                            let actions = result.unwrap();
                            prop_assert!(actions.sends.is_empty());
                            prop_assert!(actions.creates.is_empty());
                            prop_assert_eq!(actions.become_, Step::Continue);
                            members[position].evidence = Some(ModeledReading { version, load });
                        }
                    },
                }
            }
            LoadInstruction::RouteMessage(value) => {
                let result = router.receive(MailAddr(9), RouterMessage::Route(value));
                let mut selected: Option<(u8, u8)> = None;
                for entry in &members {
                    if let Some(reading) = entry.evidence
                        && selected.is_none_or(|(_, least)| reading.load < least)
                    {
                        selected = Some((entry.id, reading.load));
                    }
                }
                if let Some((member, _)) = selected {
                    let actions = result.unwrap();
                    prop_assert_eq!(actions.sends.len(), 1);
                    prop_assert_eq!(actions.sends[0].to, load_recipient(member));
                    prop_assert_eq!(actions.sends[0].message, value);
                    prop_assert!(actions.creates.is_empty());
                    prop_assert_eq!(actions.become_, Step::Continue);
                } else {
                    prop_assert!(
                        matches!(result, Err(RouterError::NoEligibleRecipients(returned)) if returned == value)
                    );
                }
            }
        }
        let recipients = members
            .iter()
            .map(|entry| load_recipient(entry.id))
            .collect::<Vec<_>>();
        prop_assert_eq!(router.recipients(), recipients.as_slice());
        for member in 0..6 {
            let expected = members
                .iter()
                .find(|current| current.id == member)
                .map(|entry| match entry.evidence {
                    Some(reading) => LoadEvidence::Observed {
                        version: LoadVersion(u64::from(reading.version)),
                        load: Load(u64::from(reading.load)),
                    },
                    None => LoadEvidence::Unknown,
                });
            prop_assert_eq!(router.strategy().evidence(load_recipient(member)), expected);
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 384, max_shrink_iters: 100_000, ..ProptestConfig::default() })]

    #[test]
    fn least_loaded_matches_versioned_membership_and_selection(
        turns in vec(prop_oneof![
            (0_u8..6).prop_map(LoadInstruction::AddMember),
            (0_u8..6).prop_map(LoadInstruction::RemoveMember),
            (0_u8..6, 0_u8..8, 0_u8..8).prop_map(|(member, version, load)| LoadInstruction::ReportLoad { member, version, load }),
            any::<u8>().prop_map(LoadInstruction::RouteMessage),
        ], 0..160),
    ) {
        check_least_loaded_trace(turns)?;
    }
}

#[test]
fn least_loaded_readdition_discards_old_evidence_and_ties_keep_first_member() {
    check_least_loaded_trace([
        LoadInstruction::AddMember(0),
        LoadInstruction::AddMember(1),
        LoadInstruction::RouteMessage(7),
        LoadInstruction::ReportLoad {
            member: 0,
            version: 0,
            load: 3,
        },
        LoadInstruction::ReportLoad {
            member: 1,
            version: 0,
            load: 3,
        },
        LoadInstruction::RouteMessage(8),
        LoadInstruction::ReportLoad {
            member: 1,
            version: 1,
            load: 1,
        },
        LoadInstruction::RouteMessage(9),
        LoadInstruction::ReportLoad {
            member: 1,
            version: 0,
            load: 0,
        },
        LoadInstruction::ReportLoad {
            member: 1,
            version: 1,
            load: 2,
        },
        LoadInstruction::RemoveMember(1),
        LoadInstruction::AddMember(1),
        LoadInstruction::RouteMessage(10),
        LoadInstruction::ReportLoad {
            member: 1,
            version: 0,
            load: 0,
        },
        LoadInstruction::RouteMessage(11),
        LoadInstruction::ReportLoad {
            member: 5,
            version: 0,
            load: 0,
        },
    ])
    .unwrap();
}

fn keyed_recipient(member: u8) -> Recipient<KeyedRoutingTarget> {
    Recipient::global(MailAddr(u64::from(member) + 1))
}

fn identity_key(key: &u64) -> u64 {
    *key
}

fn route_rendezvous(
    router: &mut Active<TestRendezvous>,
    key: u64,
    value: u8,
) -> Result<Recipient<KeyedRoutingTarget>, TestCaseError> {
    let actions = router
        .receive(
            MailAddr(9),
            RouterMessage::Route(KeyedRoutingMessage { key, value }),
        )
        .unwrap();
    prop_assert_eq!(actions.sends.len(), 1);
    prop_assert_eq!(
        &actions.sends[0].message,
        &KeyedRoutingMessage { key, value }
    );
    prop_assert!(actions.creates.is_empty());
    prop_assert_eq!(actions.become_, Step::Continue);
    Ok(actions.sends[0].to)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn rendezvous_membership_edits_only_move_keys_to_or_from_the_changed_member(
        tokens in prop::array::uniform4(any::<u64>()),
        extra_keys in vec(any::<u64>(), 0..32),
    ) {
        prop_assume!(tokens.iter().enumerate().all(|(index, token)| tokens[..index].iter().all(|earlier| earlier != token)));
        let recipients = [keyed_recipient(0), keyed_recipient(1), keyed_recipient(2), keyed_recipient(3)];
        let mut router = TestRendezvous::new(recipients[..3].to_vec(), RendezvousHash::new(identity_key))
            .initialize().unwrap().behavior;
        for (member, token) in tokens[..3].iter().copied().enumerate() {
            let actions = router.receive(MailAddr(9), RouterMessage::Observe(MemberTokenObservation {
                recipient: recipients[member],
                version: MemberTokenVersion(0),
                token: MemberToken(token),
            })).unwrap();
            prop_assert!(actions.sends.is_empty());
            prop_assert!(actions.creates.is_empty());
            prop_assert_eq!(actions.become_, Step::Continue);
        }
        let keys = (0..64_u64).chain(extra_keys).collect::<Vec<_>>();
        let mut before = Vec::with_capacity(keys.len());
        for (index, key) in keys.iter().copied().enumerate() {
            before.push(route_rendezvous(&mut router, key, u8::try_from(index).unwrap())?);
        }

        let added = router.receive(MailAddr(9), RouterMessage::Add(recipients[3])).unwrap();
        prop_assert!(added.sends.is_empty());
        prop_assert!(added.creates.is_empty());
        prop_assert_eq!(added.become_, Step::Continue);
        prop_assert_eq!(router.recipients(), recipients.as_slice());
        prop_assert_eq!(router.strategy().evidence(recipients[3]), Some(MemberTokenEvidence::Unknown));
        for ((index, key), owner) in keys.iter().copied().enumerate().zip(before.iter().copied()) {
            prop_assert_eq!(route_rendezvous(&mut router, key, u8::try_from(index).unwrap())?, owner);
        }

        let observed = router.receive(MailAddr(9), RouterMessage::Observe(MemberTokenObservation {
            recipient: recipients[3],
            version: MemberTokenVersion(0),
            token: MemberToken(tokens[3]),
        })).unwrap();
        prop_assert!(observed.sends.is_empty());
        prop_assert!(observed.creates.is_empty());
        prop_assert_eq!(observed.become_, Step::Continue);
        let mut after_addition = Vec::with_capacity(keys.len());
        for ((index, key), previous) in keys.iter().copied().enumerate().zip(before) {
            let current = route_rendezvous(&mut router, key, u8::try_from(index).unwrap())?;
            prop_assert!(current == previous || current == recipients[3]);
            after_addition.push(current);
        }

        let mut reversed = TestRendezvous::new(recipients.iter().rev().copied().collect(), RendezvousHash::new(identity_key))
            .initialize().unwrap().behavior;
        for member in (0..4).rev() {
            let actions = reversed.receive(MailAddr(9), RouterMessage::Observe(MemberTokenObservation {
                recipient: recipients[member],
                version: MemberTokenVersion(0),
                token: MemberToken(tokens[member]),
            })).unwrap();
            prop_assert!(actions.sends.is_empty());
            prop_assert!(actions.creates.is_empty());
            prop_assert_eq!(actions.become_, Step::Continue);
        }
        for ((index, key), owner) in keys.iter().copied().enumerate().zip(after_addition.iter().copied()) {
            prop_assert_eq!(route_rendezvous(&mut reversed, key, u8::try_from(index).unwrap())?, owner);
        }

        let removed = router.receive(MailAddr(9), RouterMessage::Remove(recipients[1])).unwrap();
        prop_assert!(removed.sends.is_empty());
        prop_assert!(removed.creates.is_empty());
        prop_assert_eq!(removed.become_, Step::Continue);
        prop_assert_eq!(router.recipients(), &[recipients[0], recipients[2], recipients[3]]);
        prop_assert_eq!(router.strategy().evidence(recipients[1]), None);
        for member in [0, 2, 3] {
            prop_assert_eq!(router.strategy().evidence(recipients[member]), Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(0),
                token: MemberToken(tokens[member]),
            }));
        }
        for ((index, key), previous) in keys.iter().copied().enumerate().zip(after_addition) {
            let current = route_rendezvous(&mut router, key, u8::try_from(index).unwrap())?;
            prop_assert_ne!(current, recipients[1]);
            if previous != recipients[1] {
                prop_assert_eq!(current, previous);
            }
        }
    }
}
