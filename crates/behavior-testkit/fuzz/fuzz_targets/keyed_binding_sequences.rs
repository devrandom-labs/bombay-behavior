#![no_main]
//! Arbitrary generation-exact KeyedPool binding management.

use std::collections::{BTreeSet, VecDeque};
use std::convert::Infallible;

use behavior::atomic::{
    ActivationPolicy, ActorDrainPolicy, Assignment, BacklogCapacity, BindingCapacity,
    BindingEvidence, BindingExpectation, BindingGeneration, BindingRejection, BindingReply,
    BindingRequestId, Completion, DiagnosticDisposition, ImmediateActivation, Interruption,
    KeyedCommand, KeyedPool, OrderedRoles, PoolFailureReaction, PoolRecovery, WorkerSubmission,
    keyed,
};
use behavior::{
    Actions, Activate, Active, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase,
    Creations, Delivery, EndpointAddress, EstablishedDelivery, InterpreterRequests,
    MessageProtocol, Never, NoBirths, Protocol, Recipient, ReplyDelivery, ReportToParent,
    StopOnShutdown, User,
};
use libfuzzer_sys::fuzz_target;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddress(u64);

impl Address for RuntimeAddress {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = WorkerEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
enum Role {
    Primary,
    Replica,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Account(u8);

struct SearchWorker;

type BindingPool = Active<
    KeyedPool<
        Role,
        SearchWorker,
        ImmediateActivation,
        Never,
        fn(&Account) -> Role,
        Infallible,
        Account,
        u8,
        u16,
    >,
>;
type BindingReplyDelivery = ReplyDelivery<
    Delivery<MessageProtocol<RuntimeAddress, BindingReply<RuntimeAddress, Account, Role>>>,
    EstablishedDelivery<
        MessageProtocol<RuntimeAddress, BindingReply<RuntimeAddress, Account, Role>>,
    >,
>;

impl Protocol for SearchWorker {
    type Addr = RuntimeAddress;
    type Msg = Assignment<u8>;
}

impl BehaviorBase for SearchWorker {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for SearchWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, Assignment<u8>>;
    type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn prepare_worker(_: &Role) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}

fn selected_role(_: &Account) -> Role {
    Role::Primary
}

fn pool() -> (
    BindingPool,
    Creations<behavior::CreateChild<RuntimeAddress, StopOnShutdown<SearchWorker>>>,
) {
    let pool = keyed(
        prepare_worker,
        OrderedRoles::new(Role::Primary, [Role::Replica]).expect("roles are unique"),
        selected_role as fn(&Account) -> Role,
        ActivationPolicy::new(2).expect("two activations are valid"),
        PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).expect("one binding is valid"),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|_| panic!("keyed initialization is pure"));
    (initialized.behavior, initialized.actions.creates)
}

enum CurrentBinding {
    Absent,
    Bound(BindingEvidence<Role>),
}

#[derive(Clone, Copy)]
enum Input {
    BindPrimary,
    BindReplica,
    Unbind,
    ForeignExpectation,
    StaleExpectation,
}

impl Input {
    const fn from_byte(byte: u8) -> Self {
        match byte % 5 {
            0 => Self::BindPrimary,
            1 => Self::BindReplica,
            2 => Self::Unbind,
            3 => Self::ForeignExpectation,
            _ => Self::StaleExpectation,
        }
    }
}

struct Scenario {
    pool: BindingPool,
    _pending_workers:
        Creations<behavior::CreateChild<RuntimeAddress, StopOnShutdown<SearchWorker>>>,
    current: CurrentBinding,
    foreign: BindingGeneration,
    stale: VecDeque<BindingGeneration>,
    issued: BTreeSet<u64>,
    last_issued: Option<u64>,
    next_request: u64,
}

impl Scenario {
    fn new() -> Self {
        let (mut foreign_pool, foreign_workers) = pool();
        let foreign = foreign_pool
            .receive(
                RuntimeAddress(7),
                KeyedCommand::rebalance(
                    BindingRequestId::new(1),
                    Account(2),
                    BindingExpectation::Absent,
                    Role::Primary,
                    Recipient::<
                        MessageProtocol<
                            RuntimeAddress,
                            BindingReply<RuntimeAddress, Account, Role>,
                        >,
                    >::global(RuntimeAddress(89)),
                ),
            )
            .unwrap_or_else(|_| panic!("foreign pool accepts one binding"))
            .sends
            .binding_replies
            .into_deliveries()
            .pop()
            .map(logical_reply)
            .map(|reply| match reply {
                BindingReply::Bound { current, .. } => current.generation().clone(),
                _ => panic!("foreign absent key becomes bound"),
            })
            .expect("foreign binding emits one reply");
        drop(foreign_workers);
        drop(foreign_pool);
        let (pool, pending_workers) = pool();
        Self {
            pool,
            _pending_workers: pending_workers,
            current: CurrentBinding::Absent,
            foreign,
            stale: VecDeque::new(),
            issued: BTreeSet::new(),
            last_issued: None,
            next_request: 1,
        }
    }

