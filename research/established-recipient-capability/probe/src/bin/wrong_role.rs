use behavior::CreationKind;
use established_recipient_probe::{
    ActorRef, CreationResolved, EstablishedRecipient, Parent, PrimaryRole, RuntimeAddr,
    SecondaryRole,
};

fn main() {
    let primary: CreationResolved<Parent, PrimaryRole> = CreationResolved::Installed {
        nonce: 1,
        kind: CreationKind::Birth,
        recipient: EstablishedRecipient::issued(ActorRef::issued(RuntimeAddr(7), 3)),
    };
    let _: CreationResolved<Parent, SecondaryRole> = primary;
}
