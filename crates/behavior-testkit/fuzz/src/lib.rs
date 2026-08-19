use core::marker::PhantomData;

use behavior::MailAddr;

pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> behavior::Protocol for TestRecipient<M> {
    type Addr = MailAddr;
    type Msg = M;
}
