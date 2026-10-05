use core::future::{Future, poll_fn};
use core::pin::pin;
use core::task::Poll;

use behavior::ActionItem;
use behavior::Actions;
use behavior::Address;
use behavior::Behavior;
use behavior::Births;
use behavior::ChildCreationOutcome;
use behavior::ChildHead;
use behavior::CommittedChild;
use behavior::CreateChild;
use behavior::CreationCorrelation;
use behavior::CreationId;
use behavior::CreationRejection;
use behavior::CreationSequence;
use behavior::CreationSettlement;
use behavior::Creations;
use behavior::EndpointAddress;
use behavior::EstablishChild;
use behavior::EstablishedActor;
use behavior::Here;
use behavior::InterpretItem;
use behavior::Interpretation;
use behavior::InterpreterRequests;
use behavior::ItemSettlement;
use behavior::Never;
use behavior::NoBirths;
use behavior::NoSends;
use behavior::Protocol;
use behavior::RoutedCreation;
use behavior::SendLayer;
use behavior::SettledItem;
use behavior::Step;
use behavior::User;
use behavior::{InterpretationProgress, finish_item, prepare_item};
use core::marker::PhantomData;
use std::collections::HashMap;

mod installed_control;
use installed_control::InstalledControl;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Debug, Eq, PartialEq)]
struct RuntimeEndpoint<P>(u64, PhantomData<fn() -> P>);

