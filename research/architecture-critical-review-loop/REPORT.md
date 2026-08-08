# Actor Behavior Algebra Research Report

This report is produced by the autoresearch loop from primary actor-system
research and evidence gathered from the current algebra and tests. It is not a
design proposal and does not inherit authority from retired architecture notes.

The loop must keep full primary-source citations (preferably DOI or stable
publisher/repository links), law/derivation/policy labels, failed derivations,
validation evidence, and the final obligation index here. Its research floor is
Hewitt, Agha, Agha and collaborators' later L-reActor/algebraic actor work, and
formal lambda-calculus foundations of actors—not generic reactor/event-loop
literature.

Every capability entry must begin from behaviorpass's functional combinatorics:
pure folds, typed sums, typed products, higher-order transformations, and their
composition laws. Named framework features are observations to explain, not an
API catalogue to copy.

## Actor behavior algebra evidence

Campaign: behavior-architecture-critical-review, run 2026-08-08. Outcome: an
evidence-backed verification pass with **zero production changes** — every
ratchet finished exactly at baseline, no primitive was added, no experiment
needed reverting. All 61 obligations and all 53 capability rows are resolved
in `evidence.json` and `capability-matrix.json`; both authoritative gates pass.

### Method

1. Primary-source survey (RESEARCH-AGHA, SURVEY-SEARCH) verified the semantic
   floor: Hewitt-Bishop-Steiger IJCAI'73; Hewitt AIJ 1977
   (doi:10.1016/0004-3702(77)90033-9); Greif MIT MAC-TR-146 1975; Clinger MIT
   AI-TR-633 1981; Agha MIT Press 1986; Agha CACM 33(9) 1990
   (doi:10.1145/83880.84585); Agha-Mason-Smith-Talcott JFP 7(1) 1997
   (doi:10.1017/S095679689700261X); Talcott 1996; Agha-Thati LNCS 2941
   (doi:10.1007/978-3-540-39993-3_4); Agha-Thati-Ziaei OSL report;
   Agha-Callsen ActorSpaces 1993; Varela-Agha SALSA OOPSLA 2001
   (doi:10.1145/583960.583964); Armstrong KTH 2003; Erlang book 1996; Rebeca
   FI 2004; Honda CONCUR'93/ESOP'98 for the session-type comparison. Generic
   reactor/event-loop literature was excluded from authority.
2. Every retained claim was labeled (RESEARCH-LABELS): LAW (one communication
   at a time; send/create/become effect triple; fresh allocation; acquaintance
   addressing; become distinct from allocation; per-actor serialization),
   DERIVED (creator-local nonce routing; typed event sums + extraction traits;
   SendAlgebra monoid/SendProduct; BirthMode compile-time capability;
   proxy-derived stable identity; CreationKind provenance; timer generations;
   stash replay; Fsm phases), POLICY (creation-before-dependent-send
   interpretation ordering; shutdown as a typed lane; supervision vocabulary;
   Crash classification).
3. Creation semantics (RESEARCH-CREATION): Agha's fresh allocation vs
   behavior replacement distinction confirmed in both independent semantics
   (1986 task-based, 1997 lambda-based); Bombay's nonce is a derived routing
   key, ordering is policy, replacement-at-address is absent by design.
4. Cluster audits (CORE-*, MODULE-*, COMPOSE-*, CREATE-*, RETAIN-*,
   BOUNDARY-*, REJECT-*, SURFACE-*, GEN-*, PHANTOM-01, COMPLEXITY-01,
   PANIC-01, STATIC-01, ERGO-01, TEST-*) each ran focused nextest filters and
   grep inventories; no claim rests on reading alone.
5. Capability closure (SURVEY-TAXONOMY, SURVEY-BASIS, SURVEY-GAPS,
   SURVEY-ACTORPASS): all 53 rows derived from a seven-construct basis
   (fold; typed sums + extraction traits; typed products + SendProduct;
   Never; BirthMode; function-pointer reactions; higher-order wrappers).
   Dispositions: 29 existing, 14 derived, 8 interpreter, 2 application,
   0 new-primitive.

### Obligation index

Research/survey (labels LAW/DERIVED/POLICY per entry):

