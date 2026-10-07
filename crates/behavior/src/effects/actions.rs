//! The explicit result of one actor behavior transition.

use core::future::{self, Future};
use core::pin::pin;
use core::task::Poll;

use super::sending::{
    ClassifySettlement, InterpretItem, InterpretSends, Interpretation, InterpretationProgress,
    InterpreterFault, ItemSettlement, SendEffects, SendInput, SendSettlements, SettledItem,
    SettlementStatus, SourceAdmission, SourceCustody, SourceProgress, SourceSettlementCustody,
    offer_source_in_order,
};
use crate::actor::{
    Address, BirthMode, Births, ChildCreationProduct, ChildHead, ChildNamespaceExhausted,
    CreateChild, CreationRejection, Creations, DispatchBirth, NoBirths, RetirementBirths,
    RoutedCreation,
};
use crate::next::{Never, Step, Stopped};
use crate::transition::{Behavior, BehaviorAddr};
use crate::user_event::{EventIngress, User, UserEvent};

pub type Become<Ph = Never> = Step<Ph, Stopped>;

/// Complete owned settlement of one [`Actions`] value.
///
/// Creations retain their declared vector order, sends retain their named
/// product shape, and `become_` is the exact verdict already committed by the
/// pure behavior fold. The enclosing [`Interpretation`] records whether the
/// interpreter completed or corrupted while retaining this entire shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSettlement<Creations, Sends, Ph> {
    pub creations: Creations,
    pub sends: Sends,
    pub become_: Become<Ph>,
}

/// Final settlement of the creation leg of one [`Actions`] value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreationSettlement<Requests, Settlements> {
    /// The whole batch was routed and every child has an exact settlement.
    Settled(Settlements),
    /// Route preparation rejected the complete unchanged batch.
    Rejected {
        creations: Requests,
        reason: ChildNamespaceExhausted,
    },
    /// Route preparation corrupted before transferring any request.
    Corrupt {
        creations: Requests,
        fault: InterpreterFault,
    },
}

/// Exact creation settlement deliberately retained for actor retirement.
///
/// This product distinguishes terminal custody from a settlement that must
/// return as another input to a live creator. It never erases or reconstructs
/// the enclosed accepted, rejected, corrupt, or unattempted values.
#[must_use = "a retirement creation settlement must remain in terminal custody"]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetirementCreationSettlement<Settlement> {
    settlement: Settlement,
}

impl<Settlement> RetirementCreationSettlement<Settlement> {
    #[must_use]
    pub const fn new(settlement: Settlement) -> Self {
        Self { settlement }
    }

    #[must_use]
    pub fn into_settlement(self) -> Settlement {
        self.settlement
    }
}

/// Static complete-settlement product selected by one concrete action type.
///
/// This projection performs no interpretation and names no runtime. It lets a
/// lifecycle host retain the exact result type of an action without rebuilding
/// its creation or send products.
pub trait ActionSettlements: Sized {
    type Settlements;
    type SourceCustody;
    type InterpretationCustody;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    );
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    );
}

impl<A, Ph, Sends, Birth> ActionSettlements for Actions<A, Ph, Sends, Birth>
where
    A: Address,
    Sends: SendSettlements,
    Birth: CreationSettlements<A>,
{
    type Settlements = ActionSettlement<Birth::Settlements, Sends::Settlements, Ph>;
    type SourceCustody = (
        (
            Option<SourceProgress<Birth::Settlements, Birth::SourceCustody>>,
            Option<SourceProgress<Sends::Settlements, Sends::SourceCustody>>,
        ),
        Become<Ph>,
    );
    type InterpretationCustody = (
        (
            Option<
                InterpretationProgress<
                    Creations<CreateChild<A, Birth::Child>>,
                    Birth::InterpretationCustody,
                    Birth::Settlements,
                >,
            >,
            Option<InterpretationProgress<Sends, Sends::InterpretationCustody, Sends::Settlements>>,
        ),
        Become<Ph>,
    );

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if matches!(progress, Some(InterpretationProgress::Original(_))) {
            match progress.take() {
                Some(InterpretationProgress::Original(Actions {
                    creates,
                    sends,
                    become_,
                })) => {
                    *progress = Some(InterpretationProgress::Interpreting((
                        (
                            Some(InterpretationProgress::Original(creates)),
                            Some(InterpretationProgress::Original(sends)),
                        ),
                        become_,
                    )));
                }
                retained => *progress = retained,
            }
        }
    }

    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if !matches!(
            progress,
            Some(InterpretationProgress::Interpreting((
                (
                    Some(InterpretationProgress::Completed(_)),
                    Some(InterpretationProgress::Completed(_))
                ),
                _
            )))
        ) {
            return;
        }
        // Only structural extraction after every producer was disposed and
        // every child finalizer borrowed its actual outside slot separately.
        match progress.take() {
            Some(InterpretationProgress::Interpreting((
                (
                    Some(InterpretationProgress::Completed(creations)),
                    Some(InterpretationProgress::Completed(sends)),
                ),
                become_,
            ))) => {
                let completed = match (creations, sends) {
                    (Interpretation::Complete(creations), Interpretation::Complete(sends)) => {
                        Interpretation::Complete(ActionSettlement {
                            creations,
                            sends,
                            become_,
                        })
                    }
                    (
                        Interpretation::Complete(creations) | Interpretation::Corrupt(creations),
                        Interpretation::Corrupt(sends),
                    )
                    | (Interpretation::Corrupt(creations), Interpretation::Complete(sends)) => {
                        Interpretation::Corrupt(ActionSettlement {
                            creations,
                            sends,
                            become_,
                        })
                    }
                };
                *progress = Some(InterpretationProgress::Completed(completed));
            }
            retained => *progress = retained,
        }
    }
}

