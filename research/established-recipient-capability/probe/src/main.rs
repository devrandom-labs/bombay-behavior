use behavior::CreationKind;
use established_recipient_probe::{
    ActorRef, ChildNamespace, CreationResolved, EstablishedDelivery, FreshAllocator, Installation,
    InterpretEstablished, Parent, PrimaryRole, RoleHead, StagedChild, Transfer, Worker,
    parent_bindings,
};

struct Trace(Option<(u64, u8)>);

impl InterpretEstablished<Worker> for Trace {
    type Output = ();

    fn interpret(&mut self, endpoint: ActorRef<Worker>, message: u8) {
        self.0 = Some((endpoint.slot(), message));
    }
}

fn main() {
    let staged = StagedChild::<Parent, PrimaryRole>::new(7);
    let mut allocator = FreshAllocator::new([established_recipient_probe::RuntimeAddr(9)]);
    let mut namespace = ChildNamespace::new(parent_bindings());
    let committed = namespace.realize::<PrimaryRole, RoleHead>(
        staged,
        CreationKind::Birth,
        &mut allocator,
        41,
        Installation::Succeeds,
    );
    assert!(matches!(committed, CreationResolved::Installed { .. }));

    let transfer = Transfer {
        worker: committed.into_recipient().expect("fresh child committed"),
    };
    let mut trace = Trace(None);
    EstablishedDelivery::new(transfer.worker, 11).interpret(&mut trace);
    assert_eq!(trace.0, Some((41, 11)));
}