- RESEARCH-AGHA — semantic floor verified against 14 primary sources;
  reactor/event-loop work excluded from authority.
- RESEARCH-CREATION — fresh allocation vs become verified in Agha 1986 and
  AMST 1997; nonce/ordering/provenance classified derived/policy.
- RESEARCH-LABELS — every retained claim carries a law/derivation/policy
  label; no claim cites framework folklore.
- SURVEY-SEARCH — five repeatable queries, inclusion/exclusion lists, and the
  one uncertainty (exact 'L-reActor' venue) recorded.
- SURVEY-TAXONOMY — 53/53 required rows resolved; candidate additions
  (ask, circuit breaker, virtual actors, pub-sub, CRDTs) fold into existing
  rows; no new category required.
- SURVEY-BASIS — seven-construct basis generates the pure catalogue; boundary
  rows deliberately not generated.
- SURVEY-GAPS — derivation-first discipline held; boundary obstructions
  recorded with exact algebraic reasons; zero new primitives.
- SURVEY-ACTORPASS — actorpass mapped as one interpreter (install-before-
  sends, per-lane order, clock/timer minting, observation publication, Crash
  minting, shutdown ingress); it validated but did not define the algebra.

Core fold:

- CORE-BEHAVIOR — `step` folds one typed Event into `Result<Actions, E>`;
  purity and no hidden effects verified (behavior.rs:419-431).
- CORE-ACTIONS — `Actions` is complete: sends/creates/become_ only; ordering
  contract documented at behavior.rs:310-318 as interpreter policy.
- CORE-INITIALIZATION — init precedes mailbox events; all wrappers init inner
  first then append own effects (init_contract.rs, 6/6).
- CORE-ERRORS — `Acted` Result, typed `Error`, exhaustive `Exit<A>`,
  `Step<Ph, Exit>`, uninhabited `Never`; illegal states unrepresentable.
- CORE-SENDS — Delivery/Recipient/Route/SendProduct/ServiceSends; monoid
  laws proven by proptest `send_algebra_is_a_monoid`.

Modules:

- MODULE-CORE — behavior.rs/verdict.rs/exit.rs own only the core algebra
  (grep: zero Behavior impls in verdict/exit/protocol).
- MODULE-PROTOCOL — protocol.rs owns lane values + extraction traits, no fold.
- MODULE-TRANSFORMS — each wrapper owns its concrete event sum exactly once.

Composition (no lane dropped, duplicated, reordered, relabeled, or
reinterpreted in any supported order):

- COMPOSE-AT — one-shot absolute deadline; (id, generation) consumed once;
  init inner-first then own ScheduleAt.
- COMPOSE-RECEIVE-TIMEOUT — inactivity timer; only successful continuing user
  folds rearm; stale generations dropped; GenerationExhausted is typed.
- COMPOSE-WATCHING — ObservePeer/PeerStopped lanes; LinkReaction per death;
  both wrapper orders preserve all lanes (watching.rs:25-66).
- COMPOSE-SUPERVISING — strategy/policy/budget decision pure
  (supervising.rs:492-558); proxy identity stable across restarts;
  provenance preserved through wrap.
- COMPOSE-SHUTDOWN — typed lane; FinalizeOnShutdown retains final fold's
  complete sends+creates, overrides only the verdict.
- COMPOSE-STASHING — single FIFO buffer; exact-filter proven by 256-case
  proptest + 121-sequence exhaustive enumeration; Ph=Never static constraint.
- COMPOSE-FSM — derived from receive+become only; drain on real phase change;
  no-drop/no-dup proven black-box (512-case proptest, 341 sequences).
- COMPOSE-SPEC — pure typestate facade; holds only `behavior: B` and
  `next_timer: u64`; no runtime intent graph.
- COMPOSE-MIXED — full-stack cross-lane proptests (cross_lane.rs,
  two_buffer.rs) prove lane isolation under randomized interleavings.

Creation:

- CREATE-FRESHNESS — Create is staged data; freshness/installation are
  interpreter-owned (behavior.rs:250-253).
- CREATE-NONCE — nonce is routing/correlation only; duplicate nonces create
  ambiguous routes, not identity conflicts (boundaries.rs:214).
