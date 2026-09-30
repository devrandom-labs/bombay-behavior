use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, Births, ChildCreationOutcome,
    CreateChild, CreationKind, CreationRejection, CreationSequence, Creations, Delivery,
    EndpointAddress, Here, InitializationTurn, MessageProtocol, Never, NoBirths, NoSends, Protocol,
    Recipient, RoutedCreation, Step, User, initialize,
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

struct OwnedText(Box<str>);

type PendingDelivery = Delivery<MessageProtocol<RuntimeAddr, OwnedText>>;

struct Grandchild {
    current: OwnedText,
}

impl Behavior for Grandchild {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

struct RefusedChild {
    current: OwnedText,
    pending_sends: Option<[OwnedText; 2]>,
    nested: Option<CreateChild<RuntimeAddr, Grandchild>>,
}

impl Behavior for RefusedChild {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<PendingDelivery>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Grandchild>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let [first, second] = self
            .pending_sends
            .take()
            .expect("this fixture initializes only once");
        let nested = self.nested.take().expect("one nested child is staged");
        Ok(Actions::create(Creations::one(nested))
            .with_send(Delivery::new(Recipient::global(RuntimeAddr(91)), first))
            .with_send(Delivery::new(Recipient::global(RuntimeAddr(92)), second)))
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

#[test]
fn host_refusal_returns_current_child_and_all_uninterpreted_actions() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("one creation ID is available");
    let current = OwnedText("current child".into());
    let first = OwnedText("first send".into());
    let second = OwnedText("second send".into());
    let current_ptr = current.0.as_ptr();
    let first_ptr = first.0.as_ptr();
    let second_ptr = second.0.as_ptr();
    let nested_current = OwnedText("nested child".into());
    let nested_ptr = nested_current.0.as_ptr();
    let nested_id = CreationSequence::new()
        .issue()
        .expect("one nested creation ID is available");
    let mut creation = RoutedCreation::new(
        CreateChild::<RuntimeAddr, _>::birth(
            id,
            RefusedChild {
                current,
                pending_sends: Some([first, second]),
                nested: Some(CreateChild::birth(
                    nested_id,
                    Grandchild {
                        current: nested_current,
                    },
                )),
            },
        ),
        73,
    );

    let initialization = initialize(creation.child_mut()).expect("pure initialization succeeds");
    let refusal = ChildCreationOutcome::<RefusedChild, Here>::HostRejected {
        creation,
        initialization,
        reason: CreationRejection::EnvironmentFailed,
    };
    let ChildCreationOutcome::HostRejected {
        creation,
        initialization,
        reason,
    } = refusal
    else {
        panic!("host refusal must return complete pre-commit custody");
    };

    let (request, route) = creation.into_parts();
    let (returned_id, child, kind) = request.into_parts();
    assert_eq!(returned_id, id);
    assert_eq!(route, 73);
    assert_eq!(kind, CreationKind::Birth);
    assert_eq!(child.current.0.as_ptr(), current_ptr);
    assert_eq!(child.current.0.as_ref(), "current child");
    assert!(child.pending_sends.is_none());
    assert!(child.nested.is_none());
    assert_eq!(reason, CreationRejection::EnvironmentFailed);

    assert!(matches!(initialization.become_, Step::Continue));
    assert_eq!(initialization.creates.len(), 1);
    let nested = initialization
        .creates
        .into_iter()
        .next()
        .expect("the staged nested child remains untouched");
    let (returned_nested_id, nested_child, nested_kind) = nested.into_parts();
    assert_eq!(returned_nested_id, nested_id);
    assert_eq!(nested_kind, CreationKind::Birth);
    assert_eq!(nested_child.current.0.as_ptr(), nested_ptr);
    assert_eq!(nested_child.current.0.as_ref(), "nested child");
    let [first, second]: [_; 2] = initialization
        .sends
        .try_into()
        .unwrap_or_else(|_| panic!("both initialization sends remain untouched"));
    assert_eq!(first.to.address(), RuntimeAddr(91));
    assert_eq!(first.message.0.as_ptr(), first_ptr);
    assert_eq!(first.message.0.as_ref(), "first send");
    assert_eq!(second.to.address(), RuntimeAddr(92));
    assert_eq!(second.message.0.as_ptr(), second_ptr);
    assert_eq!(second.message.0.as_ref(), "second send");
}

struct RejectingChild {
    current: OwnedText,
    error: Option<OwnedText>,
}

impl Behavior for RejectingChild {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = OwnedText;
    type Birth = NoBirths;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        Err(self.error.take().expect("one pure initialization attempt"))
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

#[test]
fn pure_initialization_rejection_returns_current_child_and_exact_error() {
    let id = CreationSequence::new()
        .issue()
        .expect("one creation ID is available");
    let current = OwnedText("current child".into());
    let error = OwnedText("exact rejection".into());
    let current_ptr = current.0.as_ptr();
    let error_ptr = error.0.as_ptr();
    let mut creation = RoutedCreation::new(
        CreateChild::<RuntimeAddr, _>::birth(
            id,
            RejectingChild {
                current,
                error: Some(error),
            },
        ),
        79,
    );
    let error = match initialize(creation.child_mut()) {
        Ok(_) => panic!("the pure initialization must reject"),
        Err(error) => error,
    };
    let outcome =
        ChildCreationOutcome::<RejectingChild, Here>::InitializationRejected { creation, error };
    let Err(ChildCreationOutcome::InitializationRejected { creation, error }) =
        outcome.into_actor()
    else {
        panic!("rejection must not issue an established actor");
    };
    let (request, route) = creation.into_parts();
    let (returned_id, child, kind) = request.into_parts();
    assert_eq!(returned_id, id);
    assert_eq!(route, 79);
    assert_eq!(kind, CreationKind::Birth);
    assert_eq!(child.current.0.as_ptr(), current_ptr);
    assert!(child.error.is_none());
    assert_eq!(error.0.as_ptr(), error_ptr);
    assert_eq!(error.0.as_ref(), "exact rejection");
}
