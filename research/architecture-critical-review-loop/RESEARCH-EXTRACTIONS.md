# Primary-source extraction notes (sequential processing log)

One source per section, in the order processed. Each entry follows the
capability extraction template from RESEARCH-SOURCES.md.

## 1. Agha, Thati, Ziaei — Actors: A Model for Reasoning about Open Distributed Systems (2001)

- Bibliography: Gul A. Agha, Prasannaa Thati, Reza Ziaei. In *Formal Methods
  for Distributed Processing: A Survey of Object-Oriented Approaches*, Bowman
  & Derrick (eds.), Cambridge University Press, 2001, chap. 8.
- Stable URL/DOI: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/agha-2001-actors.pdf>; OSL records
  <https://osl.cs.illinois.edu/publications/agha01actors.html>,
  <https://osl.cs.illinois.edu/publications/fmdp01.html>
- Source tier: 1 (author-hosted manuscript of a CUP chapter).
- Read status: complete (full PDF, 2026-08-09).
- Inclusion: semantic.
- Primitive syntax/operations: actor redexes `send(a, v)`, `newactor(b)`,
  `ready(b)` over a call-by-value functional base; configurations
  `⟨α; μ⟩_ρ` with actor map α, message pool μ, receptionist interface ρ;
  labeled transitions `<fun>`, `<new>` (fresh name returned to creator),
  `<send>` (asynchronous, into pool), `<rcv>` (ready blocks until delivery,
  applies behavior to message), `<out>`/`<in>` (open-boundary exchange
  restricted by the interface; the interface grows with free names of
  outgoing content — dynamic acquaintance).
- Operational assumptions: asynchronous buffered communication as primitive;
  fair message delivery; unique persistent actor names; names may be
  communicated (name mobility); an actor may not create actors with names
  received in messages (freshness is creation-only); names are pure (no
  creation/location information) — contrasted with π-calculus, where names
  identify stateless channels.
- Laws/theorems: computation trees/paths; i/o-path correspondence proof
  technique (from Mason–Talcott) for observational equivalence; fairness as
  eventual delivery.
- Capability claims: **local synchronization constraints** — `ready(b, c)`
  with c a predicate over messages delaying delivery — are explicitly
  *translatable into the primitive semantics*: "it is possible to translate
  the actors with local synchronization constraints into actors obeying the
  primitive semantics; a proof that this translation is semantics preserving
  can be found in [MT]." Direct primary evidence that selective receive /
  synchronization constraints are DERIVED, not primitive.
