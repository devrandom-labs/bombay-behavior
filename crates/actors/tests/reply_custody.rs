mod installed_control;

use behavior::{
    ActionItemResult, Address, Delivery, EndpointAddress, EstablishedDelivery,
    EstablishedRecipient, ExactDeliveryReason, InterpreterFault, ItemSettlement,
    LogicalDeliveryReason, MessageProtocol, Never, Protocol, Recipient, SettledItem, SourceCustody,
    SourceProgress, SourceSettlementCustody,
};
use behavior_actors::composition::{ReplyDeliveries, ReplyDelivery};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddress(u64);
impl Address for RuntimeAddress {
    type Nonce = u64;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvoiceEndpoint(u64);

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = InvoiceEndpoint
    where
        P: Protocol<Addr = Self>;
    type Installed<B>
        = installed_control::InstalledControl<B, InvoiceEndpoint>
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>;
    fn recipient<B>(installed: &Self::Installed<B>) -> InvoiceEndpoint
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
}

struct Invoice {
    number: u64,
    charges: Box<[u8; 2]>,
}
type Invoices = MessageProtocol<RuntimeAddress, Invoice>;
type ReplySettlements = ReplyDeliveries<
    ActionItemResult<Delivery<Invoices>>,
    ActionItemResult<EstablishedDelivery<Invoices>>,
>;

async fn settle_replies<S>(settlements: S) -> SourceCustody<S>
where
    S: SourceSettlementCustody<(), Never>,
{
    let mut progress = Some(SourceProgress::Original(settlements));
    S::prepare_source(&mut progress);
    if let Some(SourceProgress::Offering(custody)) = &mut progress {
        S::offer_next_to_source(custody, &mut ()).await;
    }
    S::finish_source(&mut progress);
    let Some(SourceProgress::Completed(custody)) = progress else {
        panic!("unobserved reply settlements finish without another source input");
    };
    custody
}

#[tokio::test]
async fn exact_vector_retains_the_original_rejected_invoice() {
    let invoice = Invoice {
        number: 17,
        charges: Box::new([3, 5]),
    };
    let original_charges = core::ptr::from_ref(invoice.charges.as_ref());
    let recipient = EstablishedRecipient::<Invoices>::issued(InvoiceEndpoint(29));
    let rows = vec![SettledItem::Attempted(ItemSettlement::Rejected {
        item: EstablishedDelivery::new(recipient.clone(), invoice),
        reason: ExactDeliveryReason::ClosedRecipient,
    })];
    let SourceCustody::Retained(mut rows) = settle_replies(rows).await else {
        panic!("exact rejected replies retain custody");
    };
    assert_eq!(rows.len(), 1);
    let SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) =
        rows.pop().expect("one reply")
    else {
        panic!("the complete rejected invoice remains");
    };
    assert_eq!(reason, ExactDeliveryReason::ClosedRecipient);
    assert_eq!(item.to, recipient);
    assert_eq!(item.message.number, 17);
    assert_eq!(*item.message.charges, [3, 5]);
    assert_eq!(
        core::ptr::from_ref(item.message.charges.as_ref()),
        original_charges
    );
}

#[tokio::test]
async fn mixed_logical_replies_retain_the_original_rejected_invoice() {
    let invoice = Invoice {
        number: 19,
        charges: Box::new([7, 11]),
    };
    let original_charges = core::ptr::from_ref(invoice.charges.as_ref());
    let recipient = Recipient::<Invoices>::global(RuntimeAddress(31));
    let rows: ReplySettlements = ReplyDeliveries::new(vec![ReplyDelivery::Logical(
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: Delivery::new(recipient, invoice),
            reason: LogicalDeliveryReason::UnknownAddress,
        }),
    )]);
    let SourceCustody::Retained(rows) = settle_replies(rows).await else {
        panic!("mixed logical rejection must preserve the original invoice");
    };
    let mut rows = rows.into_deliveries();
    assert_eq!(rows.len(), 1);
    let ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Rejected { item, reason })) =
        rows.pop().expect("one reply")
    else {
        panic!("the complete original logical rejection remains");
    };
    assert_eq!(reason, LogicalDeliveryReason::UnknownAddress);
    assert_eq!(item.to, recipient);
    assert_eq!(item.message.number, 19);
    assert_eq!(*item.message.charges, [7, 11]);
    assert_eq!(
        core::ptr::from_ref(item.message.charges.as_ref()),
        original_charges
    );
}

