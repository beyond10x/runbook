# Improving the workflow

The operator, 2026-11-21: "Now steadily improve the workflow. Document your gains, document it
accurately, and prepare for putting this onto a nice image on the public documentation. Cut
releases after every improvement." One change at a time, measured, merged, released; the gains are
in `docs/progress.md` and `docs/img/progress.svg`, written by `sdc-experiment progress`. The
candidates are the stories of `epic:improvement-loop` (`aep plan artifact list`).

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

## What the 0.1.0 baseline says

From the nine runs of `experiments/baseline-0.1.0`, counted in `experiments/baseline-0.1.0/losses.md`:

- Reruns took 25 % of the strong setting's cost (4.29 of 17.13 USD), 31 % of mixed (2.78 of 8.93) and 57 % of Haiku (1.50 of 2.64).
- The largest loss is the review sending the build back: 10 rejections, 5.25 USD and 1,873 s; 4 of them an offline requirement not built.
- The Haiku decider is 6 to 7 % of a mixed run's cost but 45 to 48 % of its wall time (717, 952, 693 s); at effort `xhigh` it thinks 20k to 35k tokens per merge approval.
- The Opus decider is 33 to 37 % of a strong run's cost (1.82 to 2.08 USD).
- Of 92 content refusals, 5 named an exact fix a program could apply, all a format version; haiku-3 stopped after applying it to a domain file.
- haiku-1's Postgres default (q9) was accepted by the non-interactive rule, not by the decider.
- Stories 3, 5 and 7 of `epic:improvement-loop` address about zero loss in these runs; they are parked.

Ranking rule: first what needs no new code (a setting, then an existing branch), then new code; within each group, a stop of the Haiku setting before money and time, then the larger loss the change could at most remove in the baseline runs.

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