/// One exact action-settlement product selected by a concrete behavior.
///
/// Runtime lifecycle types use this blanket projection instead of repeating
/// the behavior's internal send- and birth-product bounds through every parent
/// and application owner. The associated value remains fully concrete and is
/// never erased or reclassified.
pub trait BehaviorSettlements: Behavior {
    type Settlements;
    type SourceCustody;
    type InterpretationCustody;
}

impl<B> BehaviorSettlements for B
where
    B: Behavior,
    B::Sends: SendSettlements,
    B::Birth: CreationSettlements<BehaviorAddr<B>>,
{
    type Settlements =
        <Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth> as ActionSettlements>::Settlements;
    type SourceCustody =
        <Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth> as ActionSettlements>::SourceCustody;
    type InterpretationCustody = <Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth> as ActionSettlements>::InterpretationCustody;
}

impl<Requests, Settlements> ClassifySettlement for CreationSettlement<Requests, Settlements>
where
    Settlements: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        match self {
            Self::Settled(settlements) => settlements.settlement_status(),
            Self::Rejected { .. } => SettlementStatus::Rejected,
            Self::Corrupt { .. } => SettlementStatus::Corrupt,
        }
    }
}

impl<Settlement> ClassifySettlement for RetirementCreationSettlement<Settlement>
where
    Settlement: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        self.settlement.settlement_status()
    }
}

impl<Item> ClassifySettlement for Creations<Item>
where
    Item: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        let mut status = SettlementStatus::Accepted;
        for item in self.iter() {
            status = status.combine(item.settlement_status());
        }
        status
    }
}

/// Static creation-settlement product selected by one birth mode.
pub trait CreationSettlements<A: Address>: BirthMode {
    type Settlements: ClassifySettlement;
    type SourceCustody;
    type InterpretationCustody;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Self::Child>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    );
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Self::Child>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    );
}

impl<A: Address> CreationSettlements<A> for NoBirths {
    type Settlements = Creations<Never>;
    type SourceCustody = Self::Settlements;
    type InterpretationCustody = ();
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Never>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                let settlements = original
                    .into_iter()
                    .map(|creation| {
                        let (_, never, _) = creation.into_parts();
                        match never {}
                    })
                    .collect();
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    Creations::from_items(settlements),
                )));
            }
            retained => *progress = retained,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Never>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        Self::prepare_interpretation(progress);
    }
}

impl<A, C> CreationSettlements<A> for Births<C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    type Settlements = CreationSettlement<
        Creations<CreateChild<A, C>>,
        Creations<
            SettledItem<
                RoutedCreation<A, C>,
                ItemSettlement<
                    RoutedCreation<A, C>,
                    <C as ChildCreationProduct<A, ChildHead>>::Result,
                    CreationRejection,
                    Never,
                >,
            >,
        >,
    >;
    type SourceCustody = (
        Option<CreationsSettled<A, C>>,
        Option<Result<(), CreationsSettled<A, C>>>,
    );
    type InterpretationCustody = CreationInterpretationCustody<A, C>;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        prepare_creations(progress);
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        finish_creations(progress);
    }
}

impl<A, C> CreationSettlements<A> for RetirementBirths<C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    type Settlements =
        RetirementCreationSettlement<<Births<C> as CreationSettlements<A>>::Settlements>;
    type SourceCustody = Self::Settlements;
    type InterpretationCustody = Option<
        InterpretationProgress<
            Creations<CreateChild<A, C>>,
            <Births<C> as CreationSettlements<A>>::InterpretationCustody,
            <Births<C> as CreationSettlements<A>>::Settlements,
        >,
    >;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                *progress = Some(InterpretationProgress::Interpreting(Some(
                    InterpretationProgress::Original(original),
                )))
            }
            retained => *progress = retained,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
    ) {
        // Base finalization has already loaned the outside base progress.
        // This extraction calls no generic/custom method.
        match progress.take() {
            Some(InterpretationProgress::Interpreting(Some(
                InterpretationProgress::Completed(received),
            ))) => {
                *progress = Some(InterpretationProgress::Completed(
                    received.map(RetirementCreationSettlement::new),
                ));
            }
            retained => *progress = retained,
        }
    }
}

