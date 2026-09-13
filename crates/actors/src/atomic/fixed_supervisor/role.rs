//! Immutable role naming and unique fixed-member topology authority.

use super::super::RoleName;
use super::super::restart::RecoveryCount;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RosterPosition(usize);

impl RosterPosition {
    pub(super) const fn new(position: usize) -> Self {
        Self(position)
    }
}

pub(super) struct MemberRole<Role> {
    position: RosterPosition,
    name: RoleName<Role>,
    recoveries: RecoveryCount,
}

impl<Role> MemberRole<Role> {
    pub(super) fn declared(position: RosterPosition, role: Role) -> Self {
        Self {
            position,
            name: RoleName::new(role),
            recoveries: RecoveryCount::new(),
        }
    }

    pub(super) fn role(&self) -> &Role {
        self.name.role()
    }

    pub(super) const fn position(&self) -> RosterPosition {
        self.position
    }

    pub(super) fn name(&self) -> RoleName<Role> {
        self.name.clone()
    }

    pub(super) const fn recovery_count(&mut self) -> &mut RecoveryCount {
        &mut self.recoveries
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::atomic::restart::RecoveryProposal;

    use super::{MemberRole, RosterPosition};

    struct NonCloneRole<'a> {
        drops: &'a AtomicUsize,
    }

    #[test]
    fn member_keeps_its_declaration_position() {
        let position = RosterPosition::new(2);
        let member = MemberRole::declared(position, "search");

        assert_eq!(member.position(), position);
    }

    impl Drop for NonCloneRole<'_> {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn member_and_outgoing_name_share_one_non_clone_role() {
        let drops = AtomicUsize::new(0);
        let member = MemberRole::declared(RosterPosition::new(0), NonCloneRole { drops: &drops });
        let outgoing = member.name();

        assert!(core::ptr::eq(member.role(), outgoing.role()));
        drop(member);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(outgoing);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn accepted_recovery_advances_the_members_next_ordinal() {
        let mut member = MemberRole::declared(RosterPosition::new(0), "search");
        let RecoveryProposal::Available(first) = member.recovery_count().propose() else {
            panic!("a new member has its first recovery ordinal")
        };
        assert_eq!(first.ordinal().get(), 1);
        first.accept();

        let RecoveryProposal::Available(second) = member.recovery_count().propose() else {
            panic!("an accepted recovery leaves the next ordinal available")
        };
        assert_eq!(second.ordinal().get(), 2);
        second.decline();
    }
}
