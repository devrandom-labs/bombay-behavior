//! Supervised stable-child topology and lifecycle.

use super::super::Strategy;
use crate::CreationRejection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Available,
    Retired,
}

struct Slot<N> {
    nonce: N,
    sequence: u64,
    state: SlotState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FleetError<N> {
    UnknownChild(N),
    DuplicateChild(N),
    SequenceExhausted,
}

pub(crate) struct ReplacementCandidate<N> {
    pub index: usize,
    pub nonce: N,
}

/// Ordered topology of stable supervised children.
pub(crate) struct Fleet<N> {
    slots: Vec<Slot<N>>,
    configured: usize,
    next_sequence: u64,
}

impl<N: Copy + PartialEq> Fleet<N> {
    pub fn configured(nonces: impl IntoIterator<Item = N>) -> Result<Self, FleetError<N>> {
        let mut fleet = Self {
            slots: Vec::new(),
            configured: 0,
            next_sequence: 0,
        };
        for nonce in nonces {
            fleet.register(nonce)?;
            fleet.configured += 1;
        }
        Ok(fleet)
    }

    pub fn configured_nonces(&self) -> impl Iterator<Item = N> + '_ {
        self.slots[..self.configured].iter().map(|slot| slot.nonce)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_available(&self, nonce: N) -> Result<bool, FleetError<N>> {
        Ok(self.slot(nonce)?.state == SlotState::Available)
    }

    pub fn register(&mut self, nonce: N) -> Result<(), FleetError<N>> {
        if self.position(nonce).is_some() {
            return Err(FleetError::DuplicateChild(nonce));
        }
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(FleetError::SequenceExhausted)?;
        self.slots.push(Slot {
            nonce,
            sequence,
            state: SlotState::Available,
        });
        Ok(())
    }

    pub(crate) fn resolve_creation(&mut self, nonce: N, result: Result<(), CreationRejection>) {
        if let Some(position) = self.position(nonce) {
            self.slots[position].state = match result {
                Ok(()) => SlotState::Available,
                Err(_) => SlotState::Retired,
            };
        }
    }

    pub fn retire(&mut self, nonce: N) -> Result<(), FleetError<N>> {
        self.slot_mut(nonce)?.state = SlotState::Retired;
        Ok(())
    }

    /// A replacement command keeps the stable proxy slot available while its
    /// worker replacement is pending.
    pub fn replacement_requested(&mut self, nonce: N) -> Result<(), FleetError<N>> {
        self.slot_mut(nonce)?.state = SlotState::Available;
        Ok(())
    }

    pub fn replacements(
        &self,
        failed: N,
        strategy: Strategy,
    ) -> Result<Vec<ReplacementCandidate<N>>, FleetError<N>> {
        let failed = self
            .position(failed)
            .ok_or(FleetError::UnknownChild(failed))?;
        let sequence = self.slots[failed].sequence;
        Ok(self
            .slots
            .iter()
            .enumerate()
            .filter(|(index, slot)| match strategy {
                Strategy::OneForOne => *index == failed,
                Strategy::OneForAll => slot.state == SlotState::Available,
                Strategy::RestForOne => {
                    slot.state == SlotState::Available && slot.sequence >= sequence
                }
            })
            .map(|(index, slot)| ReplacementCandidate {
                index,
                nonce: slot.nonce,
            })
            .collect())
    }

    fn position(&self, nonce: N) -> Option<usize> {
        self.slots.iter().position(|slot| slot.nonce == nonce)
    }

    fn slot(&self, nonce: N) -> Result<&Slot<N>, FleetError<N>> {
        self.position(nonce)
            .map(|position| &self.slots[position])
            .ok_or(FleetError::UnknownChild(nonce))
    }

    fn slot_mut(&mut self, nonce: N) -> Result<&mut Slot<N>, FleetError<N>> {
        self.position(nonce)
            .map(|position| &mut self.slots[position])
            .ok_or(FleetError::UnknownChild(nonce))
    }
}

impl<N: Copy + PartialEq> TryFrom<Vec<N>> for Fleet<N> {
    type Error = FleetError<N>;

    fn try_from(nonces: Vec<N>) -> Result<Self, Self::Error> {
        Self::configured(nonces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_for_one_uses_birth_sequence_and_skips_retired_slots() {
        let mut fleet = Fleet::configured([7, 2, 9]).unwrap();
        fleet.retire(2).unwrap();
        let replacements = fleet.replacements(7, Strategy::RestForOne).unwrap();
        assert_eq!(
            replacements
                .into_iter()
                .map(|candidate| candidate.nonce)
                .collect::<Vec<_>>(),
            [7, 9]
        );
    }

    #[test]
    fn one_for_one_addresses_the_exact_slot_for_each_stop_observation() {
        let mut fleet = Fleet::configured([7]).unwrap();
        fleet.retire(7).unwrap();
        let replacements = fleet.replacements(7, Strategy::OneForOne).unwrap();
        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0].nonce, 7);
    }

    #[test]
    fn creation_resolution_commits_success_and_retires_rejection() {
        let mut fleet = Fleet::configured([7, 9]).unwrap();
        fleet.retire(7).unwrap();
        fleet.resolve_creation(7, Ok(()));
        assert_eq!(fleet.is_available(7), Ok(true));

        fleet.resolve_creation(9, Err(CreationRejection::EnvironmentFailed));
        assert_eq!(fleet.is_available(9), Ok(false));

        fleet.resolve_creation(99, Err(CreationRejection::EnvironmentFailed));
        assert_eq!(fleet.len(), 2);
    }
}
