//! Slope point — caps: D St W. Generated (bombay card #298).
use behaviorpass::{Actions, Base, Deadlined, Exit, StashRoute, Stashing, Watching, otp_propagation, run};
use bombay::capability::{Never, Step};
use fastpass::{Config, channel};

fn base() -> Base<u64, u64, Never, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Actions<Never, Never, Never>, &'static str>(if *s > 1000 { Actions::stop(Exit::Normal) } else { Actions::cont() })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Watching::new(Deadlined::new(Stashing::new(base(), |_: &u64| StashRoute::Deliver), None, |_| Ok(Step::Continue)), otp_propagation);
    let handle = tokio::spawn(run(stack, rx));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
