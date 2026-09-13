use behavior::ActionItem;
use behavior::Actions;
use behavior::Address;
use behavior::Behavior;
use behavior::Births;
use behavior::ChildCreationOutcome;
use behavior::ChildHead;
use behavior::ChildNamespaceExhausted;
use behavior::CreateChild;
use behavior::CreationCorrelation;
use behavior::CreationId;
use behavior::CreationRejection;
use behavior::CreationSequence;
use behavior::CreationSettlement;
use behavior::Creations;
use behavior::EndpointAddress;
use behavior::EstablishChild;
use behavior::EstablishedCreation;
use behavior::EstablishedRecipient;
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
use core::marker::PhantomData;
use std::collections::HashMap;

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
    type Accepted = u8;
    type Rejection = &'static str;
    type Prerequisite = CreationCorrelation<Child, ChildHead>;
}

impl ActionItem for Independent {
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
    async fn interpret_item(
        &mut self,
        creations: Creations<CreateChild<RuntimeAddr, Child>>,
    ) -> ItemSettlement<
        Creations<CreateChild<RuntimeAddr, Child>>,
        Creations<RoutedCreation<RuntimeAddr, Child>>,
        ChildNamespaceExhausted,
        Never,
    > {
        let mut route = 40_u64;
        ItemSettlement::Accepted(creations.map(|creation| {
            route += 1;
            RoutedCreation::new(creation, route)
        }))
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
                ItemSettlement::Accepted(ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        id,
                        kind,
                        EstablishedRecipient::issued(RuntimeEndpoint(route, PhantomData)),
                    ),
                })
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
    async fn interpret_item(
        &mut self,
        item: ChildWork,
    ) -> ItemSettlement<ChildWork, u8, &'static str, CreationCorrelation<Child, ChildHead>> {
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
    }
}

impl<RootEvent, Path> InterpretItem<Independent, RootEvent, Path> for Runtime {
    async fn interpret_item(
        &mut self,
        item: Independent,
    ) -> ItemSettlement<Independent, u8, Never, Never> {
        self.sends.push("independent");
        ItemSettlement::Accepted(item.0)
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

    let Interpretation::Complete(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
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

    let Interpretation::Complete(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
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
