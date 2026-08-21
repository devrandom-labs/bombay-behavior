//! Full Behavior-side established-recipient experiment.
//!
//! This remains a research probe rather than a production API. It deliberately
//! models only pure values and explicit interpreter boundaries.

use behavior::{Address, CreationKind, Protocol};
use core::marker::PhantomData;

/// One endpoint family selected by an address/runtime namespace.
pub trait EndpointAddress: Address + Sized {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

/// A fake exact-incarnation runtime endpoint.
pub struct ActorRef<P: Protocol> {
    address: P::Addr,
    slot: u64,
    protocol: PhantomData<fn(P::Msg) -> P>,
}

impl<P: Protocol> ActorRef<P> {
    #[must_use]
    pub const fn issued(address: P::Addr, slot: u64) -> Self {
        Self {
            address,
            slot,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> u64 {
        self.slot
    }

    #[must_use]
    pub const fn address(&self) -> P::Addr {
        self.address
    }
}

impl<P: Protocol> Clone for ActorRef<P> {
    fn clone(&self) -> Self {
        Self::issued(self.address, self.slot)
    }
}

/// Logical address namespace used by the probe runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAddr(pub u64);

impl Address for RuntimeAddr {
    type Nonce = u64;

    fn birth(self, nonce: Self::Nonce) -> Self {
        Self(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = ActorRef<P>
    where
        P: Protocol<Addr = Self>;
}

/// Inert exact-incarnation capability selected only by the canonical protocol.
///
/// The endpoint has no accessor and crosses only an explicit interpretation
/// boundary. Its runtime-owned representation carries the corresponding
/// freshly claimed address, so the capability has no parallel address field
/// that could disagree with the endpoint.
pub struct EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    endpoint: <P::Addr as EndpointAddress>::Established<P>,
}

impl<P> EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn issued(endpoint: <P::Addr as EndpointAddress>::Established<P>) -> Self {
        Self { endpoint }
    }
}

impl<P> Clone for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        Self::issued(self.endpoint.clone())
    }
}

/// One inert communication to an exact established incarnation.
pub struct EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub to: EstablishedRecipient<P>,
    pub message: P::Msg,
}

impl<P> EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(to: EstablishedRecipient<P>, message: P::Msg) -> Self {
        Self { to, message }
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablished<P>,
    {
        interpreter.interpret(self.to.endpoint, self.message)
    }
}

/// Public power-user interpretation boundary for one exact protocol endpoint.
pub trait InterpretEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Output;

    fn interpret(
        &mut self,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
        message: P::Msg,
    ) -> Self::Output;
}

/// Typed failure to claim an address fresh for the current configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRejection {
    Exhausted,
    AddressAlreadyClaimed,
}

/// Non-resolving allocator: it owns claims but stores no address-to-endpoint
/// association.
pub struct FreshAllocator {
    candidates: Vec<RuntimeAddr>,
    claimed: Vec<RuntimeAddr>,
}

