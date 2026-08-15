//! Recipient-membership routing.

use core::num::NonZeroU16;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// One command accepted by [`Router`].
///
/// Membership changes are processed in mailbox order. `Route` transfers
/// ownership of one destination-protocol message to the router. Duplicate
/// members are inert and removal preserves the relative order of survivors.
#[derive(Clone, PartialEq, Eq)]
pub enum RouterMessage<D: Behavior, R: RoutingStrategy<D>> {
    /// Add one eligible recipient if it is not already present.
    Add(Recipient<D>),
    /// Remove one eligible recipient if present.
    Remove(Recipient<D>),
    /// Select recipient(s) and emit typed deliveries.
    Route(D::Msg),
    /// Deliver one statically selected policy observation.
    Observe(R::Observation),
}

/// A routing rejection that preserves the unaccepted payload.
///
/// Selection failure is ordinary typed behavior failure; it does not stop the
/// actor, mutate policy state, or ask the runtime to fabricate a recipient.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RouterError<M, E> {
    /// No recipient was eligible at the instant this command was folded.
    #[error("routing rejected because no recipient is eligible")]
    NoEligibleRecipients(M),
    /// The concrete policy rejected its typed observation atomically.
    #[error("routing policy rejected an observation")]
    Policy(E),
}

/// Static recipient-selection policy used by [`Router`].
///
/// Implementations receive only the current membership length and return
/// indices into that exact snapshot. Returning an out-of-range index is a
/// policy bug and is ignored rather than converted into an untyped runtime
/// lookup. Policies perform no effects and obtain no ambient entropy.
pub trait RoutingStrategy<D: Behavior> {
    /// Closed observation type accepted by this policy.
    type Observation;
    /// Concrete observation rejection.
    type Error;

    /// Select zero or more indices from this exact typed membership snapshot.
    fn select(&mut self, members: &[Recipient<D>], message: &D::Msg) -> Vec<usize>;

    /// Fold one typed observation against the same membership snapshot.
    ///
    /// # Errors
    ///
    /// Returns the concrete policy error without changing policy state when
    /// evidence is unknown, stale, or contradictory.
    fn observe(
        &mut self,
        _members: &[Recipient<D>],
        observation: Self::Observation,
    ) -> Result<(), Self::Error>;

    /// Update policy-local state after one new membership is committed.
    fn added(&mut self, _recipient: Recipient<D>) {}

    /// Repair policy-local position after a membership removal.
    fn removed(&mut self, _index: usize, _recipient: Recipient<D>, _remaining: usize) {}
}

/// Deterministic rotating single-recipient selection.
///
/// The cursor names the next position, wraps at the current membership size,
/// and is repaired after removal. This ordering is Bombay policy, not an actor
/// model guarantee.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoundRobin {
    next: usize,
}

impl<D: Behavior> RoutingStrategy<D> for RoundRobin {
    type Observation = Never;
    type Error = Never;

    fn select(&mut self, members: &[Recipient<D>], _: &D::Msg) -> Vec<usize> {
        if members.is_empty() {
            return Vec::new();
        }
        let selected = self.next % members.len();
        self.next = (selected + 1) % members.len();
        vec![selected]
    }

    fn observe(&mut self, _: &[Recipient<D>], observation: Never) -> Result<(), Never> {
        match observation {}
    }

    fn removed(&mut self, index: usize, _: Recipient<D>, remaining: usize) {
        if remaining == 0 {
            self.next = 0;
        } else {
            if index < self.next {
                self.next -= 1;
            }
            self.next %= remaining;
        }
    }
}

/// Deterministic selection of every current recipient in membership order.
///
/// Broadcasting clones the destination payload once per recipient. The
/// application-visible clone cost is explicit in the [`Behavior`] bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Broadcast;

impl<D: Behavior> RoutingStrategy<D> for Broadcast {
    type Observation = Never;
    type Error = Never;

    fn select(&mut self, members: &[Recipient<D>], _: &D::Msg) -> Vec<usize> {
        (0..members.len()).collect()
    }

    fn observe(&mut self, _: &[Recipient<D>], observation: Never) -> Result<(), Never> {
        match observation {}
    }
}

/// Monotonic version in one recipient's load-evidence stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadVersion(pub u64);