/// One complete creation batch returned to its live creator.
///
/// The value retains either every routed child settlement or the entire
/// unchanged batch rejected during route preparation. It is interpreter-facing
/// custody, not an application protocol or another creation operation.
#[must_use = "a returned creation batch must be admitted or retained"]
pub struct CreationsSettled<A, C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    settlement: <Births<C> as CreationSettlements<A>>::Settlements,
}

/// Complete event algebra for an ordinary creator that receives its exact
/// creation settlements as a later behavior input.
///
/// The creation source is selected semantically by [`Births<C>`]. Outer
/// behavior layers lift that ingress through their existing event composition;
/// no structural path is exposed to the actor author.
pub enum CreationEvent<A, C, M>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    Settlements(CreationsSettled<A, C>),
    User(User<A, M>),
}

impl<A, C, M> EventIngress<Births<C>, CreationsSettled<A, C>> for CreationEvent<A, C, M>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    fn ingress(input: CreationsSettled<A, C>) -> Self {
        Self::Settlements(input)
    }
}

impl<A, C, M> UserEvent for CreationEvent<A, C, M>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    type Addr = A;
    type Message = M;

    fn user(from: A, message: M) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<A, M>, Self> {
        match self {
            Self::User(event) => Ok(event),
            settlements @ Self::Settlements(_) => Err(settlements),
        }
    }
}

impl<A, C> CreationsSettled<A, C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    #[must_use]
    pub const fn new(settlement: <Births<C> as CreationSettlements<A>>::Settlements) -> Self {
        Self { settlement }
    }

    #[must_use]
    pub fn into_settlement(self) -> <Births<C> as CreationSettlements<A>>::Settlements {
        self.settlement
    }
}

impl<Host, RootEvent> SourceSettlementCustody<Host, RootEvent> for Creations<Never> {
    type Custody = Self;
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Completed(
                SourceCustody::Exhausted(original),
            )),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        future::ready(())
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original) | SourceProgress::Offering(original)) => Some(
                SourceProgress::Completed(SourceCustody::Exhausted(original)),
            ),
            retained => retained,
        };
    }
}

impl<A, C, Host, RootEvent> SourceSettlementCustody<Host, RootEvent>
    for CreationSettlement<
        Creations<CreateChild<A, C>>,
        Creations<
            SettledItem<
                RoutedCreation<A, C>,
                ItemSettlement<
                    RoutedCreation<A, C>,
                    <C as ChildCreationProduct<A, ChildHead>>::Result,
                    CreationRejection,
                    Never,
                >,
            >,
        >,
    >
where
    A: Address,
    A::Nonce: Send,
    C: ChildCreationProduct<A, ChildHead> + Send,
    <C as ChildCreationProduct<A, ChildHead>>::Result: Send,
    Host: SourceAdmission<RootEvent, Births<C>, CreationsSettled<A, C>>,
    RootEvent: crate::EventIngress<Births<C>, CreationsSettled<A, C>>,
{
    type Custody = (
        Option<CreationsSettled<A, C>>,
        Option<Result<(), CreationsSettled<A, C>>>,
    );
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) if matches!(&original, CreationSettlement::Settled(settlements) if settlements.is_empty()) => {
                Some(SourceProgress::Completed(SourceCustody::Exhausted(
                    original,
                )))
            }
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Offering((
                Some(CreationsSettled::new(original)),
                None,
            ))),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if custody.1.is_none() {
                host.admit_source(&mut custody.0, &mut custody.1).await;
            }
        }
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Offering((None, Some(Ok(()))))) => {
                Some(SourceProgress::Completed(SourceCustody::Admitted(
                    CreationSettlement::Settled(Creations::empty()),
                )))
            }
            Some(SourceProgress::Offering((None, Some(Err(returned))))) => Some(
                SourceProgress::Completed(SourceCustody::Closed(returned.into_settlement())),
            ),
            progress => progress,
        };
    }
}

impl<A, C, Host, RootEvent> SourceSettlementCustody<Host, RootEvent>
    for RetirementCreationSettlement<
        CreationSettlement<
            Creations<CreateChild<A, C>>,
            Creations<
                SettledItem<
                    RoutedCreation<A, C>,
                    ItemSettlement<
                        RoutedCreation<A, C>,
                        <C as ChildCreationProduct<A, ChildHead>>::Result,
                        CreationRejection,
                        Never,
                    >,
                >,
            >,
        >,
    >
