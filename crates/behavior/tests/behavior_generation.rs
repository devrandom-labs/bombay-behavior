use behavior::{
    Actions, BehaviorActed, BirthProtocol, BirthProtocolProduct, ChildChoice, Children,
    ClassifySettlement, CreationSequence, Delivery, Interpretation, ItemSettlement,
    LogicalDeliveryProtocols, MailAddr, Never, NoBirthProtocols, Recipient, SendEffects,
    SettledItem, SettlementStatus, Step,
};

pub struct FirstDestination;

impl behavior::Protocol for FirstDestination {
    type Addr = MailAddr;
    type Msg = u8;
}

pub struct SecondDestination;

impl behavior::Protocol for SecondDestination {
    type Addr = MailAddr;
    type Msg = u8;
}

#[derive(Clone, Copy)]
struct LocalRequest(u8);

impl behavior::InterpreterRequest for LocalRequest {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
}

impl behavior::ActionItem for LocalRequest {
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

#[derive(Debug, PartialEq, Eq)]
struct FirstChild;

#[behavior::behavior(addr = MailAddr, message = Never)]
impl FirstChild {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SecondChild;

#[behavior::behavior(addr = MailAddr, message = Never)]
impl SecondChild {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct Bootstrap {
    creations: CreationSequence,
}

#[behavior::behavior(
    addr = MailAddr,
    message = (),
    sends = {
        first: Vec<Delivery<FirstDestination>>,
        second: Vec<Delivery<SecondDestination>>,
    },
    births = {
        first: FirstChild,
        second: SecondChild,
    },
    creation_settlements = retain_for_retirement,
)]
impl Bootstrap {
    fn init(&mut self) -> BehaviorActed<Self> {
        let first = self
            .creations
            .issue()
            .expect("the first fixture creation ID exists");
        let second = self
            .creations
            .issue()
            .expect("the second fixture creation ID exists");
        let creates = Children::<MailAddr>::new()
            .child(first, FirstChild)
            .child(second, SecondChild)
            .into_creates();
        Ok(Actions::create(creates)
            .send_first(Delivery::new(Recipient::global(MailAddr(10)), 1))
            .send_second(Delivery::new(Recipient::global(MailAddr(20)), 2)))
    }

    fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

impl LogicalDeliveryProtocols for BootstrapSends {
    type Protocols =
        <<Vec<Delivery<FirstDestination>> as LogicalDeliveryProtocols>::Protocols as BirthProtocolProduct>::Append<
            <Vec<Delivery<SecondDestination>> as LogicalDeliveryProtocols>::Protocols,
        >;
}

#[test]
fn generated_send_product_owner_exposes_its_exact_logical_destinations() {
    type Actual = <Bootstrap as behavior::LogicalHostRequirements>::LogicalHosts;
    type Expected =
        BirthProtocol<FirstDestination, BirthProtocol<SecondDestination, NoBirthProtocols>>;

    trait Same<T> {}
    impl<T> Same<T> for T {}
    fn exact<T: Same<Expected>, Expected>() {}

    exact::<Actual, Expected>();
}

struct Positioned;

#[behavior::behavior(
    addr = MailAddr,
    message = Never,
    births = {
        primary: FirstChild,
        secondary: SecondChild,
        fallback: FirstChild,
    },
    creation_settlements = retain_for_retirement,
)]
impl Positioned {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct LaneFamilies;

#[behavior::behavior(
    addr = MailAddr,
    message = (),
    sends = {
        requests: behavior::InterpreterRequests<LocalRequest>,
        deliveries: Vec<Delivery<FirstDestination>>,
    },
)]
impl LaneFamilies {
    fn init(&mut self) -> BehaviorActed<Self> {
        Ok(Actions::cont()
            .send_requests(LocalRequest(0))
            .send_deliveries(Delivery::new(Recipient::global(MailAddr(5)), 8)))
    }

    fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct EqualProducts;

#[behavior::behavior(
    addr = MailAddr,
    message = (),
    sends = {
        audit: behavior::InterpreterRequests<LocalRequest>,
        metrics: behavior::InterpreterRequests<LocalRequest>,
    },
)]
impl EqualProducts {
    fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
        Ok(Actions::cont()
            .send_audit(LocalRequest(1))
            .send_metrics(LocalRequest(2)))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum InitError {
    Rejected,
}

struct Fallible;

#[behavior::behavior(addr = MailAddr, message = (), error = InitError)]
impl Fallible {
    fn init(&mut self) -> BehaviorActed<Self> {
        Err(InitError::Rejected)
    }

    fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Generic<T>(core::marker::PhantomData<T>);

#[behavior::behavior(addr = MailAddr, message = T, sends = { values: Vec<T> })]
impl<T> Generic<T> {
    fn receive(&mut self, _: MailAddr, message: T) -> BehaviorActed<Self> {
        Ok(Actions::cont().send_values(message))
    }
}

struct Advanced<'a, T, const N: usize> {
    retained: &'a T,
}

#[behavior::behavior(
    addr = MailAddr,
    message = [u8; N],
    sends = {
        arrays: Vec<[u8; N]>,
        references: Vec<&'a T>,
    },
    births = {
        generic: Generic<T>,
    },
    creation_settlements = retain_for_retirement,
)]
impl<'a, T, const N: usize> Advanced<'a, T, N>
where
    T: Sync,
{
    fn receive(&mut self, _: MailAddr, message: [u8; N]) -> BehaviorActed<Self> {
        Ok(Actions::cont()
            .send_arrays(message)
            .send_references(self.retained))
    }
}

