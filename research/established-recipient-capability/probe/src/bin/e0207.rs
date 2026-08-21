//! Expected to fail with E0207: `Endpoint` is not selected by `Host` or by
//! the implemented trait, so stable Rust cannot hide it in this impl.

trait EndpointFor<Protocol> {}

trait ChoosesEndpoint<Protocol> {
    type Endpoint;
}

struct Host;

impl<Protocol, Endpoint> ChoosesEndpoint<Protocol> for Host
where
    Endpoint: EndpointFor<Protocol>,
{
    type Endpoint = Endpoint;
}

fn main() {}