/// Comparable load value where lower is preferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Load(pub u64);

/// Explicit typed load evidence for [`LeastLoaded`].
pub struct LoadObservation<D: Behavior> {
    /// Recipient whose load was observed.
    pub recipient: Recipient<D>,
    /// Version within that recipient's evidence stream.
    pub version: LoadVersion,
    /// Point-in-time comparable load.
    pub load: Load,
}

/// Complete load-evidence phase for one eligible recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadEvidence {
    /// No evidence has been accepted; the recipient is not selectable.
    Unknown,
    /// Latest committed versioned load.
    Observed {
        /// Evidence version.
        version: LoadVersion,
        /// Comparable load.
        load: Load,
    },
}

struct RecipientLoad<D: Behavior> {
    recipient: Recipient<D>,
    evidence: LoadEvidence,
}

/// Rejected [`LeastLoaded`] evidence.
#[derive(Clone, PartialEq, Eq, Error)]
pub enum LeastLoadedError<D: Behavior> {
    /// Evidence names a recipient outside current membership.
    #[error("load evidence names an unknown recipient")]
    UnknownRecipient(LoadObservation<D>),
    /// Evidence predates the committed version.
    #[error("load evidence is stale")]
    Stale(LoadObservation<D>),
    /// Evidence contradicts the committed load at the same version.
    #[error("load evidence conflicts at the committed version")]
    ConflictingVersion(LoadObservation<D>),
}

impl<D: Behavior> core::fmt::Debug for LeastLoadedError<D> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRecipient(_) => "UnknownRecipient(..)",
            Self::Stale(_) => "Stale(..)",
            Self::ConflictingVersion(_) => "ConflictingVersion(..)",
        })
    }
}

impl<D: Behavior> Clone for LoadObservation<D> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient,
            version: self.version,
            load: self.load,
        }
    }
}

impl<D: Behavior> PartialEq for LoadObservation<D> {
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
            && self.version == other.version
            && self.load == other.load
    }
}

impl<D: Behavior> Eq for LoadObservation<D> {}

/// Deterministic selection of the lowest observed load.
///
/// Membership begins `Unknown` and is ineligible until typed versioned evidence
/// arrives. Ties use membership order. Unknown, stale, and same-version
/// conflicting evidence is rejected without mutation; identical evidence is
/// idempotent. Membership removal discards its evidence. These evidence and
/// tie rules are Bombay policy; gathering load remains an Environment concern.
pub struct LeastLoaded<D: Behavior> {
    loads: Vec<RecipientLoad<D>>,
}

impl<D: Behavior> LeastLoaded<D> {
    /// Construct a policy whose membership state is populated by [`Router`].
    #[must_use]
    pub const fn new() -> Self {
        Self { loads: Vec::new() }
    }

    /// Borrow one recipient's complete evidence phase.
    #[must_use]
    pub fn evidence(&self, recipient: Recipient<D>) -> Option<LoadEvidence> {
        self.loads
            .iter()
            .find(|entry| entry.recipient == recipient)
            .map(|entry| entry.evidence)
    }
}

impl<D: Behavior> Default for LeastLoaded<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Behavior> RoutingStrategy<D> for LeastLoaded<D> {
    type Observation = LoadObservation<D>;
    type Error = LeastLoadedError<D>;

    fn select(&mut self, members: &[Recipient<D>], _: &D::Msg) -> Vec<usize> {
        members
            .iter()
            .enumerate()
            .filter_map(|(index, recipient)| {
                self.evidence(*recipient)
                    .and_then(|evidence| match evidence {
                        LoadEvidence::Unknown => None,
                        LoadEvidence::Observed { load, .. } => Some((index, load)),
                    })
            })
            .min_by_key(|(index, load)| (*load, *index))
            .map_or_else(Vec::new, |(index, _)| vec![index])
    }

