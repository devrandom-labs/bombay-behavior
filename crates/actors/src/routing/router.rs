//! Recipient-membership routing.

use core::num::NonZeroU16;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol, User,
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
/// Implementations receive the current membership snapshot and return at most
/// one index into it. Returning an out-of-range index is a typed
/// [`RouterError::InvalidSelection`]; the command is returned and the cloned
/// policy candidate is discarded without committing policy state. Policies
/// perform no effects and obtain no ambient entropy.
pub trait RoutingStrategy<Route: DeliveryRoute + Clone + PartialEq>: Clone {
    /// Closed observation type accepted by this policy.
    type Observation;
    /// Concrete observation rejection.
    type Error;

    /// Select at most one index from this exact typed membership snapshot.
    ///
    /// Returning `None` means no member is currently eligible. The selected
    /// route receives ownership of the message, so unicast routing does not
    /// require the destination protocol's message to implement [`Clone`].
    fn select(
        &mut self,
        members: &[Route],
        message: &<Route::Protocol as Protocol>::Msg,
    ) -> Option<usize>;

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

    fn select(
        &mut self,
        members: &[Route],
        _: &<Route::Protocol as Protocol>::Msg,
    ) -> Option<usize> {
        if members.is_empty() {
            return None;
        }
        let selected = self.next % members.len();
        self.next = (selected + 1) % members.len();
        Some(selected)
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
/// Evidence follows the Router's member order; only Router owns recipient
/// identity and exposes recipient-based lookup through [`Router::load_evidence`].
#[derive(Clone)]
pub struct LeastLoaded {
    loads: Vec<LoadEvidence>,
}

impl LeastLoaded {
    /// Construct a policy whose membership state is populated by [`Router`].
    #[must_use]
    pub const fn new() -> Self {
        Self { loads: Vec::new() }
    }
}

impl Default for LeastLoaded {
    fn default() -> Self {
        Self::new()
    }
}

impl<Route: DeliveryRoute + Clone + PartialEq> RoutingStrategy<Route> for LeastLoaded {
    type Observation = LoadObservation<Route>;
    type Error = LeastLoadedError<Route>;

    fn select(
        &mut self,
        members: &[Route],
        _: &<Route::Protocol as Protocol>::Msg,
    ) -> Option<usize> {
        self.loads
            .iter()
            .enumerate()
            .take(members.len())
            .filter_map(|(index, evidence)| match evidence {
                LoadEvidence::Unknown => None,
                LoadEvidence::Observed { load, .. } => Some((index, *load)),
            })
            .min_by_key(|(index, load)| (*load, *index))
            .map(|(index, _)| index)
    }

    fn observe(
        &mut self,
        members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        let Some(index) = members
            .iter()
            .position(|member| member == &observation.recipient)
        else {
            return Err(LeastLoadedError::UnknownRecipient(observation));
        };
        let Some(evidence) = self.loads.get_mut(index) else {
            return Err(LeastLoadedError::UnknownRecipient(observation));
        };
        let LoadEvidence::Observed { version, load } = *evidence else {
            *evidence = LoadEvidence::Observed {
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
        *evidence = LoadEvidence::Observed {
            version: observation.version,
            load: observation.load,
        };
        Ok(())
    }

    fn added(&mut self, _: Route) {
        self.loads.push(LoadEvidence::Unknown);
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

#[derive(Clone)]
struct HashMembership {
    evidence: Vec<MemberTokenEvidence>,
}

impl HashMembership {
    const fn new() -> Self {
        Self {
            evidence: Vec::new(),
        }
    }

    fn added(&mut self) {
        self.evidence.push(MemberTokenEvidence::Unknown);
    }

    fn removed(&mut self, index: usize) {
        if index < self.evidence.len() {
            self.evidence.remove(index);
        }
    }

    fn observe<Route: DeliveryRoute + Clone + PartialEq>(
        &mut self,
        recipients: &[Route],
        observation: MemberTokenObservation<Route>,
    ) -> Result<(), HashPolicyError<Route>> {
        let Some(index) = recipients
            .iter()
            .position(|member| member == &observation.recipient)
        else {
            return Err(HashPolicyError::UnknownRecipient(observation));
        };
        let Some(evidence) = self.evidence.get_mut(index) else {
            return Err(HashPolicyError::UnknownRecipient(observation));
        };
        let MemberTokenEvidence::Observed { version, token } = *evidence else {
            *evidence = MemberTokenEvidence::Observed {
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
        *evidence = MemberTokenEvidence::Observed {
            version: observation.version,
            token: observation.token,
        };
        Ok(())
    }

    fn tokens(&self, members: usize) -> impl Iterator<Item = (usize, MemberToken)> + '_ {
        self.evidence.iter().enumerate().take(members).filter_map(
            |(index, evidence)| match evidence {
                MemberTokenEvidence::Unknown => None,
                MemberTokenEvidence::Observed { token, .. } => Some((index, *token)),
            },
        )
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
/// Router owns recipient identity and order; this policy retains only the
/// corresponding versioned token evidence.
pub struct ConsistentHash<K> {
    membership: HashMembership,
    replicas: NonZeroU16,
    hash_key: fn(&K) -> u64,
}

impl<K> Clone for ConsistentHash<K> {
    fn clone(&self) -> Self {
        Self {
            membership: self.membership.clone(),
            replicas: self.replicas,
            hash_key: self.hash_key,
        }
    }
}

impl<K> ConsistentHash<K> {
    /// Construct a stable-ring policy with explicit virtual-point count.
    #[must_use]
    pub const fn new(replicas: NonZeroU16, hash_key: fn(&K) -> u64) -> Self {
        Self {
            membership: HashMembership::new(),
            replicas,
            hash_key,
        }
    }
}

impl<Route, K> RoutingStrategy<Route> for ConsistentHash<K>
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
    ) -> Option<usize> {
        let key = (self.hash_key)(message.route_key());
        self.membership
            .tokens(members.len())
            .flat_map(|(index, token)| {
                (0..self.replicas.get())
                    .map(move |replica| (mixed_hash(token.0, u64::from(replica)), index))
            })
            .min_by_key(|(point, index)| (*point < key, *point, *index))
            .map(|(_, index)| index)
    }

    fn observe(
        &mut self,
        members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, _: Route) {
        self.membership.added();
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
/// Router owns recipient identity and order; this policy retains only the
/// corresponding versioned token evidence.
pub struct RendezvousHash<K> {
    membership: HashMembership,
    hash_key: fn(&K) -> u64,
}

impl<K> Clone for RendezvousHash<K> {
    fn clone(&self) -> Self {
        Self {
            membership: self.membership.clone(),
            hash_key: self.hash_key,
        }
    }
}

impl<K> RendezvousHash<K> {
    /// Construct a highest-random-weight policy.
    #[must_use]
    pub const fn new(hash_key: fn(&K) -> u64) -> Self {
        Self {
            membership: HashMembership::new(),
            hash_key,
        }
    }
}

impl<Route, K> RoutingStrategy<Route> for RendezvousHash<K>
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
    ) -> Option<usize> {
        let key = (self.hash_key)(message.route_key());
        self.membership
            .tokens(members.len())
            .map(|(index, token)| (mixed_hash(key, token.0), index))
            .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)))
            .map(|(_, index)| index)
    }

    fn observe(
        &mut self,
        members: &[Route],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.membership.observe(members, observation)
    }

    fn added(&mut self, _: Route) {
        self.membership.added();
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
/// transitions emit no effects. A successful route transfers ownership to one
/// selected member and continues; an empty selection returns [`RouterError`]
/// without changing membership. Fan-out is the distinct [`crate::Topic`] or
/// [`crate::PubSub`] law and therefore does not impose a message-cloning bound
/// on this unicast actor. The router never terminates by policy and requires
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

    fn member_index(&self, recipient: &Route) -> Option<usize> {
        self.recipients
            .iter()
            .position(|member| member == recipient)
    }
}

impl<A, Route> Router<A, Route, LeastLoaded>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
{
    /// Borrow the current load-evidence phase of one member.
    ///
    /// Returns `None` when the route is not a current member. Removal retires
    /// its old evidence, so a later addition starts at [`LoadEvidence::Unknown`].
    #[must_use]
    pub fn load_evidence(&self, recipient: &Route) -> Option<LoadEvidence> {
        self.member_index(recipient)
            .and_then(|index| self.strategy.loads.get(index).copied())
    }
}

impl<A, Route, K> Router<A, Route, ConsistentHash<K>>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: RouteKey<K>,
{
    /// Borrow one current member's complete stable-token evidence.
    /// Removal retires the old evidence; later addition starts Unknown.
    #[must_use]
    pub fn member_token_evidence(&self, recipient: &Route) -> Option<MemberTokenEvidence> {
        self.member_index(recipient)
            .and_then(|index| self.strategy.membership.evidence.get(index).copied())
    }
}

impl<A, Route, K> Router<A, Route, RendezvousHash<K>>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
    <Route::Protocol as Protocol>::Msg: RouteKey<K>,
{
    /// Borrow one current member's complete stable-token evidence.
    /// Removal retires the old evidence; later addition starts Unknown.
    #[must_use]
    pub fn member_token_evidence(&self, recipient: &Route) -> Option<MemberTokenEvidence> {
        self.member_index(recipient)
            .and_then(|index| self.strategy.membership.evidence.get(index).copied())
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
    R: RoutingStrategy<Route>,
{
    type Addr = A;
    type Msg = RouterMessage<Route, R>;
}

impl<A, Route, R> Behavior for Router<A, Route, R>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A>> + Clone + PartialEq,
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

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
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
                let Some(index) = strategy.select(&self.recipients, &message) else {
                    return Err(RouterError::NoEligibleRecipients(message));
                };
                if index >= self.recipients.len() {
                    return Err(RouterError::InvalidSelection {
                        message,
                        index,
                        members: self.recipients.len(),
                    });
                }
                let sends = self.recipients[index].clone().deliver(message);
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

        fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
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

        fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
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

        let removed = router
            .receive(MailAddr(9), RouterMessage::Remove(one))
            .unwrap();
        assert!(removed.sends.is_empty());
        assert!(removed.creates.is_empty());
        assert_eq!(removed.become_, Step::Continue);
        let second = router
            .receive(MailAddr(9), RouterMessage::Route(8))
            .unwrap();
        assert!(second.sends == vec![Delivery::new(two, 8)]);
    }

    #[test]
    fn empty_membership_returns_the_owned_payload() {
        let mut router =
            (Router::<MailAddr, Recipient<Destination>, _>::new(Vec::new(), RoundRobin::default()))
                .initialize()
                .unwrap()
                .behavior;

        let rejection = router.receive(MailAddr(9), RouterMessage::Route(11));
        assert!(matches!(
            rejection,
            Err(RouterError::NoEligibleRecipients(11))
        ));
        assert!(router.recipients().is_empty());
    }

    #[test]
    fn least_loaded_requires_typed_evidence_and_breaks_ties_by_membership_order() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let two = Recipient::<Destination>::global(MailAddr(2));
        let mut router = (Router::new(vec![one, two], LeastLoaded::new()))
            .initialize()
            .unwrap()
            .behavior;

        let rejection = router.receive(MailAddr(9), RouterMessage::Route(1));
        assert!(matches!(
            rejection,
            Err(RouterError::NoEligibleRecipients(1))
        ));
        for recipient in [one, two] {
            let observed = router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(LoadObservation {
                        recipient,
                        version: LoadVersion(0),
                        load: Load(3),
                    }),
                )
                .unwrap();
            assert!(observed.sends.is_empty());
            assert!(observed.creates.is_empty());
            assert_eq!(observed.become_, Step::Continue);
        }
        let tied = router
            .receive(MailAddr(9), RouterMessage::Route(2))
            .unwrap();
        assert!(tied.sends == vec![Delivery::new(one, 2)]);

        let observed = router
            .receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: two,
                    version: LoadVersion(1),
                    load: Load(1),
                }),
            )
            .unwrap();
        assert!(observed.sends.is_empty());
        assert!(observed.creates.is_empty());
        assert_eq!(observed.become_, Step::Continue);
        let selected = router
            .receive(MailAddr(9), RouterMessage::Route(3))
            .unwrap();
        assert!(selected.sends == vec![Delivery::new(two, 3)]);
    }

