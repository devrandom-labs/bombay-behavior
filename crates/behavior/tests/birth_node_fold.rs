use core::marker::PhantomData;

use behavior::{
    Address, Behavior, BehaviorActed, BehaviorAddr, BirthNodeMapper, ChildHead, ChildTail,
    FoldedBirthNode, MailAddr, Never, NoBirths, User,
};

struct NoBindings;

struct RuntimeRef<P>(usize, PhantomData<fn() -> P>);

struct ChildBinding<Position, Child: Behavior, Tail> {
    position: PhantomData<fn() -> Position>,
    endpoints: Vec<(
        <BehaviorAddr<Child> as Address>::Nonce,
        RuntimeRef<Child::Protocol>,
    )>,
    tail: Tail,
}

struct RuntimeBindings;

impl BirthNodeMapper for RuntimeBindings {
    type Empty = NoBindings;
    type Mapped<Position, Child: Behavior, Tail> = ChildBinding<Position, Child, Tail>;
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
})]
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
fn generated_duplicate_children_fold_to_distinct_structural_slots() {
    type Handwritten = behavior::ChildChoice<Worker, behavior::ChildChoice<Worker, Never>>;
    type Folded = FoldedBirthNode<ParentChildren, RuntimeBindings>;
    type Expected =
        ChildBinding<ChildHead, Worker, ChildBinding<ChildTail<ChildHead>, Worker, NoBindings>>;

    assert_same::<ParentChildren, Handwritten>();
    assert_same::<Folded, Expected>();
}

#[test]
fn duplicate_occurrences_retain_independent_runtime_owned_storage() {
    let bindings: FoldedBirthNode<ParentChildren, RuntimeBindings> = ChildBinding {
        position: PhantomData,
        endpoints: vec![(7, RuntimeRef(11, PhantomData))],
        tail: ChildBinding {
            position: PhantomData,
            endpoints: vec![(7, RuntimeRef(13, PhantomData))],
            tail: NoBindings,
        },
    };

    assert_eq!(bindings.endpoints[0].1.0, 11);
    assert_eq!(bindings.tail.endpoints[0].1.0, 13);
}

#[test]
fn direct_leaf_and_empty_nodes_use_the_same_closed_fold() {
    type Leaf = FoldedBirthNode<Worker, RuntimeBindings>;
    type ExpectedLeaf = ChildBinding<ChildHead, Worker, NoBindings>;
    type Empty = FoldedBirthNode<Never, RuntimeBindings>;

    assert_same::<Leaf, ExpectedLeaf>();
    assert_same::<Empty, NoBindings>();
}

#[test]
fn generic_child_domain_parameters_gain_no_behavior_obligation() {
    type Folded = FoldedBirthNode<GenericChild<DomainValue>, RuntimeBindings>;
    type Expected = ChildBinding<ChildHead, GenericChild<DomainValue>, NoBindings>;

    assert_same::<Folded, Expected>();
}

#[test]
fn mapped_product_is_runtime_owned_and_has_no_behavior_value() {
    let product: FoldedBirthNode<Worker, RuntimeBindings> = ChildBinding {
        position: PhantomData,
        endpoints: vec![(13, RuntimeRef(29, PhantomData))],
        tail: NoBindings,
    };

    assert_eq!(product.endpoints[0].0, 13);
    assert_eq!(product.endpoints[0].1.0, 29);
    let ChildBinding { tail, .. } = product;
    let _: NoBindings = tail;
}
