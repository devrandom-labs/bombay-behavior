use std::panic::{AssertUnwindSafe, catch_unwind};

use behavior::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, CreateChild, CreationKind,
    CreationSequence, EndpointAddress, Here, InitializationTurn, MessageProtocol, Never, NoBirths,
    NoSends, Protocol, RoutedCreation, User, initialize,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = u64
    where
        P: Protocol<Addr = Self>;
}

enum InitializationTrace {
    Untouched,
    Entered,
}

struct PanickingChild {
    payload: Box<str>,
    trace: InitializationTrace,
}

impl Behavior for PanickingChild {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        self.trace = InitializationTrace::Entered;
        panic!("pure initialization panicked after changing the current child")
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

#[test]
fn pure_initialization_panic_returns_the_exact_outer_routed_child() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("one child identity is available");
    let payload = String::from("owned child definition").into_boxed_str();
    let original = payload.as_ptr();
    let mut routed = RoutedCreation::new(
        CreateChild::<RuntimeAddr, _>::birth(
            id,
            PanickingChild {
                payload,
                trace: InitializationTrace::Untouched,
            },
        ),
        73,
    );

    let panic = catch_unwind(AssertUnwindSafe(|| initialize(routed.child_mut())));
    assert!(panic.is_err());

    let outcome =
        ChildCreationOutcome::<PanickingChild, Here>::InitializationPanicked { creation: routed };
    let ChildCreationOutcome::InitializationPanicked { creation } = outcome else {
        panic!("the panic is distinct from a controlled initialization rejection");
    };
    let (request, route) = creation.into_parts();
    let (returned_id, child, kind) = request.into_parts();
    assert_eq!(returned_id, id);
    assert_eq!(route, 73);
    assert_eq!(kind, CreationKind::Birth);
    assert_eq!(child.payload.as_ptr(), original);
    assert_eq!(child.payload.as_ref(), "owned child definition");
    assert!(matches!(child.trace, InitializationTrace::Entered));
}
