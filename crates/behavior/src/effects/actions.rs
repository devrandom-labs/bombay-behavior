//! The explicit result of one actor behavior transition.

use super::sending::{
    ClassifySettlement, InterpretItem, InterpretSends, Interpretation, InterpreterFault,
    ItemSettlement, SendEffects, SendInput, SendSettlements, SettledItem, SettlementStatus,
    SourceAdmission, SourceCustody, SourceSettlementCustody, offer_source_in_order,
};
use crate::actor::{
    Address, BirthMode, Births, ChildCreationProduct, ChildHead, ChildNamespaceExhausted,
    CreateChild, CreationRejection, Creations, DispatchBirth, NoBirths, RoutedCreation,
};
use crate::next::{Never, Step, Stopped};
use crate::transition::{Behavior, BehaviorAddr};

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
#[doc(hidden)]
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

/// Static complete-settlement product selected by one concrete action type.
///
/// This projection performs no interpretation and names no runtime. It lets a
/// lifecycle host retain the exact result type of an action without rebuilding
/// its creation or send products.
pub trait ActionSettlements {
    type Settlements;
}

impl<A, Ph, Sends, Birth> ActionSettlements for Actions<A, Ph, Sends, Birth>
where
    A: Address,
    Sends: SendSettlements,
    Birth: CreationSettlements<A>,
{
    type Settlements = ActionSettlement<Birth::Settlements, Sends::Settlements, Ph>;
}

/// One exact action-settlement product selected by a concrete behavior.
///
/// Runtime lifecycle types use this blanket projection instead of repeating
/// the behavior's internal send- and birth-product bounds through every parent
/// and application owner. The associated value remains fully concrete and is
/// never erased or reclassified.
pub trait BehaviorSettlements: Behavior {
    type Settlements;
}

