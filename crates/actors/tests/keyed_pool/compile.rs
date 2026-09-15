use std::collections::BTreeMap;

use behavior::{MessageProtocol, Recipient};
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, BacklogCapacity, BindingCapacity, BindingExpectation,
    BindingRejection, BindingReply, BindingRequestId, CustomerDelivery, DiagnosticDisposition,
    Interruption, KeyedAdmissionRejection, KeyedCommand, KeyedOutcome, OrderedRoles,
    PoolFailureReaction, PoolRecovery, SubmissionId, keyed,
};
use behavior_actors::{Activate, ReplyDelivery, ReplyRoute};

use super::domain::{Account, RuntimeAddr, SearchJob, SearchResult, SearchRole, prepare_worker};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchDesk {
    Primary,
    Replica,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AccountPlacement {
    edition: u64,
    desk: SearchDesk,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountDirectoryError {
    AccountAlreadyPlaced,
    DirectoryFull,
    EditionExhausted,
    UnknownAccount,
}

struct AccountDirectory {
    capacity: usize,
    next_edition: Option<u64>,
    placements: BTreeMap<u64, AccountPlacement>,
}

impl AccountDirectory {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_edition: Some(1),
            placements: BTreeMap::new(),
        }
    }

    fn place(
        &mut self,
        account: u64,
        desk: SearchDesk,
    ) -> Result<AccountPlacement, AccountDirectoryError> {
        if self.placements.contains_key(&account) {
            return Err(AccountDirectoryError::AccountAlreadyPlaced);
        }
        if self.placements.len() == self.capacity {
            return Err(AccountDirectoryError::DirectoryFull);
        }
        let Some(edition) = self.next_edition else {
            return Err(AccountDirectoryError::EditionExhausted);
        };
        self.next_edition = edition.checked_add(1);
        let placement = AccountPlacement { edition, desk };
        self.placements.insert(account, placement);
        Ok(placement)
    }

    fn placement(&self, account: u64) -> Option<AccountPlacement> {
        self.placements.get(&account).copied()
    }

    fn move_account(
        &mut self,
        account: u64,
        desk: SearchDesk,
    ) -> Result<(AccountPlacement, AccountPlacement), AccountDirectoryError> {
        let Some(previous) = self.placements.get(&account).copied() else {
            return Err(AccountDirectoryError::UnknownAccount);
        };
        let Some(edition) = self.next_edition else {
            return Err(AccountDirectoryError::EditionExhausted);
        };
        self.next_edition = edition.checked_add(1);
        let current = AccountPlacement { edition, desk };
        self.placements.insert(account, current);
        Ok((previous, current))
    }

    fn remove(&mut self, account: u64) -> Option<AccountPlacement> {
        self.placements.remove(&account)
    }
}