    #[test]
    fn least_loaded_rejects_stale_and_unknown_evidence_without_mutation() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let unknown = Recipient::<Destination>::global(MailAddr(8));
        let mut router = (Router::new(vec![one], LeastLoaded::new()))
            .initialize()
            .unwrap()
            .behavior;
        let observed = router
            .receive(
                MailAddr(9),
                RouterMessage::Observe(LoadObservation {
                    recipient: one,
                    version: LoadVersion(2),
                    load: Load(4),
                }),
            )
            .unwrap();
        assert!(observed.sends.is_empty());
        assert!(observed.creates.is_empty());
        assert_eq!(observed.become_, Step::Continue);

        let rejection = router.receive(
            MailAddr(9),
            RouterMessage::Observe(LoadObservation {
                recipient: one,
                version: LoadVersion(1),
                load: Load(0),
            }),
        );
        assert!(matches!(
            rejection,
            Err(RouterError::Policy {
                error: LeastLoadedError::Stale(_),
                ..
            })
        ));
        let rejection = router.receive(
            MailAddr(9),
            RouterMessage::Observe(LoadObservation {
                recipient: one,
                version: LoadVersion(2),
                load: Load(5),
            }),
        );
        assert!(matches!(
            rejection,
            Err(RouterError::Policy {
                error: LeastLoadedError::ConflictingVersion(_),
                ..
            })
        ));
        let rejection = router.receive(
            MailAddr(9),
            RouterMessage::Observe(LoadObservation {
                recipient: unknown,
                version: LoadVersion(0),
                load: Load(0),
            }),
        );
        assert!(matches!(
            rejection,
            Err(RouterError::Policy {
                error: LeastLoadedError::UnknownRecipient(_),
                ..
            })
        ));
        assert_eq!(
            router.load_evidence(&one),
            Some(LoadEvidence::Observed {
                version: LoadVersion(2),
                load: Load(4)
            })
        );
    }

    #[test]
    fn least_loaded_returns_unknown_observation_for_untracked_member() {
        let recipient = Recipient::<Destination>::global(MailAddr(1));
        let observation = LoadObservation {
            recipient,
            version: LoadVersion(4),
            load: Load(7),
        };
        let mut policy = LeastLoaded::new();

        let rejection = policy.observe(&[recipient], observation);
        assert!(matches!(
            rejection,
            Err(LeastLoadedError::UnknownRecipient(LoadObservation {
                recipient: returned,
                version: LoadVersion(4),
                load: Load(7),
            })) if returned == recipient
        ));
    }

    fn identity_hash(key: &Key) -> u64 {
        key.0
    }

    #[test]
    fn hash_policies_return_untracked_member_evidence() {
        let recipient = Recipient::<KeyedDestination>::global(MailAddr(1));
        let observation = MemberTokenObservation {
            recipient,
            version: MemberTokenVersion(3),
            token: MemberToken(8),
        };
        let mut ring = ConsistentHash::new(NonZeroU16::new(1).unwrap(), identity_hash);
        let mut rendezvous = RendezvousHash::new(identity_hash);

        for rejection in [
            ring.observe(&[recipient], observation.clone()),
            rendezvous.observe(&[recipient], observation),
        ] {
            assert!(matches!(
                rejection,
                Err(HashPolicyError::UnknownRecipient(MemberTokenObservation {
                    recipient: returned,
                    version: MemberTokenVersion(3),
                    token: MemberToken(8),
                })) if returned == recipient
            ));
        }
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
            let observed = router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(MemberTokenObservation {
                        recipient,
                        version: MemberTokenVersion(0),
                        token: MemberToken(u64::try_from(index + 1).unwrap()),
                    }),
                )
                .unwrap();
            assert!(observed.sends.is_empty());
            assert!(observed.creates.is_empty());
            assert_eq!(observed.become_, Step::Continue);
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
        let removed = router
            .receive(MailAddr(9), RouterMessage::Remove(members[1]))
            .unwrap();
        assert!(removed.sends.is_empty());
        assert!(removed.creates.is_empty());
        assert_eq!(removed.become_, Step::Continue);
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
            let observed = router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(MemberTokenObservation {
                        recipient,
                        version: MemberTokenVersion(0),
                        token: MemberToken(token),
                    }),
                )
                .unwrap();
            assert!(observed.sends.is_empty());
            assert!(observed.creates.is_empty());
            assert_eq!(observed.become_, Step::Continue);
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
        let conflicting = MemberTokenObservation {
            recipient: one,
            version: MemberTokenVersion(0),
            token: MemberToken(99),
        };
        let rejection = router.receive(MailAddr(9), RouterMessage::Observe(conflicting.clone()));
        match rejection {
            Err(RouterError::Policy {
                observation,
                error: HashPolicyError::ConflictingVersion(returned),
            }) => {
                assert!(observation == conflicting);
                assert!(returned == conflicting);
            }
            _ => panic!("conflicting token evidence must return its exact observation"),
        }
        assert_eq!(
            router.member_token_evidence(&one),
            Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(0),
                token: MemberToken(11)
            })
        );
    }

    #[test]
    fn hash_token_versions_return_stale_evidence_and_accept_newer_observations() {
        let recipient = Recipient::<KeyedDestination>::global(MailAddr(1));
        let mut router = (Router::new(vec![recipient], RendezvousHash::new(identity_hash)))
            .initialize()
            .unwrap()
            .behavior;
        let observed = router
            .receive(
                MailAddr(9),
                RouterMessage::Observe(MemberTokenObservation {
                    recipient,
                    version: MemberTokenVersion(2),
                    token: MemberToken(11),
                }),
            )
            .unwrap();
        assert!(observed.sends.is_empty());
        assert!(observed.creates.is_empty());
        assert_eq!(observed.become_, Step::Continue);

        let stale = MemberTokenObservation {
            recipient,
            version: MemberTokenVersion(1),
            token: MemberToken(99),
        };
        let rejection = router.receive(MailAddr(9), RouterMessage::Observe(stale.clone()));
        match rejection {
            Err(RouterError::Policy {
                observation,
                error: HashPolicyError::Stale(returned),
            }) => {
                assert!(observation == stale);
                assert!(returned == stale);
            }
            _ => panic!("older token evidence must return its exact observation"),
        }
        assert_eq!(
            router.member_token_evidence(&recipient),
            Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(2),
                token: MemberToken(11),
            })
        );

        for (version, token) in [(2, 11), (3, 22)] {
            let accepted = router
                .receive(
                    MailAddr(9),
                    RouterMessage::Observe(MemberTokenObservation {
                        recipient,
                        version: MemberTokenVersion(version),
                        token: MemberToken(token),
                    }),
                )
                .unwrap();
            assert!(accepted.sends.is_empty());
            assert!(accepted.creates.is_empty());
            assert_eq!(accepted.become_, Step::Continue);
        }
        assert_eq!(
            router.member_token_evidence(&recipient),
            Some(MemberTokenEvidence::Observed {
                version: MemberTokenVersion(3),
                token: MemberToken(22),
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

        fn select(&mut self, members: &[Recipient<Destination>], _: &u8) -> Option<usize> {
            self.selections += 1;
            Some(members.len())
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

        let rejection = router.receive(MailAddr(9), RouterMessage::Route(42));
        assert!(matches!(
            rejection,
            Err(RouterError::InvalidSelection {
                message: 42,
                index: 1,
                members: 1,
            })
        ));
        assert_eq!(router.strategy().selections, 0);

        let rejection = router.receive(MailAddr(9), RouterMessage::Observe(7));
        assert!(matches!(
            rejection,
            Err(RouterError::Policy {
                observation: 7,
                error: 7,
            })
        ));
        assert_eq!(router.strategy().observations, 0);
    }
}
