# actorpass design — create-at-address, keep-address restart (distilled 2026-08-04)

> **Status:** design approved in conversation; behaviorpass algebra refinement
> is the first implementation step, actorpass follows. This doc is the record
> of the distillation: every decision cites the evidence it was derived from.

**Goal:** distill bombay's entangled actor runtime into a third pillar-pass —
**actorpass**, the concurrency primitive — as clean as behaviorpass's behavior
algebra. The layering: behavior is a pure fold (behaviorpass), the mailbox is
fastpass, and actorpass is *only* the effect interpreter: own the task, own the
mailbox, interpret the `Actions` triple, expose handles.

$$\text{actor} = \text{process}(\text{behavior}) \qquad
  \text{handle} = \text{identity} + \text{sender}\langle M\rangle$$

## Key decisions (resolved; alternatives noted)

1. **Birth = create-at-address (the behavior mints).** The creating behavior
   calls `mint() -> (Handle<M>, Consumer<C, M>)` — fresh id + `fastpass::channel`
   — stores the `Handle` in its own state in the SAME fold, and emits a
   create-spec carrying `(consumer, child_behavior)`. The driver's whole create
   interpreter is `tokio::spawn(drive(child, consumer))`. This is AMST's core
   calculus verbatim: `newadr()` allocates the address (queue exists, nobody
   home, creator's continuation receives the address synchronously),
   `initbeh(a, b)` installs the behavior — and only the creator may init
   (Agha/Mason/Smith/Talcott, JFP 7(1) 1997, §3). Same-turn create-then-send
   works: sends to the newborn queue in its mailbox before spawn.
   *Rejected:* driver-mints + feedback envelope (`Created { handle }`) — kills
   same-turn create-then-send, forces every create-user to reify a continuation
   into its state while awaiting the handle; no successful system does this
   (Erlang `spawn` returns the pid, Akka `actorOf` returns the ref, AMST
   `newadr` returns into the continuation). *Rejected:* framework token table
   (live `MailAddr`) — loses compile-time send typing, and tokens are not
   mobile across actors (Agha addresses MUST be passable in messages), which
   forces a global registry the model deliberately lacks.

2. **Local ids are hash-derived: `id = H(self_id, birth_nonce)`.** `self_id`
   is injected at construction; one root seed makes the entire id space
   deterministic — the golf's trace-equality gate gets exact reproducibility by
   construction. Freshness is AMST's side condition discharged by derivation,
   not computation. Predictable ids are safe under bombay ADR-0015 (ActorId is
   designation, not authority; the sender is the authority). *Rejected as the
   primary scheme:* global atomic counter (ambient state; harness needs counter
   hygiene — acceptable fallback if self-id threading proves noisy).

3. **Restart is NOT a create — keep-address (OTP), swapped via control lane.**
   Agha's model has no failure semantics (verified in the sources); restart is
   OTP's contribution. Fresh-address restart is *broken by address mobility*:
   once a handle escapes in messages the supervisor cannot know the holders,
   so re-pointing is impossible without a registry+healing protocol. Keep-address
   restart makes the whole feedback problem vanish: mailbox and id survive,
   the process swaps only its behavior slot, every stored handle stays valid
   forever. Mechanism: the supervising parent holds each child's
   `ControlSender` (it minted it at birth — the lifecycle handle falls out of
   creation); on `ChildStopped`, `Supervising` still makes the pure
   budget/liveness decision and emits `(build)(idx)`, now interpreted as
   `Restart(fresh_behavior)` delivered on the CHILD's control lane.
   **fastpass's API does not change** — the control lane's type parameter is
   `Ctrl<B>` including `Restart(B)`.

4. **behaviorpass algebra refinement (the only touch-point).** Today
   `Supervising` emits restarts AS creates because the golf has no live
   processes. In the live split, `creates` become **births only** and restart
   is a distinct effect — either a fourth `Actions` leg
   (`restarts: Vec<(SlotIdx, C)>`) or a create enum
   (`Birth { consumer, child } | Reincarnate { slot, child }`). Shape is a
   golf-ergonomics call, decided at implementation. `Base`, `Deadlined`,
   `Watching`, `Stashing`, `Fsm` are untouched. (Audit that forced this: only
   `Supervising` ever creates — `B: Behavior<Offspring = Never>` fences user
   handlers out of creation; only `Base`'s user handler sends; framework
   reactions are become-only via `lift`.)

5. **identitypass: NO crate.** Local identity is two functions
   (`Id::root(seed)`, `Id::birth(self, nonce)`) — a module in actorpass, not a
   crate; promote only at a second concrete consumer (the workspace YAGNI
   rule). KERI machinery already exists in cesride; rebuilding it in a pass
   repo is duplication. AIDs are earned ONLY at the dataspace boundary —
   AMST's receptionists — because keygen is entropy, an effect, and entropy
   inside the pure fold kills determinism. "Not all actors get AIDs" is forced
   by purity, not preference. Actors carry AIDs; SAIDs are for data (message
   payloads), not actor names. The local↔AID pairing remains bombay card #121.

## Repo boundaries after the distillation

- **behaviorpass** — unchanged except decision 4. The fold, the wrappers, the
  golf, the frozen-reference trace gate all stand.
- **actorpass** — starts as a copy of behaviorpass, then adds ONLY: `Handle`
  (id + senders; bombay ADR-0010 two-word shape, ref-count-driven drain-stop),
  `identity` module, `mint()`, the `drive` loop (recv → step → interpret:
  births spawn, sends deliver, restarts ride control lanes), the behavior-slot
  swap. If it grows a registry or lifecycle ceremony it is becoming bombay
  again — that is the failure mode to guard.
- **fastpass** — unchanged; control lane carries `Ctrl<B>`.
- **bombay** — shrinks to request/reply (oneshot-in-message pattern), the name
  registry, and KERI pairing at the Zenoh boundary.

## Evidence trail (primary sources)

- Agha 1986 (MIT Press): actor = mail address + sufficiently large mail queue +
  behavior function; behavior maps a communication to (sends, new actors,
  replacement behavior); the mail system is IMPLICIT configuration machinery —
  one queue per address, fairness assumed (every message eventually delivered).
- AMST 1997 (JFP 7(1)): primitives `send`/`become`/`letactor`; core split
  `newadr()` + `initbeh(a,b)`; `<new: a, a′>` returns the fresh address into the
  creator's continuation in ONE transition; the new actor exists uninitialized
  and only its creator may `initbeh` it; `become` spawns an anonymous clone —
  machinery Rust obviates with `&mut self` (the fold IS the history-sensitive
  cell). Equational laws (perm/gc/delay/cellb) double as proptest oracles.
- De Koster/Van Cutsem/De Meuter (AGERE 2016): four actor families; the 1986
  model remains the Classic-family reference semantics; selective receive (the
  Erlang/Processes family) is the feature mailbox-types research
  (de'Liguoro/Padovani ECOOP 2018; Fowler et al. 2023) exists to tame —
  behaviorpass declines it by construction (flat Envelope, explicit Stash).
- Basset (Lauterburg/Dotta/Karmani/Marinov/Agha, ASE 2009/FSE 2010): DPOR
  schedule exploration needs exactly the structure a pure fold exposes —
  deterministic steps + inspectable effects. Upgrade path for the testkit:
  seedable ids + driver-controlled delivery order = cheap schedule exploration.
- Garnock-Jones (Conversational Concurrency, 2017): Syndicate's
  assertions/presence over actors — the formal home of "dataspace" vocabulary;
  relevant at bombay's Zenoh/KERI boundary, not to the fold.

## Implementation order

1. **behaviorpass**: the decision-4 algebra refinement (restart leg), with the
   golf re-gated on trace equality.
2. **actorpass**: copy behaviorpass; add identity + mint + Handle + drive +
   slot swap, per decision 1–3.
3. fastpass untouched; bombay absorbs findings as cards per the pass-family
   protocol.