#[test]
fn keyed_construction_and_commands_need_only_domain_types() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(8),
        BindingCapacity::new(2).unwrap_or_else(|_| panic!("two bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut directory = AccountDirectory::new(2);
    let customer = Recipient::<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(80));
    let management = Recipient::<
        MessageProtocol<RuntimeAddr, BindingReply<RuntimeAddr, Account, SearchRole>>,
    >::global(RuntimeAddr(81));

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(SubmissionId::new(1), Account(42), SearchJob(9), customer),
        )
        .unwrap_or_else(|error| panic!("keyed submission failed: {error}"));
    let submitted = submitted
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("accepted work emits one customer receipt"));
    let accepted_binding = match submitted {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::Accepted {
                submission,
                job,
                binding,
            } => {
                assert_eq!(submission, SubmissionId::new(1));
                assert_eq!(job.get(), 1);
                assert_eq!(binding.role(), &SearchRole::Primary);
                binding
            }
            _ => panic!("the creating primary role accepts queued work"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("accepted work uses one logical customer route")
        }
    };
    assert_eq!(accepted_binding.generation().get(), 1);
    let accepted_prediction = directory
        .place(42, SearchDesk::Primary)
        .unwrap_or_else(|error| panic!("the directory places the selected account: {error:?}"));
    assert_eq!(
        accepted_prediction,
        AccountPlacement {
            edition: accepted_binding.generation().get(),
            desk: SearchDesk::Primary,
        }
    );

    let rebalanced = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(2),
                Account(7),
                BindingExpectation::Absent,
                SearchRole::Replica,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("keyed rebalance failed: {error}"));
    let rebalanced = rebalanced
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("absent rebalance emits one reply"));
    let replica_binding = match rebalanced {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Bound { request, current } => {
                assert_eq!(request, BindingRequestId::new(2));
                assert_eq!(current.role(), &SearchRole::Replica);
                current
            }
            _ => panic!("an absent key becomes bound"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    };
    assert_eq!(replica_binding.generation().get(), 2);
    let replica_prediction = directory
        .place(7, SearchDesk::Replica)
        .unwrap_or_else(|error| panic!("the directory places the explicit account: {error:?}"));
    assert_eq!(
        replica_prediction,
        AccountPlacement {
            edition: replica_binding.generation().get(),
            desk: SearchDesk::Replica,
        }
    );

    let capacity_rejected = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(20),
                Account(99),
                BindingExpectation::Absent,
                SearchRole::Primary,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("capacity rebalance failed: {error}"));
    let capacity_rejected = capacity_rejected
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("capacity rejection emits one reply"));
    match capacity_rejected {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Rejected { command, reason } => {
                assert_eq!(command.request(), BindingRequestId::new(20));
                assert_eq!(command.key(), &Account(99));
                assert_eq!(reason, BindingRejection::BindingCapacityExhausted);
            }
            _ => panic!("the full binding table returns the command"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    }
    assert_eq!(
        directory.place(99, SearchDesk::Primary),
        Err(AccountDirectoryError::DirectoryFull)
    );

    let unchanged = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(3),
                Account(7),
                BindingExpectation::Exact(replica_binding.generation().clone()),
                SearchRole::Replica,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("same-role rebalance failed: {error}"));
    let unchanged = unchanged
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("same-role rebalance emits one reply"));
    let replica_binding = match unchanged {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Unchanged { request, current } => {
                assert_eq!(request, BindingRequestId::new(3));
                current
            }
            _ => panic!("same-role affinity preserves its binding"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    };
    assert_eq!(replica_binding.generation().get(), 2);
    assert_eq!(
        directory.placement(7),
        Some(AccountPlacement {
            edition: replica_binding.generation().get(),
            desk: SearchDesk::Replica,
        })
    );

    let moved = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(4),
                Account(7),
                BindingExpectation::Exact(replica_binding.generation().clone()),
                SearchRole::Primary,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("cross-role rebalance failed: {error}"));
    let moved = moved
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("cross-role rebalance emits one reply"));
    let primary_binding = match moved {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Rebalanced {
                request,
                prior,
                current,
            } => {
                assert_eq!(request, BindingRequestId::new(4));
                assert_eq!(prior.generation().get(), 2);
                assert_eq!(prior.role(), &SearchRole::Replica);
                assert_eq!(current.role(), &SearchRole::Primary);
                current
            }
            _ => panic!("cross-role affinity receives a fresh binding"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    };
    assert_eq!(primary_binding.generation().get(), 3);
    let (prior_prediction, primary_prediction) = directory
        .move_account(7, SearchDesk::Primary)
        .unwrap_or_else(|error| panic!("the directory moves the exact account: {error:?}"));
    assert_eq!(prior_prediction.edition, replica_binding.generation().get());
    assert_eq!(prior_prediction.desk, SearchDesk::Replica);
    assert_eq!(
        primary_prediction.edition,
        primary_binding.generation().get()
    );
    assert_eq!(primary_prediction.desk, SearchDesk::Primary);

    let stale = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(5),
                Account(7),
                BindingExpectation::Exact(replica_binding.generation().clone()),
                SearchRole::Replica,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("stale rebalance failed: {error}"));
    let stale = stale
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("stale rebalance emits one reply"));
    match stale {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Rejected { command, reason } => {
                assert_eq!(command.request(), BindingRequestId::new(5));
                assert_eq!(command.key(), &Account(7));
                assert_eq!(
                    command.expectation(),
                    &BindingExpectation::Exact(replica_binding.generation().clone())
                );
                assert_eq!(
                    reason,
                    BindingRejection::StaleExpectation {
                        actual: BindingExpectation::Exact(primary_binding.generation().clone())
                    }
                );
            }
            _ => panic!("stale expectation returns its complete command"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    }
    assert_eq!(directory.placement(7), Some(primary_prediction));

    let unbound = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::unbind(
                BindingRequestId::new(6),
                Account(7),
                BindingExpectation::Exact(primary_binding.generation().clone()),
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("keyed unbind failed: {error}"));
    let unbound = unbound
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("exact unbind emits one reply"));
    match unbound {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Unbound {
                request,
                key,
                removed,
            } => {
                assert_eq!(request, BindingRequestId::new(6));
                assert_eq!(key, Account(7));
                assert_eq!(removed.generation(), primary_binding.generation());
                assert_eq!(
                    directory.remove(7),
                    Some(AccountPlacement {
                        edition: removed.generation().get(),
                        desk: SearchDesk::Primary,
                    })
                );
            }
            _ => panic!("exact unbind returns the retained binding"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    }

    let absent = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::unbind(
                BindingRequestId::new(7),
                Account(7),
                BindingExpectation::Absent,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("already-absent unbind failed: {error}"));
    let absent = absent
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("already-absent unbind emits one reply"));
    match absent {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::AlreadyUnbound { command } => {
                assert_eq!(command.request(), BindingRequestId::new(7));
                assert_eq!(command.key(), &Account(7));
                assert_eq!(command.expectation(), &BindingExpectation::Absent);
                assert_eq!(directory.placement(7), None);
            }
            _ => panic!("absence is an explicit accepted no-op"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    }

    let capacity_released = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::rebalance(
                BindingRequestId::new(8),
                Account(99),
                BindingExpectation::Absent,
                SearchRole::Replica,
                management,
            ),
        )
        .unwrap_or_else(|error| panic!("released-capacity rebalance failed: {error}"));
    let capacity_released = capacity_released
        .sends
        .binding_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("released capacity emits one reply"));
    match capacity_released {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            BindingReply::Bound { request, current } => {
                assert_eq!(request, BindingRequestId::new(8));
                let prediction = directory
                    .place(99, SearchDesk::Replica)
                    .unwrap_or_else(|error| {
                        panic!("the directory reuses released capacity: {error:?}")
                    });
                assert_eq!(prediction.edition, current.generation().get());
                assert_eq!(prediction.desk, SearchDesk::Replica);
            }
            _ => panic!("released capacity accepts another key"),
        },
        ReplyDelivery::Established(_) => panic!("management uses one logical route"),
    }
}