    fn observe(
        &mut self,
        members: &[Recipient<D>],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        if !members.contains(&observation.recipient) {
            return Err(LeastLoadedError::UnknownRecipient(observation));
        }
        let Some(entry) = self
            .loads
            .iter_mut()
            .find(|entry| entry.recipient == observation.recipient)
        else {
            return Err(LeastLoadedError::UnknownRecipient(observation));
        };
        let LoadEvidence::Observed { version, load } = entry.evidence else {
            entry.evidence = LoadEvidence::Observed {
                version: observation.version,
                load: observation.load,
            };
            return Ok(());
        };
        if observation.version < version {
            return Err(LeastLoadedError::Stale(observation));
        }
        if observation.version == version {
            return if observation.load == load {
                Ok(())
            } else {
                Err(LeastLoadedError::ConflictingVersion(observation))
            };
        }
        entry.evidence = LoadEvidence::Observed {
            version: observation.version,
            load: observation.load,
        };
        Ok(())
    }

    fn added(&mut self, recipient: Recipient<D>) {
        self.loads.push(RecipientLoad {
            recipient,
            evidence: LoadEvidence::Unknown,
        });
    }

    fn removed(&mut self, index: usize, _: Recipient<D>, _: usize) {
        if index < self.loads.len() {
            self.loads.remove(index);
        }
    }
}

/// Exposes the statically known routing key of one destination message.
pub trait RouteKey<K> {
    /// Borrow the key used only by the selected hash policy.
    fn route_key(&self) -> &K;
}

/// Stable Bombay-owned token for one routing member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberToken(pub u64);

/// Version within one member's stable-token evidence stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberTokenVersion(pub u64);

/// Typed versioned stable-token evidence for hash routing.
pub struct MemberTokenObservation<D: Behavior> {
    /// Eligible recipient described by the evidence.
    pub recipient: Recipient<D>,
    /// Evidence version.
    pub version: MemberTokenVersion,
    /// Stable policy token. It is not an actor identity or freshness proof.
    pub token: MemberToken,
}

impl<D: Behavior> Clone for MemberTokenObservation<D> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient,
            version: self.version,
            token: self.token,
        }
    }
}

impl<D: Behavior> PartialEq for MemberTokenObservation<D> {
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
            && self.version == other.version
            && self.token == other.token
    }
}

impl<D: Behavior> Eq for MemberTokenObservation<D> {}

/// Complete stable-token evidence phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberTokenEvidence {
    /// No stable token has been accepted; the member is ineligible.
    Unknown,
    /// Latest committed token evidence.
    Observed {
        /// Evidence version.
        version: MemberTokenVersion,
        /// Stable policy token.
        token: MemberToken,
    },
}

/// Rejected hash-membership evidence.
#[derive(Clone, PartialEq, Eq, Error)]
pub enum HashPolicyError<D: Behavior> {
    /// Evidence names a recipient outside current membership.
    #[error("hash-member evidence names an unknown recipient")]
    UnknownRecipient(MemberTokenObservation<D>),
    /// Evidence predates the current token version.
    #[error("hash-member evidence is stale")]
    Stale(MemberTokenObservation<D>),
    /// Evidence contradicts the token at the committed version.
    #[error("hash-member evidence conflicts at the committed version")]
    ConflictingVersion(MemberTokenObservation<D>),
}

impl<D: Behavior> core::fmt::Debug for HashPolicyError<D> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRecipient(_) => "UnknownRecipient(..)",
            Self::Stale(_) => "Stale(..)",
            Self::ConflictingVersion(_) => "ConflictingVersion(..)",
        })
    }
}

struct HashMember<D: Behavior> {
    recipient: Recipient<D>,
    evidence: MemberTokenEvidence,
}

struct HashMembership<D: Behavior> {
    members: Vec<HashMember<D>>,
}

impl<D: Behavior> HashMembership<D> {
    const fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    fn added(&mut self, recipient: Recipient<D>) {
        self.members.push(HashMember {
            recipient,
            evidence: MemberTokenEvidence::Unknown,
        });
    }

    fn removed(&mut self, index: usize) {
        if index < self.members.len() {
            self.members.remove(index);
        }
    }

    fn evidence(&self, recipient: Recipient<D>) -> Option<MemberTokenEvidence> {
        self.members
            .iter()
            .find(|member| member.recipient == recipient)
            .map(|member| member.evidence)
    }