- CREATE-ORDER — creation-before-dependent-send is documented Bombay policy
  (behavior.rs:309-317).
- CREATE-PROVENANCE — CreationKind survives every wrapper verbatim
  (supervising.rs:609-616; algebra.rs:727).
- CREATE-FAILURE — no installation-error path can become overwrite, birth, or
  a Restarted fact; Crash::EnvironmentFailed is the only effect-failure
  signal; replacement-at-address is absent.

Retained structures (challenged, justified):

- RETAIN-SEND-PRODUCT — nested product keeps lanes statically disjoint;
  erasure alternative would lose per-lane typing.
- RETAIN-EVENT-SUMS — closed concrete sums make matching exhaustive; a
  universal envelope was rejected.
- RETAIN-FUNCTIONS — non-capturing fn pointers: allocation-free, statically
  dispatched, Copy (supervising.rs:83-86).
- RETAIN-SPEC — facade builds concrete wrappers immediately; zero runtime
  representation.
- RETAIN-BIRTH-MODE — NoBirths::Child = Never makes births unrepresentable
  at compile time (algebra.rs:331 probes both modes).
- RETAIN-PROXY — stable restart identity derived via a live Proxy actor,
  matched against the independent IncarnationModel (model.rs:91-158).

Boundary:

- BOUNDARY-FRESHNESS — Behavior supplies nonce/kind/child; installation and
  commit are interpreter work.
- BOUNDARY-ERROR — collision/installation failure is interpreter-owned; never
  overwrite.
- BOUNDARY-LIFECYCLE — Behavior designates provenance; post-commit diagnostics
  (Restarted) are interpreter-reported only after successful installation.
- BOUNDARY-IDENTITY — runtime identity is distinct from the creator-local
  nonce throughout the observation lanes.

Rejected directions (grep-proven absent):

- REJECT-ERASURE — zero dyn/Any/TypeId/unsafe; erased effect seat is
  compile-fail (behavior.rs:412 doctest).
- REJECT-ENVELOPE — no catch-all envelope; every sum is closed.
- REJECT-REGISTRY — no runtime registries/lookup in crates/behavior/src.
- REJECT-SERIALIZATION — no serde in the crate; 'serialized' appears only as
  prose meaning sequential folds.
- REJECT-REPLACEMENT-LANE — one typed `creates` lane carries CreationKind; no
  second replacement lane exists.
- REJECT-INFERENCE — provenance is designated at two explicit constructors
  only; never inferred from generation/address/history.
- REJECT-ALLOCATOR — Create is pure data; no allocator callback in any fold.
- REJECT-SPECULATION — every trait has a law plus concrete use; no
  line-count-motivated abstraction retained.

Surface (inventories at baseline, all justified):

- SURFACE-PUBLIC — 84/84 public symbols audited and justified by ownership
  group; coverage lists the frozen set exactly.
- SURFACE-TRAITS — 11/11 traits audited: law, implementors, static role,
  diagnostics; coverage lists the frozen set exactly.
- GEN-CORE — Behavior 0 (associated types close the algebra), State 3,
  Actions 4, Create 2, SendProduct 2, Base 4, FnState 6 — every seat
  classified, none speculative.
- GEN-WRAPPERS — Spec/At/Watching/Proxy/ReceiveTimeout/Stashing 1,
  Supervising 2, Fsm 5 — every parameter classified.
- PHANTOM-01 — 4 phantoms justified: Recipient.message (fn(M),
  contravariant), Births tuple (fn() -> C capability), Base.marker
  (fn(O, Br, E) seats), Fsm.address (owning address seat).
- COMPLEXITY-01 — single clippy::type_complexity allow at behavior.rs:396-399
  (StateActed alias exposes all protocol seats); coverage exact.
- PANIC-01 — 12 production panic!/expect sites inventoried: u64 exhaustion
  boundaries, init-once contract, unknown-nonce interpreter invariants,
  usize->u64 conversions; all documented and covered by should_panic tests.
- STATIC-01 — dyn/Any/TypeId/unsafe/registry/reflection: zero occurrences,
  zero-tolerance verified by the checker.
- ERGO-01 — 31 turbofishes (proptest generators, uninhabited-error
  ascriptions, static probes), 18 helper aliases; Spec keeps application call
  sites ceremony-free; nothing increased.

