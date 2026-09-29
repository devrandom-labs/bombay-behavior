//! Exact owner input and settlement for one stable proxy.

use core::marker::PhantomData;
use std::sync::Arc;

use behavior::{
    ActionItem, ActionItemResult, Address, Behavior, BehaviorAddr, ChildInputReason, CreationId,
    EndpointAddress, EstablishedActor, ItemSettlement, Never, SettledItem, SourceAction,
};

use crate::WorkerSubmission;

use super::{ActivationPlan, ProxyControl, StableProxy};

/// Affine correlation returned after one stable-proxy input is accepted.
///
/// ```compile_fail,E0382
/// fn duplicate(id: behavior_actors::atomic::ProxyOperationId) {
///     let accepted = id;
///     let duplicate = id;
/// }
/// ```
#[doc(hidden)]
#[must_use = "a proxy operation ID must return through settlement or retire outward"]
pub struct ProxyOperationId {
    token: Arc<()>,
}

pub(crate) struct ProxyOperationWitness {
    token: Arc<()>,
}

impl ProxyOperationWitness {
    fn reserve() -> (Self, ProxyOperationId) {
        let token = Arc::new(());
        (
            Self {
                token: Arc::clone(&token),
            },
            ProxyOperationId { token },
        )
    }

    pub(crate) fn admit<Source, Worker, Plan>(
        self,
        settlement: ProxyInputResult<Source, Worker, Plan>,
    ) -> Result<
        ProxyInputResult<Source, Worker, Plan>,
        (Self, ProxyInputResult<Source, Worker, Plan>),
    >
    where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        let operation = match &settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => &receipt.operation,
            SettledItem::Attempted(
                ItemSettlement::Rejected {
                    item: operation, ..
                }
                | ItemSettlement::Corrupt {
                    item: operation, ..
                },
            )
            | SettledItem::Unattempted(operation) => operation.operation_id(),
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match *prerequisite {}
            }
        };
        if Arc::ptr_eq(&self.token, &operation.token) {
            Ok(settlement)
        } else {
            Err((self, settlement))
        }
    }

    pub(crate) fn admit_receipt<Worker, Plan>(
        self,
        receipt: ProxyInputReceipt<Worker, Plan>,
    ) -> Result<ProxyInputReceipt<Worker, Plan>, (Self, ProxyInputReceipt<Worker, Plan>)>
    where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        if Arc::ptr_eq(&self.token, &receipt.operation.token) {
            Ok(receipt)
        } else {
            Err((self, receipt))
        }
    }

    pub(crate) fn admit_rejection<Source, Worker, Plan>(
        self,
        operation: ProxyOperation<Source, Worker, Plan>,
        reason: ChildInputReason,
    ) -> Result<
        (ProxyOperation<Source, Worker, Plan>, ChildInputReason),
        (Self, ProxyOperation<Source, Worker, Plan>, ChildInputReason),
    >
    where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        if Arc::ptr_eq(&self.token, &operation.operation.token) {
            Ok((operation, reason))
        } else {
            Err((self, operation, reason))
        }
    }
}

#[cfg(test)]
mod direct_operation_admission_contract {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use super::{ActivationPlan, ProxyInputResult, ProxyOperationWitness, StableProxy};

    #[expect(dead_code, reason = "compile contract for affine operation admission")]
    fn admit<Source, Worker, Plan>(
        witness: ProxyOperationWitness,
        settlement: ProxyInputResult<Source, Worker, Plan>,
    ) -> Result<
        ProxyInputResult<Source, Worker, Plan>,
        (
            ProxyOperationWitness,
            ProxyInputResult<Source, Worker, Plan>,
        ),
    >
    where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        witness.admit(settlement)
    }
}

/// One exact private input to a stable-proxy child.
#[doc(hidden)]
#[must_use = "a proxy operation must be interpreted or retained"]
pub struct ProxyOperation<Source, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    creation: CreationId,
    control: ProxyControl<Worker, Plan>,
    operation: ProxyOperationId,
    source: PhantomData<fn() -> Source>,
}

/// Exact immediate result of submitting one private proxy input.
#[doc(hidden)]
pub type ProxyInputResult<Source, Worker, Plan> = ActionItemResult<
    ProxyOperation<Source, Worker, Plan>,
    ProxyInputReceipt<Worker, Plan>,
    ChildInputReason,
    Never,
>;

