//! Recipient-membership routing.

use core::num::NonZeroU16;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol,
    SendEffects, User,
};
use thiserror::Error;

use crate::DeliveryRoute;

/// One command accepted by [`Router`].
///
/// Membership changes are processed in mailbox order. `Route` transfers
/// ownership of one destination-protocol message to the router. Duplicate
/// members are inert and removal preserves the relative order of survivors.
pub enum RouterMessage<Route: DeliveryRoute + Clone + PartialEq, R: RoutingStrategy<Route>> {
    /// Add one eligible recipient if it is not already present.
    Add(Route),
    /// Remove one eligible recipient if present.
    Remove(Route),
    /// Select recipient(s) and emit typed deliveries.
    Route(<Route::Protocol as Protocol>::Msg),
    /// Deliver one statically selected policy observation.
    Observe(R::Observation),
}

/// A routing rejection that preserves the unaccepted payload.
///
/// Selection failure is ordinary typed behavior failure; it does not stop the
/// actor, mutate policy state, or ask the runtime to fabricate a recipient.
#[derive(Error, Clone, PartialEq, Eq)]
pub enum RouterError<M, O, E> {
    /// No recipient was eligible at the instant this command was folded.
    #[error("routing rejected because no recipient is eligible")]
    NoEligibleRecipients(M),
    /// The selected policy returned an index outside the exact membership
    /// snapshot it received. The command and policy state remain unconsumed.
    #[error("routing policy selected index {index} from {members} members")]
    InvalidSelection {
        /// Unaccepted destination command.
        message: M,
        /// Invalid index returned by the policy.
        index: usize,
        /// Size of the membership snapshot supplied to the policy.
        members: usize,
    },
    /// The concrete policy rejected its typed observation atomically.
    #[error("routing policy rejected an observation")]
    Policy {
        /// Exact observation rejected by the policy.
        observation: O,
        /// Concrete policy reason.
        error: E,
    },
}

impl<M: core::fmt::Debug, O, E: core::fmt::Debug> core::fmt::Debug for RouterError<M, O, E> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoEligibleRecipients(message) => formatter
                .debug_tuple("NoEligibleRecipients")
                .field(message)
                .finish(),
            Self::InvalidSelection {
                message,
                index,
                members,
            } => formatter
                .debug_struct("InvalidSelection")
                .field("message", message)
                .field("index", index)
                .field("members", members)
                .finish(),
            Self::Policy { error, .. } => formatter
                .debug_struct("Policy")
                .field("observation", &"<retained>")
                .field("error", error)
                .finish(),
        }
    }
}

/// Static recipient-selection policy used by [`Router`].
///
/// Implementations receive only the current membership length and return
/// indices into that exact snapshot. Returning an out-of-range index is a
/// typed [`RouterError::InvalidSelection`]; the command and a cloned policy
/// candidate are returned without committing policy state. Policies perform
/// no effects and obtain no ambient entropy.
pub trait RoutingStrategy<Route: DeliveryRoute + Clone + PartialEq>: Clone {
    /// Closed observation type accepted by this policy.
    type Observation;
    /// Concrete observation rejection.
    type Error;

    /// Select zero or more indices from this exact typed membership snapshot.
    fn select(
        &mut self,
        members: &[Route],
        message: &<Route::Protocol as Protocol>::Msg,
    ) -> Vec<usize>;

