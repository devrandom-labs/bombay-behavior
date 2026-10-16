use std::rc::Rc;

use behavior::{
    Actions, BehaviorActed, Delivery, Inside, InterpreterRequests, MailAddr, MessageProtocol,
    Never, NoBirths, Own, Recipient, ReportToParent, SendEffects, SendLayer, Step, User,
    delegate_transition, initialize,
};
use behavior_actors::{DeliveryOutcomes, WorkQueueSends};

struct Receipt {
    number: u8,
    bytes: Box<[u8]>,
}

struct Invoice;

#[behavior::behavior(
    addr = MailAddr,
    message = Receipt,
    sends = {
        receipts: Vec<Delivery<MessageProtocol<MailAddr, Receipt>>>,
    },
)]
impl Invoice {
    fn receive(&mut self, from: MailAddr, receipt: Receipt) -> BehaviorActed<Self> {
        Ok(Actions::cont().with_send(Delivery::new(Recipient::global(from), receipt)))
    }
}

#[test]
fn inferred_delivery_and_outcome_insertion_preserves_both_original_prefixes() {
    let customer = Recipient::<MessageProtocol<MailAddr, Receipt>>::global(MailAddr(7));
    let audit = Recipient::<MessageProtocol<MailAddr, u32>>::global(MailAddr(13));
    let first = Receipt {
        number: 1,
        bytes: Box::from([1, 2]),
    };
    let second = Receipt {
        number: 3,
        bytes: Box::from([3, 4]),
    };
    let allocations = [first.bytes.as_ptr(), second.bytes.as_ptr()];
    let sends = DeliveryOutcomes {
        deliveries: vec![Delivery::new(customer, first)],
        outcomes: vec![Delivery::new(audit, 5)],
    };
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
        .with_send(Delivery::new(audit, 11))
        .with_send(Delivery::new(customer, second));
    assert_eq!(actions.become_, Step::Continue);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.deliveries.len(), 2);
    for ((delivery, allocation), bytes) in actions
        .sends
        .deliveries
        .into_iter()
        .zip(allocations)
        .zip([[1, 2], [3, 4]])
    {
        assert_eq!(delivery.to, customer);
        assert_eq!(delivery.message.bytes.as_ptr(), allocation);
        assert_eq!(&*delivery.message.bytes, &bytes);
        assert_eq!(delivery.message.number, bytes[0]);
    }
    assert_eq!(actions.sends.outcomes.len(), 2);
    for (delivery, value) in actions.sends.outcomes.into_iter().zip([5, 11]) {
        assert_eq!(delivery.to, audit);
        assert_eq!(delivery.message, value);
    }
}

#[test]
fn generic_product_insertion_preserves_nested_nominal_actor_lanes() {
    let customer = Recipient::<MessageProtocol<MailAddr, Receipt>>::global(MailAddr(7));
    let audit = Recipient::<MessageProtocol<MailAddr, u32>>::global(MailAddr(13));
    let payload = Receipt {
        number: 17,
        bytes: Box::from([17, 19]),
    };
    let allocation = payload.bytes.as_ptr();
    let assignments = InvoiceSends::empty();
    let sends = WorkQueueSends {
        assignments,
        outcomes: vec![Delivery::new(audit, 23)],
    };
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::stop()
        .map_sends(|_: behavior::NoSends| sends)
        .with_send(Delivery::new(customer, payload))
        .with_send(Delivery::new(audit, 29));
    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.assignments.receipts.len(), 1);
    let delivery = actions
        .sends
        .assignments
        .receipts
        .into_iter()
        .next()
        .expect("the original payload is in the nominal actor lane");
    assert_eq!(delivery.to, customer);
    assert_eq!(delivery.message.bytes.as_ptr(), allocation);
    assert_eq!(&*delivery.message.bytes, &[17, 19]);
    assert_eq!(delivery.message.number, 17);
    assert_eq!(actions.sends.outcomes.len(), 2);
    for (delivery, value) in actions.sends.outcomes.into_iter().zip([23, 29]) {
        assert_eq!(delivery.to, audit);
        assert_eq!(delivery.message, value);
    }
}

#[test]
fn inferred_insertion_crosses_a_layer_outside_a_named_product() {
    let customer = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(7));
    let audit = Recipient::<MessageProtocol<MailAddr, u32>>::global(MailAddr(13));
    let sends = SendLayer::new(
        InterpreterRequests::one(ReportToParent::new(31_u64)),
        DeliveryOutcomes {
            deliveries: vec![Delivery::new(customer, 37)],
            outcomes: vec![Delivery::new(audit, 41)],
        },
    );
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
        .with_send(Delivery::new(customer, 43))
        .with_send(Delivery::new(audit, 47))
        .with_send(ReportToParent::new(53_u64));
    assert_eq!(actions.become_, Step::Continue);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.owned.len(), 2);
    for (report, value) in actions
        .sends
        .owned
        .into_requests()
        .into_iter()
        .zip([31, 53])
    {
        assert_eq!(report.report, value);
    }
    assert_eq!(actions.sends.inner.deliveries.len(), 2);
    for (delivery, value) in actions.sends.inner.deliveries.into_iter().zip([37, 43]) {
        assert_eq!(delivery.to, customer);
        assert_eq!(delivery.message, value);
    }
    assert_eq!(actions.sends.inner.outcomes.len(), 2);
    for (delivery, value) in actions.sends.inner.outcomes.into_iter().zip([41, 47]) {
        assert_eq!(delivery.to, audit);
        assert_eq!(delivery.message, value);
    }
}

