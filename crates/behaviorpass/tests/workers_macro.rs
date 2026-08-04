//! `workers!` — the mixed-fleet desugar (behaviorpass-macros): a `Crew`
//! sum with a delegated `Behavior`, a per-variant range `crew_build`, and
//! the total count, feeding a `Supervising` fleet with zero erasure.

use std::time::Duration;

use behaviorpass::{
    Acted, Actions, Base, Behavior, Create, Envelope, Exit, MailAddr, Never, RestartPolicy, State,
    Step, Strategy, Supervising, workers,
};

struct Alpha {
    n: u64,
}

struct Beta {
    n: u64,
}

impl State for Alpha {
    type Addr = MailAddr;
    type Msg = u64;
    fn handle(&mut self, _from: MailAddr, m: u64) -> Acted<MailAddr, Never, Never, Never, Never> {
        self.n += m;
        // Alpha stops once it has seen 10 in total — the variant marker.
        Ok(if self.n >= 10 { Actions::stop(Exit::Normal) } else { Actions::cont() })
    }
}

impl State for Beta {
    type Addr = MailAddr;
    type Msg = u64;
    fn handle(&mut self, _from: MailAddr, m: u64) -> Acted<MailAddr, Never, Never, Never, Never> {
        self.n += m;
        Ok(Actions::cont())
    }
}

fn alpha(i: usize) -> Base<Alpha> {
    Base::new(Alpha { n: i as u64 })
}

fn beta(_i: usize) -> Base<Beta> {
    Base::new(Beta { n: 0 })
}

#[tokio::test]
async fn crew_build_dispatches_variants_by_contiguous_range() {
    let (total, build) = workers![(2, Base<Alpha>, alpha), (1, Base<Beta>, beta)];
    assert_eq!(total, 3, "the counts sum to the fleet size");

    // Slots 0..2 are Alpha (stops at >= 10); slot 2 is Beta (never stops).
    let mut a1 = build(1);
    let verdict = a1
        .step(Envelope::User { from: MailAddr(0), msg: 9 })
        .await
        .expect("no crash")
        .become_;
    assert!(matches!(verdict, Step::Stop(Exit::Normal)), "slot 1 is Alpha: 1 + 9 hits the stop");

    let mut b = build(2);
    let verdict = b
        .step(Envelope::User { from: MailAddr(0), msg: 9 })
        .await
        .expect("no crash")
        .become_;
    assert!(matches!(verdict, Step::Continue), "slot 2 is Beta: it never stops");
}

#[tokio::test]
async fn crew_rides_a_supervising_fleet_and_restarts_by_nonce() {
    let (total, build) = workers![(2, Base<Alpha>, alpha), (1, Base<Beta>, beta)];
    let inner = Base::from_fn((), |(): &mut (), _from: MailAddr, _: u64| Ok::<Actions<MailAddr, Never, Never, _>, Never>(Actions::cont()));
    let mut sup = Supervising::new(
        inner,
        |i| i as u64,
        total,
        build,
        Strategy::OneForOne,
        RestartPolicy::Transient,
        2,
        Duration::MAX,
    );

    let fleet = Behavior::fleet(&sup).expect("a supervisor declares its fleet");
    assert_eq!(fleet.n, 3, "the fleet reports the macro's total");

    let at = tokio::time::Instant::now();
    let actions = sup
        .step(Envelope::ChildStopped { nonce: 1, outcome: Err(behaviorpass::Crash::Failed), at })
        .await
        .expect("no crash");
    let [Create::Restart { nonce, .. }] = actions.creates.as_slice() else {
        panic!("an abnormal stop within budget emits exactly one restart");
    };
    assert_eq!(*nonce, 1, "the restart names the dead crew member's nonce");
}
