# Session protocol derivation loop runbook

This is the independent Resource Pool validation run, not the architecture
critical-review loop and not a restart of the archived Supervised Worker run.
Its loop state, evidence IDs, and attempt numbers remain local to this
directory. See `research/README.md`.

## Start

Do not run `/loop prepare`; this directory is the prepared validation scaffold.
Commit the scaffold first. The loop must set `prepared_at_commit` to that
scaffold commit before its first experiment and must never advance the value.

```text
/loop goal Determine whether Bombay Behavior needs phase-indexed protocol typing beyond Fsm. Begin with one concrete protocol and invalid programs that must fail to compile, derive from the existing static algebra before proposing API, and retain production changes only if a failed public-API derivation proves a minimal reusable gap. --file research/session-protocol-derivation-loop/GOAL.md --max 100 --check "research/session-protocol-derivation-loop/check.sh" --check-timeout 1800 --until-done
```

Then:

```text
/loop run --model kimi-code/k3 --rescue-model kimi-code/k3-256k
```

Inspect commits, `PROGRESS.md`, the complete report, and checker output before
accepting the result.
