use behavior::{Recipient, initialize};
use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::pool::{BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery};
use behavior_actors::pool::{queue, router};
use behavior_actors::worker::{OrderedRoles, StartupLimit, WorkerSubmission};

#[path = "pool/address.rs"]
mod address;
#[path = "pool/clerk.rs"]
mod clerk;
use address::RuntimeAddr;
use clerk::Clerk;

#[derive(Debug, Eq, PartialEq)]
enum Role {
    Clerk,
}

#[test]
fn fifo_accepts_named_completion_with_business_sends_without_structural_types() {
    let mut pool = queue::new(
        |_: &Role| {
            WorkerSubmission::immediate(Clerk {
                audits: Recipient::global(RuntimeAddr(17)),
            })
        },
        OrderedRoles::new(Role::Clerk, []).expect("one clerk"),
        queue::Config {
            startup: StartupLimit::new(1).expect("one clerk startup"),
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(2),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::terminate(),
        },
    )
    .build()
    .unwrap_or_else(|_| panic!("the ordinary clerk is prepared"));
    let actions =
        initialize(&mut pool).unwrap_or_else(|_| panic!("the pool creates its actual worker"));
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.sends.worker_observations.len(), 1);
}

#[test]
fn keyed_accepts_the_same_worker_and_infers_its_completion_result() {
    let mut pool = router::new(
        |_: &Role| {
            WorkerSubmission::immediate(Clerk {
                audits: Recipient::global(RuntimeAddr(17)),
            })
        },
        OrderedRoles::new(Role::Clerk, []).expect("one clerk"),
        |_: &u64| Role::Clerk,
        router::Config {
            startup: StartupLimit::new(1).expect("one clerk startup"),
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(2),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::terminate(),
            bindings: router::BindingCapacity::new(2).expect("two bindings"),
        },
    )
    .unwrap_or_else(|_| panic!("the ordinary clerk is prepared"));
    let actions = initialize(&mut pool)
        .unwrap_or_else(|_| panic!("the keyed pool creates its actual worker"));
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.sends.worker_observations.len(), 1);
}

#[test]
fn missing_or_ambiguous_completion_lanes_are_rejected() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/pool_missing_completion.rs");
    cases.compile_fail("tests/ui/pool_ambiguous_completion.rs");
    cases.compile_fail("tests/ui/router_missing_completion.rs");
    cases.compile_fail("tests/ui/router_ambiguous_completion.rs");
}