- Bombay classification: send/newactor/ready = LAW (nucleus); open
  interface ρ = LAW for open systems; synchronization constraints =
  BOMBAY-DERIVED (per the paper's own translation result) realized as
  Stashing/enabled-set filtering.
- Candidate-basis impact: confirms nucleus; confirms open-system composition
  needs only receptionist interfaces, not new primitives; confirms
  selective-receive is derived.
- Concrete derivation or obstruction: none — supports existing basis.
- Limitations/non-transferable assumptions: untyped λ base (Bombay adds
  static typing as a derived construction); fairness is an execution
  quantifier, not a per-fold property.
- Evidence location in code/tests: crates/behavior/src/stashing.rs
  (selective receive), behavior.rs (send/create/become).

## 2. OSL publication catalogue (index)

- Bibliography: Open Systems Laboratory, "Publications," 2026-08-09 snapshot.
- Stable URL: <https://osl.cs.illinois.edu/publications/>
- Source tier: discovery index (not semantic evidence).
- Read status: complete (419 entries walked).
- Disposition recorded in AGHA-BIBLIOGRAPHY.md.

## 3. DBLP Gul A. Agha record (index)

- Bibliography: DBLP author record, XML export, 244 records, 1984–2026.
- Stable URL: <https://dblp.org/pid/a/GulAAgha>
- Source tier: discovery index.
- Read status: complete (all 244 records enumerated and reconciled).
- Disposition recorded in AGHA-BIBLIOGRAPHY.md (DBLP-only additions:
  1986 language overview, 1986 fairness abstract, 2019 Types for Progress,
  2021/2023 failure-aware consensus verification, 2025 CRGC).

## 4. Thati, Ziaei, Agha — A Theory of May Testing for Actors (FMOODS 2002)

- Bibliography: Prasannaa Thati, Reza Ziaei, Gul Agha. FMOODS 2002, IFIP
  Conference Proceedings 209:147–162, Kluwer.
- Stable URL/DOI: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/thati-2002-fmoods-a_theory_of_may_testing_for_actors.pdf>
- Source tier: 1.
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic.
- Primitive syntax/operations: Aπ, a typed asynchronous π-calculus;
  preterms `0 | x(y).C | xy | [x=y](C1,C2) | (νx)C | C1|C2 | B⟨x̃;ỹ⟩`;
  behavior identifiers with defining equations (recursive behaviors = become);
  a type system over judgments `ρ; f ⊢ C` enforces actor properties: unique
  persistent names, freshness (no creation with received or well-known
  names), and a relaxed persistence where an actor may temporarily assume a
  chain of fresh names to delegate synchronization (polyadic communication).
- Operational assumptions: same reduction semantics as asynchronous
  π-calculus; fairness = finite-but-unbounded delivery delay; **the three
  basic actions on receiving a message (create, send, become) are concurrent
  with no ordering between them** — primary evidence that Bombay's product
  `Actions` (unordered effect seats) matches the model, and that any
  cross-lane ordering is interpreter policy.
- Laws/theorems: may-testing preorder; trace-based characterization avoiding
  quantification over contexts; three calculus variants differing in
  name-matching power (match+mismatch, match-only, restricted local match).
- Capability claims: equivalence reasoning for actors reduces to trace
  inclusion; temporary-name delegation expresses synchronization without a
  blocking-receive primitive.
- Bombay classification: effect triple and fairness = LAW; name-matching
  variant choice = INTERPRETER/BOMBAY-POLICY (Bombay's `Address` exposes
  equality and `route()`, so Bombay sits in the match-capable fragment);
  temporary-name delegation = DERIVED construction (Bombay uses typed
  continuation replies instead).
- Candidate-basis impact: none — no new primitive; confirms unordered effect
  product.
- Limitations: untyped message contents (types govern names, not payloads);
  the characterization is for equivalence reasoning, not for typing payloads.
- Evidence location: crates/behavior/src/behavior.rs (Actions product),
  protocol.rs (Route equality).

## 5. Thati, Ziaei, Agha — A Theory of May Testing for Asynchronous Calculi with Locality and No Name Matching (AMAST 2002)

- Bibliography: Prasannaa Thati, Reza Ziaei, Gul Agha. AMAST 2002, LNCS
  2422:223–238. doi:10.1007/3-540-45719-4_16.
- Stable URL/DOI: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/thati-2002-amast-a_theory_of_may_testing_for_asynchronous_calculi_with_locality_and_no_name_matching.pdf>
- Source tier: 1.
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic.
- Primitive syntax/operations: subcalculi Lπ= (locality, with match) and Lπ
  (locality, no match) of asynchronous π: `0 | xy | x(y).P | P|P | (νx)P |
  [x=y]P | !x(y).P`; **locality** = for every subterm `x(y).P`, the bound y
  never occurs as the subject of an input in P — received names cannot be
  listened on (object-paradigm non-interference). Early-style LTS
  (INP/OUT/PAR/COM/RES/OPEN/CLOSE/REP/MATCH).
- Operational assumptions: asynchronous communication; no global name
  comparison in Lπ (name matching likened to pointer comparison; Join and
  Pict disallow it to enable transformations).
- Laws/theorems: parameterized may preorder `≼_ρ` with observers forbidden
  to listen on owned names ρ (encapsulation); trace-based characterization;
  **complete axiomatization of the finitary (replication-free) fragment**;
  extra laws valid only under locality and no-matching.
- Capability claims: encapsulation reasoning; safety via may testing.
- Bombay classification: locality = LAW-adjacent model assumption that Bombay
  enforces structurally (only the owning behavior folds its mailbox events;
  no behavior can subscribe another actor's lane); no-name-matching = NOT
  adopted — Bombay exposes address equality, so only the Lπ= variant's
  results transfer; this qualification is recorded on the routing and
  correlation rows.
- Candidate-basis impact: none; equivalence-level results, not primitives.
- Limitations: finitary fragment only for completeness; transfer to Bombay
  limited by Bombay's name-matching (address equality) exposure.
- Evidence location: crates/behavior/src/behavior.rs (Address, MailAddr
  equality), supervising.rs (no cross-actor lane subscription).

## 6. Thati — Towards an Algebraic Formulation of Actors (MS thesis, 2000/2001)

- Bibliography: Prasannaa Thati. MS thesis, UIUC, 2000 (OSL catalogue 2001).
- Stable URL: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/thati-2001-towards_an_algebriac_formulation_of_actors.pdf>
- Source tier: 1 (original dissertation).
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic.
- Primitive syntax/operations: System A — fair asynchronous name-passing
  calculus; standard process-algebra combinators (parallel composition,
  restriction, input/output prefixes, matching) plus a type system enforcing
  the actor name discipline; semantics by labeled transitions with unfair
  infinite sequences eliminated.
- Operational assumptions (stated as the model): three basic actions per
  message — create finitely many actors with universally fresh names, send
  finitely many messages, assume a new behavior with the same name; **all
  concurrent, no sequential ordering between them**; no shared state;
  messages consumed on delivery (no redelivery).
- Laws/theorems: **locality laws** — (1) finite acquaintances only; (2)
  messages carry finitely many names; (3) actors cannot guess names:
  post-transition acquaintances ⊆ prior acquaintances ∪ message contents ∪
  freshly created names. Encapsulation: configuration interface [ρ, χ],
  receptionists ρ ⊂ dom, external names χ ∩ dom = ∅; interface is dynamic
  (ρ grows when a hidden name is sent out). Fairness: delivery cannot be
  infinitely delayed; the weaker no-starvation property is insufficient for
  general eventuality equivalences (garbage-collection equivalence example).
  Composition constraints: disjoint actor names across composed
  configurations.
- Capability claims: call/return communication derived two ways —
  continuation actors and insensitive behaviors (Chapter 5); direct primary
  evidence that request-reply is a derived pattern, not a primitive.
- Bombay classification: locality laws 1–3 = LAW (Bombay: typed Recipient
  values embody acquaintances; no name guessing = no address fabrication in
  the pure fold — strengthening recorded as bombay-derived static
  enforcement); fairness = LAW at configuration level, interpreter property;
  receptionist/external interface = LAW for open composition.
- Candidate-basis impact: confirms nucleus; supplies the acquaintance law in
  its strongest stated form; confirms request-reply derived.
- Limitations: name-passing setting (Bombay types message payloads
  statically, a derived strengthening).
- Evidence location: crates/behavior/src/behavior.rs (Recipient, Address),
  request-reply row of capability-matrix.json.

## 7. Thati — A Theory of Testing for Asynchronous Concurrent Systems (PhD 2003), subsuming Agha–Thati LNCS 2004

- Bibliography: Prasannaa Thati. PhD dissertation, UIUC, October 2003; and
  Gul Agha, Prasanna Thati. "An Algebraic Theory of Actors and Its
  Application to a Simple Object-Based Language." LNCS 2635:26–57, 2004.
  doi:10.1007/978-3-540-39993-3_4.
- Stable URL/DOI: dissertation PDF
  <https://osl.cs.illinois.edu/media/papers/thati-2003-a_theory_of_testing_for_asynchronous_concurrent_systems.pdf>
- Source tier: 1.
- Read status: dissertation complete; the LNCS 2004 chapter itself is
  paywalled and was **not** separately read — its technical basis (the
  algebra Aπ and the simple actor language SAL) is Chapters 3–4 of the
  dissertation, which were inspected. Recorded as an access limitation.
- Inclusion: semantic.
- Primitive syntax/operations: Aπ (Chapter 3): actor model as a typed
  asynchronous π-calculus with behavior identifiers; type rules (Table 3.1)
  enforcing uniqueness, persistence, freshness; new transition rules (Table
  3.2). SAL (Chapter 4): a simple actor language — expressions, commands
  (send, create, ready/become), behavior definitions — with a formal
  translation into Aπ, demonstrating the algebraic theory applied to an
  object-based language (the 2004 paper's stated program).
- Operational assumptions: asynchrony, locality, object paradigm,
  restrictions on name matching, as typed commitments over π.
- Laws/theorems: alternate characterization of may testing for Aπ;
  variants Aπ=, Aπ match-only, restricted match; complete axiomatization
  and decision procedures for testing equivalences over asynchronous finite
  state machines (Chapter 5, Karp–Miller coverability); Maude-executable
  specifications of the calculi and the may preorder (Chapter 6).
- Capability claims: object-language constructs (methods, fields, behavior
  definitions) are derived encodings over the primitive algebra — the core
  "derived vs primitive" methodology precedent for Bombay's calculus.
- Bombay classification: LAW (nucleus confirmations); DERIVED (language
  constructs over the algebra; Bombay's Fsm/Spec combinators play the SAL
  role).
- Candidate-basis impact: none — strengthens the derived-form methodology.
- Limitations: decidability results are for finite-state fragments; SAL is
  untyped in payload.
- Evidence location: crates/behavior/src/fsm.rs, spec.rs (derived language
  combinators).

## 8. Agha, Meseguer, Sen — PMaude (QAPL 2005 / ENTCS 2006)

- Bibliography: Gul A. Agha, José Meseguer, Koushik Sen. "PMaude:
  Rewrite-Based Specification Language for Probabilistic Object Systems."
  QAPL 2005; ENTCS 153(2):213–239, 2006. doi:10.1016/j.entcs.2005.10.040.
- Stable URL/DOI: doi:10.1016/j.entcs.2005.10.040; OSL record
  <https://osl.cs.illinois.edu/publications/journals/entcs/AghaMS06.html>
- Source tier: 1.
- Read status: abstract + OSL record (full text not author-hosted on OSL;
  recorded as access limitation). The abstract answers this campaign's queue
  question directly.
- Inclusion: semantic.
- Primitive syntax/operations: probabilistic rewrite theories; an actor
  PMaude module modeling actors as concurrent objects with asynchronous
  message passing; QuaTEx quantitative temporal expressions.
- Operational assumptions: purely probabilistic (quantified) models vs
  unquantified nondeterminism; statistical analysis via discrete-event
  simulation samples.
- Laws/theorems: rewriting-logic semantics; statistical evaluation of
  quantitative queries.
- Capability claims: probability is a **specification and observation layer
  over actor configurations**, not an actor transition primitive.
- Bombay classification: INTERPRETER/environment observation; no behavior-
  algebra content claimed.
- Candidate-basis impact: none.
- Limitations: full text not re-inspected; classification rests on the
  abstract plus the citing line (FMOODS 2003 precursor, inspected record).
- Evidence location: none in code (boundary classification).

## 9. Agha et al. — Formal Modeling and Analysis of DoS Using Probabilistic Rewrite Theories (FCS 2005)

- Bibliography: Gul Agha, Carl Gunter, Michael Greenwald, Sanjeev Khanna,
  José Meseguer, Koushik Sen, Prasanna Thati. FCS 2005.
- Stable URL/DOI: OSL record
  <https://osl.cs.illinois.edu/publications/fcs05.html> (no full text hosted;
  recorded as access limitation).
- Source tier: 1.
- Read status: bibliographic record only.
- Inclusion: semantic (application of the PMaude line to DoS analysis).
- Bombay classification: INTERPRETER/environment; no primitive impact.
- Limitations: content not directly inspected; carried by the PMaude
  extraction above.

## 10. Karmani, Agha — Actors (Encyclopedia of Parallel Computing, 2011)

- Bibliography: Rajesh K. Karmani, Gul Agha. "Actors." Encyclopedia of
  Parallel Computing, Springer, 2011, pp. 1–11.
  doi:10.1007/978-0-387-09766-4_125.
- Stable URL/DOI: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/karmani-2011-actors.pdf>
- Source tier: 1 (later Agha-coauthored semantic summary).
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic.
- Primitive syntax/operations: send messages, create new actors, update
  local state (become); globally unique names; names cannot be guessed but
  may be communicated.
- Operational assumptions: asynchronous non-blocking send; indeterminate
  arrival order; eventual delivery (fairness); one-at-a-time atomic message
  processing (macro-step semantics); encapsulation (no shared state);
  location transparency (location does not affect names; enables migration).
- Laws/theorems: **"Language constructs can enable actor programmers to
  specify such patterns. Such language constructs are definable in terms of
  primitive actor constructs, but providing them as first-class linguistic
  objects simplifies the task of writing parallel code."** — the derived-form
  doctrine stated by Agha's own later summary. RPC-like messaging derived via
  buffering non-reply messages (the stash pattern) and continuation;
  synchronization constraints as declarative predicates (enabled sets).
- Capability claims: RPC-like messaging = derived (request-reply row);
  local synchronization constraints = derived (selective-receive/stashing
  rows); pipeline and divide-and-conquer = application patterns; location
  transparency = naming/migration service (interpreter); actor garbage
  collection = open systems problem (inverse acquaintances), interpreter.
- Bombay classification: LAW (macro-step atomicity, fairness, encapsulation,
  unique unguessable names); DERIVED (RPC, synchronization constraints);
  INTERPRETER (location transparency service, GC, scheduling).
- Candidate-basis impact: confirms the seven-construct basis doctrine; no
  gap.
- Limitations: summary article; individual claims trace to the primary works
  dispositioned separately.
- Evidence location: capability-matrix.json rows request-reply,
  selective-receive, stashing, location-transparency, scheduling.

## 11. Plyukhin, Agha — Scalable Termination Detection (CONCUR 2020 / LMCS 2022)

- Bibliography: Dan Plyukhin, Gul Agha. "Scalable Termination Detection for
  Distributed Actor Systems." CONCUR 2020, LIPIcs 171:11:1–11:23,
  doi:10.4230/LIPIcs.CONCUR.2020.11; journal version "A Scalable Algorithm
  for Decentralized Actor Termination Detection," LMCS 18(1:39), 2022,
  doi:10.46298/lmcs-18(1:39)2022; arXiv:2104.05128.
- Stable URL/DOI: <https://arxiv.org/abs/2104.05128>
- Source tier: 1.
- Read status: abstract + records (CONCUR/LMCS/CoRR); the abstract states
  the theorems relevant to this campaign's queue question.
- Inclusion: semantic (lifecycle), classified interpreter for Bombay.
- Primitive syntax/operations: DRL (deferred reference listing) — asynchronous
  local snapshots plus message passing; no causal delivery or nonlocal
  monitoring required.
- Operational assumptions: actor termination/quiescence = not processing a
  message and cannot receive one in future; reachability from roots is
  inadequate for actors (an unreachable actor may message a reachable one).
- Laws/theorems: safety (all identified garbage have terminated) and liveness
  (terminated actors eventually identified, under stated assumptions).
- Capability claims: termination detection is a decentralized runtime
  algorithm over configurations — interpreter boundary, not a behavior
  primitive.
- Bombay classification: INTERPRETER (lifecycle/termination detection);
  no pure-fold content.
- Candidate-basis impact: none; confirms lifecycle-publication and
  termination stay at the boundary.
- Limitations: full formal development not re-inspected line by line;
  classification does not depend on the algorithm's internals.
- Evidence location: capability-matrix.json row lifecycle-publication.

## 12. Agha, Mason, Smith, Talcott — A Foundation for Actor Computation (JFP 1997)

- Bibliography: Gul A. Agha, Ian A. Mason, Scott F. Smith, Carolyn L.
  Talcott. JFP 7(1):1–72, 1997. doi:10.1017/S095679689700261X.
- Stable URL/DOI: author-hosted PDF
  <https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf>
- Source tier: 1.
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic — the semantic nucleus authority.
- Primitive syntax/operations: call-by-value λ (computational lambda
  calculus laws preserved) + structure constructors/recognizers/destructors +
  actor primitives `send(a,v)`, `letactor{x := b}e` (fresh creation; new
  address bound in body and behavior; multiple mutually-acquainted creations),
  `become(b)` (replacement behavior; spawns anonymous continuation actor for
  the rest of the computation, which can never receive). Configurations =
  actors + messages en route + receptionists + external names.
- Operational assumptions: asynchronous fair message passing; receptionist/
  external interface explicitly models open distributed systems;
  nondeterministic arrival order across sources; "the current behavior of an
  actor always remains a deterministic function of the sequence of messages
  that the actor has thus far received" — direct primary support for
  Bombay's pure deterministic fold.
- Laws/theorems: configuration composition is **associative, commutative,
  with unit**; fairness built into the semantics (only fair infinite
  sequences); **in the presence of fairness, convex/must/may observational
  equivalences collapse to two**; observational equivalence related to
  de Nicola–Hennessy testing; equational laws include permutation of
  adjacent sends and unobservability of become/send order (b5 ≡ b5′) —
  primary evidence that effect seats within one transition commute, i.e.
  Bombay's unordered `Actions` product is law-backed; join continuations
  derived (treeprod example); active-garbage collection does not preserve
  semantics without fairness.
- Capability claims: cells/state = become-derived; join continuations =
  derived synchronization; sink = derived; receptionists = open interface.
- Bombay classification: LAW (deterministic per-actor fold, effect triple,
  fresh creation, fairness, interface composition laws); DERIVED (Bombay's
  typed payloads, nonce routing); POLICY (creation-before-send
  interpretation).
- Candidate-basis impact: nucleus confirmed with laws (assoc/comm/unit of
  composition; send permutation; become/send commute).
- Limitations: untyped; `become` spawns an anonymous continuation actor —
  Bombay's become is the simpler verdict replacement; the difference is
  recorded as a derived-construction simplification (no observable-content
  difference under one-communication-at-a-time interpretation).
- Evidence location: crates/behavior/src/behavior.rs (Behavior, Actions,
  Create), verdict.rs.

## 13. Hewitt, Bishop, Steiger — A Universal Modular ACTOR Formalism for AI (IJCAI 1973)

- Bibliography: Carl Hewitt, Peter Bishop, Richard Steiger. IJCAI 1973,
  pp. 235–245.
- Stable URL: <https://www.ijcai.org/Proceedings/73/Papers/027B.pdf>
- Source tier: 1.
- Read status: complete (PDF inspected 2026-08-09).
- Inclusion: semantic (origin paper).
- Primitive syntax/operations: one kind of object (actors); all behavior
  modes defined via message sending; `(%A M%)` send; `=>` receive with
  pattern matching; `cases`; cells; ANONYMOUS fresh-name generation;
  continuations are full actors; unidirectional sends (no reply
  presupposition).
- Operational assumptions: acquaintance addressing ("each [actor] can
  address others with whom it is acquainted"); actor controls the names it
  uses; access rights by mutual agreement between creating and accessing
  actors; no goto/interrupt/semaphore; per-actor scheduler/banker/monitors.
- Laws/theorems: actor induction principle for intentions (contracts);
  comparative schematology hierarchy of control structures.
- Capability claims: protection/capability claims (actors as a universal
  protection mechanism) — noted as aspiration; no formal enforcement
  mechanism is defined, which supports classifying security-capability as
  interpreter-side.
- Bombay classification: LAW (acquaintance addressing, fresh names, message
  passing as sole mode); the scheduler/banker/monitor hierarchies are
  runtime structure = INTERPRETER.
- Candidate-basis impact: origin authority for the acquaintance and
  freshness laws.
- Limitations: pre-operational-semantics presentation; later Agha/Clinger
  work is authoritative for transition-level laws.
- Evidence location: evidence.json RESEARCH-AGHA; behavior.rs.

## 14. Talcott — Composable Semantic Models for Actor Theories (TACS 1997 / HOSC 2000); subsumes "An Actor Algebra" (AFP 1996)

- Bibliography: Carolyn L. Talcott. "Composable Semantic Models for Actor
  Theories." TACS 1997, LNCS 1281:321–364, doi:10.1007/BFb0014558; journal
  version *Higher-Order and Symbolic Computation*, doi:10.1023/A:1010042915896.
  The AFP 1996 lecture notes "An Actor Algebra" were not directly accessible;
  their technical content is subsumed by these papers (access limitation
  recorded).
- Stable URL/DOI: doi:10.1007/BFb0014558 (abstract + full reference list
  inspected 2026-08-09; full text paywalled).
- Source tier: 1.
- Read status: abstract + references (access limitation).
- Inclusion: semantic.
- Primitive syntax/operations: three semantic models — event diagrams
  generalized to open systems, interaction diagrams, interaction paths;
  algebra on each domain with **parallel composition, hiding of internal
  actors, renaming**; semantics is a component-algebra homomorphism.
- Laws/theorems: compositionality via algebra homomorphism; the cited
  companion result (Mason–Talcott, "A semantically sound actor translation,"
  ICALP 1997) is the [MT] semantics-preserving translation of actor language
  features (synchronization constraints) into the primitive basis that the
  2001 open-systems paper invokes.
- Bombay classification: composition/hiding/renaming laws = LAW-level
  composition results; Bombay's closed typed sums and wrapper composition
  play the hiding role at the type level (bombay-derived).
- Candidate-basis impact: none; confirms composition algebra exists at the
  configuration level, not the behavior-fold level.
- Limitations: full text not inspected; algebra details taken from the
  abstract.
- Evidence location: evidence.json RESEARCH-FORMALISMS; RESEARCH-AGHA.

## 15. De Koster, De Meuter — A Formal Specification For Half a Century of Actor Systems (LNCS 16120, 2025/2026)

- Bibliography: Joeri De Koster, Wolfgang De Meuter. In *Concurrent
  Programming, Open Systems and Formal Methods* (festschrift), LNCS 16120,
  pp. 3–35, 2025/2026. doi:10.1007/978-3-032-05291-9_1. PLT Redex models:
  <https://gitlab.soft.vub.ac.be/jdekoste/actormodelhistorypltredex>
- Source tier: 1 (formal paper with mechanized operational semantics).
- Read status: the paper's full text is paywalled (access limitation), but
  its complete mechanized content — the PLT Redex models for all four
  families — was read from the authors' public repository
  (gitlab.soft.vub.ac.be/jdekoste/actormodelhistorypltredex), classic.rkt
  inspected in full on 2026-08-09.
- Inclusion: semantic (taxonomy and unifying principle).
- Mechanized Classic Actors semantics (classic.rkt): configuration k = list
  of actors (ιa μ e o) = address, mailbox, current expression, object
  (class + fields); expressions exactly `spawn`, `send`, `become`, `let`;
  BECOME replaces the actor's object at the same address (no allocation);
  SEND appends to the target mailbox (asynchronous); SPAWN creates with a
  `fresh` address returned to the creator; RECEIVE runs one method body to
  completion before the next (isolated turn); mailbox `match` scans for the
  first message with a matching method — selective receive realized as
  mailbox filtering at the interpreter level.
- Claims: four actor families — Classic Actors, Active Objects, Processes,
  Communicating Event Loops; the **Isolated Turn Principle** is identified
  as the key unifying principle of all actor models; each family's core
  subset gets a formal operational semantics.
- Bombay classification: Bombay is a Classic Actors realization; the
  Isolated Turn Principle corresponds to the one-communication-at-a-time
  LAW; the mechanized basis (spawn/send/become only) independently confirms
  Bombay's nucleus; their mailbox-scan selective receive vs Bombay's
  explicit Stashing are two realizations of the same derived capability
  (Bombay's keeps it pure and inspectable in the fold).
- Candidate-basis impact: none; independent 2025 mechanized confirmation
  that spawn/send/become suffice for the classic family.
- Limitations: paper prose not read (paywalled); Active Objects/Processes/
  Event Loops family models exist in the same repo but were not needed for
  the Bombay placement and were not inspected.
- Evidence location: RESEARCH-FORMALISMS comparison table.

## 16. Charalambides, Dinges, Agha — Parameterized Concurrent Multi-Party Session Types (FOCLASA 2012 / SCP 2016)

- Bibliography: Minas Charalambides, Peter Dinges, Gul Agha. FOCLASA 2012,
  EPTCS 91:16–30, doi:10.4204/EPTCS.91.2, arXiv:1208.4632; journal version
  "Parameterized, Concurrent Session Types for Asynchronous Multi-Actor
  Interactions," SCP 115-116:100–126, 2016, doi:10.1016/j.scico.2015.10.006.
- Source tier: 1.
- Read status: FOCLASA 2012 version complete (arXiv:1208.4632 full text
  extracted 2026-08-09); the SCP 2016 journal extension remains paywalled
  (access limitation), dispositioned via the FOCLASA text plus abstract.
- Inclusion: semantic (typed actor protocols).
- Claims: System-A, a typing language for parameterized, asynchronous,
  multi-actor protocols; programmer supplies a global type; a projection
  algorithm generates endpoint (local) types; implementations are checked
  for conformance against the projected types. Supports arbitrary shuffles
  (asynchrony), parallelism, and parameterization (indexed participant
  families); statically verifies protocols such as sliding window that are
  inexpressible in prior session-type systems. Explicitly omits session
  delegation; realizability results stated with restrictions. Global
  types -> projection -> local conformance is exactly the session-typing
  discipline: a static guarantee layer over actor addressing.
- Bombay classification: session-style protocol verification is a TYPING
  DISCIPLINE layered over actor addressing — a derived/static-guarantee
  layer, not a behavior primitive. Direct support for the protocol-session
  row's classification (partial representation; no basis gap).
- Candidate-basis impact: none; an adversarial-probe result.
- Limitations: type-system layer assumes cooperative participants; not an
  actor-model law.
- Evidence location: capability-matrix.json row protocol-session.

## 17. Charalambides, Palmskog, Agha — Types for Progress in Actor Programs (2017 workshop / LNCS 11665 2019)

- Bibliography: Minas Charalambides, Karl Palmskog, Gul Agha. In *Models,
  Languages, and Tools for Concurrent and Distributed Programming*, LNCS
  11665:315–339, 2019, doi:10.1007/978-3-030-21485-2_18; earlier workshop
  version 2017. Full development: Charalambides, *Actor Programming with
  Static Guarantees*, PhD thesis, UIUC 2018, hdl:2142/101036.
- Source tier: 1.
- Read status: abstract + references (full text paywalled; access
  limitation).
- Inclusion: semantic (liveness typing for actors).
- Claims: typestate-tracking type system over a simple actor language
  statically guarantees dynamically generated reply requirements are
  eventually satisfied, for a restricted class of programs — liveness via
  types over the standard model.
- Bombay classification: a static-guarantee layer over the nucleus
  (bombay-derived analogue: Bombay's typed lanes); no primitive impact.
- Candidate-basis impact: none.
- Limitations: restricted class of programs; assumes fairness for the
  "eventually".
- Evidence location: RESEARCH-FORMALISMS comparison table.

## 18. Paul, Agha, Patterson, Varela — Failure-Aware Actor Model / Synod (NFM 2021 / ISSE 2023)

- Bibliography: Saswata Paul, Gul A. Agha, Stacy Patterson, Carlos A.
  Varela. NFM 2021, LNCS 12673, doi:10.1007/978-3-030-76384-8_16,
  arXiv:2103.14576; journal extension ISSE 2023,
  doi:10.1007/s11334-022-00463-5.
- Source tier: 1.
- Read status: abstract (arXiv, inspected 2026-08-09).
- Inclusion: capability (formal failure modeling).
- Claims: failure-aware actor model (FAM) extends the actor model with agent
  failure reasoning; sufficient conditions for eventual consensus in Synod;
  machine-checked in Athena.
- Bombay classification: failure detection/failure awareness is modeled at
  the system/configuration level = INTERPRETER boundary; no behavior-fold
  primitive.
- Candidate-basis impact: none; confirms failure-detection row disposition.
- Limitations: abstract-level.
- Evidence location: capability-matrix.json row failure-detection.

## 19. Clinger — Foundations of Actor Semantics (MIT AI-TR-633, 1981)

- Bibliography: William D. Clinger. PhD dissertation, MIT AI Lab, AI-TR-633,
  May 1981.
- Stable URL: <https://dspace.mit.edu/bitstream/handle/1721.1/6935/AITR-633.pdf?sequence=2&isAllowed=y>
- Source tier: 1.
- Read status: partial — the PDF is a pure image scan without a text layer;
  front matter (TOC, introduction, fairness chapter opening) and the key
  sections were OCRed on 2026-08-09 (poppler+tesseract via nix): §II.1 The
  Actor Model, §III fairness/unbounded nondeterminism, §IV.2 Actor
  Behaviors, §V.1-2 Actor Acquaintances/Creation, Chapter VI Conclusion.
- Inclusion: semantic.
- Primitive syntax/operations: event = message arrival at an actor ("all
  events in the model are arrival events, and there is no such thing as a
  sending event"); message passing resembles mail service — messages may
  always be sent but are subject to variable delays. **Actor behavior
  domain: the reflexive equation F ≅ [M → (F × P(A × M))]** — a behavior is
  a function from one message to a next behavior and a set of
  (address, message) deliveries; the direct denotational analogue of
  Bombay's `(behavior, one communication) → (next behavior, sends)` fold.
  Actor = script + vector of acquaintances; the acquaintance vector is
  alterable only by the actor itself and may change over time. Actor
  creation evaluates a behavior abstraction, gathering identifier bindings
  into the new actor's acquaintance vector.
- Operational assumptions: fairness as finite-but-unbounded delay;
  **fairness implies unbounded nondeterminism** (formalized folk wisdom,
  CSP counterexample analysis); power domains over incomplete domains
  handle fairness denotationally.
- Laws/theorems: ordering laws (global time is necessary; strong/weak
  realizability axioms; a strong independence result); locality laws —
  actor acquaintances (§V.1), actor creation (§V.2), "locality laws add
  power" (§V.3), semantics with actor creation (§V.4); primitive
  serializers as the semantic actors.
- Capability claims: iterative vs recursive programs distinguished by
  whether new actors are created; whether a complex actor system can be
  regarded as a single actor left open (later closed by AMST 1997
  configuration composition and Talcott's component algebras).
- Bombay classification: LAW (arrival-event model, mail-service asynchrony,
  behavior-as-function domain equation, acquaintance locality, fairness as
  execution property); the domain-theoretic machinery itself does not
  transfer — Bombay realizes the same transition law as a typed pure fold.
- Candidate-basis impact: the F ≅ [M → (F × P(A × M))] equation is the
  earliest precise mathematical statement of the nucleus Bombay realizes;
  confirms the fold shape (one message in; next behavior plus finite send
  set out) is the 1981 denotational core.
- Limitations: OCR of selected sections only; no equivalence notion was
  defined (per AMST 1997 §1.3.1 and the Conclusion's own open questions).
- Evidence location: evidence.json RESEARCH-AGHA; REPORT.md foundational
  comparison table (Clinger row upgraded from access-limited).

## 20. Sirjani, Movaghar, Shali, de Boer — Modeling and Verification of Reactive Systems using Rebeca (FI 2004)

- Bibliography: Marjan Sirjani, Ali Movaghar, Amin Shali, Frank S. de Boer.
  Fundamenta Informaticae 63(4):385–410, 2004.
- Stable URL: CiteSeerX PDF via the Rebeca project publications page
  <https://rebeca-lang.org/publications.html>
  (citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.107.2074).
- Source tier: 1.
- Read status: complete text layer extracted 2026-08-09 (pdftotext via nix);
  model definition (§3), syntax, and verification approach read.
- Inclusion: semantic (formal actor language for verification).
- Primitive syntax/operations: rebecs (reactive objects) instantiated from
  reactive classes; unbounded FIFO inbox; each message names a unique
  message server invoked atomically on service (run-to-completion, no
  interleaving, no suspending receive); known rebecs = statically declared
  acquaintances; caller implicitly passes `self`; creation by class
  instantiation; `initial` message server starts each rebec; models are
  closed systems; components = user-decomposed open sub-models.
- Operational assumptions: coarse-grained atomic method execution chosen to
  match asynchronous communication and to reduce model-checking state
  space; typed variables; rebec identifiers passable but not assignable.
- Laws/theorems: compositional verification — component abstractions
  preserve a set of temporal-logic behavioral specifications; soundness
  proved via a weak simulation relation between constructs.
- Capability claims: verification-oriented actor subset; dynamic topology
  limited to creation; no mobility/location features.
- Bombay classification: macro-step (isolated-turn) execution = LAW
  confirmation; known rebecs = static acquaintance declaration
  (Bombay: typed Recipient topology); model checking/abstraction =
  verification tooling around the fold (testkit/interpreter concern), not
  new primitives; closed-world assumption contrasts with Bombay's open
  receptionist model — recorded as a non-transferable assumption for
  equivalence results.
- Candidate-basis impact: none; an independently designed typed actor
  language whose per-message atomic fold matches the nucleus.
- Limitations: closed models; compositional-verification results depend on
  the user-defined decomposition; no fairness theorem in the extracted
  sections.
- Evidence location: REPORT.md post-2000 comparison table (Rebeca row
  upgraded from record-level).

## 21. Greif — Semantics of Communicating Parallel Processes (MIT MAC-TR-154, 1975)

- Bibliography: Irene Greif. PhD dissertation, MIT Project MAC, MAC-TR-154,
  September 1975 (also DTIC ADA016302).
- Stable URL: MIT DSpace handle <https://dspace.mit.edu/handle/1721.1/57710>
  (bitstream downloaded 2026-08-09).
- Source tier: 1.
- Read status: substantial — the scan carries an OCR text layer (305 KB
  extracted); abstract, model introduction, and actor-model sections read.
- Inclusion: semantic.
- Primitive syntax/operations: a process is a totally ordered set of events;
  events represent the receiving of a message by an actor (arrival events);
  system behavior = a partial order properly containing the union of the
  processes' total orders; synchronization primitives and side-effect
  primitives both induce the system ordering.
- Operational assumptions: **no global clock** — specifications in terms of
  an external global time ordering are rejected as possibly unrealizable;
  only orderings derivable within the system count.
- Laws/theorems: synchronization properties = guarantees that events can be
  so partially ordered; a specification language whose program meanings are
  behavior specifications of the compiled system.
- Capability claims: applies equally to busy-waiting (shared cells) and
  non-busy-waiting (semaphores, structured primitives) synchronization.
- Bombay classification: LAW-level foundation for the event-ordering view
  (per-actor total order of arrivals + causal partial order across actors);
  the cross-actor ordering is observational/interpreter-level, consistent
  with Bombay imposing no cross-lane send order; the no-global-clock
  result supports keeping clocks out of the pure fold (timers are typed
  inputs).
- Candidate-basis impact: none; the earliest formal statement of the
  event-order semantics that Clinger 1981 and AMST 1997 build on.
- Limitations: OCR quality is imperfect (scan artifacts); the 1975 actor
  model predates the become/create crisp formulation of Agha 1986, which
  remains authoritative for the transition law.
- Evidence location: REPORT.md foundational comparison table (Greif row
  upgraded from access-limited).
