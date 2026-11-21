# Roadmap toward retiring Conductor and takt

Store mirror: `initiative:roadmap` (AEP has no roadmap kind yet; it is being built in the aep
repository). Ids are stories in `.engineering/planning/` unless marked as epics, listed per
milestone in the order `aep plan artifact waves --status proposed` derives. No dates.

## Goal (proposed: the operator has not confirmed the wording)

The system delivers real stories on a real repository unattended, with evidence a program checked,
cheaper and with fewer operator touches than Conductor did: 10 stories on `decider` at autonomy A3
or higher, at most 11 USD per merged item and at most 0.245 operator touches per item, running on
Loom with every command confined. Then Conductor and takt are retired.

Grounds: `approval-record:operator-decision-conductor-succession`,
`approval-record:operator-decision-takt-freeze`; thresholds as in `story:operating-record-report`.

## Milestones

| | stories | blocked upstream today |
|---|---|---|
| M0 Baseline in git | 0 (work outside the store) | 0 |
| M1 Unattended clean run | 24 (1 implemented) | 0 |
| M2 Real verification and real git | 11 | 0 (one waits on ESS work in progress) |
| M3 Operating record | 8 | 1 |
| M4 On Loom, confined | 8 | 7 |
| M5 Many items | 25 | 1 |
| M6 Succession complete | 2 | 0 (both are requests or open decisions) |

### M0 Baseline in git

- Exit: trial 2 is written up in `docs/trial-2.md`; the repository has its first commit
  (`b10x-gates bot -- commit`); the public repository is enrolled in Gates.
- Stories: none in the store.
- Waits on: the operator (first commit, public visibility); the Gates enrolment.
- KPIs: R16 replay match and L1-L6 of trial 2 (`docs/kpis.md` § 1b, § 4) as the recorded baseline.

### M1 Unattended clean run

- Exit: one idea runs from intake to close at A5 with zero time waiting on a person, twice (two
  model settings), with the comparison report of `clean-unattended-run-compare`.
- Stories: goals-format-check (implemented), amendment-keeps-person-decisions,
  decision-contract-goals-grounds (reported in implementation), late-findings-per-stage-kpi,
  story-finding-reopens-spec, goals-root-input, interview-question-context,
  decision-request-from-question, llm-call-count-matches-record, kpi-decisions-returned-for-context,
  model-per-node-variable, ui-decision-request, constraint-consistency-rules, requester-decider,
  ui-goal-grounds, stop-message-prints-request, decider-finding-decision, decision-cold-read,
  replay-after-child-close, clean-unattended-run-compare, interview-derived-from-goals,
  eval-requester-persona, intake-asks-what-stories-will-need, requester-kpis. Epics:
  `requester-side-and-goals-context` (less requester-decider-loom, M4), `decisions-a-person-can-take-cold`.
- Waits on: nothing upstream. KPIs: L1 interventions, L2 autonomous run time, H1-H4 decisions, R4-R7 calls, tokens, cost and
  wall time per model setting; time waiting on a person (target 0).

### M2 Real verification and real git

- Exit: the eval's scripted false "done" (`story-readme`) is caught (R2 scripted false done = 0);
  a story merges through a real branch and pull request, with CI evidence bound to its revision.
- Stories: conformance-against-implementation, independent-reviews, per-story-worktree,
  verifier-independence, story-branch, story-merge-step, change-merge-git, forge-pr-evidence,
  forge-ci-evidence, release-tag, forge-release-evidence. per-story-worktree and story-merge-step
  belong to epic `concurrent-child-cases` and are pulled forward: story-branch and
  change-merge-git depend on them.
- Waits on: ESS (a conformance target that executes an outside implementation, in progress; the
  default is a runner on the SDC side first); engineering-protocols (the data-only catalog, for the
  ported gate validator); takt sources (frozen, read only).
- KPIs: R2 false-done rate, W3 story conformance pass rate, W4 review rejection rate.

### M3 Operating record

- Exit: the pilot's record states cost per merged item and operator touches per item for 10
  stories on `decider` at A3, against 11 USD and 0.245.
- Stories: ess-ratchet-gate, import-approvals, intake-reconstruct, roadmap-reconstruct, flow-kpis,
  operating-record-report, pilot-real-repository, roadmap-kind-adopt. flow-kpis (epic
  `flow-limits`) is pulled forward: operating-record-report reads F13 and F16 from it.
- Waits on: AEP roadmap kind (`upstream-blocker:aep-roadmap-kind`; being built today), for
  roadmap-kind-adopt only.
- KPIs: R1 verified success, F13 cost per closed item, F16 person decisions per closed item.

### M4 On Loom, confined

- Exit: the same pilot story runs through Loom's tree run with Substrate confinement and replays.
- Stories: authority-revision-binding-uptake, bindings-to-loom-bindings-1, canon-slice-bound-uptake,
  sdc-on-loom-tree-run, protocol-imports-uptake, resume-cut-step, confined-bound-commands,
  requester-decider-loom.
- Waits on: Canon (release after 0.9.0 with the slice-wise bound and `--import`; authority bound
  to a revision); Loom (bindings document, tree run, decider per class); Loom and Substrate
  (confinement of bound commands and child launches). Only resume-cut-step is not blocked.
- KPIs: R16 replay match on Loom; confinement refusals per run; the M3 figures repeated on Loom.

### M5 Many items

- Exit: a wave of stories runs concurrently under WIP limits, and the in-progress-limit sweep sets
  the default per level (F17).
- Stories: root-outcome-delivered, concurrency-limit, orchestrate-protocol-adopt, area-board-shell,
  epic-stories-to-portfolio, orchestrate-backlog-survey, orchestrate-wave-admit, per-case-lock,
  area-admission-strip, orchestrate-intake-triage, orchestrate-wave-approve,
  plan-partial-activation, ui-concurrent-writers, area-intake-strip, orchestrate-wave-propose,
  area-orchestrator-panel, orchestrate-wave-integrate, standing-wakeup, wip-limits-admission,
  concurrent-sibling-drive, orchestrate-standing-horizon, loom-concurrent-children, wip-sweep-eval,
  limits-changed-event, area-board-flow-kpis. Epics: `orchestrate-area-case`,
  `concurrent-child-cases`, `flow-limits`, `area-board`, rest of `brownfield-entry`.
- Waits on: Loom (child cases, concurrent siblings, standing trees, run ceilings), for
  loom-concurrent-children only.
- KPIs: F1-F17 (`docs/kpis.md` § 5).

### M6 Succession complete

- Exit: Conductor and takt carry freeze notices, and nothing compiles against engineering-protocols.
- Stories: takt-freeze-notice (a request to the takt repository), project-rename (the name is the
  operator's decision, postponed).
- Waits on: the takt and Conductor repositories (freeze notices); engineering-protocols (catalog as
  data). None of these three has a story here: they belong to those repositories.
- KPIs: none new.

## Next three actions

1. M0: finish the trial 2 write-up and make the first commit through the bot.
2. M1: implement `decider-finding-decision` and `model-per-node-variable` beside the requester and
   decision stories already under way.
3. M1 exit: run `clean-unattended-run-compare` (trial 3) in both model settings.
