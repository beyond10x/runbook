---
format: aep.planning-md/3
id: review-result:adversary-unit-b-pass-4
kind: review-result
status: active
title: 'Confirmation of unit B (producer ids, old bindings) at b91f2b3: red, 1 note'
summary: F-A, F-B and F-C of the narrow check fixed; 1 introduced note (a bare host name with a port still passes as file:line in a provided_by cite).
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: B (producer ids, old bindings) at b91f2b3, confirmation
verdict: red
cases: executed 415→418, red 1
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: <scratch>/removal/line3-b/adversary/pass-4/ (cases.patch, mut.sh, muts.txt, full.sh, mut.log, run1.log, tree/)
needs-coordinator: no
```

The `before` figure is the test count of the `main` binary from nextest's skip count. I ran only the filtered sets.

- **F-A: fixed, with one weak spot.**
  - `a_web_address_is_not_a_file_and_line` and the other `runtime_block` cases pass (24 of 24 together with `producer_ids`).
  - My seven non-cite forms are refused with exit 2: `/abs/path.rs:5`, `noext:5`, `a.b/c:5`, `.hidden:5`, `file:///a.rs:5`, `a.rs:`, `a.rs:5-`.
  - Four good cites pass with exit 0, and the cites the live `rule-ends.json` holds pass.
  - New: a bare host name with a port, `loom: see example.com:443`, is accepted as `file.ext:line` (red case, `driver/tests/adv_b4.rs`, exit 0 where 2 is meant). It stays open as a note.
- **F-B: fixed.**
  - The mapping hides no real difference. Mutants run on a scratch copy of the script cut before the experiment records, 11 of 11 as expected:
    - A YAML `from: effect` result for exit 0 set to `weird`: caught.
    - That result key set to `bogus`: caught.
    - An extra `rejected` result: caught.
    - A `from: exit` result for exit 0 renamed: caught.
    - An evidence subject changed: caught.
    - JSON `max_runs` changed: caught.
    - JSON result moved from exit 0 to exit 1: caught.
    - JSON `manual` removed: caught.
    - JSON exit 8 result changed: caught.
    - JSON `handoff` added: caught.
    - The one that passes is YAML key `0` next to `from: effect`, which is the same value as the mapped `performed`.
  - `stale-retry` is gone from the live output (0 lines).
  - `docs/reject-routes.md` equals the live output: "routes: 36 findings, all listed".
  - The counts in the text agree: 13+8+6+4+3+1 not-handed+1 unrecorded-exit = 36.
- **F-C: fixed.**
  - `producer_ids` is green with its three new mutants (unparsable file named, second YAML document, `jsonl` line).
  - Shell scripts are deliberately not scanned, and the doc comment says so.
- **Diff `0ff4325..b91f2b3`.**
  - It is 11 files, all within F-A, F-B and F-C plus the doc, site data and CHANGELOG.
  - `sdc-protocol-site check` is current.
  - The added lines hold no name, email, `/home/` path, `file://` or `[patch]`.
  - All four commits are `b10x-bot[bot]|b10x-bot[bot]`.
- **Tree afterwards.** `git status --porcelain` is empty, and `git apply --check` of `<scratch>/removal/line3-b/adversary/pass-4/cases.patch` succeeds. The patch holds `driver/tests/adv_b4.rs` (65 lines) and one line in `main.rs`.

```findings
[
  {"file": "driver/src/bin/sdc-check-routes.rs", "line": 303, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "has_file_line still accepts a bare host name with a port (`loom: see example.com:443`) as a file:line because `example.com` has an extension; the red case a_host_name_with_a_port_is_not_a_file_and_line in driver/tests/adv_b4.rs shows exit 0 where 2 is meant."}
]
```
