struct Missing;

#[behavior_macros::behavior(
    addr = u8,
    message = u8,
)]
impl Missing {
    fn receive(&mut self, _: u8, _: u8) -> () {}
}
