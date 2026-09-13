//! Exact owner input and settlement for one stable proxy.

use std::sync::Arc;

use core::marker::PhantomData;

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

    /// Transfer every owned part to Bombay after the exact child is resolved.
    #[doc(hidden)]
    #[must_use]
    pub fn into_parts(self) -> (CreationId, ProxyControl<Worker, Plan>, ProxyOperationId) {
        (self.creation, self.control, self.operation)
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

    /// Construct Bombay's receipt after exact child control admission succeeds.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(
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

    struct Owner;

    fn creation(id: u64) -> CreationId {
        let mut sequence = behavior::CreationSequence::new();
        (0..id)
            .map(|_| sequence.issue())
            .last()
            .flatten()
            .unwrap_or_else(|| panic!("test creation ID is issued"))
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
