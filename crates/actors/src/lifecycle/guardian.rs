//! Application-root lifecycle composition.

use crate::{ShutdownEvent, ShutdownRequested};
use behavior::{Actions, Address, Behavior, BehaviorActed, BirthMode, RouteInput, SendAlgebra};

/// The application or subtree lifecycle boundary around one concrete behavior.
///
/// `Guardian` preserves the wrapped behavior's initialization, event, send,
/// creation, error, phase, and termination contracts. In particular, children
/// staged by wrapped initialization remain ordinary fresh Bombay creations in
/// their original order. A shutdown request stops the guardian normally; the
/// interpreter remains responsible for retiring the guardian's established
/// child namespace.
///
/// This is a Bombay application-structure policy, not an actor-model primitive
/// and not Hewitt and Attardi's protected-resource guardian. It deliberately
/// owns no observation, restart strategy, replacement, or restart budget.
///
/// A Guardian definition cannot receive mailbox input before its consuming
/// activation transition:
///
/// ```compile_fail
/// use behavior_actors::{Guardian, Machine, MailAddr, Move, Never};
///
/// let mut definition = Guardian::new(Machine::<MailAddr, _, _, _, Never>::new(
///     (),
///     (),
///     |_, _, _| Ok(Move::Stay),
/// ));
/// definition.receive(MailAddr(0), ());
/// ```
///
/// The boundary is not a second recipient identity; callers retain the
/// wrapped actor's protocol instead:
///
/// ```compile_fail
/// use behavior_actors::{Guardian, MailAddr, Machine, Never, Recipient};
/// let _ = Recipient::<Guardian<Machine<MailAddr, (), (), (), Never>>>::global(MailAddr(0));
/// ```
pub struct Guardian<B> {
    inner: B,
}

impl<B> Guardian<B> {
    /// Establish `inner` as an application or subtree lifecycle root.
    #[must_use]
    pub const fn new(inner: B) -> Self {
        Self { inner }
    }

    /// Consume the boundary and return its wrapped behavior definition.
    #[must_use]
    pub fn into_inner(self) -> B {
        self.inner
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for Guardian<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B: crate::StashStatus> crate::StashStatus for Guardian<B> {
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Guardian<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    B::Event: RouteInput<ShutdownRequested>,
{
    type Protocol = B::Protocol;
    type Event = ShutdownEvent<B::Event>;
    type Sends = Sends;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ShutdownEvent::Behavior(event) => behavior::delegate_transition(&mut self.inner, event),
            ShutdownEvent::ShutdownRequested(request) => match B::Event::route(request) {
                Ok(event) => behavior::delegate_transition(&mut self.inner, event),
                Err(ShutdownRequested) => Ok(Actions::stop()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use crate::{Crash, Exit, PeerStopped, WatchEvent, WatchSends};
    use behavior::{Births, Create, MailAddr, Never, ServiceSends, Step, User};

    struct Application;

    impl crate::BehaviorBase for Application {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Application {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Application {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<u8>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<()>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::new(
                vec![1],
                vec![Create::birth(7, ())],
                Step::Continue,
            ))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            let User { message, .. } = event;
            Ok(Actions::send(vec![message]))
        }
    }

    #[test]
    fn initialization_preserves_application_bootstrap_exactly() {
        let initialized = Guardian::new(Application).initialize().unwrap();
        assert_eq!(initialized.actions.sends, [1]);
        assert_eq!(initialized.actions.creates, [Create::birth(7, ())]);
        assert!(matches!(initialized.actions.become_, Step::Continue));
    }

    #[test]
    fn user_turns_delegate_and_shutdown_stops_without_policy_effects() {
        let mut active = Guardian::new(Application).initialize().unwrap().behavior;
        let user = active.receive(MailAddr(9), 3).unwrap();
        assert_eq!(user.sends, [3]);
        assert!(user.creates.is_empty());

        let stopped = active.on(ShutdownRequested).unwrap();
        assert!(stopped.sends.is_empty());
        assert!(stopped.creates.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "watch reactions have the wrapped behavior's fallible signature"
    )]
    fn continue_after_stop(
        _: &mut Application,
        _: MailAddr,
        _: &Result<Exit<MailAddr>, Crash>,
    ) -> Result<behavior::Become, Never> {
        Ok(Step::Continue)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "watch reactions have the wrapped behavior's fallible signature"
    )]
    fn continue_guardian_after_stop(
        _: &mut Guardian<Application>,
        _: MailAddr,
        _: &Result<Exit<MailAddr>, Crash>,
    ) -> Result<behavior::Become, Never> {
        Ok(Step::Continue)
    }

    #[test]
    fn guardian_and_watch_preserve_initialization_in_both_wrapper_orders() {
        let outer_watch = crate::Watch::new(
            Guardian::new(Application),
            MailAddr(4),
            continue_guardian_after_stop,
        )
        .initialize()
        .unwrap()
        .actions;
        assert_eq!(outer_watch.sends.behavior, [1]);
        assert_eq!(
            outer_watch.sends.observations,
            ServiceSends::one(crate::ObservePeer::new(MailAddr(4)))
        );
        assert_eq!(outer_watch.creates, [Create::birth(7, ())]);

        let outer_guardian = Guardian::new(crate::Watch::new(
            Application,
            MailAddr(4),
            continue_after_stop,
        ))
        .initialize()
        .unwrap()
        .actions;
        assert_eq!(
            outer_guardian.sends,
            WatchSends {
                behavior: vec![1],
                observations: ServiceSends::one(crate::ObservePeer::new(MailAddr(4))),
            }
        );
        assert_eq!(outer_guardian.creates, [Create::birth(7, ())]);
    }

    #[test]
    fn guardian_shutdown_routes_through_an_outer_watch_without_observing_a_peer_stop() {
        let mut active = crate::Watch::new(
            Guardian::new(Application),
            MailAddr(4),
            continue_guardian_after_stop,
        )
        .initialize()
        .unwrap()
        .behavior;

        let stopped = active.on(ShutdownRequested).unwrap();
        assert_eq!(stopped.sends, WatchSends::empty());
        assert!(stopped.creates.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));

        let mut peer_active = crate::Watch::new(
            Guardian::new(Application),
            MailAddr(4),
            continue_guardian_after_stop,
        )
        .initialize()
        .unwrap()
        .behavior;
        let peer = peer_active
            .transition(WatchEvent::PeerStopped(PeerStopped::new(
                MailAddr(4),
                Ok(Exit::Normal),
            )))
            .unwrap();
        assert!(matches!(peer.become_, Step::Continue));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Rejection {
        Initialization,
        Transition,
    }

    struct Rejecting {
        initialization: bool,
    }

    impl behavior::Protocol for Rejecting {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Rejecting {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Rejection;
        type Birth = behavior::NoBirths;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            if self.initialization {
                Err(Rejection::Initialization)
            } else {
                Ok(Actions::cont())
            }
        }

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Err(Rejection::Transition)
        }
    }

    #[test]
    fn guardian_preserves_controlled_inner_failures_exactly() {
        assert!(matches!(
            Guardian::new(Rejecting {
                initialization: true,
            })
            .initialize(),
            Err(Rejection::Initialization)
        ));

        let mut active = Guardian::new(Rejecting {
            initialization: false,
        })
        .initialize()
        .unwrap()
        .behavior;
        assert!(matches!(
            active.receive(MailAddr(0), ()),
            Err(Rejection::Transition)
        ));
    }
}
