//! Slope point — caps: Ph W. Generated (bombay card #298).
use behaviorpass::{Admit, Base, Exit, Phased, Watching, otp_propagation, run};
use bombay::capability::{Never, Step};
use fastpass::{Config, channel};


fn base_phase() -> Base<u64, u64, bool, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Step<bool, Exit>, &'static str>(if *s > 1000 { Step::Goto(true) } else { Step::Continue })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Watching::new(Phased::new(base_phase(), false, |_, _| Admit::Deliver), otp_propagation);
    let handle = tokio::spawn(run(stack, rx));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
