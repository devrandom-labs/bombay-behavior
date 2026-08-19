//! Proven construction orders spanning existing behavior templates.

use behavior::{Address, Behavior};

use crate::{
    Backoff, BackoffSupervisor, ChildTopology, FleetError, RestartConfiguration, Supervisor,
    TimerId,
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use behavior::{Actions, MailAddr, Never, NoBirths, User};

    use super::*;
    use crate::{
        Activate as _, Crash, RestartPolicy, ScheduleAfter, Strategy, TimerGeneration,
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
    fn recipe_and_manual_stack_have_identical_initialization_and_backoff_trace() {
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

        assert_eq!(recipe.actions.creates.len(), manual.actions.creates.len());
        assert_eq!(
            recipe.actions.sends.supervision.creation_observations.len(),
            manual.actions.sends.supervision.creation_observations.len()
        );
        assert_eq!(
            recipe.actions.sends.supervision.child_observations.len(),
            manual.actions.sends.supervision.child_observations.len()
        );
        assert!(recipe.actions.sends.schedules.is_empty());
        assert!(manual.actions.sends.schedules.is_empty());

        let stopped_at = Instant::now();
        let mut recipe = recipe.behavior;
        let mut manual = manual.behavior;
        let recipe_actions = recipe
            .on_path(WorkerStopped::new(1, 101, Err(Crash::Panicked), stopped_at))
            .unwrap();
        let manual_actions = manual
            .on_path(WorkerStopped::new(1, 101, Err(Crash::Panicked), stopped_at))
            .unwrap();
        let expected = ScheduleAfter::new(TimerId(41), TimerGeneration(0), Duration::from_secs(3));
        assert_eq!(recipe_actions.sends.schedules.as_slice(), [expected]);
        assert_eq!(manual_actions.sends.schedules.as_slice(), [expected]);
        assert_eq!(recipe.pending_restarts(), manual.pending_restarts());
    }
}