    fn observe(
        &mut self,
        recipients: &[Recipient<D>],
        observation: MemberTokenObservation<D>,
    ) -> Result<(), HashPolicyError<D>> {
        if !recipients.contains(&observation.recipient) {
            return Err(HashPolicyError::UnknownRecipient(observation));
        }
        let Some(member) = self
            .members
            .iter_mut()
            .find(|member| member.recipient == observation.recipient)
        else {
            return Err(HashPolicyError::UnknownRecipient(observation));
        };
        let MemberTokenEvidence::Observed { version, token } = member.evidence else {
            member.evidence = MemberTokenEvidence::Observed {
                version: observation.version,
                token: observation.token,
            };
            return Ok(());
        };
        if observation.version < version {
            return Err(HashPolicyError::Stale(observation));
        }
        if observation.version == version {
            return if observation.token == token {
                Ok(())
            } else {
                Err(HashPolicyError::ConflictingVersion(observation))
            };
        }
        member.evidence = MemberTokenEvidence::Observed {
            version: observation.version,
            token: observation.token,
        };
        Ok(())
    }

    fn tokens(&self, recipients: &[Recipient<D>]) -> Vec<(usize, MemberToken)> {
        recipients
            .iter()
            .enumerate()
            .filter_map(|(index, recipient)| {
                self.evidence(*recipient)
                    .and_then(|evidence| match evidence {
                        MemberTokenEvidence::Unknown => None,
                        MemberTokenEvidence::Observed { token, .. } => Some((index, token)),
                    })
            })
            .collect()
    }
}

fn mixed_hash(left: u64, right: u64) -> u64 {
    let mut value = left ^ right.rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Stable ring selection over explicit member-token evidence.
///
/// Each eligible member contributes a positive fixed number of deterministic
/// virtual points. A key selects the first clockwise point, wrapping at the
/// ring end. Unknown members are ineligible; evidence rejection is atomic.
/// Token and key hashes are application/System facts supplied through concrete
/// functions and observations. Tokens are policy data, never actor identities
/// or freshness evidence. Ring mixing, clockwise tie order, and replica count
/// are deliberate Bombay policy; no external hash-routing crate is used.
pub struct ConsistentHash<D: Behavior, K> {
    membership: HashMembership<D>,
    replicas: NonZeroU16,
    hash_key: fn(&K) -> u64,
}

impl<D: Behavior, K> ConsistentHash<D, K> {
    /// Construct a stable-ring policy with explicit virtual-point count.
    #[must_use]
    pub const fn new(replicas: NonZeroU16, hash_key: fn(&K) -> u64) -> Self {
        Self {
            membership: HashMembership::new(),
            replicas,
            hash_key,
        }
    }

    /// Borrow one recipient's complete token-evidence phase.
    #[must_use]
    pub fn evidence(&self, recipient: Recipient<D>) -> Option<MemberTokenEvidence> {
        self.membership.evidence(recipient)
    }
}

impl<D, K> RoutingStrategy<D> for ConsistentHash<D, K>
where
    D: Behavior,
    D::Msg: RouteKey<K>,
{
    type Observation = MemberTokenObservation<D>;
    type Error = HashPolicyError<D>;

    fn select(&mut self, members: &[Recipient<D>], message: &D::Msg) -> Vec<usize> {
        let key = (self.hash_key)(message.route_key());
        self.membership
            .tokens(members)
            .into_iter()
            .flat_map(|(index, token)| {
                (0..self.replicas.get())
                    .map(move |replica| (mixed_hash(token.0, u64::from(replica)), index))
            })
            .min_by_key(|(point, index)| (*point < key, *point, *index))
            .map_or_else(Vec::new, |(_, index)| vec![index])
    }

    fn observe(
        &mut self,
        members: &[Recipient<D>],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, recipient: Recipient<D>) {
        self.membership.added(recipient);
    }

    fn removed(&mut self, index: usize, _: Recipient<D>, _: usize) {
        self.membership.removed(index);
    }
}

/// Highest-random-weight selection over explicit stable member tokens.
///
/// The deterministic score mixes the route-key hash with each eligible member
/// token and selects the greatest score, breaking ties by membership order.
/// Evidence and identity laws are the same as [`ConsistentHash`]. This is a
/// reviewed local algorithm so external crates cannot silently own Bombay's
/// membership or hash policy.
pub struct RendezvousHash<D: Behavior, K> {
    membership: HashMembership<D>,
    hash_key: fn(&K) -> u64,
}

impl<D: Behavior, K> RendezvousHash<D, K> {
    /// Construct a highest-random-weight policy.
    #[must_use]
    pub const fn new(hash_key: fn(&K) -> u64) -> Self {
        Self {
            membership: HashMembership::new(),
            hash_key,
        }
    }

    /// Borrow one recipient's complete token-evidence phase.
    #[must_use]
    pub fn evidence(&self, recipient: Recipient<D>) -> Option<MemberTokenEvidence> {
        self.membership.evidence(recipient)
    }
}

impl<D, K> RoutingStrategy<D> for RendezvousHash<D, K>
where
    D: Behavior,
    D::Msg: RouteKey<K>,
{
    type Observation = MemberTokenObservation<D>;
    type Error = HashPolicyError<D>;

    fn select(&mut self, members: &[Recipient<D>], message: &D::Msg) -> Vec<usize> {
        let key = (self.hash_key)(message.route_key());
        self.membership
            .tokens(members)
            .into_iter()
            .map(|(index, token)| (mixed_hash(key, token.0), index))
            .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)))
            .map_or_else(Vec::new, |(_, index)| vec![index])
    }

    fn observe(
        &mut self,
        members: &[Recipient<D>],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, recipient: Recipient<D>) {
        self.membership.added(recipient);
    }

    fn removed(&mut self, index: usize, _: Recipient<D>, _: usize) {
        self.membership.removed(index);
    }
}