where
    A: Address,
    A::Nonce: Send,
    C: ChildCreationProduct<A, ChildHead> + Send,
    <C as ChildCreationProduct<A, ChildHead>>::Result: Send,
{
    type Custody = Self;
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) if matches!(&original.settlement, CreationSettlement::Settled(settlements) if settlements.is_empty()) => {
                Some(SourceProgress::Completed(SourceCustody::Exhausted(
                    original,
                )))
            }
            Some(SourceProgress::Original(original)) => {
                Some(SourceProgress::Completed(SourceCustody::Retained(original)))
            }
            progress => progress,
        };
    }
    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        future::ready(())
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original) | SourceProgress::Offering(original)) if matches!(&original.settlement, CreationSettlement::Settled(settlements) if settlements.is_empty()) => {
                Some(SourceProgress::Completed(SourceCustody::Exhausted(
                    original,
                )))
            }
            Some(SourceProgress::Original(original) | SourceProgress::Offering(original)) => {
                Some(SourceProgress::Completed(SourceCustody::Retained(original)))
            }
            retained => retained,
        };
    }
}

/// Outside custody of the actual routing reply or ordered child attempts.
/// Native causes remain runtime retirement facts, not this normal typed product.
pub enum CreationInterpretationCustody<A, C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    Routing {
        input: Option<Creations<CreateChild<A, C>>>,
        received: Option<
            ItemSettlement<
                Creations<CreateChild<A, C>>,
                Creations<RoutedCreation<A, C>>,
                ChildNamespaceExhausted,
                Never,
            >,
        >,
    },
    Children(
        Vec<(
            Option<RoutedCreation<A, C>>,
            Option<
                ItemSettlement<
                    RoutedCreation<A, C>,
                    <C as ChildCreationProduct<A, ChildHead>>::Result,
                    CreationRejection,
                    Never,
                >,
            >,
        )>,
    ),
}

fn prepare_creations<A, C>(
    progress: &mut Option<
        InterpretationProgress<
            Creations<CreateChild<A, C>>,
            CreationInterpretationCustody<A, C>,
            CreationSettlement<
                Creations<CreateChild<A, C>>,
                Creations<
                    SettledItem<
                        RoutedCreation<A, C>,
                        ItemSettlement<
                            RoutedCreation<A, C>,
                            <C as ChildCreationProduct<A, ChildHead>>::Result,
                            CreationRejection,
                            Never,
                        >,
                    >,
                >,
            >,
        >,
    >,
) where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    if matches!(progress, Some(InterpretationProgress::Original(_))) {
        match progress.take() {
            Some(InterpretationProgress::Original(input)) => {
                *progress = Some(InterpretationProgress::Interpreting(
                    CreationInterpretationCustody::Routing {
                        input: Some(input),
                        received: None,
                    },
                ));
            }
            retained => *progress = retained,
        }
    }
}

fn finish_routing<A, C>(
    progress: &mut Option<
        InterpretationProgress<
            Creations<CreateChild<A, C>>,
            CreationInterpretationCustody<A, C>,
            <Births<C> as CreationSettlements<A>>::Settlements,
        >,
    >,
) where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    if !matches!(
        progress,
        Some(InterpretationProgress::Interpreting(
            CreationInterpretationCustody::Routing {
                input: None,
                received: Some(_)
            }
        ))
    ) {
        return;
    }
    match progress.take() {
        Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Routing {
            input: None,
            received: Some(received),
        })) => {
            *progress = Some(match received {
                ItemSettlement::Accepted(routed) => {
                    InterpretationProgress::Interpreting(CreationInterpretationCustody::Children(
                        routed
                            .into_iter()
                            .map(|input| (Some(input), None))
                            .collect(),
                    ))
                }
                ItemSettlement::Rejected { item, reason } => InterpretationProgress::Completed(
                    Interpretation::Complete(CreationSettlement::Rejected {
                        creations: item,
                        reason,
                    }),
                ),
                ItemSettlement::Corrupt { item, fault } => InterpretationProgress::Completed(
                    Interpretation::Corrupt(CreationSettlement::Corrupt {
                        creations: item,
                        fault,
                    }),
                ),
                ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
            });
        }
        retained => *progress = retained,
    }
}

async fn settle_child<A, C, Interpreter>(
    input: &mut Option<RoutedCreation<A, C>>,
    received: &mut Option<
        ItemSettlement<
            RoutedCreation<A, C>,
            <C as ChildCreationProduct<A, ChildHead>>::Result,
            CreationRejection,
            Never,
        >,
    >,
    interpreter: &mut Interpreter,
) where
    A: Address,
    C: ChildCreationProduct<A, ChildHead> + DispatchBirth<A, Interpreter>,
{
    if received.is_some() {
        return;
    }
    let Some(original) = input.take() else {
        return;
    };
    let (creation, route) = original.into_parts();
    let (id, child, kind) = creation.into_parts();
    let mut attempt = pin!(child.dispatch_birth(id, route, kind, interpreter));
    future::poll_fn(|context| match attempt.as_mut().poll(context) {
        Poll::Pending => Poll::Pending,
        Poll::Ready(reply) => {
            *received = Some(reply);
            Poll::Ready(())
        }
    })
    .await;
}

