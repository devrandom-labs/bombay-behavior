//! Slope point — caps: St Sup W. Generated (bombay card #298).
use std::time::Duration;
use behaviorpass::{Actions, Base, Exit, MailAddr, RestartPolicy, StashRoute, Stashing, Strategy, Supervising, Watching, stop_on_abnormal_death, run};
use behaviorpass::{Never};
use fastpass::{Config, channel};

type Kid = Base<MailAddr, u64, u64, Never, &'static str>;

fn base<N>() -> Base<MailAddr, u64, u64, Never, &'static str, Never, N> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Actions<MailAddr, Never, Never, N>, &'static str>(if *s > 1000 { Actions::stop(Exit::Normal) } else { Actions::cont() })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Supervising::new(Watching::new(Stashing::new(base::<Kid>(), |_: &u64| StashRoute::Deliver), stop_on_abnormal_death), |i| i as u64, 1, |_| base::<Never>(), Strategy::OneForOne, RestartPolicy::Transient, 3, Duration::MAX);
    let handle = tokio::spawn(run(stack, rx, MailAddr(0)));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
