use behavior::{Address, Behavior, EndpointAddress, Protocol};

#[path = "../installed_control/mod.rs"]
mod installed_control;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAddr(pub u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint(pub u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;

    type Installed<B>
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed.endpoint()
    }
}
