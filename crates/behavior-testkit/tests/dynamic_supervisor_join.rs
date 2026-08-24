//! Public-contract tests for joining dynamic-supervisor creation facts.
//!
//! The oracle in this file is intentionally phrased in terms of facts a
//! runtime and a customer can observe. It does not mirror the supervisor's
//! internal phases.

use behavior::{
    Activate as _, Active, ChildStopped, CreationResolved, DynamicSupervisor,
    DynamicSupervisorError, DynamicSupervisorMessage, DynamicSupervisorOutcome, Exit,
    ShutdownRequested, WorkerCreationResolved,
};
use foundation::{
    Behavior, BehaviorActed, CreationKind, CreationRejection, MailAddr, Never, NoBirths, Protocol,
    Recipient, Step, User,
};
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
struct Worker;

impl Protocol for Worker {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Replies;

impl Protocol for Replies {
    type Addr = MailAddr;
    type Msg = DynamicSupervisorOutcome<MailAddr, Worker>;
}

type Subject = DynamicSupervisor<MailAddr, Worker, Recipient<Replies>>;

fn installing() -> Active<Subject> {
    let mut active = Subject::new().initialize().unwrap().behavior;
    let accepted = active
        .receive(
            MailAddr(1),
            DynamicSupervisorMessage::Start {
                nonce: 7,
                child: Worker,
                reply_to: Recipient::global(MailAddr(99)),
            },
        )
        .unwrap();

    assert_eq!(accepted.creates.len(), 1);
    assert_eq!(accepted.sends.outcomes.len(), 1);
    assert!(matches!(
        accepted.sends.outcomes[0].message,
        DynamicSupervisorOutcome::StartAccepted { nonce: 7 }
    ));
    active
}

#[derive(Clone, Copy, Debug)]
enum ArrivalOrder {
    ProxyThenWorker,
    WorkerThenProxy,
}

#[test]
fn successful_creation_facts_form_an_order_independent_exactly_once_join() {
    for order in [ArrivalOrder::ProxyThenWorker, ArrivalOrder::WorkerThenProxy] {
        let mut active = installing();
        let proxy = CreationResolved::birth(7, MailAddr(70));
        let worker = WorkerCreationResolved::new(7, 0, CreationKind::Birth, Ok(()));

        let (first, second) = match order {
            ArrivalOrder::ProxyThenWorker => (
                active.on_path(proxy).unwrap(),
                active.on_path(worker).unwrap(),
            ),
            ArrivalOrder::WorkerThenProxy => (
                active.on_path(worker).unwrap(),
                active.on_path(proxy).unwrap(),
            ),
        };

        assert!(
            first.sends.outcomes.is_empty(),
            "the first fact cannot establish readiness in {order:?}"
        );
        assert_eq!(
            second.sends.outcomes.len(),
            1,
            "the matching second fact emits one outcome in {order:?}"
        );
        assert!(matches!(
            second.sends.outcomes[0].message,
            DynamicSupervisorOutcome::Started { nonce: 7, child }
                if child.address() == MailAddr(70)
        ));

        let duplicate_proxy = active.on_path(proxy);
        assert!(matches!(
            duplicate_proxy,
            Err(DynamicSupervisorError::UnexpectedCreation(returned)) if returned == proxy
        ));
        let duplicate_worker = active.on_path(worker);
        assert!(matches!(
            duplicate_worker,
            Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned)) if returned == worker
        ));
    }
}