async fn interpret_creations<A, C, Interpreter, RootEvent, Path>(
    progress: &mut Option<
        InterpretationProgress<
            Creations<CreateChild<A, C>>,
            CreationInterpretationCustody<A, C>,
            <Births<C> as CreationSettlements<A>>::Settlements,
        >,
    >,
    interpreter: &mut Interpreter,
) where
    A: Address,
    A::Nonce: Send,
    C: ChildCreationProduct<A, ChildHead> + DispatchBirth<A, Interpreter> + Send,
    <C as ChildCreationProduct<A, ChildHead>>::Result: Send,
    Interpreter: InterpretItem<Creations<CreateChild<A, C>>, RootEvent, Path> + Send,
{
    prepare_creations(progress);
    if let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Routing {
        input,
        received,
    })) = progress
    {
        <Interpreter as InterpretItem<Creations<CreateChild<A,C>>, RootEvent, Path>>::interpret_item(
            interpreter, input, received,
        ).await;
    }
    finish_routing(progress);
    let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Children(rows))) =
        progress
    else {
        return;
    };
    for (input, received) in rows {
        match (input.as_ref(), received.as_ref()) {
            (None, Some(ItemSettlement::Corrupt { .. })) => break,
            (None, Some(_)) => continue,
            (Some(_), None) => {}
            _ => return,
        }
        settle_child(input, received, interpreter).await;
        match (input.as_ref(), received.as_ref()) {
            (None, Some(ItemSettlement::Corrupt { .. })) => break,
            (None, Some(_)) => {}
            _ => return,
        }
    }
    finish_creations(progress);
}

fn finish_creations<A, C>(
    progress: &mut Option<
        InterpretationProgress<
            Creations<CreateChild<A, C>>,
            CreationInterpretationCustody<A, C>,
            <Births<C> as CreationSettlements<A>>::Settlements,
        >,
    >,
) where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    finish_routing(progress);
    let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Children(rows))) =
        progress.as_ref()
    else {
        return;
    };
    let corrupt = rows
        .iter()
        .position(|(_, received)| matches!(received, Some(ItemSettlement::Corrupt { .. })));
    let complete = rows.iter().enumerate().all(|(index, (input, received))| {
        if corrupt.is_some_and(|corrupt| index > corrupt) {
            input.is_some() && received.is_none()
        } else {
            input.is_none() && received.is_some()
        }
    });
    if !complete {
        return;
    }
    let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Children(rows))) =
        progress.take()
    else {
        return;
    };
    let mut remaining = rows.into_iter();
    let mut settled = Vec::with_capacity(remaining.len());
    while let Some((input, received)) = remaining.next() {
        match (input, received) {
            (None, Some(received)) => settled.push(SettledItem::Attempted(received)),
            (Some(input), None) => settled.push(SettledItem::Unattempted(input)),
            (input, received) => {
                let rows = settled
                    .into_iter()
                    .map(|settled| match settled {
                        SettledItem::Attempted(received) => (None, Some(received)),
                        SettledItem::Unattempted(input) => (Some(input), None),
                    })
                    .chain(core::iter::once((input, received)))
                    .chain(remaining)
                    .collect();
                *progress = Some(InterpretationProgress::Interpreting(
                    CreationInterpretationCustody::Children(rows),
                ));
                return;
            }
        }
    }
    let settlement = CreationSettlement::Settled(Creations::from_items(settled));
    *progress = Some(InterpretationProgress::Completed(match corrupt {
        Some(_) => Interpretation::Corrupt(settlement),
        None => Interpretation::Complete(settlement),
    }));
}

/// Generic interpretation of the one creation leg selected by a birth mode.
///
/// `NoBirths` needs no runtime capability. `Births<C>` performs one real batch
/// route attempt followed by independent child establishment in declared order.
pub trait InterpretCreations<A, Interpreter, RootEvent, Path>: CreationSettlements<A>
where
    A: Address,
{
    fn interpret_creations(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Self::Child>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send;
}

impl<A, Interpreter, RootEvent, Path> InterpretCreations<A, Interpreter, RootEvent, Path>
    for NoBirths
where
    A: Address,
    Creations<CreateChild<A, Never>>: Send,
{
    fn interpret_creations(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, Never>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
        _: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            <Self as CreationSettlements<A>>::prepare_interpretation(progress);
        }
    }
}

impl<A, C, Interpreter, RootEvent, Path> InterpretCreations<A, Interpreter, RootEvent, Path>
    for Births<C>
where
    A: Address,
    A::Nonce: Send,
    C: ChildCreationProduct<A, ChildHead> + DispatchBirth<A, Interpreter> + Send,
    <C as ChildCreationProduct<A, ChildHead>>::Result: Send,
    Interpreter: InterpretItem<Creations<CreateChild<A, C>>, RootEvent, Path> + Send,
{
    fn interpret_creations(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        interpret_creations::<A, C, Interpreter, RootEvent, Path>(progress, interpreter)
    }
}

