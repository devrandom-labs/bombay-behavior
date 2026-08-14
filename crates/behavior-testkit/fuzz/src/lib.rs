use core::marker::PhantomData;

use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};

pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> Behavior for TestRecipient<M> {
    type Addr = MailAddr;
    type Msg = M;
    type Event = User<MailAddr, M>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}
