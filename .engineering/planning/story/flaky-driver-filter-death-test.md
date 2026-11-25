---
format: aep.planning-md/3
id: story:flaky-driver-filter-death-test
kind: story
status: active
title: The driver cases of a filter that dies kill the filter they name on every run
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-25T11:09:43Z", actor: "agent:runbook", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-25T11:09:43Z", actor: "agent:runbook", revision: 3}
---
## Outcome

The driver test `gateway_readers_and_filter::guard_the_stderr_filter_that_dies_after_writing_leaves_no_file`
(`driver/tests/gateway_readers_and_filter.rs:240`) passes on every run of the gate, and so does its sibling
`guard_the_events_filter_that_dies_after_writing_leaves_no_file`: each case kills the filter it names on every run,
whatever order the adapter's two filters start in. A flaky test is not settled by a further green run.

## The failure seen

Forge CI on `main` 20dee431, job `Build documentation`, step "Test the driver crate with the local gate's task".
The same job passed on 5baa102e; the merge in between touched no driver file.

```
Summary [  54.986s] 929 tests run: 928 passed, 1 failed, 3 skipped
FAIL [   0.119s] (432/929) sdc-tools::main gateway_readers_and_filter::guard_the_stderr_filter_that_dies_after_writing_leaves_no_file
thread 'gateway_readers_and_filter::guard_the_stderr_filter_that_dies_after_writing_leaves_no_file' (139438) panicked at tests/gateway_readers_and_filter.rs:240:5:
assertion `left == right` failed: {
  "adapter": "claude-cli",
  "exit": 0,
  "error": "no readable output"
}
  left: Some(0)
 right: Some(127)
```

The adapter ended with exit 0 where 127 was wanted: no death of a filter was seen.

## The reading to check first

The stand-in `jq` of `dying_jq` (`driver/tests/gateway_readers_and_filter.rs:219`) picks the filter that dies by
the order the two `jq --rawfile` filters start (`mkdir <counter>.lock`: the first starter is number 1). The adapter
(`bindings/tools/llm-adapter-claude-cli.sh`) starts the events filter and the stderr filter as two process
substitutions that run at the same time, so their start order is not fixed, and victim 2 is the stderr filter only
when it starts second.

## Acceptance

- A loop that makes the case fail on demand (the start order forced, or the run repeated), shown red before the fix.
- The stand-in picks the filter that dies by what the filter is (the file it writes or reads, or its arguments),
  never by start order; `filter_death(1, ...)` kills the events filter and `filter_death(2, ...)` the stderr filter
  on every run, and each case asserts which filter died.
- After the fix the same loop is green at the same count, and the driver tests, clippy of `driver/` and
  `task check` pass.
- No product code changes for this flaky test unless a deterministic test that fails first shows a real defect of
  the adapter; such a defect is reported before the adapter is changed.
- No timeout is raised as the fix (`AGENTS.md` § Tests stay fast).
