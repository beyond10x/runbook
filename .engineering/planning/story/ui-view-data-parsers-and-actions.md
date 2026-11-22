---
format: aep.planning-md/3
id: story:ui-view-data-parsers-and-actions
kind: story
status: active
title: The run inspector reads the parsers of a runbook's records and the decisions a person can take from its view file
relations:
- decomposes: epic:live-view
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: ui/README.md
- confidence: cited
  path: ui/decisions.mjs
- confidence: cited
  path: ui/server.mjs
- confidence: cited
  path: ui/test
- confidence: cited
  path: ui/web
- confidence: cited
  path: views
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:12:21Z", actor: "human:operator", revision: 3, executor: "agent:wave-rb2-ui-view-data", correlation: "operator-standing-acceptance-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T01:12:21Z", actor: "human:operator", revision: 4, executor: "agent:wave-rb2-ui-view-data", correlation: "operator-standing-acceptance-2026-11-22"}
---
# Story: ui-view-data-parsers-and-actions

## Outcome

The live view (`ui/`) works for a runbook that is not `sdc`, also where it reads what a change means and
what a person can decide. The two places that still held `sdc` knowledge, the parsers of the direction
and intake records in `ui/web/meaning.js` and the scope-answer, amend, context and continue actions in
`ui/decisions.mjs`, read it from the view file (format `runbook.view/1`, new members `runbook`,
`protocols`, `file_kinds.parsers`, `decisions`). The code keeps what the runtime defines: events,
evaluations, evidence, decisions, bindings and the AEP, ESS and ADR file formats. A view member the
inspector does not know is refused with a message that names it. A run no loaded view covers renders with
the generic parts only and the page says which view is missing; there is no `sdc` fallback.

## Context

Follows `story:ui-view-data-per-runbook`, which moved phases, file kinds and tool owners. The remaining
`sdc` knowledge, listed from the base commit `5d9e679` (list (a) of the brief). Part (i) is
runtime-generic and stays code; part (ii) is runbook-specific and moves.

(ii) moves to `views/sdc.view.json`:

| Item (base file:line) | New home |
| --- | --- |
| `ui/web/meaning.js:386-395` `idList` and `:397-415` the parsers of `requirements`, `questions` (`answer`, `answered_by`, `status`), `findings` (`raised_by`, `text`, `status`), `dissection` (`items`, `type`, `note`, `span`) | `file_kinds.parsers.<kind>`, a `keyed-list` with `added`, `removed`, `changed` rules and sentence templates |
| `ui/web/meaning.js:416-430` the parser of `context` (a map of fields with `value`, `status`, `reason`) | `file_kinds.parsers.context`, `keyed-list` with `as: "map"` |
| `ui/web/meaning.js:431-455` the parsers of `roadmap` and `vision` (`epics:` summary, section words) | `file_kinds.parsers.roadmap` and `.vision`, `document` with `entries` and `created` |
| `ui/decisions.mjs:107-109` `SCOPE_ACTION = 'scope.answer'`, `SCOPE_CHECK = 'questions.check'`, `principalOk` refusing `non-interactive` | `decisions.scope_answer.action`, `.check`, `.refused_answerers` |
| `ui/decisions.mjs:118-124, 203-209, 256-262, 347-375` the artifact `questions`, the list `questions`, the flag `scope_changing`, the fields `answer`, `answered_by`, `answered_at`, `reason`, the statuses `open`, `accepted`, `changed`, the yq env names `SDC_*` | `decisions.scope_answer.artifact`, `.list`, `.flag`, `.fields`, `.statuses`; env names are `RB_*` |
| `ui/decisions.mjs:133-139, 198` `AMEND_ACTION = 'amend.upstream'`, artifact `findings`, status `needs_person` | `decisions.stall` |
| `ui/decisions.mjs:378-392` the artifact `context`, the field pattern `^[a-z_]+$`, the status `changed` | `decisions.context` |
| `ui/decisions.mjs:451-487` `scripts/run-trial.sh`, the folder `runs`, `SDC_PRINCIPAL`, the text "run under sdc/runs/" | `decisions.continue` (`script`, `runs_dir`, `env`); the text takes the view's `runbook` |
| `ui/web/now.js:62-64,130-131,399-418`, `ui/web/inbox.js:80`, `ui/web/kpis.js:142`, `ui/web/app.js:376` the literals `scope.answer`, `amend.upstream`, `findings.yaml`, `run-trial.sh` | read through `scopeAnswer()`, `stallRule()` of `ui/web/view.js` |
| `ui/web/feed.js:62` the file kind `questions` | `questionsKind()` of the scope-answer rule |
| `ui/web/index.html:39` the label "SDC phases" | "Runbook phases" |

(i) runtime-generic, stays code: the event names (`awaiting_authority`, `awaiting_person`, `approved`,
`action_started`), the `approve` act and the capability record, the evaluation and evidence reading, the
planning, spec and ADR parsers (formats of AEP, ESS, ADR), the generic top-level-keys diff for a kind
with no parser, the stale-view check by file digest, the audit file.

Remaining `sdc` in `ui/server.mjs` (outside the two files of the brief, left in code): the driver and
canon binary paths (`sdc-drive`), the format ids `sdc.run-status/1`, `sdc.experiment/1`, `sdc.fs-change/1`
and the like (formats of the run store), `.sdc-drive.lock`, the `x-sdc-ui` header. They name the
runtime's files, not a runbook.

## Acceptance

- `git grep -n -i sdc -- ui/web/meaning.js ui/decisions.mjs` prints nothing; the hits left elsewhere are
  listed above with their reasons.
- The sdc runs of `ui/test/recorded/**` render as before, with no existing assertion changed: the full
  UI test step passes.
- A second, non-sdc view in `ui/test/view.spec.mjs` has a parser for its record `asks.yaml` and a
  decision of its own (action `ticket.ask`, fields, statuses and flag that are not sdc's): the page's
  feed shows "ask a1 raised: “Which queue handles refunds?”" and "ask a1 answered: “queue-b” by ana", the
  inbox offers the question with the approval of `ticket.ask`, and the answer is written to `asks.yaml`
  with the view's own field names.
- A view with a member the inspector does not know is refused: `checkView` returns a line that names
  the member, and `node server.mjs --view <file>` exits 2 with that line on stderr.
- A run no loaded view covers (the same store with `--no-view`, or with `views/sdc.view.json`) shows the
  generic phase list, no question card, and a note that names the missing view and its protocol.
- The servers of `ui/test/view.spec.mjs` take free ports.
- `aep plan artifact validate` exits 0.

## Scope

`ui/web/`, `ui/decisions.mjs`, `ui/server.mjs`, `ui/test/`, `ui/README.md`, `views/sdc.view.json`,
`CHANGELOG.md`.

## Out of Scope

- A view format version 2, or a way to add a decision kind from a view file: a new kind of decision
  needs a code change.
- The wording of the page ("interview", "scope-changing") where it is not a name in the data.
- Serving two runbooks to concurrent requests of one server: the view is chosen per request and per
  source, and a request that waits inside an act could see another source's view. Not tested.
- `website/**`, `driver/**`, `bindings/**`, `Taskfile.yml`.

## Ambiguities

- The view that covers a run: `inferable`, the protocol ids of the run's cases against `protocols` of the
  view, else against the protocols of its phases (`ui/web/view.js`, `viewCovers`).
