//! Black-box model for application-owned child composition.
//!
//! The test-only `Provisioned` behavior stands in for Bombay's application
//! owner. Child behavior types and the resulting application type are inferred.

use behavior_actors::{FinalizeOnShutdown, ShutdownRequested, StopOnShutdown};
use behavior_core::{
    Actions, Behavior, BehaviorActed, BehaviorBase, BirthMode, BirthNodeAppend, Births,
    ChildChoice, ChildCons, ChildHead, ChildOccurrence, ChildProduct, ChildRole, Children,
    CreateChild, CreationId, CreationKind, CreationSequence, Creations, DeclaredChildOccurrence,
    MailAddr, Never, NoBirths, NoChildren, NoSends, Protocol, ResolveChildOccurrence,
    ResolvedChild, ResolvedChildPosition, Step, User, delegate_transition, initialize,
};
use proptest::collection::vec;
use proptest::prelude::*;

trait Same<T> {}

impl<T> Same<T> for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Owned;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct First;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Second;

macro_rules! inert {
    ($actor:ident) => {
        impl Protocol for $actor {
            type Addr = MailAddr;
            type Msg = Never;
        }

        impl Behavior for $actor {
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
    };
}

inert!(Owned);
inert!(First);
inert!(Second);

struct Root {
    creations: CreationSequence,
}

impl Protocol for Root {
    type Addr = MailAddr;
    type Msg = Never;
}

impl BehaviorBase for Root {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for Root {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Owned>;

    fn init(&mut self, _: behavior_core::InitializationTurn) -> BehaviorActed<Self> {
        let id = self.creations.issue().expect("the root fixture ID exists");
        Ok(Actions::create(Creations::one(CreateChild::birth(
            id, Owned,
        ))))
    }

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct OwnedRole;

impl ChildRole<Root> for OwnedRole {
    type Child = Owned;
    type Position = ChildHead;
}

impl ChildOccurrence<Root> for OwnedRole {
    type Resolution = DeclaredChildOccurrence;
}

#[derive(Debug, PartialEq, Eq)]
enum ProvisionError<E> {
    Inner(E),
    InitializedTwice,
}

struct Provisioned<R, Product = NoChildren> {
    root: R,
    children: Option<Children<MailAddr, Product>>,
    creations: CreationSequence,
}

impl<R> Provisioned<R> {
    fn new(root: R) -> Self {
        Self {
            root,
            children: Some(Children::new()),
            creations: CreationSequence::new(),
        }
    }
}

impl<R, Product> Provisioned<R, Product> {
    fn child<C>(mut self, child: C) -> Provisioned<R, ChildCons<MailAddr, C, Product>>
    where
        C: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    {
        let id = self
            .creations
            .issue()
            .expect("the application fixture ID exists");
        Provisioned {
            root: self.root,
            children: self.children.map(|children| children.child(id, child)),
            creations: self.creations,
        }
    }
}

impl<R, Product> BehaviorBase for Provisioned<R, Product>
where
    R: Behavior + BehaviorBase,
    Product: ChildProduct<MailAddr>,
    <R::Birth as BirthMode>::Child: BirthNodeAppend<Product::Choice>,
{
    type Base = R::Base;

    fn base(&self) -> &Self::Base {
        self.root.base()
    }
}

impl<R, Product> Behavior for Provisioned<R, Product>
where
    R: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    Product: ChildProduct<MailAddr>,
    <R::Birth as BirthMode>::Child: BirthNodeAppend<Product::Choice>,
{
    type Protocol = R::Protocol;
    type Event = R::Event;
    type Sends = R::Sends;
    type Ph = R::Ph;
    type Error = ProvisionError<R::Error>;
    type Birth =
        Births<<<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::Output>;

    fn init(&mut self, _: behavior_core::InitializationTurn) -> BehaviorActed<Self> {
        let inner = initialize(&mut self.root).map_err(ProvisionError::Inner)?;
        let children = self
            .children
            .take()
            .ok_or(ProvisionError::InitializedTwice)?
            .into_creates();
        let creates =
            <<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::append_creations(
                inner.creates,
                children,
            );
        Ok(Actions::new(inner.sends, creates, inner.become_))
    }

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        let inner = delegate_transition(&mut self.root, event).map_err(ProvisionError::Inner)?;
        let creates =
            <<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::append_creations(
                inner.creates,
                Creations::empty(),
            );
        Ok(Actions::new(inner.sends, creates, inner.become_))
    }
}

fn finalize_second(
    _: &mut Second,
    _: ShutdownRequested,
) -> Actions<MailAddr, Never, NoSends, NoBirths> {
    Actions::cont()
}

fn assert_owned_occurrence_is_unchanged<B>(_: &B)
where
    B: ResolveChildOccurrence<OwnedRole>,
    ResolvedChild<B, OwnedRole>: Same<Owned>,
    ResolvedChildPosition<B, OwnedRole>: Same<ChildHead>,
{
}

#[test]
fn inferred_application_children_append_after_root_children_without_aliases_or_ids() {
    let definition = Provisioned::new(StopOnShutdown::new(Root {
        creations: CreationSequence::new(),
    }))
    .child(StopOnShutdown::new(First))
    .child(FinalizeOnShutdown::new(Second, finalize_second));

    assert_owned_occurrence_is_unchanged(&definition);

    let initialized = behavior_actors::Activate::initialize(definition).unwrap();
    assert_eq!(initialized.actions.become_, Step::Continue);
    assert_eq!(initialized.actions.creates.len(), 3);

    let mut creates = initialized.actions.creates.into_iter();
    let owned = creates.next().expect("the root child is retained");
    let owned_id = owned.id();
    assert_eq!(owned.kind(), CreationKind::Birth);
    assert!(matches!(owned.child(), ChildChoice::Head(Owned)));

    let first = creates
        .next()
        .expect("the first application child is retained");
    let first_id = first.id();
    assert_eq!(first.kind(), CreationKind::Birth);
    assert!(matches!(
        first.child(),
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(_)))
    ));

    let second = creates
        .next()
        .expect("the second application child is retained");
    assert_eq!(second.kind(), CreationKind::Birth);
    assert!(matches!(
        second.child(),
        ChildChoice::Tail(ChildChoice::Head(_))
    ));
    assert!(creates.next().is_none());

    assert_eq!(owned_id, first_id);
    assert_ne!(first_id, second.id());
}

