# Supervisor diagnostics and preparation overlap

Status: diagnostic ingress retained on `codex/arc010-owner-contract`; preparation
overlap remains an unretained experiment. The acceptance target is Bombay
ARC-010's observable supervisor and pool trace, not a prescribed method name.

## Law and failing trace

The actor-model law is that an actor handles an admitted communication as one
transition and communicates through typed messages. Agha et al., *A foundation
for actor computation*, sections 2–3, do not prescribe supervisor diagnostics,
graceful shutdown, or source scheduling. Bombay's single actor turn and explicit
`Actions` interpretation are derived contracts. Its choice to fold shutdown
while an external replacement source remains in progress, reject a subsequent
job as `ShuttingDown`, account for the eventual source result without starting
a replacement, and retire the actor tree is a deliberate policy.

The focused live regression is Bombay's
`docs/research-probes/fifo-shutdown-during-preparation.patch`. With the 0.19
contract, its held source prevents the Driver from folding shutdown; the
subsequent job reply times out before source release. The fixed supervisor
additionally lacked `ChildReport<ProxyDiagnostic<Worker, Plan>>` ingress.
Bombay's `docs/research-probes/behavior-actors-proxy-diagnostic-gap.md` records
the original compile failure and the isolated owner proof.

## Retained diagnostic batch

`156ac58` adds typed proxy-diagnostic ingress to fixed and dynamic supervisors.
Fixed supervision retains the complete report in its existing
`FixedDiagnostic::UnexpectedInput`; dynamic supervision carries it in its
existing diagnostic lane. A proxy report does not change roster state or become
a successful proxy outcome. The child occurrence, phase, reason, and owned
unexpected input survive. Existing diagnostic dispositions decide delivery or
terminal custody. The three focused `proxy_diagnostic` tests pass in this
branch. The isolated owner checkout previously passed its complete workspace
and Bombay fixed-supervisor shutdown probe in debug and release.

Aggregate checkpoint: fixed and dynamic supervisor control-state sums and
selection policies are unchanged. One dynamic diagnostic alternative and one
event alternative in each supervisor were added; each owns the complete
report needed for diagnostic transfer. No arrival-history state, repeated
cause, cardinality assumption, semantic boolean, nested transition authority,
or structural caller syntax was added. Eight files changed, including three
test files; production `+110/-0/net +110`, no new public type. Cross-checks:
`docs/actor-laws/proxy.md`, `docs/actor-laws/fixed-supervisor.md`,
`docs/actor-laws/dynamic-supervisor.md`, and Bombay's ARC-010 probe.
Disposition: `pass` for diagnostic ingress.

## Preparation overlap checkpoint

The separate `codex/late-worker-preparation` worktree contains a test-first,
unretained candidate. Its focused FIFO and fixed-supervisor callers fail
against untouched 0.19 source in debug and release. The candidate distinguishes
source-action admission from the later complete source result, so the actor
can fold shutdown between them. The candidate is not selected by this ledger;
`docs/engineering/worker-preparation-interleaving.md` in that worktree records
its ownership equation, alternatives, falsifiers, and missing Bombay active
task-failure proof.

Before the candidate, FIFO and keyed pools each have five aggregate states;
fixed supervision also has five. The candidate adds no aggregate state. Its
issued/started expectation is one subordinate two-alternative sum needed to
reject early, foreign, or duplicate results. The candidate has two proposed
public products with distinct timing, plus the existing complete preparation
result. Every surviving alternative must retain the exact source, ticket,
ordered roles, and any prepared prefix that a later decision needs; a source
rejection after start is a late result. The residue scan finds no proposed
arrival-history flag, repeated cause, false cardinality, nested actor engine,
semantic boolean, or positional wrapper path. The existing source product,
`ItemSettlement`, FIFO/keyed/fixed event and effect products, both wrapper
orders, and Bombay's typed source interpreter must be checked together.
Cross-checks: `docs/atomic-runtime-settlement.md`, the FIFO, keyed, and fixed
actor-law documents, and Bombay ARC-010. Disposition: `reopen` until the
candidate compiles, passes pure transitions and the live timing witness, and
accounts for active task failure and retirement. It is not part of this branch.

The separate preparation experiment currently changes eleven paths,
production `+664/-176/net +488`, tests `+88/-11`, and two proposed public
types. Combining that current experiment with the retained diagnostic batch
would already exceed 15 changed files and 500 net new production lines; the
unmigrated keyed and fixed consumers require further files. Under `AGENTS.md`'s
change-containment rule, further production edits require explicit
authorization for the expanded cumulative surface. This measurement is a
checkpoint, not permission to retain the candidate or its method spelling.
