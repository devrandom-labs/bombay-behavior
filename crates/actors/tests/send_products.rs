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

// Legal caller names must not become reserved macro implementation details.
macro_rules! send_names {
    ($($module:ident: $parameter:ident),+ $(,)?) => {$ (
        mod $module {
            use super::*;

            #[derive(behavior_macros::SendProduct)]
            struct Notices<$parameter, Receipts> {
                invoices: $parameter,
                receipts: Receipts,
            }

            struct Clerk<$parameter>(core::marker::PhantomData<$parameter>);

            #[behavior::behavior(
                addr = MailAddr,
                message = $parameter,
                sends = {
                    invoices: Vec<Delivery<MessageProtocol<MailAddr, $parameter>>>,
                    receipts: Vec<Delivery<MessageProtocol<MailAddr, u32>>>,
                },
            )]
            impl<$parameter> Clerk<$parameter> {
                fn receive(&mut self, from: MailAddr, message: $parameter) -> BehaviorActed<Self> {
                    Ok(Actions::cont()
                        .send_invoices(Delivery::new(Recipient::global(from), message))
                        .send_receipts(Delivery::new(Recipient::global(from), 17)))
                }
            }

            #[test]
            fn generic_and_nominal_products_preserve_originals() {
                let invoice = Recipient::<MessageProtocol<MailAddr, Box<[u8]>>>::global(MailAddr(7));
                let receipt = Recipient::<MessageProtocol<MailAddr, u32>>::global(MailAddr(11));
                let bytes: Box<[u8]> = Box::from([3, 5]);
                let allocation = bytes.as_ptr();
                let notices = Notices {
                    invoices: Vec::<Delivery<MessageProtocol<MailAddr, Box<[u8]>>>>::new(),
                    receipts: Vec::<Delivery<MessageProtocol<MailAddr, u32>>>::new(),
                };
                let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(notices)
                    .with_send(Delivery::new(invoice, bytes))
                    .with_send(Delivery::new(receipt, 13));
                assert_eq!(actions.become_, Step::Continue);
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.invoices.len(), 1);
                assert_eq!(actions.sends.receipts.len(), 1);
                let delivery = actions.sends.invoices.into_iter().next().expect("one original invoice");
                assert_eq!(delivery.to, invoice);
                assert_eq!(delivery.message.as_ptr(), allocation);
                assert_eq!(&*delivery.message, &[3, 5]);
                let acknowledgment = &actions.sends.receipts[0];
                assert_eq!(acknowledgment.to, receipt);
                assert_eq!(acknowledgment.message, 13);

                let mut clerk = Clerk::<Box<[u8]>>(core::marker::PhantomData);
                let actions = delegate_transition(&mut clerk, User { from: invoice.address(), message: delivery.message })
                    .expect("named lane methods retain the original invoice")
                    .with_send(Delivery::new(receipt, 19));
                assert_eq!(actions.become_, Step::Continue);
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.invoices.len(), 1);
                assert_eq!(actions.sends.receipts.len(), 2);
                let delivery = &actions.sends.invoices[0];
                assert_eq!(delivery.to, invoice);
                assert_eq!(delivery.message.as_ptr(), allocation);
                assert_eq!(&*delivery.message, &[3, 5]);
                for (delivery, (to, message)) in actions.sends.receipts.into_iter().zip([(Recipient::global(invoice.address()), 17), (receipt, 19)]) {
                    assert_eq!(delivery.to, to);
                    assert_eq!(delivery.message, message);
                }
            }
        }
    )+};
}

send_names!(
    event: __BombayEvent,
    interpreter: __BombayInterpreter,
    root: __BombayRootEvent,
    path: __BombayPath,
    host: __BombaySettlementHost,
    settlement: __BombaySettlementEvent,
    input: __BombayInput,
    route: __BombaySendPath,
    address: __BombayAddress,
    phase: __BombayPh,
    birth: __BombayBirth,
    raw: r#__BombayInput,
);

mod aliases {
    use super::*;

    type __BombayInput = Vec<Delivery<MessageProtocol<MailAddr, Box<[u8]>>>>;
    type __BombayInterpreter = Vec<Delivery<MessageProtocol<MailAddr, u32>>>;
    type __BombayAddress = Vec<Delivery<MessageProtocol<MailAddr, u16>>>;
    struct Clerk;

    #[behavior::behavior(
        addr = MailAddr,
        message = Box<[u8]>,
        sends = {
            invoices: __BombayInput,
            receipts: __BombayInterpreter,
            notices: __BombayAddress,
        },
    )]
    impl Clerk {
        fn receive(&mut self, from: MailAddr, message: Box<[u8]>) -> BehaviorActed<Self> {
            Ok(Actions::cont()
                .send_invoices(Delivery::new(Recipient::global(from), message))
                .send_receipts(Delivery::new(Recipient::global(from), 23))
                .send_notices(Delivery::new(Recipient::global(from), 29)))
        }
    }

    #[test]
    fn referenced_aliases_keep_their_meanings() {
        let mut clerk = Clerk;
        let message: Box<[u8]> = Box::from([31, 37]);
        let allocation = message.as_ptr();
        let actions = delegate_transition(
            &mut clerk,
            User {
                from: MailAddr(41),
                message,
            },
        )
        .expect("concrete aliases denote the original delivery products");
        assert_eq!(actions.become_, Step::Continue);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.sends.invoices.len(), 1);
        assert_eq!(
            actions.sends.invoices[0].to,
            Recipient::global(MailAddr(41))
        );
        assert_eq!(actions.sends.invoices[0].message.as_ptr(), allocation);
        assert_eq!(&*actions.sends.invoices[0].message, &[31, 37]);
        assert_eq!(actions.sends.receipts.len(), 1);
        assert_eq!(
            actions.sends.receipts[0].to,
            Recipient::global(MailAddr(41))
        );
        assert_eq!(actions.sends.receipts[0].message, 23);
        assert_eq!(actions.sends.notices.len(), 1);
        assert_eq!(actions.sends.notices[0].to, Recipient::global(MailAddr(41)));
        assert_eq!(actions.sends.notices[0].message, 29);
    }
}