#[test]
fn rejected_worker_creation_joins_in_either_order_and_returns_one_failure() {
    for order in [ArrivalOrder::ProxyThenWorker, ArrivalOrder::WorkerThenProxy] {
        let mut active = installing();
        let proxy = CreationResolved::birth(7, MailAddr(70));
        let worker = WorkerCreationResolved::new(
            7,
            0,
            CreationKind::Birth,
            Err(CreationRejection::EnvironmentFailed),
        );

        let (first, second) = match order {
            ArrivalOrder::ProxyThenWorker => (
                active.on_path(proxy).unwrap(),
                active.on_path(worker).unwrap(),
            ),
            ArrivalOrder::WorkerThenProxy => (
                active.on_path(worker).unwrap(),
                active.on_path(proxy).unwrap(),
            ),
        };

        assert!(first.sends.outcomes.is_empty());
        assert!(first.sends.shutdowns.is_empty());
        assert_eq!(second.sends.outcomes.len(), 1);
        assert!(matches!(
            second.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 7,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));
        assert_eq!(second.sends.shutdowns.len(), 1);
        assert_eq!(second.sends.shutdowns[0].nonce, 7);
    }
}

#[test]
fn malformed_or_duplicate_first_facts_are_returned_without_poisoning_the_join() {
    let mut active = installing();

    let foreign_proxy = CreationResolved::birth(8, MailAddr(80));
    assert!(matches!(
        active.on_path(foreign_proxy),
        Err(DynamicSupervisorError::UnexpectedCreation(returned)) if returned == foreign_proxy
    ));
    let foreign_worker = WorkerCreationResolved::new(8, 0, CreationKind::Birth, Ok(()));
    assert!(matches!(
        active.on_path(foreign_worker),
        Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
            if returned == foreign_worker
    ));
    let wrong_proxy_kind =
        CreationResolved::installed(7, CreationKind::replacement_of(6), MailAddr(70));
    assert!(matches!(
        active.on_path(wrong_proxy_kind),
        Err(DynamicSupervisorError::UnexpectedCreation(returned)) if returned == wrong_proxy_kind
    ));

    let worker = WorkerCreationResolved::new(7, 0, CreationKind::Birth, Ok(()));
    let first = active.on_path(worker).unwrap();
    assert!(first.sends.outcomes.is_empty());
    assert!(matches!(
        active.on_path(worker),
        Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned)) if returned == worker
    ));

    let wrong_worker_kind =
        WorkerCreationResolved::new(7, 0, CreationKind::replacement_of(6), Ok(()));
    assert!(matches!(
        active.on_path(wrong_worker_kind),
        Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
            if returned == wrong_worker_kind
    ));

    let joined = active
        .on_path(CreationResolved::birth(7, MailAddr(70)))
        .unwrap();
    assert_eq!(joined.sends.outcomes.len(), 1);
    assert!(matches!(
        joined.sends.outcomes[0].message,
        DynamicSupervisorOutcome::Started { nonce: 7, .. }
    ));
}

#[test]
fn contradictory_authoritative_results_return_both_facts_without_consuming_the_join() {
    let mut active = installing();
    let worker = WorkerCreationResolved::new(7, 0, CreationKind::Birth, Ok(()));
    active.on_path(worker).unwrap();

    let rejected_proxy = CreationResolved::rejected(
        7,
        CreationKind::Birth,
        CreationRejection::InitializationFailed,
    );
    assert!(matches!(
        active.on_path(rejected_proxy),
        Err(DynamicSupervisorError::ContradictoryInitialCreation {
            proxy,
            worker: returned_worker,
        }) if proxy == rejected_proxy && returned_worker == worker
    ));

    assert!(matches!(
        active.on_path(worker),
        Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned)) if returned == worker
    ));
    let completed = active
        .on_path(CreationResolved::birth(7, MailAddr(70)))
        .unwrap();
    assert_eq!(completed.sends.outcomes.len(), 1);
    assert!(matches!(
        completed.sends.outcomes[0].message,
        DynamicSupervisorOutcome::Started { nonce: 7, .. }
    ));
}

#[derive(Clone, Copy, Debug)]
enum ShutdownPoint {
    BeforeEitherFact,
    AfterProxyFact,
    AfterWorkerFact,
    AfterBothFacts,
}

