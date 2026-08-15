//! Independent reference models shared by adversarial tests. The supervision
//! model encodes the DOCUMENTED contract (eligibility by policy; lazy window
//! pruning inclusive at the edge, future stamps survive; candidate sets over
//! alive slots with birth-sequence ordering for `RestForOne`; budget counts
//! every replacement; denial kills the dead slot) — written from the spec,
//! never from implementation branches.

use behavior::{Crash, Exit, MailAddr, RestartPolicy, Strategy};

/// Independent activity-history model for one-notification-per-idle-period
/// receive timeout. Service traffic is deliberately absent from `activity`:
/// it never changes the expected live token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InactivityModel {
    last_token: Option<u64>,
    live_token: Option<u64>,
}

impl InactivityModel {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_token: None,
            live_token: None,
        }
    }

    /// Successful continuing initialization begins the first idle period.
    pub fn initialize(&mut self) -> u64 {
        self.arm()
    }

    /// A successful continuing user communication begins a new idle period.
    pub fn activity(&mut self) -> Option<u64> {
        self.last_token
            .and_then(|token| token.checked_add(1))
            .inspect(|&token| {
                self.last_token = Some(token);
                self.live_token = Some(token);
            })
    }

    /// Errors, terminal turns, and all service traffic leave timer state alone.
    #[must_use]
    pub const fn no_activity(&self) -> Option<u64> {
        self.live_token
    }

    /// Consume only the token for the current idle period.
    pub fn notification(&mut self, token: u64) -> bool {
        if self.live_token == Some(token) {
            self.live_token = None;
            true
        } else {
            false
        }
    }

    fn arm(&mut self) -> u64 {
        self.last_token = Some(0);
        self.live_token = Some(0);
        0
    }
}

impl Default for InactivityModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Independent vocabulary for the semantic role expected on a creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedCreation {
    Initial,
    Ordinary,
    Successor,
}

/// An expected creation emitted by the independent incarnation model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedIncarnation {
    pub nonce: u64,
    pub role: ExpectedCreation,
}

/// Independent model of a stable slot and its non-overlapping incarnations.
///
/// This model deliberately uses different state and vocabulary from `Proxy`:
/// a slot is either occupied or vacant, with at most one queued successor.
pub struct IncarnationModel {
    occupied: Option<u64>,
    queued_successor: bool,
    next_nonce: u64,
}

impl IncarnationModel {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            occupied: None,
            queued_successor: false,
            next_nonce: 0,
        }
    }

    /// Establish the slot's first incarnation.
    #[must_use]
    pub fn initialize(&mut self) -> ExpectedIncarnation {
        self.occupied = Some(0);
        self.next_nonce = 1;
        ExpectedIncarnation {
            nonce: 0,
            role: ExpectedCreation::Initial,
        }
    }

    /// Model an unrelated dynamic birth outside the stable slot.
    #[must_use]
    pub const fn ordinary(nonce: u64) -> ExpectedIncarnation {
        ExpectedIncarnation {
            nonce,
            role: ExpectedCreation::Ordinary,
        }
    }

    /// Admit a successor request, creating immediately only when vacant.
    pub fn request_successor(&mut self) -> Option<ExpectedIncarnation> {
        if self.occupied.is_some() {
            self.queued_successor = true;
            None
        } else {
            Some(self.install_successor())
        }
    }

    /// Vacate the matching incarnation and install a queued successor.
    pub fn stopped(&mut self, nonce: u64) -> Option<ExpectedIncarnation> {
        if self.occupied != Some(nonce) {
            return None;
        }
        self.occupied = None;
        self.queued_successor.then(|| {
            self.queued_successor = false;
            self.install_successor()
        })
    }

    fn install_successor(&mut self) -> ExpectedIncarnation {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.checked_add(1).unwrap();
        self.occupied = Some(nonce);
        ExpectedIncarnation {
            nonce,
            role: ExpectedCreation::Successor,
        }
    }
}

impl Default for IncarnationModel {
    fn default() -> Self {
        Self::new()
    }
}

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
    last_restart_denied: bool,
    last_replacements_requested: usize,
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
            last_restart_denied: false,
            last_replacements_requested: 0,
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

    #[must_use]
    pub const fn last_restart_denied(&self) -> bool {
        self.last_restart_denied
    }

    #[must_use]
    pub const fn last_replacements_requested(&self) -> usize {
        self.last_replacements_requested
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
        self.last_restart_denied = false;
        self.last_replacements_requested = 0;
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
        self.last_replacements_requested = candidates.len();
        if self.restarts.len() + candidates.len() > maximum as usize {
            self.slots[dead].alive = false;
            self.last_restart_denied = true;
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
