use behavior_actors::atomic::FixedCommand;
use behavior_actors::{Address, EndpointAddress, EstablishedRecipient, Protocol, Recipient};

#[derive(Clone, Copy, Eq, PartialEq)]
struct RuntimeAddress(u64);

impl Address for RuntimeAddress {
    type Nonce = u64;
}

#[derive(Clone)]
struct Endpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

enum SearchRole {
    Search,
    Index,
}

struct SearchService;

impl Protocol for SearchService {
    type Addr = RuntimeAddress;
    type Msg = u8;
}

fn accepts(_: FixedCommand<RuntimeAddress, SearchRole, SearchService>) {}

#[test]
fn logical_status_reply_infers_without_a_route_type() {
    accepts(FixedCommand::status(Recipient::global(RuntimeAddress(7))));
}

#[test]
fn exact_capability_reply_infers_without_a_route_type() {
    accepts(FixedCommand::capability(
        SearchRole::Search,
        EstablishedRecipient::issued(Endpoint),
    ));
}

#[test]
fn shutdown_needs_no_reply_placeholder() {
    accepts(FixedCommand::shutdown());
}

#[test]
fn every_domain_role_variant_is_usable() {
    accepts(FixedCommand::capability(
        SearchRole::Index,
        Recipient::global(RuntimeAddress(9)),
    ));
}
