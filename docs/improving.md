# Improving the workflow

The operator, 2026-11-21: "Now steadily improve the workflow. Document your gains, document it
accurately, and prepare for putting this onto a nice image on the public documentation. Cut
releases after every improvement." One change at a time, measured, merged, released; the gains are
in `docs/progress.md` and `docs/img/progress.svg`, written by `sdc-experiment progress`. The
candidates are the stories of `epic:improvement-loop` (`aep plan artifact list`).

## Target

The operator, 2026-11-21, after one free Sonnet session built the same idea for 0.41 to 0.48 USD:
"so, this is our target then 10% costs". And how: "reduce context to minimal fact sets / inputs -
dont collect big conversations, figure out where we can use cheeper models".

- **Figure**: `run.cost_usd` 0.47 USD on the `todo-app` input in every setting, the median of the
  three runs of `free-agent-0.1.0` (record on branch `feat/run-timeline`). Now: strong 5.50, mixed
  2.55 (`baseline-0.1.0`).
- **Condition**: neither delivery nor the product check gets worse. `run.delivered` and `run.clean`
  stay at least the setting's reference record. The product check is the oracle once runs write
  one; until then, the passing tests and the requirements covered in `work/impl`.
- **Machine-readable**: `experiments/targets.yaml` (`sdc.targets/1`), validated by
  `sdc-experiment check --all`.
- **Where the money goes and the levers**: `experiments/baseline-0.1.0/costs.md` (budget by token
  kind and node class, the floor), `inputs.md` (fact sets per node), `models.md` (cheaper models
  per class, the replay test).
- **The floor with today's protocols**: about 2.0 USD with Sonnet on the writer, implement and
  review nodes, even with no reruns. 0.47 USD needs Haiku work nodes that pass and fewer sessions
  (34 per run against 1).

## The loop

1. **Baseline.** On the last release's tree, n ≥ 3 runs per model setting, started by
   `sdc-experiment run <id>` from a prospective record (`docs/workflow-metrics.md` § 7). A
   baseline is reused, not rerun, while the input's sha256, the settings and the tree match.
2. **Pick one change.** From the loop nodes and edges of the last reports, or the next story of
   the epic. One change per experiment.
3. **Write the record** before any run: hypothesis (one sentence, the node or edge, the direction
   and the size), one variable, primary metric, guards (at least `run.delivered` and
   `run.cost_usd`), `runs_per_variant` ≥ 3, `decision_rule: complete-separation`. Commit it.
4. **Run.** The change goes behind a run setting where it can, so one record on the change's tree
   holds baseline and treatment and `report` computes the verdict. A record names one tree: the
   tree itself as the variable (baseline tree against change tree) is not built yet.
5. **Verdict.** `sdc-experiment report <id>`: held, contradicted or inconclusive, from the rule,
   never from reading the numbers. Inconclusive: a new record `<id>-2` with the same settings.
6. **Merge** the change with green gates when the verdict held and every guard held; record
   `adoption.decision: adopted` with the commit. A contradicted change is dropped; it still gets
   its line in the ledger.
7. **Release** (`docs/releasing.md`): the version, `CHANGELOG.md`, `docs/releases/<version>.md`.
8. **Ledger.** Add the entry to `experiments/releases.yaml`: version, date, tree commit (null with
   a `tree_note` until the tag exists), a `short` of at most 28 characters for the image, the
   experiment ids, each change by commit with one line and
   its effect (`measured` with the experiment, `unmeasured`, or `none`: no run metric affected).
   Run `sdc-experiment progress`; `sdc-experiment check --all` (in `scripts/check-bindings.sh`)
   fails until the ledger and the image match the records. Commit them with the release.
9. The new release is the next baseline: its tree is new, so step 1 runs again.

## Run switches