#[test]
fn child_append_is_associative_in_type_and_value_order() {
    type OwnedThenFirst = <Owned as BirthNodeAppend<First>>::Output;
    type Left = <OwnedThenFirst as BirthNodeAppend<Second>>::Output;
    type FirstThenSecond = <First as BirthNodeAppend<Second>>::Output;
    type Right = <Owned as BirthNodeAppend<FirstThenSecond>>::Output;

    fn same_type<T: Same<Right>>() {}
    same_type::<Left>();

    let mut owned_ids = CreationSequence::new();
    let owned = owned_ids.issue().expect("the owned ID exists");
    let mut first_ids = CreationSequence::new();
    let previous_first = first_ids.issue().expect("the previous first ID exists");
    let first = first_ids.issue().expect("the replacement first ID exists");
    let mut second_ids = CreationSequence::new();
    let second = second_ids.issue().expect("the second ID exists");

    let owned_then_first = <Owned as BirthNodeAppend<First>>::append_creations::<MailAddr>(
        Creations::one(CreateChild::birth(owned, Owned)),
        Creations::one(CreateChild::replacement(first, previous_first, First)),
    );
    let left = <OwnedThenFirst as BirthNodeAppend<Second>>::append_creations::<MailAddr>(
        owned_then_first,
        Creations::one(CreateChild::birth(second, Second)),
    );

    let first_then_second = <First as BirthNodeAppend<Second>>::append_creations::<MailAddr>(
        Creations::one(CreateChild::replacement(first, previous_first, First)),
        Creations::one(CreateChild::birth(second, Second)),
    );
    let right = <Owned as BirthNodeAppend<FirstThenSecond>>::append_creations::<MailAddr>(
        Creations::one(CreateChild::birth(owned, Owned)),
        first_then_second,
    );

    assert_eq!(left, right);
}

#[test]
fn empty_child_algebra_is_a_left_and_right_identity() {
    let mut first_ids = CreationSequence::new();
    let previous = first_ids.issue().expect("the previous first ID exists");
    let replacement = first_ids.issue().expect("the replacement first ID exists");
    let left = <Never as BirthNodeAppend<First>>::append_creations::<MailAddr>(
        Creations::empty(),
        Creations::one(CreateChild::replacement(replacement, previous, First)),
    );
    let mut left = left.into_iter();
    let first = left.next().expect("the left child is retained");
    assert_eq!(first.id(), replacement);
    assert_eq!(first.kind(), CreationKind::replacement(previous));
    assert!(matches!(first.child(), First));
    assert!(left.next().is_none());

    let mut owned_ids = CreationSequence::new();
    let owned = owned_ids.issue().expect("the owned ID exists");
    let right = <Owned as BirthNodeAppend<Never>>::append_creations::<MailAddr>(
        Creations::one(CreateChild::birth(owned, Owned)),
        Creations::empty(),
    );
    let mut right = right.into_iter();
    let owned_child = right.next().expect("the right child is retained");
    assert_eq!(owned_child.id(), owned);
    assert_eq!(owned_child.kind(), CreationKind::Birth);
    assert!(matches!(owned_child.child(), Owned));
    assert!(right.next().is_none());
}

