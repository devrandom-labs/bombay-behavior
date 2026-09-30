use core_behavior::{
    BirthProtocol, LogicalDeliveryProtocols, MailAddr, MessageProtocol, NoBirthProtocols,
};
use direct_and_facade::{ExportedSends, ExportedSettlements};

fn main() {
    type Actual = <ExportedSends as LogicalDeliveryProtocols>::Protocols;
    type Expected = BirthProtocol<MessageProtocol<MailAddr, u8>, NoBirthProtocols>;
    let _: core::marker::PhantomData<Expected> = core::marker::PhantomData::<Actual>;
    let _: core::marker::PhantomData<ExportedSettlements> = core::marker::PhantomData;
}