impl FreshAllocator {
    #[must_use]
    pub fn new(candidates: impl IntoIterator<Item = RuntimeAddr>) -> Self {
        let mut candidates = candidates.into_iter().collect::<Vec<_>>();
        candidates.reverse();
        Self {
            candidates,
            claimed: Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> Result<RuntimeAddr, AllocationRejection> {
        let candidate = self
            .candidates
            .pop()
            .ok_or(AllocationRejection::Exhausted)?;
        if self.claimed.contains(&candidate) {
            return Err(AllocationRejection::AddressAlreadyClaimed);
        }
        self.claimed.push(candidate);
        Ok(candidate)
    }

    #[must_use]
    pub fn claimed(&self) -> &[RuntimeAddr] {
        &self.claimed
    }
}

/// Complete semantic rejection of one staged creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationRejection {
    NonceAlreadyBound,
    Allocation(AllocationRejection),
    InitializationFailed,
    EnvironmentFailed,
}

/// Nominal proof that one role belongs to a parent's closed child topology.
pub trait ChildRole<Parent> {
    type Protocol: Protocol<Addr = RuntimeAddr>;
}

pub type RoleProtocol<Parent, Role> = <Role as ChildRole<Parent>>::Protocol;

/// Creator-local staged route. It contains correlation evidence, not an actor
/// name, endpoint, or freshness proof.
pub struct StagedChild<Parent, Role>
where
    Role: ChildRole<Parent>,
{
    nonce: u64,
    role: PhantomData<fn() -> (Parent, Role)>,
}

impl<Parent, Role> StagedChild<Parent, Role>
where
    Role: ChildRole<Parent>,
{
    #[must_use]
    pub const fn new(nonce: u64) -> Self {
        Self {
            nonce,
            role: PhantomData,
        }
    }

    #[must_use]
    pub const fn nonce(self) -> u64 {
        self.nonce
    }
}

impl<Parent, Role> Copy for StagedChild<Parent, Role> where Role: ChildRole<Parent> {}

impl<Parent, Role> Clone for StagedChild<Parent, Role>
where
    Role: ChildRole<Parent>,
{
    fn clone(&self) -> Self {
        *self
    }
}

/// Complete committed or rejected result for one exact child role.
pub enum CreationResolved<Parent, Role>
where
    Role: ChildRole<Parent>,
{
    Installed {
        nonce: u64,
        kind: CreationKind<u64>,
        recipient: EstablishedRecipient<RoleProtocol<Parent, Role>>,
    },
    Rejected {
        nonce: u64,
        kind: CreationKind<u64>,
        reason: CreationRejection,
    },
}

impl<Parent, Role> CreationResolved<Parent, Role>
where
    Role: ChildRole<Parent>,
{
    #[must_use]
    pub const fn nonce(&self) -> u64 {
        match self {
            Self::Installed { nonce, .. } | Self::Rejected { nonce, .. } => *nonce,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> CreationKind<u64> {
        match self {
            Self::Installed { kind, .. } | Self::Rejected { kind, .. } => *kind,
        }
    }

    pub fn into_recipient(
        self,
    ) -> Result<EstablishedRecipient<RoleProtocol<Parent, Role>>, CreationRejection> {
        match self {
            Self::Installed { recipient, .. } => Ok(recipient),
            Self::Rejected { reason, .. } => Err(reason),
        }
    }
}

/// Structural position selecting the current role binding.
pub struct RoleHead;

/// Structural position selecting a binding in the remaining role product.
pub struct RoleTail<Position>(PhantomData<fn() -> Position>);

/// Empty role-binding product.
pub struct NoRoleBindings;

/// One named role binding followed by the remaining structural product.
pub struct RoleBindings<Parent, Role, Tail>
where
    Role: ChildRole<Parent>,
{
    entries: Vec<(u64, EstablishedRecipient<RoleProtocol<Parent, Role>>)>,
    tail: Tail,
    parent: PhantomData<fn() -> Parent>,
}

impl<Parent, Role, Tail> RoleBindings<Parent, Role, Tail>
where
    Role: ChildRole<Parent>,
{
    #[must_use]
    pub const fn new(tail: Tail) -> Self {
        Self {
            entries: Vec::new(),
            tail,
            parent: PhantomData,
        }
    }
}

/// Static selection of one role in a closed binding product.
pub trait BindingAt<Parent, Role, Position>
where
    Role: ChildRole<Parent>,
{
    fn bind(&mut self, nonce: u64, recipient: EstablishedRecipient<RoleProtocol<Parent, Role>>);

    fn resolve(&self, nonce: u64) -> Option<EstablishedRecipient<RoleProtocol<Parent, Role>>>;
}

impl<Parent, Role, Tail> BindingAt<Parent, Role, RoleHead> for RoleBindings<Parent, Role, Tail>
where
    Role: ChildRole<Parent>,
{
    fn bind(&mut self, nonce: u64, recipient: EstablishedRecipient<RoleProtocol<Parent, Role>>) {
        self.entries.push((nonce, recipient));
    }

    fn resolve(&self, nonce: u64) -> Option<EstablishedRecipient<RoleProtocol<Parent, Role>>> {
        self.entries
            .iter()
            .find(|(bound, _)| *bound == nonce)
            .map(|(_, recipient)| recipient.clone())
    }
}

impl<Parent, HeadRole, Tail, Role, Position> BindingAt<Parent, Role, RoleTail<Position>>
    for RoleBindings<Parent, HeadRole, Tail>
where
    HeadRole: ChildRole<Parent>,
    Role: ChildRole<Parent>,
    Tail: BindingAt<Parent, Role, Position>,
{
    fn bind(&mut self, nonce: u64, recipient: EstablishedRecipient<RoleProtocol<Parent, Role>>) {
        self.tail.bind(nonce, recipient);
    }

    fn resolve(&self, nonce: u64) -> Option<EstablishedRecipient<RoleProtocol<Parent, Role>>> {
        self.tail.resolve(nonce)
    }
}

/// Installation disposition supplied by the fake interpreter boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installation {
    Succeeds,
    InitializationFails,
    EnvironmentFails,
}

/// Creator-local role bindings plus one cross-role nonce-claim set.
///
/// This is not address-indexed and cannot resolve an ordinary actor address.
pub struct ChildNamespace<Parent, Roles> {
    claimed_nonces: Vec<u64>,
    roles: Roles,
    parent: PhantomData<fn() -> Parent>,
}

impl<Parent, Roles> ChildNamespace<Parent, Roles> {
    #[must_use]
    pub const fn new(roles: Roles) -> Self {
        Self {
            claimed_nonces: Vec::new(),
            roles,
            parent: PhantomData,
        }
    }

    pub fn realize<Role, Position>(
        &mut self,
        route: StagedChild<Parent, Role>,
        kind: CreationKind<u64>,
        allocator: &mut FreshAllocator,
        endpoint_slot: u64,
        installation: Installation,
    ) -> CreationResolved<Parent, Role>
    where
        Role: ChildRole<Parent>,
        Roles: BindingAt<Parent, Role, Position>,
    {
        let nonce = route.nonce();
        if self.claimed_nonces.contains(&nonce) {
            return CreationResolved::Rejected {
                nonce,
                kind,
                reason: CreationRejection::NonceAlreadyBound,
            };
        }

        let address = match allocator.allocate() {
            Ok(address) => address,
            Err(reason) => {
                return CreationResolved::Rejected {
                    nonce,
                    kind,
                    reason: CreationRejection::Allocation(reason),
                };
            }
        };

        let rejection = match installation {
            Installation::Succeeds => None,
            Installation::InitializationFails => Some(CreationRejection::InitializationFailed),
            Installation::EnvironmentFails => Some(CreationRejection::EnvironmentFailed),
        };
        if let Some(reason) = rejection {
            return CreationResolved::Rejected {
                nonce,
                kind,
                reason,
            };
        }

        let endpoint = ActorRef::issued(address, endpoint_slot);
        let recipient = EstablishedRecipient::issued(endpoint);
        self.roles.bind(nonce, recipient.clone());
        self.claimed_nonces.push(nonce);
        CreationResolved::Installed {
            nonce,
            kind,
            recipient,
        }
    }

    #[must_use]
    pub fn resolve_local<Role, Position>(
        &self,
        route: StagedChild<Parent, Role>,
    ) -> Option<EstablishedRecipient<RoleProtocol<Parent, Role>>>
    where
        Role: ChildRole<Parent>,
        Roles: BindingAt<Parent, Role, Position>,
    {
        self.roles.resolve(route.nonce())
    }
}

/// Exact observation correlation chosen by Behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationId(pub u64);

/// Request to observe one exact established incarnation.
pub struct ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub id: ObservationId,
    recipient: EstablishedRecipient<P>,
}

impl<P> ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(id: ObservationId, recipient: EstablishedRecipient<P>) -> Self {
        Self { id, recipient }
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretObservation<P>,
    {
        interpreter.observe(self.id, self.recipient.endpoint)
    }
}

/// Exact, protocol-indexed observation cancellation.
pub struct CancelObservation<P: Protocol> {
    pub id: ObservationId,
    protocol: PhantomData<fn() -> P>,
}

impl<P: Protocol> CancelObservation<P> {
    #[must_use]
    pub const fn new(id: ObservationId) -> Self {
        Self {
            id,
            protocol: PhantomData,
        }
    }
}

/// Terminal fact returned only to the structural owner of one observation.
pub struct EstablishedStopped<P: Protocol> {
    pub id: ObservationId,
    pub outcome: StopOutcome,
    protocol: PhantomData<fn() -> P>,
}

impl<P: Protocol> EstablishedStopped<P> {
    #[must_use]
    pub const fn new(id: ObservationId, outcome: StopOutcome) -> Self {
        Self {
            id,
            outcome,
            protocol: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    Normal,
    Failed,
}

/// Public exact-observation interpretation boundary.
pub trait InterpretObservation<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Output;

    fn observe(
        &mut self,
        id: ObservationId,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
    ) -> Self::Output;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationRejection {
    IdAlreadyBound,
    NotObserved,
}

/// Exact-incarnation shutdown request; it cannot retarget through an address.
pub struct ShutdownEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    recipient: EstablishedRecipient<P>,
}

impl<P> ShutdownEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(recipient: EstablishedRecipient<P>) -> Self {
        Self { recipient }
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> Result<(), ShutdownRejection>
    where
        I: InterpretShutdown<P>,
    {
        interpreter.shutdown(self.recipient.endpoint)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownRejection {
    AlreadyStopping,
    AlreadyStopped,
}

/// Public exact-shutdown interpretation boundary.
pub trait InterpretShutdown<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn shutdown(
        &mut self,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
    ) -> Result<(), ShutdownRejection>;
}

/// No address resolver exists here: lifecycle relationships are keyed by
/// Behavior-issued observation correlation and exact endpoint slot.
pub struct LifecycleRuntime<P: Protocol> {
    observations: Vec<(ObservationId, u64)>,
    endpoints: Vec<(u64, EndpointPhase)>,
    protocol: PhantomData<fn() -> P>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointPhase {
    Running,
    Stopping,
    Stopped,
}

impl<P: Protocol> LifecycleRuntime<P> {
    #[must_use]
    pub fn new(endpoints: impl IntoIterator<Item = (u64, EndpointPhase)>) -> Self {
        Self {
            observations: Vec::new(),
            endpoints: endpoints.into_iter().collect(),
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub fn observed_slot(&self, id: ObservationId) -> Option<u64> {
        self.observations
            .iter()
            .find(|(observed, _)| *observed == id)
            .map(|(_, slot)| *slot)
    }

    pub fn cancel(&mut self, request: CancelObservation<P>) -> Result<(), ObservationRejection> {
        let Some(position) = self
            .observations
            .iter()
            .position(|(id, _)| *id == request.id)
        else {
            return Err(ObservationRejection::NotObserved);
        };
        self.observations.remove(position);
        Ok(())
    }

    pub fn complete(
        &mut self,
        id: ObservationId,
        outcome: StopOutcome,
    ) -> Result<EstablishedStopped<P>, ObservationRejection> {
        let Some(position) = self
            .observations
            .iter()
            .position(|(observed, _)| *observed == id)
        else {
            return Err(ObservationRejection::NotObserved);
        };
        self.observations.remove(position);
        Ok(EstablishedStopped::new(id, outcome))
    }

    #[must_use]
    pub fn phase(&self, slot: u64) -> Option<EndpointPhase> {
        self.endpoints
            .iter()
            .find(|(candidate, _)| *candidate == slot)
            .map(|(_, phase)| *phase)
    }
}

impl<P> InterpretObservation<P> for LifecycleRuntime<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    type Output = Result<(), ObservationRejection>;

    fn observe(
        &mut self,
        id: ObservationId,
        endpoint: ActorRef<P>,
    ) -> Result<(), ObservationRejection> {
        if self
            .observations
            .iter()
            .any(|(observed, _)| *observed == id)
        {
            return Err(ObservationRejection::IdAlreadyBound);
        }
        self.observations.push((id, endpoint.slot()));
        Ok(())
    }
}

impl<P> InterpretShutdown<P> for LifecycleRuntime<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    fn shutdown(&mut self, endpoint: ActorRef<P>) -> Result<(), ShutdownRejection> {
        let Some((_, phase)) = self
            .endpoints
            .iter_mut()
            .find(|(slot, _)| *slot == endpoint.slot())
        else {
            return Err(ShutdownRejection::AlreadyStopped);
        };
        match phase {
            EndpointPhase::Running => {
                *phase = EndpointPhase::Stopping;
                Ok(())
            }
            EndpointPhase::Stopping => Err(ShutdownRejection::AlreadyStopping),
            EndpointPhase::Stopped => Err(ShutdownRejection::AlreadyStopped),
        }
    }
}

/// Trace event used to prove real `Actions`/`SendLayer` preservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEffect {
    Base,
    Observe,
    Timer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceLane(pub Vec<TraceEffect>);

impl behavior::SendEffects for TraceLane {
    fn empty() -> Self {
        Self(Vec::new())
    }