#[test]
fn repeated_child_types_retain_each_static_occurrence() {
    type Prefix = ChildChoice<Owned, ChildChoice<Owned, Never>>;
    type Tail = ChildChoice<First, ChildChoice<First, Never>>;

    let mut first_owned_ids = CreationSequence::new();
    let first_owned = first_owned_ids.issue().expect("the first owned ID exists");
    let mut second_owned_ids = CreationSequence::new();
    let second_owned = second_owned_ids
        .issue()
        .expect("the second owned ID exists");
    let prefix = Creations::one(CreateChild::<MailAddr, Prefix>::birth(
        first_owned,
        ChildChoice::Head(Owned),
    ))
    .and(CreateChild::<MailAddr, Prefix>::birth(
        second_owned,
        ChildChoice::Tail(ChildChoice::Head(Owned)),
    ));

    let mut first_tail_ids = CreationSequence::new();
    let first_tail = first_tail_ids.issue().expect("the first tail ID exists");
    let mut second_tail_ids = CreationSequence::new();
    let previous_tail = second_tail_ids
        .issue()
        .expect("the previous second tail ID exists");
    let second_tail = second_tail_ids
        .issue()
        .expect("the replacement second tail ID exists");
    let tail = Creations::one(CreateChild::<MailAddr, Tail>::birth(
        first_tail,
        ChildChoice::Head(First),
    ))
    .and(CreateChild::<MailAddr, Tail>::replacement(
        second_tail,
        previous_tail,
        ChildChoice::Tail(ChildChoice::Head(First)),
    ));

    let combined = <Prefix as BirthNodeAppend<Tail>>::append_creations(prefix, tail);
    let mut combined = combined.into_iter();

    let first = combined.next().expect("the first owned child is retained");
    assert_eq!(first.id(), first_owned);
    assert!(matches!(first.child(), ChildChoice::Head(Owned)));

    let second = combined.next().expect("the second owned child is retained");
    assert_eq!(second.id(), second_owned);
    assert!(matches!(
        second.child(),
        ChildChoice::Tail(ChildChoice::Head(Owned))
    ));

    let third = combined.next().expect("the first tail child is retained");
    assert_eq!(third.id(), first_tail);
    assert!(matches!(
        third.child(),
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(First)))
    ));

    let fourth = combined
        .next()
        .expect("the replacement tail child is retained");
    assert_eq!(fourth.id(), second_tail);
    assert_eq!(fourth.kind(), CreationKind::replacement(previous_tail));
    assert!(matches!(
        fourth.child(),
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(
            First
        ))))
    ));
    assert!(combined.next().is_none());

    assert_eq!(first_owned, second_owned);
    assert_eq!(first_owned, first_tail);
}

#[derive(Clone, Copy, Debug)]
enum CreationPlan {
    Birth,
    Replacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildKind {
    Owned,
    First,
}

fn planned_creations<C: Copy>(
    plans: &[CreationPlan],
    child: C,
) -> (
    Creations<CreateChild<MailAddr, C>>,
    Vec<(CreationId, CreationKind)>,
) {
    let mut ids = CreationSequence::new();
    let mut expected = Vec::with_capacity(plans.len());
    let creations = plans
        .iter()
        .map(|plan| match plan {
            CreationPlan::Birth => {
                let id = ids.issue().expect("the generated birth ID exists");
                expected.push((id, CreationKind::Birth));
                CreateChild::birth(id, child)
            }
            CreationPlan::Replacement => {
                let previous = ids.issue().expect("the generated previous ID exists");
                let id = ids.issue().expect("the generated replacement ID exists");
                let kind = CreationKind::replacement(previous);
                expected.push((id, kind));
                CreateChild::replacement(id, previous, child)
            }
        })
        .collect();
    (creations, expected)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn append_preserves_complete_creation_order(
        prefix in vec(prop_oneof![Just(CreationPlan::Birth), Just(CreationPlan::Replacement)], 0..40),
        tail in vec(prop_oneof![Just(CreationPlan::Birth), Just(CreationPlan::Replacement)], 0..40),
    ) {
        let (prefix_creations, prefix_expected) = planned_creations(&prefix, Owned);
        let (tail_creations, tail_expected) = planned_creations(&tail, First);
        let expected = prefix_expected
            .into_iter()
            .map(|(id, kind)| (id, kind, ChildKind::Owned))
            .chain(
                tail_expected
                    .into_iter()
                    .map(|(id, kind)| (id, kind, ChildKind::First)),
            )
            .collect::<Vec<_>>();

        let combined = <Owned as BirthNodeAppend<First>>::append_creations(
            prefix_creations,
            tail_creations,
        );
        let actual = combined
            .into_iter()
            .map(|creation| {
                let (id, child, kind) = creation.into_parts();
                let child = match child {
                    ChildChoice::Head(Owned) => ChildKind::Owned,
                    ChildChoice::Tail(First) => ChildKind::First,
                };
                (id, kind, child)
            })
            .collect::<Vec<_>>();

        prop_assert_eq!(actual, expected);
    }
}