impl<A, C, Interpreter, RootEvent, Path> InterpretCreations<A, Interpreter, RootEvent, Path>
    for RetirementBirths<C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
    Births<C>: BirthMode<Child = C> + InterpretCreations<A, Interpreter, RootEvent, Path>,
    Creations<CreateChild<A, C>>: Send,
    <Births<C> as CreationSettlements<A>>::InterpretationCustody: Send,
    <Births<C> as CreationSettlements<A>>::Settlements: Send,
    Interpreter: Send,
{
    fn interpret_creations(
        progress: &mut Option<
            InterpretationProgress<
                Creations<CreateChild<A, C>>,
                Self::InterpretationCustody,
                Self::Settlements,
            >,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            <Self as CreationSettlements<A>>::prepare_interpretation(progress);
            let Some(InterpretationProgress::Interpreting(base)) = progress else {
                return;
            };
            <Births<C> as InterpretCreations<A,Interpreter,RootEvent,Path>>::interpret_creations(base,interpreter).await;
            <Births<C> as CreationSettlements<A>>::finish_interpretation(base);
            <Self as CreationSettlements<A>>::finish_interpretation(progress);
        }
    }
}

impl<Creations, Sends, Ph> ClassifySettlement for ActionSettlement<Creations, Sends, Ph>
where
    Creations: ClassifySettlement,
    Sends: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        self.creations
            .settlement_status()
            .combine(self.sends.settlement_status())
    }
}

impl<Host, RootEvent, Creations, Sends, Ph> SourceSettlementCustody<Host, RootEvent>
    for ActionSettlement<Creations, Sends, Ph>
where
    Host: Send,
    Creations: SourceSettlementCustody<Host, RootEvent> + Send,
    Sends: SourceSettlementCustody<Host, RootEvent> + Send,
    Ph: Send,
{
    type Custody = (
        <(Creations, Sends) as SourceSettlementCustody<Host, RootEvent>>::Custody,
        Become<Ph>,
    );
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Offering((
                (
                    Some(SourceProgress::Original(original.creations)),
                    Some(SourceProgress::Original(original.sends)),
                ),
                original.become_,
            ))),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        offer_source_in_order::<Host, RootEvent, Creations, Sends>(&mut custody.0, host)
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        let (custody, become_) = match progress.take() {
            Some(SourceProgress::Offering(custody)) => custody,
            retained => {
                *progress = retained;
                return;
            }
        };
        let mut product = Some(SourceProgress::Offering(custody));
        <(Creations, Sends) as SourceSettlementCustody<Host, RootEvent>>::finish_source(
            &mut product,
        );
        *progress = match product {
            Some(SourceProgress::Original((creations, sends))) => {
                Some(SourceProgress::Original(Self {
                    creations,
                    sends,
                    become_,
                }))
            }
            Some(SourceProgress::Offering(custody)) => {
                Some(SourceProgress::Offering((custody, become_)))
            }
            Some(SourceProgress::Completed(reply)) => Some(SourceProgress::Completed(reply.map(
                |(creations, sends)| Self {
                    creations,
                    sends,
                    become_,
                },
            ))),
            None => None,
        };
    }
}

/// Capability to append one communication at a statically selected lane while
/// preserving every other actor-transition effect.
pub trait AppendSend<Input, Path>: Sized {
    /// Append `input` exactly once and preserve creations and the next verdict.
    #[must_use]
    fn append_send(self, input: Input) -> Self;
}

/// Bombay's typed realization of the actor transition effects: communications,
/// fresh actor creation, and next behavior or termination.
///
/// [`Actions::interpret`] resolves every fresh creation in `creates` before the
/// named `sends` product. A committed resolution installs and binds the child;
/// a semantic rejection binds nothing but remains an accepted interpretation
/// receipt. This ordering lets a same-action [`crate::ChildDelivery`] or typed
/// creation-observation request use the authoritative result rather than
/// Behavior's intent. Each child-operation interpreter returns `Blocked` only
/// when its exact binding prerequisite rejected, while independent later items
/// are still attempted. Interpreter corruption retains the factual prefix and
/// every exact remaining value. No later creation may inherit a failed binding.
/// Creation order is vector order, and named send products declare their own
/// stable order. Constructing a value remains pure.
#[must_use = "actor transition effects must be interpreted, inspected, or retained"]
pub struct Actions<A: Address, Ph, Sends, Birth: BirthMode> {
    pub sends: Sends,
    pub creates: Creations<CreateChild<A, Birth::Child>>,
    pub become_: Become<Ph>,
}

impl<A, Ph, Sends, Birth> core::fmt::Debug for Actions<A, Ph, Sends, Birth>
where
    A: Address + core::fmt::Debug,
    A::Nonce: core::fmt::Debug,
    Ph: core::fmt::Debug,
    Sends: core::fmt::Debug,
    Birth: BirthMode,
    Birth::Child: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Actions")
            .field("sends", &self.sends)
            .field("creates", &self.creates)
            .field("become", &self.become_)
            .finish()
    }
}

