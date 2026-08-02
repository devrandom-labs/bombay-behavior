//! Slope point — caps: St W. Generated (bombay card #298).
use behaviorpass::{Base, Exit, StashRoute, Stashing, Watching, otp_propagation, run};
use bombay::capability::{Never, Step};
use fastpass::{Config, channel};

fn base() -> Base<u64, u64, Never, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Step<Never, Exit>, &'static str>(if *s > 1000 { Step::Stop(Exit::Normal) } else { Step::Continue })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Watching::new(Stashing::new(base(), |_: &u64| StashRoute::Deliver), otp_propagation);
    let handle = tokio::spawn(run(stack, rx));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