impl<P> Clone for RuntimeEndpoint<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for RuntimeEndpoint<P> {}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = RuntimeEndpoint<P>
    where
        P: Protocol<Addr = Self>;

    type Installed<B>
        = InstalledControl<B, RuntimeEndpoint<B::Protocol>>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> RuntimeEndpoint<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed.endpoint()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Child(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildError {
    InitializationRejected,
}

impl Protocol for Child {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Child {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = ChildError;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChildWork {
    creation: CreationId,
    value: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Independent(u8);

impl ActionItem for ChildWork {
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

    type Accepted = u8;
    type Rejection = &'static str;
    type Prerequisite = CreationCorrelation<Child, ChildHead>;
}

impl ActionItem for Independent {
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

    type Accepted = u8;
    type Rejection = Never;
    type Prerequisite = Never;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreationStatus {
    Created,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreationPlan {
    CreateChild,
    RejectInitialization,
    RejectByHost,
}

#[derive(Default)]
struct Runtime {
    plans: HashMap<CreationId, CreationPlan>,
    resolutions: HashMap<CreationId, CreationStatus>,
    sends: Vec<&'static str>,
}

impl Runtime {
    fn with_plan(id: CreationId, plan: CreationPlan) -> Self {
        Self {
            plans: HashMap::from([(id, plan)]),
            resolutions: HashMap::new(),
            sends: Vec::new(),
        }
    }
}

impl<RootEvent, Path> InterpretItem<Creations<CreateChild<RuntimeAddr, Child>>, RootEvent, Path>
    for Runtime
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Creations<CreateChild<RuntimeAddr, Child>>>,
        received: &'a mut Option<<Creations<CreateChild<RuntimeAddr, Child>> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Creations<CreateChild<RuntimeAddr, Child>>: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(creations) = input.take() else {
                return;
            };
            let producer = async move {
                let mut route = 40_u64;
                ItemSettlement::Accepted(creations.map(|creation| {
                    route += 1;
                    RoutedCreation::new(creation, route)
                }))
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

impl EstablishChild<ChildHead, Child> for Runtime {
    async fn establish_child(
        &mut self,
        creation: RoutedCreation<RuntimeAddr, Child>,
    ) -> ItemSettlement<
        RoutedCreation<RuntimeAddr, Child>,
        ChildCreationOutcome<Child, ChildHead>,
        CreationRejection,
        Never,
    > {
        let id = creation.id();
        let plan = self
            .plans
            .get(&id)
            .copied()
            .unwrap_or(CreationPlan::CreateChild);
        match plan {
            CreationPlan::CreateChild => {
                self.resolutions.insert(id, CreationStatus::Created);
                let kind = creation.kind();
                let route = creation.route();
                let (creation, _) = creation.into_parts();
                let (_, _child, _) = creation.into_parts();
                ItemSettlement::Accepted(ChildCreationOutcome::Established(CommittedChild::new(
                    id,
                    kind,
                    EstablishedActor::issued(InstalledControl::new(RuntimeEndpoint(
                        route,
                        PhantomData,
                    ))),
                )))
            }
            CreationPlan::RejectInitialization => {
                self.resolutions.insert(id, CreationStatus::Rejected);
                ItemSettlement::Accepted(ChildCreationOutcome::InitializationRejected {
                    creation,
                    error: ChildError::InitializationRejected,
                })
            }
            CreationPlan::RejectByHost => {
                self.resolutions.insert(id, CreationStatus::Rejected);
                ItemSettlement::Accepted(ChildCreationOutcome::HostRejected {
                    creation,
                    initialization: Actions::cont(),
                    reason: CreationRejection::EnvironmentFailed,
                })
            }
        }
    }
}

impl<RootEvent, Path> InterpretItem<ChildWork, RootEvent, Path> for Runtime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ChildWork>,
        received: &'a mut Option<<ChildWork as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ChildWork: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            let producer = async move {
                self.sends.push("child");
                match self.resolutions.get(&item.creation) {
                    Some(CreationStatus::Created) => ItemSettlement::Accepted(item.value),
                    Some(CreationStatus::Rejected) => ItemSettlement::Blocked {
                        item,
                        prerequisite: CreationCorrelation::new(item.creation),
                    },
                    None => ItemSettlement::Rejected {
                        item,
                        reason: "missing child binding",
                    },
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

impl<RootEvent, Path> InterpretItem<Independent, RootEvent, Path> for Runtime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Independent>,
        received: &'a mut Option<<Independent as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Independent: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            let producer = async move {
                self.sends.push("independent");
                ItemSettlement::Accepted(item.0)
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

type Sends = SendLayer<InterpreterRequests<Independent>, InterpreterRequests<ChildWork>>;
type TestActions = Actions<RuntimeAddr, Never, Sends, Births<Child>>;

fn creation_id() -> CreationId {
    CreationSequence::new()
        .issue()
        .expect("the first creation ID exists")
}

#[tokio::test]
async fn creation_rejection_blocks_only_its_exact_dependents() {
    let id = creation_id();
    let actions = TestActions::new(
        SendLayer::new(
            InterpreterRequests::one(Independent(9)),
            InterpreterRequests::new(vec![
                ChildWork {
                    creation: id,
                    value: 3,
                },
                ChildWork {
                    creation: id,
                    value: 4,
                },
            ]),
        ),
        Creations::one(CreateChild::birth(id, Child(1))),
        Step::Continue,
    );
    let mut runtime = Runtime::with_plan(id, CreationPlan::RejectByHost);

    let Interpretation::Complete(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        TestActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual dependent creation/actions product must retain its full settlement");
        };
        settlement
    }) else {
        panic!("expected a complete action settlement");
    };
    let CreationSettlement::Settled(creations) = settlement.creations else {
        panic!("the creation batch must have routed");
    };
    let Some(SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::HostRejected {
            creation,
            initialization,
            reason,
        },
    ))) = creations.into_iter().next()
    else {
        panic!("expected the complete child-host rejection");
    };
    assert_eq!(creation.id(), id);
    assert_eq!(creation.route(), 41);
    assert_eq!(initialization.sends, NoSends);
    assert!(initialization.creates.is_empty());
    assert_eq!(reason, CreationRejection::EnvironmentFailed);
    assert_eq!(runtime.sends, ["child", "child", "independent"]);
    assert!(settlement.sends.inner.iter().all(|item| matches!(
        item,
        SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. })
            if prerequisite.id() == id
    )));
    assert_eq!(
        settlement.sends.owned,
        [SettledItem::Attempted(ItemSettlement::Accepted(9))]
    );
}

#[tokio::test]
async fn initialization_rejection_returns_the_current_child_and_exact_error() {
    let id = creation_id();
    let actions = TestActions::new(
        SendLayer::new(
            InterpreterRequests::one(Independent(9)),
            InterpreterRequests::one(ChildWork {
                creation: id,
                value: 3,
            }),
        ),
        Creations::one(CreateChild::birth(id, Child(1))),
        Step::Continue,
    );
    let mut runtime = Runtime::with_plan(id, CreationPlan::RejectInitialization);

    let Interpretation::Complete(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        TestActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual dependent creation/actions product must retain its full settlement");
        };
        settlement
    }) else {
        panic!("expected a complete action settlement");
    };
    let CreationSettlement::Settled(creations) = settlement.creations else {
        panic!("the creation batch must have routed");
    };
    let Some(SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::InitializationRejected { creation, error },
    ))) = creations.into_iter().next()
    else {
        panic!("expected the exact initialization rejection");
    };
    assert_eq!(creation.id(), id);
    assert_eq!(creation.route(), 41);
    assert_eq!(error, ChildError::InitializationRejected);
    assert_eq!(runtime.sends, ["child", "independent"]);
}
