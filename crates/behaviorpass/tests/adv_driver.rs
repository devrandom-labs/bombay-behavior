//! Driver (`run`) invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`. Pins the recording-peer contract: Transcript sends and
//! creates are the exact emission-order accumulation, `Stop(exit)` ends the
//! fold immediately (nothing after it folds), a fully-closed mailbox is
//! `Collected`, an `Err` short-circuits with its exact value.
//! Methods: handcrafted sequences/lifecycle + a differential property model +
//! long-sequence fuzz through a real mailbox.

use behaviorpass::{Actions, Base, Create, Exit, MailAddr, Target, run};
use behaviorpass::{Never, Step};
use fastpass::{Config, channel};

/// A behavior that records what it folds and sends one message per fold; stops
/// on a designated id; crashes on another.
fn driver_base() -> Base<MailAddr, Vec<u64>, u64, Never, &'static str, u64, Never> {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        let become_ = if id == 99 {
            Step::Stop(Exit::Normal)
        } else if id == 7 {
            return Err("boom");
        } else {
            Step::Continue
        };
        Ok::<Actions<MailAddr, Never, u64, Never>, &'static str>(Actions {
            sends: vec![(Target::Global(MailAddr(id)), id)],
            creates: Vec::new(),
            become_,
        })
    })
}

/// Transcript.sends is the exact emission-order accumulation across folds.
#[tokio::test]
async fn driver_transcript_preserves_emission_order() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(driver_base(), rx, MailAddr(0)));

    usr.send(1).await.expect("mailbox open");
    usr.send(2).await.expect("mailbox open");
    usr.send(3).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.sends, vec![(Target::Global(MailAddr(1)), 1), (Target::Global(MailAddr(2)), 2), (Target::Global(MailAddr(3)), 3)]);
    assert_eq!(transcript.exit, Exit::Collected, "a fully-closed mailbox is collection");
}

/// Creates accumulate in order too (a create-emitting behavior).
#[tokio::test]
async fn driver_accumulates_creates_in_order() {
    let creator: Base<MailAddr, Vec<u64>, u64, Never, &'static str, Never, u32> =
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<MailAddr, Never, Never, u32>, &'static str>(Actions {
                sends: Vec::new(),
                creates: vec![Create::Birth { nonce: id, child: u32::try_from(id).expect("test message ids fit u32") }],
                become_: Step::Continue,
            })
        });
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(creator, rx, MailAddr(0)));
    usr.send(4).await.expect("mailbox open");
    usr.send(9).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.creates, vec![Create::Birth { nonce: 4, child: 4 }, Create::Birth { nonce: 9, child: 9 }]);
    assert_eq!(transcript.exit, Exit::Collected);
}


/// One transcript carries BOTH traces: sends and creates accumulate in their
/// own emission orders side by side, and the Stop's exit ends the fold.
#[tokio::test]
async fn driver_combines_sends_and_creates_in_one_transcript() {
    let both: Base<MailAddr, Vec<u64>, u64, Never, &'static str, u64, u32> =
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<MailAddr, Never, u64, u32>, &'static str>(Actions {
                sends: vec![(Target::Global(MailAddr(id)), id)],
                creates: vec![Create::Birth { nonce: id, child: u32::try_from(id).expect("test message ids fit u32") }],
                become_: if id == 99 { Step::Stop(Exit::Normal) } else { Step::Continue },
            })
        });
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(both, rx, MailAddr(0)));

    usr.send(4).await.expect("mailbox open");
    usr.send(99).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.sends, vec![(Target::Global(MailAddr(4)), 4), (Target::Global(MailAddr(99)), 99)]);
    assert_eq!(transcript.creates, vec![Create::Birth { nonce: 4, child: 4 }, Create::Birth { nonce: 99, child: 99 }]);
    assert_eq!(transcript.exit, Exit::Normal);
}

/// Nothing folds after a Stop: the messages queued behind the stop id are
/// never folded, never recorded.
#[tokio::test]
async fn driver_nothing_folds_after_stop() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(driver_base(), rx, MailAddr(0)));

    usr.send(1).await.expect("mailbox open");
    usr.send(99).await.expect("mailbox open"); // Stop(Normal)
    usr.send(5).await.expect("mailbox open"); // behind the stop — must never fold
    usr.send(6).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.sends, vec![(Target::Global(MailAddr(1)), 1), (Target::Global(MailAddr(99)), 99)], "only the pre-Stop folds recorded");
    assert_eq!(transcript.exit, Exit::Normal, "the Stop's exit rides out");
}

/// An Err short-circuits with its exact value (and the driver task returns it).
#[tokio::test]
async fn driver_err_short_circuits_with_exact_error() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(driver_base(), rx, MailAddr(0)));

    usr.send(1).await.expect("mailbox open");
    usr.send(7).await.expect("mailbox open"); // boom
    usr.send(8).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let out = handle.await.expect("driver joins");
    assert_eq!(out.err().expect("expected a crash"), "boom", "the crash surfaces with its exact error");
}

