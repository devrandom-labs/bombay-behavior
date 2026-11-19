//! Management replies use only the destination capabilities selected by callers.

mod installed_control;

use behavior::{
    Address, Behavior, BehaviorActed, EndpointAddress, EstablishedDelivery, EstablishedRecipient,
    LogicalHostRequirements, MessageProtocol, Never, NoBirthProtocols, Protocol, Recipient,
    SendLayer,
};
use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::supervisor::directory::{
    self, DynamicCommand, DynamicLifecycle, EntryCapacity, QueryReply, ReplaceRejection,
    UnexpectedExit, WorkerChangeReceipt, WorkerChangeRejection,
};
use behavior_actors::worker::{ImmediateStartup, StartupLimit, WorkerSubmission};
use behavior_actors::{Activate, Deadline, OwnedTimerElapsed, ReceiveTimeout, ReplyDelivery};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DocumentAddress(u64);

impl Address for DocumentAddress {
    type Nonce = u64;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocumentEndpoint(u64);

impl EndpointAddress for DocumentAddress {
    type Established<P>
        = DocumentEndpoint
    where
        P: Protocol<Addr = Self>;
    type Installed<B>
        = installed_control::InstalledControl<B, DocumentEndpoint>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> DocumentEndpoint
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Document {
    original: Box<[u8; 2]>,
}

#[behavior::behavior(addr = DocumentAddress, message = Never)]
impl Document {
    fn receive(&mut self, _: DocumentAddress, input: Never) -> BehaviorActed<Self> {
        match input {}
    }
}

type Lifecycle =
    MessageProtocol<DocumentAddress, DynamicLifecycle<Box<str>, Document, ImmediateStartup>>;
type Queries = MessageProtocol<DocumentAddress, QueryReply<Box<str>, Document>>;
type Replacements = MessageProtocol<
    DocumentAddress,
    Result<
        WorkerChangeReceipt<Box<str>>,
        WorkerChangeRejection<Box<str>, Document, ImmediateStartup, ReplaceRejection>,
    >,
>;

#[test]
fn ordinary_query_returns_the_original_key_to_the_exact_endpoint() {
    let lifecycle = EstablishedRecipient::<Lifecycle>::issued(DocumentEndpoint(11));
    let mut directory = directory::new(directory::Config {
        entries: EntryCapacity::new(1).expect("one retained document fits"),
        startup: StartupLimit::new(1).expect("one document may start"),
        exit: UnexpectedExit::KeepEmpty,
        shutdown: ActorDrainPolicy::WaitForActorGraph,
        lifecycle,
        diagnostics: DiagnosticDisposition::terminate(),
    })
    .initialize()
    .unwrap_or_else(|_| panic!("directory initialization is pure"))
    .behavior;
    let key = Box::<str>::from("contract");
    let original = key.as_ptr();
    let recipient = EstablishedRecipient::<Queries>::issued(DocumentEndpoint(13));
    let actions = directory
        .receive(
            DocumentAddress(17),
            DynamicCommand::Query {
                key,
                reply_to: recipient.clone(),
            },
        )
        .unwrap_or_else(|_| panic!("an unknown document has a complete query reply"));

    let replies: Vec<EstablishedDelivery<Queries>> = actions.sends.query_replies;
    let [EstablishedDelivery { to, message }] = replies
        .try_into()
        .unwrap_or_else(|_| panic!("exactly one reply was emitted"));
    assert_eq!(to, recipient);
    let QueryReply::Unknown { key } = message else {
        panic!("no document was installed");
    };
    assert_eq!(&*key, "contract");
    assert_eq!(key.as_ptr(), original);
    assert!(actions.sends.proxy_observations.is_empty());
    assert!(actions.sends.proxy_operations.is_empty());
    assert!(actions.sends.shutdown_schedules.is_empty());
    assert!(actions.sends.start_replies.as_slice().is_empty());
    assert!(actions.sends.replace_replies.as_slice().is_empty());
    assert!(actions.sends.stop_replies.as_slice().is_empty());
    assert!(actions.sends.cancel_replies.as_slice().is_empty());
    assert!(actions.sends.lifecycle.is_empty());
    assert!(actions.sends.diagnostics.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
}

#[test]
fn exact_replacement_refusal_returns_the_original_submission_and_key() {
    let lifecycle = EstablishedRecipient::<Lifecycle>::issued(DocumentEndpoint(19));
    let mut directory = directory::new(directory::Config {
        entries: EntryCapacity::new(1).expect("one retained document fits"),
        startup: StartupLimit::new(1).expect("one document may start"),
        exit: UnexpectedExit::KeepEmpty,
        shutdown: ActorDrainPolicy::WaitForActorGraph,
        lifecycle,
        diagnostics: DiagnosticDisposition::terminate(),
    })
    .initialize()
    .unwrap_or_else(|_| panic!("directory initialization is pure"))
    .behavior;
    let key = Box::<str>::from("ledger");
    let original_key = key.as_ptr();
    let document = Box::new([23, 29]);
    let recipient = EstablishedRecipient::<Replacements>::issued(DocumentEndpoint(31));
    let actions = directory
        .receive(
            DocumentAddress(37),
            DynamicCommand::Replace {
                key,
                submission: WorkerSubmission::immediate(Document { original: document }),
                reply_to: recipient.clone(),
            },
        )
        .unwrap_or_else(|_| panic!("an unknown key returns its complete replacement"));

    let replies: Vec<EstablishedDelivery<Replacements>> = actions.sends.replace_replies;
    let [EstablishedDelivery { to, message }] = replies
        .try_into()
        .unwrap_or_else(|_| panic!("exactly one refusal was emitted"));
    assert_eq!(to, recipient);
    let Err(WorkerChangeRejection {
        key,
        submission,
        reason,
    }) = message
    else {
        panic!("an absent document cannot be replaced");
    };
    assert_eq!(reason, ReplaceRejection::Unknown);
    assert_eq!(&*key, "ledger");
    assert_eq!(key.as_ptr(), original_key);
    let expected = WorkerSubmission::immediate(Document {
        original: Box::new([23, 29]),
    });
    assert_eq!(submission, expected);
    assert!(actions.sends.proxy_observations.is_empty());
    assert!(actions.sends.proxy_operations.is_empty());
    assert!(actions.sends.shutdown_schedules.is_empty());
    assert!(actions.sends.start_replies.as_slice().is_empty());
    assert!(actions.sends.stop_replies.as_slice().is_empty());
    assert!(actions.sends.query_replies.as_slice().is_empty());
    assert!(actions.sends.cancel_replies.as_slice().is_empty());
    assert!(actions.sends.lifecycle.is_empty());
    assert!(actions.sends.diagnostics.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
}

#[test]
fn explicit_logical_query_keeps_the_original_logical_destination() {
    let lifecycle = EstablishedRecipient::<Lifecycle>::issued(DocumentEndpoint(41));
    let mut directory = directory::new(directory::Config {
        entries: EntryCapacity::new(1).expect("one retained document fits"),
        startup: StartupLimit::new(1).expect("one document may start"),
        exit: UnexpectedExit::KeepEmpty,
        shutdown: ActorDrainPolicy::WaitForActorGraph,
        lifecycle,
        diagnostics: DiagnosticDisposition::terminate(),
    })
    .logical_replies()
    .initialize()
    .unwrap_or_else(|_| panic!("directory initialization is pure"))
    .behavior;
    let key = Box::<str>::from("journal");
    let original = key.as_ptr();
    let recipient = Recipient::<Queries>::global(DocumentAddress(43));
    let actions = directory
        .receive(
            DocumentAddress(47),
            DynamicCommand::Query {
                key,
                reply_to: recipient.clone().into(),
            },
        )
        .unwrap_or_else(|_| panic!("logical lookup retains the configured route"));

    let [reply] = <[_; 1]>::try_from(actions.sends.query_replies.into_deliveries())
        .unwrap_or_else(|_| panic!("exactly one logical reply was emitted"));
    let ReplyDelivery::Logical(reply) = reply else {
        panic!("logical opt-in does not rewrite the reply as an exact endpoint");
    };
    assert_eq!(reply.to, recipient);
    let QueryReply::Unknown { key } = reply.message else {
        panic!("no journal was installed");
    };
    assert_eq!(&*key, "journal");
    assert_eq!(key.as_ptr(), original);
    assert!(actions.sends.proxy_observations.is_empty());
    assert!(actions.sends.proxy_operations.is_empty());
    assert!(actions.sends.shutdown_schedules.is_empty());
    assert!(actions.sends.start_replies.as_slice().is_empty());
    assert!(actions.sends.replace_replies.as_slice().is_empty());
    assert!(actions.sends.stop_replies.as_slice().is_empty());
    assert!(actions.sends.cancel_replies.as_slice().is_empty());
    assert!(actions.sends.lifecycle.is_empty());
    assert!(actions.sends.diagnostics.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
}

#[test]
fn deadline_inside_keeps_exact_replies_and_its_terminal_reaction() {
    fn no_logical_hosts<B: LogicalHostRequirements<LogicalHosts = NoBirthProtocols>>(_: &B) {}

    let directory = directory::new(directory::Config {
        entries: EntryCapacity::new(1).expect("one document fits"),
        startup: StartupLimit::new(1).expect("one document may start"),
        exit: UnexpectedExit::KeepEmpty,
        shutdown: ActorDrainPolicy::WaitForActorGraph,
        lifecycle: EstablishedRecipient::<Lifecycle>::issued(DocumentEndpoint(43)),
        diagnostics: DiagnosticDisposition::terminate(),
    });
    let at = Instant::now();
    let delay = Duration::from_secs(5);
    let layered = ReceiveTimeout::new(
        Deadline::new(directory, Some(at), |_| {
            behavior::Step::Stop(behavior::Stopped)
        }),
        delay,
        |_| behavior::Actions::stop(),
    );
    no_logical_hosts(&layered);
    let initialized = layered
        .initialize()
        .unwrap_or_else(|_| panic!("both independent timers initialize"));
    let SendLayer {
        owned: inactivity,
        inner: SendLayer {
            owned: deadline,
            inner: management,
        },
    } = initialized.actions.sends;
    assert_eq!(deadline.as_slice().len(), 1);
    let deadline_request = deadline.as_slice()[0];
    assert_eq!(deadline_request.at, at);
    assert_eq!(inactivity.as_slice().len(), 1);
    let idle_request = inactivity.as_slice()[0];
    assert_eq!(idle_request.after, delay);
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.query_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(initialized.actions.creates.is_empty());
    assert!(matches!(
        initialized.actions.become_,
        behavior::Step::Continue
    ));
    let mut directory = initialized.behavior;
    let key = Box::<str>::from("contract");
    let original = key.as_ptr();
    let recipient = EstablishedRecipient::<Queries>::issued(DocumentEndpoint(45));
    let actions = directory
        .receive(
            DocumentAddress(47),
            DynamicCommand::Query {
                key,
                reply_to: recipient.clone(),
            },
        )
        .unwrap_or_else(|_| panic!("both wrappers preserve the query"));
    let SendLayer {
        owned: inactivity,
        inner: SendLayer {
            owned: deadline,
            inner: management,
        },
    } = actions.sends;
    assert!(deadline.as_slice().is_empty());
    assert_eq!(inactivity.as_slice().len(), 1);
    let rearmed = inactivity.as_slice()[0];
    assert_ne!(rearmed.generation, idle_request.generation);
    assert_eq!(rearmed.after, delay);
    let replies: Vec<EstablishedDelivery<Queries>> = management.query_replies;
    let [EstablishedDelivery { to, message }] = replies
        .try_into()
        .unwrap_or_else(|_| panic!("exactly one complete query reply survives both wrappers"));
    assert_eq!(to, recipient);
    let QueryReply::Unknown { key } = message else {
        panic!("no document was installed");
    };
    assert_eq!(&*key, "contract");
    assert_eq!(key.as_ptr(), original);
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
    let elapsed = OwnedTimerElapsed::new(deadline_request.generation);
    let stopped = directory
        .transition(behavior::EventLayer::Inner(behavior::EventLayer::Owned(
            elapsed,
        )))
        .unwrap_or_else(|_| panic!("the original inner timer selects its stop reaction"));
    let SendLayer {
        owned: inactivity,
        inner: SendLayer {
            owned: deadline,
            inner: management,
        },
    } = stopped.sends;
    assert!(deadline.as_slice().is_empty());
    assert!(inactivity.as_slice().is_empty());
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.query_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(
        stopped.become_,
        behavior::Step::Stop(behavior::Stopped)
    ));
}

#[test]
fn inactivity_inside_keeps_exact_replies_and_its_terminal_reaction() {
    fn no_logical_hosts<B: LogicalHostRequirements<LogicalHosts = NoBirthProtocols>>(_: &B) {}

    let directory = directory::new(directory::Config {
        entries: EntryCapacity::new(1).expect("one document fits"),
        startup: StartupLimit::new(1).expect("one document may start"),
        exit: UnexpectedExit::KeepEmpty,
        shutdown: ActorDrainPolicy::WaitForActorGraph,
        lifecycle: EstablishedRecipient::<Lifecycle>::issued(DocumentEndpoint(53)),
        diagnostics: DiagnosticDisposition::terminate(),
    });
    let at = Instant::now();
    let delay = Duration::from_secs(5);
    let layered = Deadline::new(
        ReceiveTimeout::new(directory, delay, |_| behavior::Actions::stop()),
        Some(at),
        |_| behavior::Step::Stop(behavior::Stopped),
    );
    no_logical_hosts(&layered);
    let initialized = layered
        .initialize()
        .unwrap_or_else(|_| panic!("both independent timers initialize"));
    let SendLayer {
        owned: deadline,
        inner: SendLayer {
            owned: inactivity,
            inner: management,
        },
    } = initialized.actions.sends;
    assert_eq!(deadline.as_slice().len(), 1);
    let deadline_request = deadline.as_slice()[0];
    assert_eq!(deadline_request.at, at);
    assert_eq!(inactivity.as_slice().len(), 1);
    let idle_request = inactivity.as_slice()[0];
    assert_eq!(idle_request.after, delay);
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.query_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(initialized.actions.creates.is_empty());
    assert!(matches!(
        initialized.actions.become_,
        behavior::Step::Continue
    ));
    let mut directory = initialized.behavior;
    let key = Box::<str>::from("contract");
    let original = key.as_ptr();
    let recipient = EstablishedRecipient::<Queries>::issued(DocumentEndpoint(55));
    let actions = directory
        .receive(
            DocumentAddress(57),
            DynamicCommand::Query {
                key,
                reply_to: recipient.clone(),
            },
        )
        .unwrap_or_else(|_| panic!("both wrappers preserve the query"));
    let SendLayer {
        owned: deadline,
        inner: SendLayer {
            owned: inactivity,
            inner: management,
        },
    } = actions.sends;
    assert!(deadline.as_slice().is_empty());
    assert_eq!(inactivity.as_slice().len(), 1);
    let rearmed = inactivity.as_slice()[0];
    assert_ne!(rearmed.generation, idle_request.generation);
    assert_eq!(rearmed.after, delay);
    let replies: Vec<EstablishedDelivery<Queries>> = management.query_replies;
    let [EstablishedDelivery { to, message }] = replies
        .try_into()
        .unwrap_or_else(|_| panic!("exactly one complete query reply survives both wrappers"));
    assert_eq!(to, recipient);
    let QueryReply::Unknown { key } = message else {
        panic!("no document was installed");
    };
    assert_eq!(&*key, "contract");
    assert_eq!(key.as_ptr(), original);
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
    let elapsed = OwnedTimerElapsed::new(rearmed.generation);
    let stopped = directory
        .transition(behavior::EventLayer::Inner(behavior::EventLayer::Owned(
            elapsed,
        )))
        .unwrap_or_else(|_| panic!("the original inner timer selects its stop reaction"));
    let SendLayer {
        owned: deadline,
        inner: SendLayer {
            owned: inactivity,
            inner: management,
        },
    } = stopped.sends;
    assert!(deadline.as_slice().is_empty());
    assert!(inactivity.as_slice().is_empty());
    assert!(management.proxy_observations.as_slice().is_empty());
    assert!(management.proxy_operations.is_empty());
    assert!(management.shutdown_schedules.is_empty());
    assert!(management.start_replies.as_slice().is_empty());
    assert!(management.replace_replies.as_slice().is_empty());
    assert!(management.stop_replies.as_slice().is_empty());
    assert!(management.query_replies.as_slice().is_empty());
    assert!(management.cancel_replies.as_slice().is_empty());
    assert!(management.lifecycle.as_slice().is_empty());
    assert!(management.diagnostics.as_slice().is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(
        stopped.become_,
        behavior::Step::Stop(behavior::Stopped)
    ));
}
