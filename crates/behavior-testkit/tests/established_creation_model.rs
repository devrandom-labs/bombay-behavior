//! Independent state-model evidence for exact established creation facts.

use core::marker::PhantomData;
use foundation::{
    Address, AllocationRejection, CreationKind, CreationRejection, EndpointAddress,
    EstablishedCreation, EstablishedRecipient, Protocol,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Debug, PartialEq, Eq)]
struct Endpoint<P> {
    address: RuntimeAddr,
    protocol: PhantomData<fn() -> P>,
}

impl<P> Endpoint<P> {
    const fn new(address: RuntimeAddr) -> Self {
        Self {
            address,
            protocol: PhantomData,
        }
    }
}

impl<P> Copy for Endpoint<P> {}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

enum Primary {}
enum Secondary {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Installation {
    Succeeds,
    InitializationFails,
    EnvironmentFails,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attempt {
    role: Role,
    nonce: u64,
    installation: Installation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolution {
    Installed,
    NonceAlreadyBound,
    AllocationExhausted,
    AddressAlreadyClaimed,
    InitializationFailed,
    EnvironmentFailed,
}

trait RoleBindings<RoleMarker> {
    fn bindings(&self) -> &[(u64, RuntimeAddr)];
    fn bindings_mut(&mut self) -> &mut Vec<(u64, RuntimeAddr)>;
}

struct FreshClaims {
    candidates: Vec<RuntimeAddr>,
    next: usize,
    claimed: Vec<RuntimeAddr>,
}

impl FreshClaims {
    fn new(candidates: impl IntoIterator<Item = RuntimeAddr>) -> Self {
        Self {
            candidates: candidates.into_iter().collect(),
            next: 0,
            claimed: Vec::new(),
        }
    }

    fn claim(&mut self) -> Result<RuntimeAddr, AllocationRejection> {
        let Some(candidate) = self.candidates.get(self.next).copied() else {
            return Err(AllocationRejection::Exhausted);
        };
        self.next += 1;
        if self.claimed.contains(&candidate) {
            return Err(AllocationRejection::AddressAlreadyClaimed);
        }
        self.claimed.push(candidate);
        Ok(candidate)
    }
}

struct Harness {
    allocation: FreshClaims,
    claimed_nonces: Vec<u64>,
    primary: Vec<(u64, RuntimeAddr)>,
    secondary: Vec<(u64, RuntimeAddr)>,
}

impl Harness {
    fn new(candidates: impl IntoIterator<Item = RuntimeAddr>) -> Self {
        Self {
            allocation: FreshClaims::new(candidates),
            claimed_nonces: Vec::new(),
            primary: Vec::new(),
            secondary: Vec::new(),
        }
    }

    fn realize<RoleMarker>(&mut self, nonce: u64, installation: Installation) -> Resolution
    where
        Self: RoleBindings<RoleMarker>,
    {
        if self.claimed_nonces.contains(&nonce) {
            return classify(EstablishedCreation::<Worker, RoleMarker>::rejected(
                nonce,
                CreationKind::Birth,
                CreationRejection::NonceAlreadyBound,
            ));
        }
        let address = match self.allocation.claim() {
            Ok(address) => address,
            Err(reason) => {
                return classify(EstablishedCreation::<Worker, RoleMarker>::rejected(
                    nonce,
                    CreationKind::Birth,
                    CreationRejection::Allocation(reason),
                ));
            }
        };
        let rejection = match installation {
            Installation::Succeeds => None,
            Installation::InitializationFails => Some(CreationRejection::InitializationFailed),
            Installation::EnvironmentFails => Some(CreationRejection::EnvironmentFailed),
        };
        if let Some(reason) = rejection {
            return classify(EstablishedCreation::<Worker, RoleMarker>::rejected(
                nonce,
                CreationKind::Birth,
                reason,
            ));
        }

        self.claimed_nonces.push(nonce);
        <Self as RoleBindings<RoleMarker>>::bindings_mut(self).push((nonce, address));
        classify(EstablishedCreation::<Worker, RoleMarker>::installed(
            nonce,
            CreationKind::Birth,
            EstablishedRecipient::issued(Endpoint::new(address)),
        ))
    }
}

impl RoleBindings<Primary> for Harness {
    fn bindings(&self) -> &[(u64, RuntimeAddr)] {
        &self.primary
    }

    fn bindings_mut(&mut self) -> &mut Vec<(u64, RuntimeAddr)> {
        &mut self.primary
    }
}

impl RoleBindings<Secondary> for Harness {
    fn bindings(&self) -> &[(u64, RuntimeAddr)] {
        &self.secondary
    }

    fn bindings_mut(&mut self) -> &mut Vec<(u64, RuntimeAddr)> {
        &mut self.secondary
    }
}

fn classify<RoleMarker>(fact: EstablishedCreation<Worker, RoleMarker>) -> Resolution {
    match fact {
        EstablishedCreation::Installed { .. } => Resolution::Installed,
        EstablishedCreation::Rejected { reason, .. } => match reason {
            CreationRejection::NonceAlreadyBound => Resolution::NonceAlreadyBound,
            CreationRejection::Allocation(AllocationRejection::Exhausted) => {
                Resolution::AllocationExhausted
            }
            CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed) => {
                Resolution::AddressAlreadyClaimed
            }
            CreationRejection::InitializationFailed => Resolution::InitializationFailed,
            CreationRejection::EnvironmentFailed => Resolution::EnvironmentFailed,
        },
    }
}

/// Deliberately uses different state vocabulary and transition structure from
/// the capability harness above.
struct ReferenceWorld {
    ticket: u64,
    occupied_routes: [bool; 2],
    locations: [[Option<RuntimeAddr>; 2]; 2],
    allocated: Vec<RuntimeAddr>,
}

impl ReferenceWorld {
    fn new() -> Self {
        Self {
            ticket: 1_000,
            occupied_routes: [false; 2],
            locations: [[None; 2]; 2],
            allocated: Vec::new(),
        }
    }