Testing and synthesis:

- TEST-MODELS — model.rs is genuinely independent (spec-derived vocabulary:
  occupied/vacant/queued_successor vs implementation's worker/pending);
  layered coverage: exhaustive enumeration, proptest model suites, black-box
  reconciliations, 9 fuzz targets with populated corpora.
- TEST-COMPILE-FAIL — one compile_fail doctest (erased effect seat) plus
  positive compile-time probes (algebra.rs:19-40); phase/birth negatives are
  uninhabited-type impossibilities; honest scope recorded.
- DOC-01 — this section: every obligation, measurements, decisions, dead
  ends, risks.
- VERIFY-01 — gates green (see below).

### Before/after measurements

All ratchets are unchanged — this campaign was verification, not redesign:

| Metric | Before (baseline) | After |
| --- | --- | --- |
| test turbofish expressions | 31 | 31 |
| test helper type aliases | 18 | 18 |
| production panic!/expect sites | 12 | 12 |
| public algebra symbols | 84 | 84 |
| public traits | 11 | 11 |
| phantom fields | 4 | 4 |
| type-complexity files | 1 | 1 |
| generic arities (15 measured) | all at baseline | unchanged |
| dyn / Any / TypeId / unsafe | 0 | 0 |
| obligations resolved | 0/61 | 61/61 |
| capabilities resolved | 0/53 | 53/53 |

### Retained/reverted decisions

Retained: every structure under audit (no counterfactual experiment produced
evidence for removal or simplification). Reverted: none — no production
experiment was needed; the one Fsm mid-drain error asymmetry found by
error_paths.rs (dropped unprocessed batch vs Stop preserving it) is a
documented, test-pinned behavior, retained per the audit charter. Dead ends:
backpressure, fairness, persistence, durable-state, remoting, serialization,
distribution, scheduling derivations terminate at explicit interpreter
boundaries (queue-depth unobservable in a one-event fold; fairness quantifies
over executions; I/O is outside the effect triple) — recorded, not fixed.

### Exact commands

- `research/architecture-critical-review-loop/check.sh` — evidence gates,
  ratchets, inventories, then both repository gates.
- `cargo nextest run --workspace` — 154 tests, 154 passed, 0 skipped
  (bombay-behavior 31, bombay-behavior-testkit 123).
- `nix flake check` — all 7 aarch64-darwin checks pass (build, nextest, doc,
  fmt, toml-fmt, audit, deny).
- `cargo test --doc -p bombay-behavior` — 1 passed (compile_fail probe).
- Focused cluster gates: per-test nextest filters for algebra, core,
  boundaries, error_paths, driver_accumulation, init_contract,
  receive_timeout, shutdown_model, stash_properties, fsm_properties,
  cross_lane, two_buffer, compositions, supervision_model, workers_fleet —
  all green (counts recorded per obligation in evidence.json).

### Remaining risks

- Compile-fail coverage is intentionally thin (one doctest): phase/birth
  negatives are uninhabited-type impossibilities, but a future combinator
  with runtime validation would need a trybuild harness.
- Wrapper init is not idempotent under direct misuse (double-init duplicates
  init sends; step-before-init routes to unborn proxy generations). This is
  unreachable through the driver, which inits first; the init-once contract
  is documented and panic-guarded where cheap (Proxy::init).
- The 'L-reActor' name was not pinned to a DOI in this pass; the algebraic
  actor line is covered by the verified Agha-Thati and Agha-Thati-Ziaei
  papers, so no adopted law depends on the missing citation.
- Interpreter obligations (freshness acceptance, collision errors, Restarted
  diagnostics) are specified and testkit-validated but enforced in actorpass,
  outside this repository's gates.

### Conclusion

The pure behavior algebra is complete against the surveyed capability
catalogue: every pure capability is either an existing basis instance or a
derivation from the seven-construct basis; every remaining capability is an
explicit interpreter or application boundary with a recorded algebraic
obstruction. No primitive was added, no dynamic escape exists, every
composition preserves every lane, and both authoritative gates pass. The
audit found no genuine gap in the reusable pure actor-behavior algebra.