impl<B> BehaviorSettlements for B
where
    B: Behavior,
    B::Sends: SendSettlements,
    B::Birth: CreationSettlements<BehaviorAddr<B>>,
{
    type Settlements =
        <Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth> as ActionSettlements>::Settlements;
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
#[doc(hidden)]
pub trait CreationSettlements<A: Address>: BirthMode {
    type Settlements: ClassifySettlement;
}

impl<A: Address> CreationSettlements<A> for NoBirths {
    type Settlements = Creations<Never>;
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
}

/// One complete creation batch returned to its live creator.
///
/// The value retains either every routed child settlement or the entire
/// unchanged batch rejected during route preparation. It is interpreter-facing
/// custody, not an application protocol or another creation operation.
#[doc(hidden)]
#[must_use = "a returned creation batch must be admitted or retained"]
pub struct CreationsSettled<A, C>
where
    A: Address,
    C: ChildCreationProduct<A, ChildHead>,
{
    settlement: <Births<C> as CreationSettlements<A>>::Settlements,
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
    fn offer_next_to_source(
        self,
        _: &mut Host,
    ) -> impl core::future::Future<Output = SourceCustody<Self>> + Send {
        core::future::ready(SourceCustody::Exhausted(self))
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
    fn offer_next_to_source(
        self,
        host: &mut Host,
    ) -> impl core::future::Future<Output = SourceCustody<Self>> + Send {
        async move {
            if matches!(&self, CreationSettlement::Settled(settlements) if settlements.is_empty()) {
                return SourceCustody::Exhausted(self);
            }
            match host.admit_source(CreationsSettled::new(self)).await {
                Ok(()) => SourceCustody::Admitted(CreationSettlement::Settled(Creations::empty())),
                Err(returned) => SourceCustody::Closed(returned.into_settlement()),
            }
        }
    }
}

/// Generic interpretation of the one creation leg selected by a birth mode.
///
/// `NoBirths` needs no runtime capability. `Births<C>` performs one real batch
/// route attempt followed by independent child establishment in declared order.
#[doc(hidden)]
pub trait InterpretCreations<A, Interpreter, RootEvent, Path>: CreationSettlements<A>
where
    A: Address,
{
    fn interpret_creations(
        creations: Creations<CreateChild<A, Self::Child>>,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Interpretation<Self::Settlements>> + Send;
}

impl<A, Interpreter, RootEvent, Path> InterpretCreations<A, Interpreter, RootEvent, Path>
    for NoBirths
where
    A: Address,
{
    fn interpret_creations(
        _: Creations<CreateChild<A, Never>>,
        _: &mut Interpreter,
    ) -> impl core::future::Future<Output = Interpretation<Self::Settlements>> + Send {
        core::future::ready(Interpretation::Complete(Creations::empty()))
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
        creations: Creations<CreateChild<A, C>>,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            let routed = match interpreter.interpret_item(creations).await {
                ItemSettlement::Accepted(routed) => routed,
                ItemSettlement::Rejected { item, reason } => {
                    return Interpretation::Complete(CreationSettlement::Rejected {
                        creations: item,
                        reason,
                    });
                }
                ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
                ItemSettlement::Corrupt { item, fault } => {
                    return Interpretation::Corrupt(CreationSettlement::Corrupt {
                        creations: item,
                        fault,
                    });
                }
            };

            let mut remaining = routed.into_iter();
            let mut settled = Vec::with_capacity(remaining.len());
            while let Some(creation) = remaining.next() {
                let (creation, route) = creation.into_parts();
                let (id, child, kind) = creation.into_parts();
                let settlement = child.dispatch_birth(id, route, kind, interpreter).await;
                match settlement {
                    corrupt @ ItemSettlement::Corrupt { .. } => {
                        settled.push(SettledItem::Attempted(corrupt));
                        settled.extend(remaining.map(SettledItem::Unattempted));
                        return Interpretation::Corrupt(CreationSettlement::Settled(
                            Creations::from_items(settled),
                        ));
                    }
                    creation => settled.push(SettledItem::Attempted(creation)),
                }
            }
            Interpretation::Complete(CreationSettlement::Settled(Creations::from_items(settled)))
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
    fn offer_next_to_source(
        self,
        host: &mut Host,
    ) -> impl core::future::Future<Output = SourceCustody<Self>> + Send {
        async move {
            let Self {
                creations,
                sends,
                become_,
            } = self;
            offer_source_in_order(creations, sends, host)
                .await
                .map(|(creations, sends)| Self {
                    creations,
                    sends,
                    become_,
                })
        }
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

    /// Interpret this complete action value using one statically typed runtime.
    ///
    /// Creation items are attempted in vector order before the named send
    /// product. Lawful creation rejection or blocking is retained and does not
    /// suppress sends; concrete child-operation interpreters decide whether an
    /// exact binding prerequisite was satisfied. Interpreter corruption stops
    /// traversal and retains every later creation and the complete send product
    /// as unattempted. The next-behavior verdict is never reconstructed.
    pub async fn interpret<Interpreter, RootEvent, Path>(
        self,
        interpreter: &mut Interpreter,
    ) -> Interpretation<
        ActionSettlement<Birth::Settlements, <Sends as SendSettlements>::Settlements, Ph>,
    >
    where
        Ph: Send,
        Sends: InterpretSends<Interpreter, RootEvent, Path>,
        Birth: InterpretCreations<A, Interpreter, RootEvent, Path>,
        Interpreter: Send,
    {
        let Actions {
            sends,
            creates,
            become_,
        } = self;
        let creations = match Birth::interpret_creations(creates, interpreter).await {
            Interpretation::Complete(creations) => creations,
            Interpretation::Corrupt(creations) => {
                return Interpretation::Corrupt(ActionSettlement {
                    creations,
                    sends: Sends::unattempted(sends),
                    become_,
                });
            }
        };

        match Sends::interpret(sends, interpreter).await {
            Interpretation::Complete(sends) => Interpretation::Complete(ActionSettlement {
                creations,
                sends,
                become_,
            }),
            Interpretation::Corrupt(sends) => Interpretation::Corrupt(ActionSettlement {
                creations,
                sends,
                become_,
            }),
        }
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
        assert_eq!(
            mapped.creates.iter().next().map(CreateChild::id),
            Some(first)
        );
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