/// Trusted interpreter port for one exact private stable-proxy control.
///
/// The interpreter selects the child occurrence statically, uses `creation`
/// only within that occurrence, and returns the exact actor only after the
/// control is admitted. A rejected or corrupt admission returns the actual
/// owned control. This port cannot see or construct the operation ID retained
/// by [`ProxyOperation`].
pub trait ProxyControlAdmission<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// Admit one control or return its complete owned rejection.
    fn admit_proxy_control(
        &mut self,
        creation: CreationId,
        control: ProxyControl<Worker, Plan>,
    ) -> ItemSettlement<
        ProxyControl<Worker, Plan>,
        EstablishedActor<StableProxy<Worker, Plan>>,
        ChildInputReason,
        Never,
    >;
}

impl<Source, Worker, Plan> ProxyOperation<Source, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(crate) fn initial(
        creation: CreationId,
        submission: WorkerSubmission<Worker, Plan>,
    ) -> (ProxyOperationWitness, Self) {
        let (witness, operation) = ProxyOperationWitness::reserve();
        (
            witness,
            Self {
                creation,
                control: ProxyControl::start_with(submission.worker, submission.activation),
                operation,
                source: PhantomData,
            },
        )
    }

    pub(crate) fn replacement(
        creation: CreationId,
        submission: WorkerSubmission<Worker, Plan>,
    ) -> (ProxyOperationWitness, Self) {
        let (witness, operation) = ProxyOperationWitness::reserve();
        (
            witness,
            Self {
                creation,
                control: ProxyControl::replace_with(submission.worker, submission.activation),
                operation,
                source: PhantomData,
            },
        )
    }

    pub(crate) fn shutdown(creation: CreationId) -> (ProxyOperationWitness, Self) {
        let (witness, operation) = ProxyOperationWitness::reserve();
        (
            witness,
            Self {
                creation,
                control: ProxyControl::shutdown(),
                operation,
                source: PhantomData,
            },
        )
    }

    /// Inspect the exact proxy creation before attempting control admission.
    #[doc(hidden)]
    #[must_use]
    pub const fn creation(&self) -> CreationId {
        self.creation
    }

    const fn operation_id(&self) -> &ProxyOperationId {
        &self.operation
    }

    /// Transfer every owned part within the atomic catalogue.
    #[cfg(test)]
    #[must_use]
    pub(in crate::atomic) fn into_parts(
        self,
    ) -> (CreationId, ProxyControl<Worker, Plan>, ProxyOperationId) {
        (self.creation, self.control, self.operation)
    }

    /// Transfer only the complete control while retaining the operation ID
    /// until the lower interpreter returns its exact admission outcome.
    pub fn settle<Host>(
        self,
        host: &mut Host,
    ) -> ItemSettlement<Self, ProxyInputReceipt<Worker, Plan>, ChildInputReason, Never>
    where
        Host: ProxyControlAdmission<Worker, Plan>,
    {
        let Self {
            creation,
            control,
            operation,
            source: _,
        } = self;
        match host.admit_proxy_control(creation, control) {
            ItemSettlement::Accepted(proxy) => ItemSettlement::Accepted(ProxyInputReceipt {
                creation,
                proxy,
                operation,
            }),
            ItemSettlement::Rejected {
                item: control,
                reason,
            } => ItemSettlement::Rejected {
                item: Self {
                    creation,
                    control,
                    operation,
                    source: PhantomData,
                },
                reason,
            },
            ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
            ItemSettlement::Corrupt {
                item: control,
                fault,
            } => ItemSettlement::Corrupt {
                item: Self {
                    creation,
                    control,
                    operation,
                    source: PhantomData,
                },
                fault,
            },
        }
    }
}

/// Exact receipt for one accepted stable-proxy input.
#[doc(hidden)]
#[must_use = "accepted proxy input must return to its owning aggregate"]
pub struct ProxyInputReceipt<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    creation: CreationId,
    proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    operation: ProxyOperationId,
}

impl<Worker, Plan> ProxyInputReceipt<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(crate) const fn creation(&self) -> CreationId {
        self.creation
    }

    /// Construct an exact receipt for internal catalogue tests.
    #[cfg(test)]
    #[must_use]
    pub(in crate::atomic) const fn new(
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        operation: ProxyOperationId,
    ) -> Self {
        Self {
            creation,
            proxy,
            operation,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CreationId,
        EstablishedActor<StableProxy<Worker, Plan>>,
        ProxyOperationId,
    ) {
        (self.creation, self.proxy, self.operation)
    }
}