    fn apply(mut self, input: Input) -> Self {
        let route = Recipient::<
            MessageProtocol<RuntimeAddress, BindingReply<RuntimeAddress, Account, Role>>,
        >::global(RuntimeAddress(89));
        let expectation = match input {
            Input::ForeignExpectation => BindingExpectation::Exact(self.foreign.clone()),
            Input::StaleExpectation => self
                .stale
                .pop_front()
                .map(BindingExpectation::Exact)
                .unwrap_or_else(|| BindingExpectation::Exact(self.foreign.clone())),
            Input::BindPrimary | Input::BindReplica | Input::Unbind => match &self.current {
                CurrentBinding::Absent => BindingExpectation::Absent,
                CurrentBinding::Bound(current) => {
                    BindingExpectation::Exact(current.generation().clone())
                }
            },
        };
        let request = BindingRequestId::new(self.next_request);
        self.next_request += 1;
        let command = match input {
            Input::BindPrimary | Input::ForeignExpectation | Input::StaleExpectation => {
                KeyedCommand::rebalance(request, Account(2), expectation, Role::Primary, route)
            }
            Input::BindReplica => {
                KeyedCommand::rebalance(request, Account(2), expectation, Role::Replica, route)
            }
            Input::Unbind => KeyedCommand::unbind(request, Account(2), expectation, route),
        };
        let acted = self
            .pool
            .receive(RuntimeAddress(7), command)
            .unwrap_or_else(|_| panic!("binding command is a total keyed transition"));
        assert!(acted.creates.is_empty());
        assert!(acted.sends.customer_outcomes.is_empty());
        assert!(acted.sends.worker_assignments.is_empty());
        assert!(acted.sends.worker_preparations.is_empty());
        assert!(acted.sends.restart_schedules.is_empty());
        assert!(acted.sends.worker_shutdowns.is_empty());
        assert!(acted.sends.diagnostics.is_empty());
        let reply = acted
            .sends
            .binding_replies
            .into_deliveries()
            .pop()
            .map(logical_reply)
            .expect("one command emits one binding reply");
        self.accept(reply);
        self
    }

    fn accept(&mut self, reply: BindingReply<RuntimeAddress, Account, Role>) {
        let current = core::mem::replace(&mut self.current, CurrentBinding::Absent);
        self.current = match (current, reply) {
            (CurrentBinding::Absent, BindingReply::Bound { current, .. }) => {
                self.accept_generation(&current);
                CurrentBinding::Bound(current)
            }
            (CurrentBinding::Bound(prior), BindingReply::Unchanged { current, .. }) => {
                assert_eq!(current.role(), prior.role());
                assert_eq!(current.generation(), prior.generation());
                CurrentBinding::Bound(current)
            }
            (
                CurrentBinding::Bound(prior),
                BindingReply::Rebalanced {
                    prior: returned,
                    current,
                    ..
                },
            ) => {
                assert_eq!(returned.role(), prior.role());
                assert_eq!(returned.generation(), prior.generation());
                assert_ne!(current.role(), prior.role());
                self.stale.push_back(returned.generation().clone());
                self.accept_generation(&current);
                CurrentBinding::Bound(current)
            }
            (CurrentBinding::Bound(prior), BindingReply::Unbound { key, removed, .. }) => {
                assert_eq!(key, Account(2));
                assert_eq!(removed.role(), prior.role());
                assert_eq!(removed.generation(), prior.generation());
                self.stale.push_back(removed.generation().clone());
                CurrentBinding::Absent
            }
            (CurrentBinding::Absent, BindingReply::AlreadyUnbound { command }) => {
                assert_eq!(command.key(), &Account(2));
                assert_eq!(command.expectation(), &BindingExpectation::Absent);
                CurrentBinding::Absent
            }
            (state, BindingReply::Rejected { command, reason }) => {
                assert_eq!(command.key(), &Account(2));
                let BindingRejection::StaleExpectation { actual } = reason else {
                    panic!("the open one-key table rejects only stale expectations")
                };
                match (&state, actual) {
                    (CurrentBinding::Absent, BindingExpectation::Absent) => {}
                    (CurrentBinding::Bound(current), BindingExpectation::Exact(actual)) => {
                        assert_eq!(&actual, current.generation());
                    }
                    _ => panic!("stale rejection returns the current binding"),
                }
                state
            }
            _ => panic!("binding reply contradicts current ownership"),
        };
    }

    fn accept_generation(&mut self, binding: &BindingEvidence<Role>) {
        let ordinal = binding.generation().get();
        assert!(self.issued.insert(ordinal));
        if let Some(previous) = self.last_issued {
            assert!(ordinal > previous);
        }
        self.last_issued = Some(ordinal);
    }
}

fn logical_reply(reply: BindingReplyDelivery) -> BindingReply<RuntimeAddress, Account, Role> {
    match reply {
        ReplyDelivery::Logical(delivery) => {
            assert_eq!(delivery.to.address(), RuntimeAddress(89));
            delivery.message
        }
        ReplyDelivery::Established(_) => panic!("management route changed"),
    }
}

fn exercise(bytes: &[u8]) {
    let mut scenario = Scenario::new();
    for byte in bytes.iter().copied().take(256) {
        scenario = scenario.apply(Input::from_byte(byte));
    }
}

fuzz_target!(|bytes: &[u8]| {
    exercise(bytes);
});
