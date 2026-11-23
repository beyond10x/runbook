---
format: aep.planning-md/3
id: story:check-bindings-exits-reads-any-runbook
kind: story
status: active
title: check-bindings.sh --exits checks a runbook of another repository without this repository's pool list
summary: 'Consumer finding on main after the pin c9b4c177: --exits <root> over a runbook of another repository reports this repository''s five pool bindings as missing. The pool list holds only for this repository.'
relations:
- informed_by: story:bindings-state-their-exits
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T08:27:33Z", actor: "human:operator", revision: 2, executor: "agent:runbook", correlation: "host-operator-2026-11-23-d671"}
- {from: "proposed", to: "active", at: "2026-11-23T08:27:33Z", actor: "human:operator", revision: 3, executor: "agent:runbook", correlation: "host-operator-2026-11-23-d671"}
---
## Outcome

`scripts/check-bindings.sh --exits <root>` checks the bindings of a runbook that is not this repository and
reports only what is wrong in that runbook's own files.

## Source

The consumer session, 2026-11-23, on `main` at the merge of the Loom pin `c9b4c177`: run over a runbook
directory of its own, the option reports the five bindings of this repository that wait for the build pool
as missing ("a binding that waits for the build pool is not in bindings/"). That list is this repository's
(`scripts/check-bindings.sh`, the pool list near line 35); a runbook of another repository has none of those
bindings and must not be asked for them.

## Acceptance

- A root that holds program bindings with `exits.error` and none of this repository's pool bindings:
  `--exits <root>` exits 0 and prints no line about the build pool. One test, red before.
- The same root with one binding that lacks `exits.error`: one named `FAIL` line for that binding, and no
  line about the build pool.
- This repository as the root, and the check without `--exits`: unchanged. A pool binding removed from
  `bindings/` is still reported (the existing case stays green with an unchanged assertion), and a pool
  binding without `--pool-wait` in its argv is still reported.
- `docs/` where it names the option says what the option checks for a root that is another runbook, with
  the one line a user adds to a program binding (`exits.error: [75, 126, 127]`).
- The fixtures are written by hand with invented text (protocol id `example@1`); nothing of the consumer's
  directory is read into the repository.
- POSIX sh or the script's own language; no test over 5 s.

## Scope

`scripts/check-bindings.sh`, `driver/tests/check_bindings_exits.rs`, `driver/tests/fixtures/`, `docs/`,
`CHANGELOG.md`.
