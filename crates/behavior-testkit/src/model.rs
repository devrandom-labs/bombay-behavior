//! Independent reference models shared by adversarial tests. The supervision
//! model encodes the DOCUMENTED contract (eligibility by policy; lazy window
//! pruning inclusive at the edge, future stamps survive; candidate sets over
//! alive slots with birth-sequence ordering for `RestForOne`; budget counts
//! every replacement; denial kills the dead slot) — written from the spec,
//! never from implementation branches.

use behavior::{Crash, Exit, MailAddr, RestartPolicy, Strategy};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Normal,
    Collected,
    LinkDied,
    Failed,
    EnvironmentFailed,
    Panicked,
    Cancelled,
}

impl Outcome {
    /// Maps the outcome tag to its trace-level result.
    ///
    /// # Errors
    /// Returns the corresponding crash domain for an abnormal runtime outcome.
    #[must_use = "maps the outcome to its trace result"]
    pub fn into_result(self) -> Result<Exit<MailAddr>, Crash> {
        match self {
            Self::Normal => Ok(Exit::Normal),
            Self::Collected => Ok(Exit::Collected),
            Self::LinkDied => Ok(Exit::LinkDied(MailAddr(9))),
            Self::Failed => Err(Crash::Failed),
            Self::EnvironmentFailed => Err(Crash::EnvironmentFailed),
            Self::Panicked => Err(Crash::Panicked),
            Self::Cancelled => Err(Crash::Cancelled),
        }
    }

    #[must_use = "decodes the generator tag"]
    pub fn from_tag(tag: u8) -> Self {
        match tag {
            0 => Self::Normal,
            1 => Self::Collected,
            2 => Self::LinkDied,
            3 => Self::Failed,
            4 => Self::EnvironmentFailed,
            5 => Self::Panicked,
            _ => Self::Cancelled,
        }
    }

    #[must_use = "reports transient-policy eligibility"]
    pub const fn eligible_transient(self) -> bool {
        !matches!(self, Self::Normal | Self::Collected)
    }
}

pub struct Slot {
    pub nonce: u64,
    pub alive: bool,
    pub sequence: u64,
}

/// The independent supervision reference model.
pub struct Model {
    slots: Vec<Slot>,
    restarts: Vec<u64>,
    next_sequence: u64,
}

impl Model {
    /// Builds the configured fleet with identity nonces.
    ///
    /// # Panics
    /// Only if a fleet index cannot be represented as `u64` (128-bit hosts).
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            slots: (0..count)
                .map(|index| Slot {
                    nonce: u64::try_from(index).unwrap(),
                    alive: true,
                    sequence: u64::try_from(index).unwrap(),
                })
                .collect(),
            restarts: Vec::new(),
            next_sequence: u64::try_from(count).unwrap(),
        }
    }

    /// The parent created a dynamic child with this fresh nonce.
    ///
    /// # Panics
    /// If the birth sequence overflows `u64`.
    pub fn birth(&mut self, nonce: u64) {
        self.slots.push(Slot {
            nonce,
            alive: true,
            sequence: self.next_sequence,
        });
        self.next_sequence = self.next_sequence.checked_add(1).unwrap();
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn alive(&self, nonce: u64) -> Option<bool> {
        self.slots
            .iter()
            .find(|slot| slot.nonce == nonce)
            .map(|slot| slot.alive)
    }

    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    #[must_use]
    pub fn restarts(&self) -> usize {
        self.restarts.len()
    }

    /// Returns the nonces of the replacement sends the contract demands.
    ///
    /// # Panics
    /// If `dead` is not a slot of this fleet.
    #[allow(
        clippy::too_many_arguments,
        reason = "the model mirrors the supervisor's full parameter surface"
    )]
    pub fn apply(
        &mut self,
        dead: u64,
        outcome: Outcome,
        at: u64,
        strategy: Strategy,
        policy: RestartPolicy,
        maximum: u32,
        window: Option<u64>,
    ) -> Vec<u64> {
        // First slot with this nonce — identity nonces make this unique.
        let dead = self
            .slots
            .iter()
            .position(|slot| slot.nonce == dead)
            .expect("model: unknown supervised nonce");
        let eligible = match policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => outcome.eligible_transient(),
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.slots[dead].alive = false;
            return Vec::new();
        }
        if let Some(window) = window {
            self.restarts
                .retain(|stamp| *stamp > at || at - stamp <= window);
        }
        let sequence = self.slots[dead].sequence;
        let candidates: Vec<usize> = match strategy {
            Strategy::OneForOne => vec![dead],
            Strategy::OneForAll => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| slot.alive.then_some(index))
                .collect(),
            Strategy::RestForOne => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| {
                    (slot.alive && slot.sequence >= sequence).then_some(index)
                })
                .collect(),
        };
        if self.restarts.len() + candidates.len() > maximum as usize {
            self.slots[dead].alive = false;
            return Vec::new();
        }
        self.restarts
            .resize(self.restarts.len() + candidates.len(), at);
        for index in &candidates {
            self.slots[*index].alive = true;
        }
        candidates
            .into_iter()
            .map(|index| self.slots[index].nonce)
            .collect()
    }
}