#[derive(Default)]
struct CompleteInterpreter(Vec<&'static str>);

impl<RootEvent, Path> behavior::InterpretItem<Delivery<FirstDestination>, RootEvent, Path>
    for CompleteInterpreter
{
    fn interpret_item(
        &mut self,
        _: Delivery<FirstDestination>,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Delivery<FirstDestination>,
            <Delivery<FirstDestination> as behavior::ActionItem>::Accepted,
            <Delivery<FirstDestination> as behavior::ActionItem>::Rejection,
            <Delivery<FirstDestination> as behavior::ActionItem>::Prerequisite,
        >,
    > + Send {
        self.0.push("first");
        async { ItemSettlement::Accepted(()) }
    }
}

impl<RootEvent, Path> behavior::InterpretItem<Delivery<SecondDestination>, RootEvent, Path>
    for CompleteInterpreter
{
    fn interpret_item(
        &mut self,
        _: Delivery<SecondDestination>,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Delivery<SecondDestination>,
            <Delivery<SecondDestination> as behavior::ActionItem>::Accepted,
            <Delivery<SecondDestination> as behavior::ActionItem>::Rejection,
            <Delivery<SecondDestination> as behavior::ActionItem>::Prerequisite,
        >,
    > + Send {
        self.0.push("second");
        async { ItemSettlement::Accepted(()) }
    }
}

impl<RootEvent, Path> behavior::InterpretItem<LocalRequest, RootEvent, Path>
    for CompleteInterpreter
{
    fn interpret_item(
        &mut self,
        _: LocalRequest,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            LocalRequest,
            <LocalRequest as behavior::ActionItem>::Accepted,
            <LocalRequest as behavior::ActionItem>::Rejection,
            <LocalRequest as behavior::ActionItem>::Prerequisite,
        >,
    > + Send {
        self.0.push("request");
        async { ItemSettlement::Accepted(()) }
    }
}

#[derive(Clone, Copy)]
enum FirstPlan {
    Reject,
    Corrupt,
}

struct PlannedInterpreter {
    first: FirstPlan,
    attempts: Vec<&'static str>,
}

impl<RootEvent, Path> behavior::InterpretItem<Delivery<FirstDestination>, RootEvent, Path>
    for PlannedInterpreter
{
    fn interpret_item(
        &mut self,
        item: Delivery<FirstDestination>,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Delivery<FirstDestination>,
            <Delivery<FirstDestination> as behavior::ActionItem>::Accepted,
            <Delivery<FirstDestination> as behavior::ActionItem>::Rejection,
            <Delivery<FirstDestination> as behavior::ActionItem>::Prerequisite,
        >,
    > + Send {
        self.attempts.push("first");
        let plan = self.first;
        async move {
            match plan {
                FirstPlan::Reject => ItemSettlement::Rejected {
                    item,
                    reason: behavior::LogicalDeliveryReason::ClosedRecipient,
                },
                FirstPlan::Corrupt => ItemSettlement::Corrupt {
                    item,
                    fault: behavior::InterpreterFault::CorruptTraversal,
                },
            }
        }
    }
}

impl<RootEvent, Path> behavior::InterpretItem<Delivery<SecondDestination>, RootEvent, Path>
    for PlannedInterpreter
{
    fn interpret_item(
        &mut self,
        _: Delivery<SecondDestination>,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Delivery<SecondDestination>,
            <Delivery<SecondDestination> as behavior::ActionItem>::Accepted,
            <Delivery<SecondDestination> as behavior::ActionItem>::Rejection,
            <Delivery<SecondDestination> as behavior::ActionItem>::Prerequisite,
        >,
    > + Send {
        self.attempts.push("second");
        async { ItemSettlement::Accepted(()) }
    }
}

fn exact_actions(
    _: &Actions<MailAddr, Never, BootstrapSends, behavior::RetirementBirths<BootstrapChildren>>,
) {
}

#[test]
fn generated_products_preserve_exact_initialization_actions() {
    let mut actor = Bootstrap {
        creations: CreationSequence::new(),
    };
    let actions = behavior::initialize(&mut actor).expect("fixture initialization succeeds");
    exact_actions(&actions);

    assert_eq!(actions.sends.first.len(), 1);
    assert_eq!(actions.sends.second.len(), 1);
    assert_eq!(actions.sends.first[0].message, 1);
    assert_eq!(actions.sends.second[0].message, 2);
    assert_eq!(actions.creates.len(), 2);
    let mut creations = actions.creates.into_iter();
    let first = creations.next().expect("the first child is retained");
    let (_, first, _) = first.into_parts();
    assert!(matches!(
        first,
        ChildChoice::Tail(ChildChoice::Head(FirstChild))
    ));
    let second = creations.next().expect("the second child is retained");
    let (_, second, _) = second.into_parts();
    assert!(matches!(second, ChildChoice::Head(SecondChild)));
    assert!(creations.next().is_none());
}

fn accepts_named_child<Parent, Role>(_: Role, _: Role::Child)
where
    Parent: behavior::Behavior,
    Role: behavior::ChildRole<Parent>,
{
}

fn has_child_position<Parent, Role, Position>()
where
    Parent: behavior::Behavior,
    Role: behavior::ChildRole<Parent, Position = Position>,
{
}

fn resolves_child_occurrence<Emitter, Occurrence, Child, Position>()
where
    Emitter: behavior::ResolveChildOccurrence<Occurrence, Child = Child, Position = Position>,
    Child: behavior::Behavior,
{
}

#[derive(Debug, PartialEq, Eq)]
enum IndependentTarget<Head, Tail> {
    Selected(u64, core::marker::PhantomData<fn() -> Head>),
    Remaining(Tail),
}

enum NoIndependentTargets {}

trait LowerAt<Child, Position> {
    fn lower(nonce: u64) -> Self;
}

impl<Child, Tail> LowerAt<Child, behavior::ChildHead> for IndependentTarget<Child, Tail> {
    fn lower(nonce: u64) -> Self {
        Self::Selected(nonce, core::marker::PhantomData)
    }
}

impl<Head, Tail, Child, Position> LowerAt<Child, behavior::ChildTail<Position>>
    for IndependentTarget<Head, Tail>
where
    Tail: LowerAt<Child, Position>,
{
    fn lower(nonce: u64) -> Self {
        Self::Remaining(Tail::lower(nonce))
    }
}

fn lower_named_child<Parent, Role, Targets>(_: Role, nonce: u64) -> Targets
where
    Parent: behavior::Behavior,
    Role: behavior::ChildRole<Parent>,
    Targets: LowerAt<Role::Child, Role::Position>,
{
    Targets::lower(nonce)
}

#[test]
fn generated_child_selectors_prove_the_exact_parent_role_and_child() {
    accepts_named_child::<Bootstrap, _>(BootstrapChild::First, FirstChild);
    accepts_named_child::<Bootstrap, _>(BootstrapChild::Second, SecondChild);

    let _: BootstrapChildrenFirst = BootstrapChild::First;
    let _: BootstrapChildrenSecond = BootstrapChild::Second;
    has_child_position::<Bootstrap, BootstrapChildrenFirst, behavior::ChildTail<behavior::ChildHead>>(
    );
    has_child_position::<Bootstrap, BootstrapChildrenSecond, behavior::ChildHead>();
    resolves_child_occurrence::<
        Bootstrap,
        BootstrapChildrenFirst,
        FirstChild,
        behavior::ChildTail<behavior::ChildHead>,
    >();
    resolves_child_occurrence::<Bootstrap, BootstrapChildrenSecond, SecondChild, behavior::ChildHead>(
    );
    resolves_child_occurrence::<Bootstrap, behavior::ChildHead, SecondChild, behavior::ChildHead>();
    resolves_child_occurrence::<
        Bootstrap,
        behavior::ChildTail<behavior::ChildHead>,
        FirstChild,
        behavior::ChildTail<behavior::ChildHead>,
    >();
}

#[test]
fn generated_positions_lower_named_roles_into_an_independent_sum() {
    type Targets = IndependentTarget<
        FirstChild,
        IndependentTarget<SecondChild, IndependentTarget<FirstChild, NoIndependentTargets>>,
    >;

    let primary: Targets = lower_named_child::<Positioned, _, _>(PositionedChild::Primary, 3);
    let secondary: Targets = lower_named_child::<Positioned, _, _>(PositionedChild::Secondary, 5);
    let fallback: Targets = lower_named_child::<Positioned, _, _>(PositionedChild::Fallback, 7);

    assert!(matches!(
        primary,
        IndependentTarget::Remaining(IndependentTarget::Remaining(IndependentTarget::Selected(
            3,
            _
        )))
    ));
    assert!(matches!(
        secondary,
        IndependentTarget::Remaining(IndependentTarget::Selected(5, _))
    ));
    assert!(matches!(fallback, IndependentTarget::Selected(7, _)));

    resolves_child_occurrence::<
        Positioned,
        PositionedChildrenPrimary,
        FirstChild,
        behavior::ChildTail<behavior::ChildTail<behavior::ChildHead>>,
    >();
    resolves_child_occurrence::<
        Positioned,
        PositionedChildrenFallback,
        FirstChild,
        behavior::ChildHead,
    >();
}

#[test]
fn generated_child_roles_preserve_only_the_child_types_required_generics() {
    accepts_named_child::<Advanced<'static, u16, 3>, _>(
        AdvancedChild::Generic,
        Generic(core::marker::PhantomData),
    );
    has_child_position::<Advanced<'static, u16, 3>, AdvancedChildrenGeneric, behavior::ChildHead>();
    resolves_child_occurrence::<
        Advanced<'static, u16, 3>,
        AdvancedChildrenGeneric,
        Generic<u16>,
        behavior::ChildHead,
    >();
}

#[test]
fn equal_payload_protocols_remain_distinct_named_lanes() {
    let sends = BootstrapSends {
        first: vec![Delivery::new(Recipient::global(MailAddr(3)), 7)],
        second: vec![Delivery::new(Recipient::global(MailAddr(4)), 7)],
    };

    assert_eq!(sends.first[0].to.address(), MailAddr(3));
    assert_eq!(sends.second[0].to.address(), MailAddr(4));
}

#[test]
fn generated_fluent_lanes_preserve_verdict_order_and_lane_identity() {
    let actions: Actions<MailAddr, Never, BootstrapSends, behavior::NoBirths> = Actions::stop()
        .send_first(Delivery::new(Recipient::global(MailAddr(1)), 1))
        .send_second(Delivery::new(Recipient::global(MailAddr(2)), 2))
        .send_first(Delivery::new(Recipient::global(MailAddr(3)), 3));

    assert_eq!(
        actions
            .sends
            .first
            .iter()
            .map(|delivery| delivery.message)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        actions
            .sends
            .second
            .iter()
            .map(|delivery| delivery.message)
            .collect::<Vec<_>>(),
        [2]
    );
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(_)));
}

#[test]
fn omitted_capabilities_are_empty_and_uninhabited() {
    let mut child = FirstChild;
    let actions = behavior::initialize(&mut child).expect("default initialization succeeds");
    let actions: Actions<MailAddr, Never, behavior::NoSends, behavior::NoBirths> = actions;
    assert!(matches!(actions.sends, behavior::NoSends));
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn generated_products_append_each_lane_without_crossing_them() {
    let mut left = BootstrapSends {
        first: vec![Delivery::new(Recipient::global(MailAddr(1)), 1)],
        second: vec![Delivery::new(Recipient::global(MailAddr(2)), 2)],
    };
    left.append(BootstrapSends {
        first: vec![Delivery::new(Recipient::global(MailAddr(3)), 3)],
        second: vec![Delivery::new(Recipient::global(MailAddr(4)), 4)],
    });

    assert_eq!(
        left.first
            .iter()
            .map(|send| send.message)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        left.second
            .iter()
            .map(|send| send.message)
            .collect::<Vec<_>>(),
        [2, 4]
    );
}

#[tokio::test]
async fn generated_interpreter_visits_every_lane_in_declaration_order() {
    let sends = BootstrapSends {
        first: vec![Delivery::new(Recipient::global(MailAddr(1)), 1)],
        second: vec![Delivery::new(Recipient::global(MailAddr(2)), 2)],
    };
    let mut interpreter = CompleteInterpreter::default();

    let settlement = <BootstrapSends as behavior::InterpretSends<
        CompleteInterpreter,
        behavior::User<MailAddr, ()>,
        behavior::Here,
    >>::interpret(sends, &mut interpreter)
    .await;

    assert_eq!(interpreter.0, ["first", "second"]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("infallible fixture interpretation corrupted");
    };
    assert!(matches!(
        settlement.first.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(()))]
    ));
    assert!(matches!(
        settlement.second.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(()))]
    ));
}

