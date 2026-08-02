//! Slope point — caps: Ph Sup W. Generated (bombay card #298).
use behaviorpass::{Base, Exit, Phased, Supervising, Watching, otp_propagation, run};
use bombay::capability::{Disposition, Never, Step};
use fastpass::{Config, channel};


fn base() -> Base<u64, u64, Never, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Step<Never, Exit>, &'static str>(if *s > 1000 { Step::Stop(Exit::Normal) } else { Step::Continue })
    })
}

fn base_phase() -> Base<u64, u64, bool, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Step<bool, Exit>, &'static str>(if *s > 1000 { Step::Goto(true) } else { Step::Continue })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Supervising::new(Watching::new(Phased::new(base_phase(), false, |_, _| Disposition::Deliver), otp_propagation), vec![base()], |_| base(), 3);
    let handle = tokio::spawn(run(stack, rx));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
