# Dead ends

Rejected and reverted approaches, exact evidence, and conditions that would
justify reopening them.

## Rejected findings (near-misses)

### Uniform primitive conversions for semantic newtypes (E05)
- Evidence: twelve tuple newtypes gained `From<u64>`/`Into<u64>` and
  `TokenCount` gained conversions solely to match a convention; the experiment
  recorded that no call site used them.
- Rejected because: `AGENTS.md` prohibits convenience constructors without
  demonstrated semantic value. Uniform syntax is not semantic reuse, and the
  extra impls enlarge the public obligation without making an illegal state
  unrepresentable.
- Resolution: independently audited and reverted after the loop reached its
  claimed fixed point. Existing conversions required by concrete generic
  creation protocols (`TimerId`, `TimerGeneration`, address nonces) remain.
- Reopen if: a concrete composition requires generic primitive conversion and
  the bound itself expresses a documented semantic law.

### `ReadinessReport.ready: bool` — remove as "duplicated authority"
- Evidence: `ready` is a pure projection of `dependencies` (`all(Observed{Ready})`),
  so it is representable-only-consistent if a future constructor forgets to set it.
- Rejected because: (1) `report()` is the sole constructor and maintains the
  invariant by construction; (2) `ready` has demonstrated consumers (readiness
  tests assert `message.ready`); (3) removal is a public API break with no
  semantic gain — it is a report DTO convenience, not a lifecycle flag.
- Reopen if: a second constructor appears, or the field is observed drifting.

### `Duration::MAX` as "unbounded window" sentinel
- Evidence: `RestartBudget::prune` special-cases `window == Duration::MAX`.
- Rejected because: it is a pure optimization — `age <= Duration::MAX` is always
  true, so the early return is equivalent to the retain; `Duration::MAX` is a
  legitimately-large value, not a "none/unknown" sentinel. No correctness hazard.

### Remove zero-consumer type aliases (`PoolEvent`, `OneShotEvent`, `PeriodicEvent`, `ChildrenResult`)
- Evidence: 0 external references to the alias names.
- Rejected because: they are truthful aliases for verbose event-sum types
  (`SupervisionEvent<User<A, PoolMessage<...>>>`, `TimedEvent<E>`); AGENTS.md
  permits truthful aliases over verbose types. Their targets are also
  re-exported, so removal would only shrink the surface, not reduce redundancy.

### `RoutingStrategy` trait — "0 consumers"
- Evidence: scout flagged it as unreferenced.
- Rejected (false positive): it is the trait bound on `RouterMessage`/`Router`
  (`R: RoutingStrategy<D>`) and is implemented by all five strategies in the
  same module. It is the public custom-strategy extension point.

## Conditions that would reopen any of these
- A second constructor/consumer of the affected type appears.
- A demonstrated drift or invalid state in the wild.
- Operator policy prefers surface-minimization over DTO convenience.

## Rejected findings (convergence pass)

### order_gate `open()` watermark clone + intermediate `Vec<K>` (E11a)
- Evidence: `range(..=through.clone())` clones `through`; an intermediate `Vec<K>`
  materializes the in-range keys to split the borrow from `remove`.
- Rejected because: the "clean" fix `range(..=&through)` does not typecheck for
  generic `K` (requires `K: Borrow<&K>`); the correct form `range((Unbounded,
  Included(&through)))` is more verbose than the clone it saves. The
  intermediate `Vec` also preserves `Vec::with_capacity(keys.len())` for
  `deliveries`. `open()` is not a hot path and `K` is typically a cheap key.
- Reopen if: profiling shows `open()` allocation dominates, or `K` clones become
  measurably expensive.
