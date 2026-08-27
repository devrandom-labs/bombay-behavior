//! Black-box model for application-owned child provisioning.
//!
//! The test-only `Provisioned` behavior stands in for Bombay's application
//! topology.  Its type is inferred from `.child(...)` calls; application code
//! never names either wrapped child or the resulting topology type.

use behavior::{FinalizeOnShutdown, ShutdownRequested, StopOnShutdown};
use foundation::{
    Actions, Behavior, BehaviorActed, BehaviorBase, BirthMode, BirthNodeAppend, Births,
    ChildChoice, ChildCons, ChildHead, ChildOccurrence, ChildProduct, ChildRole, Children,
    ChildrenError, Create, CreationKind, DeclaredChildOccurrence, MailAddr, Never, NoBirths,
    NoChildren, NoSends, Protocol, ResolveChildOccurrence, ResolvedChild, ResolvedChildPosition,
    Step, User, delegate_transition, initialize,
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
                _: foundation::ActiveTurn,
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

struct Root;

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

    fn init(&mut self, _: foundation::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::create(vec![Create::replacement_incarnation(
            3, 2, Owned,
        )]))
    }

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
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
enum ProvisionError<E, N> {
    Inner(E),
    Children(ChildrenError<N>),
    InitializedTwice,
}

struct Provisioned<R, Product = NoChildren> {
    root: R,
    children: Option<Children<MailAddr, Product>>,
}

impl<R> Provisioned<R> {
    fn new(root: R) -> Self {
        Self {
            root,
            children: Some(Children::new()),
        }
    }
}

impl<R, Product> Provisioned<R, Product> {
    fn child<C>(self, nonce: u64, child: C) -> Provisioned<R, ChildCons<MailAddr, C, Product>>
    where
        C: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    {
        Provisioned {
            root: self.root,
            children: self.children.map(|children| children.child(nonce, child)),
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
    type Error = ProvisionError<R::Error, u64>;
    type Birth =
        Births<<<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::Output>;

    fn init(&mut self, _: foundation::InitializationTurn) -> BehaviorActed<Self> {
        let inner = initialize(&mut self.root).map_err(ProvisionError::Inner)?;
        let children = self
            .children
            .take()
            .ok_or(ProvisionError::InitializedTwice)?
            .into_creates()
            .map_err(ProvisionError::Children)?;
        let creates =
            <<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::append_creations(
                inner.creates,
                children,
            );
        Ok(Actions::new(inner.sends, creates, inner.become_))
    }

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let inner = delegate_transition(&mut self.root, event).map_err(ProvisionError::Inner)?;
        let creates =
            <<R::Birth as BirthMode>::Child as BirthNodeAppend<Product::Choice>>::append_creations(
                inner.creates,
                Vec::new(),
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
fn inferred_application_children_append_after_root_births_without_aliases() {
    let definition = Provisioned::new(StopOnShutdown::new(Root))
        .child(7, StopOnShutdown::new(First))
        .child(8, FinalizeOnShutdown::new(Second, finalize_second));

    assert_owned_occurrence_is_unchanged(&definition);

    let initialized = behavior::Activate::initialize(definition).unwrap();
    assert_eq!(initialized.actions.become_, Step::Continue);
    assert_eq!(initialized.actions.creates.len(), 3);

    let mut creates = initialized.actions.creates.into_iter();
    let owned = creates.next().unwrap();
    assert_eq!(owned.nonce, 3);
    assert_eq!(
        owned.kind,
        CreationKind::ReplacementIncarnation { replaces: 2 }
    );
    assert!(matches!(owned.child, ChildChoice::Head(Owned)));

    let first = creates.next().unwrap();
    assert_eq!(first.nonce, 7);
    assert_eq!(first.kind, CreationKind::Birth);
    assert!(matches!(
        first.child,
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(_)))
    ));

    let second = creates.next().unwrap();
    assert_eq!(second.nonce, 8);
    assert_eq!(second.kind, CreationKind::Birth);
    assert!(matches!(
        second.child,
        ChildChoice::Tail(ChildChoice::Head(_))
    ));
    assert!(creates.next().is_none());
}

#[test]
fn birth_append_is_associative_in_type_and_value_order() {
    type OwnedThenFirst = <Owned as BirthNodeAppend<First>>::Output;
    type Left = <OwnedThenFirst as BirthNodeAppend<Second>>::Output;
    type FirstThenSecond = <First as BirthNodeAppend<Second>>::Output;
    type Right = <Owned as BirthNodeAppend<FirstThenSecond>>::Output;

    fn same_type<T: Same<Right>>() {}
    same_type::<Left>();

    let owned_then_first = <Owned as BirthNodeAppend<First>>::append_creations::<MailAddr>(
        vec![Create::birth(1, Owned)],
        vec![Create::replacement_incarnation(2, 20, First)],
    );
    let left = <OwnedThenFirst as BirthNodeAppend<Second>>::append_creations::<MailAddr>(
        owned_then_first,
        vec![Create::birth(3, Second)],
    );

    let first_then_second = <First as BirthNodeAppend<Second>>::append_creations::<MailAddr>(
        vec![Create::replacement_incarnation(2, 20, First)],
        vec![Create::birth(3, Second)],
    );
    let right = <Owned as BirthNodeAppend<FirstThenSecond>>::append_creations::<MailAddr>(
        vec![Create::birth(1, Owned)],
        first_then_second,
    );

    assert_eq!(left, right);
}

#[test]
fn empty_birth_algebra_is_a_left_and_right_identity() {
    let left = <Never as BirthNodeAppend<First>>::append_creations::<MailAddr>(
        Vec::new(),
        vec![Create::replacement_incarnation(5, 4, First)],
    );
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].nonce, 5);
    assert_eq!(
        left[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 4 }
    );
    assert!(matches!(left[0].child, First));

