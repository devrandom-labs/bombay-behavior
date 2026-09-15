use core::marker::PhantomData;

use behavior_core::MailAddr;

pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> behavior_core::Protocol for TestRecipient<M> {
    type Addr = MailAddr;
    type Msg = M;
}