impl<Source, Worker, Plan> ActionItem for ProxyOperation<Source, Worker, Plan>
where
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Send,
{
    type Accepted = ProxyInputReceipt<Worker, Plan>;
    type Rejection = ChildInputReason;
    type Prerequisite = Never;
}

impl<Source, Worker, Plan> SourceAction for ProxyOperation<Source, Worker, Plan>
where
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Send,
{
    type Source = Source;
}

#[cfg(test)]
mod tests {
    use core::future::Future;
    use std::collections::VecDeque;

    use behavior::{
        ActiveTurn, Address, BehaviorActed, ClassifySettlement, InterpreterFault, NoBirths,
        NoSends, Protocol, SettlementStatus, User,
    };

    use super::super::ProxyCommand;
    use super::*;
    use crate::atomic::ImmediateActivation;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RuntimeAddr(u64);

    impl Address for RuntimeAddr {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Endpoint(u64);

    impl EndpointAddress for RuntimeAddr {
        type Established<P>
            = Endpoint
        where
            P: Protocol<Addr = Self>;
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Worker(u8);

    impl Protocol for Worker {
        type Addr = RuntimeAddr;
        type Msg = Never;
    }

    impl Behavior for Worker {
        type Protocol = Self;
        type Event = User<RuntimeAddr, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    struct OwnedActivation(Box<str>);

    impl ActivationPlan for OwnedActivation {
        type Ready = Box<str>;
        type Rejection = Never;

        fn activate(self) -> impl Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
            core::future::ready(Ok(self.0))
        }
    }

    struct Owner;

    fn creation(id: u64) -> CreationId {
        let mut sequence = behavior::CreationSequence::new();
        (0..id)
            .map(|_| sequence.issue())
            .last()
            .flatten()
            .unwrap_or_else(|| panic!("test creation ID is issued"))
    }

    enum ControlAdmission {
        Accept(EstablishedActor<StableProxy<Worker, ImmediateActivation>>),
        Reject,
    }

    struct ProxyControlHost {
        admissions: VecDeque<ControlAdmission>,
        observed: Vec<(u64, u8)>,
    }

    impl ProxyControlHost {
        fn admit(
            &mut self,
            creation: CreationId,
            control: ProxyControl<Worker, ImmediateActivation>,
        ) -> ItemSettlement<
            ProxyControl<Worker, ImmediateActivation>,
            EstablishedActor<StableProxy<Worker, ImmediateActivation>>,
            ChildInputReason,
            Never,
        > {
            let worker = match &control.command {
                ProxyCommand::Start(submission) | ProxyCommand::Replace(submission) => {
                    submission.worker.0
                }
                ProxyCommand::Shutdown => panic!("this witness issues worker starts only"),
            };
            self.observed.push((creation.get(), worker));
            match self
                .admissions
                .pop_front()
                .expect("one lower admission per proxy control")
            {
                ControlAdmission::Accept(proxy) => ItemSettlement::Accepted(proxy),
                ControlAdmission::Reject => ItemSettlement::Rejected {
                    item: control,
                    reason: ChildInputReason::ClosedControlLane,
                },
            }
        }
    }

    impl ProxyControlAdmission<Worker, ImmediateActivation> for ProxyControlHost {
        fn admit_proxy_control(
            &mut self,
            creation: CreationId,
            control: ProxyControl<Worker, ImmediateActivation>,
        ) -> ItemSettlement<
            ProxyControl<Worker, ImmediateActivation>,
            EstablishedActor<StableProxy<Worker, ImmediateActivation>>,
            ChildInputReason,
            Never,
        > {
            self.admit(creation, control)
        }
    }

    enum ObservedControl {
        Start(usize),
        Replace(usize),
        Shutdown,
    }

    struct RejectingProxyControlHost {
        reason: ChildInputReason,
        observed: Vec<(CreationId, ObservedControl)>,
    }

    impl ProxyControlAdmission<Worker, OwnedActivation> for RejectingProxyControlHost {
        fn admit_proxy_control(
            &mut self,
            creation: CreationId,
            control: ProxyControl<Worker, OwnedActivation>,
        ) -> ItemSettlement<
            ProxyControl<Worker, OwnedActivation>,
            EstablishedActor<StableProxy<Worker, OwnedActivation>>,
            ChildInputReason,
            Never,
        > {
            let observed = match &control.command {
                ProxyCommand::Start(submission) => {
                    ObservedControl::Start(submission.activation.0.as_ptr() as usize)
                }
                ProxyCommand::Replace(submission) => {
                    ObservedControl::Replace(submission.activation.0.as_ptr() as usize)
                }
                ProxyCommand::Shutdown => ObservedControl::Shutdown,
            };
            self.observed.push((creation, observed));
            ItemSettlement::Rejected {
                item: control,
                reason: self.reason,
            }
        }
    }