/// A pure typed router over one concrete destination protocol.
///
/// State is the insertion-ordered recipient product plus a statically selected
/// policy. Inputs are [`RouterMessage`]; outputs are ordinary
/// [`Delivery<D>`] values. Initialization is empty. Successful membership
/// transitions emit no effects. A successful route emits deliveries in policy
/// order and continues; an empty selection returns [`RouterError`] without
/// changing membership. The router never terminates by policy and requires
/// only Bombay Address and Communication interpretation for its send lane.
pub struct Router<A: Address, D: Behavior<Addr = A>, R> {
    recipients: Vec<Recipient<D>>,
    strategy: R,
}

impl<A: Address, D: Behavior<Addr = A>, R: RoutingStrategy<D>> Router<A, D, R> {
    /// Construct a definition from explicit initial membership and policy.
    #[must_use]
    pub fn new(recipients: Vec<Recipient<D>>, strategy: R) -> Self {
        let mut unique = Vec::with_capacity(recipients.len());
        for recipient in recipients {
            if !unique.contains(&recipient) {
                unique.push(recipient);
            }
        }
        let mut strategy = strategy;
        for recipient in &unique {
            strategy.added(*recipient);
        }
        Self {
            recipients: unique,
            strategy,
        }
    }

    /// Current eligible recipients in observable routing order.
    #[must_use]
    pub fn recipients(&self) -> &[Recipient<D>] {
        &self.recipients
    }

    /// Borrow the concrete static policy state.
    #[must_use]
    pub const fn strategy(&self) -> &R {
        &self.strategy
    }
}

