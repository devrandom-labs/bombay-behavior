# Concrete control probe

This isolated probe links Behavior's installed actor port to Bombay
Communication 0.1.2's typed `ControlSender<B::Event>`. It uses no Bombay
production changes. It proves one generic interpreter can send exact
shutdown events for two behaviors sharing a protocol, retain the rejected
event from a closed old control, and reach a new incarnation at the reused
logical endpoint without retargeting the old handle.

Run from this directory:

```sh
cargo test
cargo test --release
```

The downstream Bombay application installer still needs to adopt the new
Behavior contract. This probe establishes the concrete control-transfer
feasibility; it does not claim Bombay's existing `ActorRef` or application
installation path already implements it.