#[test]
fn inferred_insertion_crosses_a_layer_inside_a_named_product() {
    let customer = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(7));
    let audit = Recipient::<MessageProtocol<MailAddr, u32>>::global(MailAddr(13));
    let sends = WorkQueueSends {
        assignments: SendLayer::new(
            InterpreterRequests::one(ReportToParent::new(59_u64)),
            vec![Delivery::new(customer, 61)],
        ),
        outcomes: vec![Delivery::new(audit, 67)],
    };
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
        .with_send(ReportToParent::new(71_u64))
        .with_send(Delivery::new(audit, 73))
        .with_send(Delivery::new(customer, 79));
    assert_eq!(actions.become_, Step::Continue);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.assignments.owned.len(), 2);
    for (report, value) in actions
        .sends
        .assignments
        .owned
        .into_requests()
        .into_iter()
        .zip([59, 71])
    {
        assert_eq!(report.report, value);
    }
    assert_eq!(actions.sends.assignments.inner.len(), 2);
    for (delivery, value) in actions.sends.assignments.inner.into_iter().zip([61, 79]) {
        assert_eq!(delivery.to, customer);
        assert_eq!(delivery.message, value);
    }
    assert_eq!(actions.sends.outcomes.len(), 2);
    for (delivery, value) in actions.sends.outcomes.into_iter().zip([67, 73]) {
        assert_eq!(delivery.to, audit);
        assert_eq!(delivery.message, value);
    }
}

#[test]
fn duplicate_capabilities_keep_distinct_explicit_static_selections() {
    let mut sends = DeliveryOutcomes {
        deliveries: vec![3_u8],
        outcomes: vec![5_u8],
    };
    sends.send::<_, (Own, Own)>(7_u8);
    sends.send::<_, Inside<(Own, Own)>>(11_u8);
    assert_eq!(sends.deliveries, [3, 7]);
    assert_eq!(sends.outcomes, [5, 11]);
}

#[test]
fn nominal_actor_insertion_keeps_its_existing_unique_semantic_lane() {
    let customer = Recipient::<MessageProtocol<MailAddr, Receipt>>::global(MailAddr(7));
    let mut invoice = Invoice;
    let started = initialize(&mut invoice).expect("ordinary actor initialization");
    assert!(started.sends.receipts.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let first = Receipt {
        number: 83,
        bytes: Box::from([83, 89]),
    };
    let second = Receipt {
        number: 97,
        bytes: Box::from([97, 101]),
    };
    let allocations = [first.bytes.as_ptr(), second.bytes.as_ptr()];
    let received = delegate_transition(
        &mut invoice,
        User {
            from: MailAddr(7),
            message: first,
        },
    )
    .expect("the nominal actor returns the original receipt");
    let actions = received.with_send(Delivery::new(customer, second));
    assert_eq!(actions.become_, Step::Continue);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.receipts.len(), 2);
    for ((delivery, allocation), (number, bytes)) in actions
        .sends
        .receipts
        .into_iter()
        .zip(allocations)
        .zip([(83, [83, 89]), (97, [97, 101])])
    {
        assert_eq!(delivery.to, customer);
        assert_eq!(delivery.message.bytes.as_ptr(), allocation);
        assert_eq!(delivery.message.number, number);
        assert_eq!(&*delivery.message.bytes, &bytes);
    }
}

#[test]
fn local_product_insertion_requires_no_thread_transfer() {
    let first = Rc::new([103_u8, 107]);
    let second = Rc::new([109_u8, 113]);
    let allocations = [Rc::as_ptr(&first), Rc::as_ptr(&second)];
    let sends = DeliveryOutcomes {
        deliveries: vec![first],
        outcomes: vec![127_u32],
    };
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends).with_send(second);
    assert_eq!(actions.become_, Step::Continue);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.outcomes, [127]);
    assert_eq!(actions.sends.deliveries.len(), 2);
    for ((receipt, allocation), bytes) in actions
        .sends
        .deliveries
        .into_iter()
        .zip(allocations)
        .zip([[103, 107], [109, 113]])
    {
        assert_eq!(Rc::as_ptr(&receipt), allocation);
        assert_eq!(*receipt, bytes);
        assert_eq!(Rc::strong_count(&receipt), 1);
    }
}
