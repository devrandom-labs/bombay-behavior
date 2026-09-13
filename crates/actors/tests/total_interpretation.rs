use behavior_actors::atomic::{ImmediateActivation, ProxyEffects, StableProxy};
use behavior_actors::{
    ActionItem, Actions, ActiveTurn, Address, Behavior, BehaviorActed, BufferSends,
    ClassifySettlement, EndpointAddress, Here, InterpretItem, InterpretSends, Interpretation,
    InterpreterFault, InterpreterRequests, ItemSettlement, LeaseSends, Never, NoBirths, Protocol,
    SettledItem, SettlementStatus, User,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum Plan {
    Accept,
    Reject,
    Corrupt,
}

struct Runtime {
    plans: BTreeMap<u8, Plan>,
    attempts: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Work(u8);

impl ActionItem for Work {
    type Accepted = Work;
    type Rejection = WorkRejection;
    type Prerequisite = Never;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkRejection {
    Closed,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ProxyAddress;

impl Address for ProxyAddress {
    type Nonce = u64;
}

#[derive(Clone, Copy)]
struct ProxyEndpoint;

impl EndpointAddress for ProxyAddress {
    type Established<P>
        = ProxyEndpoint
    where
        P: Protocol<Addr = Self>;
}

struct ProxyWorker;

impl Protocol for ProxyWorker {
    type Addr = ProxyAddress;
    type Msg = u8;
}

impl Behavior for ProxyWorker {
    type Protocol = Self;
    type Event = User<ProxyAddress, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct ProxyRuntimeWitness;

impl<Item, RootEvent, Path> InterpretItem<Item, RootEvent, Path> for ProxyRuntimeWitness
where
    Item: ActionItem,
{
    fn interpret_item(
        &mut self,
        item: Item,
    ) -> impl core::future::Future<
        Output = ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
    > + Send {
        async move {
            ItemSettlement::Corrupt {
                item,
                fault: InterpreterFault::MissingCapability,
            }
        }
    }
}

impl Runtime {
    fn new(plans: impl IntoIterator<Item = (u8, Plan)>) -> Self {
        Self {
            plans: plans.into_iter().collect(),
            attempts: Vec::new(),
        }
    }
}

impl InterpretItem<Work, (), Here> for Runtime {
    fn interpret_item(
        &mut self,
        item: Work,
    ) -> impl core::future::Future<Output = ItemSettlement<Work, Work, WorkRejection, Never>> + Send
    {
        self.attempts.push(item.0);
        let plan = self.plans.get(&item.0).copied().unwrap_or(Plan::Accept);
        async move {
            match plan {
                Plan::Accept => ItemSettlement::Accepted(item),
                Plan::Reject => ItemSettlement::Rejected {
                    item,
                    reason: WorkRejection::Closed,
                },
                Plan::Corrupt => ItemSettlement::Corrupt {
                    item,
                    fault: InterpreterFault::CorruptTraversal,
                },
            }
        }
    }
}

#[tokio::test]
async fn retained_buffer_continues_after_rejection_at_the_same_event_path() {
    let sends = BufferSends {
        deliveries: InterpreterRequests::one(Work(1)),
        outcomes: InterpreterRequests::one(Work(2)),
    };
    let mut runtime = Runtime::new([(1, Plan::Reject)]);

    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(sends, &mut runtime).await;
    assert_eq!(runtime.attempts, [1, 2]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("lawful rejection cannot corrupt a named product");
    };
    assert_eq!(settlement.settlement_status(), SettlementStatus::Rejected);
    assert!(matches!(
        settlement.deliveries.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Rejected {
            item: Work(1),
            reason: WorkRejection::Closed,
        })]
    ));
    assert!(matches!(
        settlement.outcomes.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(2)))]
    ));
}

#[tokio::test]
async fn retained_lease_preserves_the_exact_second_lane_after_corruption() {
    let sends = LeaseSends {
        outcomes: InterpreterRequests::one(Work(3)),
        schedules: InterpreterRequests::one(Work(4)),
    };
    let mut runtime = Runtime::new([(3, Plan::Corrupt)]);

    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(sends, &mut runtime).await;
    assert_eq!(runtime.attempts, [3]);
    let Interpretation::Corrupt(settlement) = settlement else {
        panic!("corrupt first lane must mark the named product corrupt");
    };
    assert_eq!(settlement.settlement_status(), SettlementStatus::Corrupt);
    assert!(matches!(
        settlement.outcomes.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Corrupt {
            item: Work(3),
            fault: InterpreterFault::CorruptTraversal,
        })]
    ));
    assert!(matches!(
        settlement.schedules.as_slice(),
        [SettledItem::Unattempted(Work(4))]
    ));
}

#[tokio::test]
async fn proxy_observations_settle_before_owner_outcomes_and_diagnostics() {
    let sends = ProxyEffects {
        worker_observations: InterpreterRequests::one(Work(5)),
        worker_initializations: InterpreterRequests::one(Work(6)),
        worker_activations: InterpreterRequests::one(Work(7)),
        worker_shutdowns: InterpreterRequests::one(Work(8)),
        worker_deliveries: InterpreterRequests::one(Work(9)),
        owner_outcomes: InterpreterRequests::one(Work(10)),
        diagnostics: InterpreterRequests::one(Work(11)),
    };
    let mut runtime = Runtime::new([]);

    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(sends, &mut runtime).await;

    assert_eq!(runtime.attempts, [5, 6, 7, 8, 9, 10, 11]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("accepted proxy reports cannot corrupt interpretation");
    };
    assert!(matches!(
        settlement.worker_observations.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(5)))]
    ));
    assert!(matches!(
        settlement.worker_initializations.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(6)))]
    ));
    assert!(matches!(
        settlement.worker_activations.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(7)))]
    ));
    assert!(matches!(
        settlement.worker_shutdowns.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(8)))]
    ));
    assert!(matches!(
        settlement.worker_deliveries.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(9)))]
    ));
    assert!(matches!(
        settlement.owner_outcomes.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(10)))]
    ));
    assert!(matches!(
        settlement.diagnostics.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(11)))]
    ));
}

#[test]
fn concrete_proxy_effects_have_one_total_interpretation_path() {
    fn requires_total<Sends>()
    where
        Sends: InterpretSends<ProxyRuntimeWitness, (), Here>,
    {
    }

    requires_total::<<StableProxy<ProxyWorker, ImmediateActivation> as Behavior>::Sends>();
}
