use core::num::NonZeroU64;

use crate::{ScheduleAfter, TimerGeneration, TimerId};

use super::super::super::schedule::ScheduleKey;

use super::super::restart::{RecoveryDenialReason, RecoveryRelease};

#[derive(Debug, Eq, PartialEq)]
enum CorrelationSequence {
    Next(NonZeroU64),
    Exhausted,
}

#[derive(Debug, Eq, PartialEq)]
struct ProposedCorrelation {
    issued: NonZeroU64,
    successor: CorrelationSequence,
}

impl CorrelationSequence {
    const fn new() -> Self {
        Self::Next(NonZeroU64::MIN)
    }

    fn propose(self) -> Result<ProposedCorrelation, Self> {
        match self {
            Self::Next(issued) => Ok(ProposedCorrelation {
                issued,
                successor: match issued.get().checked_add(1).and_then(NonZeroU64::new) {
                    Some(next) => Self::Next(next),
                    None => Self::Exhausted,
                },
            }),
            Self::Exhausted => Err(self),
        }
    }
}

impl ProposedCorrelation {
    fn accept(self) -> (CorrelationSequence, NonZeroU64) {
        (self.successor, self.issued)
    }

    fn decline(self) -> CorrelationSequence {
        CorrelationSequence::Next(self.issued)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in super::super) struct RecoveryCorrelations {
    recoveries: CorrelationSequence,
    timers: CorrelationSequence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct RecoveryTicket(pub(in super::super) NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum RecoveryTimer {
    Immediate,
    Delayed(ScheduleKey),
}

pub(in super::super) struct RecoveryTicketProposal {
    recovery: ProposedCorrelation,
    timers: CorrelationSequence,
}

pub(in super::super) struct RecoveryCorrelationProposal {
    timing: RecoveryTimingProposal,
}

enum RecoveryTimingProposal {
    Immediate {
        recovery: ProposedCorrelation,
        timers: CorrelationSequence,
    },
    Delayed {
        recovery: ProposedCorrelation,
        timer: ProposedCorrelation,
        after: core::time::Duration,
    },
}

pub(in super::super) struct AcceptedRecoveryCorrelations {
    pub(in super::super) correlations: RecoveryCorrelations,
    pub(in super::super) recovery: RecoveryTicket,
    pub(in super::super) timer: RecoveryTimer,
    pub(in super::super) schedule: Option<ScheduleAfter>,
}

impl RecoveryCorrelationProposal {
    pub(in super::super) fn accept(self) -> AcceptedRecoveryCorrelations {
        match self.timing {
            RecoveryTimingProposal::Immediate { recovery, timers } => {
                let (recoveries, recovery) = recovery.accept();
                AcceptedRecoveryCorrelations {
                    correlations: RecoveryCorrelations { recoveries, timers },
                    recovery: RecoveryTicket(recovery),
                    timer: RecoveryTimer::Immediate,
                    schedule: None,
                }
            }
            RecoveryTimingProposal::Delayed {
                recovery,
                timer,
                after,
            } => {
                let (recoveries, recovery) = recovery.accept();
                let (timers, timer) = timer.accept();
                let timer = ScheduleKey::new(TimerId(timer.get()), TimerGeneration(0));
                AcceptedRecoveryCorrelations {
                    correlations: RecoveryCorrelations { recoveries, timers },
                    recovery: RecoveryTicket(recovery),
                    timer: RecoveryTimer::Delayed(timer),
                    schedule: Some(timer.after(after)),
                }
            }
        }
    }
}

pub(in super::super) struct RecoveryCorrelationDenial {
    pub(in super::super) unchanged: RecoveryCorrelations,
    pub(in super::super) reason: RecoveryDenialReason,
}

impl RecoveryCorrelations {
    pub(in super::super) const fn new() -> Self {
        Self {
            recoveries: CorrelationSequence::new(),
            timers: CorrelationSequence::new(),
        }
    }

    pub(in super::super) fn propose_recovery(
        self,
    ) -> Result<RecoveryTicketProposal, RecoveryCorrelationDenial> {
        let Self { recoveries, timers } = self;
        let recovery = match recoveries.propose() {
            Ok(recovery) => recovery,
            Err(recoveries) => {
                return Err(RecoveryCorrelationDenial {
                    unchanged: Self { recoveries, timers },
                    reason: RecoveryDenialReason::RecoveryTicketsExhausted,
                });
            }
        };
        Ok(RecoveryTicketProposal { recovery, timers })
    }
}

impl RecoveryTicketProposal {
    pub(in super::super) fn select_release(
        self,
        release: RecoveryRelease,
    ) -> Result<RecoveryCorrelationProposal, RecoveryCorrelationDenial> {
        let Self { recovery, timers } = self;
        match release {
            RecoveryRelease::Immediate => Ok(RecoveryCorrelationProposal {
                timing: RecoveryTimingProposal::Immediate { recovery, timers },
            }),
            RecoveryRelease::Delayed(after) => match timers.propose() {
                Ok(timer) => Ok(RecoveryCorrelationProposal {
                    timing: RecoveryTimingProposal::Delayed {
                        recovery,
                        timer,
                        after,
                    },
                }),
                Err(timers) => Err(RecoveryCorrelationDenial {
                    unchanged: Self { recovery, timers }.decline(),
                    reason: RecoveryDenialReason::RestartTimersExhausted,
                }),
            },
        }
    }

    pub(in super::super) fn decline(self) -> RecoveryCorrelations {
        RecoveryCorrelations {
            recoveries: self.recovery.decline(),
            timers: self.timers,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use core::time::Duration;

    use super::super::super::super::schedule::ScheduleKey;
    use super::{
        AcceptedRecoveryCorrelations, CorrelationSequence, RecoveryCorrelationDenial,
        RecoveryCorrelationProposal, RecoveryCorrelations, RecoveryTicket, RecoveryTimer,
    };
    use crate::atomic::fixed_supervisor::restart::{RecoveryDenialReason, RecoveryRelease};
    use crate::{TimerGeneration, TimerId};

    fn number(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("test correlation is positive")
    }

    fn proposed(
        correlations: RecoveryCorrelations,
        release: RecoveryRelease,
    ) -> RecoveryCorrelationProposal {
        let Ok(recovery) = correlations.propose_recovery() else {
            panic!("test recovery ticket is proposed")
        };
        let Ok(proposal) = recovery.select_release(release) else {
            panic!("test timer is proposed")
        };
        proposal
    }

    #[test]
    fn recovery_ticket_can_be_declined_before_release_selection() {
        let Ok(proposal) = RecoveryCorrelations::new().propose_recovery() else {
            panic!("fresh recovery ticket is proposed")
        };
        assert_eq!(proposal.decline(), RecoveryCorrelations::new());
    }

    #[test]
    fn immediate_accepts_only_recovery_and_delayed_accepts_both() {
        let AcceptedRecoveryCorrelations {
            correlations,
            recovery: RecoveryTicket(immediate_recovery),
            timer: RecoveryTimer::Immediate,
            schedule: None,
        } = proposed(RecoveryCorrelations::new(), RecoveryRelease::Immediate).accept()
        else {
            panic!("immediate proposal owns no timer")
        };
        assert_eq!(immediate_recovery, number(1));
        assert_eq!(correlations.timers, CorrelationSequence::Next(number(1)));

        let AcceptedRecoveryCorrelations {
            correlations,
            recovery: RecoveryTicket(first_recovery),
            timer: RecoveryTimer::Delayed(first_timer),
            schedule: Some(first_schedule),
        } = proposed(
            correlations,
            RecoveryRelease::Delayed(Duration::from_secs(3)),
        )
        .accept()
        else {
            panic!("delayed proposal owns one timer")
        };
        let AcceptedRecoveryCorrelations {
            recovery: RecoveryTicket(second_recovery),
            timer: RecoveryTimer::Delayed(second_timer),
            ..
        } = proposed(
            correlations,
            RecoveryRelease::Delayed(Duration::from_secs(3)),
        )
        .accept()
        else {
            panic!("overlapping delayed proposal owns a distinct timer")
        };
        assert_ne!(first_recovery, second_recovery);
        assert_ne!(first_timer, second_timer);
        assert_eq!(first_schedule.id, first_timer.id);
        assert_eq!(first_schedule.generation, first_timer.generation);
        assert_eq!(
            first_timer,
            ScheduleKey {
                id: TimerId(1),
                generation: TimerGeneration(0),
            }
        );
    }

    #[test]
    fn maximum_recovery_is_issued_once_then_exhaustion_is_unchanged() {
        let correlations = RecoveryCorrelations {
            recoveries: CorrelationSequence::Next(number(u64::MAX)),
            timers: CorrelationSequence::new(),
        };
        let AcceptedRecoveryCorrelations { correlations, .. } =
            proposed(correlations, RecoveryRelease::Immediate).accept();
        let Err(RecoveryCorrelationDenial { unchanged, reason }) = correlations.propose_recovery()
        else {
            panic!("exhausted recovery sequence denies")
        };
        assert_eq!(reason, RecoveryDenialReason::RecoveryTicketsExhausted);
        assert_eq!(unchanged.recoveries, CorrelationSequence::Exhausted);
        assert_eq!(unchanged.timers, CorrelationSequence::Next(number(1)));
    }

    #[test]
    fn timer_exhaustion_declines_the_recovery_number() {
        let correlations = RecoveryCorrelations {
            recoveries: CorrelationSequence::new(),
            timers: CorrelationSequence::Next(number(u64::MAX)),
        };
        let AcceptedRecoveryCorrelations { correlations, .. } = proposed(
            correlations,
            RecoveryRelease::Delayed(Duration::from_secs(3)),
        )
        .accept();
        let Ok(recovery) = correlations.propose_recovery() else {
            panic!("next recovery ticket is proposed")
        };
        let Err(RecoveryCorrelationDenial { unchanged, reason }) =
            recovery.select_release(RecoveryRelease::Delayed(Duration::from_secs(3)))
        else {
            panic!("exhausted timer sequence denies")
        };
        assert_eq!(reason, RecoveryDenialReason::RestartTimersExhausted);
        assert_eq!(unchanged.recoveries, CorrelationSequence::Next(number(2)));
        assert_eq!(unchanged.timers, CorrelationSequence::Exhausted);
    }
}
