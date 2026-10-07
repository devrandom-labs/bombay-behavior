//! Exact owner input and settlement for one stable proxy.

use core::marker::PhantomData;
use std::sync::Arc;

use behavior::{
    ActionItem, ActionItemResult, Address, Behavior, BehaviorAddr, ChildInputReason, CreationId,
    EndpointAddress, EstablishedActor, Interpretation, InterpretationProgress, ItemSettlement,
    Never, SettledItem, SourceAction,
};

use crate::WorkerSubmission;

use super::{ActivationPlan, ProxyControl, StableProxy};

/// Affine correlation returned after one stable-proxy input is accepted.
#[must_use = "a proxy operation ID must return through settlement or retire outward"]
pub(crate) struct ProxyOperationId {
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

/// Caller-owned partial admission of one original stable-proxy operation.
///
/// The issued operation authority and creator correlation remain mandatory
/// while only the actual control is loaned to an interpreter. Construction is
/// restricted to interpreting the original ProxyOperation; this owner provides
/// no authority constructor, field access, getter, Clone, or event policy.
pub struct ProxyOperationCustody<Source, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    creation: CreationId,
    operation: ProxyOperationId,
    control: Option<ProxyControl<Worker, Plan>>,
    reply: Option<
        ItemSettlement<
            ProxyControl<Worker, Plan>,
            EstablishedActor<StableProxy<Worker, Plan>>,
            ChildInputReason,
            Never,
        >,
    >,
    source: PhantomData<fn() -> Source>,
}

/// Exact immediate result of submitting one private proxy input.
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

    /// Inspect the exact creator-local proxy creation correlation before
    /// attempting control admission. This is not an actor identity or proof
    /// that the proxy was installed.
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

    /// Admit only the actual control while the caller retains the original
    /// creator correlation, operation authority, and acquired lower reply.
    pub fn settle<Host>(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                ProxyOperationCustody<Source, Worker, Plan>,
                ItemSettlement<Self, ProxyInputReceipt<Worker, Plan>, ChildInputReason, Never>,
            >,
        >,
        host: &mut Host,
    ) where
        Host: ProxyControlAdmission<Worker, Plan>,
    {
        Self::prepare_interpretation(progress);
        if let Some(InterpretationProgress::Interpreting(custody)) = progress {
            if let Some(((creation, input), received)) = Self::interpretation_input(custody) {
                if let Some(control) = input.take() {
                    *received = Some(host.admit_proxy_control(creation, control));
                }
            }
        }
        Self::finish_interpretation(progress);
    }

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                ProxyOperationCustody<Source, Worker, Plan>,
                ItemSettlement<Self, ProxyInputReceipt<Worker, Plan>, ChildInputReason, Never>,
            >,
        >,
    ) {
        if !matches!(progress, Some(InterpretationProgress::Original(_))) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Original(Self {
                creation,
                control,
                operation,
                source,
            })) => {
                *progress = Some(InterpretationProgress::Interpreting(
                    ProxyOperationCustody {
                        creation,
                        operation,
                        control: Some(control),
                        reply: None,
                        source,
                    },
                ));
            }
            retained => *progress = retained,
        }
    }

    fn interpretation_input<'a>(
        custody: &'a mut ProxyOperationCustody<Source, Worker, Plan>,
    ) -> Option<(
        (CreationId, &'a mut Option<ProxyControl<Worker, Plan>>),
        &'a mut Option<
            ItemSettlement<
                ProxyControl<Worker, Plan>,
                EstablishedActor<StableProxy<Worker, Plan>>,
                ChildInputReason,
                Never,
            >,
        >,
    )>
    where
        Self: 'a,
    {
        let ProxyOperationCustody {
            creation,
            control,
            reply,
            ..
        } = custody;
        if control.is_some() && reply.is_none() {
            Some(((*creation, control), reply))
        } else {
            None
        }
    }

    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                ProxyOperationCustody<Source, Worker, Plan>,
                ItemSettlement<Self, ProxyInputReceipt<Worker, Plan>, ChildInputReason, Never>,
            >,
        >,
    ) {
        if !matches!(
            progress,
            Some(InterpretationProgress::Interpreting(
                ProxyOperationCustody {
                    control: None,
                    reply: Some(_),
                    ..
                }
            ))
        ) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Interpreting(ProxyOperationCustody {
                creation,
                operation,
                control: None,
                reply: Some(reply),
                source,
            })) => {
                let received = match reply {
                    ItemSettlement::Accepted(proxy) => {
                        ItemSettlement::Accepted(ProxyInputReceipt {
                            creation,
                            proxy,
                            operation,
                        })
                    }
                    ItemSettlement::Rejected {
                        item: control,
                        reason,
                    } => ItemSettlement::Rejected {
                        item: Self {
                            creation,
                            control,
                            operation,
                            source,
                        },
                        reason,
                    },
                    ItemSettlement::Corrupt {
                        item: control,
                        fault,
                    } => ItemSettlement::Corrupt {
                        item: Self {
                            creation,
                            control,
                            operation,
                            source,
                        },
                        fault,
                    },
                    ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
                };
                let interpretation = match received {
                    received @ ItemSettlement::Corrupt { .. } => Interpretation::Corrupt(received),
                    received => Interpretation::Complete(received),
                };
                *progress = Some(InterpretationProgress::Completed(interpretation));
            }
            retained => *progress = retained,
        }
    }
}