    /// Fold one typed observation against the same membership snapshot.
    ///
    /// # Errors
    ///
    /// Returns the concrete policy error without changing policy state when
    /// evidence is unknown, stale, or contradictory.
    fn observe(
        &mut self,
        _members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error>;

    /// Update policy-local state after one new membership is committed.
    fn added(&mut self, _recipient: Route) {}

    /// Repair policy-local position after a membership removal.
    fn removed(&mut self, _index: usize, _recipient: Route, _remaining: usize) {}
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

impl<Route: DeliveryRoute + Clone + PartialEq> RoutingStrategy<Route> for RoundRobin {
    type Observation = Never;
    type Error = Never;

    fn select(&mut self, members: &[Route], _: &<Route::Protocol as Protocol>::Msg) -> Vec<usize> {
        if members.is_empty() {
            return Vec::new();
        }
        let selected = self.next % members.len();
        self.next = (selected + 1) % members.len();
        vec![selected]
    }

    fn observe(&mut self, _: &[Route], observation: Never) -> Result<(), Never> {
        match observation {}
    }

    fn removed(&mut self, index: usize, _: Route, remaining: usize) {
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

impl<Route: DeliveryRoute + Clone + PartialEq> RoutingStrategy<Route> for Broadcast {
    type Observation = Never;
    type Error = Never;

    fn select(&mut self, members: &[Route], _: &<Route::Protocol as Protocol>::Msg) -> Vec<usize> {
        (0..members.len()).collect()
    }

    fn observe(&mut self, _: &[Route], observation: Never) -> Result<(), Never> {
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
pub struct LoadObservation<Route: DeliveryRoute + Clone + PartialEq> {
    /// Recipient whose load was observed.
    pub recipient: Route,
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

struct RecipientLoad<Route: DeliveryRoute + Clone + PartialEq> {
    recipient: Route,
    evidence: LoadEvidence,
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for RecipientLoad<Route> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient.clone(),
            evidence: self.evidence,
        }
    }
}

/// Rejected [`LeastLoaded`] evidence.
#[derive(Clone, PartialEq, Eq, Error)]
pub enum LeastLoadedError<Route: DeliveryRoute + Clone + PartialEq> {
    /// Evidence names a recipient outside current membership.
    #[error("load evidence names an unknown recipient")]
    UnknownRecipient(LoadObservation<Route>),
    /// Evidence predates the committed version.
    #[error("load evidence is stale")]
    Stale(LoadObservation<Route>),
    /// Evidence contradicts the committed load at the same version.
    #[error("load evidence conflicts at the committed version")]
    ConflictingVersion(LoadObservation<Route>),
}

impl<Route: DeliveryRoute + Clone + PartialEq> core::fmt::Debug for LeastLoadedError<Route> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRecipient(_) => "UnknownRecipient(..)",
            Self::Stale(_) => "Stale(..)",
            Self::ConflictingVersion(_) => "ConflictingVersion(..)",
        })
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for LoadObservation<Route> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient.clone(),
            version: self.version,
            load: self.load,
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> PartialEq for LoadObservation<Route> {
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
            && self.version == other.version
            && self.load == other.load
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> Eq for LoadObservation<Route> {}

/// Deterministic selection of the lowest observed load.
///
/// Membership begins `Unknown` and is ineligible until typed versioned evidence
/// arrives. Ties use membership order. Unknown, stale, and same-version
/// conflicting evidence is rejected without mutation; identical evidence is
/// idempotent. Membership removal discards its evidence. These evidence and
/// tie rules are Bombay policy; gathering load remains an Environment concern.
pub struct LeastLoaded<Route: DeliveryRoute + Clone + PartialEq> {
    loads: Vec<RecipientLoad<Route>>,
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for LeastLoaded<Route> {
    fn clone(&self) -> Self {
        Self {
            loads: self.loads.clone(),
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> LeastLoaded<Route> {
    /// Construct a policy whose membership state is populated by [`Router`].
    #[must_use]
    pub const fn new() -> Self {
        Self { loads: Vec::new() }
    }

    /// Borrow one recipient's complete evidence phase.
    #[must_use]
    pub fn evidence(&self, recipient: Route) -> Option<LoadEvidence> {
        self.loads
            .iter()
            .find(|entry| entry.recipient == recipient)
            .map(|entry| entry.evidence)
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> Default for LeastLoaded<Route> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> RoutingStrategy<Route> for LeastLoaded<Route> {
    type Observation = LoadObservation<Route>;
    type Error = LeastLoadedError<Route>;

    fn select(&mut self, members: &[Route], _: &<Route::Protocol as Protocol>::Msg) -> Vec<usize> {
        members
            .iter()
            .enumerate()
            .filter_map(|(index, recipient)| {
                self.evidence(recipient.clone())
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
        members: &[Route],
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

    fn added(&mut self, recipient: Route) {
        self.loads.push(RecipientLoad {
            recipient,
            evidence: LoadEvidence::Unknown,
        });
    }

    fn removed(&mut self, index: usize, _: Route, _: usize) {
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
pub struct MemberTokenObservation<Route: DeliveryRoute + Clone + PartialEq> {
    /// Eligible recipient described by the evidence.
    pub recipient: Route,
    /// Evidence version.
    pub version: MemberTokenVersion,
    /// Stable policy token. It is not an actor identity or freshness proof.
    pub token: MemberToken,
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for MemberTokenObservation<Route> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient.clone(),
            version: self.version,
            token: self.token,
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> PartialEq for MemberTokenObservation<Route> {
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
            && self.version == other.version
            && self.token == other.token
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> Eq for MemberTokenObservation<Route> {}

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
pub enum HashPolicyError<Route: DeliveryRoute + Clone + PartialEq> {
    /// Evidence names a recipient outside current membership.
    #[error("hash-member evidence names an unknown recipient")]
    UnknownRecipient(MemberTokenObservation<Route>),
    /// Evidence predates the current token version.
    #[error("hash-member evidence is stale")]
    Stale(MemberTokenObservation<Route>),
    /// Evidence contradicts the token at the committed version.
    #[error("hash-member evidence conflicts at the committed version")]
    ConflictingVersion(MemberTokenObservation<Route>),
}

impl<Route: DeliveryRoute + Clone + PartialEq> core::fmt::Debug for HashPolicyError<Route> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRecipient(_) => "UnknownRecipient(..)",
            Self::Stale(_) => "Stale(..)",
            Self::ConflictingVersion(_) => "ConflictingVersion(..)",
        })
    }
}

struct HashMember<Route: DeliveryRoute + Clone + PartialEq> {
    recipient: Route,
    evidence: MemberTokenEvidence,
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for HashMember<Route> {
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient.clone(),
            evidence: self.evidence,
        }
    }
}

struct HashMembership<Route: DeliveryRoute + Clone + PartialEq> {
    members: Vec<HashMember<Route>>,
}

impl<Route: DeliveryRoute + Clone + PartialEq> Clone for HashMembership<Route> {
    fn clone(&self) -> Self {
        Self {
            members: self.members.clone(),
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> HashMembership<Route> {
    const fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    fn added(&mut self, recipient: Route) {
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

    fn evidence(&self, recipient: Route) -> Option<MemberTokenEvidence> {
        self.members
            .iter()
            .find(|member| member.recipient == recipient)
            .map(|member| member.evidence)
    }

    fn observe(
        &mut self,
        recipients: &[Route],
        observation: MemberTokenObservation<Route>,
    ) -> Result<(), HashPolicyError<Route>> {
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

    fn tokens(&self, recipients: &[Route]) -> Vec<(usize, MemberToken)> {
        recipients
            .iter()
            .enumerate()
            .filter_map(|(index, recipient)| {
                self.evidence(recipient.clone())
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
pub struct ConsistentHash<Route: DeliveryRoute + Clone + PartialEq, K> {
    membership: HashMembership<Route>,
    replicas: NonZeroU16,
    hash_key: fn(&K) -> u64,
}

impl<Route: DeliveryRoute + Clone + PartialEq, K> Clone for ConsistentHash<Route, K> {
    fn clone(&self) -> Self {
        Self {
            membership: self.membership.clone(),
            replicas: self.replicas,
            hash_key: self.hash_key,
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq, K> ConsistentHash<Route, K> {
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
    pub fn evidence(&self, recipient: Route) -> Option<MemberTokenEvidence> {
        self.membership.evidence(recipient)
    }
}

impl<Route, K> RoutingStrategy<Route> for ConsistentHash<Route, K>
where
    Route: DeliveryRoute + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: RouteKey<K>,
{
    type Observation = MemberTokenObservation<Route>;
    type Error = HashPolicyError<Route>;

    fn select(
        &mut self,
        members: &[Route],
        message: &<Route::Protocol as Protocol>::Msg,
    ) -> Vec<usize> {
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
        members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, recipient: Route) {
        self.membership.added(recipient);
    }

    fn removed(&mut self, index: usize, _: Route, _: usize) {
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
pub struct RendezvousHash<Route: DeliveryRoute + Clone + PartialEq, K> {
    membership: HashMembership<Route>,
    hash_key: fn(&K) -> u64,
}

impl<Route: DeliveryRoute + Clone + PartialEq, K> Clone for RendezvousHash<Route, K> {
    fn clone(&self) -> Self {
        Self {
            membership: self.membership.clone(),
            hash_key: self.hash_key,
        }
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq, K> RendezvousHash<Route, K> {
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
    pub fn evidence(&self, recipient: Route) -> Option<MemberTokenEvidence> {
        self.membership.evidence(recipient)
    }
}

impl<Route, K> RoutingStrategy<Route> for RendezvousHash<Route, K>
where
    Route: DeliveryRoute + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: RouteKey<K>,
{
    type Observation = MemberTokenObservation<Route>;
    type Error = HashPolicyError<Route>;

    fn select(
        &mut self,
        members: &[Route],
        message: &<Route::Protocol as Protocol>::Msg,
    ) -> Vec<usize> {
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
        members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, recipient: Route) {
        self.membership.added(recipient);
    }

    fn removed(&mut self, index: usize, _: Route, _: usize) {
        self.membership.removed(index);
    }
}

/// A pure typed router over one concrete destination protocol.
///
/// State is the insertion-ordered recipient product plus a statically selected
/// policy. Inputs are [`RouterMessage`]; outputs are the concrete send product
/// selected by `Route`. Initialization is empty. Successful membership
/// transitions emit no effects. A successful route emits deliveries in policy
/// order and continues; an empty selection returns [`RouterError`] without
/// changing membership. The router never terminates by policy and requires
/// only Bombay Address and Communication interpretation for its send lane.
pub struct Router<
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    R,
> {
    recipients: Vec<Route>,
    strategy: R,
}

impl<A, Route, R> Router<A, Route, R>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    R: RoutingStrategy<Route>,
{
    /// Construct a definition from explicit initial membership and policy.
    #[must_use]
    pub fn new(recipients: Vec<Route>, strategy: R) -> Self {
        let mut unique = Vec::with_capacity(recipients.len());
        for recipient in recipients {
            if !unique.contains(&recipient) {
                unique.push(recipient);
            }
        }
        let mut strategy = strategy;
        for recipient in &unique {
            strategy.added(recipient.clone());
        }
        Self {
            recipients: unique,
            strategy,
        }
    }

    /// Current eligible recipients in observable routing order.
    #[must_use]
    pub fn recipients(&self) -> &[Route] {
        &self.recipients
    }

    /// Borrow the concrete static policy state.
    #[must_use]
    pub const fn strategy(&self) -> &R {
        &self.strategy
    }
}

impl<A, Route, R> BehaviorBase for Router<A, Route, R>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    R: RoutingStrategy<Route>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, Route, R> behavior::Protocol for Router<A, Route, R>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: Clone,
    R: RoutingStrategy<Route>,
    R::Observation: Clone,
{
    type Addr = A;
    type Msg = RouterMessage<Route, R>;
}

impl<A, Route, R> Behavior for Router<A, Route, R>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: Clone,
    R: RoutingStrategy<Route>,
    R::Observation: Clone,
    Route::Sends: behavior::SendsFor<User<A, RouterMessage<Route, R>>>,
{
    type Protocol = Self;
    type Event = User<A, RouterMessage<Route, R>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = RouterError<<Route::Protocol as Protocol>::Msg, R::Observation, R::Error>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            RouterMessage::Add(recipient) => {
                if !self.recipients.contains(&recipient) {
                    self.recipients.push(recipient.clone());
                    self.strategy.added(recipient);
                }
                Ok(Actions::cont())
            }
            RouterMessage::Remove(recipient) => {
                if let Some(index) = self.recipients.iter().position(|item| item == &recipient) {
                    let removed = self.recipients.remove(index);
                    self.strategy.removed(index, removed, self.recipients.len());
                }
                Ok(Actions::cont())
            }
            RouterMessage::Route(message) => {
                let mut strategy = self.strategy.clone();
                let selected = strategy.select(&self.recipients, &message);
                if let Some(index) = selected
                    .iter()
                    .copied()
                    .find(|index| *index >= self.recipients.len())
                {
                    return Err(RouterError::InvalidSelection {
                        message,
                        index,
                        members: self.recipients.len(),
                    });
                }
                if selected.is_empty() {
                    return Err(RouterError::NoEligibleRecipients(message));
                }
                let mut sends = Route::Sends::empty();
                for index in selected {
                    sends.append(self.recipients[index].clone().deliver(message.clone()));
                }
                self.strategy = strategy;
                Ok(Actions::send(sends))
            }
            RouterMessage::Observe(observation) => {
                let mut strategy = self.strategy.clone();
                let retained = observation.clone();
                strategy
                    .observe(&self.recipients, observation)
                    .map_err(|error| RouterError::Policy {
                        observation: retained,
                        error,
                    })?;
                self.strategy = strategy;
                Ok(Actions::cont())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{Delivery, MailAddr, Recipient, Step};

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

    impl behavior::Protocol for Destination {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Destination {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    impl behavior::Protocol for KeyedDestination {
        type Addr = MailAddr;
        type Msg = KeyedMessage;
    }

    impl Behavior for KeyedDestination {
        type Protocol = Self;
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
            (Router::<MailAddr, Recipient<Destination>, _>::new(Vec::new(), RoundRobin::default()))
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
        let mut router =
            (Router::new(vec![one, two], LeastLoaded::<Recipient<Destination>>::new()))
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
        let mut router = (Router::new(vec![one], LeastLoaded::<Recipient<Destination>>::new()))
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
            Err(RouterError::Policy {
                error: LeastLoadedError::Stale(_),
                ..
            })
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
            Err(RouterError::Policy {
                error: LeastLoadedError::ConflictingVersion(_),
                ..
            })
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
            Err(RouterError::Policy {
                error: LeastLoadedError::UnknownRecipient(_),
                ..
            })
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
            Err(RouterError::Policy {
                error: HashPolicyError::ConflictingVersion(_),
                ..
            })
        ));
        assert_eq!(
            router.strategy().evidence(one),
            Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(0),
                token: MemberToken(11)
            })
        );
    }

    #[derive(Clone, Default)]
    struct RejectAfterMutation {
        selections: usize,
        observations: usize,
    }

    impl RoutingStrategy<Recipient<Destination>> for RejectAfterMutation {
        type Observation = u8;
        type Error = u8;

        fn select(&mut self, members: &[Recipient<Destination>], _: &u8) -> Vec<usize> {
            self.selections += 1;
            vec![members.len()]
        }

        fn observe(
            &mut self,
            _: &[Recipient<Destination>],
            observation: Self::Observation,
        ) -> Result<(), Self::Error> {
            self.observations += 1;
            Err(observation)
        }
    }

    #[test]
    fn rejected_policy_turns_preserve_the_command_and_policy_snapshot() {
        let member = Recipient::<Destination>::global(MailAddr(1));
        let mut router = Router::new(vec![member], RejectAfterMutation::default())
            .initialize()
            .unwrap()
            .behavior;

        assert!(matches!(
            router.receive(MailAddr(9), RouterMessage::Route(42)),
            Err(RouterError::InvalidSelection {
                message: 42,
                index: 1,
                members: 1,
            })
        ));
        assert_eq!(router.strategy().selections, 0);

        assert!(matches!(
            router.receive(MailAddr(9), RouterMessage::Observe(7)),
            Err(RouterError::Policy {
                observation: 7,
                error: 7,
            })
        ));
        assert_eq!(router.strategy().observations, 0);
    }
}
