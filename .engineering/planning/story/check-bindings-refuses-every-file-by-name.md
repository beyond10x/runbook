---
format: aep.planning-md/3
id: story:check-bindings-refuses-every-file-by-name
kind: story
status: draft
title: check-bindings.sh refuses every file it reads by a line that names the file
summary: 'Four findings of adversary pass 5 of check-bindings-exits-reads-any-runbook that exist on main: YAML that yq cannot write as JSON, a multi-document bindings file, the JSON copies and the protocol files are read without the rule the unit gave the YAML bindings file.'
relations:
- informed_by: story:check-bindings-exits-reads-any-runbook
revision: 1
---
## Outcome

`scripts/check-bindings.sh` refuses every file it reads by a line that names the file, whatever the file
holds: exit 0 or 1, never a raw message of `jq` or `yq`, never another exit code.

## Source

Adversary pass 5 of `story:check-bindings-exits-reads-any-runbook` (2026-11-23). That unit gave the rule to
the YAML bindings file of a protocol (a table of every member with every JSON type). Four kinds of file are
outside it; the behaviour is the same on `main` before the unit:

- YAML that `yq` cannot write as JSON ends the exits step with `yq`'s own message, which names no file, and
  no `FAIL` line.
- A bindings file of more than one YAML document reaches `jq` as two values.
- The JSON copies of the bindings under `driver/examples/` are read without a type check.
- The protocol files the registrations step reads are read without a type check of `protocol.id` and
  `protocol.revision`.

The adversary's three failing cases are committed with that unit as ignored measurements that name this
story.

## Scope

- `scripts/check-bindings.sh` (the read of a YAML file, the steps over the JSON copies, the registrations
  step).
- `driver/tests/`: the three measurements become cases of the default step; the table of the unit gains the
  JSON copies and the protocol files.

## Acceptance

- Each of the four kinds of file ends in exit 1 with a `FAIL` line that names the file, in the full check
  and under `--exits <root>` where that mode reads the file.
- The table test covers the members the script reads from a JSON copy and from a protocol file, each with
  every JSON type; no test of it takes over 10 s on the build host.
- The script's output for this repository's committed files is unchanged.

## Not in this story

- The steps that go on after a failed reader (`story:check-bindings-steps-fail-on-a-failed-reader`).