#[tokio::test]
async fn mixed_exact_replies_retain_the_original_rejected_invoice() {
    let invoice = Invoice {
        number: 23,
        charges: Box::new([13, 17]),
    };
    let original_charges = core::ptr::from_ref(invoice.charges.as_ref());
    let recipient = EstablishedRecipient::<Invoices>::issued(InvoiceEndpoint(37));
    let rows: ReplySettlements = ReplyDeliveries::new(vec![ReplyDelivery::Established(
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: EstablishedDelivery::new(recipient.clone(), invoice),
            reason: ExactDeliveryReason::ClosedRecipient,
        }),
    )]);
    let SourceCustody::Retained(rows) = settle_replies(rows).await else {
        panic!("mixed exact rejection must preserve the original invoice");
    };
    let mut rows = rows.into_deliveries();
    assert_eq!(rows.len(), 1);
    let ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Rejected {
        item,
        reason,
    })) = rows.pop().expect("one reply")
    else {
        panic!("the complete original exact rejection remains");
    };
    assert_eq!(reason, ExactDeliveryReason::ClosedRecipient);
    assert_eq!(item.to, recipient);
    assert_eq!(item.message.number, 23);
    assert_eq!(*item.message.charges, [13, 17]);
    assert_eq!(
        core::ptr::from_ref(item.message.charges.as_ref()),
        original_charges
    );
}

#[tokio::test]
async fn accepted_unit_receipts_exhaust_without_retirement_residue() {
    let rows: ReplySettlements = ReplyDeliveries::new(vec![
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Accepted(()))),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Accepted(()))),
    ]);
    let SourceCustody::Exhausted(rows) = settle_replies(rows).await else {
        panic!("accepted unit receipts have no remaining reply custody");
    };
    for row in rows.into_deliveries() {
        match row {
            ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Accepted(())))
            | ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Accepted(()))) => {}
            _ => panic!("exhausted reply custody contains only accepted unit receipts"),
        }
    }
}

#[tokio::test]
async fn interleaved_failures_and_untouched_replies_preserve_order_and_allocations() {
    let invoices: [Invoice; 6] = core::array::from_fn(|position| Invoice {
        number: 41 + position as u64,
        charges: Box::new([53 + position as u8, 61 + position as u8]),
    });
    let original_charges = invoices
        .each_ref()
        .map(|invoice| core::ptr::from_ref(invoice.charges.as_ref()));
    let [first, second, third, fourth, fifth, sixth] = invoices;
    let logical = Recipient::<Invoices>::global(RuntimeAddress(71));
    let exact = EstablishedRecipient::<Invoices>::issued(InvoiceEndpoint(73));
    let rows: ReplySettlements = ReplyDeliveries::new(vec![
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Accepted(()))),
        ReplyDelivery::Logical(SettledItem::Unattempted(Delivery::new(logical, first))),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Corrupt {
            item: EstablishedDelivery::new(exact.clone(), second),
            fault: InterpreterFault::MissingCapability,
        })),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Accepted(()))),
        ReplyDelivery::Established(SettledItem::Unattempted(EstablishedDelivery::new(
            exact.clone(),
            third,
        ))),
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Corrupt {
            item: Delivery::new(logical, fourth),
            fault: InterpreterFault::CorruptTraversal,
        })),
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Accepted(()))),
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Rejected {
            item: Delivery::new(logical, fifth),
            reason: LogicalDeliveryReason::ClosedRecipient,
        })),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Rejected {
            item: EstablishedDelivery::new(exact.clone(), sixth),
            reason: ExactDeliveryReason::ClosedRecipient,
        })),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Accepted(()))),
    ]);
    let SourceCustody::Retained(rows) = settle_replies(rows).await else {
        panic!("all six original obligations survive");
    };
    // A fresh offer must preserve the same residual; completed phase calls must not consume it.
    let SourceCustody::Retained(rows) = settle_replies(rows).await else {
        panic!("offering retained obligations again preserves them");
    };
    let mut progress = Some(SourceProgress::Completed(SourceCustody::Retained(rows)));
    <ReplySettlements as SourceSettlementCustody<(), Never>>::prepare_source(&mut progress);
    <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
    <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
    let Some(SourceProgress::Completed(SourceCustody::Retained(rows))) = progress else {
        panic!("completed custody is absorbing");
    };
    let rows = rows.into_deliveries();
    let [
        ReplyDelivery::Logical(SettledItem::Unattempted(logical_unsent)),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Corrupt {
            item: exact_corrupt,
            fault: InterpreterFault::MissingCapability,
        })),
        ReplyDelivery::Established(SettledItem::Unattempted(exact_unsent)),
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Corrupt {
            item: logical_corrupt,
            fault: InterpreterFault::CorruptTraversal,
        })),
        ReplyDelivery::Logical(SettledItem::Attempted(ItemSettlement::Rejected {
            item: logical_rejected,
            reason: LogicalDeliveryReason::ClosedRecipient,
        })),
        ReplyDelivery::Established(SettledItem::Attempted(ItemSettlement::Rejected {
            item: exact_rejected,
            reason: ExactDeliveryReason::ClosedRecipient,
        })),
    ] = rows.as_slice()
    else {
        panic!("authored interleaving, attempt status, reasons and faults remain exact");
    };
    for delivery in [logical_unsent, logical_corrupt, logical_rejected] {
        assert_eq!(delivery.to, logical);
    }
    for delivery in [exact_corrupt, exact_unsent, exact_rejected] {
        assert_eq!(delivery.to, exact);
    }
    let returned = [
        &logical_unsent.message,
        &exact_corrupt.message,
        &exact_unsent.message,
        &logical_corrupt.message,
        &logical_rejected.message,
        &exact_rejected.message,
    ];
    for ((invoice, original_charges), position) in
        returned.into_iter().zip(original_charges).zip(0..6)
    {
        assert_eq!(invoice.number, 41 + position as u64);
        assert_eq!(*invoice.charges, [53 + position as u8, 61 + position as u8]);
        assert_eq!(
            core::ptr::from_ref(invoice.charges.as_ref()),
            original_charges
        );
    }
}

