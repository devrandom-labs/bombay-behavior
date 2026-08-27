//! Independent interpreter checks for creation-only heterogeneous birth sums.

use std::collections::BTreeSet;

use foundation::{
    Actions, Behavior, BehaviorActed, Births, ChildChoice, Create, CreationKind, DispatchBirth,
    InstallBirth, MailAddr, Never, NoBirths, Recipient, User,
};
use proptest::prelude::*;

struct DeviceGroups;
impl behavior::Protocol for DeviceGroups {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for DeviceGroups {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Queries;
impl behavior::Protocol for Queries {
    type Addr = MailAddr;
    type Msg = &'static str;
}

impl Behavior for Queries {
    type Protocol = Self;
    type Event = User<MailAddr, &'static str>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type IoTChildren = ChildChoice<DeviceGroups, ChildChoice<Queries, Never>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstalledKind {
    DeviceGroups,
    Queries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallError {
    Collision,
}

struct ModelInstaller {
    occupied: BTreeSet<u64>,
    trace: Vec<(u64, InstalledKind, CreationKind<u64>)>,
    device_groups: Vec<Recipient<DeviceGroups>>,
    queries: Vec<Recipient<Queries>>,
    next_address: u64,
}

impl ModelInstaller {
    fn admit(
        &mut self,
        nonce: u64,
        kind: InstalledKind,
        provenance: CreationKind<u64>,
    ) -> Result<(), InstallError> {
        if !self.occupied.insert(nonce) {
            return Err(InstallError::Collision);
        }
        self.trace.push((nonce, kind, provenance));
        Ok(())
    }
}

impl InstallBirth<foundation::ChildHead, DeviceGroups, (), InstallError> for ModelInstaller {
    async fn install_birth(
        &mut self,
        creation: Create<MailAddr, DeviceGroups>,
    ) -> Result<(), InstallError> {
        self.admit(creation.nonce, InstalledKind::DeviceGroups, creation.kind)?;
        self.device_groups
            .push(Recipient::global(MailAddr(self.next_address)));
        self.next_address += 1;
        Ok(())
    }
}

impl InstallBirth<foundation::ChildTail<foundation::ChildHead>, Queries, (), InstallError>
    for ModelInstaller
{
    async fn install_birth(
        &mut self,
        creation: Create<MailAddr, Queries>,
    ) -> Result<(), InstallError> {
        self.admit(creation.nonce, InstalledKind::Queries, creation.kind)?;
        self.queries
            .push(Recipient::global(MailAddr(self.next_address)));
        self.next_address += 1;
        Ok(())
    }
}

fn installer() -> ModelInstaller {
    ModelInstaller {
        occupied: BTreeSet::new(),
        trace: Vec::new(),
        device_groups: Vec::new(),
        queries: Vec::new(),
        next_address: 1_000,
    }
}

async fn dispatch(
    child: IoTChildren,
    nonce: u64,
    kind: CreationKind<u64>,
    installer: &mut ModelInstaller,
) -> Result<(), InstallError> {
    <IoTChildren as DispatchBirth<MailAddr, ModelInstaller, (), InstallError>>::dispatch_birth(
        child, nonce, kind, installer,
    )
    .await
}

fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn one_ordered_creation_vector_dispatches_to_concrete_protocol_installers() {
    let actions: Actions<MailAddr, Never, Vec<Never>, Births<IoTChildren>> = Actions::create(vec![
        Create::birth(9, ChildChoice::Head(DeviceGroups)),
        Create::replacement_incarnation(4, 2, ChildChoice::Tail(ChildChoice::Head(Queries))),
        Create::birth(7, ChildChoice::Tail(ChildChoice::Head(Queries))),
    ]);
    let mut model = installer();
    for creation in actions.creates {
        dispatch(creation.child, creation.nonce, creation.kind, &mut model)
            .await
            .unwrap();
    }

    assert_eq!(
        model.trace,
        [
            (9, InstalledKind::DeviceGroups, CreationKind::Birth),
            (4, InstalledKind::Queries, CreationKind::replacement_of(2)),
            (7, InstalledKind::Queries, CreationKind::Birth),
        ]
    );
    let _: Recipient<DeviceGroups> = model.device_groups[0];
    let _: Recipient<Queries> = model.queries[0];
}

#[tokio::test]
async fn nonce_collision_is_global_across_variants_and_preserves_the_first_binding() {
    let mut model = installer();
    let first = dispatch(
        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Head(DeviceGroups),
        5,
        CreationKind::Birth,
        &mut model,
    );
    assert_send(&first);
    first.await.unwrap();
    let collision = dispatch(
        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Tail(ChildChoice::Head(Queries)),
        5,
        CreationKind::Birth,
        &mut model,
    );
    assert_send(&collision);
    assert_eq!(collision.await, Err(InstallError::Collision));
    assert_eq!(
        model.trace,
        [(5, InstalledKind::DeviceGroups, CreationKind::Birth)]
    );
    assert_eq!(model.device_groups, [Recipient::global(MailAddr(1_000))]);
    assert!(model.queries.is_empty());
}

proptest! {
    #[test]
    fn arbitrary_cross_variant_sequences_match_one_global_nonce_model(
        inputs in proptest::collection::vec((any::<bool>(), 0_u8..12), 0..80)
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut installer = installer();
        let mut occupied = BTreeSet::new();
        let mut expected = Vec::new();
        for (device, raw_nonce) in inputs {
            let nonce = u64::from(raw_nonce);
            let expected_result = if occupied.insert(nonce) {
                expected.push((
                    nonce,
                    if device { InstalledKind::DeviceGroups } else { InstalledKind::Queries },
                    CreationKind::Birth,
                ));
                Ok(())
            } else {
                Err(InstallError::Collision)
            };
            let actual = runtime.block_on(async {
                if device {
                    dispatch(
                        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Head(DeviceGroups),
                        nonce,
                        CreationKind::Birth,
                        &mut installer,
                    ).await
                } else {
                    dispatch(
                        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Tail(
                            ChildChoice::Head(Queries),
                        ),
                        nonce,
                        CreationKind::Birth,
                        &mut installer,
                    ).await
                }
            });
            prop_assert_eq!(actual, expected_result);
            prop_assert_eq!(&installer.trace, &expected);
        }
    }
}
