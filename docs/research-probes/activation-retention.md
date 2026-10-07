# Activation request retention mutation

The passing proxy custody regression retains the original `BeginActivation`
in `ProxyEffects::worker_activations` before calling the application Started
conversion. The existing callback can panic. Once the callback panics and its
borrowed producer is dropped, that original request must remain available,
with its permit, target, initialization and original Plan allocation.

The normal test target contains the two passing comparisons:
`started_conversion_keeps_original_pending_activation_in_named_rows` and
`named_activation_retention_preserves_normal_admission_and_ready_reply`.
Their bodies, assertions, fixtures and lawful producer are unchanged.
The original deliberately failing duplicate and its single-use producer remain
in Git history at commit `6f7e966c0df9dc5b0c44617ad2be1fdc3e09f3bb`.
They are negative research evidence, not an expected passing CI test.

`activation-retention.patch` changes only the existing test producer's order:
Started conversion occurs before the complete original request enters its named
retained row. It adds no actor law, fixture, type, feature or production change.
Apply it only to a disposable source copy after authenticating the healthy file.
Run the retained custody test exactly in debug and optimized builds through
Bombay's pinned Nix shell, then restore the whole healthy source and repeat both
passing comparisons. The mutated source never belongs in the normal CI cohort.

The historical comparison receipt
`733ab1881158c752bb781c460649a4c3e26e97e6b557e4405af51073b807c542`
ran original and finite order inversions in both profiles. Each selected exactly
one test and failed at the first `worker_activations.len()` oracle: actual zero,
expected one, after the original native panic payload and producer were disposed.
The original site was line3156 and the finite retained-test site was line2994
in that historical source. Later assertions did not execute on those failures.
The overall historical collector remained NONPASS because of an inherited
formatting failure. These results carry only ordinary pre-spawn admission
composition evidence; they do not prove actual runtime InjectEvent, task joining,
FIFO, every template, generic producer closure or an owning release gate.

For this source epoch, require the exact retained test, one selected/one failed,
and that same post-disposal zero-versus-one row assertion. A compiler error,
zero tests, an earlier arbitrary panic or a different assertion is not mutation
credit. Current line numbers and actual source must be captured from the run.
Current focused receipt `c4f417d9` and independent review `9de7c338` qualify
both healthy controls, both intended post-disposal row-count failures at2962:5
and whole806-source restorations in debug and optimized builds. The complete
56-test target then passes in both profiles under receipt `57897d1b`, with no
ignored or filtered tests. Full owning repository, package and release checks
remain required; these focused results do not establish runtime task ownership.
