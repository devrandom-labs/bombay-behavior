use std::hint::black_box;
use std::time::{Duration, Instant};

use behaviorpass::{
    Acted, Actions, Base, Behavior, Delivery, MailAddr, Never, Proxy, ProxyCommand, State, User,
    UserEvent,
};
use tokio::runtime::Builder;

const ITERATIONS: usize = 250_000;

struct Sink(u64);

impl State for Sink {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Never, Never> {
        self.0 = self.0.wrapping_add(message);
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Base<Sink> {
    Base::new(Sink(0))
}

fn main() {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let base_rate = runtime.block_on(measure_base());
    let proxy_rate = runtime.block_on(measure_proxy());
    let score = base_rate.min(proxy_rate);

    println!("METRIC score={score:.0}");
    println!("info base_transitions_per_s={base_rate:.0}");
    println!("info proxy_transitions_per_s={proxy_rate:.0}");
}

async fn measure_base() -> f64 {
    let mut behavior = Base::new(Sink(0));
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let message = u64::try_from(index).unwrap();
        black_box(
            behavior
                .step(User::user(MailAddr(0), black_box(message)))
                .await
                .unwrap(),
        );
    }
    rate(started.elapsed())
}

async fn measure_proxy() -> f64 {
    let mut proxy = Proxy::new(child(0));
    proxy.init().await.unwrap();
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let command = if index % 64 == 0 {
            ProxyCommand::Replace(child(index))
        } else {
            ProxyCommand::Forward(u64::try_from(index).unwrap())
        };
        black_box(
            proxy
                .step(User::user(MailAddr(0), black_box(command)))
                .await
                .unwrap(),
        );
    }
    rate(started.elapsed())
}

fn rate(elapsed: Duration) -> f64 {
    f64::from(u32::try_from(ITERATIONS).unwrap()) / elapsed.as_secs_f64()
}
