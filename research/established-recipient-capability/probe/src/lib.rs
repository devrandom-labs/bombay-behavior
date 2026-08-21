//! Standalone representation probe; this is not a proposed production API.

use behavior::{Address, Protocol};
use core::marker::PhantomData;

/// One endpoint family selected by an address/runtime namespace.
pub trait EndpointAddress: Address + Sized {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

/// A fake direct runtime endpoint, kept concrete and protocol-indexed.
pub struct ActorRef<P: Protocol> {
    slot: u64,
    protocol: PhantomData<fn(P::Msg) -> P>,
}

impl<P: Protocol> ActorRef<P> {
    #[must_use]
    pub const fn issued(slot: u64) -> Self {
        Self {
            slot,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> u64 {
        self.slot
    }
}

impl<P: Protocol> Clone for ActorRef<P> {
    fn clone(&self) -> Self {
        Self::issued(self.slot)
    }
}

/// Logical namespace selected by domain protocols; the runtime implements its
/// endpoint family once rather than every protocol supplying an endpoint type.
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

/// Candidate inert established capability. Its public type mentions only the
/// canonical protocol; the concrete endpoint is an associated projection.
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

/// Delivery remains inert data; only an interpreter can use its endpoint.
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

    /// Cross the explicit interpreter boundary without exposing the endpoint
    /// through a recipient operation available to the behavior fold.
    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablished<P>,
    {
        interpreter.interpret(self.to.endpoint, self.message)
    }
}

/// Static runtime capability for one exact protocol endpoint family.
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

/// A route exists before an endpoint does and is meaningful only in the
/// creator's child namespace.
pub struct StagedChild<P>
where
    P: Protocol,
{
    nonce: <P::Addr as Address>::Nonce,
    protocol: PhantomData<fn() -> P>,
}

impl<P> StagedChild<P>
where
    P: Protocol,
{
    #[must_use]
    pub const fn new(nonce: <P::Addr as Address>::Nonce) -> Self {
        Self {
            nonce,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn nonce(self) -> <P::Addr as Address>::Nonce {
        self.nonce
    }
}

impl<P: Protocol> Copy for StagedChild<P> {}

impl<P: Protocol> Clone for StagedChild<P> {
    fn clone(&self) -> Self {
        *self
    }
}

pub enum Target<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    Established(EstablishedRecipient<P>),
    LocalChild(StagedChild<P>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationRejection {
    NonceAlreadyBound,
    InstallationFailed,
}

/// A committed fact owns a capability only in the successful variant.
pub struct CreationCommitted<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub nonce: <P::Addr as Address>::Nonce,
    pub result: Result<EstablishedRecipient<P>, CreationRejection>,
}

/// Minimal creator-local binding probe for one statically known child
/// protocol. This is not an application-wide protocol registry.
pub struct ChildNamespace<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    bindings: Vec<(u64, ActorRef<P>)>,
}

impl<P> ChildNamespace<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn commit(&mut self, nonce: u64, endpoint: ActorRef<P>) -> CreationCommitted<P> {
        if self.bindings.iter().any(|(bound, _)| *bound == nonce) {
            return CreationCommitted {
                nonce,
                result: Err(CreationRejection::NonceAlreadyBound),
            };
        }
        self.bindings.push((nonce, endpoint.clone()));
        CreationCommitted {
            nonce,
            result: Ok(EstablishedRecipient::issued(endpoint)),
        }
    }

    fn resolve(&self, target: Target<P>) -> Option<ActorRef<P>> {
        match target {
            Target::Established(recipient) => Some(recipient.endpoint),
            Target::LocalChild(child) => self
                .bindings
                .iter()
                .find(|(nonce, _)| *nonce == child.nonce())
                .map(|(_, endpoint)| endpoint.clone()),
        }
    }

    /// Observation-only probe surface; production delivery would keep the
    /// endpoint inside its interpreter just as `EstablishedDelivery` does.
    #[must_use]
    pub fn resolved_slot(&self, target: Target<P>) -> Option<u64> {
        self.resolve(target).map(|endpoint| endpoint.slot())
    }
}

impl<P> Default for ChildNamespace<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    fn default() -> Self {
        Self::new()
    }
}

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

/// No endpoint parameter appears in this transferable domain message.
pub struct Transfer {
    pub worker: EstablishedRecipient<Worker>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_creation_returns_capability_and_enables_same_action_child_send() {
        let child = StagedChild::<Worker>::new(7);
        let mut namespace = ChildNamespace::new();
        let committed = namespace.commit(child.nonce(), ActorRef::issued(41));

        assert_eq!(namespace.resolved_slot(Target::LocalChild(child)), Some(41));

        let recipient = committed.result.expect("fresh binding commits");
        let transferred = Transfer {
            worker: recipient.clone(),
        };
        assert_eq!(
            namespace.resolved_slot(Target::Established(transferred.worker)),
            Some(41)
        );
    }

    #[test]
    fn collision_returns_no_capability_and_preserves_first_binding() {
        let mut namespace = ChildNamespace::<Worker>::new();
        assert!(namespace.commit(7, ActorRef::issued(41)).result.is_ok());
        assert!(matches!(
            namespace.commit(7, ActorRef::issued(99)).result,
            Err(CreationRejection::NonceAlreadyBound)
        ));
        assert_eq!(
            namespace.resolved_slot(Target::LocalChild(StagedChild::new(7))),
            Some(41)
        );
    }
}
