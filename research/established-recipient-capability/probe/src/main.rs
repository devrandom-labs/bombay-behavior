use established_recipient_probe::{
    ActorRef, ChildNamespace, EstablishedDelivery, EstablishedRecipient, InterpretEstablished,
    Queue, RuntimeAddr, StagedChild, Target, Transfer, Worker,
};

struct Trace(Option<(u64, u8)>);

impl InterpretEstablished<Queue> for Trace {
    type Output = ();

    fn interpret(&mut self, endpoint: ActorRef<Queue>, message: u8) {
        self.0 = Some((endpoint.slot(), message));
    }
}

fn main() {
    let queue = EstablishedRecipient::<Queue>::issued(ActorRef::issued(3));
    let delivery = EstablishedDelivery::new(queue, 11);
    let mut trace = Trace(None);
    delivery.interpret(&mut trace);
    assert_eq!(trace.0, Some((3, 11)));

    let staged = StagedChild::<Worker>::new(7);
    let mut namespace = ChildNamespace::new();
    let committed = namespace.commit(staged.nonce(), ActorRef::issued(41));
    assert_eq!(
        namespace.resolved_slot(Target::LocalChild(staged)),
        Some(41)
    );

    let transfer = Transfer {
        worker: committed.result.expect("fresh child committed"),
    };
    assert_eq!(
        namespace.resolved_slot(Target::Established(transfer.worker)),
        Some(41)
    );

    let _address_type_remains_separate = RuntimeAddr(9);
}
