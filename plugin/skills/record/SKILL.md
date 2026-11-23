---
name: record
description: Guided procedure that turns a manual session (a person or an agent handling one case by hand) into a draft runbook, a Canon `protocol/1` file with bindings, checked with `canon check`, then handed to `harden`. No tool records the session or writes the protocol; you do every step by hand. Use when someone has handled a case by hand and wants a fixed, checked way to handle the next one, or asks to turn a session, a procedure or a checklist into a runbook. Not for building and tuning a workflow that already has protocols (the `workflow-builder` skill), not for checking an existing protocol (the `harden` skill), and not for a case no one has handled yet.
---

# record

Guided procedure. Nothing here is automated: there is no recorder, no generator and no importer.
You read what the session did, write the facts down, and write the protocol and bindings yourself.

Works in a checkout of this repository, because the protocol pattern, the binding pattern and the
checks are here. Run `test -f protocol/sdc-bugfix.protocol.yaml` from the checkout root; when it
fails, stop and tell the person.

The rules come from `plugin/skills/workflow-builder/SKILL.md` part A (phases 0 and 1) and
`docs/protocol-notes.md`. Read part A, phases 0 and 1, before step 1.

Keep the session material (transcripts, prompts, ticket text, names) in the session scratch
directory. The draft names facts and steps, never a person, a customer or a path of a home
directory.

## 1. Name the case and its end states

Write four lines in a scratch file:

- the case: one sentence, what one instance is (one bug fix, one release, one onboarding);
- its input: the smallest input that still crosses every step the session took;
- its success outcome, and every failure outcome (`not_delivered`, `withheld`, `dropped`,
  `abandoned`);
- the pass rule: closes in the success outcome by itself, no stop, no forced step.

Done when each failure outcome has a sentence saying which fact makes it legitimate. A failure
outcome with no such fact is removed or the fact is found.

## 2. List the steps and their facts

Read the session in order. Write one row per step the person or agent took:

| step id | what was done | actor now (program, model, person) | facts needed | facts produced | can it say no |

- A fact is something a program can read: a file at a revision, a command's exit code, a person's
  recorded decision. A fact that is only in someone's head is a step: add the row that records it.
- Name every artifact a step writes (a file, a directory, a store).
- Mark each step whose actor can say no (a check with a failing result, a review, an approval)
  and write its route: which writer is sent back, or which outcome ends the case after the bound
  (`plugin/skills/workflow-builder/references/checklists.md`, section 1).

Done when every fact needed by a row is produced by an earlier row or is the case's input, and
every "can say no" row has a route.

## 3. Write the draft protocol

Copy the shape of `protocol/sdc-bugfix.protocol.yaml` (one case, no child cases) into
`protocol/draft/<name>.protocol.yaml`. Fill, in this order:

1. `protocol` (`id`, `revision: 1`, `description`) from step 1.
2. `artifacts` from the artifacts of step 2.
3. `evidence_kinds`: one per fact produced; its result values are the results a program or person
   can record.
4. `claims` with `true_when` over an evidence kind; add a claim that reads the kind without a
   result for every observation that must run once per revision, and put it in that action's
   `precondition` as `is: unknown` (`docs/protocol-notes.md`, "An observation runs once per
   revision").
5. `obligations` and `actions`: one action per row, with `effect` (`read`, `write`, `external`),
   `precondition` from the facts needed, `may_produce` from the facts produced, and `requires` for
   a person's authority. A model only writes artifacts: the result of every check is produced by a
   program or a person.
6. `outcomes` from step 1, each with `requires`.
7. `invalidation` for every fact that an upstream revision makes stale.

An approval is decided once per revision of what it approves; after `rejected` the artifact's
writer is admissible again (`docs/protocol-notes.md`, "An approval is decided once per revision").
Every loop has a bound the protocol can see, and an exit.

## 4. Write the draft bindings

Copy the shape of `driver/examples/sdc/sdc-bugfix.bindings.json` into
`driver/examples/sdc/draft/<name>.bindings.json`. Per action: `argv` (the tool the action runs),
`produces`, `subject`, `producer`, `results` (exit code to result), and for a writer `retry_when`,
`max_runs` and `timeout_s`; `manual: true` for a person's action. `$artifacts` maps each artifact
to its path in the case's work directory. The fields and what the driver does with them:
`docs/driver.md`.

A tool that does not exist yet is written as a stub that exits with the result the session
produced, and listed in a scratch file as "tool to write". No new tool is invented in the draft.

## 5. Check the draft

From the repository root (`runbook_resolve` finds or builds `runbook`, which links the pinned Canon):

```console
repo=$(pwd)
source scripts/evaluate-cases.sh && runbook_resolve && "$runbook" canon validate --path "$repo/protocol/draft/<name>.protocol.yaml"
"$runbook" canon check --path "$repo/protocol/draft/<name>.protocol.yaml"
```

Read each finding, change the draft, run both again. Done when `validate` exits 0 and `check`
prints no finding, or each remaining finding is written into the draft's `description` as a known
limit with its reason.

## 6. Hand over to `harden`

Tell the person, in one message: the draft's two paths, the "tool to write" list, the findings left,
and that the draft is not yet part of the checked set. A draft joins it by moving the protocol to
`protocol/<name>.protocol.yaml` and the bindings to `driver/examples/sdc/<name>.bindings.json`;
`scripts/check-protocols.sh --repin` then pins its simulate report. Then run the `harden` skill on
the moved protocol.
