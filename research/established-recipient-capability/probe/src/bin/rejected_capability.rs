use behavior::CreationKind;
use established_recipient_probe::{CreationRejection, CreationResolved, Parent, PrimaryRole};

fn main() {
    let rejected: CreationResolved<Parent, PrimaryRole> = CreationResolved::Rejected {
        nonce: 1,
        kind: CreationKind::Birth,
        reason: CreationRejection::EnvironmentFailed,
    };
    let _ = rejected.recipient;
}
