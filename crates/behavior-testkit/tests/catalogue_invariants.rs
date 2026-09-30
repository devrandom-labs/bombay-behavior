//! Independent sequence models for catalogue actors whose owner tests cover
//! examples but not long, adversarial histories. The models use ordinary
//! collections and domain facts rather than reproducing template branches.

use std::collections::BTreeMap;

use behavior_actors::{
    Activate as _, Cache, CacheConfiguration, CacheEntry, CacheMessage, CacheResult,
    ComponentHealth, ComponentHealthState, Configuration, ConfigurationError, ConfigurationMessage,
    ConfigurationState, ConfigurationVersion, Health, HealthError, HealthEvidence, HealthMessage,
    HealthStatus, ObservationVersion, PubSub, PubSubError, PubSubMessage, Readiness,
    ReadinessError, ReadinessEvidence, ReadinessMessage, ReadinessStatus, Registry, RegistryError,
    RegistryMessage, RegistryResult, Topic, TopicError, TopicMessage,
};

use behavior_core::{MailAddr, MessageProtocol, Recipient, Step};
use proptest::collection::vec;
use proptest::prelude::*;

macro_rules! protocol {
    ($name:ident, $message:ty) => {
        struct $name;
        impl behavior_core::Protocol for $name {
            type Addr = MailAddr;
            type Msg = $message;
        }
    };
}

protocol!(ConfigurationReply, ConfigurationState<u8>);
protocol!(ReadinessReply, behavior_actors::ReadinessReport<u8>);
protocol!(HealthReply, behavior_actors::HealthReport<u8>);
protocol!(RegistryDestination, u8);
protocol!(RegistryReply, RegistryResult<u8, RegistryDestination>);