    fn step(&mut self, attempt: Attempt) -> Resolution {
        let nonce = usize::try_from(attempt.nonce).unwrap();
        if self.occupied_routes[nonce] {
            return Resolution::NonceAlreadyBound;
        }
        let address = RuntimeAddr(self.ticket);
        self.ticket += 1;
        self.allocated.push(address);
        match attempt.installation {
            Installation::InitializationFails => Resolution::InitializationFailed,
            Installation::EnvironmentFails => Resolution::EnvironmentFailed,
            Installation::Succeeds => {
                self.occupied_routes[nonce] = true;
                let role = match attempt.role {
                    Role::Primary => 0,
                    Role::Secondary => 1,
                };
                self.locations[role][nonce] = Some(address);
                Resolution::Installed
            }
        }
    }
}

fn attempts() -> Vec<Attempt> {
    [Role::Primary, Role::Secondary]
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
        .collect()
}

fn apply(harness: &mut Harness, attempt: Attempt) -> Resolution {
    match attempt.role {
        Role::Primary => harness.realize::<Primary>(attempt.nonce, attempt.installation),
        Role::Secondary => harness.realize::<Secondary>(attempt.nonce, attempt.installation),
    }
}

fn ordered(mut bindings: Vec<(u64, RuntimeAddr)>) -> Vec<(u64, RuntimeAddr)> {
    bindings.sort_by_key(|(nonce, _)| *nonce);
    bindings
}

#[test]
fn all_three_attempt_sequences_match_the_independent_creation_model() {
    let choices = attempts();
    let mut checked = 0;
    for first in &choices {
        for second in &choices {
            for third in &choices {
                let sequence = [*first, *second, *third];
                let mut harness = Harness::new((1_000..1_003).map(RuntimeAddr));
                let mut model = ReferenceWorld::new();
                for attempt in sequence {
                    assert_eq!(apply(&mut harness, attempt), model.step(attempt));
                    assert_eq!(harness.allocation.claimed, model.allocated);
                    assert_eq!(
                        ordered(<Harness as RoleBindings<Primary>>::bindings(&harness).to_vec()),
                        ordered(
                            model.locations[0]
                                .iter()
                                .enumerate()
                                .filter_map(|(nonce, address)| {
                                    address.map(|address| (u64::try_from(nonce).unwrap(), address))
                                })
                                .collect::<Vec<_>>()
                        )
                    );
                    assert_eq!(
                        ordered(<Harness as RoleBindings<Secondary>>::bindings(&harness).to_vec()),
                        ordered(
                            model.locations[1]
                                .iter()
                                .enumerate()
                                .filter_map(|(nonce, address)| {
                                    address.map(|address| (u64::try_from(nonce).unwrap(), address))
                                })
                                .collect::<Vec<_>>()
                        )
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 1_728);
}

#[test]
fn allocator_collision_is_rejected_without_binding_the_nonce() {
    let mut harness = Harness::new([RuntimeAddr(5), RuntimeAddr(5), RuntimeAddr(8)]);
    assert_eq!(
        harness.realize::<Primary>(0, Installation::InitializationFails),
        Resolution::InitializationFailed
    );
    assert_eq!(
        harness.realize::<Secondary>(0, Installation::Succeeds),
        Resolution::AddressAlreadyClaimed
    );
    assert!(harness.claimed_nonces.is_empty());
    assert!(harness.primary.is_empty());
    assert!(harness.secondary.is_empty());
}