/// Collection with zero messages: an empty, fully-closed mailbox is Collected
/// with an empty transcript.
#[tokio::test]
async fn driver_collected_with_zero_messages() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let handle = tokio::spawn(run(driver_base(), rx, MailAddr(0)));
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.sends, Vec::<(Target<MailAddr>, u64)>::new());
    assert_eq!(transcript.exit, Exit::Collected);
}

// ---------------------------------------------------------------------------
// Differential property model + fuzz
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Op {
    Push(u64),
    StopOn(u64),
    Boom(u64),
}

fn op_strategy() -> impl proptest::strategy::Strategy<Value = Op> {
    use proptest::prelude::*;
    prop_oneof![
        Just(Op::Push(0)),
        Just(Op::Push(1)),
        Just(Op::Push(u64::MAX)),
        any::<u64>().prop_map(Op::Push),
        Just(Op::StopOn(99)),
        Just(Op::Boom(7)),
    ]
}

/// The independent model: the exact fold the driver must reproduce — sends in
/// emission order, stop on the first `StopOn`, crash on the first Boom.
struct DriverModel {
    sends: Vec<(u64, u64)>,
    outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Collected,
    Stopped,
    Crashed,
}

impl DriverModel {
    fn new() -> Self {
        Self { sends: Vec::new(), outcome: Outcome::Collected }
    }

    /// Fold one op; the outcome is final once set.
    fn fold(&mut self, op: Op) {
        if self.outcome != Outcome::Collected {
            return;
        }
        match op {
            Op::Push(n) => self.sends.push((n, n)),
            Op::StopOn(n) => {
                self.sends.push((n, n));
                self.outcome = Outcome::Stopped;
            }
            Op::Boom(_) => self.outcome = Outcome::Crashed,
        }
    }
}

fn fold_driver_and_check(rt: &tokio::runtime::Runtime, ops: &[Op]) {
    rt.block_on(async {
        let mut model = DriverModel::new();
        let (ctl, usr, rx) = channel::<Never, Op>(Config::new(16));
        let handle = tokio::spawn(run(
            Base::new(Vec::<Op>::new(), |_seen: &mut Vec<Op>, op: Op| {
                if let Op::Boom(_) = op {
                    return Err("boom");
                }
                let become_ = if let Op::StopOn(_) = op {
                    Step::Stop(Exit::Normal)
                } else {
                    Step::Continue
                };
                Ok::<Actions<MailAddr, Never, u64, Never>, &'static str>(Actions {
                    sends: vec![(Target::Global(MailAddr(id_of(op))), id_of(op))],
                    creates: Vec::new(),
                    become_,
                })
            }),
            rx,
            MailAddr(0),
        ));

        for op in ops {
            if model.outcome != Outcome::Collected {
                break; // nothing folds after the model stopped
            }
            usr.send(*op).await.expect("mailbox open");
            model.fold(*op);
        }
        drop(usr);
        drop(ctl);

        let out = handle.await.expect("driver joins");
        match model.outcome {
            Outcome::Collected => {
                let transcript = out.expect("no crash");
                assert_eq!(transcript.sends, model.sends.iter().map(|&(a, b)| (Target::Global(MailAddr(a)), b)).collect::<Vec<_>>());
                assert_eq!(transcript.exit, Exit::Collected);
            }
            Outcome::Stopped => {
                let transcript = out.expect("no crash");
                assert_eq!(transcript.sends, model.sends.iter().map(|&(a, b)| (Target::Global(MailAddr(a)), b)).collect::<Vec<_>>());
                assert_eq!(transcript.exit, Exit::Normal, "the Stop's exit rides out");
            }
            Outcome::Crashed => {
                assert_eq!(out.err().expect("expected a crash"), "boom", "the crash surfaces exactly");
            }
        }
    });
}

fn id_of(op: Op) -> u64 {
    match op {
        Op::Push(n) | Op::StopOn(n) | Op::Boom(n) => n,
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Any op script (empty, max-length, boundary ids) drives the mailbox to
    /// exactly the model's outcome, with the model's exact send trace.
    #[test]
    fn prop_driver_matches_differential_model(ops in proptest::collection::vec(op_strategy(), 0..=16)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_driver_and_check(&rt, &ops);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..proptest::prelude::ProptestConfig::default() })]

    /// Long interleavings: the mailbox never reorders, never folds post-stop,
    /// and the exit is exactly the model's.
    #[test]
    fn prop_driver_long_sequences_match_model(ops in proptest::collection::vec(op_strategy(), 0..=64)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_driver_and_check(&rt, &ops);
    }
}
