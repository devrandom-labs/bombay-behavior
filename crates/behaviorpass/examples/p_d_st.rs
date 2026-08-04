//! Slope point — caps: D St. Generated (bombay card #298).
use behaviorpass::{Actions, Base, FnState, Deadlined, Exit, MailAddr, StashRoute, Stashing, run};
use behaviorpass::{Never, Step};
use fastpass::{Config, channel};

type Floor = Base<FnState<u64, MailAddr, u64, Never, Never, &'static str>, Never, Never, &'static str>;

fn base() -> Floor {
    Base::from_fn(0, |s: &mut u64, _from: MailAddr, m: u64| {
        *s += m;
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(if *s > 1000 { Actions::stop(Exit::Normal) } else { Actions::cont() })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Deadlined::new(Stashing::new(base(), |_: &u64| StashRoute::Deliver), None, |_| Ok(Step::Continue));
    let handle = tokio::spawn(run(stack, rx, MailAddr(0)));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