impl<A: Address, D: Behavior<Addr = A>, R: RoutingStrategy<D>> BehaviorBase for Router<A, D, R> {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, D, R> Behavior for Router<A, D, R>
where
    A: Address,
    D: Behavior<Addr = A>,
    D::Msg: Clone,
    R: RoutingStrategy<D>,
{
    type Addr = A;
    type Msg = RouterMessage<D, R>;
    type Event = User<A, RouterMessage<D, R>>;
    type Sends = Vec<Delivery<D>>;
    type Ph = Never;
    type Error = RouterError<D::Msg, R::Error>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            RouterMessage::Add(recipient) => {
                if !self.recipients.contains(&recipient) {
                    self.recipients.push(recipient);
                    self.strategy.added(recipient);
                }
                Ok(Actions::cont())
            }
            RouterMessage::Remove(recipient) => {
                if let Some(index) = self.recipients.iter().position(|item| *item == recipient) {
                    let removed = self.recipients.remove(index);
                    self.strategy.removed(index, removed, self.recipients.len());
                }
                Ok(Actions::cont())
            }
            RouterMessage::Route(message) => {
                let selected = self.strategy.select(&self.recipients, &message);
                let sends = selected
                    .into_iter()
                    .filter_map(|index| self.recipients.get(index).copied())
                    .map(|recipient| Delivery::new(recipient, message.clone()))
                    .collect::<Vec<_>>();
                if sends.is_empty() {
                    return Err(RouterError::NoEligibleRecipients(message));
                }
                Ok(Actions::send(sends))
            }
            RouterMessage::Observe(observation) => {
                self.strategy
                    .observe(&self.recipients, observation)
                    .map_err(RouterError::Policy)?;
                Ok(Actions::cont())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{MailAddr, Step};

    struct Destination;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct KeyedMessage {
        key: Key,
        value: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Key(u64);

    impl RouteKey<Key> for KeyedMessage {
        fn route_key(&self) -> &Key {
            &self.key
        }
    }

    struct KeyedDestination;

    impl Behavior for Destination {
        type Addr = MailAddr;
        type Msg = u8;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    impl Behavior for KeyedDestination {
        type Addr = MailAddr;
        type Msg = KeyedMessage;
        type Event = User<MailAddr, KeyedMessage>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[test]
    fn round_robin_repairs_cursor_after_removal() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let two = Recipient::<Destination>::global(MailAddr(2));
        let three = Recipient::<Destination>::global(MailAddr(3));
        let mut router = (Router::new(vec![one, two, three], RoundRobin::default()))
            .initialize()
            .unwrap()
            .behavior;

        let first = router
            .receive(MailAddr(9), RouterMessage::Route(7))
            .unwrap();
        assert!(first.sends == vec![Delivery::new(one, 7)]);
        assert!(matches!(first.become_, Step::Continue));

        router
            .receive(MailAddr(9), RouterMessage::Remove(one))
            .unwrap();
        let second = router
            .receive(MailAddr(9), RouterMessage::Route(8))
            .unwrap();
        assert!(second.sends == vec![Delivery::new(two, 8)]);
    }

    #[test]
    fn broadcast_preserves_membership_order_and_deduplicates() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let two = Recipient::<Destination>::global(MailAddr(2));
        let mut router = (Router::new(vec![one, two, one], Broadcast))
            .initialize()
            .unwrap()
            .behavior;

        let actions = router
            .receive(MailAddr(9), RouterMessage::Route(4))
            .unwrap();
        assert!(actions.sends == vec![Delivery::new(one, 4), Delivery::new(two, 4)]);
    }

    #[test]
    fn empty_membership_returns_the_owned_payload() {
        let mut router =
            (Router::<MailAddr, Destination, _>::new(Vec::new(), RoundRobin::default()))
                .initialize()
                .unwrap()
                .behavior;

        assert!(matches!(
            router.receive(MailAddr(9), RouterMessage::Route(11)),
            Err(RouterError::NoEligibleRecipients(11))
        ));
        assert!(router.recipients().is_empty());
    }

    #[test]
    fn least_loaded_requires_typed_evidence_and_breaks_ties_by_membership_order() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let two = Recipient::<Destination>::global(MailAddr(2));
        let mut router = (Router::new(vec![one, two], LeastLoaded::<Destination>::new()))
            .initialize()
            .unwrap()
            .behavior;

        assert!(matches!(
            router.receive(MailAddr(9), RouterMessage::Route(1)),
            Err(RouterError::NoEligibleRecipients(1))
        ));
        for recipient in [one, two] {
            router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(LoadObservation {
                        recipient,
                        version: LoadVersion(0),
                        load: Load(3),
                    }),
                )
                .unwrap();
        }
        let tied = router
            .receive(MailAddr(9), RouterMessage::Route(2))
            .unwrap();
        assert!(tied.sends == vec![Delivery::new(one, 2)]);

        router
            .receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: two,
                    version: LoadVersion(1),
                    load: Load(1),
                }),
            )
            .unwrap();
        let selected = router
            .receive(MailAddr(9), RouterMessage::Route(3))
            .unwrap();
        assert!(selected.sends == vec![Delivery::new(two, 3)]);
    }

    #[test]
    fn least_loaded_rejects_stale_and_unknown_evidence_without_mutation() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let unknown = Recipient::<Destination>::global(MailAddr(8));
        let mut router = (Router::new(vec![one], LeastLoaded::<Destination>::new()))
            .initialize()
            .unwrap()
            .behavior;
        router
            .receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: one,
                    version: LoadVersion(2),
                    load: Load(4),
                }),
            )
            .unwrap();

        assert!(matches!(
            router.receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: one,
                    version: LoadVersion(1),
                    load: Load(0),
                })
            ),
            Err(RouterError::Policy(LeastLoadedError::Stale(_)))
        ));
        assert!(matches!(
            router.receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: one,
                    version: LoadVersion(2),
                    load: Load(5),
                })
            ),
            Err(RouterError::Policy(LeastLoadedError::ConflictingVersion(_)))
        ));
        assert!(matches!(
            router.receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: unknown,
                    version: LoadVersion(0),
                    load: Load(0),
                })
            ),
            Err(RouterError::Policy(LeastLoadedError::UnknownRecipient(_)))
        ));
        assert_eq!(
            router.strategy().evidence(one),
            Some(LoadEvidence::Observed {
                version: LoadVersion(2),
                load: Load(4)
            })
        );
    }

    fn identity_hash(key: &Key) -> u64 {
        key.0
    }

    #[test]
    fn consistent_hash_removal_moves_only_keys_owned_by_the_removed_member() {
        let members = [
            Recipient::<KeyedDestination>::global(MailAddr(1)),
            Recipient::<KeyedDestination>::global(MailAddr(2)),
            Recipient::<KeyedDestination>::global(MailAddr(3)),
        ];
        let mut router = (Router::new(
            members.to_vec(),
            ConsistentHash::new(NonZeroU16::new(8).unwrap(), identity_hash),
        ))
        .initialize()
        .unwrap()
        .behavior;
        for (index, recipient) in members.into_iter().enumerate() {
            router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(MemberTokenObservation {
                        recipient,
                        version: MemberTokenVersion(0),
                        token: MemberToken(u64::try_from(index + 1).unwrap()),
                    }),
                )
                .unwrap();
        }
        let before = (0..128_u64)
            .map(|key| {
                router
                    .receive(
                        MailAddr(9),
                        RouterMessage::Route(KeyedMessage {
                            key: Key(key),
                            value: 1,
                        }),
                    )
                    .unwrap()
                    .sends[0]
                    .to
            })
            .collect::<Vec<_>>();
        router
            .receive(MailAddr(9), RouterMessage::Remove(members[1]))
            .unwrap();
        for (key, previous) in before.into_iter().enumerate() {
            let current = router
                .receive(
                    MailAddr(9),
                    RouterMessage::Route(KeyedMessage {
                        key: Key(u64::try_from(key).unwrap()),
                        value: 1,
                    }),
                )
                .unwrap()
                .sends[0]
                .to;
            if previous != members[1] {
                assert!(current == previous);
            }
        }
    }

    #[test]
    fn rendezvous_hash_is_deterministic_and_rejects_conflicting_tokens() {
        let one = Recipient::<KeyedDestination>::global(MailAddr(1));
        let two = Recipient::<KeyedDestination>::global(MailAddr(2));
        let mut router = (Router::new(vec![one, two], RendezvousHash::new(identity_hash)))
            .initialize()
            .unwrap()
            .behavior;
        for (recipient, token) in [(one, 11), (two, 22)] {
            router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(MemberTokenObservation {
                        recipient,
                        version: MemberTokenVersion(0),
                        token: MemberToken(token),
                    }),
                )
                .unwrap();
        }
        let first = router
            .receive(
                MailAddr(9),
                RouterMessage::Route(KeyedMessage {
                    key: Key(7),
                    value: 1,
                }),
            )
            .unwrap()
            .sends[0]
            .to;
        let again = router
            .receive(
                MailAddr(9),
                RouterMessage::Route(KeyedMessage {
                    key: Key(7),
                    value: 2,
                }),
            )
            .unwrap()
            .sends[0]
            .to;
        assert!(first == again);
        assert!(matches!(
            router.receive(
                MailAddr(9),
                RouterMessage::Observe(MemberTokenObservation {
                    recipient: one,
                    version: MemberTokenVersion(0),
                    token: MemberToken(99),
                })
            ),
            Err(RouterError::Policy(HashPolicyError::ConflictingVersion(_)))
        ));
        assert_eq!(
            router.strategy().evidence(one),
            Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(0),
                token: MemberToken(11)
            })
        );
    }
}
