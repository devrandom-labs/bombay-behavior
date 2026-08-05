# behaviorpass

`behaviorpass` is a pure actor algebra:

```text
receive(event) -> sends* x creates* x become
```

`Behavior` supplies associated `Event`, `Effect`, `Done`, and `Error`
protocols. Sends remain statically typed. Independent protocols compose as
`SendProduct` values; there is no `dyn`, `Any`, boxed message, global envelope,
timer query, fleet query, or runtime side channel.

The creation leg has a pure type-level mode. `NoBirths` means its `creates`
vector contains the uninhabited `Never`; `Births<C>` means it contains typed
`Create<Addr, C>` values. Ordinary states default to `NoBirths`, while any
Agha-style creator may explicitly select `Births<C>`. The marker has no runtime
methods or machinery.

Timing and observation are ordinary actor protocols. `At` sends `ScheduleAt`
to a clock service and receives `TimeReached`; `Watching` sends `ObservePeer`
to a monitor service and receives `PeerStopped`. The interpreter selects the
service from the statically known message batch type.

Supervision is derived with stable proxy actors. A replacement is a message to
the proxy, and the proxy creates a fresh worker incarnation. `Create` therefore
means only fresh birth. `Supervising<B, C>` declares `Births<Proxy<C>>`, and
`Proxy<C>` declares `Births<C>`.

`Spec` is a DX-only typestate composer. Calls such as `.at(...)`, `.watch(...)`,
`.stash(...)`, `.children(...)`, `.restart(...)`, `.when(...)`, and
`.within(...)` directly build concrete behavior wrappers while hiding their
nested protocol types.