#[tokio::test]
async fn generated_product_continues_rejection_and_retains_corrupt_suffix() {
    fn sends() -> BootstrapSends {
        BootstrapSends {
            first: vec![Delivery::new(Recipient::global(MailAddr(1)), 1)],
            second: vec![Delivery::new(Recipient::global(MailAddr(2)), 2)],
        }
    }

    let mut rejecting = PlannedInterpreter {
        first: FirstPlan::Reject,
        attempts: Vec::new(),
    };
    let settlement = <BootstrapSends as behavior::InterpretSends<
        PlannedInterpreter,
        behavior::User<MailAddr, ()>,
        behavior::Here,
    >>::interpret(sends(), &mut rejecting)
    .await;
    assert_eq!(rejecting.attempts, ["first", "second"]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("lawful rejection cannot corrupt the product");
    };
    assert_eq!(settlement.settlement_status(), SettlementStatus::Rejected);
    assert!(matches!(
        settlement.first.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Rejected {
            item: Delivery { message: 1, .. },
            reason: behavior::LogicalDeliveryReason::ClosedRecipient,
        })]
    ));

    let mut corrupting = PlannedInterpreter {
        first: FirstPlan::Corrupt,
        attempts: Vec::new(),
    };
    let settlement = <BootstrapSends as behavior::InterpretSends<
        PlannedInterpreter,
        behavior::User<MailAddr, ()>,
        behavior::Here,
    >>::interpret(sends(), &mut corrupting)
    .await;
    assert_eq!(corrupting.attempts, ["first"]);
    let Interpretation::Corrupt(settlement) = settlement else {
        panic!("interpreter corruption must mark the complete product");
    };
    assert_eq!(settlement.settlement_status(), SettlementStatus::Corrupt);
    assert!(matches!(
        settlement.first.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Corrupt {
            item: Delivery { message: 1, .. },
            fault: behavior::InterpreterFault::CorruptTraversal,
        })]
    ));
    assert!(matches!(
        settlement.second.as_slice(),
        [SettledItem::Unattempted(Delivery { message: 2, .. })]
    ));
}

