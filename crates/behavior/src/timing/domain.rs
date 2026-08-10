//! Typed timer lifecycle domains used by behavior wrappers.

use tokio::time::Instant;

use crate::{TimerGeneration, TimerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GenerationExhausted;

/// Lifecycle of a re-armable generation-tagged timer.
pub(crate) enum TimerLease {
    NeverIssued,
    Armed(TimerGeneration),
    Idle(TimerGeneration),
}

impl TimerLease {
    pub(crate) const fn new() -> Self {
        Self::NeverIssued
    }

    #[cfg(test)]
    pub(crate) const fn idle(generation: TimerGeneration) -> Self {
        Self::Idle(generation)
    }

    pub(crate) fn arm(&mut self) -> Result<TimerGeneration, GenerationExhausted> {
        let generation = match *self {
            Self::NeverIssued => TimerGeneration(0),
            Self::Armed(TimerGeneration(previous)) | Self::Idle(TimerGeneration(previous)) => {
                TimerGeneration(previous.checked_add(1).ok_or(GenerationExhausted)?)
            }
        };
        *self = Self::Armed(generation);
        Ok(generation)
    }

    pub(crate) fn accept(&mut self, generation: TimerGeneration) -> bool {
        match *self {
            Self::Armed(live) if live == generation => {
                *self = Self::Idle(live);
                true
            }
            Self::NeverIssued | Self::Armed(_) | Self::Idle(_) => false,
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
            Self::NeverIssued | Self::Idle(_) => None,
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

    pub(crate) fn accept(&mut self, id: TimerId, generation: TimerGeneration) -> bool {
        match self {
            Self::Scheduled {
                id: expected_id,
                generation: expected_generation,
                ..
            } if *expected_id == id && *expected_generation == generation => {
                *self = Self::Unscheduled;
                true
            }
            Self::Unscheduled | Self::Scheduled { .. } => false,
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
        assert!(lease.accept(generation));
        assert!(!lease.accept(generation));
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
        assert!(!schedule.accept(TimerId(4), TimerGeneration(0)));
    }
}
