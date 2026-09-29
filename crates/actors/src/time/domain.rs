//! Typed timer lifecycle domains used by behavior wrappers.

use std::time::Instant;

use crate::{TimerGeneration, TimerId};

/// Whether one timer arrival consumed the currently armed schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimerAdmission {
    /// The arrival matched and consumed the current schedule.
    Accepted,
    /// The arrival did not match an armed schedule.
    Ignored,
}

/// Lifecycle of a re-armable generation-tagged timer.
pub(crate) enum TimerLease {
    NeverIssued,
    Armed(TimerGeneration),
    Idle(TimerGeneration),
    Exhausted,
}

impl TimerLease {
    pub(crate) const fn new() -> Self {
        Self::NeverIssued
    }

    #[cfg(test)]
    pub(crate) const fn idle(generation: TimerGeneration) -> Self {
        Self::Idle(generation)
    }

    pub(crate) fn arm(&mut self) -> Option<TimerGeneration> {
        let generation = match *self {
            Self::NeverIssued => TimerGeneration(0),
            Self::Armed(TimerGeneration(previous)) | Self::Idle(TimerGeneration(previous)) => {
                let Some(next) = previous.checked_add(1) else {
                    *self = Self::Exhausted;
                    return None;
                };
                TimerGeneration(next)
            }
            Self::Exhausted => return None,
        };
        *self = Self::Armed(generation);
        Some(generation)
    }

    pub(crate) fn accept(&mut self, generation: TimerGeneration) -> TimerAdmission {
        match *self {
            Self::Armed(live) if live == generation => {
                *self = Self::Idle(live);
                TimerAdmission::Accepted
            }
            Self::NeverIssued | Self::Armed(_) | Self::Idle(_) | Self::Exhausted => {
                TimerAdmission::Ignored
            }
        }
    }

    pub(crate) fn disarm(&mut self) {
        if let Self::Armed(generation) = *self {
            *self = Self::Idle(generation);
        }
    }

    #[cfg(test)]
    pub(crate) const fn live(&self) -> Option<TimerGeneration> {
        match self {
            Self::Armed(generation) => Some(*generation),
            Self::NeverIssued | Self::Idle(_) | Self::Exhausted => None,
        }
    }
}

/// Lifecycle of a one-shot absolute schedule.
pub(crate) enum OneShotSchedule {
    Unscheduled,
    Scheduled {
        id: TimerId,
        generation: TimerGeneration,
        at: Instant,
    },
}

impl OneShotSchedule {
    pub(crate) fn new(id: TimerId, at: Option<Instant>) -> Self {
        at.map_or(Self::Unscheduled, |at| Self::Scheduled {
            id,
            generation: TimerGeneration(0),
            at,
        })
    }

    pub(crate) const fn request(&self) -> Option<(TimerId, TimerGeneration, Instant)> {
        match self {
            Self::Unscheduled => None,
            Self::Scheduled { id, generation, at } => Some((*id, *generation, *at)),
        }
    }

    pub(crate) fn accept(&mut self, id: TimerId, generation: TimerGeneration) -> TimerAdmission {
        match self {
            Self::Scheduled {
                id: expected_id,
                generation: expected_generation,
                ..
            } if *expected_id == id && *expected_generation == generation => {
                *self = Self::Unscheduled;
                TimerAdmission::Accepted
            }
            Self::Unscheduled | Self::Scheduled { .. } => TimerAdmission::Ignored,
        }
    }

    pub(crate) fn cancel(&mut self) {
        *self = Self::Unscheduled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumed_generation_cannot_fire_twice() {
        let mut lease = TimerLease::new();
        let generation = lease.arm().unwrap();
        let first = lease.accept(generation);
        let duplicate = lease.accept(generation);
        assert_eq!(first, TimerAdmission::Accepted);
        assert_eq!(duplicate, TimerAdmission::Ignored);
    }

    #[test]
    fn cancelling_a_schedule_removes_its_request_and_prevents_acceptance() {
        let now = Instant::now();
        let mut schedule = OneShotSchedule::new(TimerId(4), Some(now));
        assert_eq!(
            schedule.request(),
            Some((TimerId(4), TimerGeneration(0), now))
        );
        schedule.cancel();
        assert_eq!(schedule.request(), None);
        let cancelled = schedule.accept(TimerId(4), TimerGeneration(0));
        assert_eq!(cancelled, TimerAdmission::Ignored);
    }

    #[test]
    fn foreign_schedule_arrivals_leave_the_current_deadline_available() {
        let now = Instant::now();
        let mut schedule = OneShotSchedule::new(TimerId(4), Some(now));
        let foreign_id = schedule.accept(TimerId(5), TimerGeneration(0));
        let foreign_generation = schedule.accept(TimerId(4), TimerGeneration(1));
        assert_eq!(foreign_id, TimerAdmission::Ignored);
        assert_eq!(foreign_generation, TimerAdmission::Ignored);
        assert_eq!(
            schedule.request(),
            Some((TimerId(4), TimerGeneration(0), now))
        );
        let accepted = schedule.accept(TimerId(4), TimerGeneration(0));
        let duplicate = schedule.accept(TimerId(4), TimerGeneration(0));
        assert_eq!(accepted, TimerAdmission::Accepted);
        assert_eq!(duplicate, TimerAdmission::Ignored);
        assert_eq!(schedule.request(), None);
    }
}