    let right = <Owned as BirthNodeAppend<Never>>::append_creations::<MailAddr>(
        vec![Create::birth(9, Owned)],
        Vec::new(),
    );
    assert_eq!(right.len(), 1);
    assert_eq!(right[0].nonce, 9);
    assert_eq!(right[0].kind, CreationKind::Birth);
    assert!(matches!(right[0].child, Owned));
}

#[test]
fn duplicate_child_types_retain_each_structural_occurrence() {
    type Prefix = ChildChoice<Owned, ChildChoice<Owned, Never>>;
    type Tail = ChildChoice<First, ChildChoice<First, Never>>;

    let prefix = vec![
        Create::<MailAddr, Prefix>::birth(1, ChildChoice::Head(Owned)),
        Create::<MailAddr, Prefix>::replacement_incarnation(
            2,
            20,
            ChildChoice::Tail(ChildChoice::Head(Owned)),
        ),
    ];
    let tail = vec![
        Create::<MailAddr, Tail>::birth(3, ChildChoice::Head(First)),
        Create::<MailAddr, Tail>::replacement_incarnation(
            4,
            40,
            ChildChoice::Tail(ChildChoice::Head(First)),
        ),
    ];

    let combined = <Prefix as BirthNodeAppend<Tail>>::append_creations(prefix, tail);
    assert_eq!(combined.len(), 4);
    assert!(matches!(combined[0].child, ChildChoice::Head(Owned)));
    assert!(matches!(
        combined[1].child,
        ChildChoice::Tail(ChildChoice::Head(Owned))
    ));
    assert!(matches!(
        combined[2].child,
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(First)))
    ));
    assert!(matches!(
        combined[3].child,
        ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Tail(ChildChoice::Head(
            First
        ))))
    ));
    assert_eq!(
        combined
            .iter()
            .map(|creation| (creation.nonce, creation.kind))
            .collect::<Vec<_>>(),
        vec![
            (1, CreationKind::Birth),
            (2, CreationKind::ReplacementIncarnation { replaces: 20 }),
            (3, CreationKind::Birth),
            (4, CreationKind::ReplacementIncarnation { replaces: 40 }),
        ]
    );
}

fn kind(replacement: bool, replaces: u64) -> CreationKind<u64> {
    if replacement {
        CreationKind::ReplacementIncarnation { replaces }
    } else {
        CreationKind::Birth
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn append_preserves_complete_creation_traces(
        prefix in vec((any::<u64>(), any::<bool>(), any::<u64>()), 0..40),
        tail in vec((any::<u64>(), any::<bool>(), any::<u64>()), 0..40),
    ) {
        let prefix_creations = prefix
            .iter()
            .map(|&(nonce, replacement, replaces)| {
                Create::<MailAddr, Owned>::new(nonce, Owned, kind(replacement, replaces))
            })
            .collect();
        let tail_creations = tail
            .iter()
            .map(|&(nonce, replacement, replaces)| {
                Create::<MailAddr, First>::new(nonce, First, kind(replacement, replaces))
            })
            .collect();

        let combined = <Owned as BirthNodeAppend<First>>::append_creations(
            prefix_creations,
            tail_creations,
        );
        prop_assert_eq!(combined.len(), prefix.len() + tail.len());

        for (index, creation) in combined.into_iter().enumerate() {
            let (nonce, replacement, replaces) = if index < prefix.len() {
                prefix[index]
            } else {
                tail[index - prefix.len()]
            };
            prop_assert_eq!(creation.nonce, nonce);
            prop_assert_eq!(creation.kind, kind(replacement, replaces));
            if index < prefix.len() {
                prop_assert!(matches!(creation.child, ChildChoice::Head(Owned)));
            } else {
                prop_assert!(matches!(creation.child, ChildChoice::Tail(First)));
            }
        }
    }
}