type TestConfiguration = Configuration<MailAddr, u8, Recipient<ConfigurationReply>>;
type TestReadiness = Readiness<MailAddr, u8, Recipient<ReadinessReply>>;
type TestHealth = Health<MailAddr, u8, Recipient<HealthReply>>;
type TestCache = Cache<MailAddr, u8, u8, Recipient<MessageProtocol<MailAddr, CacheResult<u8, u8>>>>;
type TestRegistry = Registry<MailAddr, u8, RegistryDestination, Recipient<RegistryReply>>;
type TestPubSub = PubSub<MailAddr, u8, String, Recipient<MessageProtocol<MailAddr, String>>>;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 384,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn configuration_is_a_monotonic_atomic_register(
        proposals in vec((0_u8..12, any::<u8>()), 0..160),
    ) {
        let initialized = TestConfiguration::new().initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected: Option<(u8, u8)> = None;

        for (version, value) in proposals {
            let before = expected;
            let accepted = match before {
                None => true,
                Some((current, committed)) => version > current || (version == current && value == committed),
            };
            let result = actual.receive(
                MailAddr(9),
                ConfigurationMessage::Apply {
                    version: ConfigurationVersion(u64::from(version)),
                    value,
                },
            );
            if accepted {
                let actions = result.expect("the independent model accepted this configuration");
                prop_assert!(actions.sends.is_empty());
                prop_assert!(actions.creates.is_empty());
                prop_assert!(matches!(actions.become_, Step::Continue));
                if before.is_none_or(|(current, _)| version > current) {
                    expected = Some((version, value));
                }
            } else {
                let matched = match (before.unwrap(), result) {
                    ((current, _), Err(ConfigurationError::Stale { proposed, current: observed, value: returned })) => {
                        version < current
                            && proposed == ConfigurationVersion(u64::from(version))
                            && observed == ConfigurationVersion(u64::from(current))
                            && returned == value
                    }
                    ((current, _), Err(ConfigurationError::ConflictingVersion { version: observed, value: returned })) => {
                        version == current
                            && observed == ConfigurationVersion(u64::from(version))
                            && returned == value
                    }
                    _ => false,
                };
                prop_assert!(matched);
            }
            let state = match expected {
                None => ConfigurationState::Unconfigured,
                Some((version, value)) => ConfigurationState::Configured {
                    version: ConfigurationVersion(u64::from(version)),
                    value,
                },
            };
            prop_assert_eq!(actual.state(), &state);
        }
    }

    #[test]
    fn readiness_matches_per_dependency_version_registers(
        operations in vec((
            0_u8..5,
            prop_oneof![0_u64..10, Just(u64::MAX - 1), Just(u64::MAX)],
            prop_oneof![Just(ReadinessStatus::Ready), Just(ReadinessStatus::NotReady)],
        ), 0..160),
    ) {
        let initialized = TestReadiness::new([0, 1, 2]).initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected = [None; 3];

        for (dependency, version, status) in operations {
            let result = actual.receive(MailAddr(9), ReadinessMessage::Observe {
                dependency,
                version: ObservationVersion(version),
                status,
            });
            if dependency >= 3 {
                let matched = matches!(result, Err(ReadinessError::UnknownDependency { dependency: returned, observed, status: returned_status }) if returned == dependency && observed == ObservationVersion(version) && returned_status == status);
                prop_assert!(matched);
            } else {
                let slot = &mut expected[usize::from(dependency)];
                let accepted = slot.is_none_or(|(current, committed)| {
                    version > current || (version == current && status == committed)
                });
                if accepted {
                    let actions = result.expect("the independent model accepted this observation");
                    prop_assert!(actions.sends.is_empty());
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    if slot.is_none_or(|(current, _)| version > current) {
                        *slot = Some((version, status));
                    }
                } else if version < slot.unwrap().0 {
                    let matched = matches!(result, Err(ReadinessError::Stale { dependency: returned, observed, current, status: returned_status }) if returned == dependency && observed == ObservationVersion(version) && current == ObservationVersion(slot.unwrap().0) && returned_status == status);
                    prop_assert!(matched);
                } else {
                    let matched = matches!(result, Err(ReadinessError::ConflictingVersion { dependency: returned, version: returned_version, status: returned_status }) if returned == dependency && returned_version == ObservationVersion(version) && returned_status == status);
                    prop_assert!(matched);
                }
            }

            prop_assert_eq!(actual.dependencies().len(), expected.len());
            for (index, state) in actual.dependencies().iter().enumerate() {
                let modeled = expected[index].map_or(ReadinessEvidence::Unknown, |(version, status)| {
                    ReadinessEvidence::Observed {
                        version: ObservationVersion(version),
                        status,
                    }
                });
                prop_assert_eq!(state.dependency, u8::try_from(index).unwrap());
                prop_assert_eq!(state.evidence, modeled);
            }
        }
    }

    #[test]
    fn health_tombstones_and_versions_match_an_independent_map(
        operations in vec((0_u8..4, 0_u8..10, 0_u8..4), 0..160),
    ) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Evidence { Present(HealthStatus), Removed }
        let initialized = TestHealth::new().initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected: Vec<(u8, u8, Evidence)> = Vec::new();

        for (component, version, tag) in operations {
            let evidence = match tag {
                0 => Evidence::Removed,
                1 => Evidence::Present(HealthStatus::Healthy),
                2 => Evidence::Present(HealthStatus::Degraded),
                _ => Evidence::Present(HealthStatus::Unhealthy),
            };
            let message = match evidence {
                Evidence::Removed => HealthMessage::Remove {
                    component,
                    version: ObservationVersion(u64::from(version)),
                },
                Evidence::Present(status) => HealthMessage::Observe {
                    component,
                    version: ObservationVersion(u64::from(version)),
                    status,
                },
            };
            let result = actual.receive(MailAddr(9), message);
            let existing = expected.iter().position(|(key, _, _)| *key == component);
            let accepted = existing.is_none_or(|index| {
                let (_, current, committed) = expected[index];
                version > current || (version == current && evidence == committed)
            });
            if accepted {
                let actions = result.expect("the independent model accepted this health evidence");
                prop_assert!(actions.sends.is_empty());
                prop_assert!(actions.creates.is_empty());
                prop_assert!(matches!(actions.become_, Step::Continue));
                match existing {
                    Some(index) if version > expected[index].1 => expected[index] = (component, version, evidence),
                    None => expected.push((component, version, evidence)),
                    _ => {}
                }
            } else if version < expected[existing.unwrap()].1 {
                let submitted = match evidence { Evidence::Present(status) => HealthEvidence::Present(status), Evidence::Removed => HealthEvidence::Removed };
                let matched = matches!(result, Err(HealthError::Stale { component: returned, observed, current, evidence: returned_evidence }) if returned == component && observed == ObservationVersion(u64::from(version)) && current == ObservationVersion(u64::from(expected[existing.unwrap()].1)) && returned_evidence == submitted);
                prop_assert!(matched);
            } else {
                let submitted = match evidence { Evidence::Present(status) => HealthEvidence::Present(status), Evidence::Removed => HealthEvidence::Removed };
                let matched = matches!(result, Err(HealthError::ConflictingVersion { component: returned, version: returned_version, evidence: returned_evidence }) if returned == component && returned_version == ObservationVersion(u64::from(version)) && returned_evidence == submitted);
                prop_assert!(matched);
            }

            let modeled = expected.iter().map(|(component, version, evidence)| match evidence {
                Evidence::Present(status) => ComponentHealthState::Present(ComponentHealth {
                    component: *component,
                    version: ObservationVersion(u64::from(*version)),
                    status: *status,
                }),
                Evidence::Removed => ComponentHealthState::Removed {
                    component: *component,
                    version: ObservationVersion(u64::from(*version)),
                },
            }).collect::<Vec<_>>();
            prop_assert_eq!(actual.components(), modeled.as_slice());
        }
    }

    #[test]
    fn cache_matches_a_plain_recency_list_after_every_operation(
        capacity in 1_usize..7,
        operations in vec((0_u8..3, 0_u8..10, any::<u8>()), 0..180),
    ) {
        let initialized = TestCache::new(CacheConfiguration::new(capacity).unwrap())
            .initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected: Vec<(u8, u8)> = Vec::new();
        let reply = Recipient::global(MailAddr(1));

        for (operation, key, value) in operations {
            let expected_result = match operation {
                0 => {
                    let replaced = expected.iter().position(|(candidate, _)| *candidate == key)
                        .map(|index| expected.remove(index).1);
                    let evicted = if replaced.is_none() && expected.len() == capacity {
                        let (key, value) = expected.remove(0);
                        Some(CacheEntry { key, value })
                    } else { None };
                    expected.push((key, value));
                    CacheResult::Stored { key, replaced, evicted }
                }
                1 => match expected.iter().position(|(candidate, _)| *candidate == key) {
                    Some(index) => {
                        let entry = expected.remove(index);
                        expected.push(entry);
                        CacheResult::Hit { key, value: entry.1 }
                    }
                    None => CacheResult::Miss { key },
                },
                _ => match expected.iter().position(|(candidate, _)| *candidate == key) {
                    Some(index) => CacheResult::Removed { key, value: expected.remove(index).1 },
                    None => CacheResult::Absent { key },
                },
            };
            let message = match operation {
                0 => CacheMessage::Put { key, value, reply_to: reply },
                1 => CacheMessage::Get { key, reply_to: reply },
                _ => CacheMessage::Remove { key, reply_to: reply },
            };
            let actions = actual.receive(MailAddr(9), message).unwrap();
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, Step::Continue));
            let replies = actions.sends.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message))
                .collect::<Vec<_>>();
            prop_assert_eq!(replies, [(MailAddr(1), expected_result)]);
            let retained = actual.state().entries().iter().map(|entry| (entry.key, entry.value)).collect::<Vec<_>>();
            prop_assert_eq!(retained, expected.clone());
            prop_assert!(actual.state().len() <= capacity);
        }
    }

    #[test]
    fn registry_matches_atomic_compare_and_remove_bindings(
        operations in vec((0_u8..3, 0_u8..8, 0_u8..8), 0..160),
    ) {
        let initialized = TestRegistry::new().initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected: Vec<(u8, Recipient<RegistryDestination>)> = Vec::new();
        let lookup_reply = Recipient::global(MailAddr(99));

        for (operation, key, address) in operations {
            let recipient = Recipient::global(MailAddr(u64::from(address)));
            match operation {
                0 => {
                    let result = actual.receive(MailAddr(9), RegistryMessage::Bind { key, recipient });
                    if expected.iter().any(|(candidate, _)| *candidate == key) {
                        let current = expected
                            .iter()
                            .find(|(candidate, _)| *candidate == key)
                            .expect("the independent model found the binding")
                            .1;
                        let matched = matches!(result, Err(RegistryError::AlreadyBound { key: returned, recipient: returned_recipient, current: returned_current }) if returned == key && returned_recipient == recipient && returned_current == current);
                        prop_assert!(matched);
                    } else {
                        let actions = result.expect("the independent model accepted this binding");
                        prop_assert!(actions.sends.is_empty());
                        prop_assert!(actions.creates.is_empty());
                        prop_assert!(matches!(actions.become_, Step::Continue));
                        expected.push((key, recipient));
                    }
                }
                1 => {
                    let position = expected.iter().position(|(candidate, _)| *candidate == key);
                    let result = actual.receive(MailAddr(9), RegistryMessage::Unbind { key, recipient });
                    match position {
                        None => {
                            let matched = matches!(result, Err(RegistryError::NotBound { key: returned, recipient: returned_recipient }) if returned == key && returned_recipient == recipient);
                            prop_assert!(matched);
                        }
                        Some(index) if expected[index].1 != recipient => {
                            let matched = matches!(result, Err(RegistryError::StaleBinding { key: returned, recipient: returned_recipient, current }) if returned == key && returned_recipient == recipient && current == expected[index].1);
                            prop_assert!(matched);
                        }
                        Some(index) => {
                            let actions = result.expect("the independent model removed this binding");
                            prop_assert!(actions.sends.is_empty());
                            prop_assert!(actions.creates.is_empty());
                            prop_assert!(matches!(actions.become_, Step::Continue));
                            expected.remove(index);
                        }
                    }
                }
                _ => {
                    let actions = actual.receive(MailAddr(9), RegistryMessage::Lookup { key, reply_to: lookup_reply }).unwrap();
                    let expected_result = expected.iter().find(|(candidate, _)| *candidate == key).map_or(
                        RegistryResult::Missing { key },
                        |(_, recipient)| RegistryResult::Found { key, recipient: *recipient },
                    );
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    let replies = actions.sends.into_iter()
                        .map(|delivery| (delivery.to.address(), delivery.message))
                        .collect::<Vec<_>>();
                    prop_assert!(replies == [(MailAddr(99), expected_result)]);
                }
            }
            prop_assert_eq!(actual.bindings(), expected.as_slice());
        }
    }

    #[test]
    fn pub_sub_preserves_topic_membership_and_rejected_publications(
        operations in vec((0_u8..3, 0_u8..8, 0_u8..8, any::<u8>()), 0..160),
    ) {
        let initialized = TestPubSub::new().initialize().unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut membership = BTreeMap::<
            u8,
            Vec<Recipient<MessageProtocol<MailAddr, String>>>,
        >::new();
        let mut introduced = Vec::<u8>::new();

        for (turn, (operation, topic, address, content)) in operations.into_iter().enumerate() {
            let subscriber = Recipient::global(MailAddr(u64::from(address)));
            match operation {
                0 => {
                    let actions = actual.receive(
                        MailAddr(9),
                        PubSubMessage::Subscribe { topic, subscriber },
                    ).unwrap();
                    prop_assert!(actions.sends.is_empty());
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    if let Some(members) = membership.get_mut(&topic) {
                        if !members.contains(&subscriber) { members.push(subscriber); }
                    } else {
                        membership.insert(topic, vec![subscriber]);
                        introduced.push(topic);
                    }
                }
                1 => {
                    let result = actual.receive(
                        MailAddr(9),
                        PubSubMessage::Unsubscribe { topic, subscriber },
                    );
                    if let Some(members) = membership.get_mut(&topic) {
                        if let Some(index) = members.iter().position(|member| *member == subscriber) {
                            members.remove(index);
                            let actions = result.unwrap();
                            prop_assert!(actions.sends.is_empty());
                            prop_assert!(actions.creates.is_empty());
                            prop_assert!(matches!(actions.become_, Step::Continue));
                        } else {
                            let exact = matches!(result, Err(PubSubError::NotSubscribed {
                                topic: returned_topic,
                                subscriber: returned_subscriber,
                            }) if returned_topic == topic && returned_subscriber == subscriber);
                            prop_assert!(exact);
                        }
                    } else {
                        let exact = matches!(result, Err(PubSubError::UnknownTopic {
                            topic: returned_topic,
                            subscriber: returned_subscriber,
                        }) if returned_topic == topic && returned_subscriber == subscriber);
                        prop_assert!(exact);
                    }
                }
                _ => {
                    let expected_publication = format!("publication-{turn}-{content}");
                    let publication = expected_publication.clone();
                    let original_pointer = publication.as_ptr();
                    let result = actual.receive(
                        MailAddr(9),
                        PubSubMessage::Publish { topic, value: publication },
                    );
                    if let Some(members) = membership.get(&topic).filter(|members| !members.is_empty()) {
                        let actions = result.unwrap();
                        let deliveries = actions.sends.into_iter()
                            .map(|delivery| (delivery.to, delivery.message))
                            .collect::<Vec<_>>();
                        let expected_deliveries = members.iter().copied()
                            .map(|to| (to, expected_publication.clone()))
                            .collect::<Vec<_>>();
                        prop_assert_eq!(deliveries, expected_deliveries);
                        prop_assert!(actions.creates.is_empty());
                        prop_assert!(matches!(actions.become_, Step::Continue));
                    } else {
                        let exact = matches!(result, Err(PubSubError::NoSubscribers {
                            topic: returned_topic,
                            value: returned_value,
                        }) if returned_topic == topic && returned_value == expected_publication
                            && returned_value.as_ptr() == original_pointer);
                        prop_assert!(exact);
                    }
                }
            }
            prop_assert_eq!(actual.topics().len(), introduced.len());
            for (state, topic) in actual.topics().iter().zip(&introduced) {
                prop_assert_eq!(&state.topic, topic);
                prop_assert_eq!(state.subscribers.as_slice(), membership[topic].as_slice());
            }
        }
    }

    #[test]
    fn topic_is_an_ordered_idempotent_membership_snapshot(
        operations in vec((0_u8..3, 0_u8..8, any::<u8>()), 0..160),
    ) {
        let initialized = Topic::<
            MailAddr,
            u8,
            Recipient<MessageProtocol<MailAddr, u8>>,
        >::new()
        .initialize()
        .unwrap();
        prop_assert!(initialized.actions.sends.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut expected: Vec<Recipient<MessageProtocol<MailAddr, u8>>> = Vec::new();

        for (operation, address, value) in operations {
            let subscriber = Recipient::global(MailAddr(u64::from(address)));
            match operation {
                0 => {
                    let actions = actual
                        .receive(MailAddr(9), TopicMessage::Subscribe(subscriber))
                        .unwrap();
                    prop_assert!(actions.sends.is_empty());
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    if !expected.contains(&subscriber) { expected.push(subscriber); }
                }
                1 => {
                    let actions = actual
                        .receive(MailAddr(9), TopicMessage::Unsubscribe(subscriber))
                        .unwrap();
                    prop_assert!(actions.sends.is_empty());
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    expected.retain(|candidate| *candidate != subscriber);
                }
                _ if expected.is_empty() => {
                    let result = actual.receive(MailAddr(9), TopicMessage::Publish(value));
                    let matched = matches!(result, Err(TopicError::NoSubscribers(returned)) if returned == value);
                    prop_assert!(matched);
                }
                _ => {
                    let actions = actual.receive(MailAddr(9), TopicMessage::Publish(value)).unwrap();
                    let deliveries = actions.sends.into_iter()
                        .map(|delivery| (delivery.to, delivery.message))
                        .collect::<Vec<_>>();
                    let expected_deliveries = expected.iter().copied()
                        .map(|to| (to, value)).collect::<Vec<_>>();
                    prop_assert_eq!(deliveries, expected_deliveries);
                    prop_assert!(actions.creates.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                }
            }
            prop_assert_eq!(actual.subscribers(), expected.as_slice());
        }
    }
}
