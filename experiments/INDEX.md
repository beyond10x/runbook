# Experiments

Written by `sdc-experiment index` (and by every `sdc-experiment report`); `sdc-experiment check --all` fails when it is stale. One row per record `experiments/<id>.yaml`; the procedure is in `docs/workflow-metrics.md`.

| experiment | created | kind | purpose | tree | variants × runs | status | verdict |
|---|---|---|---|---|---|---|---|
| [baseline-0.1.0](baseline-0.1.0/report.md) | 2026-11-21 | prospective | Baseline of release 0.1.0; first runs on the hermetic decider with explicit efforts. | 4468bfd | strong 3/3, mixed 3/3, haiku 3/3 | complete | descriptive |
| [bugfix-seeds-1](bugfix-seeds-1/report.md) | 2026-11-21 | retrospective | Run sdc.bugfix (red first, then the fix) unattended on four seeded bugs, once per seed on each model, to see whether each model writes a red test and a fix that passes the gate. | several | haiku 4, sonnet 5 | complete | descriptive |
| [decider-haiku-vs-opus](decider-haiku-vs-opus/report.md) | 2026-11-21 | retrospective | Whether the decider model changes how often a rejected merge sends a story back to the implementer, read from the trial-3 runs that exist. | several | haiku 7, opus 2 | complete | inconclusive |
| [effort-implement-levels](effort-implement-levels/report.md) | 2026-11-21 | prospective | Whether more reasoning effort on the implement class lowers how often change.implement is sent back, three runs per level, as the operator described. | - | low 0/3, medium 0/3, high 0/3 | planned | - |
| [small-idea-1](small-idea-1/report.md) | 2026-11-21 | retrospective | Run a deliberately small idea from idea to close with no person, to see whether the loops of the to-do runs come from the idea's size. | acd6f12 | haiku 1, sonnet 1 | complete | descriptive |
| [trial-3-attempts](trial-3-attempts/report.md) | 2026-11-21 | retrospective | Run the to-do idea from idea to close with no person, all Haiku beside Sonnet work with a Haiku decider, and fix what stops each attempt before the next. | several | haiku 5, mixed 4 | complete | descriptive |
| [trial-3-batch-1](trial-3-batch-1/report.md) | 2026-11-21 | retrospective | Run the to-do idea from idea to close with no person on one tree in five model settings, to see which settings pass and where each one loops. | acd6f12 | haiku 2, mixed 2, sonnet 1, strong 2, opus 1 | complete | descriptive |
| [trial-3-main-1](trial-3-main-1/report.md) | 2026-11-21 | retrospective | Run the to-do idea from main with the review node (change.review, class review) in three model settings, the first runs in which a review model is called. | 8db4b93 | strong 2, mixed 1, haiku 1 | complete | descriptive |
| [trial-3-main-2](trial-3-main-2/report.md) | 2026-11-21 | retrospective | Rerun trial-3-main-1's settings on main with the review reply fix, to see whether runs pass with a real review model. | 1001970 | strong 2, mixed 1, haiku 1 | running | descriptive |
| [work-haiku-vs-sonnet](work-haiku-vs-sonnet/report.md) | 2026-11-21 | retrospective | Whether Sonnet on the work nodes lowers change.implement's looping factor against Haiku, read from the trial-3 runs that exist. | several | haiku 7, sonnet 7 | complete | inconclusive |
