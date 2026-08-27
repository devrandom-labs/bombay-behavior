use core::marker::PhantomData;

use behavior::{Proxy, StopOnShutdown};
use foundation::{
    Actions, Behavior, BehaviorActed, BirthMode, BirthNodeMapper, Births, ChildHead,
    FoldedBirthNode, MailAddr, Never, NoBirths, NoSends, User,
};

struct Empty;
struct Slot<Position, Child, Tail>(PhantomData<fn() -> (Position, Child, Tail)>);
struct Shape;

impl BirthNodeMapper for Shape {
    type Empty = Empty;
    type Mapped<Position, Child: Behavior, Tail> = Slot<Position, Child, Tail>;
}

trait Same<T> {}
impl<T> Same<T> for T {}
fn assert_same<T: Same<Expected>, Expected>() {}

type ChildrenOf<B> = <<B as Behavior>::Birth as BirthMode>::Child;
type BindingsOf<B> = FoldedBirthNode<ChildrenOf<B>, Shape>;

struct Leaf;

impl foundation::Protocol for Leaf {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Leaf {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Parent;

impl foundation::Protocol for Parent {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Parent {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Proxy<Leaf>>;

    fn init(&mut self, _: foundation::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn transparent_wrappers_preserve_the_inner_direct_binding_shape() {
    type Expected = Slot<ChildHead, Proxy<Leaf>, Empty>;

    assert_same::<BindingsOf<Parent>, Expected>();
    assert_same::<BindingsOf<StopOnShutdown<Parent>>, Expected>();
}

#[test]
fn nested_templates_fold_in_their_own_creator_namespace() {
    type ParentExpected = Slot<ChildHead, Proxy<Leaf>, Empty>;
    type ProxyExpected = Slot<ChildHead, Leaf, Empty>;

    assert_same::<BindingsOf<Parent>, ParentExpected>();
    assert_same::<BindingsOf<Proxy<Leaf>>, ProxyExpected>();
    assert_same::<BindingsOf<StopOnShutdown<Proxy<Leaf>>>, ProxyExpected>();
}