    #[test]
    fn exact_proxy_control_settlement_keeps_operation_evidence_private() {
        let first_creation = creation(21);
        let second_creation = creation(22);
        let (first_witness, first) = ProxyOperation::<Owner, _, _>::initial(
            first_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let (second_witness, second) = ProxyOperation::<Owner, _, _>::replacement(
            second_creation,
            WorkerSubmission::immediate(Worker(8)),
        );
        let proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(Endpoint(31));
        let expected_proxy = proxy.recipient();
        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy), ControlAdmission::Reject]),
            observed: Vec::new(),
        };

        let ItemSettlement::Accepted(accepted) = second.settle(&mut host) else {
            panic!("the second control reaches the exact proxy");
        };
        let accepted = second_witness
            .admit_receipt(accepted)
            .unwrap_or_else(|_| panic!("the accepted receipt retains the second operation ID"));
        let (accepted_creation, accepted_proxy, accepted_operation) = accepted.into_parts();
        assert_eq!(accepted_creation, second_creation);
        assert_eq!(accepted_proxy.recipient(), expected_proxy);
        drop(accepted_operation);

        let ItemSettlement::Rejected {
            item: returned,
            reason,
        } = first.settle(&mut host)
        else {
            panic!("the first control returns its actual rejected request");
        };
        let (returned, reason) = first_witness
            .admit_rejection(returned, reason)
            .unwrap_or_else(|_| panic!("the rejected request retains the first operation ID"));
        let (returned_creation, control, returned_operation) = returned.into_parts();
        assert_eq!(returned_creation, first_creation);
        assert_eq!(reason, ChildInputReason::ClosedControlLane);
        match control.command {
            ProxyCommand::Start(submission) => assert_eq!(submission.worker, Worker(7)),
            ProxyCommand::Replace(_) | ProxyCommand::Shutdown => {
                panic!("rejection returns the original start control")
            }
        }
        drop(returned_operation);
        assert_eq!(host.observed, [(22, 8), (21, 7)]);
        assert!(host.admissions.is_empty());
    }

    #[test]
    fn rejected_proxy_control_returns_move_only_plan_and_shutdown_authority() {
        let activation = OwnedActivation(Box::from("activation custody"));
        let expected_plan = activation.0.as_ptr();
        let first_creation = creation(23);
        let (first_witness, first) = ProxyOperation::<Owner, _, _>::initial(
            first_creation,
            WorkerSubmission::activated(Worker(9), activation),
        );
        let second_creation = creation(24);
        let (second_witness, second) =
            ProxyOperation::<Owner, Worker, OwnedActivation>::shutdown(second_creation);
        let replacement_activation = OwnedActivation(Box::from("replacement custody"));
        let expected_replacement_plan = replacement_activation.0.as_ptr();
        let third_creation = creation(25);
        let (third_witness, third) = ProxyOperation::<Owner, _, _>::replacement(
            third_creation,
            WorkerSubmission::activated(Worker(10), replacement_activation),
        );
        let mut host = RejectingProxyControlHost {
            reason: ChildInputReason::ClosedControlLane,
            observed: Vec::new(),
        };

        let ItemSettlement::Rejected {
            item: returned,
            reason,
        } = first.settle(&mut host)
        else {
            panic!("closed private control returns the complete start request");
        };
        let (returned, reason) = first_witness
            .admit_rejection(returned, reason)
            .unwrap_or_else(|_| panic!("the first operation ID remains paired"));
        assert_eq!(reason, ChildInputReason::ClosedControlLane);
        let (creation, control, operation) = returned.into_parts();
        assert_eq!(creation, first_creation);
        match control.command {
            ProxyCommand::Start(submission) => {
                assert_eq!(submission.worker, Worker(9));
                assert_eq!(submission.activation.0.as_ptr(), expected_plan);
                assert_eq!(&*submission.activation.0, "activation custody");
            }
            ProxyCommand::Replace(_) | ProxyCommand::Shutdown => {
                panic!("the rejected start remains a start")
            }
        }
        drop(operation);

        host.reason = ChildInputReason::MissingBinding;
        let ItemSettlement::Rejected {
            item: returned,
            reason,
        } = second.settle(&mut host)
        else {
            panic!("missing binding returns the complete shutdown request");
        };
        let (returned, reason) = second_witness
            .admit_rejection(returned, reason)
            .unwrap_or_else(|_| panic!("the shutdown operation ID remains paired"));
        assert_eq!(reason, ChildInputReason::MissingBinding);
        let (creation, control, operation) = returned.into_parts();
        assert_eq!(creation, second_creation);
        assert!(matches!(control.command, ProxyCommand::Shutdown));
        drop(operation);

        host.reason = ChildInputReason::ClosedControlLane;
        let ItemSettlement::Rejected {
            item: returned,
            reason,
        } = third.settle(&mut host)
        else {
            panic!("closed private control returns the complete replacement request");
        };
        let (returned, reason) = third_witness
            .admit_rejection(returned, reason)
            .unwrap_or_else(|_| panic!("the replacement operation ID remains paired"));
        assert_eq!(reason, ChildInputReason::ClosedControlLane);
        let (creation, control, operation) = returned.into_parts();
        assert_eq!(creation, third_creation);
        match control.command {
            ProxyCommand::Replace(submission) => {
                assert_eq!(submission.worker, Worker(10));
                assert_eq!(submission.activation.0.as_ptr(), expected_replacement_plan);
                assert_eq!(&*submission.activation.0, "replacement custody");
            }
            ProxyCommand::Start(_) | ProxyCommand::Shutdown => {
                panic!("the rejected replacement remains a replacement")
            }
        }
        drop(operation);

        assert_eq!(host.observed.len(), 3);
        assert_eq!(host.observed[0].0, first_creation);
        assert!(matches!(
            host.observed[0].1,
            ObservedControl::Start(pointer) if pointer == expected_plan as usize
        ));
        assert_eq!(host.observed[1].0, second_creation);
        assert!(matches!(host.observed[1].1, ObservedControl::Shutdown));
        assert_eq!(host.observed[2].0, third_creation);
        assert!(matches!(
            host.observed[2].1,
            ObservedControl::Replace(pointer) if pointer == expected_replacement_plan as usize
        ));
    }

    #[test]
    fn accepted_input_returns_the_exact_proxy_and_operation_id() {
        let (witness, operation) = ProxyOperation::<Owner, _, _>::initial(
            creation(3),
            WorkerSubmission::immediate(Worker(7)),
        );
        let (creation, control, operation) = operation.into_parts();
        match control.command {
            ProxyCommand::Start(submission) => assert_eq!(submission.worker, Worker(7)),
            ProxyCommand::Replace(_) | ProxyCommand::Shutdown => {
                panic!("initial operation retains initial control")
            }
        }
        let proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(Endpoint(11));
        let expected_proxy = proxy.recipient();
        let accepted = ProxyInputReceipt::new(creation, proxy, operation);
        let result: ProxyInputResult<Owner, Worker, ImmediateActivation> =
            SettledItem::Attempted(ItemSettlement::Accepted(accepted));
        let result = witness
            .admit(result)
            .unwrap_or_else(|_| panic!("the exact witness admits its operation settlement"));

        match result {
            SettledItem::Attempted(ItemSettlement::Accepted(accepted)) => {
                let (creation, proxy, operation) = accepted.into_parts();
                assert_eq!(creation.get(), 3);
                assert_eq!(proxy.recipient(), expected_proxy);
                assert_eq!(proxy.clone().recipient(), expected_proxy);
                drop(operation);
            }
            SettledItem::Attempted(
                ItemSettlement::Rejected { .. }
                | ItemSettlement::Blocked { .. }
                | ItemSettlement::Corrupt { .. },
            )
            | SettledItem::Unattempted(_) => panic!("acceptance stays accepted"),
        }
    }

    #[test]
    fn another_reservation_cannot_satisfy_the_operation_witness() {
        let (search_witness, search) = ProxyOperation::<Owner, _, _>::initial(
            creation(4),
            WorkerSubmission::immediate(Worker(8)),
        );
        let (index_witness, index) = ProxyOperation::<Owner, _, _>::initial(
            creation(5),
            WorkerSubmission::immediate(Worker(9)),
        );
        let index = SettledItem::Unattempted(index);
        let (search_witness, index) = match search_witness.admit(index) {
            Ok(_) => panic!("a foreign operation cannot satisfy this witness"),
            Err(returned) => returned,
        };
        let search = search_witness
            .admit(SettledItem::Unattempted(search))
            .unwrap_or_else(|_| panic!("the search witness admits its exact operation"));
        let index = index_witness
            .admit(index)
            .unwrap_or_else(|_| panic!("the index witness admits its exact operation"));
        match (search, index) {
            (SettledItem::Unattempted(search), SettledItem::Unattempted(index)) => {
                assert_eq!(search.creation().get(), 4);
                assert_eq!(index.creation().get(), 5);
            }
            _ => panic!("admission preserves both settlement alternatives"),
        }
    }

    #[test]
    fn shutdown_operation_carries_only_the_owner_shutdown_command() {
        let (witness, operation) =
            ProxyOperation::<Owner, Worker, ImmediateActivation>::shutdown(creation(6));
        let operation = witness
            .admit(SettledItem::Unattempted(operation))
            .unwrap_or_else(|_| panic!("the shutdown witness admits its exact operation"));
        let operation = match operation {
            SettledItem::Unattempted(operation) => operation,
            _ => panic!("admission preserves the settlement alternative"),
        };
        let (creation, control, operation) = operation.into_parts();

        assert_eq!(creation.get(), 6);
        drop(operation);
        match control.command {
            ProxyCommand::Shutdown => {}
            ProxyCommand::Start(_) | ProxyCommand::Replace(_) => {
                panic!("proxy retirement cannot start or replace a worker")
            }
        }
    }

    #[test]
    fn rejection_corruption_and_no_attempt_retain_the_complete_operation() {
        let (rejected_witness, rejected) = ProxyOperation::<Owner, _, _>::replacement(
            creation(6),
            WorkerSubmission::immediate(Worker(10)),
        );
        let rejected: ProxyInputResult<Owner, Worker, ImmediateActivation> =
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: rejected,
                reason: ChildInputReason::ClosedControlLane,
            });
        let rejected = rejected_witness
            .admit(rejected)
            .unwrap_or_else(|_| panic!("the rejection returns to its exact witness"));
        match rejected {
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: operation,
                reason,
            }) => {
                let (_, control, _) = operation.into_parts();
                assert_eq!(reason, ChildInputReason::ClosedControlLane);
                match control.command {
                    ProxyCommand::Replace(submission) => {
                        assert_eq!(submission.worker, Worker(10));
                    }
                    ProxyCommand::Start(_) | ProxyCommand::Shutdown => {
                        panic!("replacement rejection retains replacement control")
                    }
                }
            }
            SettledItem::Attempted(
                ItemSettlement::Accepted(_)
                | ItemSettlement::Blocked { .. }
                | ItemSettlement::Corrupt { .. },
            )
            | SettledItem::Unattempted(_) => panic!("rejection stays rejected"),
        }

        let (corrupt_witness, corrupt) = ProxyOperation::<Owner, _, _>::initial(
            creation(7),
            WorkerSubmission::immediate(Worker(11)),
        );
        let corrupt: ProxyInputResult<Owner, Worker, ImmediateActivation> =
            SettledItem::Attempted(ItemSettlement::Corrupt {
                item: corrupt,
                fault: InterpreterFault::CorruptTraversal,
            });
        let corrupt = corrupt_witness
            .admit(corrupt)
            .unwrap_or_else(|_| panic!("the corrupt result returns to its exact witness"));
        assert_eq!(corrupt.settlement_status(), SettlementStatus::Corrupt);
        match corrupt {
            SettledItem::Attempted(ItemSettlement::Corrupt {
                item: operation,
                fault,
            }) => {
                let (_, _, _) = operation.into_parts();
                assert_eq!(fault, InterpreterFault::CorruptTraversal);
            }
            SettledItem::Attempted(
                ItemSettlement::Accepted(_)
                | ItemSettlement::Rejected { .. }
                | ItemSettlement::Blocked { .. },
            )
            | SettledItem::Unattempted(_) => panic!("corruption stays corrupt"),
        }

        let (untouched_witness, untouched) = ProxyOperation::<Owner, _, _>::initial(
            creation(8),
            WorkerSubmission::immediate(Worker(12)),
        );
        let untouched: ProxyInputResult<Owner, Worker, ImmediateActivation> =
            SettledItem::Unattempted(untouched);
        let untouched = untouched_witness
            .admit(untouched)
            .unwrap_or_else(|_| panic!("the unattempted input returns to its exact witness"));
        assert_eq!(untouched.settlement_status(), SettlementStatus::Corrupt);
        match untouched {
            SettledItem::Unattempted(operation) => {
                let (_, _, _) = operation.into_parts();
            }
            SettledItem::Attempted(_) => panic!("unattempted stays unattempted"),
        }
    }
}
