use std::collections::VecDeque;

use behavior::{
    Actions, Behavior, BehaviorActed, BehaviorAddr, BehaviorMessage, Creations, Never, Protocol,
    SendEffects, SendsFor, Step, User, UserEvent,
};

use crate::composition::DeliveryRoute;

use super::{StashRoute, StashStatus, StaticallyInfallible, UnlimitedStash};

/// Inner actor effects followed by complete original inputs refused by a full stash.
///
/// Both lanes retain the actor's existing event algebra. Interpretation exhausts
/// `actor` before `refusals`, preserving each lane's order and terminal custody.
#[derive(behavior_macros::SendProduct)]
pub struct StashSends<ActorSends, Refusals> {
    pub actor: ActorSends,
    pub refusals: Refusals,
}

/// A finite FIFO of original mailbox inputs around an infallible actor.
///
/// Only inputs selected by [`StashRoute::Stash`] occupy slots. When full, the
/// original [`User`] moves to the configured refusal recipient through Actions;
/// the inner actor and held FIFO stay unchanged and this actor continues.
/// Zero refuses every newly selected held input. Deliver, release and non-user
/// events remain available at capacity. Replay preserves successful effects
/// before the first stop and retains the untouched suffix.
///
/// The limit counts held inputs, not bytes, allocation capacity, runtime mailbox
/// entries or queues owned by an inner actor. The selector is pure. The inner
/// actor must have a closed phase and statically uninhabited transition error:
/// replay cannot undo effects from earlier inputs. Construction performs no
/// initialization, transition or runtime operation.
pub struct Stash<B: Behavior, R> {
    queue: UnlimitedStash<B>,
    limit: usize,
    refusals: R,
}

impl<B: Behavior, R> Stash<B, R> {
    /// Move all held original inputs out in FIFO order, leaving the stash empty.
    ///
    /// The inner actor, selector, limit and refusal recipient remain unchanged.
    /// This does not replay inputs, clone messages, emit effects or take unread
    /// runtime mailbox entries. Repeating the operation returns an empty queue.
    #[must_use]
    pub fn take(&mut self) -> VecDeque<User<BehaviorAddr<B>, BehaviorMessage<B>>> {
        self.queue.take()
    }
}

impl<B: Behavior<Ph = Never>, R> Stash<B, R> {
    /// Start an empty finite stash with an explicit refusal recipient.
    ///
    /// The recipient receives the complete original User, including its sender.
    /// Its capability is cloned for repeatable delivery; inputs and the actor
    /// need no Clone implementation. This is not a conversion of a populated
    /// queue. Use [`UnlimitedStash::new`] for deliberate unbounded retention.
    #[must_use]
    pub fn new(
        inner: B,
        limit: usize,
        refusals: R,
        route: fn(&B, &BehaviorMessage<B>) -> StashRoute,
    ) -> Self
    where
        B::Error: StaticallyInfallible,
        R: DeliveryRoute + Clone,
        R::Protocol:
            Protocol<Addr = BehaviorAddr<B>, Msg = User<BehaviorAddr<B>, BehaviorMessage<B>>>,
        R::Sends: SendsFor<B::Event>,
    {
        Self {
            queue: UnlimitedStash::new(inner, route),
            limit,
            refusals,
        }
    }

    /// Number of original inputs currently held by this stash.
    #[must_use]
    pub fn held(&self) -> usize {
        self.queue.held()
    }
}

impl<B: Behavior<Ph = Never>, R> StashStatus for Stash<B, R> {
    fn stashed_messages(&self) -> usize {
        self.held()
    }
}

impl<B: Behavior<Ph = Never> + behavior::BehaviorBase, R> behavior::BehaviorBase for Stash<B, R> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.queue.base()
    }
}

impl<B, R> Behavior for Stash<B, R>
where
    B: Behavior<Ph = Never>,
    B::Error: StaticallyInfallible,
    R: DeliveryRoute + Clone,
    R::Protocol: Protocol<Addr = BehaviorAddr<B>, Msg = User<BehaviorAddr<B>, BehaviorMessage<B>>>,
    R::Sends: SendsFor<B::Event>,
{
    type Protocol = B::Protocol;
    type Event = B::Event;
    type Sends = StashSends<B::Sends, R::Sends>;
    type Ph = Never;
    type Error = B::Error;
    type Birth = B::Birth;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.queue)?;
        Ok(actions.map_sends(|actor| StashSends {
            actor,
            refusals: R::Sends::empty(),
        }))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let actions = match event.into_user() {
            Ok(user) => {
                let route = self.queue.route(&user.message);
                if matches!(route, StashRoute::Stash) && self.held() >= self.limit {
                    return Ok(Actions::new(
                        StashSends {
                            actor: B::Sends::empty(),
                            refusals: self.refusals.clone().deliver(user),
                        },
                        Creations::empty(),
                        Step::Continue,
                    ));
                }
                self.queue.receive(user, route)
            }
            Err(other) => behavior::delegate_transition(&mut self.queue, other),
        }?;
        Ok(actions.map_sends(|actor| StashSends {
            actor,
            refusals: R::Sends::empty(),
        }))
    }
}
