//! Probe: eliminate the disjoint-lane `SendProduct` encoding (P-products) by
//! collapsing independent send lanes into a single merged vector.
//!
//! Expected outcome: the merged encoding COMPILES but admits out-of-lane sends
//! — a static-safety loss that the disjoint product prevents.

use behavior::{Actions, Delivery, MailAddr, Recipient, SendAlgebra, SendProduct};

// ---------- Disjoint product (current Bombay encoding) ----------
// Two independent lanes: lane0 carries u8, lane1 carries &str.
type DisjointSends = SendProduct<Vec<Delivery<MailAddr, u8>>, Vec<Delivery<MailAddr, &'static str>>>;

fn disjoint_lane_sends_are_typed() {
    let mut sends: DisjointSends = SendAlgebra::empty();

    // Lane 0 gets only u8 deliveries:
    sends.inner.push(Delivery::new(Recipient::global(MailAddr(0)), 42u8));

    // Lane 1 gets only &str deliveries:
    sends.own.push(Delivery::new(Recipient::global(MailAddr(0)), "hello"));

    // Crucially, this does NOT compile — a u8 cannot land in the &str lane:
    // sends.own.push(Delivery::new(Recipient::global(MailAddr(0)), 42u8)); // compile error

    // And an empty-lane Actions with disjoint sends is well-typed:
    let _acts: Actions<MailAddr, behavior::Never, DisjointSends, behavior::NoBirths> =
        Actions::cont();
    drop(sends);
}

// ---------- Merged-lane encoding ----------
// One vector where the message type is a sum of all lane messages.
#[derive(Debug)]
enum MergedMsg {
    Lane0(u8),
    Lane1(&'static str),
}

type MergedSends = Vec<Delivery<MailAddr, MergedMsg>>;

fn merged_lane_sends_compile_but_hole() {
    let mut sends: MergedSends = Vec::new();

    // Both message kinds go into the same vector:
    sends.push(Delivery::new(Recipient::global(MailAddr(0)), MergedMsg::Lane0(42u8)));
    sends.push(Delivery::new(Recipient::global(MailAddr(0)), MergedMsg::Lane1("hello")));

    // THE HOLE: with merged lanes, out-of-lane sends are representable.
    // Nothing prevents us from adding Lane0 messages after Lane1 messages
    // in a context where the protocol dictates Lane1-only. The type checker
    // cannot distinguish — MergedMsg covers both.
    sends.push(Delivery::new(Recipient::global(MailAddr(0)), MergedMsg::Lane0(99u8)));
    // This compiles clean, but in the disjoint product this would be a
    // compile error if the context expects only Lane1 sends.

    // With merged sends, an Actions value compiles:
    let _acts: Actions<MailAddr, behavior::Never, MergedSends, behavior::NoBirths> =
        Actions::cont();
    let _ = sends;
}

fn main() {
    disjoint_lane_sends_are_typed();
    merged_lane_sends_compile_but_hole();
    println!(
        "merged-lane probe: disjoint product prevents out-of-lane sends; \
         merged vector compiles but admits the hole"
    );
}