#[test]
fn shutdown_is_total_at_every_successful_join_boundary() {
    for point in [
        ShutdownPoint::BeforeEitherFact,
        ShutdownPoint::AfterProxyFact,
        ShutdownPoint::AfterWorkerFact,
        ShutdownPoint::AfterBothFacts,
    ] {
        let mut active = installing();
        let proxy = CreationResolved::birth(7, MailAddr(70));
        let worker = WorkerCreationResolved::new(7, 0, CreationKind::Birth, Ok(()));
        let mut started = 0;
        let mut shutdowns = 0;

        match point {
            ShutdownPoint::BeforeEitherFact => {}
            ShutdownPoint::AfterProxyFact => {
                let actions = active.on_path(proxy).unwrap();
                started += actions.sends.outcomes.len();
                shutdowns += actions.sends.shutdowns.len();
            }
            ShutdownPoint::AfterWorkerFact => {
                let actions = active.on_path(worker).unwrap();
                started += actions.sends.outcomes.len();
                shutdowns += actions.sends.shutdowns.len();
            }
            ShutdownPoint::AfterBothFacts => {
                let first = active.on_path(worker).unwrap();
                let second = active.on_path(proxy).unwrap();
                started += first.sends.outcomes.len() + second.sends.outcomes.len();
                shutdowns += first.sends.shutdowns.len() + second.sends.shutdowns.len();
            }
        }

        let requested = active.on_path(ShutdownRequested).unwrap();
        started += requested.sends.outcomes.len();
        shutdowns += requested.sends.shutdowns.len();
        assert!(matches!(requested.become_, Step::Continue));

        match point {
            ShutdownPoint::BeforeEitherFact => {
                let first = active.on_path(worker).unwrap();
                let second = active.on_path(proxy).unwrap();
                started += first.sends.outcomes.len() + second.sends.outcomes.len();
                shutdowns += first.sends.shutdowns.len() + second.sends.shutdowns.len();
            }
            ShutdownPoint::AfterProxyFact => {
                let actions = active.on_path(worker).unwrap();
                started += actions.sends.outcomes.len();
                shutdowns += actions.sends.shutdowns.len();
            }
            ShutdownPoint::AfterWorkerFact => {
                let actions = active.on_path(proxy).unwrap();
                started += actions.sends.outcomes.len();
                shutdowns += actions.sends.shutdowns.len();
            }
            ShutdownPoint::AfterBothFacts => {}
        }

        assert_eq!(started, 1, "one Started outcome at {point:?}");
        assert_eq!(shutdowns, 1, "one proxy shutdown request at {point:?}");
        let stopped = active
            .on_path(ChildStopped::new(7, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        assert!(matches!(stopped.become_, Step::Stop(_)));
    }
}

#[test]
fn shutdown_is_total_with_a_worker_rejection_on_either_side_of_the_join() {
    for order in [ArrivalOrder::ProxyThenWorker, ArrivalOrder::WorkerThenProxy] {
        let mut active = installing();
        let proxy = CreationResolved::birth(7, MailAddr(70));
        let worker = WorkerCreationResolved::new(
            7,
            0,
            CreationKind::Birth,
            Err(CreationRejection::EnvironmentFailed),
        );

        let first = match order {
            ArrivalOrder::ProxyThenWorker => active.on_path(proxy).unwrap(),
            ArrivalOrder::WorkerThenProxy => active.on_path(worker).unwrap(),
        };
        assert!(first.sends.outcomes.is_empty());

        let requested = active.on_path(ShutdownRequested).unwrap();
        assert!(matches!(requested.become_, Step::Continue));

        let second = match order {
            ArrivalOrder::ProxyThenWorker => active.on_path(worker).unwrap(),
            ArrivalOrder::WorkerThenProxy => active.on_path(proxy).unwrap(),
        };
        assert_eq!(second.sends.outcomes.len(), 1);
        assert!(matches!(
            second.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 7,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));
        assert_eq!(
            first.sends.shutdowns.len()
                + requested.sends.shutdowns.len()
                + second.sends.shutdowns.len(),
            1,
            "one proxy shutdown request at {order:?}"
        );

        let stopped = active
            .on_path(ChildStopped::new(7, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        assert!(matches!(stopped.become_, Step::Stop(_)));
    }
}
