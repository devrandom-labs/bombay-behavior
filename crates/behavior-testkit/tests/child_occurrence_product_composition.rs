use core::marker::PhantomData;

use behavior_actors::StopOnShutdown;
use behavior_core::{
    Actions, Behavior, BehaviorActed, BirthMode, Births, ChildHead, ChildOccurrenceShape,
    ChildOccurrences, MailAddr, Never, NoBirths, NoSends, User,
};

struct Empty;
struct Slot<Occurrence, Child, Tail>(PhantomData<fn() -> (Occurrence, Child, Tail)>);
struct TestOccurrences;

impl ChildOccurrenceShape for TestOccurrences {
    type Empty = Empty;
    type Member<Occurrence, Child: Behavior, Tail> = Slot<Occurrence, Child, Tail>;
}

trait Same<T> {}
impl<T> Same<T> for T {}
fn assert_same<T: Same<Expected>, Expected>() {}

type ChildrenOf<B> = <<B as Behavior>::Birth as BirthMode>::Child;
type OccurrencesOf<B> = ChildOccurrences<ChildrenOf<B>, TestOccurrences>;

struct Worker;

impl behavior_core::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Team;

impl behavior_core::Protocol for Team {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Team {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Worker>;

    fn init(&mut self, _: behavior_core::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Company;

impl behavior_core::Protocol for Company {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Company {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Team>;

    fn init(&mut self, _: behavior_core::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn transparent_wrappers_preserve_the_company_team_binding() {
    type Expected = Slot<ChildHead, Team, Empty>;

    assert_same::<OccurrencesOf<Company>, Expected>();
    assert_same::<OccurrencesOf<StopOnShutdown<Company>>, Expected>();
}

#[test]
fn company_and_team_keep_their_own_child_namespaces() {
    type CompanyExpected = Slot<ChildHead, Team, Empty>;
    type TeamExpected = Slot<ChildHead, Worker, Empty>;

    assert_same::<OccurrencesOf<Company>, CompanyExpected>();
    assert_same::<OccurrencesOf<Team>, TeamExpected>();
    assert_same::<OccurrencesOf<StopOnShutdown<Team>>, TeamExpected>();
}