/// Exact receipt for one accepted stable-proxy input.
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
    type Custody = ProxyOperationCustody<Source, Worker, Plan>;
    type Input<'a>
        = (CreationId, &'a mut Option<ProxyControl<Worker, Plan>>)
    where
        Self: 'a;
    type Reply = ItemSettlement<
        ProxyControl<Worker, Plan>,
        EstablishedActor<StableProxy<Worker, Plan>>,
        ChildInputReason,
        Never,
    >;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    ) {
        Self::prepare_interpretation(progress)
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        Self::interpretation_input(custody)
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    ) {
        Self::finish_interpretation(progress)
    }

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
    use core::pin::pin;
    use core::task::{Context, Poll, Waker};
    use std::collections::VecDeque;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, mpsc};

    use behavior::{
        ActiveTurn, Address, BehaviorActed, ClassifySettlement, CreationSequence,
        EstablishedDelivery, EstablishedRecipient, ExactDeliveryReason, Here, InterpretItem,
        InterpretSends, Interpretation, InterpreterFault, InterpreterRequests, MessageProtocol,
        NoBirths, NoSends, Protocol, SendLayer, SettledItem, SettlementStatus, User,
    };

    use super::super::ProxyCommand;
    use super::*;
    use crate::atomic::pool::assignment::{
        AcceptedJobSequence, AssignWorker, AssignedJob, Assignment, AssignmentReceipt,
        AssignmentSequence, CorrelationMatch, CustomerJob,
    };
    use crate::atomic::{ImmediateActivation, WorkerAttempt};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RuntimeAddr(u64);

    impl Address for RuntimeAddr {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Endpoint(u64);

    struct Installed<B: Behavior> {
        endpoint: Endpoint,
        control: mpsc::Sender<B::Event>,
        inbox: Arc<Mutex<mpsc::Receiver<B::Event>>>,
    }

    impl<B: Behavior> Clone for Installed<B> {
        fn clone(&self) -> Self {
            Self {
                endpoint: self.endpoint,
                control: self.control.clone(),
                inbox: Arc::clone(&self.inbox),
            }
        }
    }

    impl<B: Behavior> Installed<B> {
        fn new(endpoint: Endpoint) -> Self {
            let (control, inbox) = mpsc::channel();
            Self {
                endpoint,
                control,
                inbox: Arc::new(Mutex::new(inbox)),
            }
        }
    }

    impl EndpointAddress for RuntimeAddr {
        type Established<P>
            = Endpoint
        where
            P: Protocol<Addr = Self>;

        type Installed<B>
            = Installed<B>
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>;

        fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>,
        {
            installed.endpoint
        }
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
        Corrupt,
        Interrupt,
    }

    enum AssignmentAdmission {
        Accept,
        Reject,
        Corrupt,
        Interrupt,
    }

    #[derive(Debug, Eq, PartialEq)]
    enum CapabilityAttempt {
        Proxy(CreationId, u8),
        Assignment(usize),
    }

    struct ProxyControlHost {
        admissions: VecDeque<ControlAdmission>,
        observed: Vec<(u64, u8)>,
        assignment_admissions: VecDeque<AssignmentAdmission>,
        attempts: Vec<CapabilityAttempt>,
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
            self.attempts
                .push(CapabilityAttempt::Proxy(creation, worker));
            match self
                .admissions
                .pop_front()
                .expect("one lower admission per proxy control")
            {
                ControlAdmission::Accept(proxy) => ItemSettlement::Accepted(proxy),
                ControlAdmission::Corrupt => ItemSettlement::Corrupt {
                    item: control,
                    fault: InterpreterFault::CorruptTraversal,
                },
                ControlAdmission::Interrupt => {
                    panic!("application proxy control conversion interrupted")
                }
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
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let expected_proxy = proxy.recipient();
        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy), ControlAdmission::Reject]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::new(),
            attempts: Vec::new(),
        };

        let ItemSettlement::Accepted(accepted) = ({
            let mut progress = Some(InterpretationProgress::Original(second));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        }) else {
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
        } = ({
            let mut progress = Some(InterpretationProgress::Original(first));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        })
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
        } = ({
            let mut progress = Some(InterpretationProgress::Original(first));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        })
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
        } = ({
            let mut progress = Some(InterpretationProgress::Original(second));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        })
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
        } = ({
            let mut progress = Some(InterpretationProgress::Original(third));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        })
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
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(11)),
        );
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

    impl<Path>
        InterpretItem<
            EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
            (),
            Path,
        > for ProxyControlHost
    {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<
                EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
            >,
            received: &'a mut Option<<EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>> as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let Some(delivery) = input.take() else {
                    return;
                };
                *received = Some({
                    self.attempts.push(CapabilityAttempt::Assignment(
                        delivery.message.payload().as_ptr() as usize,
                    ));
                    match self
                        .assignment_admissions
                        .pop_front()
                        .expect("one actual assignment admission")
                    {
                        AssignmentAdmission::Accept => ItemSettlement::Accepted(()),
                        AssignmentAdmission::Reject => ItemSettlement::Rejected {
                            item: delivery,
                            reason: ExactDeliveryReason::ClosedRecipient,
                        },
                        AssignmentAdmission::Corrupt => ItemSettlement::Corrupt {
                            item: delivery,
                            fault: InterpreterFault::CorruptTraversal,
                        },
                        AssignmentAdmission::Interrupt => {
                            panic!("application assignment conversion interrupted")
                        }
                    }
                });
            }
        }
    }

    impl<Path>
        InterpretItem<
            AssignWorker<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>, Box<str>>,
            (),
            Path,
        > for ProxyControlHost
    {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<
                EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
            >,
            received: &'a mut Option<<EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>> as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            AssignWorker<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>, Box<str>>: 'a,
        {
            <Self as InterpretItem<
                EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
                (),
                Path,
            >>::interpret_item(self, input, received)
        }
    }

    impl<Path> InterpretItem<ProxyOperation<Owner, Worker, ImmediateActivation>, (), Path>
        for ProxyControlHost
    {
        fn interpret_item<'a>(
            &'a mut self,
            input: (
                CreationId,
                &'a mut Option<ProxyControl<Worker, ImmediateActivation>>,
            ),
            received: &'a mut Option<
                ItemSettlement<
                    ProxyControl<Worker, ImmediateActivation>,
                    EstablishedActor<StableProxy<Worker, ImmediateActivation>>,
                    ChildInputReason,
                    Never,
                >,
            >,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            ProxyOperation<Owner, Worker, ImmediateActivation>: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let (creation, input) = input;
                let Some(control) = input.take() else {
                    return;
                };
                *received = Some(self.admit_proxy_control(creation, control));
            }
        }
    }

    #[tokio::test]
    async fn unrelated_templates_proxy_then_assignment_complete_original_products() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let original_proxy_recipient = proxy.recipient();

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Reject]),
            attempts: Vec::new(),
        };
        let product = SendLayer::new(
            InterpreterRequests::one(assignment_request),
            InterpreterRequests::one(proxy_request),
        );
        let interpretation = {
            let mut progress = Some(InterpretationProgress::Original(product));
            InterpretSends::<_, (), Here>::interpret(&mut progress, &mut host).await;
            let Some(InterpretationProgress::Completed(interpretation)) = progress else {
                panic!("both real templates must complete their exact product");
            };
            interpretation
        };
        let Interpretation::Complete(product) = interpretation else {
            panic!("both real templates complete");
        };
        let SendLayer { owned, inner } = product;
        let (mut assignment_rows, mut proxy_rows) = (owned, inner);

        let assignment_row = assignment_rows
            .pop()
            .expect("whole original assignment row");
        let proxy_row = proxy_rows.pop().expect("whole original proxy row");
        let SettledItem::Attempted(ItemSettlement::Rejected {
            item: returned,
            reason,
        }) = assignment_row
        else {
            panic!("actual original rejection");
        };
        let (target, assignment, receipt) = returned.into_parts();
        let receipt_match = assigned.compare_receipt(&receipt);
        let SettledItem::Attempted(ItemSettlement::Accepted(proxy_receipt)) = proxy_row else {
            panic!("actual proxy admission");
        };
        let proxy_receipt = proxy_witness
            .admit_receipt(proxy_receipt)
            .unwrap_or_else(|_| panic!("original proxy operation correlation"));
        let (returned_creation, returned_proxy, operation) = proxy_receipt.into_parts();
        assert_eq!(target, assignment_target);
        assert_eq!(assignment.payload().as_ptr() as usize, original_payload);
        assert_eq!(&**assignment.payload(), "original assignment");
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(reason, ExactDeliveryReason::ClosedRecipient);
        assert_eq!(returned_creation, proxy_creation);
        assert_eq!(returned_proxy.recipient(), original_proxy_recipient);
        assert!(assignment_rows.is_empty());
        assert!(proxy_rows.is_empty());
        assert_eq!(
            host.attempts,
            [
                CapabilityAttempt::Proxy(proxy_creation, 7),
                CapabilityAttempt::Assignment(original_payload)
            ]
        );
        assert_eq!(host.observed, [(proxy_creation.get(), 7)]);
        assert!(host.admissions.is_empty());
        assert!(host.assignment_admissions.is_empty());
        drop(operation);
        assert_eq!(proxy_allocation.strong_count(), 0);
    }

    #[test]
    fn unrelated_templates_proxy_then_assignment_borrowed_progress_retains_original_proxy_allocation()
     {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Interrupt]),
            attempts: Vec::new(),
        };

        let product = SendLayer::new(
            InterpreterRequests::one(assignment_request),
            InterpreterRequests::one(proxy_request),
        );
        let mut progress = Some(InterpretationProgress::Original(product));
        let (interruption, retained_owners) = {
            let mut interpretation = pin!(InterpretSends::<_, (), Here>::interpret(
                &mut progress,
                &mut host
            ));
            let mut context = Context::from_waker(Waker::noop());
            let interruption = catch_unwind(AssertUnwindSafe(|| {
                interpretation.as_mut().poll(&mut context)
            }));
            (interruption, proxy_allocation.strong_count())
        };
        drop(progress);
        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(assigned);
        drop(worker_attempt);
        drop(proxy_witness);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Proxy(proxy_creation, 7),
                CapabilityAttempt::Assignment(original_payload)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(discharged_owners, 0);
        // Acquired prefix remains in caller-owned progress until explicit discharge.
        assert_eq!(
            retained_owners, 2,
            "original proxy correlation must coexist with its external witness"
        );
    }

    #[test]
    fn unrelated_templates_proxy_then_assignment_lexical_receipt_custody_survives_lower_panic() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let original_proxy_recipient = proxy.recipient();

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Interrupt]),
            attempts: Vec::new(),
        };

        let ItemSettlement::Accepted(proxy_receipt) = ({
            let mut progress = Some(InterpretationProgress::Original(proxy_request));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        }) else {
            panic!("normal first proxy admission");
        };
        let (target, assignment, receipt) = assignment_request.into_parts();
        let mut delivery = Some(EstablishedDelivery::new(target, assignment));
        let mut received = None;
        let (interruption, retained_owners) = {
            let mut lower = pin!(<ProxyControlHost as InterpretItem<
                EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
                (),
                Here,
            >>::interpret_item(
                &mut host, &mut delivery, &mut received
            ));
            let mut context = Context::from_waker(Waker::noop());
            let interruption = catch_unwind(AssertUnwindSafe(|| lower.as_mut().poll(&mut context)));
            (interruption, proxy_allocation.strong_count())
        };
        drop(delivery);
        drop(received);
        let receipt_match = assigned.compare_receipt(&receipt);
        let proxy_receipt = proxy_witness
            .admit_receipt(proxy_receipt)
            .unwrap_or_else(|_| panic!("whole prior proxy receipt"));
        let (returned_creation, returned_proxy, operation) = proxy_receipt.into_parts();
        let returned_recipient = returned_proxy.recipient();
        drop(returned_proxy);
        drop(operation);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(receipt);
        drop(assigned);
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_creation, proxy_creation);
        assert_eq!(returned_recipient, original_proxy_recipient);

        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Proxy(proxy_creation, 7),
                CapabilityAttempt::Assignment(original_payload)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }

    #[tokio::test]
    async fn unrelated_templates_assignment_then_proxy_complete_original_products() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let original_proxy_recipient = proxy.recipient();

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Reject]),
            attempts: Vec::new(),
        };
        let product = SendLayer::new(
            InterpreterRequests::one(proxy_request),
            InterpreterRequests::one(assignment_request),
        );
        let interpretation = {
            let mut progress = Some(InterpretationProgress::Original(product));
            InterpretSends::<_, (), Here>::interpret(&mut progress, &mut host).await;
            let Some(InterpretationProgress::Completed(interpretation)) = progress else {
                panic!("both real templates must complete their exact product");
            };
            interpretation
        };
        let Interpretation::Complete(product) = interpretation else {
            panic!("both real templates complete");
        };
        let SendLayer { owned, inner } = product;
        let (mut proxy_rows, mut assignment_rows) = (owned, inner);

        let assignment_row = assignment_rows
            .pop()
            .expect("whole original assignment row");
        let proxy_row = proxy_rows.pop().expect("whole original proxy row");
        let SettledItem::Attempted(ItemSettlement::Rejected {
            item: returned,
            reason,
        }) = assignment_row
        else {
            panic!("actual original rejection");
        };
        let (target, assignment, receipt) = returned.into_parts();
        let receipt_match = assigned.compare_receipt(&receipt);
        let SettledItem::Attempted(ItemSettlement::Accepted(proxy_receipt)) = proxy_row else {
            panic!("actual proxy admission");
        };
        let proxy_receipt = proxy_witness
            .admit_receipt(proxy_receipt)
            .unwrap_or_else(|_| panic!("original proxy operation correlation"));
        let (returned_creation, returned_proxy, operation) = proxy_receipt.into_parts();
        assert_eq!(target, assignment_target);
        assert_eq!(assignment.payload().as_ptr() as usize, original_payload);
        assert_eq!(&**assignment.payload(), "original assignment");
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(reason, ExactDeliveryReason::ClosedRecipient);
        assert_eq!(returned_creation, proxy_creation);
        assert_eq!(returned_proxy.recipient(), original_proxy_recipient);
        assert!(assignment_rows.is_empty());
        assert!(proxy_rows.is_empty());
        assert_eq!(
            host.attempts,
            [
                CapabilityAttempt::Assignment(original_payload),
                CapabilityAttempt::Proxy(proxy_creation, 7)
            ]
        );
        assert_eq!(host.observed, [(proxy_creation.get(), 7)]);
        assert!(host.admissions.is_empty());
        assert!(host.assignment_admissions.is_empty());
        drop(operation);
        assert_eq!(proxy_allocation.strong_count(), 0);
    }

    #[test]
    fn unrelated_templates_assignment_then_proxy_borrowed_progress_retains_original_proxy_allocation()
     {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Interrupt]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Accept]),
            attempts: Vec::new(),
        };
        drop(proxy);

        let product = SendLayer::new(
            InterpreterRequests::one(proxy_request),
            InterpreterRequests::one(assignment_request),
        );
        let mut progress = Some(InterpretationProgress::Original(product));
        let (interruption, retained_owners) = {
            let mut interpretation = pin!(InterpretSends::<_, (), Here>::interpret(
                &mut progress,
                &mut host
            ));
            let mut context = Context::from_waker(Waker::noop());
            let interruption = catch_unwind(AssertUnwindSafe(|| {
                interpretation.as_mut().poll(&mut context)
            }));
            (interruption, proxy_allocation.strong_count())
        };
        drop(progress);
        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(assigned);
        drop(worker_attempt);
        drop(proxy_witness);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Assignment(original_payload),
                CapabilityAttempt::Proxy(proxy_creation, 7)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(discharged_owners, 0);
        // Acquired prefix remains in caller-owned progress until explicit discharge.
        assert_eq!(
            retained_owners, 2,
            "original proxy correlation must coexist with its external witness"
        );
    }

    #[test]
    fn unrelated_templates_assignment_then_proxy_lexical_receipt_custody_survives_lower_panic() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Interrupt]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Accept]),
            attempts: Vec::new(),
        };

        drop(proxy);
        let mut context = Context::from_waker(Waker::noop());
        let mut assignment_progress = Some(InterpretationProgress::Original(assignment_request));
        let settlement = {
            let mut normal = pin!(AssignWorker::<
                MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
                Box<str>,
            >::settle::<_, (), Here>(
                &mut assignment_progress, &mut host
            ));
            normal.as_mut().poll(&mut context)
        };
        let Poll::Ready(()) = settlement else {
            panic!("normal first assignment producer completes");
        };
        let Some(InterpretationProgress::Completed(Interpretation::Complete(
            ItemSettlement::Accepted(receipt),
        ))) = assignment_progress
        else {
            panic!("normal first assignment receipt");
        };
        let (returned_creation, control, operation) = proxy_request.into_parts();
        let interruption = catch_unwind(AssertUnwindSafe(|| {
            host.admit_proxy_control(returned_creation, control)
        }));
        let retained_owners = proxy_allocation.strong_count();
        let receipt_match = assigned.compare_receipt(&receipt);
        let original_operation = Arc::ptr_eq(&operation.token, &proxy_witness.token);
        drop(operation);
        drop(proxy_witness);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(receipt);
        drop(assigned);
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_creation, proxy_creation);
        assert!(original_operation);

        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Assignment(original_payload),
                CapabilityAttempt::Proxy(proxy_creation, 7)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }
    enum NativeBoundary {
        BeforeInputTransfer,
        ApplicationConsumption,
    }

    impl ProxyControlHost {
        async fn interpret_assignment_custody(
            &mut self,
            delivery: &mut Option<
                EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
            >,
            boundary: NativeBoundary,
        ) -> ItemSettlement<
            EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
            (),
            ExactDeliveryReason,
            Never,
        > {
            match boundary {
                NativeBoundary::BeforeInputTransfer => {
                    panic!("host interrupted before acquiring assignment input")
                }
                NativeBoundary::ApplicationConsumption => {
                    let original = delivery
                        .take()
                        .expect("original assignment input retained before acquisition");
                    let mut input = Some(original);
                    let mut received = None;
                    <Self as InterpretItem<
                        EstablishedDelivery<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>>,
                        (),
                        Here,
                    >>::interpret_item(self, &mut input, &mut received)
                    .await;
                    received.expect("normal lower assignment callback publishes its actual reply")
                }
            }
        }

        fn admit_proxy_custody(
            &mut self,
            creation: CreationId,
            control: &mut Option<ProxyControl<Worker, ImmediateActivation>>,
            boundary: NativeBoundary,
        ) -> ItemSettlement<
            ProxyControl<Worker, ImmediateActivation>,
            EstablishedActor<StableProxy<Worker, ImmediateActivation>>,
            ChildInputReason,
            Never,
        > {
            match boundary {
                NativeBoundary::BeforeInputTransfer => {
                    panic!("host interrupted before acquiring proxy input")
                }
                NativeBoundary::ApplicationConsumption => {
                    let original = control
                        .take()
                        .expect("original proxy input retained before acquisition");
                    self.admit_proxy_control(creation, original)
                }
            }
        }
    }
    #[test]
    fn unrelated_templates_proxy_then_assignment_borrowed_custody_before_input_transfer() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let original_proxy_recipient = proxy.recipient();

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Interrupt]),
            attempts: Vec::new(),
        };

        let ItemSettlement::Accepted(proxy_receipt) = ({
            let mut progress = Some(InterpretationProgress::Original(proxy_request));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        }) else {
            panic!("normal first proxy admission");
        };
        let (target, assignment, receipt) = assignment_request.into_parts();
        let mut delivery = Some(EstablishedDelivery::new(target, assignment));
        let (interruption, retained_owners) = {
            let mut lower =
                pin!(host.interpret_assignment_custody(
                    &mut delivery,
                    NativeBoundary::BeforeInputTransfer
                ));
            let mut context = Context::from_waker(Waker::noop());
            let interruption = catch_unwind(AssertUnwindSafe(|| lower.as_mut().poll(&mut context)));
            (interruption, proxy_allocation.strong_count())
        };
        let original_delivery = delivery
            .as_ref()
            .expect("unacquired assignment remains caller-owned after loan disposal");
        let returned_payload = original_delivery.message.payload().as_ptr() as usize;
        let returned_contents = original_delivery.message.payload().to_string();
        let returned_target = original_delivery.to.clone();
        let receipt_match = assigned.compare_receipt(&receipt);
        let proxy_receipt = proxy_witness
            .admit_receipt(proxy_receipt)
            .unwrap_or_else(|_| panic!("whole prior proxy receipt"));
        let (returned_creation, returned_proxy, operation) = proxy_receipt.into_parts();
        let returned_recipient = returned_proxy.recipient();
        drop(returned_proxy);
        drop(operation);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(delivery);
        drop(receipt);
        drop(assigned);
        let assignment_target_observed = assignment_target.clone();
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_payload, original_payload);
        assert_eq!(returned_contents, "original assignment");
        assert_eq!(returned_target, assignment_target_observed);
        assert_eq!(returned_creation, proxy_creation);
        assert_eq!(returned_recipient, original_proxy_recipient);

        assert_eq!(attempts, [CapabilityAttempt::Proxy(proxy_creation, 7)]);
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert_eq!(assignment_admissions.len(), 1);
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }
    #[test]
    fn unrelated_templates_proxy_then_assignment_borrowed_custody_application_consumption() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );
        let original_proxy_recipient = proxy.recipient();

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Interrupt]),
            attempts: Vec::new(),
        };

        let ItemSettlement::Accepted(proxy_receipt) = ({
            let mut progress = Some(InterpretationProgress::Original(proxy_request));
            ProxyOperation::settle(&mut progress, &mut host);
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        }) else {
            panic!("normal first proxy admission");
        };
        let (target, assignment, receipt) = assignment_request.into_parts();
        let mut delivery = Some(EstablishedDelivery::new(target, assignment));
        let (interruption, retained_owners) = {
            let mut lower = pin!(host.interpret_assignment_custody(
                &mut delivery,
                NativeBoundary::ApplicationConsumption
            ));
            let mut context = Context::from_waker(Waker::noop());
            let interruption = catch_unwind(AssertUnwindSafe(|| lower.as_mut().poll(&mut context)));
            (interruption, proxy_allocation.strong_count())
        };
        let remaining_delivery = delivery;
        let receipt_match = assigned.compare_receipt(&receipt);
        let proxy_receipt = proxy_witness
            .admit_receipt(proxy_receipt)
            .unwrap_or_else(|_| panic!("whole prior proxy receipt"));
        let (returned_creation, returned_proxy, operation) = proxy_receipt.into_parts();
        let returned_recipient = returned_proxy.recipient();
        drop(returned_proxy);
        drop(operation);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(receipt);
        drop(assigned);
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(remaining_delivery.is_none());
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_creation, proxy_creation);
        assert_eq!(returned_recipient, original_proxy_recipient);

        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Proxy(proxy_creation, 7),
                CapabilityAttempt::Assignment(original_payload)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }
    #[test]
    fn unrelated_templates_assignment_then_proxy_borrowed_custody_before_input_transfer() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Interrupt]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Accept]),
            attempts: Vec::new(),
        };

        drop(proxy);
        let mut context = Context::from_waker(Waker::noop());
        let mut assignment_progress = Some(InterpretationProgress::Original(assignment_request));
        let settlement = {
            let mut normal = pin!(AssignWorker::<
                MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
                Box<str>,
            >::settle::<_, (), Here>(
                &mut assignment_progress, &mut host
            ));
            normal.as_mut().poll(&mut context)
        };
        let Poll::Ready(()) = settlement else {
            panic!("normal first assignment producer completes");
        };
        let Some(InterpretationProgress::Completed(Interpretation::Complete(
            ItemSettlement::Accepted(receipt),
        ))) = assignment_progress
        else {
            panic!("normal first assignment receipt");
        };
        let (returned_creation, control, operation) = proxy_request.into_parts();
        let mut control = Some(control);
        let interruption = catch_unwind(AssertUnwindSafe(|| {
            host.admit_proxy_custody(
                returned_creation,
                &mut control,
                NativeBoundary::BeforeInputTransfer,
            )
        }));
        let original_control = control
            .as_ref()
            .expect("unacquired proxy input remains caller-owned after call disposal");
        let returned_worker = match &original_control.command {
            ProxyCommand::Start(submission) => submission.worker.0,
            ProxyCommand::Replace(_) | ProxyCommand::Shutdown => {
                panic!("original input is the issued worker start")
            }
        };
        let retained_owners = proxy_allocation.strong_count();
        let receipt_match = assigned.compare_receipt(&receipt);
        let original_operation = Arc::ptr_eq(&operation.token, &proxy_witness.token);
        drop(operation);
        drop(proxy_witness);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(receipt);
        drop(assigned);
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_creation, proxy_creation);
        assert!(original_operation);
        assert_eq!(returned_worker, 7);

        assert_eq!(attempts, [CapabilityAttempt::Assignment(original_payload)]);
        assert!(observed.is_empty());
        assert_eq!(admissions.len(), 1);
        assert!(assignment_admissions.is_empty());
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }
    #[test]
    fn unrelated_templates_assignment_then_proxy_borrowed_custody_application_consumption() {
        let mut creations = CreationSequence::new();
        let proxy_creation = creations.issue().expect("original proxy creation");
        let worker_creation = creations.issue().expect("original worker attempt");
        let worker_attempt = WorkerAttempt::issued(worker_creation);
        let mut assignments = AssignmentSequence::new();
        let (correlation, assignment) = assignments
            .assign(&worker_attempt, Box::<str>::from("original assignment"))
            .expect("original assignment correlation");
        let original_payload = assignment.payload().as_ptr() as usize;
        let assignment_target = EstablishedRecipient::issued(Endpoint(41));
        let assignment_request = AssignWorker::<
            MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
            _,
        >::new(assignment_target.clone(), &correlation, assignment);
        let mut jobs = AcceptedJobSequence::new();
        let (job_id, admitted) = jobs.issue().expect("original customer admission");
        let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
            CustomerJob {
                id: job_id,
                admitted,
                payload: 7,
                customer: 9,
            },
            correlation,
        );
        let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
            proxy_creation,
            WorkerSubmission::immediate(Worker(7)),
        );
        let proxy_allocation = Arc::downgrade(&proxy_witness.token);
        let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
            Installed::new(Endpoint(31)),
        );

        let mut host = ProxyControlHost {
            admissions: VecDeque::from([ControlAdmission::Interrupt]),
            observed: Vec::new(),
            assignment_admissions: VecDeque::from([AssignmentAdmission::Accept]),
            attempts: Vec::new(),
        };

        drop(proxy);
        let mut context = Context::from_waker(Waker::noop());
        let mut assignment_progress = Some(InterpretationProgress::Original(assignment_request));
        let settlement = {
            let mut normal = pin!(AssignWorker::<
                MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
                Box<str>,
            >::settle::<_, (), Here>(
                &mut assignment_progress, &mut host
            ));
            normal.as_mut().poll(&mut context)
        };
        let Poll::Ready(()) = settlement else {
            panic!("normal first assignment producer completes");
        };
        let Some(InterpretationProgress::Completed(Interpretation::Complete(
            ItemSettlement::Accepted(receipt),
        ))) = assignment_progress
        else {
            panic!("normal first assignment receipt");
        };
        let (returned_creation, control, operation) = proxy_request.into_parts();
        let mut control = Some(control);
        let interruption = catch_unwind(AssertUnwindSafe(|| {
            host.admit_proxy_custody(
                returned_creation,
                &mut control,
                NativeBoundary::ApplicationConsumption,
            )
        }));
        let remaining_control = control;
        let retained_owners = proxy_allocation.strong_count();
        let receipt_match = assigned.compare_receipt(&receipt);
        let original_operation = Arc::ptr_eq(&operation.token, &proxy_witness.token);
        drop(operation);
        drop(proxy_witness);

        let ProxyControlHost {
            admissions,
            observed,
            assignment_admissions,
            attempts,
        } = host;
        drop(receipt);
        drop(assigned);
        drop(worker_attempt);
        let discharged_owners = proxy_allocation.strong_count();
        let interruption_was_caught = interruption.is_err();
        drop(interruption);
        assert!(interruption_was_caught);
        assert!(remaining_control.is_none());
        assert!(matches!(receipt_match, CorrelationMatch::Exact));
        assert_eq!(returned_creation, proxy_creation);
        assert!(original_operation);

        assert_eq!(
            attempts,
            [
                CapabilityAttempt::Assignment(original_payload),
                CapabilityAttempt::Proxy(proxy_creation, 7)
            ]
        );
        assert_eq!(observed, [(proxy_creation.get(), 7)]);
        assert!(admissions.is_empty());
        assert!(assignment_admissions.is_empty());
        assert_eq!(retained_owners, 2);
        assert_eq!(discharged_owners, 0);
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum BorrowedNormalAdmission {
        Accepted,
        Rejected,
        Corrupt,
    }

    #[tokio::test]
    async fn unrelated_templates_proxy_then_assignment_borrowed_normal_products() {
        for expected in [
            BorrowedNormalAdmission::Accepted,
            BorrowedNormalAdmission::Rejected,
            BorrowedNormalAdmission::Corrupt,
        ] {
            let mut creations = CreationSequence::new();
            let proxy_creation = creations.issue().expect("original proxy creation");
            let worker_creation = creations.issue().expect("original worker attempt");
            let worker_attempt = WorkerAttempt::issued(worker_creation);
            let mut assignments = AssignmentSequence::new();
            let (correlation, assignment) = assignments
                .assign(&worker_attempt, Box::<str>::from("original assignment"))
                .expect("original assignment correlation");
            let original_payload = assignment.payload().as_ptr() as usize;
            let assignment_target = EstablishedRecipient::issued(Endpoint(41));
            let assignment_request = AssignWorker::<
                MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
                _,
            >::new(
                assignment_target.clone(), &correlation, assignment
            );
            let mut jobs = AcceptedJobSequence::new();
            let (job_id, admitted) = jobs.issue().expect("original customer admission");
            let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
                CustomerJob {
                    id: job_id,
                    admitted,
                    payload: 7,
                    customer: 9,
                },
                correlation,
            );
            let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
                proxy_creation,
                WorkerSubmission::immediate(Worker(7)),
            );
            let proxy_allocation = Arc::downgrade(&proxy_witness.token);
            let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                Installed::new(Endpoint(31)),
            );
            let original_proxy_recipient = proxy.recipient();

            let admission = match expected {
                BorrowedNormalAdmission::Accepted => AssignmentAdmission::Accept,
                BorrowedNormalAdmission::Rejected => AssignmentAdmission::Reject,
                BorrowedNormalAdmission::Corrupt => AssignmentAdmission::Corrupt,
            };
            let mut host = ProxyControlHost {
                admissions: VecDeque::from([ControlAdmission::Accept(proxy)]),
                observed: Vec::new(),
                assignment_admissions: VecDeque::from([admission]),
                attempts: Vec::new(),
            };
            let ItemSettlement::Accepted(prior) = ({
                let mut progress = Some(InterpretationProgress::Original(proxy_request));
                ProxyOperation::settle(&mut progress, &mut host);
                let Some(InterpretationProgress::Completed(settlement)) = progress else {
                    panic!("the exact host returns its complete original settlement");
                };
                settlement.into_settlement()
            }) else {
                panic!("actual previous proxy accepted receipt");
            };
            let (target, assignment, receipt) = assignment_request.into_parts();
            let mut delivery = Some(EstablishedDelivery::new(target, assignment));
            let lower = host
                .interpret_assignment_custody(&mut delivery, NativeBoundary::ApplicationConsumption)
                .await;
            // The lower future is fully disposed before mandatory receipt transfer.
            let current: ItemSettlement<
                AssignWorker<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>, Box<str>>,
                AssignmentReceipt,
                ExactDeliveryReason,
                Never,
            > = match lower {
                ItemSettlement::Accepted(()) => ItemSettlement::Accepted(receipt),
                ItemSettlement::Rejected {
                    item: EstablishedDelivery { to, message },
                    reason,
                } => ItemSettlement::Rejected {
                    item: AssignWorker::returned(to, message, receipt),
                    reason,
                },
                ItemSettlement::Corrupt {
                    item: EstablishedDelivery { to, message },
                    fault,
                } => ItemSettlement::Corrupt {
                    item: AssignWorker::returned(to, message, receipt),
                    fault,
                },
                ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
            };
            let prior = proxy_witness
                .admit_receipt(prior)
                .unwrap_or_else(|_| panic!("whole prior original proxy receipt"));
            let (returned_creation, returned_proxy, operation) = prior.into_parts();
            let returned_recipient = returned_proxy.recipient();
            drop(returned_proxy);
            drop(operation);
            let ProxyControlHost {
                admissions,
                observed,
                assignment_admissions,
                attempts,
            } = host;
            match (expected, current) {
                (BorrowedNormalAdmission::Accepted, ItemSettlement::Accepted(receipt)) => {
                    let receipt_match = assigned.compare_receipt(&receipt);
                    drop(receipt);
                    assert!(matches!(receipt_match, CorrelationMatch::Exact));
                }
                (BorrowedNormalAdmission::Rejected, ItemSettlement::Rejected { item, reason }) => {
                    let (target, assignment, receipt) = item.into_parts();
                    let receipt_match = assigned.compare_receipt(&receipt);
                    assert!(matches!(receipt_match, CorrelationMatch::Exact));
                    assert_eq!(reason, ExactDeliveryReason::ClosedRecipient);
                    assert_eq!(target, assignment_target);
                    assert_eq!(assignment.payload().as_ptr() as usize, original_payload);
                    assert_eq!(assignment.payload().as_ref(), "original assignment");
                    drop((target, assignment, receipt));
                }
                (BorrowedNormalAdmission::Corrupt, ItemSettlement::Corrupt { item, fault }) => {
                    let (target, assignment, receipt) = item.into_parts();
                    let receipt_match = assigned.compare_receipt(&receipt);
                    assert!(matches!(receipt_match, CorrelationMatch::Exact));
                    assert_eq!(fault, InterpreterFault::CorruptTraversal);
                    assert_eq!(target, assignment_target);
                    assert_eq!(assignment.payload().as_ptr() as usize, original_payload);
                    assert_eq!(assignment.payload().as_ref(), "original assignment");
                    drop((target, assignment, receipt));
                }
                _ => panic!(
                    "complete current assignment product disagrees with actual lower normal admission"
                ),
            }
            assert!(delivery.is_none());
            assert_eq!(returned_creation, proxy_creation);
            assert_eq!(returned_recipient, original_proxy_recipient);
            assert_eq!(
                attempts,
                [
                    CapabilityAttempt::Proxy(proxy_creation, 7),
                    CapabilityAttempt::Assignment(original_payload)
                ]
            );
            assert_eq!(observed, [(proxy_creation.get(), 7)]);
            assert!(admissions.is_empty());
            assert!(assignment_admissions.is_empty());
            drop(assigned);
            drop(worker_attempt);
            let discharged = proxy_allocation.strong_count();
            assert_eq!(discharged, 0);
        }
    }

    #[tokio::test]
    async fn unrelated_templates_assignment_then_proxy_borrowed_normal_products() {
        for expected in [
            BorrowedNormalAdmission::Accepted,
            BorrowedNormalAdmission::Rejected,
            BorrowedNormalAdmission::Corrupt,
        ] {
            let mut creations = CreationSequence::new();
            let proxy_creation = creations.issue().expect("original proxy creation");
            let worker_creation = creations.issue().expect("original worker attempt");
            let worker_attempt = WorkerAttempt::issued(worker_creation);
            let mut assignments = AssignmentSequence::new();
            let (correlation, assignment) = assignments
                .assign(&worker_attempt, Box::<str>::from("original assignment"))
                .expect("original assignment correlation");
            let original_payload = assignment.payload().as_ptr() as usize;
            let assignment_target = EstablishedRecipient::issued(Endpoint(41));
            let assignment_request = AssignWorker::<
                MessageProtocol<RuntimeAddr, Assignment<Box<str>>>,
                _,
            >::new(
                assignment_target.clone(), &correlation, assignment
            );
            let mut jobs = AcceptedJobSequence::new();
            let (job_id, admitted) = jobs.issue().expect("original customer admission");
            let assigned: AssignedJob<u8, u8, u8, RuntimeAddr> = AssignedJob::new(
                CustomerJob {
                    id: job_id,
                    admitted,
                    payload: 7,
                    customer: 9,
                },
                correlation,
            );
            let (proxy_witness, proxy_request) = ProxyOperation::<Owner, _, _>::initial(
                proxy_creation,
                WorkerSubmission::immediate(Worker(7)),
            );
            let proxy_allocation = Arc::downgrade(&proxy_witness.token);
            let proxy = EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                Installed::new(Endpoint(31)),
            );
            let original_proxy_recipient = proxy.recipient();

            let admission = match expected {
                BorrowedNormalAdmission::Accepted => ControlAdmission::Accept(proxy),
                BorrowedNormalAdmission::Rejected => {
                    drop(proxy);
                    ControlAdmission::Reject
                }
                BorrowedNormalAdmission::Corrupt => {
                    drop(proxy);
                    ControlAdmission::Corrupt
                }
            };
            let mut host = ProxyControlHost {
                admissions: VecDeque::from([admission]),
                observed: Vec::new(),
                assignment_admissions: VecDeque::from([AssignmentAdmission::Accept]),
                attempts: Vec::new(),
            };
            let mut assignment_progress =
                Some(InterpretationProgress::Original(assignment_request));
            AssignWorker::<MessageProtocol<RuntimeAddr, Assignment<Box<str>>>, Box<str>>::settle::<
                _,
                (),
                Here,
            >(&mut assignment_progress, &mut host)
            .await;
            let Some(InterpretationProgress::Completed(Interpretation::Complete(
                ItemSettlement::Accepted(prior),
            ))) = assignment_progress
            else {
                panic!("actual previous assignment accepted receipt");
            };
            let (creation, control, operation) = proxy_request.into_parts();
            let mut control = Some(control);
            let lower = host.admit_proxy_custody(
                creation,
                &mut control,
                NativeBoundary::ApplicationConsumption,
            );
            // The lower call is over before original private authority transfer.
            let current: ItemSettlement<
                ProxyOperation<Owner, Worker, ImmediateActivation>,
                ProxyInputReceipt<Worker, ImmediateActivation>,
                ChildInputReason,
                Never,
            > = match lower {
                ItemSettlement::Accepted(proxy) => {
                    ItemSettlement::Accepted(ProxyInputReceipt::new(creation, proxy, operation))
                }
                ItemSettlement::Rejected {
                    item: control,
                    reason,
                } => ItemSettlement::Rejected {
                    item: ProxyOperation {
                        creation,
                        control,
                        operation,
                        source: PhantomData,
                    },
                    reason,
                },
                ItemSettlement::Corrupt {
                    item: control,
                    fault,
                } => ItemSettlement::Corrupt {
                    item: ProxyOperation {
                        creation,
                        control,
                        operation,
                        source: PhantomData,
                    },
                    fault,
                },
                ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
            };
            let prior_match = assigned.compare_receipt(&prior);
            drop(prior);
            let admitted = proxy_witness
                .admit(SettledItem::Attempted(current))
                .unwrap_or_else(|_| panic!("whole original current proxy product"));
            let ProxyControlHost {
                admissions,
                observed,
                assignment_admissions,
                attempts,
            } = host;
            assert!(matches!(prior_match, CorrelationMatch::Exact));
            match (expected, admitted) {
                (
                    BorrowedNormalAdmission::Accepted,
                    SettledItem::Attempted(ItemSettlement::Accepted(receipt)),
                ) => {
                    let (returned_creation, actor, operation) = receipt.into_parts();
                    let returned_recipient = actor.recipient();
                    drop((actor, operation));
                    assert_eq!(returned_creation, proxy_creation);
                    assert_eq!(returned_recipient, original_proxy_recipient);
                }
                (
                    BorrowedNormalAdmission::Rejected,
                    SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
                ) => {
                    let (returned_creation, original_control, operation) = item.into_parts();
                    let ProxyCommand::Start(submission) = original_control.command else {
                        panic!("original proxy input is Start");
                    };
                    assert_eq!(returned_creation, proxy_creation);
                    assert_eq!(submission.worker.0, 7);
                    assert_eq!(reason, ChildInputReason::ClosedControlLane);
                    drop((submission, operation));
                }
                (
                    BorrowedNormalAdmission::Corrupt,
                    SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }),
                ) => {
                    let (returned_creation, original_control, operation) = item.into_parts();
                    let ProxyCommand::Start(submission) = original_control.command else {
                        panic!("original proxy input is Start");
                    };
                    assert_eq!(returned_creation, proxy_creation);
                    assert_eq!(submission.worker.0, 7);
                    assert_eq!(fault, InterpreterFault::CorruptTraversal);
                    drop((submission, operation));
                }
                _ => panic!(
                    "complete current proxy product disagrees with actual lower normal admission"
                ),
            }
            assert!(control.is_none());
            assert_eq!(
                attempts,
                [
                    CapabilityAttempt::Assignment(original_payload),
                    CapabilityAttempt::Proxy(proxy_creation, 7)
                ]
            );
            assert_eq!(observed, [(proxy_creation.get(), 7)]);
            assert!(admissions.is_empty());
            assert!(assignment_admissions.is_empty());
            drop(assigned);
            drop(worker_attempt);
            let discharged = proxy_allocation.strong_count();
            assert_eq!(discharged, 0);
        }
    }
    struct ColdWorker(Rc<Vec<u64>>);

    impl Protocol for ColdWorker {
        type Addr = RuntimeAddr;
        type Msg = Never;
    }
    impl Behavior for ColdWorker {
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

    struct ColdControlRejection;
    impl ProxyControlAdmission<ColdWorker, ImmediateActivation> for ColdControlRejection {
        fn admit_proxy_control(
            &mut self,
            _: CreationId,
            control: ProxyControl<ColdWorker, ImmediateActivation>,
        ) -> ItemSettlement<
            ProxyControl<ColdWorker, ImmediateActivation>,
            EstablishedActor<StableProxy<ColdWorker, ImmediateActivation>>,
            ChildInputReason,
            Never,
        > {
            ItemSettlement::Rejected {
                item: control,
                reason: ChildInputReason::ClosedControlLane,
            }
        }
    }

    #[test]
    fn synchronous_proxy_admission_preserves_cold_non_send_worker_state() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("cold original creation is issued");
        let state = Rc::new(vec![31, 37]);
        let original = Rc::as_ptr(&state);
        let (witness, operation) = ProxyOperation::<Owner, _, _>::initial(
            creation,
            WorkerSubmission::immediate(ColdWorker(state)),
        );
        let mut progress = Some(InterpretationProgress::Original(operation));
        ProxyOperation::settle(&mut progress, &mut ColdControlRejection);
        let Some(InterpretationProgress::Completed(Interpretation::Complete(
            ItemSettlement::Rejected { item, reason },
        ))) = progress
        else {
            panic!("cold control rejection must return the whole original operation");
        };
        let (item, reason) = witness
            .admit_rejection(item, reason)
            .unwrap_or_else(|_| panic!("the same operation authority remains paired"));
        let (returned_creation, control, authority) = item.into_parts();
        let ProxyCommand::Start(submission) = control.command else {
            panic!("the original cold input is a start control");
        };
        assert_eq!(returned_creation, creation);
        assert_eq!(reason, ChildInputReason::ClosedControlLane);
        assert_eq!(Rc::as_ptr(&submission.worker.0), original);
        assert_eq!(submission.worker.0.as_slice(), &[31, 37]);
        drop(authority);
    }
}
