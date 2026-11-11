//! Admission of complete original inputs to a held FIFO.

use std::collections::VecDeque;

use behavior::{Address, Never, Protocol, SendEffects, User};

use crate::composition::DeliveryRoute;

mod sealed {
    pub trait InputRetention {}
    impl InputRetention for () {}
    impl<R> InputRetention for super::InputLimit<R> {}
}

/// Library-owned admission of one complete original to its authoritative queue.
///
/// Unit holds without a bound. [`InputLimit`] holds within its fixed quota or
/// returns the complete original through its typed refusal sends. Neither
/// implementation clones payloads or selects an actor transition.
///
/// Applications cannot forge another retention policy:
///
/// ```compile_fail,E0277
/// struct Forged;
/// impl behavior_actors::InputRetention<behavior::MailAddr, u8> for Forged {
///     type Sends = Vec<behavior::Never>;
///     fn hold(&self, held: &mut std::collections::VecDeque<behavior::User<behavior::MailAddr, u8>>, input: behavior::User<behavior::MailAddr, u8>) -> Self::Sends {
///         held.push_back(input);
///         Vec::new()
///     }
/// }
/// ```
pub trait InputRetention<A: Address, M>: sealed::InputRetention {
    type Sends: SendEffects;

    /// Hold the original or return it intact through the configured refusal lane.
    /// Finite holding can unwind if capability cloning or delivery construction
    /// panics. Already held inputs remain; an incoming value consumed by unwind
    /// is not restored. No payload clone or state rollback is promised.
    fn hold(&self, held: &mut VecDeque<User<A, M>>, input: User<A, M>) -> Self::Sends;
}

/// A fixed number of held-input slots and their repeatable refusal capability.
///
/// Zero is valid. This counts original inputs, not bytes, allocated capacity,
/// other actor-owned queues or a runtime mailbox. The owning actor supplies its
/// current FIFO; full admission emits the complete original without changing
/// that FIFO. The configuration does not execute or undo a behavior callback.
#[derive(Clone)]
pub struct InputLimit<R> {
    slots: usize,
    refused: R,
}

impl<R: DeliveryRoute + Clone> InputLimit<R> {
    /// Configure finite input holding without processing or allocating an input.
    #[must_use]
    pub fn new(slots: usize, refused: R) -> Self {
        Self { slots, refused }
    }
}

impl<A: Address, M> InputRetention<A, M> for () {
    type Sends = Vec<Never>;

    fn hold(&self, held: &mut VecDeque<User<A, M>>, input: User<A, M>) -> Self::Sends {
        held.push_back(input);
        Vec::new()
    }
}

impl<A, M, R> InputRetention<A, M> for InputLimit<R>
where
    A: Address,
    R: DeliveryRoute + Clone,
    R::Protocol: Protocol<Addr = A, Msg = User<A, M>>,
{
    type Sends = R::Sends;

    fn hold(&self, held: &mut VecDeque<User<A, M>>, input: User<A, M>) -> Self::Sends {
        if held.len() < self.slots {
            held.push_back(input);
            Self::Sends::empty()
        } else {
            self.refused.clone().deliver(input)
        }
    }
}
