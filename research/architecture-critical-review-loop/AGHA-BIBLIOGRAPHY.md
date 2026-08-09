# Gul Agha bibliography: OSL and DBLP disposition

Status: complete disposition of the Open Systems Laboratory publication
catalogue (419 entries, 1983–2022, fetched 2026-08-09 from
<https://osl.cs.illinois.edu/publications/>) and reconciliation against the
DBLP record for Gul A. Agha (<https://dblp.org/pid/a/GulAAgha>).

Dispositions: `included-semantic` (a formal claim that can affect the
candidate primitive basis), `included-capability` (a capability or
interpreter/platform claim informing the capability matrix),
`included-framework-comparison` (empirical/comparative actor-framework
evidence), `excluded` (with a specific reason).

Scope rule: the OSL catalogue lists all OSL members' publications. Entries
without Agha as author are excluded from the Agha bibliography by rule; where
such an entry matters to the capability survey (e.g. Rebeca model checking) it
is dispositioned under the capability line, not the Agha line.

## Included — semantic

1. Agha, Mason, Smith, Talcott. "A Foundation for Actor Computation." JFP
   7(1):1–72, 1997. doi:10.1017/S095679689700261X. [OSL #354]
   Lambda-calculus foundation; primitive syntax `send`, `newactor`, `ready`,
   `letrec`/`rho`; labeled transition semantics over configurations; fairness;
   testing equivalence. Governs the nucleus.
2. Agha, Mason, Smith, Talcott. "Towards a Theory of Actor Computation."
   CONCUR 1992, LNCS 630:565–579. [OSL #394] Precursor of AMST 1997; same
   primitive basis, earlier formulation.
3. Agha. *ACTORS: A Model of Concurrent Computation in Distributed Systems.*
   MIT Press, 1986 (OSL lists the 1990 MIT Press AI series printing). [OSL
   #403] Task-based operational semantics; one communication at a time;
   send/create/become effect structure; fresh mail addresses; acquaintances.
4. Agha. "The Structure and Semantics of Actor Languages." REX Workshop 1990,
   LNCS 489:1–59. [OSL #405] Actor-language constructs as derived syntax over
   the semantic basis; directly supports derived-form classification.
5. Agha. "Concurrent Object-Oriented Programming." CACM 33(9):125–141, 1990.
   doi:10.1145/83880.84585. [OSL #406] Language-level encapsulation, behavior
   replacement, coordination; separates model law from language design.
6. Agha. "Semantic Considerations in the Actor Paradigm of Concurrent
   Computation." LNCS 197:151–179, 1984. [OSL #418] Early semantic statement;
   orders, arrival nondeterminism.
7. Hewitt, Reinhardt, Agha, Attardi. "Linguistic Support of Receptionists for
   Shared Resources." LNCS 197:330–359, 1984. [OSL #417] Receptionists define
   the open-system interface; basis for open-configuration composition.
8. Agha, Hewitt. "Concurrent Programming Using Actors: Exploiting Large-Scale
   Parallelism." FSTTCS 1985, LNCS 206:19–41. [OSL #415] Early model
   statement; replacement behavior; buffering.
9. Agha, Hewitt. "Actors: A Conceptual Foundation for Concurrent
   Object-Oriented Programming." In *Research Directions in Object-Oriented
   Programming*, MIT Press, 49–74, 1987. [OSL #413]
10. Agha, Hewitt. "Actor Formalisms." Encyclopedia of Artificial Intelligence,
    Addison Wesley, 1987. [OSL #414] Summary; no new primitives.
11. Agha. "Foundational Issues in Concurrent Computing." SIGPLAN Notices
    24(4):60–65, 1989. [OSL #410] Position on foundations; motivates the
    later algebraic program.
12. Agha. "Formal Methods for Actor Systems: A Progress Report." FORTE 1992,
    IFIP C-10:217–228. [OSL #397] Survey of the formalization program;
    identifies fairness and equivalence obligations.
13. Agha. "Abstracting Interaction Patterns: A Programming Paradigm for Open
    Distributed Systems." FMOODS 1997, pp. 135–153. [OSL #355] Open-system
    composition; interaction patterns as language-level abstractions over
    message passing.
14. Agha, Kim. "Actors: A Unifying Model for Parallel and Distributed
    Computing." JSA 45(15):1263–1277, 1999. doi:10.1016/S1383-7621(98)00067-8.
    [OSL #333; duplicate #335] Survey separating semantic model from runtime
    realization.
15. Agha, Thati, Ziaei. "Actors: A Model for Reasoning About Open Distributed
    Systems." In *Formal Methods for Distributed Processing*, CUP, 2001. [OSL
    #320, #321] Open configurations, receptionist interfaces, locality,
    observational reasoning.
16. Thati, Ziaei, Agha. "A Theory of May Testing for Actors." FMOODS 2002,
    IFIP 209:147–162. [OSL #302] May-testing preorder for actors; trace
    characterization; compositionality.
17. Thati, Ziaei, Agha. "A Theory of May Testing for Asynchronous Calculi with
    Locality and No Name Matching." AMAST 2002, LNCS 2422:223–238.
    doi:10.1007/3-540-45719-4_16. [OSL #301] Locality and no-name-matching
    restrictions; complete axiomatization of the finitary fragment.
18. Thati. *A Theory of Testing for Asynchronous Concurrent Systems.* PhD
    dissertation, UIUC, 2003. [OSL #279] Full development of the may-testing
    theory behind #16–#17.
19. Thati. "Towards an Algebraic Formulation of Actors." MS thesis, UIUC,
    2001. [OSL #317] Precursor of the 2004 algebraic theory.
20. Agha, Thati. "An Algebraic Theory of Actors and Its Application to a
    Simple Object-Based Language." LNCS 2635:26–57, 2004.
    doi:10.1007/978-3-540-39993-3_4. [OSL #271] Primitive actor terms,
    algebraic operators, transition rules, derived object constructs.
21. Agha, Thati. "Actors." Encyclopedia of Distributed Computing, Kluwer,
    2002. [OSL #310] Authoritative summary; no new primitives.
22. Karmani, Agha. "Actors." Encyclopedia of Parallel Computing, Springer,
    2011, pp. 1–11. doi:10.1007/978-0-387-09766-4_125. [OSL #81] Later
    Agha-coauthored semantic summary; confirms the send/create/become basis
    without revision.
23. Agha, Meseguer, Sen. "PMaude: Rewrite-Based Specification Language for
    Probabilistic Object Systems." QAPL 2005; ENTCS 153(2):213–239, 2006.
    [OSL #224, #186] Rewriting-logic actor configurations; probability as
    model-level observation, not a behavior primitive.
24. Kumar, Sen, Meseguer, Agha. "A Rewriting Based Model for Probabilistic
    Distributed Object Systems." FMOODS 2003, LNCS 2884:32–46. [OSL #290]
    Precursor of PMaude.
25. Agha, Gunter, Greenwald, Khanna, Meseguer, Sen, Thati. "Formal Modeling
    and Analysis of DoS Using Probabilistic Rewrite Theories." FCS 2005.
    [OSL #225] Application of #23 to DoS; no new actor primitive.
26. Plyukhin, Agha. "Scalable Termination Detection for Distributed Actor
    Systems." CONCUR 2020, LIPIcs 171:11:1–11:23. [OSL #4] Formal actor
    termination detection; safety/liveness; interpreter-owned.
27. Plyukhin, Agha. "A Scalable Algorithm for Decentralized Actor Termination
    Detection." LMCS, 2022. [OSL #3] Journal extension of #26.
28. Charalambides, Dinges, Agha. "Parameterized, Concurrent Session Types for
    Asynchronous Multi-Actor Interactions." SCP 115-116:100–126, 2016. [OSL
    #33; journal version of #69, #36] Multiparty session types over actor
    addressing; the primary formal paper governing the protocol-session
    capability row.

## Included — capability

29. Agha, Callsen. "ActorSpaces: An Open Distributed Programming Paradigm."
    PPOPP 1993, pp. 23–32. [OSL #384] Pattern-based open coordination spaces.
30. Callsen, Agha. "Open Heterogeneous Computing in ActorSpace." JPDC
    21(3):289–300, 1994. [OSL #379] ActorSpace realization; group addressing.
31. Varela, Agha. "Programming Dynamically Reconfigurable Open Systems with
    SALSA." OOPSLA 2001, SIGPLAN Notices 36(12):20–34.
    doi:10.1145/583960.583964. [OSL #314] Universal naming, migration,
    reference cells; location transparency as a runtime service.
32. Varela, Agha. "A Hierarchical Model for Coordination of Concurrent
    Activities." COORDINATION 1999, LNCS 1594:166–182. [OSL #328]
33. Varela, Agha. "What After Java? From Objects to Actors." Computer Networks
    30(1-7):573–577, 1998. [OSL #338] Position; motivates SALSA line.
34. Agha, Jamali, Varela. "Agent Naming and Coordination: Actor Based Models
    and Infrastructures." In *Coordination of Internet Agents*, Springer,
    225–246, 2001. [OSL #322]
35. Jamali, Thati, Agha. "An Actor-Based Architecture for Customizing and
    Controlling Agent Ensembles." IEEE Intelligent Systems 14(2), 1999. [OSL
    #330]
36. Frølund, Agha. "A Language Framework for Multi-Object Coordination."
    ECOOP 1993, LNCS 707:346–360. [OSL #381] Synchronizers; declarative
    multi-object coordination constraints.
37. Frølund, Agha. "Abstracting Interactions Based on Message Sets." ECOOP
    Workshop 1994; LNCS 924:107–124, 1996. [OSL #378, #363] Message-set
    interfaces; pattern-based receive.
38. Agha, Frølund, Kim, Panwar, Patterson, Sturman. "Abstraction and
    Modularity Mechanisms for Concurrent Computing." IEEE P&DT 1(2):3–14,
    1993; extended 1995. [OSL #382, #374]
39. Agha, Frølund, Panwar, Sturman. "A Linguistic Framework for Dynamic
    Composition of Dependability Protocols." DCCA-3 1992/1993. [OSL #396,
    #383] Dependability protocol composition; fault-tolerance vocabulary.
40. Agha, Sturman. "A Methodology for Adapting to Patterns of Faults."
    *Foundations of Dependable Computing*, Kluwer, 1994. [OSL #380]
41. Sturman, Agha. "A Protocol Description Language for Customizing
    Semantics." SRDS 1994, pp. 148–157. [OSL #375]
42. Sturman. *Modular Specification of Interaction Policies in Distributed
    Computing.* PhD thesis, UIUC, 1996. [OSL #356]
43. Ren, Agha. "RTSynchronizer: Language Support for Real-Time Specifications
    in Distributed Systems." LCT-RTS 1995, pp. 50–59. [OSL #368]
44. Ren, Agha, Saito. "A Modular Approach to Programming Distributed Real-Time
    Systems." JPDC 36(1):4–12, 1996. [OSL #358]
45. Saito, Agha. "A Modular Approach to Real-Time Synchronization." OOPS
    Messenger 7(1):13–20, 1996. [OSL #357]
46. Ren. *An Actor-Based Framework for Real-Time Coordination.* PhD thesis,
    UIUC, 1997. [OSL #351]
47. Nielsen, Agha. "Semantics for an Actor-Based Real-Time Language." WPDRTS
    1996, pp. 223–228. [OSL #361]
48. Nielsen, Ren, Agha. "Specification of Real-Time Interaction Constraints."
    ISORC 1998, pp. 206–214. [OSL #342]
49. Nielsen, Agha. "Towards Reusable Real-Time Objects." Ann. Software Eng.
    7:257–282, 1999. [OSL #329]
50. Ren, Agha. "A Modular Approach for Programming Embedded Systems." LNCS
    1494:170–207, 1998 (also 1996 EEF). [OSL #341, #359]
51. Kim, Agha. "Efficient Support of Location Transparency in Concurrent
    Object-Oriented Programming Languages." SC 1995, p. 39. [OSL #371]
    Location transparency as a runtime naming service — evidence for the
    interpreter classification of the location-transparency row.
52. Kim. *Thal: An Actor System for Efficient and Scalable Concurrent
    Computing.* PhD thesis, UIUC, 1997. [OSL #352] Actor runtime architecture.
53. Kim, Panwar, Agha. "Efficient Compilation of Call/Return Communication for
    Actor-Based Programming Languages." HiPC 1996, pp. 62–67. [OSL #362]
    Call/return is a derived pattern compiled onto asynchronous send —
    evidence that request-reply is derived, not primitive.
54. Houck, Agha. "HAL: A High-Level Actor Language and Its Distributed
    Implementation." ICPP 1992. [OSL #391]
55. Agha, Houck, Panwar. "Distributed Execution of Actor Programs." LCPC 1991,
    LNCS 589:1–17 (also WPC 1992). [OSL #401, #395] Interpreter work.
56. Tomlinson, Kim, Scheevel, Singh, Will, Agha. "Rosette: An Object-Oriented
    Concurrent Systems Architecture." SIGPLAN Notices 24(4):91–93, 1989. [OSL
    #407] Actor runtime architecture.
57. Agha. "Supporting Multiparadigm Programming on Actor Architectures."
    PARLE 1989, LNCS 366:1–19. [OSL #408]
58. Venkatasubramanian, Agha, Talcott. "Scalable Distributed Garbage
    Collection for Systems of Active Objects." IWMM 1992, LNCS 637:134–147.
    [OSL #386] Actor GC as interpreter infrastructure.
59. Vardhan, Agha. "Using Passive Object Garbage Collection Algorithms for
    Garbage Collection of Active Objects." MSP/ISMM 2002, pp. 213–220. [OSL
    #298]
60. Plyukhin, Agha. "Concurrent Garbage Collection in the Actor Model." AGERE
    2018, pp. 44–53. [OSL #8]
61. Agha, Ziaei. "Security and Fault-Tolerance in Distributed Systems: An
    Actor Based Approach." In *Computer Security, Dependability, and
    Assurance*, IEEE CS, 72–88, 1999. [OSL #334]
62. Milojicic, Agha, Bernadat, Chauhan, Guday, Jamali, Lambright, Travostino.
    "Case Studies in Security and Resource Management for Mobile Object
    Systems." AAMAS 5(1):45–79, 2002 (also ECOOPW 1998). [OSL #307, #343]
63. Agha, Jamali. "Concurrent Programming for Distributed Artificial
    Intelligence." In *Multiagent Systems*, MIT Press, chap. 12, 505–534,
    1999. [OSL #336]
64. Venkatasubramanian, Talcott, Agha. "A Formal Model for Reasoning About
    Adaptive QoS-Enabled Middleware." FME 2001, LNCS 2021:197–221; TOSEM
    13(1):86–147, 2004. [OSL #313, #228] Rewriting-logic middleware model.
65. Thati, Talcott, Agha. "Techniques for Executing and Reasoning About
    Specification Diagrams." AMAST 2004, LNCS 3116:521–536. [OSL #241]
66. Dinges, Agha. "Scoped Synchronization Constraints for Large Scale Actor
    Systems." COORDINATION 2012, pp. 89–103. [OSL #68] Continuation of the
    synchronizer line.
67. Lauterburg, Dotta, Marinov, Agha. "A Framework for State-Space Exploration
    of Java-Based Actor Programs." ASE 2009, pp. 468–479. [OSL #122]
68. Lauterburg, Karmani, Marinov, Agha. "Basset: A Tool for Systematic Testing
    of Actor Programs." FSE 2010, pp. 363–364. [OSL #99]
69. Lauterburg, Karmani, Marinov, Agha. "Evaluating Ordering Heuristics for
    Dynamic Partial-Order Reduction Techniques." FASE 2010, LNCS 6013:308–322.
    [OSL #98]
70. Tasharofi, Karmani, Lauterburg, Legay, Marinov, Agha. "TransDPOR: A Novel
    Dynamic Partial-Order Reduction Technique for Testing Actor Programs."
    FMOODS/FORTE 2012, LNCS 7273:219–234. [OSL #61]
71. Jagannath, Gligoric, Lauterburg, Marinov, Agha. "Mutation Operators for
    Actor Systems." ICST Workshops 2010, pp. 157–162. [OSL #110]
72. Li, Hariri, Agha. "Targeted Test Generation for Actor Systems." ECOOP
    2018, LIPIcs 109:8:1–8:31. [OSL #9]
73. Dinges, Agha. "Targeted Test Input Generation Using Symbolic-Concrete
    Backward Execution." ASE 2014 (also UIUC TR). [OSL #42, #43]
74. Dinges, Charalambides, Agha. "Automated Inference of Atomic Sets for Safe
    Concurrent Execution." PASTE 2013 (also UIUC TR). [OSL #56, #51]
75. Negara, Karmani, Agha. "Inferring Ownership Transfer for Efficient Message
    Passing." PPOPP 2011, pp. 81–90. [OSL #74] Ownership inference; message
    representation is interpreter work.
76. Khamespanah, Mechitov, Sirjani, Agha. "Schedulability Analysis of
    Distributed Real-Time Sensor Network Applications Using Actor-Based Model
    Checking." SPIN 2016, LNCS 9641:165–181. [OSL #31] Timed Rebeca line.
77. Khamespanah, Sirjani, Mechitov, Agha. "Modeling and Analyzing Real-Time
    Wireless Sensor and Actuator Networks Using Actors and Model Checking."
    STTT 20(5):547–561, 2018. [OSL #10]
78. Sirjani, Khamespanah, Mechitov, Agha. "A Compositional Approach for
    Modeling and Timing Analysis of Wireless Sensor and Actor Networks."
    SIGBED Review 14(3):49–56, 2017. [OSL #20]
79. Kwon, Mechitov, Agha. "Design and Implementation of a Mobile Actor
    Platform for Wireless Sensor Networks." LNCS 8665:276–316, 2014. [OSL
    #48] ActorNet; mobile actor platform.
80. Kwon, Sundresh, Mechitov, Kim, Agha. "ActorNet: An Actor Platform for
    Wireless Sensor Networks." AAMAS 2006, pp. 1297–1300 (also UIUC TR 2005).
    [OSL #177, #209]
81. Agha, Palmskog. "Transforming Threads into Actors: Learning Concurrency
    Structure from Execution Traces." In *Principles of Modeling*, LNCS
    10760:16–37, 2018. [OSL #16]
82. Agha, Panwar. "An Actor-Based Framework for Heterogeneous Computing
    Systems." Workshop on Heterogeneous Processing 1992. [OSL #393]
83. Panwar, Kim, Agha. "Parallel Implementations of Irregular Problems Using
    High-Level Actor Language." IPPS 1996, pp. 857–862. [OSL #360]
84. Agha, Kim, Panwar. "Actor Languages for Specification of Parallel
    Computations." DIMACS vol. 18:239–258, 1995. [OSL #373]
85. Kim, Agha. "Parallel Programming and Complexity Analysis Using Actors."
    MPPM 1997/1998. [OSL #344]
86. Thati, Chang, Agha. "Crawlets: Agents for High Performance Web Search
    Engines." Mobile Agents 2001, LNCS 2240:119–134. [OSL #316] Mobile
    agents; actor-based mobility.
87. Ding, Zheng, Sha, Agha. "Specification and Validation of Fault-Tolerant
    Software Architectures Based on Actor Model." SEKE 2003, pp. 458–466.
    [OSL #294]
88. Ding, Zheng, Agha, Sha. "Automated Verification of the Dependability of
    Object-Oriented Real-Time Systems." WORDS 2003, pp. 171–178. [OSL #295]

## Included — framework comparison

89. Karmani, Shali, Agha. "Actor Frameworks for the JVM Platform: A
    Comparative Analysis." PPPJ 2009, pp. 11–20. [OSL #125] Empirical
    comparison; no semantic authority.
90. Tasharofi, Dinges, Johnson. "Why Do Scala Developers Mix the Actor Model
    with Other Concurrency Models?" ECOOP 2013, LNCS 7920:302–326. [OSL #52]
    Empirical; Agha is not an author but OSL-line; used only for terminology.

## Excluded (with reasons)

Group E1 — edited volumes, prefaces, panels, keynotes, track introductions (no
technical content to disposition): OSL #35 (QEST 2016 ed.), #50 (Yonezawa
festschrift ed.), #57 (COORDINATION preface), #72/#73 (AGERE workshop
announcements), #88 (Talcott festschrift ed.), #116 (COORDINATION 2010 ed.),
#143 (MMAS ed.), #152/#187/#226/#272 (HICSS track introductions), #324
(COOPN ed.), #345 (TAPOS editorial), #353 (ICSE workshop), #400/#404
(OOPSLA panels), #402/#409 (OBCP workshop proceedings), #311 (CACM
introduction), #142 (CACM vision piece), #158 (biosensors workshop talk), #21
(SEKE 2017 keynote page), #34 (SEFM 2016 extended abstract), #49 (ISPDC 2014
keynote), #323 (CCGRID 2001 keynote), #326 (Euro-Par 2000 abstract), #367
(CSUR 2-page paradigms note).

Group E2 — not actor-related research (different domain): civil-infrastructure
sensor networks and structural health monitoring (#6, #11, #19, #22–#26,
#28–#30, #32, #37, #39–#41, #46, #47, #53, #54, #62–#67, #71, #76, #82, #91–
#94, #109, #115, #133–#135, #149, #150, #175, #176, #203, #210, #217, #251,
#253, #255, #280, #288, #289, #291, #297), economics (#419), indoor
positioning (#39–#41), video/multimedia retrieval (#308, #312, #318, #331),
document clustering (#252, #287, #305, #306), web services/mining (#227,
#257, #286), storage-systems QoS and autonomic storage (#188, #191–#193,
#231–#233, #285, #299, #300), HVAC performance prediction (#339), air-
conditioning/refrigeration (#369), planar linkages education (#369 group),
gesture UIs (#370).

Group E3 — concurrency verification/testing methods not specific to actor
semantics (used only as background for TEST-* obligations, not for primitive
claims): #17 (statistical model checking survey), #42/#43 dispositioned as
capability above, #44 (polytope path conditions), #59 (Euclidean model
checking), #77 (DTMC evolution), #83/#84 (multithreaded unit testing), #87
(MDP invariants), #102 (MDP transformers), #103–#106, #123 (energy/parallel
algorithms), #78/#79 (energy complexity), #85 (geometry synthesis), #90
(test-data generation), #107/#108/#80 (thread contracts), #111–#113 (mutation
testing), #118 (DART), #126/#128 (test repair), #129 (collective patterns),
#124 (barrier pattern), #144 (learning branching-time), #145 (instrumentation
technique), #148 (narrowing crypto protocols), #151 (Markov reward), #159
(Vardhan thesis — general learning-to-verify), #160–#163, #194–#200, #234–
#239 (Tosic cellular automata/coalition complexity), #164 (narrowing), #165
(Markov-chain uncertainties), #166 (async atomic methods), #167–#172 (Sen
runtime analysis), #173 (Sen thesis), #189/#190 (learning to verify), #201
(MTL monitoring), #202 (narrowing), #204–#207 (statistical model checking,
CUTE), #208 (narrowing crypto), #211 (iLTL checker), #215 (DART), #216
(natural narrowing), #222 (test generation + runtime verification), #229/#230
(learning safety), #240 (unbounded buffers verification), #243–#248 (Sen
verification line), #249 (task swapping), #250 (intrusion detection), #256
(iLTL), #265 (natural rewriting), #268/#269 (Eagle monitoring), #281–#284
(Sen/Rosu monitoring), #303 (pi-calculus may testing in Maude — noted as
adjacent formal work, not actor), #337 (Venkatasubramanian PhD — resource
management; superseded by #64), #340 (Vardhan MS — superseded by #59), #144
branching-time learning.

Group E4 — multi-agent/market/coordination applications without actor-model
semantic content: #95–#97, #119–#121, #139–#141, #154, #182, #180/#181,
#213, #214, #223, #258–#264, #276–#278, #292, #293, #296 (reinforcement
learning teamwork), #309 (Ahmed MSc), #220 (Brown thesis — crash-failure
coordination; reviewed: algorithmic, no actor-calculus content), #184
(structured overlays), #185/#266/#267 (adaptive web objects), #156/#157
(context-aware web), #132 (ActorNet fault-tolerance poster), #138 (WSN
debugging poster), #136/#137/#178/#179 (Kwon localization/probabilistic
modeling; #179 PhD reviewed: verification methods, no actor primitive),
#117/#131 (Sundresh request-based execution), #130 (AJ system), #155/#183
(Donkervoet reflection/monitoring), #153 (market-based reallocation), #60
(congestion pricing DoS), #89 (misbehavior detection VANETs), #221 (knowledge
management metaphor), #270 (worldwide computing middleware chapter), #274
(Thal++), #275 (Eos policy), #299/#300 (StorageAgent), #304 (thin
middleware), #319 (customizable middleware CACM), #327 (QoS metaobject
framework), #332 (Astley PhD middleware), #346/#347/#348 (middleware
composition), #349 (multimedia QoS), #350 (RTsynchronizer infeasibility
detection), #364/#365 (visualization), #372 (visualization), #377/#376
(methodology/fault adaptation MS), #388 (visualization), #389 (compilation
LCPC), #390 (reflective inheritance — OO, not actor transition), #392
(Frølund synchronization-constraint inheritance — noted under capability
line as related work; not Agha-authored), #398/#399 (parallel sorting), #411
(guarded horn clauses — logic programming), #412 (distributed databases),
#416 (IEEE Database Eng — database application), #385 (scalable concurrent
computing survey — subsumed by #14).

## DBLP reconciliation

DBLP <https://dblp.org/pid/a/GulAAgha> was fetched 2026-08-09 (HTML index plus
the complete XML export, 244 records, 1984–2026) and reconciled against the
OSL disposition above.

DBLP-only actor-relevant records absent from the OSL catalogue:

- Agha. "An Overview of Actor Languages." OOPWORK 1986. — `included-semantic`
  (language-overview summary; no new primitives beyond the 1986 book).
- Agha. "Fair Concurrency in Actors (abstract only): eager evaluation
  producers strong convergence." OOPWORK 1986. — `included-semantic` (early
  fairness note; abstract only, superseded by Clinger 1981 and AMST 1997
  fairness treatments; read status: abstract-only).
- Charalambides, Palmskog, Agha. "Types for Progress in Actor Programs." In
  *Models, Languages, and Tools for Concurrent and Distributed Programming*
  (festschrift), LNCS 11765, 2019. doi:10.1007/978-3-030-21485-2_18. —
  `included-semantic` (type system giving progress for actor programs; the
  post-2016 continuation of the typed-actor line begun by the 2012/2016
  session-type work).
- Paul, Agha, Patterson, Varela. "Verification of Eventual Consensus in Synod
  Using a Failure-Aware Actor Model." NFM 2021, LNCS 12673;
  doi:10.1007/978-3-030-76384-8_16; journal extension *Innov. Syst. Softw.
  Eng.* 2023, doi:10.1007/s11334-022-00463-5. — `included-capability`
  (failure-aware actor model used for verified consensus; failure detection
  is modeled at the system level, supporting the interpreter classification
  of the failure-detection row).
- Plyukhin, Agha, Montesi. "CRGC: Fault-Recovering Actor Garbage Collection in
  Pekko." Proc. ACM Program. Lang., 2025. doi:10.1145/3729288. —
  `included-capability` (actor GC under failures; interpreter lifecycle
  infrastructure; Pekko appears as the implementation vehicle, not as
  semantic authority).

DBLP-only excluded records (absent from OSL): quantum computing (Kwon-Agha
QCE 2023, Quantum Inf. Process. 2026, Hamiltonian automaton 2026), energy/SoC
measurement (CRAVE EuroSys 2025 plus its Zenodo artifacts, eScope CoRR 2024),
near-data processing systems (Jarvis ICDE 2022 + CoRR, streaming analytics
WWW 2022, UCC 2022 — already excluded in E2/E4 spirit), NII Shonan meeting
report 2019 (meeting report, no technical content), IEEE P&DT editorials
1995–1996 ("The mountains are in labor", "The elusive goal of intelligence",
"Our magazine's new face", "Strategic directions", "Different approaches
around the world" — magazine editorials), OOPSLA Addendum workshop summaries
(1997 dependable distributed object systems), and CoRR preprints that
duplicate published records above.

Reconciliation outcomes:

- DBLP indexes 244 records versus OSL's 419 catalogue entries; the OSL list is
  the union superset for 1983–2022 because it includes technical reports,
  theses, and workshop papers DBLP omits, while DBLP adds 2019–2026 items the
  stale OSL page lacks. Every DBLP record from 1984–2018 corresponds to an
  OSL entry already dispositioned, except the additions listed above.
- Duplicates resolved in favor of the journal/publisher version with DOI
  (AMST 1997 JFP over CONCUR 1992 for citations of record; SCP 2016 over
  FOCLASA 2012; ISSE 2023 over NFM 2021; LMCS 2022 over CONCUR 2020).
- Name disambiguation: "Prasannaa Thati" and "Prasanna Thati" are one author
  (DBLP notes the aka); "AghaK99"/"jsa99" in OSL are the same JSA article;
  DBLP's 1988 Rosette entry and OSL's 1989 SIGPLAN Notices entry are the same
  workshop paper.
- No DBLP record post-2019 introduces a new actor semantic family. The
  formal post-2000 line resolves to exactly: open systems (2001), may testing
  (2002 ×2, Thati dissertation 2003), the algebraic theory (MS 2001 → LNCS
  2004), rewriting-logic object/actor specifications (2003, 2005, 2005),
  typed actors and progress (2012, 2016, 2019), testing/DPOR for actor
  programs (2009–2018, capability only), termination detection (2020/2022),
  and actor GC (2018, 2025).

## Uncertainty and access notes

- Paywalled items whose full text was not re-read in this campaign (CUP 2001
  chapter; some LNCS volumes) are dispositioned from author-hosted copies on
  the OSL media server where available, otherwise from the official abstract
  plus citing primary works; every semantic claim in REPORT.md cites a source
  whose full text or author-hosted PDF was inspected.
- The 1986 MIT Press book is catalogued by OSL under the 1990 printing; the
  1986 first edition is the citable original.
- No OSL/DBLP entry post-2018 introduces a new actor semantic family; the
  formal line after 2016 is termination detection (Plyukhin-Agha) and
  concurrent actor GC (Plyukhin-Agha 2018), both interpreter/lifecycle.