#[tokio::test]
async fn generated_product_composes_delivery_and_interpreter_request_lanes() {
    let mut actor = LaneFamilies;
    let actions = behavior::initialize(&mut actor).expect("lane initialization succeeds");
    let mut interpreter = CompleteInterpreter::default();

    let settlement = <LaneFamiliesSends as behavior::InterpretSends<
        CompleteInterpreter,
        behavior::User<MailAddr, ()>,
        behavior::Here,
    >>::interpret(actions.sends, &mut interpreter)
    .await;

    assert_eq!(interpreter.0, ["request", "first"]);
    let Interpretation::Complete(settlement) = settlement else {
        panic!("infallible fixture interpretation corrupted");
    };
    assert!(matches!(
        settlement.requests.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(()))]
    ));
    assert!(matches!(
        settlement.deliveries.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(()))]
    ));
}

#[test]
fn identical_product_types_require_distinct_lane_selectors() {
    let mut actor = EqualProducts;
    let actions = behavior::delegate_transition(
        &mut actor,
        behavior::User {
            from: MailAddr(0),
            message: (),
        },
    )
    .expect("equal-product transition succeeds");
    assert_eq!(actions.sends.audit.as_slice()[0].0, 1);
    assert_eq!(actions.sends.metrics.as_slice()[0].0, 2);
}

