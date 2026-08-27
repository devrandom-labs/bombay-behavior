//! Independent sequence models for catalogue actors whose owner tests cover
//! examples but not long, adversarial histories. The models use ordinary
//! collections and domain facts rather than reproducing template branches.

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, Cache, CacheConfiguration, CacheEntry,
    CacheMessage, CacheResult, ComponentHealth, ComponentHealthState, Configuration,
    ConfigurationError, ConfigurationMessage, ConfigurationState, ConfigurationVersion, Health,
    HealthError, HealthEvidence, HealthMessage, HealthStatus, MailAddr, MessageProtocol, Never,
    NoBirths, ObservationVersion, Readiness, ReadinessError, ReadinessEvidence, ReadinessMessage,
    ReadinessStatus, Recipient, Registry, RegistryError, RegistryMessage, RegistryResult, Step,
    Topic, TopicError, TopicMessage, User,
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

protocol!(ConfigurationReply, ConfigurationState<u8>);
protocol!(ReadinessReply, behavior::ReadinessReport<u8>);
protocol!(HealthReply, behavior::HealthReport<u8>);
protocol!(RegistryDestination, u8);
protocol!(RegistryReply, RegistryResult<u8, RegistryDestination>);

type TestConfiguration = Configuration<MailAddr, u8, Recipient<ConfigurationReply>>;
type TestReadiness = Readiness<MailAddr, u8, Recipient<ReadinessReply>>;
type TestHealth = Health<MailAddr, u8, Recipient<HealthReply>>;
type TestCache = Cache<MailAddr, u8, u8, Recipient<MessageProtocol<MailAddr, CacheResult<u8, u8>>>>;
type TestRegistry = Registry<MailAddr, u8, RegistryDestination, Recipient<RegistryReply>>;

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
        let mut actual = TestConfiguration::new().initialize().unwrap().behavior;
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
                prop_assert!(result.is_ok());
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
        operations in vec((0_u8..5, 0_u8..10, any::<bool>()), 0..160),
    ) {
        let mut actual = TestReadiness::new([0, 1, 2]).initialize().unwrap().behavior;
        let mut expected = [None; 3];

        for (dependency, version, ready) in operations {
            let status = if ready { ReadinessStatus::Ready } else { ReadinessStatus::NotReady };
            let result = actual.receive(MailAddr(9), ReadinessMessage::Observe {
                dependency,
                version: ObservationVersion(u64::from(version)),
                status,
            });
            if dependency >= 3 {
                let matched = matches!(result, Err(ReadinessError::UnknownDependency { dependency: returned, observed, status: returned_status }) if returned == dependency && observed == ObservationVersion(u64::from(version)) && returned_status == status);
                prop_assert!(matched);
            } else {
                let slot = &mut expected[usize::from(dependency)];
                let accepted = slot.is_none_or(|(current, committed)| {
                    version > current || (version == current && ready == committed)
                });
                if accepted {
                    prop_assert!(result.is_ok());
                    if slot.is_none_or(|(current, _)| version > current) {
                        *slot = Some((version, ready));
                    }
                } else if version < slot.unwrap().0 {
                    let matched = matches!(result, Err(ReadinessError::Stale { dependency: returned, observed, status: returned_status, .. }) if returned == dependency && observed == ObservationVersion(u64::from(version)) && returned_status == status);
                    prop_assert!(matched);
                } else {
                    let matched = matches!(result, Err(ReadinessError::ConflictingVersion { dependency: returned, version: returned_version, status: returned_status }) if returned == dependency && returned_version == ObservationVersion(u64::from(version)) && returned_status == status);
                    prop_assert!(matched);
                }
            }

            for (index, state) in actual.dependencies().iter().enumerate() {
                let modeled = expected[index].map_or(ReadinessEvidence::Unknown, |(version, ready)| {
                    ReadinessEvidence::Observed {
                        version: ObservationVersion(u64::from(version)),
                        status: if ready { ReadinessStatus::Ready } else { ReadinessStatus::NotReady },
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
        let mut actual = TestHealth::new().initialize().unwrap().behavior;
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
                prop_assert!(result.is_ok());
                match existing {
                    Some(index) if version > expected[index].1 => expected[index] = (component, version, evidence),
                    None => expected.push((component, version, evidence)),
                    _ => {}
                }
            } else if version < expected[existing.unwrap()].1 {
                let submitted = match evidence { Evidence::Present(status) => HealthEvidence::Present(status), Evidence::Removed => HealthEvidence::Removed };
                let matched = matches!(result, Err(HealthError::Stale { component: returned, observed, evidence: returned_evidence, .. }) if returned == component && observed == ObservationVersion(u64::from(version)) && returned_evidence == submitted);
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
        let mut actual = TestCache::new(CacheConfiguration::new(capacity).unwrap())
            .initialize().unwrap().behavior;
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
            prop_assert_eq!(&actions.sends[0].message, &expected_result);
            let retained = actual.state().entries().iter().map(|entry| (entry.key, entry.value)).collect::<Vec<_>>();
            prop_assert_eq!(retained, expected.clone());
            prop_assert!(actual.state().len() <= capacity);
        }
    }

    #[test]
    fn registry_matches_atomic_compare_and_remove_bindings(
        operations in vec((0_u8..3, 0_u8..8, 0_u8..8), 0..160),
    ) {
        let mut actual = TestRegistry::new().initialize().unwrap().behavior;
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
                        prop_assert!(result.is_ok());
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
                            prop_assert!(result.is_ok());
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
                    prop_assert!(actions.sends[0].message == expected_result);
                }
            }
            prop_assert_eq!(actual.bindings(), expected.as_slice());
        }
    }

    #[test]
    fn topic_is_an_ordered_idempotent_membership_snapshot(
        operations in vec((0_u8..3, 0_u8..8, any::<u8>()), 0..160),
    ) {
        let mut actual = Topic::<
            MailAddr,
            u8,
            Recipient<MessageProtocol<MailAddr, u8>>,
        >::new()
        .initialize()
        .unwrap()
        .behavior;
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
                    let recipients = actions.sends.iter().map(|delivery| delivery.to).collect::<Vec<_>>();
                    prop_assert_eq!(recipients, expected.clone());
                    prop_assert!(actions.sends.iter().all(|delivery| delivery.message == value));
                }
            }
            prop_assert_eq!(actual.subscribers(), expected.as_slice());
        }
    }
}