A change in what a run does is measured before it becomes the default (the operator's standing
rule). Such a change merges behind a switch that is off by default; the experiment record turns it
on in its variant (`settings`, a launch setting of `sdc-experiment`, `LAUNCH` in
`driver/src/bin/sdc-experiment.rs`), and `run-settings.jsonl` records it per run. A switch becomes
the default only after its record's verdict held (step 6). With every switch at its default a run
does what main did before these changes: the `<switch>-default` cycles read one re-drive of the
committed stub recording (run `switches-default`) with every call recorded, none diverged and no
prompt digest changed (`scripts/run-finding-cycles.sh`).

| Switch | Values (default first) | Setting | Turns on | Record | Cycles (default; on) |
|---|---|---|---|---|---|
| `SDC_IMPLEMENT_MODE` | `session`, `request` | `implement_mode` | `change.implement` as one structured request (`story:implement-one-request`) | `implement-one-request`, `implement-one-request-strong` | `implement-request-*` |
| `SDC_RULE_APPROVALS` | `off`, `on` | `rule_approvals` | under A5, context, roadmap and release approvals taken by `rule:autopilot` on their program checks (`story:decider-program-approvals`) | `decider-program-approvals`, `decider-program-approvals-mixed` | `rule-approvals-default`; `rule-approvals*` |
| `SDC_REVIEW_MODE` | `session`, `facts` | `review_mode` | `change.review` as one request on one document of facts, no tools (`story:review-minimal-facts`) | `review-minimal-facts` | `review-mode-default`; `review-minimal-input`, `review-real-path-facts` |
| `SDC_IMPLEMENT_COVERAGE` | `off`, `on` | `implement_coverage` | `change.implement` cites each requirement, refused by program before `tests.run` (`story:implement-cites-requirements`) | `implement-cites-requirements` | `implement-coverage-default`; `coverage-*` |

## What the 0.1.0 baseline says

From the nine runs of `experiments/baseline-0.1.0`, counted in `experiments/baseline-0.1.0/losses.md`:

- Reruns took 25 % of the strong setting's cost (4.29 of 17.13 USD), 31 % of mixed (2.78 of 8.93) and 57 % of Haiku (1.50 of 2.64).
- The largest loss is the review sending the build back: 10 rejections, 5.25 USD and 1,873 s; 4 of them an offline requirement not built.
- The Haiku decider is 6 to 7 % of a mixed run's cost but 45 to 48 % of its wall time (717, 952, 693 s); at effort `xhigh` it thinks 20k to 35k tokens per merge approval.
- The Opus decider is 33 to 37 % of a strong run's cost (1.82 to 2.08 USD).
- Of 92 content refusals, 5 named an exact fix a program could apply, all a format version; haiku-3 stopped after applying it to a domain file.
- haiku-1's Postgres default (q9) was accepted by the non-interactive rule, not by the decider.
- Stories 3, 5 and 7 of `epic:improvement-loop` address about zero loss in these runs; they are parked.

Ranking rule, since the target: by saving per run toward 0.47 USD (`costs.md`, `inputs.md`, `models.md`). Equal savings go first to what needs no new code (a setting, then an existing branch), then to what a recorded replay can test before a full run.

## Versioning

- **Patch** (`0.1.x`): a fix of a defect, a tool, a check, documentation, a measurement.
- **Minor** (`0.x.0`): a protocol change, or a change of what the driver, a node or the decider
  does in a run.
- No major version while the prototype has no stable interface.

## The CHANGELOG entry

Each change of the release, by commit, with one of:

- the experiment id, its verdict, and the primary metric of baseline and treatment as median
  [min, max] (n), with the clean passes of ended runs per setting as counts;
- or the sentence "No run metric is affected." when the change cannot change a run (documentation,
  scripted cycles, a protocol the benchmark input does not run);
- or "Not measured: no experiment has tested this change." for a change that affects runs but
  ships before its experiment. The ledger lists it the same way.

Inconclusive and contradicted results are written like held ones. A figure from a run that is not
in git says so (`docs/releasing.md`, Notes and changelog).
