//! Proven construction orders spanning existing behavior templates.

use behavior::{Address, Behavior};

use crate::{
    Backoff, BackoffSupervisor, BackoffSupervisorWithParent, ChildTopology, FleetError,
    ProxyParentIngress, RestartConfiguration, Supervisor, SupervisorWithParent, TimerId,
};

/// Construct fixed-fleet supervision inside generation-safe restart backoff.
///
/// This is a derived Bombay construction policy, not an actor-model law. The
/// topology and restart configuration govern only [`Supervisor`]; the checked
/// backoff policy and nonce-to-timer selector govern only
/// [`BackoffSupervisor`]. No input is inferred or redirected to another lane.
///
/// # Errors
///
/// Returns the exact [`FleetError`] produced by [`Supervisor::new`] when the
/// supplied topology cannot establish a fresh, unambiguous fixed fleet.
pub fn supervised_backoff<A, C>(
    topology: ChildTopology<A::Nonce, C>,
    restart: RestartConfiguration,
    backoff: Backoff,
    timer: fn(A::Nonce) -> TimerId,
) -> Result<BackoffSupervisor<A, C>, FleetError<A::Nonce>>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    Supervisor::new(topology, restart)
        .map(|supervisor| BackoffSupervisor::new(supervisor, backoff, timer))
}

