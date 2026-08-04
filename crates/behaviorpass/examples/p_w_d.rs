//! Slope point — caps: D W. Generated (bombay card #298).
use behaviorpass::{Actions, Base, Deadlined, Exit, MailAddr, Watching, stop_on_abnormal_death, run};
use behaviorpass::{Never, Step};
use fastpass::{Config, channel};

fn base() -> Base<MailAddr, u64, u64, Never, &'static str> {
    Base::new(0, |s: &mut u64, m: u64| {
        *s += m;
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(if *s > 1000 { Actions::stop(Exit::Normal) } else { Actions::cont() })
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = Watching::new(Deadlined::new(base(), None, |_| Ok(Step::Continue)), stop_on_abnormal_death);
    let handle = tokio::spawn(run(stack, rx, MailAddr(0)));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}