    fn append(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }
}

impl<Event> behavior::SendsFor<Event> for TraceLane {}

pub struct TraceInterpreter(pub Vec<TraceEffect>);

impl behavior::SendInterpreter for TraceInterpreter {
    type Error = core::convert::Infallible;
}

impl<RootEvent, Path> behavior::InterpretSends<TraceInterpreter, RootEvent, Path> for TraceLane {
    async fn interpret(
        self,
        interpreter: &mut TraceInterpreter,
    ) -> Result<(), core::convert::Infallible> {
        interpreter.0.extend(self.0);
        Ok(())
    }
}

pub struct Parent;
pub struct PrimaryRole;
pub struct SecondaryRole;
pub struct Queue;
pub struct Worker;

impl Protocol for Queue {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

impl ChildRole<Parent> for PrimaryRole {
    type Protocol = Worker;
}

impl ChildRole<Parent> for SecondaryRole {
    type Protocol = Worker;
}

pub type ParentBindings =
    RoleBindings<Parent, PrimaryRole, RoleBindings<Parent, SecondaryRole, NoRoleBindings>>;

#[must_use]
pub const fn parent_bindings() -> ParentBindings {
    RoleBindings::new(RoleBindings::new(NoRoleBindings))
}

/// No endpoint parameter appears in this transferable domain message.
pub struct Transfer {
    pub worker: EstablishedRecipient<Worker>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::{
        Actions, Behavior, Births, EventLayer, InterpretSends, Never, NoBirths, SendLayer, Step,
        User,
    };

