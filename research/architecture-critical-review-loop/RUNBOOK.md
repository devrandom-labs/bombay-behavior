# Behavior architecture loop runbook

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