/// Construct fixed-fleet supervision with an explicit final proxy-report
/// ingress before applying generation-safe restart backoff.
///
/// `ParentPath` is authored against the complete behavior event algebra that
/// will host the supervisor. Wrapping the result later does not rewrite this
/// ancestor acquaintance; callers lift [`ProxyParentIngress`] once for every
/// outer structural event layer.
pub fn supervised_backoff_with_parent<A, C, ParentPath>(
    topology: ChildTopology<A::Nonce, C>,
    restart: RestartConfiguration,
    backoff: Backoff,
    timer: fn(A::Nonce) -> TimerId,
    parent: ProxyParentIngress<A, ParentPath>,
) -> Result<BackoffSupervisorWithParent<A, C, ParentPath>, FleetError<A::Nonce>>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    SupervisorWithParent::with_parent(topology, restart, parent)
        .map(|supervisor| BackoffSupervisorWithParent::new(supervisor, backoff, timer))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use behavior::{Actions, MailAddr, Never, NoBirths, User};

    use super::*;
    use crate::{
        Activate as _, BackoffSupervisorSends, Crash, CreationKind, CreationResolved, Proxy,
        RestartPolicy, Strategy, TimerElapsed, TimerGeneration, WorkerCreationResolved,
        WorkerStopped,
    };

    struct Child;

    impl behavior::Protocol for Child {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }

    fn restart() -> RestartConfiguration {
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            2,
            Duration::from_secs(30),
        )
    }

    fn timer(nonce: u64) -> TimerId {
        TimerId(nonce + 40)
    }

    fn policy() -> Backoff {
        Backoff::linear(Duration::from_secs(3), Duration::from_secs(9)).unwrap()
    }

    type Subject = BackoffSupervisor<MailAddr, Child>;

    fn assert_actions_equal(
        recipe: &Actions<
            MailAddr,
            Never,
            BackoffSupervisorSends<MailAddr, Child>,
            behavior::Births<Proxy<Child>>,
        >,
        manual: &Actions<
            MailAddr,
            Never,
            BackoffSupervisorSends<MailAddr, Child>,
            behavior::Births<Proxy<Child>>,
        >,
    ) {
        assert_eq!(recipe.creates.len(), manual.creates.len());
        for (recipe, manual) in recipe.creates.iter().zip(&manual.creates) {
            assert_eq!(recipe.nonce, manual.nonce);
            assert_eq!(recipe.kind, manual.kind);
        }
        assert!(recipe.sends.schedules.as_slice() == manual.sends.schedules.as_slice());
        assert!(
            recipe.sends.supervision.child_observations.as_slice()
                == manual.sends.supervision.child_observations.as_slice()
        );
        assert!(
            recipe.sends.supervision.creation_observations.as_slice()
                == manual.sends.supervision.creation_observations.as_slice()
        );
        assert_eq!(
            recipe.sends.supervision.replacement_commands.len(),
            manual.sends.supervision.replacement_commands.len()
        );
        for (recipe, manual) in recipe
            .sends
            .supervision
            .replacement_commands
            .iter()
            .zip(&manual.sends.supervision.replacement_commands)
        {
            assert_eq!(recipe.nonce, manual.nonce);
            assert!(matches!(recipe.message, crate::ProxyCommand::Replace(_)));
            assert!(matches!(manual.message, crate::ProxyCommand::Replace(_)));
        }
        assert_eq!(
            recipe.sends.supervision.failure_reports.len(),
            manual.sends.supervision.failure_reports.len()
        );
        for (recipe, manual) in recipe
            .sends
            .supervision
            .failure_reports
            .iter()
            .zip(&manual.sends.supervision.failure_reports)
        {
            assert!(recipe.failure == manual.failure);
        }
        assert!(
            recipe.sends.supervision.shutdowns.as_slice()
                == manual.sends.supervision.shutdowns.as_slice()
        );
        assert_eq!(
            matches!(recipe.become_, behavior::Step::Stop(_)),
            matches!(manual.become_, behavior::Step::Stop(_))
        );
    }

    fn assert_turn_equal(
        recipe: crate::BehaviorActed<Subject>,
        manual: crate::BehaviorActed<Subject>,
    ) {
        match (recipe, manual) {
            (Ok(recipe), Ok(manual)) => assert_actions_equal(&recipe, &manual),
            (Err(recipe), Err(manual)) => assert_eq!(recipe, manual),
            (Ok(_), Err(error)) => panic!("manual stack alone rejected the turn: {error:?}"),
            (Err(error), Ok(_)) => panic!("recipe stack alone rejected the turn: {error:?}"),
        }
    }

    #[test]
    fn exact_type_and_topology_error_are_preserved() {
        fn exact(_: BackoffSupervisor<MailAddr, Child>) {}

        exact(
            supervised_backoff(ChildTopology::new([1], child), restart(), policy(), timer).unwrap(),
        );
        assert!(matches!(
            supervised_backoff(
                ChildTopology::new([7, 7], child),
                restart(),
                policy(),
                timer,
            ),
            Err(FleetError::DuplicateChild(7))
        ));
    }

    #[test]
    fn recipe_and_manual_stack_have_identical_complete_backoff_trace() {
        let recipe = supervised_backoff(
            ChildTopology::new([1, 2], child),
            restart(),
            policy(),
            timer,
        )
        .unwrap()
        .initialize()
        .unwrap();
        let manual = BackoffSupervisor::new(
            Supervisor::new(ChildTopology::new([1, 2], child), restart()).unwrap(),
            policy(),
            timer,
        )
        .initialize()
        .unwrap();

        assert_actions_equal(&recipe.actions, &manual.actions);
        let mut recipe = recipe.behavior;
        let mut manual = manual.behavior;

        assert_turn_equal(
            recipe.on_path(CreationResolved::birth(1, MailAddr(21))),
            manual.on_path(CreationResolved::birth(1, MailAddr(21))),
        );

        let first_failure = Instant::now();
        assert_turn_equal(
            recipe.on_path(WorkerStopped::new(
                1,
                101,
                Err(Crash::Panicked),
                first_failure,
            )),
            manual.on_path(WorkerStopped::new(
                1,
                101,
                Err(Crash::Panicked),
                first_failure,
            )),
        );
        assert_eq!(recipe.pending_restarts(), manual.pending_restarts());
        assert_turn_equal(
            recipe.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(9))),
            manual.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(9))),
        );
        assert_eq!(recipe.pending_restarts(), manual.pending_restarts());
        assert_turn_equal(
            recipe.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(0))),
            manual.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(0))),
        );
        assert_eq!(recipe.pending_restarts(), manual.pending_restarts());
        assert_turn_equal(
            recipe.on_path(WorkerCreationResolved::new(
                1,
                201,
                CreationKind::replacement_of(101),
                Ok(()),
            )),
            manual.on_path(WorkerCreationResolved::new(
                1,
                201,
                CreationKind::replacement_of(101),
                Ok(()),
            )),
        );

        let second_failure = first_failure + Duration::from_secs(1);
        assert_turn_equal(
            recipe.on_path(WorkerStopped::new(
                1,
                201,
                Err(Crash::Failed),
                second_failure,
            )),
            manual.on_path(WorkerStopped::new(
                1,
                201,
                Err(Crash::Failed),
                second_failure,
            )),
        );
        assert_turn_equal(
            recipe.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(1))),
            manual.on_path(TimerElapsed::new(TimerId(41), TimerGeneration(1))),
        );
        assert_turn_equal(
            recipe.on_path(WorkerCreationResolved::new(
                1,
                301,
                CreationKind::replacement_of(201),
                Ok(()),
            )),
            manual.on_path(WorkerCreationResolved::new(
                1,
                301,
                CreationKind::replacement_of(201),
                Ok(()),
            )),
        );

        let exhausted = first_failure + Duration::from_secs(2);
        assert_turn_equal(
            recipe.on_path(WorkerStopped::new(1, 301, Err(Crash::Cancelled), exhausted)),
            manual.on_path(WorkerStopped::new(1, 301, Err(Crash::Cancelled), exhausted)),
        );
        assert_eq!(recipe.pending_restarts(), manual.pending_restarts());
    }
}