    type PrimaryPosition = RoleHead;
    type SecondaryPosition = RoleTail<RoleHead>;

    struct DeliveryTrace(Option<(u64, u8)>);

    impl InterpretEstablished<Worker> for DeliveryTrace {
        type Output = ();

        fn interpret(&mut self, endpoint: ActorRef<Worker>, message: u8) {
            self.0 = Some((endpoint.slot(), message));
        }
    }

    struct ParentProtocol;

    impl Protocol for ParentProtocol {
        type Addr = RuntimeAddr;
        type Msg = Never;
    }

    enum CapabilityPhase {
        Awaiting,
        Active(EstablishedRecipient<Worker>),
        Rejected(CreationRejection),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CapabilityFoldError {
        StaleCreationFact,
    }

    struct CapabilityFold {
        phase: CapabilityPhase,
    }

    impl CapabilityFold {
        const fn new() -> Self {
            Self {
                phase: CapabilityPhase::Awaiting,
            }
        }
    }

    impl Behavior for CapabilityFold {
        type Protocol = ParentProtocol;
        type Event = EventLayer<CreationResolved<Parent, PrimaryRole>, User<RuntimeAddr, Never>>;
        type Sends = Vec<EstablishedDelivery<Worker>>;
        type Ph = Never;
        type Error = CapabilityFoldError;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: behavior::ActiveTurn,
            event: Self::Event,
        ) -> behavior::BehaviorActed<Self> {
            match event {
                EventLayer::Inner(user) => match user.message {},
                EventLayer::Owned(resolution) => {
                    if !matches!(self.phase, CapabilityPhase::Awaiting) {
                        return Err(CapabilityFoldError::StaleCreationFact);
                    }
                    match resolution {
                        CreationResolved::Installed { recipient, .. } => {
                            self.phase = CapabilityPhase::Active(recipient.clone());
                            Ok(Actions::send(vec![EstablishedDelivery::new(recipient, 1)]))
                        }
                        CreationResolved::Rejected { reason, .. } => {
                            self.phase = CapabilityPhase::Rejected(reason);
                            Ok(Actions::cont())
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn allocation_is_independent_of_nonce_address_derivation() {
        let first_creator = RuntimeAddr(0);
        let first_nonce = 1;
        let colliding_hint = first_creator.birth(first_nonce);
        let second_creator = colliding_hint;
        let second_nonce = 0;
        assert_eq!(
            first_creator.birth(first_nonce),
            second_creator.birth(second_nonce)
        );

        let mut allocator = FreshAllocator::new([RuntimeAddr(100), RuntimeAddr(101)]);
        assert_eq!(allocator.allocate(), Ok(RuntimeAddr(100)));
        assert_eq!(allocator.allocate(), Ok(RuntimeAddr(101)));
        assert_eq!(allocator.claimed(), [RuntimeAddr(100), RuntimeAddr(101)]);
    }

    #[test]
    fn address_claim_collision_is_typed_and_never_issues_a_capability() {
        let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
        let mut allocator = FreshAllocator::new([RuntimeAddr(20), RuntimeAddr(20)]);
        let primary = StagedChild::<Parent, PrimaryRole>::new(1);
        let secondary = StagedChild::<Parent, SecondaryRole>::new(2);

        let first = namespace.realize::<PrimaryRole, PrimaryPosition>(
            primary,
            CreationKind::Birth,
            &mut allocator,
            31,
            Installation::Succeeds,
        );
        assert!(matches!(first, CreationResolved::Installed { .. }));

        let second = namespace.realize::<SecondaryRole, SecondaryPosition>(
            secondary,
            CreationKind::Birth,
            &mut allocator,
            32,
            Installation::Succeeds,
        );
        assert!(matches!(
            second,
            CreationResolved::Rejected {
                reason: CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed),
                ..
            }
        ));
        assert!(
            namespace
                .resolve_local::<SecondaryRole, SecondaryPosition>(secondary)
                .is_none()
        );
    }

    #[test]
    fn failed_installation_returns_no_capability_and_leaves_nonce_retryable() {
        let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
        let mut allocator = FreshAllocator::new([RuntimeAddr(20), RuntimeAddr(21)]);
        let route = StagedChild::<Parent, PrimaryRole>::new(7);

        let failed = namespace.realize::<PrimaryRole, PrimaryPosition>(
            route,
            CreationKind::Birth,
            &mut allocator,
            41,
            Installation::InitializationFails,
        );
        assert!(matches!(
            failed,
            CreationResolved::Rejected {
                reason: CreationRejection::InitializationFailed,
                ..
            }
        ));
        assert!(
            namespace
                .resolve_local::<PrimaryRole, PrimaryPosition>(route)
                .is_none()
        );

        let retried = namespace.realize::<PrimaryRole, PrimaryPosition>(
            route,
            CreationKind::Birth,
            &mut allocator,
            42,
            Installation::Succeeds,
        );
        let recipient = retried
            .into_recipient()
            .expect("a failed installation did not bind the nonce");
        let mut trace = DeliveryTrace(None);
        EstablishedDelivery::new(recipient, 3).interpret(&mut trace);
        assert_eq!(trace.0, Some((42, 3)));
        assert_eq!(allocator.claimed(), [RuntimeAddr(20), RuntimeAddr(21)]);
    }

    #[test]
    fn duplicate_roles_and_nonces_cannot_silently_alias() {
        let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
        let mut allocator = FreshAllocator::new([RuntimeAddr(20), RuntimeAddr(21)]);
        let primary = StagedChild::<Parent, PrimaryRole>::new(7);
        let secondary = StagedChild::<Parent, SecondaryRole>::new(7);

        let installed = namespace.realize::<PrimaryRole, PrimaryPosition>(
            primary,
            CreationKind::Birth,
            &mut allocator,
            41,
            Installation::Succeeds,
        );
        assert!(matches!(installed, CreationResolved::Installed { .. }));

        let duplicate = namespace.realize::<SecondaryRole, SecondaryPosition>(
            secondary,
            CreationKind::Birth,
            &mut allocator,
            42,
            Installation::Succeeds,
        );
        assert!(matches!(
            duplicate,
            CreationResolved::Rejected {
                reason: CreationRejection::NonceAlreadyBound,
                ..
            }
        ));
    }

    #[test]
    fn committed_creation_enables_same_action_local_delivery_then_transfer() {
        let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
        let mut allocator = FreshAllocator::new([RuntimeAddr(20)]);
        let route = StagedChild::<Parent, PrimaryRole>::new(7);
        let committed = namespace.realize::<PrimaryRole, PrimaryPosition>(
            route,
            CreationKind::Birth,
            &mut allocator,
            41,
            Installation::Succeeds,
        );

        let same_action = namespace
            .resolve_local::<PrimaryRole, PrimaryPosition>(route)
            .expect("creation commits before dependent sends");
        let mut trace = DeliveryTrace(None);
        EstablishedDelivery::new(same_action, 5).interpret(&mut trace);
        assert_eq!(trace.0, Some((41, 5)));

        let transfer = Transfer {
            worker: committed
                .into_recipient()
                .expect("committed creation owns the capability"),
        };
        EstablishedDelivery::new(transfer.worker, 9).interpret(&mut trace);
        assert_eq!(trace.0, Some((41, 9)));
    }

    #[test]
    fn pure_behavior_fold_accepts_one_committed_capability_and_emits_it_only_in_actions() {
        let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
        let mut allocator = FreshAllocator::new([RuntimeAddr(20)]);
        let route = StagedChild::<Parent, PrimaryRole>::new(7);
        let committed = namespace.realize::<PrimaryRole, PrimaryPosition>(
            route,
            CreationKind::Birth,
            &mut allocator,
            41,
            Installation::Succeeds,
        );

        let mut fold = CapabilityFold::new();
        let initialized = behavior::initialize(&mut fold).expect("default initialization is pure");
        assert!(initialized.sends.is_empty());
        assert!(initialized.creates.is_empty());
        assert_eq!(initialized.become_, Step::Continue);

        let actions = behavior::delegate_transition(&mut fold, EventLayer::Owned(committed))
            .expect("first committed fact is accepted");
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        assert_eq!(actions.sends.len(), 1);
        let CapabilityPhase::Active(retained) = &fold.phase else {
            panic!("the committed capability must become behavior state");
        };
        let retained = retained.clone();

        let mut trace = DeliveryTrace(None);
        actions
            .sends
            .into_iter()
            .next()
            .expect("one explicit delivery")
            .interpret(&mut trace);
        assert_eq!(trace.0, Some((41, 1)));

        let stale = CreationResolved::<Parent, PrimaryRole>::Rejected {
            nonce: 7,
            kind: CreationKind::Birth,
            reason: CreationRejection::NonceAlreadyBound,
        };
        let error = match behavior::delegate_transition(&mut fold, EventLayer::Owned(stale)) {
            Err(error) => error,
            Ok(_) => panic!("a second resolution must be a typed stale fact"),
        };
        assert_eq!(error, CapabilityFoldError::StaleCreationFact);
        assert!(matches!(fold.phase, CapabilityPhase::Active(_)));

        EstablishedDelivery::new(retained, 2).interpret(&mut trace);
        assert_eq!(trace.0, Some((41, 2)));
    }

    #[test]
    fn rejected_creation_enters_a_disjoint_behavior_phase_without_effects() {
        let mut fold = CapabilityFold::new();
        let rejected = CreationResolved::<Parent, PrimaryRole>::Rejected {
            nonce: 7,
            kind: CreationKind::Birth,
            reason: CreationRejection::EnvironmentFailed,
        };
        let actions = behavior::delegate_transition(&mut fold, EventLayer::Owned(rejected))
            .expect("first rejected fact is accepted");
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        assert!(matches!(
            fold.phase,
            CapabilityPhase::Rejected(CreationRejection::EnvironmentFailed)
        ));
    }

    #[test]
    fn observation_and_shutdown_select_exact_incarnations_without_address_lookup() {
        let old = EstablishedRecipient::<Worker>::issued(ActorRef::issued(RuntimeAddr(9), 41));
        let replacement =
            EstablishedRecipient::<Worker>::issued(ActorRef::issued(RuntimeAddr(9), 42));
        let mut runtime = LifecycleRuntime::<Worker>::new([
            (41, EndpointPhase::Running),
            (42, EndpointPhase::Running),
            (43, EndpointPhase::Stopped),
        ]);

        ObserveEstablished::new(ObservationId(3), old.clone())
            .interpret(&mut runtime)
            .expect("fresh observation correlation");
        assert_eq!(runtime.observed_slot(ObservationId(3)), Some(41));
        assert_eq!(
            ObserveEstablished::new(ObservationId(3), old.clone()).interpret(&mut runtime),
            Err(ObservationRejection::IdAlreadyBound)
        );

        ShutdownEstablished::new(old.clone())
            .interpret(&mut runtime)
            .expect("old exact incarnation is running");
        assert_eq!(runtime.phase(41), Some(EndpointPhase::Stopping));
        assert_eq!(runtime.phase(42), Some(EndpointPhase::Running));
        assert_eq!(
            ShutdownEstablished::new(old).interpret(&mut runtime),
            Err(ShutdownRejection::AlreadyStopping)
        );
        let stopped = EstablishedRecipient::<Worker>::issued(ActorRef::issued(RuntimeAddr(10), 43));
        assert_eq!(
            ShutdownEstablished::new(stopped).interpret(&mut runtime),
            Err(ShutdownRejection::AlreadyStopped)
        );

        ObserveEstablished::new(ObservationId(4), replacement)
            .interpret(&mut runtime)
            .expect("distinct observation correlation");
        assert_eq!(runtime.observed_slot(ObservationId(4)), Some(42));
        let completed = runtime
            .complete(ObservationId(3), StopOutcome::Failed)
            .expect("matching observation completes exactly once");
        assert_eq!(completed.id, ObservationId(3));
        assert_eq!(completed.outcome, StopOutcome::Failed);
        assert_eq!(runtime.observed_slot(ObservationId(3)), None);
        assert!(matches!(
            runtime.complete(ObservationId(3), StopOutcome::Failed),
            Err(ObservationRejection::NotObserved)
        ));
        runtime
            .cancel(CancelObservation::<Worker>::new(ObservationId(4)))
            .expect("matching observation cancellation");
        assert_eq!(
            runtime.cancel(CancelObservation::<Worker>::new(ObservationId(4))),
            Err(ObservationRejection::NotObserved)
        );
        assert_eq!(runtime.observed_slot(ObservationId(4)), None);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ModelRole {
        Primary,
        Secondary,
    }

    #[derive(Debug, Clone, Copy)]
    struct Attempt {
        role: ModelRole,
        nonce: u64,
        installation: Installation,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ResolutionClass {
        Installed,
        NonceAlreadyBound,
        AddressAlreadyClaimed,
        Exhausted,
        InitializationFailed,
        EnvironmentFailed,
    }

    struct IndependentCreationModel {
        candidates: [RuntimeAddr; 3],
        next_candidate: usize,
        claimed_addresses: Vec<RuntimeAddr>,
        bindings: Vec<(ModelRole, u64)>,
    }

    impl IndependentCreationModel {
        fn new() -> Self {
            Self {
                candidates: [RuntimeAddr(30), RuntimeAddr(30), RuntimeAddr(31)],
                next_candidate: 0,
                claimed_addresses: Vec::new(),
                bindings: Vec::new(),
            }
        }

        fn apply(&mut self, attempt: Attempt) -> ResolutionClass {
            if self
                .bindings
                .iter()
                .any(|(_, nonce)| *nonce == attempt.nonce)
            {
                return ResolutionClass::NonceAlreadyBound;
            }
            let Some(address) = self.candidates.get(self.next_candidate).copied() else {
                return ResolutionClass::Exhausted;
            };
            self.next_candidate += 1;
            if self.claimed_addresses.contains(&address) {
                return ResolutionClass::AddressAlreadyClaimed;
            }
            self.claimed_addresses.push(address);
            match attempt.installation {
                Installation::Succeeds => {
                    self.bindings.push((attempt.role, attempt.nonce));
                    ResolutionClass::Installed
                }
                Installation::InitializationFails => ResolutionClass::InitializationFailed,
                Installation::EnvironmentFails => ResolutionClass::EnvironmentFailed,
            }
        }

        fn is_bound(&self, role: ModelRole, nonce: u64) -> bool {
            self.bindings.contains(&(role, nonce))
        }
    }

    fn classify<Parent, Role>(resolution: &CreationResolved<Parent, Role>) -> ResolutionClass
    where
        Role: ChildRole<Parent>,
    {
        match resolution {
            CreationResolved::Installed { .. } => ResolutionClass::Installed,
            CreationResolved::Rejected { reason, .. } => match reason {
                CreationRejection::NonceAlreadyBound => ResolutionClass::NonceAlreadyBound,
                CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed) => {
                    ResolutionClass::AddressAlreadyClaimed
                }
                CreationRejection::Allocation(AllocationRejection::Exhausted) => {
                    ResolutionClass::Exhausted
                }
                CreationRejection::InitializationFailed => ResolutionClass::InitializationFailed,
                CreationRejection::EnvironmentFailed => ResolutionClass::EnvironmentFailed,
            },
        }
    }

    fn apply_attempt(
        namespace: &mut ChildNamespace<Parent, ParentBindings>,
        allocator: &mut FreshAllocator,
        attempt: Attempt,
        endpoint_slot: u64,
    ) -> ResolutionClass {
        let kind = if attempt.nonce == 0 {
            CreationKind::Birth
        } else {
            CreationKind::replacement_of(99)
        };
        match attempt.role {
            ModelRole::Primary => {
                let resolution = namespace.realize::<PrimaryRole, PrimaryPosition>(
                    StagedChild::new(attempt.nonce),
                    kind,
                    allocator,
                    endpoint_slot,
                    attempt.installation,
                );
                assert_eq!(resolution.nonce(), attempt.nonce);
                assert_eq!(resolution.kind(), kind);
                classify(&resolution)
            }
            ModelRole::Secondary => {
                let resolution = namespace.realize::<SecondaryRole, SecondaryPosition>(
                    StagedChild::new(attempt.nonce),
                    kind,
                    allocator,
                    endpoint_slot,
                    attempt.installation,
                );
                assert_eq!(resolution.nonce(), attempt.nonce);
                assert_eq!(resolution.kind(), kind);
                classify(&resolution)
            }
        }
    }

    #[test]
    fn all_short_creation_sequences_match_an_independent_model_after_every_step() {
        let attempts = [ModelRole::Primary, ModelRole::Secondary]
            .into_iter()
            .flat_map(|role| {
                [0, 1].into_iter().flat_map(move |nonce| {
                    [
                        Installation::Succeeds,
                        Installation::InitializationFails,
                        Installation::EnvironmentFails,
                    ]
                    .into_iter()
                    .map(move |installation| Attempt {
                        role,
                        nonce,
                        installation,
                    })
                })
            })
            .collect::<Vec<_>>();

        for first in &attempts {
            for second in &attempts {
                for third in &attempts {
                    let mut namespace = ChildNamespace::<Parent, _>::new(parent_bindings());
                    let mut allocator =
                        FreshAllocator::new([RuntimeAddr(30), RuntimeAddr(30), RuntimeAddr(31)]);
                    let mut model = IndependentCreationModel::new();

                    for (step, attempt) in [*first, *second, *third].into_iter().enumerate() {
                        let actual = apply_attempt(
                            &mut namespace,
                            &mut allocator,
                            attempt,
                            100 + step as u64,
                        );
                        let expected = model.apply(attempt);
                        assert_eq!(actual, expected);
                        assert_eq!(allocator.claimed(), model.claimed_addresses);

                        for nonce in [0, 1] {
                            assert_eq!(
                                namespace
                                    .resolve_local::<PrimaryRole, PrimaryPosition>(
                                        StagedChild::new(nonce),
                                    )
                                    .is_some(),
                                model.is_bound(ModelRole::Primary, nonce)
                            );
                            assert_eq!(
                                namespace
                                    .resolve_local::<SecondaryRole, SecondaryPosition>(
                                        StagedChild::new(nonce),
                                    )
                                    .is_some(),
                                model.is_bound(ModelRole::Secondary, nonce)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wrapper_orders_preserve_initialization_effects_creations_and_verdict() {
        type InnerActions = Actions<RuntimeAddr, u8, TraceLane, Births<()>>;
        let base: InnerActions = Actions::new(
            TraceLane(vec![TraceEffect::Base]),
            vec![behavior::Create::birth(7, ())],
            Step::Goto(2),
        );

        let watch_then_timer = base.map_sends(|inner| {
            SendLayer::new(
                TraceLane(vec![TraceEffect::Timer]),
                SendLayer::new(TraceLane(vec![TraceEffect::Observe]), inner),
            )
        });
        assert_eq!(watch_then_timer.creates, [behavior::Create::birth(7, ())]);
        assert_eq!(watch_then_timer.become_, Step::Goto(2));
        let mut first_trace = TraceInterpreter(Vec::new());
        futures_lite::future::block_on(InterpretSends::<_, (), behavior::Here>::interpret(
            watch_then_timer.sends,
            &mut first_trace,
        ))
        .unwrap();
        assert_eq!(
            first_trace.0,
            [TraceEffect::Base, TraceEffect::Observe, TraceEffect::Timer]
        );

        let base: InnerActions = Actions::new(
            TraceLane(vec![TraceEffect::Base]),
            vec![behavior::Create::birth(7, ())],
            Step::Goto(2),
        );
        let timer_then_watch = base.map_sends(|inner| {
            SendLayer::new(
                TraceLane(vec![TraceEffect::Observe]),
                SendLayer::new(TraceLane(vec![TraceEffect::Timer]), inner),
            )
        });
        let mut second_trace = TraceInterpreter(Vec::new());
        futures_lite::future::block_on(InterpretSends::<_, (), behavior::Here>::interpret(
            timer_then_watch.sends,
            &mut second_trace,
        ))
        .unwrap();
        assert_eq!(
            second_trace.0,
            [TraceEffect::Base, TraceEffect::Timer, TraceEffect::Observe]
        );
    }
}