mod suffixes {
    use super::*;

    #[derive(behavior_macros::SendProduct)]
    struct Notices<
        r#__BombayInput,
        __BombayInput_,
        __BombayInput__,
        __BombaySendPath,
        __BombaySendPath_,
    > {
        invoices: r#__BombayInput,
        receipts: __BombayInput_,
        notices: __BombayInput__,
        reminders: __BombaySendPath,
        letters: __BombaySendPath_,
    }

    #[test]
    fn occupied_fallback_names_remain_ordinary_lane_parameters() {
        let sends = Notices {
            invoices: vec![3_u8],
            receipts: vec![5_u16],
            notices: vec![7_u32],
            reminders: vec![11_u64],
            letters: vec![13_u128],
        };
        let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
            .with_send(17_u8)
            .with_send(19_u16)
            .with_send(23_u32)
            .with_send(29_u64)
            .with_send(31_u128);
        assert_eq!(actions.become_, Step::Continue);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.sends.invoices, [3, 17]);
        assert_eq!(actions.sends.receipts, [5, 19]);
        assert_eq!(actions.sends.notices, [7, 23]);
        assert_eq!(actions.sends.reminders, [11, 29]);
        assert_eq!(actions.sends.letters, [13, 31]);
    }
}

mod bounds {
    use super::*;
    use core::marker::Sized as __BombayInput;

    #[derive(behavior_macros::SendProduct)]
    struct Notices<Invoices, Receipts>
    where
        Invoices: __BombayInput,
    {
        invoices: Invoices,
        receipts: Receipts,
    }

    #[test]
    fn referenced_bounds_keep_their_meaning() {
        let notices = Notices {
            invoices: vec![3_u8],
            receipts: vec![5_u16],
        };
        let actions: Actions<MailAddr, Never, _, NoBirths> =
            Actions::send(notices).with_send(7_u8).with_send(11_u16);
        assert_eq!(actions.become_, Step::Continue);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.sends.invoices, [3, 7]);
        assert_eq!(actions.sends.receipts, [5, 11]);
    }
}

#[allow(
    non_upper_case_globals,
    reason = "the const name challenges a generated type binder"
)]
mod arrays {
    use super::*;

    struct Clerk<'a, const __BombayInput: usize> {
        retained: &'a [u8; __BombayInput],
    }

    #[behavior::behavior(
        addr = MailAddr,
        message = [u8; __BombayInput],
        sends = {
            arrays: Vec<Delivery<MessageProtocol<MailAddr, [u8; __BombayInput]>>>,
            references: Vec<Delivery<MessageProtocol<MailAddr, &'a [u8; __BombayInput]>>>,
        },
    )]
    impl<'a, const __BombayInput: usize> Clerk<'a, __BombayInput> {
        fn receive(&mut self, from: MailAddr, message: [u8; __BombayInput]) -> BehaviorActed<Self> {
            Ok(Actions::cont()
                .send_arrays(Delivery::new(Recipient::global(from), message))
                .send_references(Delivery::new(Recipient::global(from), self.retained)))
        }
    }

    #[test]
    fn lifetimes_and_const_parameters_keep_their_domains() {
        let retained = [13, 17];
        let mut clerk = Clerk {
            retained: &retained,
        };
        let actions = delegate_transition(
            &mut clerk,
            User {
                from: MailAddr(19),
                message: [23, 29],
            },
        )
        .expect("existing borrowed and const-parameterized lanes remain usable");
        assert_eq!(actions.become_, Step::Continue);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.sends.arrays.len(), 1);
        assert_eq!(actions.sends.arrays[0].to, Recipient::global(MailAddr(19)));
        assert_eq!(actions.sends.arrays[0].message, [23, 29]);
        assert_eq!(actions.sends.references.len(), 1);
        assert_eq!(
            actions.sends.references[0].to,
            Recipient::global(MailAddr(19))
        );
        assert!(core::ptr::eq(
            actions.sends.references[0].message,
            &retained
        ));
        assert_eq!(*actions.sends.references[0].message, [13, 17]);
    }
}

macro_rules! product_names {
    ($($module:ident: $name:ident),+ $(,)?) => {$ (
        mod $module {
            use super::*;

            #[derive(behavior_macros::SendProduct)]
            struct $name<Invoices, Receipts> {
                invoices: Invoices,
                receipts: Receipts,
            }

            #[test]
            fn product_names_remain_visible_inside_generated_contracts() {
                let sends = $name { invoices: vec![3_u8], receipts: vec![5_u16] };
                let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
                    .with_send(7_u8).with_send(11_u16);
                assert_eq!(actions.become_, Step::Continue);
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.invoices, [3, 7]);
                assert_eq!(actions.sends.receipts, [5, 11]);
            }
        }
    )+};
}

product_names!(
    invoice: __BombayInput,
    notice: __BombayEvent,
    receipt: __BombaySettlementHost,
);