#[test]
fn rejected_submission_keeps_both_customer_capabilities() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let customer = Recipient::<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(90));

    let accepted = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(SubmissionId::new(10), Account(2), SearchJob(1), customer),
        )
        .unwrap_or_else(|error| panic!("first keyed submission failed: {error}"));
    assert_eq!(accepted.sends.customer_outcomes.as_slice().len(), 1);

    let rejected = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(SubmissionId::new(11), Account(4), SearchJob(2), customer),
        )
        .unwrap_or_else(|error| panic!("second keyed submission failed: {error}"));
    let rejected = rejected
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected work emits one customer action"));
    match rejected {
        CustomerDelivery::RejectedLogical {
            delivery,
            customer: ReplyRoute::Logical(original),
        } => {
            assert_eq!(delivery.to, customer);
            assert_eq!(original, customer);
            match delivery.message {
                KeyedOutcome::Rejected {
                    submission,
                    key,
                    payload,
                    reason,
                } => {
                    assert_eq!(submission, SubmissionId::new(11));
                    assert_eq!(key, Account(4));
                    assert_eq!(payload, SearchJob(2));
                    assert_eq!(reason, KeyedAdmissionRejection::BindingCapacityExhausted);
                }
                _ => panic!("binding exhaustion returns the rejected submission"),
            }
        }
        CustomerDelivery::Logical { .. }
        | CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedEstablished { .. }
        | CustomerDelivery::RejectedLogical {
            customer: ReplyRoute::Established(_),
            ..
        } => panic!("logical rejection preserves both logical capabilities"),
    }
}
