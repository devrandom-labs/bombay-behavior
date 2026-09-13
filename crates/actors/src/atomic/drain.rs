//! Actor-graph retirement policy shared by atomic actor owners.

use core::time::Duration;

use behavior::{ActionItemResult, ItemSettlement, SettledItem};

use crate::{ScheduleAfter, TimerElapsed, TimerGeneration, TimerId};

use super::schedule::ScheduleKey;

/// Actor-graph retirement policy after aggregate shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorDrainPolicy {
    /// Wait until every owned actor has retired.
    WaitForActorGraph,
    /// Force-transfer unresolved actor ownership after this duration.
    RetireActorGraphAfter {
        /// Duration from drain start to forced actor retirement.
        deadline: Duration,
    },
}

pub(super) enum ShutdownDeadline {
    Unlimited,
    Scheduling(ScheduleKey),
    Waiting(ScheduleKey),
    NotScheduled(ActionItemResult<ScheduleAfter>),
    Elapsed(TimerElapsed),
}

impl ShutdownDeadline {
    const KEY: ScheduleKey = ScheduleKey::new(TimerId(0), TimerGeneration(0));

    pub(super) fn begin(policy: ActorDrainPolicy) -> (Self, Option<ScheduleAfter>) {
        match policy {
            ActorDrainPolicy::WaitForActorGraph => (Self::Unlimited, None),
            ActorDrainPolicy::RetireActorGraphAfter { deadline } => {
                (Self::Scheduling(Self::KEY), Some(Self::KEY.after(deadline)))
            }
        }
    }

    pub(super) fn accept_schedule(
        &mut self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> Result<(), ActionItemResult<ScheduleAfter>> {
        let Self::Scheduling(expected) = self else {
            return Err(input);
        };
        let expected = *expected;
        let input = match expected.admit_result(input) {
            Ok(input) => input,
            Err(input) => return Err(input),
        };
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(_)) => {
                *self = Self::Waiting(expected);
                Ok(())
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
            input @ (SettledItem::Attempted(
                ItemSettlement::Rejected { .. } | ItemSettlement::Corrupt { .. },
            )
            | SettledItem::Unattempted(_)) => {
                *self = Self::NotScheduled(input);
                Ok(())
            }
        }
    }

    pub(super) fn accept_elapsed(&mut self, elapsed: TimerElapsed) -> Result<(), TimerElapsed> {
        let Self::Waiting(expected) = self else {
            return Err(elapsed);
        };
        let expected = *expected;
        match expected.admit_elapsed(elapsed) {
            Ok(elapsed) => {
                *self = Self::Elapsed(elapsed);
                Ok(())
            }
            Err(elapsed) => Err(elapsed),
        }
    }
}

#[expect(
    dead_code,
    reason = "Bombay's retirement custodian receives the complete cause"
)]
pub(super) enum ForcedRetirementCause {
    WorkerShutdownIdsExhausted,
    DeadlineNotScheduled(ActionItemResult<ScheduleAfter>),
    DeadlineElapsed(TimerElapsed),
}
