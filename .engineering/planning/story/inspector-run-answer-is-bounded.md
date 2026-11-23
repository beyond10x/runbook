---
format: aep.planning-md/3
id: story:inspector-run-answer-is-bounded
kind: story
status: draft
title: The inspector answers a run within a bound of time and bytes
summary: 'Three findings of security pass 3 in code older than the step-products unit: /api/run lists llm/ once per call-log line, the read limit is per file with no bound per request, and the log of a finding is not confined to blobs/.'
tags:
- backlog-2026-11-23
relations:
- informed_by: story:driver-runtime-removed
revision: 2
---
## Outcome

The inspector answers a request for a run within a bound of time and of bytes read, whatever the run
directory holds.

## Source

Security review passes 3, 4 and 5 of `story:inspector-shows-step-products` (2026-11-23,
`review-result:security-inspector-step-products-pass-3`, `-pass-4`, `-pass-5`), findings in code older than
that unit:

- `/api/run` lists the directory `llm/` once per line of the call log (`ui/server.mjs:805`), so its cost
  grows with lines times entries; the reviewer's case got no answer in 55 s and a small request beside it
  waited as long.
- The 64 MiB limit of `readText` is per file; the number of files one request reads is not bounded
  (`ui/server.mjs:811`, `ui/runfiles.mjs:276`; the events file per case costs the same).
- `run.log` makes one object per command line without a bound.
- Pass 4: `fsChanges` reads a changes file of any size in chunks within one request (`ui/server.mjs:563`);
  the work per request has no bound.
- Pass 5: a parse of an artifact's YAML (`ui/decisions.mjs:53`, `yamlOf`) can hold the server for the 10 s of
  its timeout, and artifacts are parsed again on every poll.
- Pass 4, a note: a refused read of `events.jsonl` or `changes.jsonl` shows as absent (`ui/server.mjs:106`);
  the page does not say that a read was refused.

Closed since revision 1 of this story, by fix round 4 of the unit: the log of a finding is read with the case
directory as root, inside `blobs/`, and `yq` gets the text on standard input.

The reviewer's two failing cases of pass 3 are kept outside the repository until this story is built
(`<scratch>/ui-products/review-3/backlog-specs/`).

## Scope

- `ui/server.mjs` (`/api/run`, `fsChanges`), `ui/runfiles.mjs` (`readText`, a budget per request),
  `ui/decisions.mjs` (`yamlOf`).
- `ui/test/`: the two cases above, one for the changes file and one for the YAML parse.

## Acceptance

- `/api/run` lists `llm/` once per request; the reviewer's case with 1,500 lines and 300 directories
  answers within the test's bound and a small request beside it is not held.
- One request reads at most a stated number of bytes in total, the changes file included; what is left out
  is counted in the answer and shown on the page as left out.
- The text given to a YAML parse has a stated size bound, and one content digest is parsed once; no read of
  the inspector blocks the server's loop longer than a stated bound.
- A read the checked open refused is shown on the page as refused, apart from a file that is absent.
- The inspector stays read-only: no route that acts on a run, no token field, no proxy.

## Not in this story

- The dot-directory rule by real path (`story:inspector-dot-directories-by-real-path`).
- The page of the daemon (`story:daemon-page-shows-step-products`, not filed yet).