impl<A, Ph, Sends, Birth> PartialEq for Actions<A, Ph, Sends, Birth>
where
    A: Address + PartialEq,
    A::Nonce: PartialEq,
    Ph: PartialEq,
    Sends: PartialEq,
    Birth: BirthMode,
    Birth::Child: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.sends == other.sends && self.creates == other.creates && self.become_ == other.become_
    }
}

impl<A, Ph, Sends, Birth> Eq for Actions<A, Ph, Sends, Birth>
where
    A: Address + Eq,
    A::Nonce: Eq,
    Ph: Eq,
    Sends: Eq,
    Birth: BirthMode,
    Birth::Child: Eq,
{
}

impl<A: Address, Ph, Sends, Birth: BirthMode> Actions<A, Ph, Sends, Birth> {
    /// Transform only the send effects, preserving creation order and the
    /// next-behavior verdict exactly.
    #[must_use]
    pub fn map_sends<Mapped>(
        self,
        map: impl FnOnce(Sends) -> Mapped,
    ) -> Actions<A, Ph, Mapped, Birth> {
        Actions {
            sends: map(self.sends),
            creates: self.creates,
            become_: self.become_,
        }
    }

    /// Transform only the next-behavior verdict, preserving sends and
    /// creation order exactly.
    #[must_use]
    pub fn map_become<NextPh>(
        self,
        map: impl FnOnce(Become<Ph>) -> Become<NextPh>,
    ) -> Actions<A, NextPh, Sends, Birth> {
        Actions {
            sends: self.sends,
            creates: self.creates,
            become_: map(self.become_),
        }
    }

    /// Append one communication to its statically selected send lane.
    ///
    /// This is a pure transformation of the send leg. It preserves creation
    /// order and the exact continue, phase-change, or termination verdict.
    /// `Path` is compile-time lane evidence; no runtime lookup is performed.
    #[must_use]
    pub fn with_send<Input, Path>(mut self, input: Input) -> Self
    where
        Sends: SendInput<Input, Path>,
    {
        <Sends as SendInput<Input, Path>>::emit(&mut self.sends, input);
        self
    }

    /// Borrow caller-owned action progress through one statically typed runtime.
    ///
    /// Creation items are attempted in vector order before the named send
    /// product. Lawful creation rejection or blocking is retained and does not
    /// suppress sends; concrete child-operation interpreters decide whether an
    /// exact binding prerequisite was satisfied. Interpreter corruption stops
    /// traversal and retains every later creation and the complete send product
    /// as unattempted. The next-behavior verdict is never reconstructed.
    pub async fn interpret<Interpreter, RootEvent, Path>(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                <Self as ActionSettlements>::InterpretationCustody,
                <Self as ActionSettlements>::Settlements,
            >,
        >,
        interpreter: &mut Interpreter,
    ) where
        Ph: Send,
        Sends: InterpretSends<Interpreter, RootEvent, Path>,
        Birth: InterpretCreations<A, Interpreter, RootEvent, Path>,
        Self: Send,
        <Self as ActionSettlements>::Settlements: Send,
        <Self as ActionSettlements>::InterpretationCustody: Send,
        Interpreter: Send,
    {
        Self::prepare_interpretation(progress);
        let Some(InterpretationProgress::Interpreting(((creates, sends), _))) = progress else {
            return;
        };
        Birth::interpret_creations(creates, interpreter).await;
        Birth::finish_interpretation(creates);
        match creates {
            Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {
                Sends::interpret(sends, interpreter).await;
                Sends::finish_interpretation(sends);
            }
            Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => {
                Sends::unattempted(sends);
            }
            // Neither source closure, normal exhaustion, interpreter fault,
            // nor a completed settlement can be inferred from a partial slot.
            _ => return,
        }
        Self::finish_interpretation(progress);
    }
}

impl<A, Ph, Sends, Birth, Input, Path> AppendSend<Input, Path> for Actions<A, Ph, Sends, Birth>
where
    A: Address,
    Birth: BirthMode,
    Sends: SendInput<Input, Path>,
{
    fn append_send(self, input: Input) -> Self {
        self.with_send::<Input, Path>(input)
    }
}

impl<A: Address, Ph, Sends: SendEffects, Birth: BirthMode> Actions<A, Ph, Sends, Birth> {
    #[must_use]
    pub const fn new(
        sends: Sends,
        creates: Creations<CreateChild<A, Birth::Child>>,
        become_: Become<Ph>,
    ) -> Self {
        Self {
            sends,
            creates,
            become_,
        }
    }

    #[must_use]
    pub fn just(become_: Become<Ph>) -> Self {
        Self {
            sends: Sends::empty(),
            creates: Creations::empty(),
            become_,
        }
    }

    #[must_use]
    pub fn cont() -> Self {
        Self::just(Step::Continue)
    }
    #[must_use]
    pub fn stop() -> Self {
        Self::just(Step::Stop(Stopped))
    }
    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }

    /// Continue after emitting the complete declared send product.
    #[must_use]
    pub fn send(sends: Sends) -> Self {
        Self::new(sends, Creations::empty(), Step::Continue)
    }

    /// Continue after staging the complete declared creation product.
    #[must_use]
    pub fn create(creates: Creations<CreateChild<A, Birth::Child>>) -> Self {
        Self::new(Sends::empty(), creates, Step::Continue)
    }
}

