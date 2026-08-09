# Behavior architecture loop runbook

## Resume an interrupted schema-v2 run

Read `HANDOFF.md` before invoking the loop. The current campaign is partial and
must continue from the committed three-layer reclassification; do not restart
the survey or restore the superseded seven-item basis.

Use this continuation goal with any replacement model/profile:

```text
/loop goal Resume the partial Bombay behavior-calculus schema-v2 reclassification from research/architecture-critical-review-loop/HANDOFF.md. Preserve all committed research and probes; do not restart the literature survey. Recover or downgrade missing probe claims, separate actor-semantic, host-calculus, and Bombay-representation layers with independent semantic/representation/public statuses, migrate the checker and capability derivations, evaluate production representation adequacy without editing production code, and keep reopened obligations pending until all artifacts and the report agree. Process any unavoidable external source sequentially, never in parallel. Do not claim LOOP_DONE while HANDOFF.md lists an unresolved inconsistency or the checker fails. --file research/architecture-critical-review-loop/GOAL.md --max 200 --check "research/architecture-critical-review-loop/check.sh" --check-timeout 1800 --until-done
```

This runbook drives only the broad architecture audit. Do not insert concrete
session-protocol derivation attempts into this loop. If the audit needs that
evidence, consume the focused session campaign's report and record only the
resulting architecture classification here. See `research/README.md`.

## Start

```sh
cd /Users/joel/Code/devrandom/behaviorpass
omp --profile kimi
```

Inside OMP:

```text
/loop goal Complete the independent actor-behavior architecture, literature survey, and compositional capability closure exactly as specified in the prepared goal file. --file research/architecture-critical-review-loop/GOAL.md --max 200 --check "research/architecture-critical-review-loop/check.sh" --check-timeout 1800 --until-done
```

Confirm, then run:

```text
/loop status
/loop run --model kimi-code/k3 --rescue-model kimi-code/k3-256k
```

Do not run `/loop prepare`; the goal, matrix, baseline, and checker are already
curated. The 200-iteration cap pauses rather than discards progress. Resume only
after inspecting commits, `PROGRESS.md`, `/loop stats`, and checker output.

## Control and monitoring

```text
/loop status
/loop stats
/loop finish
/loop stop
/loop resume --model kimi-code/k3 --rescue-model kimi-code/k3-256k
/loop end
```

Outside OMP:

```sh
research/architecture-critical-review-loop/check.sh
git log --oneline --decorate -20
git status --short
```

The loop stops only when every evidence obligation and every capability row is
resolved, all ratchets hold, the primary-source research report accounts for
the audit, and
both `cargo nextest run --workspace` and `nix flake check` succeed.
