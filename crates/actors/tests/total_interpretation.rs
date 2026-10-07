mod installed_control;

use core::future::{Future, poll_fn};
use core::pin::pin;
use core::task::Poll;

use behavior::{
    ActionItem, Actions, ActiveTurn, Address, Behavior, BehaviorActed, ChildHead,
    ClassifySettlement, EndpointAddress, EstablishedDelivery, Here, InterpretItem, InterpretSends,
    Interpretation, InterpretationProgress, InterpreterFault, InterpreterRequests, ItemSettlement,
    Never, NoBirths, Protocol, ReportToParent, SettledItem, SettlementStatus, User, finish_item,
    prepare_item,
};
use behavior_actors::atomic::{
    BeginActivation, ImmediateActivation, InitializeWorker, ProxyDiagnostic, ProxyEffects,
    ProxyOutcome, StableProxy,
};
use behavior_actors::{
    DeliveryOutcomes, LeaseSends, ObserveChild, PresenceSends, ShutdownEstablished, StopOnShutdown,
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
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

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

    type Installed<B>
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
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

impl<RootEvent, Path> InterpretItem<ObserveChild<ProxyWorker, ChildHead>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ObserveChild<ProxyWorker, ChildHead>>,
        received: &'a mut Option<<ObserveChild<ProxyWorker, ChildHead> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ObserveChild<ProxyWorker, ChildHead>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path>
    InterpretItem<InitializeWorker<ProxyWorker, ImmediateActivation>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<InitializeWorker<ProxyWorker, ImmediateActivation>>,
        received: &'a mut Option<
            <InitializeWorker<ProxyWorker, ImmediateActivation> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        InitializeWorker<ProxyWorker, ImmediateActivation>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path>
    InterpretItem<BeginActivation<ProxyWorker, ImmediateActivation>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<BeginActivation<ProxyWorker, ImmediateActivation>>,
        received: &'a mut Option<
            <BeginActivation<ProxyWorker, ImmediateActivation> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        BeginActivation<ProxyWorker, ImmediateActivation>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path>
    InterpretItem<ShutdownEstablished<StopOnShutdown<ProxyWorker>, Here>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ShutdownEstablished<StopOnShutdown<ProxyWorker>, Here>>,
        received: &'a mut Option<
            <ShutdownEstablished<StopOnShutdown<ProxyWorker>, Here> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ShutdownEstablished<StopOnShutdown<ProxyWorker>, Here>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path> InterpretItem<EstablishedDelivery<ProxyWorker>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<EstablishedDelivery<ProxyWorker>>,
        received: &'a mut Option<<EstablishedDelivery<ProxyWorker> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        EstablishedDelivery<ProxyWorker>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path>
    InterpretItem<ReportToParent<ProxyOutcome<ProxyWorker, ImmediateActivation>>, RootEvent, Path>
    for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ReportToParent<ProxyOutcome<ProxyWorker, ImmediateActivation>>>,
        received: &'a mut Option<
            <ReportToParent<ProxyOutcome<ProxyWorker, ImmediateActivation>> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ReportToParent<ProxyOutcome<ProxyWorker, ImmediateActivation>>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
            }
        }
    }
}

impl<RootEvent, Path>
    InterpretItem<
        ReportToParent<ProxyDiagnostic<ProxyWorker, ImmediateActivation>>,
        RootEvent,
        Path,
    > for ProxyRuntimeWitness
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ReportToParent<ProxyDiagnostic<ProxyWorker, ImmediateActivation>>>,
        received: &'a mut Option<<ReportToParent<ProxyDiagnostic<ProxyWorker, ImmediateActivation>> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ReportToParent<ProxyDiagnostic<ProxyWorker, ImmediateActivation>>: 'a,
    {
        async move {
            if received.is_none() {
                if let Some(item) = input.take() {
                    *received = Some(ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::MissingCapability,
                    });
                }
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
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Work>,
        received: &'a mut Option<<Work as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Work: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            let producer = {
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
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

#[tokio::test]
async fn delivery_outcomes_continue_after_rejection_at_the_same_event_path() {
    let sends = DeliveryOutcomes {
        deliveries: InterpreterRequests::one(Work(1)),
        outcomes: InterpreterRequests::one(Work(2)),
    };
    let mut runtime = Runtime::new([(1, Plan::Reject)]);

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact named product must return its actual settlement");
        };
        settlement
    };
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

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact named product must return its actual settlement");
        };
        settlement
    };
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
async fn presence_reports_settle_before_schedules_after_rejection() {
    let sends = PresenceSends {
        replies: InterpreterRequests::one(Work(12)),
        schedules: InterpreterRequests::one(Work(13)),
    };
    let mut runtime = Runtime::new([(12, Plan::Reject)]);

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact named product must return its actual settlement");
        };
        settlement
    };
    assert_eq!(runtime.attempts, [12, 13]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("lawful rejection cannot skip the independent schedule lane");
    };
    assert_eq!(settlement.settlement_status(), SettlementStatus::Rejected);
    assert!(matches!(
        settlement.replies.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Rejected {
            item: Work(12),
            reason: WorkRejection::Closed,
        })]
    ));
    assert!(matches!(
        settlement.schedules.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(Work(13)))]
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

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact named product must return its actual settlement");
        };
        settlement
    };

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