#[test]
fn declared_error_is_exact_for_initialization() {
    let mut actor = Fallible;
    assert_eq!(behavior::initialize(&mut actor), Err(InitError::Rejected));
}

#[test]
fn generated_products_preserve_actor_generics() {
    let mut actor = Generic::<u16>(core::marker::PhantomData);
    let actions = behavior::delegate_transition(
        &mut actor,
        behavior::User {
            from: MailAddr(1),
            message: 9,
        },
    )
    .expect("generic transition succeeds");
    assert_eq!(actions.sends.values, [9]);
}

#[test]
fn generated_products_project_lifetime_type_const_and_where_clause_generics() {
    static RETAINED: u32 = 11;
    let mut actor = Advanced::<u32, 3> {
        retained: &RETAINED,
    };
    let actions = behavior::delegate_transition(
        &mut actor,
        behavior::User {
            from: MailAddr(1),
            message: [1, 2, 3],
        },
    )
    .expect("advanced generic transition succeeds");
    let _: &AdvancedSends<'static, u32, 3> = &actions.sends;
    let _: core::marker::PhantomData<AdvancedChildren<u32>> = core::marker::PhantomData;
    assert_eq!(actions.sends.arrays, [[1, 2, 3]]);
    assert_eq!(actions.sends.references, [&11]);
}

mod capability_matrix {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum MatrixError {
        Rejected,
    }

    macro_rules! without_init {
        ($name:ident $(, $($args:tt)*)?) => {
            struct $name;

            #[behavior::behavior(addr = MailAddr, message = () $(, $($args)*)?)]
            impl $name {
                fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }
        };
    }