impl<A: Address, Ph, Sends, Birth: BirthMode>
    From<(Sends, Creations<CreateChild<A, Birth::Child>>, Become<Ph>)>
    for Actions<A, Ph, Sends, Birth>
{
    fn from(
        (sends, creates, become_): (Sends, Creations<CreateChild<A, Birth::Child>>, Become<Ph>),
    ) -> Self {
        Self {
            sends,
            creates,
            become_,
        }
    }
}

pub type Acted<A, Ph, Sends, Birth, E> = Result<Actions<A, Ph, Sends, Birth>, E>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Births, CreationSequence, MailAddr, NoBirths, Own};

    fn two_ids() -> (crate::CreationId, crate::CreationId) {
        let mut sequence = CreationSequence::new();
        let first = sequence.issue().expect("the first creation ID exists");
        let second = sequence.issue().expect("the second creation ID exists");
        (first, second)
    }

    #[test]
    fn equality_and_debug_cover_every_named_effect_leg() {
        type Plain = Actions<MailAddr, u8, Vec<u8>, NoBirths>;
        type Creating = Actions<MailAddr, Never, Vec<u8>, Births<u8>>;

        let value = Plain::new(vec![1], Creations::empty(), Step::Goto(3));
        assert_eq!(
            value,
            Plain::new(vec![1], Creations::empty(), Step::Goto(3))
        );
        assert_ne!(
            value,
            Plain::new(vec![2], Creations::empty(), Step::Goto(3))
        );
        assert_ne!(
            value,
            Plain::new(vec![1], Creations::empty(), Step::Goto(4))
        );
        assert_ne!(
            value,
            Plain::new(vec![1], Creations::empty(), Step::Continue)
        );
        assert_eq!(
            format!("{value:?}"),
            "Actions { sends: [1], creates: Creations { items: [] }, become: Goto(3) }"
        );

        let (first, second) = two_ids();
        let created = Creating::new(
            Vec::new(),
            Creations::one(CreateChild::birth(first, 9)),
            Step::Continue,
        );
        let other_creation = Creating::new(
            Vec::new(),
            Creations::one(CreateChild::birth(second, 9)),
            Step::Continue,
        );
        assert_ne!(created, other_creation);
    }

    #[test]
    fn mapping_sends_preserves_creation_order_and_verdict() {
        let (first, second) = two_ids();
        let actions: Actions<MailAddr, u8, Vec<u8>, Births<()>> = Actions::new(
            vec![1, 2],
            Creations::one(CreateChild::birth(first, ())).and(CreateChild::replacement(
                second,
                first,
                (),
            )),
            Step::Goto(7),
        );

        let mapped = actions.map_sends(|sends| sends.len());
        assert_eq!(mapped.sends, 2);
        assert_eq!(
            mapped
                .creates
                .iter()
                .map(CreateChild::id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        assert!(matches!(mapped.become_, Step::Goto(7)));
    }

    #[test]
    fn mapping_become_preserves_sends_and_creation_order() {
        let (first, _) = two_ids();
        let actions: Actions<MailAddr, u8, Vec<u8>, Births<()>> = Actions::new(
            vec![1, 2],
            Creations::one(CreateChild::birth(first, ())),
            Step::Goto(7),
        );

        let mapped: Actions<MailAddr, Never, Vec<u8>, Births<()>> =
            actions.map_become(|_| Step::Stop(Stopped));
        assert_eq!(mapped.sends, [1, 2]);
        let mapped_creation_id = mapped.creates.iter().next().map(CreateChild::id);
        assert_eq!(mapped_creation_id, Some(first));
        assert!(matches!(mapped.become_, Step::Stop(Stopped)));
    }

    #[test]
    fn fluent_send_changes_only_the_selected_effect_leg() {
        let (first, second) = two_ids();
        let actions: Actions<MailAddr, u8, Vec<u8>, Births<()>> = Actions::new(
            vec![1],
            Creations::one(CreateChild::birth(first, ())).and(CreateChild::replacement(
                second,
                first,
                (),
            )),
            Step::Goto(7),
        )
        .with_send::<_, Own>(2)
        .with_send::<_, Own>(3);

        assert_eq!(actions.sends, [1, 2, 3]);
        assert_eq!(
            actions
                .creates
                .iter()
                .map(CreateChild::id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        assert!(matches!(actions.become_, Step::Goto(7)));

        let stopped: Actions<MailAddr, Never, Vec<u8>, NoBirths> =
            Actions::stop().with_send::<_, Own>(5);
        assert_eq!(stopped.sends, [5]);
        assert!(matches!(stopped.become_, Step::Stop(Stopped)));
    }
}
