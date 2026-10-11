use core::marker::PhantomData;

use behavior::{
    Address, Behavior, BehaviorActed, BehaviorAddr, ChildHead, ChildOccurrenceMember,
    ChildOccurrenceShape, ChildOccurrences, ChildTail, MailAddr, Never, NoBirths, NoSends,
    Protocol, User,
};

struct NoBindings;

struct RuntimeRef<P>(usize, PhantomData<fn() -> P>);

struct ChildBinding<Occurrence, Child: Behavior, Tail> {
    occurrence: PhantomData<fn() -> Occurrence>,
    endpoints: Vec<(
        <BehaviorAddr<Child> as Address>::Nonce,
        RuntimeRef<Child::Protocol>,
    )>,
    tail: Tail,
}

struct RuntimeBindings;

impl ChildOccurrenceShape for RuntimeBindings {
    type Empty = NoBindings;
}
impl<Occurrence, Child: Behavior, Tail> ChildOccurrenceMember<Occurrence, Child, Tail>
    for RuntimeBindings
{
    type Member = ChildBinding<Occurrence, Child, Tail>;
}

trait Same<T> {}

impl<T> Same<T> for T {}

fn assert_same<T: Same<Expected>, Expected>() {}

struct Worker;

#[behavior::behavior(addr = MailAddr, message = Never)]
impl Worker {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct Parent;

#[behavior::behavior(addr = MailAddr, message = Never, births = {
    primary: Worker,
    fallback: Worker,
}, creation_settlements = retain_for_retirement)]
impl Parent {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct DomainValue;
struct GenericChild<T>(PhantomData<fn() -> T>);

impl<T> behavior::Protocol for GenericChild<T> {
    type Addr = MailAddr;
    type Msg = Never;
}

impl<T> Behavior for GenericChild<T> {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn generated_duplicate_workers_have_distinct_occurrence_storage() {
    type Handwritten = behavior::ChildChoice<Worker, behavior::ChildChoice<Worker, Never>>;
    type WorkerBindings = ChildOccurrences<ParentChildren, RuntimeBindings>;
    type Expected =
        ChildBinding<ChildHead, Worker, ChildBinding<ChildTail<ChildHead>, Worker, NoBindings>>;

    assert_same::<ParentChildren, Handwritten>();
    assert_same::<WorkerBindings, Expected>();
}

#[test]
fn duplicate_occurrences_retain_independent_runtime_owned_storage() {
    let bindings: ChildOccurrences<ParentChildren, RuntimeBindings> = ChildBinding {
        occurrence: PhantomData,
        endpoints: vec![(7, RuntimeRef(11, PhantomData))],
        tail: ChildBinding {
            occurrence: PhantomData,
            endpoints: vec![(7, RuntimeRef(13, PhantomData))],
            tail: NoBindings,
        },
    };

    assert_eq!(bindings.endpoints[0].1.0, 11);
    assert_eq!(bindings.tail.endpoints[0].1.0, 13);
}

#[test]
fn direct_worker_and_empty_children_use_the_same_closed_product() {
    type Leaf = ChildOccurrences<Worker, RuntimeBindings>;
    type ExpectedLeaf = ChildBinding<ChildHead, Worker, NoBindings>;
    type Empty = ChildOccurrences<Never, RuntimeBindings>;

    assert_same::<Leaf, ExpectedLeaf>();
    assert_same::<Empty, NoBindings>();
}

#[test]
fn generic_child_domain_parameters_gain_no_behavior_obligation() {
    type WorkerBindings = ChildOccurrences<GenericChild<DomainValue>, RuntimeBindings>;
    type Expected = ChildBinding<ChildHead, GenericChild<DomainValue>, NoBindings>;

    assert_same::<WorkerBindings, Expected>();
}

#[test]
fn child_occurrence_storage_is_runtime_owned_and_has_no_behavior_value() {
    let product: ChildOccurrences<Worker, RuntimeBindings> = ChildBinding {
        occurrence: PhantomData,
        endpoints: vec![(13, RuntimeRef(29, PhantomData))],
        tail: NoBindings,
    };

    assert_eq!(product.endpoints[0].0, 13);
    assert_eq!(product.endpoints[0].1.0, 29);
    let ChildBinding { tail, .. } = product;
    let _: NoBindings = tail;
}

struct NoChildren;
struct EmptyOnly;
impl ChildOccurrenceShape for EmptyOnly {
    type Empty = NoChildren;
}
struct OccurrenceSlot<Position, Child, Tail> {
    child: Child,
    tail: Tail,
    position: PhantomData<fn() -> Position>,
}
struct EveryChild;
impl ChildOccurrenceShape for EveryChild {
    type Empty = NoChildren;
}
impl<Position, Child: Behavior, Tail> ChildOccurrenceMember<Position, Child, Tail> for EveryChild {
    type Member = OccurrenceSlot<Position, Child, Tail>;
}
struct StoredWorker {
    payload: u32,
}
impl Protocol for StoredWorker {
    type Addr = MailAddr;
    type Msg = Never;
}
impl Behavior for StoredWorker {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Birth = NoBirths;
    type Ph = Never;
    type Error = Never;
    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}
#[test]
fn empty_children_need_no_member_representation() {
    let empty: ChildOccurrences<Never, EmptyOnly> = NoChildren;
    let NoChildren = empty;
}
#[test]
fn repeated_children_retain_exact_positions_and_owned_payloads() {
    type Workers = behavior::ChildChoice<StoredWorker, behavior::ChildChoice<StoredWorker, Never>>;
    let workers: ChildOccurrences<Workers, EveryChild> = OccurrenceSlot {
        child: StoredWorker { payload: 11 },
        position: PhantomData,
        tail: OccurrenceSlot {
            child: StoredWorker { payload: 13 },
            tail: NoChildren,
            position: PhantomData,
        },
    };
    let _: &OccurrenceSlot<
        ChildHead,
        StoredWorker,
        OccurrenceSlot<ChildTail<ChildHead>, StoredWorker, NoChildren>,
    > = &workers;
    assert_eq!(workers.child.payload, 11);
    assert_eq!(workers.tail.child.payload, 13);
}
