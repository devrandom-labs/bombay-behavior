//! Action-producing peer termination observation.

use crate::{PeerStopped, WatchEvent, WatchSends};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, SendAlgebra, ServiceSends,
};

/// Pure fold applied to the exact matching terminal fact.
pub type TerminationReaction<B> =
    fn(&mut B, PeerStopped<crate::BehaviorAddr<B>>) -> BehaviorActed<B>;

/// Cleanup specialization of an action-producing terminal reaction.
pub type CleanupReaction<B> = TerminationReaction<B>;

/// Lifecycle-publication specialization of an action-producing terminal reaction.
pub type LifecyclePublication<B> = TerminationReaction<B>;

/// Complete consumption phase of one exact terminal observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationObservation {
    /// The configured peer's terminal fact has not been accepted.
    Observing,
    /// One matching terminal fact was accepted and cannot be folded again.
    Observed,
}

/// Observe one peer and fold its terminal fact into complete behavior actions.
///
/// Unlike [`crate::Watch`], the reaction returns the wrapped behavior's full
/// [`Actions`]. Cleanup communications, lifecycle publication, fresh
/// creations, and termination therefore remain explicit behavior decisions.
/// The runtime owns only exact-incarnation observation and delivery of the
/// authoritative [`PeerStopped`] fact.
pub struct TerminationMonitor<B: Behavior> {
    inner: B,
    peer: crate::BehaviorAddr<B>,
    on_stopped: TerminationReaction<B>,
    observation: TerminationObservation,
}

/// A termination monitor configured to emit explicit cleanup actions.
///
/// Cleanup uses the monitor's exact-once observation phase and differs only in
/// the supplied action-producing policy, so it is a named specialization and
/// not a second state machine.
pub type Reaper<B> = TerminationMonitor<B>;

/// A termination monitor configured to publish explicit lifecycle actions.
///
/// Subscriber membership remains in the wrapped behavior or a composed typed
/// topic; no ambient lifecycle side channel is introduced.
pub type LifecyclePublisher<B> = TerminationMonitor<B>;
type TerminationMonitorActions<B> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    WatchSends<crate::BehaviorAddr<B>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior> TerminationMonitor<B> {
    /// Construct an action-producing observation definition.
    #[must_use]
    pub const fn new(
        inner: B,
        peer: crate::BehaviorAddr<B>,
        on_stopped: TerminationReaction<B>,
    ) -> Self {
        Self {
            inner,
            peer,
            on_stopped,
            observation: TerminationObservation::Observing,
        }
    }

    /// Return whether this monitor awaits or has consumed its terminal fact.
    #[must_use]
    pub const fn observation(&self) -> TerminationObservation {
        self.observation
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        observations: ServiceSends<crate::ObservePeer<crate::BehaviorAddr<B>>>,
    ) -> TerminationMonitorActions<B> {
        actions.map_sends(|behavior| WatchSends {
            behavior,
            observations,
        })
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for TerminationMonitor<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B: Behavior + crate::StashStatus> crate::StashStatus for TerminationMonitor<B> {
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for TerminationMonitor<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event>;
    type Sends = WatchSends<A, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(Self::wrap(
            actions,
            ServiceSends::one(crate::ObservePeer::new(self.peer)),
        ))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(stopped)
                if stopped.peer == self.peer
                    && self.observation == TerminationObservation::Observing =>
            {
                let actions = (self.on_stopped)(&mut self.inner, stopped)?;
                self.observation = TerminationObservation::Observed;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, ServiceSends::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use crate::{Crash, Exit};
    use behavior::{Births, Create, MailAddr, Never, Step, User};

    struct Probe;

    impl crate::BehaviorBase for Probe {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Probe {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Probe {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<u8>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<()>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![1]))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "termination reactions preserve the behavior's fallible signature"
    )]
    #[allow(clippy::needless_pass_by_value)]
    fn reap(_: &mut Probe, stopped: PeerStopped<MailAddr>) -> BehaviorActed<Probe> {
        assert_eq!(stopped.peer, MailAddr(4));
        assert_eq!(stopped.outcome, Err(Crash::Panicked));
        Ok(Actions::new(
            vec![9],
            vec![Create::birth(8, ())],
            Step::Continue,
        ))
    }

    #[test]
    fn matching_terminal_fact_preserves_complete_reaction_actions() {
        let initialized = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap();
        assert_eq!(initialized.actions.sends.behavior, [1]);
        assert_eq!(
            initialized.actions.sends.observations,
            ServiceSends::one(crate::ObservePeer::new(MailAddr(4)))
        );

        let mut active = initialized.behavior;
        let actions = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::Panicked)))
            .unwrap();
        assert_eq!(actions.sends.behavior, [9]);
        assert!(actions.sends.observations.is_empty());
        assert_eq!(actions.creates, [Create::birth(8, ())]);
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(active.observation(), TerminationObservation::Observed);

        let duplicate = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::Panicked)))
            .unwrap();
        assert_eq!(duplicate.sends, WatchSends::empty());
        assert!(duplicate.creates.is_empty());
    }

    #[test]
    fn unmatched_terminal_fact_is_inert_and_user_actions_still_delegate() {
        let mut active = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap()
            .behavior;
        let unmatched = active
            .on_path(PeerStopped::new(MailAddr(5), Ok(Exit::Normal)))
            .unwrap();
        assert_eq!(unmatched.sends, WatchSends::empty());
        assert!(unmatched.creates.is_empty());

        let delegated = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(delegated.sends.behavior, [7]);
        assert!(delegated.sends.observations.is_empty());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Rejected;

    struct Fallible;

    impl behavior::Protocol for Fallible {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Fallible {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Rejected;
        type Birth = behavior::NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn reject(_: &mut Fallible, _: PeerStopped<MailAddr>) -> BehaviorActed<Fallible> {
        Err(Rejected)
    }

    #[test]
    fn rejected_reaction_does_not_commit_observation_consumption() {
        let mut active = TerminationMonitor::new(Fallible, MailAddr(4), reject)
            .initialize()
            .unwrap()
            .behavior;
        assert_eq!(
            active.on_path(PeerStopped::new(MailAddr(4), Err(Crash::Failed))),
            Err(Rejected)
        );
        assert_eq!(active.observation(), TerminationObservation::Observing);
    }
}
