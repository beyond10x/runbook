---
format: aep.planning-md/3
id: story:inspector-reads-regular-files
kind: story
status: draft
title: The inspector reads only regular files of a run
summary: A named pipe called like a run file makes the inspector wait (10 s per pipe through yq, without a limit through readFileSync); files that are not regular are skipped as unreadable.
tags:
- from-finding
relations:
- informed_by: review-result:adversary-line2-main-merge-pass-1
revision: 1
---
## Outcome

The inspector reads a run file only when it is a regular file. A named pipe, a device or a
socket in a run directory is skipped as unreadable and does not make the server wait.

## Finding

`review-result:adversary-line2-main-merge-pass-1` (origin: on `main` at `17c00ab57f`, came into
line 2 with the merge of `main`; verdict NEEDS-CHANGE, severity note): `ui/decisions.mjs:50`
(`yamlJson`), `ui/runfacts.mjs:28-29`. `yamlJson` starts `yq` with `execFileSync`. A named pipe
called `x.protocol.yaml` in `<run>/protocols/` passes `runPath` (its real path lies inside the
run), `yq` blocks on opening it until its 10 s timeout, and the whole server waits: `/api/tree`
took 10.07 s in the reviewer's case. The wait repeats once per pipe and per change of the run's
file signature. The process stays up. Named in the same finding: `readText` and `readJson`
(`readFileSync`) on a named pipe that is called like a run file block without a timeout.

## Acceptance

- `runPath` (or the helpers over it: `readText`, `readJson`, `listDir`, the reads of
  `ui/runfacts.mjs`) refuses a path whose target is not a regular file or a directory; the run
  is shown with that file named as unreadable.
- The reviewer's case passes: `/api/tree` answers within 3 s with a named pipe in
  `<run>/protocols/`.
- A case with a named pipe called `events.jsonl`, one called `case.json` and one called
  `run-settings.jsonl`: the server answers within 3 s and stays up.
- `yq` is not started on a file the check refused.

## Scope

- `ui/runfiles.mjs`, `ui/runfacts.mjs`, `ui/decisions.mjs`, `ui/server.mjs`.
- `ui/test/` (one spec).

## Not in this story

Starting `yq` without blocking the server for a large regular file: the 10 s timeout stays.
