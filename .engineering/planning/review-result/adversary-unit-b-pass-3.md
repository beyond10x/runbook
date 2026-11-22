---
format: aep.planning-md/3
id: review-result:adversary-unit-b-pass-3
kind: review-result
status: active
title: 'Narrow check of unit B (producer ids, old bindings) at 0ff4325: red, 3 findings'
summary: N1, N4 and N5 of pass 2 fixed, N2 in part; 3 introduced (a web address with a port passes as file:line; the questions.answer copy lacks what it produces, so the routes check prints one false stale-retry finding; the producer scan reads only files that parse).
tags:
- wave-2026-11-22-w1
relations:
- reviews: story:driver-runtime-removed
revision: 1
---
```
unit: B (producer ids, old bindings) at 0ff4325, narrow check
verdict: red
cases: executed 412→414, red 2
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: <scratch>/removal/line3-b/adversary/pass-3/ (cases.patch, mut.sh, pidmut.sh, logs, tree/, scan/, pid/, files.txt)
needs-coordinator: no
```

The `before` figure is the test count of the `main` binary from nextest's skip count. I ran only the filtered set.

**N1: fixed.** My five red cases and the seven `runtime_block` refusal cases pass. I tried five more value forms and four were refused with exit 2: `["Write"]`, a `null` flag, `[]` next to `loom:`, and `a:1-2-3` / `:5` / an Arabic-digit line. A `runtime` that is an array is also refused with no stdout. One form is not refused (new finding F-A). `check-bindings.sh` on a scratch copy gave clean 0, a `retry_when` finding 1, and a refused file 3 with `REFUSED routes`.

**N2: partly fixed, with a new regression.**
- The twins now equal in `retry_when`, and `check-bindings.sh` goes red when they differ: 4 mutants caught, each exit 1, with a message naming action and both values.
- The "Check output" block equals the live output (37 findings), and the counts in the text agree: 13+8+6+4+3+1+1+1.
- Adding `retry_when` to the `questions.answer` twin created a false finding, `stale-retry sdc.intake questions.answer: retry_when names answer_applied, which no bound action produces` (`docs/reject-routes.md:206`). The twin has no `produces` or `results` for that action, while the YAML records `answer_applied` with results `{"0":null,"1":"refused"}` (F-B).

**N4: fixed for the named case, but the scan got narrower.**
- Prose after the word `producer` no longer fails the test, in JSON or YAML.
- All four of my mutants are still caught, and `producer_ids` has its own four-mutant case on real files.
- The line scan is gone entirely. A `producer` of `sdc-drive` in a `.jsonl` file or a `.sh` file passes, and so does an unparsable or multi-document YAML file, because a failed parse silently gives no ids (F-C). All three scanned dirs parse today.

**N5: fixed.** `backstop` is gone from `rule-ends.json` and from the known members. A file that still has it is refused with exit 3 and the message `member backstop is not one the check knows`. At Loom `70b0e6e0`, `retry.rs:4-8` is two whole sentences and `retry.rs:8-10` is a whole sentence. `docs/reject-routes.md` gives `lib.rs:84`, the constant itself.

**Diff `802e9f5..0ff4325`.** 14 files, all inside the four findings plus the doc, site data, change log and `check-bindings.sh`. `sdc-protocol-site check` says "current". The added lines hold no name, email, `/home/` path, `file://` or `[patch]`. All four commits are `b10x-bot[bot]|b10x-bot[bot]`.

**Cases** (`<scratch>/removal/line3-b/adversary/pass-3/cases.patch`, `driver/tests/adv_b3.rs` plus one line in `main.rs`)
- `a_web_address_is_not_a_file_and_line`: red. `loom: see https://example.com:443` is accepted with exit 0.
- `no_stale_retry_names_a_kind_the_yaml_binding_produces`: red. The live output contains the `stale-retry` line above.
- `five_more_value_forms_are_refused`: green.
- Each takes 0.03 s or less.
- Run: 23 run, 21 passed, 2 failed.

**Tree afterwards.** `git status --porcelain` is empty and `git apply --check` succeeds.

```findings
[
  {"file": "driver/src/bin/sdc-check-routes.rs", "line": 301, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "has_file_line accepts any token path:N, so `loom: see https://example.com:443` passes as a file:line (a port number read as a line); the red case a_web_address_is_not_a_file_and_line in driver/tests/adv_b3.rs shows exit 0 where 2 is meant."},
  {"file": "driver/examples/sdc/sdc-intake.bindings.json", "line": 38, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Adding retry_when answer_applied to the questions.answer twin without its produces or results makes the route check print `stale-retry sdc.intake questions.answer ... which no bound action produces` (docs/reject-routes.md:206), a finding that is false for the YAML Loom runs, which records answer_applied there."},
  {"file": "driver/tests/producer_ids.rs", "line": 51, "category": "mutant", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "With the line scan removed, the scan reads only JSON and YAML that parse, so a producer sdc-drive in a .jsonl or .sh file, or in a YAML file that no longer parses (multi-document), passes; nothing in the scanned directories has one today (shown on a compiled scratch copy)."}
]
```