    macro_rules! with_init {
        ($name:ident $(, $($args:tt)*)?) => {
            struct $name;

            #[behavior::behavior(addr = MailAddr, message = () $(, $($args)*)?)]
            impl $name {
                fn init(&mut self) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }

                fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }
        };
    }

    without_init!(NoneNoInit);
    without_init!(SendsNoInit, sends = { output: behavior::InterpreterRequests<LocalRequest> });
    without_init!(BirthsNoInit,
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement
    );
    without_init!(ErrorNoInit, error = MatrixError);
    without_init!(SendsBirthsNoInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement
    );
    without_init!(SendsErrorNoInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        error = MatrixError
    );
    without_init!(BirthsErrorNoInit,
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement,
        error = MatrixError
    );
    without_init!(AllNoInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement,
        error = MatrixError
    );

    with_init!(NoneWithInit);
    with_init!(SendsWithInit, sends = { output: behavior::InterpreterRequests<LocalRequest> });
    with_init!(BirthsWithInit,
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement
    );
    with_init!(ErrorWithInit, error = MatrixError);
    with_init!(SendsBirthsWithInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement
    );
    with_init!(SendsErrorWithInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        error = MatrixError
    );
    with_init!(BirthsErrorWithInit,
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement,
        error = MatrixError
    );
    with_init!(AllWithInit,
        sends = { output: behavior::InterpreterRequests<LocalRequest> },
        births = { child: FirstChild },
        creation_settlements = retain_for_retirement,
        error = MatrixError
    );

    fn assert_shape<B, Sends, Birth, Error>()
    where
        B: behavior::Behavior<Sends = Sends, Birth = Birth, Error = Error>,
        Sends: behavior::SendEffects + behavior::SendsFor<B::Event>,
        Birth: behavior::BirthMode,
    {
    }

    fn assert_initializes<B>(mut actor: B)
    where
        B: behavior::Behavior<Ph = Never>,
        B::Error: core::fmt::Debug,
    {
        let actions = behavior::initialize(&mut actor).expect("matrix initialization succeeds");
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }

    #[test]
    fn every_capability_and_initializer_combination_has_the_exact_shape() {
        let _ = MatrixError::Rejected;
        type NoBirth = behavior::NoBirths;
        type OneBirth = behavior::RetirementBirths<BirthsNoInitChildren>;

        assert_shape::<NoneNoInit, behavior::NoSends, NoBirth, Never>();
        assert_shape::<SendsNoInit, SendsNoInitSends, NoBirth, Never>();
        assert_shape::<BirthsNoInit, behavior::NoSends, OneBirth, Never>();
        assert_shape::<ErrorNoInit, behavior::NoSends, NoBirth, MatrixError>();
        assert_shape::<
            SendsBirthsNoInit,
            SendsBirthsNoInitSends,
            behavior::RetirementBirths<SendsBirthsNoInitChildren>,
            Never,
        >();
        assert_shape::<SendsErrorNoInit, SendsErrorNoInitSends, NoBirth, MatrixError>();
        assert_shape::<
            BirthsErrorNoInit,
            behavior::NoSends,
            behavior::RetirementBirths<BirthsErrorNoInitChildren>,
            MatrixError,
        >();
        assert_shape::<
            AllNoInit,
            AllNoInitSends,
            behavior::RetirementBirths<AllNoInitChildren>,
            MatrixError,
        >();

        assert_shape::<NoneWithInit, behavior::NoSends, NoBirth, Never>();
        assert_shape::<SendsWithInit, SendsWithInitSends, NoBirth, Never>();
        assert_shape::<
            BirthsWithInit,
            behavior::NoSends,
            behavior::RetirementBirths<BirthsWithInitChildren>,
            Never,
        >();
        assert_shape::<ErrorWithInit, behavior::NoSends, NoBirth, MatrixError>();
        assert_shape::<
            SendsBirthsWithInit,
            SendsBirthsWithInitSends,
            behavior::RetirementBirths<SendsBirthsWithInitChildren>,
            Never,
        >();
        assert_shape::<SendsErrorWithInit, SendsErrorWithInitSends, NoBirth, MatrixError>();
        assert_shape::<
            BirthsErrorWithInit,
            behavior::NoSends,
            behavior::RetirementBirths<BirthsErrorWithInitChildren>,
            MatrixError,
        >();
        assert_shape::<
            AllWithInit,
            AllWithInitSends,
            behavior::RetirementBirths<AllWithInitChildren>,
            MatrixError,
        >();
    }

    #[test]
    fn every_capability_and_initializer_combination_initializes() {
        assert_initializes(NoneNoInit);
        assert_initializes(SendsNoInit);
        assert_initializes(BirthsNoInit);
        assert_initializes(ErrorNoInit);
        assert_initializes(SendsBirthsNoInit);
        assert_initializes(SendsErrorNoInit);
        assert_initializes(BirthsErrorNoInit);
        assert_initializes(AllNoInit);
        assert_initializes(NoneWithInit);
        assert_initializes(SendsWithInit);
        assert_initializes(BirthsWithInit);
        assert_initializes(ErrorWithInit);
        assert_initializes(SendsBirthsWithInit);
        assert_initializes(SendsErrorWithInit);
        assert_initializes(BirthsErrorWithInit);
        assert_initializes(AllWithInit);
    }
}
