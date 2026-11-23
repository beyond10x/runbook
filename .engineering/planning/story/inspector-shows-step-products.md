---
format: aep.planning-md/3
id: story:inspector-shows-step-products
kind: story
status: active
title: The inspector shows what a step produced
summary: Evidence records, artifacts and files under work/ (Markdown rendered) and the exit code, duration and output tails of an action can be read in the inspector; the content of a run never becomes markup of the page.
tags:
- operator-requirement
scope:
- confidence: cited
  path: docs
- confidence: cited
  path: runbook
- confidence: cited
  path: ui
- confidence: cited
  path: views
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T21:36:47Z", actor: "human:operator", revision: 3, executor: "agent:runbook"}
- {from: "proposed", to: "active", at: "2026-11-22T21:36:47Z", actor: "human:operator", revision: 4, executor: "agent:runbook"}
---
## Outcome

A person who opens a run in the inspector can read what each step produced: the result and the
recorded fields of an evidence record, the content of an artifact or of a file the run wrote under
`work/` (Markdown rendered), and the exit code, the duration and the output tails of an action.

## Source

The operator, 2026-11-22, relayed by the consumer session that runs runbooks other than SDC:
"these nodes ... do create artifacts or return results, but in this web UI ... there is no way to
look at them at all". The consumer's runs write one `.txt` result per step under
`work/<runbook>/`; its next runbooks write a Markdown report as their main product, and that
report is what the operator reads in the page.

## Acceptance

- Evidence: a click on an evidence record opens a panel with its result, its subject, its producer,
  its time and every field the step recorded (nested values readable, nothing dropped).
- Files: the page lists the artifacts of the run and the files under the run's `work/` directory;
  a click on one shows its content. Text is shown as text. A `.md` file is rendered: headings,
  paragraphs, lists, code, emphasis, tables and links. JSON and YAML are shown indented. A file
  that is not text is shown with its size and type and no content. A file over the display limit
  is cut at the limit and the panel says how many bytes it has.
- Action: a click on an action shows its exit code, its duration, its start and its end, and the
  tail of its standard output and of its standard error when the run recorded them. Where the run
  did not record a value the panel says "not recorded by this run"; the page never shows an empty
  field as if the value were empty.
- The product of a run is reachable from the run's first screen in one click. A view file names
  it with the top-level key `product` (one path relative to the run directory, checked by
  `runbook check`); without the key the file `work/<runbook>/<runbook>.md` is the product;
  otherwise the Markdown files under `work/` are linked, newest first.
- A path with a component that starts with a dot (the cache directory of a tool) is neither
  listed nor served; the listing says how many such directories it left out.
- It works for a run of `runbook run` of any runbook (a fixture written by hand with invented
  text, protocol id `example@1`) and for an SDC run, in the inspector and in the page the daemon
  serves.
- Safety: every read goes through the run-path check (the resolved path lies under the run) and
  serves a regular file only; a symbolic link that leaves the run, a named pipe, a device and a
  directory are refused. The content of a run never becomes markup or code of the page: the
  Markdown renderer builds nodes from text, raw HTML in a file is shown as text, a link is
  followed only when it is `http:`, `https:` or a path inside the run, an image is shown as a link
  and is not loaded. A test with a hostile Markdown file (a script element, an element with an
  event handler, a `javascript:` link) finds no script node, no handler attribute and no such
  link in the page. The inspector gains no route that acts on a run.
- No request leaves the machine to render a file (no library from a network address).
- Tests: Playwright specs of the gate lane, each under 60 s, none needs `runs/`.

## Scope

- `ui/` (the server's read routes, the page, `ui/test/`).
- `runbook/` only where the daemon's page embeds the UI files and its tests list them.
- `docs/` (the inspector's page of the documents, one section).

## Not in this story

Recording the output of an action where the runner does not record it today: named as a draft
story by the unit that finds the gap. Editing or deleting a file of a run.