#[test]
fn direct_phase_completion_retains_original_replies_and_empty_custody_stays_empty() {
    let reports = || {
        ReplyDeliveries::new(vec![ReplyDelivery::Logical(SettledItem::Unattempted(
            Delivery::<Invoices>::new(
                Recipient::global(RuntimeAddress(79)),
                Invoice {
                    number: 83,
                    charges: Box::new([89, 97]),
                },
            ),
        ))])
    };
    let phases: [SourceProgress<ReplySettlements, ReplySettlements>; 2] = [
        SourceProgress::Original(reports()),
        SourceProgress::Offering(reports()),
    ];
    for phase in phases {
        let original_charges = match &phase {
            SourceProgress::Original(rows) | SourceProgress::Offering(rows) => {
                match rows.as_slice() {
                    [ReplyDelivery::Logical(SettledItem::Unattempted(item))] => {
                        core::ptr::from_ref(item.message.charges.as_ref())
                    }
                    _ => panic!("one actual unsent reply"),
                }
            }
            SourceProgress::Completed(_) => panic!("the fixture starts with unfinished custody"),
        };
        let mut progress = Some(phase);
        <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
        <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
        let Some(SourceProgress::Completed(SourceCustody::Retained(rows))) = progress else {
            panic!("both unfinished phases preserve their original input");
        };
        let mut rows = rows.into_deliveries();
        assert_eq!(rows.len(), 1);
        let ReplyDelivery::Logical(SettledItem::Unattempted(item)) =
            rows.pop().expect("one complete reply")
        else {
            panic!("the original reply remains unattempted");
        };
        assert_eq!(item.to.address(), RuntimeAddress(79));
        assert_eq!(item.message.number, 83);
        assert_eq!(*item.message.charges, [89, 97]);
        assert_eq!(
            core::ptr::from_ref(item.message.charges.as_ref()),
            original_charges
        );
    }
    let mut progress = None;
    <ReplySettlements as SourceSettlementCustody<(), Never>>::prepare_source(&mut progress);
    <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
    assert!(progress.is_none());
    let mut progress = Some(SourceProgress::Original(ReplyDeliveries::new(Vec::new())));
    <ReplySettlements as SourceSettlementCustody<(), Never>>::prepare_source(&mut progress);
    <ReplySettlements as SourceSettlementCustody<(), Never>>::finish_source(&mut progress);
    let Some(SourceProgress::Completed(SourceCustody::Exhausted(rows))) = progress else {
        panic!("empty reply custody exhausts");
    };
    let rows = rows.into_deliveries();
    assert!(rows.is_empty());
}
