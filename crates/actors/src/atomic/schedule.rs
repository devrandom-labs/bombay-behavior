//! Exact timer correlation shared by atomic actor policies.

use behavior::{ActionItemResult, ItemSettlement, SettledItem};

use crate::{ScheduleAfter, TimerElapsed, TimerGeneration, TimerId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScheduleKey {
    pub(super) id: TimerId,
    pub(super) generation: TimerGeneration,
}

impl ScheduleKey {
    pub(super) const fn new(id: TimerId, generation: TimerGeneration) -> Self {
        Self { id, generation }
    }

    pub(super) const fn after(self, delay: core::time::Duration) -> ScheduleAfter {
        ScheduleAfter::new(self.id, self.generation, delay)
    }

    pub(super) fn issue(next: &mut u64) -> Option<Self> {
        let issued = *next;
        *next = issued.checked_add(1)?;
        Some(Self::new(TimerId(issued), TimerGeneration(0)))
    }

    pub(super) fn admit_result(
        self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> Result<ActionItemResult<ScheduleAfter>, ActionItemResult<ScheduleAfter>> {
        if self.accepts_result(&input) {
            Ok(input)
        } else {
            Err(input)
        }
    }

    pub(super) fn accepts_result(&self, input: &ActionItemResult<ScheduleAfter>) -> bool {
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(scheduled)) => {
                self.id == scheduled.id && self.generation == scheduled.generation
            }
            SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
            | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })
            | SettledItem::Unattempted(item) => {
                self.id == item.id && self.generation == item.generation
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match *prerequisite {}
            }
        }
    }

    pub(super) fn admit_elapsed(self, elapsed: TimerElapsed) -> Result<TimerElapsed, TimerElapsed> {
        match elapsed {
            elapsed if self.id == elapsed.id && self.generation == elapsed.generation => {
                Ok(elapsed)
            }
            elapsed => Err(elapsed),
        }
    }
}
