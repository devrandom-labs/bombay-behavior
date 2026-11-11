//! Custody of a reheld prefix and its still-unread input suffix.

use std::collections::VecDeque;

use behavior::{Address, User};

// Selection borrows the front before removing it. On every return or unwind,
// reunite unread originals after the prefix already retained by the actor.
// This product selects no transitions and cannot restore consumed inputs.
pub(crate) struct HeldInputs<'a, A: Address, M> {
    pub(crate) held: &'a mut VecDeque<User<A, M>>,
    pub(crate) unread: VecDeque<User<A, M>>,
}

impl<A: Address, M> Drop for HeldInputs<'_, A, M> {
    fn drop(&mut self) {
        self.held.append(&mut self.unread);
    }
}
