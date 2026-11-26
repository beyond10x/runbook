# The bindings in Loom's format (`bindings/`)

Step 1 of the move of Runbook onto Loom (`story:loom-bindings-files`): the nine bindings files of
`driver/examples/sdc/*.bindings.json` written again in Loom's `loom.bindings/1`, one file per
protocol under `bindings/`. The JSON files stay: the documentation site's data is still made from them. At this step nothing ran the YAML files; `runbook run` runs them now (`docs/loom-run.md`).
Step 2 (`story:loom-tools-program-contract`) moved the tools both formats call to
`bindings/tools/` and gave them Loom's program contract beside the driver's ("Tool changes").

The format is the one Loom's loader reads at Loom commit `8a8fad3b` (`../loom`,
`crates/loom-bindings/src/decode.rs`, `crates/loom-bindings/src/load.rs`,
`ess/domains/bindings.yaml`). The differences between the two formats are numbered `D1` to `D39`
in the Loom session's comparison of the two formats (2026-11-21); this page cites those numbers.
What the format cannot express yet is listed in `bindings/PENDING.md`, never written in invented
syntax. The bindings ran under Loom commit `5af3f9c2` in the identical comparison with the driver
(`stub-cycle`, 2026-11-22; `docs/parity.md`, "Identical comparison on Loom `5af3f9c2`"), after the
first equal one at `2c0a6f48` ("First equal comparison").

## Files

| Protocol | YAML | Registration | Bindings | Unbound (as in the JSON) |
|---|---|---|---|---|
| `sdc` | `bindings/sdc.yaml` | `sdc@8` | 4 | none |
| `sdc.intake` | `bindings/sdc-intake.yaml` | `sdc-intake@9` | 10 | none |
| `sdc.direction` | `bindings/sdc-direction.yaml` | `sdc-direction@7` | 9 | `context.change` |
| `sdc.epic` | `bindings/sdc-epic.yaml` | `sdc-epic@5` | 6 | none |
| `sdc.spec` | `bindings/sdc-spec.yaml` | `sdc-spec@5` | 9 | `spec.reuse`, `spec.reconstruct` |
| `sdc.plan` | `bindings/sdc-plan.yaml` | `sdc-plan@7` | 8 | none |
| `sdc.story` | `bindings/sdc-story.yaml` | `sdc-story@7` | 7 | none |
| `sdc.release` | `bindings/sdc-release.yaml` | `sdc-release@4` | 5 | `release.rollback` |
| `sdc.bugfix` | `bindings/sdc-bugfix.yaml` | `sdc-bugfix@1` | 10 | none |

Every action bound in a JSON file is bound in its YAML file: 68 bindings.

## Field mapping

| JSON (`driver/examples/sdc/*.bindings.json`) | `loom.bindings/1` | D |
|---|---|---|
| one object keyed by action id | `actions:`, a list; each binding names its `action` | 1 |
| (none) | `format: loom.bindings/1` | 2 |
| (the protocol file given to `init --protocol`) | `protocol: <name>@<revision>`: the protocol id with `.` and `_` as `-`, then the revision of `protocol/<name>.protocol.yaml` (`sdc@8`), as Loom's catalog registers it | 3 |
| (none) | `program_root: bindings` on every file that binds a program: a program path with a `/` is relative to the directory of the bindings file | 13 |
| `$artifacts` | `artifacts` | 4 |
| `$independence` | `independence` | 5 |
| `argv` | `executor: {kind: program, argv}` | 6 |
| `produces`, `subject`, `producer`, `upstream`, `results` on the binding | `evidence:`, one entry per kind with `kind`, `subject`, `producer`, `from`, `results`, `upstream` | 6 |
| `produces: [a, b]` with `results` exit → `{a: r, b: s}` (`questions.check`) | two entries over the same exit, each with its own table | 7 |
| a table keyed by exit code | `from: exit` | 8 |
| `producer: "{principal}"` | `producer: person:owner` (the approving principal replaces it at run time, `story:tree-run-command`) | 9 |
| `manual: true` | `executor: {kind: person, principal: person:owner}` | 10 |
| `manual: true` with `argv: ["true"]` | dropped: `true` does nothing; a `person` executor runs no program | 11 |
| `"6": "rejected"` on a manual binding | pending (`bindings/PENDING.md`) | 12 |
| `{dir}/tools/<tool>` (since step 2 `{dir}/../../../bindings/tools/<tool>`) | `tools/<tool>`, relative to `bindings/` (`bindings/tools/`); no `..`, and a program without a `/` comes from `PATH`. The file names `program_root: bindings`, so Loom (at `af8de2c9`) reads the path beside the bindings file; `scripts/check-bindings.sh` refuses a file that binds a program without it | 13 |
| `{work}` in `handoff` | "of the workspace"; the placeholder is pending | 14 |
| `handoff` | `handoff` | 16 |
| `handoff_when` | `handoff_when`: the argv `tools/sdc-tool.sh findings-wait --action amend.upstream`, run by Loom before the writer (exit 0 offers the handoff, another exit lets the writer run); the old driver reads the JSON file and never this one | 17 |
| `llm: {model, max_turns, max_budget_usd, stub, operations}` | argv of `sdc-node` (the model step since step 2): `--model`, `--max-turns`, `--max-budget-usd`, `--operations`, `--instructions <json>`, `--stub <argv>`; the `agent` executor is pending: a `program` executor takes no agent member, the agent text is in `bindings/PENDING.md` | 18-22 |
| `retry_when` | `retry_when` | 23 |
| `max_runs` | `budget: {max_runs}` | 24 |
| `timeout_s` | `timeout_seconds` (see "Timeouts") | 25 |
| `copy: {kind, from: sdc.direction}` | `executor: {kind: copy, evidence: <kind>, from: sdc-direction@7}`, the registration of `protocol/sdc-direction.protocol.yaml` | 26 |
| `child: {name \| each, bindings}` | `executor: {kind: open, name \| each, bindings: <file>.yaml}` | 27, 29 |
| `child.protocol` (a file path) | dropped: the child's protocol is the name the action's `case_open.protocol` declares (`protocol/sdc.protocol.yaml:96-98`), resolved in the catalog `tree open --protocols <dir>` builds from the `*.yaml` files of that directory (Loom wave 2); the loader at `8a8fad3b` asks for nothing in its place | 28 |
| `child.revisions` | pending | 30 |
| opening `results: {"0": null}` | `from: effect`, `results: {performed: null}` | 31 |
| `args_from` | `arguments_from` | 32 |
| a manual binding's `subject` with no `produces` (`scope.answer`) | pending | 38 |

The JSON files hold no `arguments` and nothing else that is the same in both formats.

## Timeouts

Loom's default is 60 s (`../loom/crates/loom-intake-slice/src/bindings.rs:69`); the former driver's was
1800 s (deleted with it). Each binding with a `timeout_s` keeps its value. A binding
without one gets `timeout_seconds: 1800`, today's effective value, when its program calls `ess`
beyond `ess specify validate`, `cargo` or `slot` (a compile, a synthesis, a test suite or a
build). The others are checks over files with `yq`, `jq`, `aep`, `grep`, `cp`, `cmp` or an
`sdc-findings` or `sdc-bugfix` check, which end in seconds, and keep Loom's 60 s. A `person`,
`open` or `copy` binding runs no program and names none; an `arguments_from` argv (`list-epics`,
`list-stories`, `spec-domain`: `aep` or `yq` over files) runs under its binding's timeout, at most 3600 s.

The limits (Loom 0.61.0, `story:bound-program-timeout-above-an-hour`; Loom refuses the bindings file at
load past them, `TimeoutOutOfRange`):

- A `program` binding's `timeout_seconds` is from 1 to 14400 (`MAX_PROGRAM_TIMEOUT_SECONDS`). Past
  3600 s the program gets 3600/`timeout_seconds` of one core (a 14400 s step gets a quarter of a core),
  so a longer limit buys waiting time, not more CPU time than an hour of one core.
- Every other executor keeps 1 to 3600 (`MAX_TIMEOUT_SECONDS`); Loom's words past it: `<n> is not from
  1 to 3600; only a program executor may name up to 14400`. A program past 14400: `<n> is not from 1 to
  14400, the range of a program executor`.
- A binding's `handoff_when` check and its `arguments_from` program run under the binding's
  `timeout_seconds` but at most 3600 s, whatever the binding names; `LOOM_TIMEOUT_SECONDS` tells each
  the limit it runs under. Loom does not refuse a `handoff_when` beside a `timeout_seconds` above 3600:
  the check keeps the hour.
- `runbook check` names a `timeout_seconds` past its executor's range in Loom's words
  (`runbook/src/check.rs`, `timeout_defect`). The cases: `check::tests` in `runbook/src/check.rs` and
  the two `inproc` cases of `runbook/tests/loom_run/inproc.rs` (a program at 14400 runs and its
  `handoff_when` check gets 3600; 14401 and a person step at 3601 are refused at the opening).

A run killed at its timeout gives the value `timeout`, and from Loom 0.61.0 only a `timeout` row of
the table matches it, never `*` (`story:wildcard-result-row-does-not-match-timeout`): a table with a
`*` row and no `timeout` row ends such a run `ResultNotInTable`, and the case may stop `Stuck`. Every
program binding of `bindings/*.yaml` and of the test fixtures whose table has a `*` row has a `timeout`
row with the result `*` gave before (`failed`, `invalid`, `open`, ...), and the JSON copies in
`driver/examples/sdc/` name it as well, so `scripts/check-bindings.sh` compares equal tables; that
script takes `timeout` as a key of a `from: exit` table beside the exit codes and `*`.

| Value | Bindings | Why |
|---|---|---|
| 1800 | `spec.gate` (epic, plan, story, release) | `ess-gate`: compile, synthesis, interpreted suite (`bindings/tools/sdc-tool.sh:238-243`) |
| 3000 | story `tests.run` | `story-tests` with `--pool-wait 1200`: wait 1200 + 2 pool jobs x 600 + 600 of other work; the limit covers the queue wait and the run (`docs/driver.md`, "The build pool wait") |
| 2400 | release `acceptance.run` | `conform-run release` with `--pool-wait 1200`: 1200 + 1 job x 600 + 600 |
| 1800 | spec `spec.compile`, `spec.synthesize`, `spec.trace`; plan `plan.cover` | `ess specify compile` or the interpreted suite (`sdc-tool.sh:245`, `:248`, `:418-427`; `sdc-findings` `gate spec` compiles, `driver/src/bin/sdc-findings.rs:590`) |
| 3000, 3000, 4200 (4200 in `bindings/sdc-bugfix.yaml` since Loom 0.61.0; it was held at 3600 while Loom refused more) | bugfix `repro.run`, `tests.run`, `gate.run` | `--pool-wait 1200` with 2, 2 and 4 pool jobs of 600 and 600 of other work; the limit covers the queue wait and the run (`docs/driver.md`, "The build pool wait") |
| 1500 | spec `spec.author`, story `change.implement`, bugfix `change.fix` | `timeout_s` of the JSON; agent nodes |
| 900 | the other agent nodes: intake 4, direction 3, plan `plan.decompose`, story `change.review`, bugfix `test.reproduce` | `timeout_s` of the JSON |
| 600 | bugfix `bug.dissect` | `timeout_s` of the JSON |
| 300 | every `amend.upstream` (intake, direction, spec, plan, story) | `timeout_s` of the JSON |
| 60 (default) | intake `idea.check`, `dissection.check`, `questions.check`, `requirements.check`; direction `context.check`, `roadmap.validate`; spec `spec.validate`, `spec.scan`, `spec.review`; plan `plan.validate`, `plan.scope`, `plan.status`; story `change.merge`; release `release.build`, `release.publish`, `release.observe`; bugfix `dissection.check`, `story.update` | checks over files and copies |

## The budget

`budget: {max_runs: 3}` bounds the runs of an action in a case: the run after the third is refused
(`RunsSpent`). A run that ends in an exit of `exits` (the error and not-called classes) is not counted.
Three more members say what a rejection does to that count (Loom `d8944d49`,
`story:rework-allowance-after-a-rejection`). They are read on a program or a person's action only; on
an `agent`, `open` or `copy` action Loom refuses them (`MemberNotForExecutor`).

| Member | What a person writes | Default |
|---|---|---|
| `runs_per` | `case`: every run of the action in the case counts against `max_runs`. `rejection`: only the runs since the latest rejection count, so a rejection gives the action a new allowance of `max_runs` runs | `case` |
| `rework_limit` | a whole number from 1: how many rejections the action answers. The run after more rejections than that is refused | 2 |
| `rework_on` | a map of evidence kind to results, written as `retry_when` is: the records that are a rejection, and no others. Each kind and result has to be in `retry_when` of the same action | absent: every `retry_when` entry that another action's record matches is a rejection |

A rejection is a record of the case that `rework_on` lists (or `retry_when`, when `rework_on` is
absent) made by anyone: the decider (`decider:<kind>`), a rule (`rule:<name>`) or a person. A record
that cites a run or an approval of the action itself is not one (an approval record of the person
action opens no window). The window is the runs of the action since the latest rejection. A failed
check or a refused coverage that sends the action back through `retry_when`, and is not in
`rework_on`, stays inside the window: it opens none.

Loom refuses at load (`InvalidBudget`, naming the member): a `runs_per` that is neither `case` nor
`rejection`; a `rework_limit` that is not a whole number from 1; `rework_limit` or `rework_on` with
no `runs_per: rejection`; `runs_per: rejection` where `retry_when` lists no result ("lists no result":
no record could open an allowance); an empty `rework_on`; a `rework_on` kind with no result; a pair
`rework_on` lists that `retry_when` does not list. `runbook check` and `scripts/check-bindings.sh`
(`--budget <root>`) refuse the same, in Loom's words, before a run does.

A refused run is `RunsSpent`, in one of two texts: "`<action>` was rejected `<k>` times in case
`<c>`, the latest `<evidence id>` by `<producer>`; its budget.rework_limit is `<n>`" (the rework
limit, tested first) and "`<action>` has run `<n>` times in case `<c>` since the rejection
`<evidence id>` by `<producer>`, its budget.max_runs is `<m>`" (a full window; without `since` when
no rejection was made). `runbook run` ends such a run as any stuck tree (`docs/loom-run.md`, "The
runs of an action are spent").

`change.implement` of `bindings/sdc-story.yaml` is bound with `max_runs: 3`, `runs_per: rejection`,
`rework_limit: 2` and `rework_on: {code_review: [rejected], merge_approval: [rejected]}`: the review
and the merge approval send the writer back and each gives it three more runs; a failed
`conformance_run` and a refused coverage (`implementation_change: [coverage_refused]`) are in
`retry_when` only, so they use the runs of the window; the third rejection ends the run (the writer
runs once and after each of the first two rejections). The value 2 follows Loom's rules for the two
entries of `cycles/tree-suite.yaml` that hold it (`decider-rework-spent`: three rejections, then exit 4;
`rework-fresh-allowance`: two rejections, then four writer runs and exit 0): at 2 the third rejection is
the last, at 3 a fourth run follows it. No run has told 2 from 3: both entries ended the same way at both
(before the change of the stub) and, after it, on Loom's 60 s bound in every run, three runs each at
a one-minute load of 45 to 75 among them, before `change.implement` ran (`docs/cycles-suite.md`). Both
stay `pending`; nothing here says a run settles them.

## The per-call budget of a program

A run reserves, before each bound program starts, what that program may spend on models
(`docs/daemon-api.md`, "Caps of a run"; `runbook run`: `docs/loom-run.md`, "The spend ledger"). A
program that takes the option names it in its argv (`--max-budget-usd <v>` or
`--max-budget-usd=<v>`). A program that does not take it (a check, a deploy, a script that calls no
model) gets it from its binding, never through its argv:

| Member | Where | What it says |
|---|---|---|
| `max_budget_usd: <number>` | a program action (one with `executor.argv` or `arguments_from`) | the per-call budget, in USD, of that action's programs; `0`: they call no model |
| `max_budget_usd: <number>` | the top of a bindings file | the same for every program action of that file that names none itself; not for a child file an `open` action names (each file names its own) |

The order a program's budget is read in: `--max-budget-usd` in its argv, the action's member, the
file's member, and else the largest budget the runbook's bindings name (in an argv or as a member).
`runbook run` lowers an argv's budget to what its cap has left and leaves a member as written: a
member is reserved as written (`runbook/src/budget.rs`).

Refused at load, naming the action (or the file, for the file's member): a value that is no YAML
number of 0 or more (text such as `"0.3"`, `-1`, `.nan`, `.inf`, `null`, `true`, a list); an action
whose argv and member name two different budgets; a member on an action that runs no program.
`runbook check` names the same. The member is Runbook's, not Loom's: Loom refuses a member it does
not know (`UndeclaredMember`), so a run takes the members out of its copy of the bindings and keeps
them beside it (`bindings/.call-budgets.json`, `budget::seal`); the files of the home keep them.

## A command's deadline, retry and poll (`sdc-command`)

A bound program runs one command with a deadline, a retry or a poll through the helper `sdc-command`
(`driver/src/bin/sdc-command.rs`). It is built with the driver and installed in `libexec/runbook/` beside the
other helpers, which the `runbook` of an installed prefix puts first on the `PATH` of the programs it starts
(`scripts/install.sh`, `runbook/src/assets.rs`, `path_with_libexec`); in a checkout it is `driver/target/debug/sdc-command`.
A binding names it as the first word of a program's argv, so its keys are that step's own; nothing of it is a
setting of the run or the daemon.

| Key | What it says |
|---|---|
| `--deadline <time>` | the time of the whole step from the helper's start: the read of its standard input, every try and every wait between. At the deadline the try that runs is ended together with every process it started (SIGTERM to every process below the helper, SIGKILL 1 s later) and the helper exits 124. Under Loom with no `--deadline`, the step's time less 2 s |
| `--retry <n>` | at most `n` tries after the first |
| `--retry-delay <time>` | the wait before the next try: the poll interval (default 0) |
| `--retry-on <code>[,<code>...]` | the exits that are tried again (state `pending`); without it, with `--retry`, every exit other than 0. With `--retry-on` and no `--retry` the tries go on until the deadline, and `--deadline` is required |
| `--stop <code>=<name>` | an exit that ends the step at once with the outcome `<name>` (repeatable) |
| `--guard <program> [<argument>...]` | a command run before each try, up to the `--` before the command; an exit other than 0 ends the step before the command runs |
| `--guard-stop <name>` | the outcome of a step the guard ended (default `guard-stopped`) |
| `--keep-tries <n>` | how many tries the report holds, the last ones, 1 to 1000 (default 100); the others are counted |
| `-- <program> [<argument>...]` | the command |

A time is `<n>ms`, `<n>s`, `<n>m` or `<n>h`; a plain number is seconds. A name is `a-z`, `0-9`, `-` and `_`,
and none of the helper's own states (`done`, `pending`, `failed`, `deadline`, `refused`, `none`,
`guard-not-started`).

Each try gets a state from its exit: `done` (0), `pending` (tried again), the name `--stop` gives the exit, else
`failed`; a try ended at the deadline is `deadline`, a try the guard ended has the guard's name and the guard's
exit, and a try whose guard could not be started is `guard-not-started`. The helper's exit:

| Exit | Outcome |
|---|---|
| 0 | `done` |
| the command's own | a `--stop` name, `failed`, or `failed` with the text `retries spent after <n> tries, last state pending` |
| 1 | as the line above, for a command whose own exit is 64, 124, 125 or 127: the helper never passes on one of its own exits. The try keeps the command's exit, and the text adds `the helper exits 1, as <code> is one of its own exits` |
| 124 | `deadline`, with the text `deadline after <s> s, last state <state>`: the state of the last try that ended (`none` when none did, also when the deadline came while the helper still read its standard input) |
| 125 | the `--guard-stop` name |
| 127 | `failed`: the command could not be started; `guard-not-started`: the guard could not be started (the command did not run) |
| 64 | `refused`: a command line the helper does not take; nothing ran |

Standard output is one JSON object, the report: `format` (`runbook.command/1`), `outcome`, `text`, `exit`,
`deadline_s`, `elapsed_ms`, `tries_total`, `tries_dropped` and `tries`, one object per kept try with `try`, `at`
(seconds since the Unix epoch), `ms`, `exit` (null when the command did not end by itself or did not run),
`state`, `deadline`, and `guard` on a try the guard ended. What the command and the guard print goes to standard
error. Loom keeps a program's standard output and standard error as the step's blob in the run's store (the
`ActionFinished` line of the run log names it), so the report is the step's record of its tries. A binding reads
the outcome with `from: /outcome`, the pointer into the program's report. Under Loom's program contract the helper
reads its standard input (Loom's argument object, at most 1 MiB) once and gives it to every try and every guard;
a standard input that does not end before the deadline ends the step at the deadline with no try.

The deadline is held under the step's own time: Loom tells a bound program its limit in `LOOM_TIMEOUT_SECONDS`
(the binding's `timeout_seconds`), and a `--deadline` that leaves less than 2 s of it (the time to end a command
and write the report) is refused by name, exit 64, before anything runs: `--deadline 10s is longer than this
step's 5 s (LOOM_TIMEOUT_SECONDS) less the 2 s ...`. It is never cut to fit. With no `--deadline` the helper takes
the step's time less those 2 s as its deadline (`deadline_s` of the report names it); a step of 2 s or less leaves
none and is refused, exit 64.

What a try starts ends with it. The guard and the command run in the helper's own process group, and the helper is
the child subreaper of what they start (`PR_SET_CHILD_SUBREAPER`): when a try ends, by itself or at the deadline,
the helper kills every process still below it, also one that made a group or a session of its own (`setsid`).
When the runner ends the step by killing the helper's process group (Loom under `local`), that kill reaches the
try, which is in the group, and the program the helper started gets SIGKILL when the helper ends
(`PR_SET_PDEATHSIG`). A process that the command moved out of the group and that outlives the command is not
reached by a SIGKILL of the helper itself: only the helper's own end of a try kills it.

A program the helper runs, the command or the guard, is found on `PATH` or by an absolute path. Loom resolves
only the first word of a binding's argv beside the bindings file (`program_root: bindings`); a `tools/<file>`
further on is a path in the workspace, where the program starts. A command of a runbook's own `bindings/tools/`
behind the helper needs Loom to resolve such a word too, or to tell a program the directory of its bindings; it is
not built.

The cases: `driver/tests/command_helper.rs` (the deadline that leaves no process, the retry, `--retry-on`, the
refusal, the poll, the guard, the bound of the record, the refused command lines, the helper's own exits, a guard
that cannot start, the deadline a step limit gives) and `driver/tests/adversary_w6_command.rs` (a kill of the
helper's group, no deadline under a step limit, a process in a session of its own, a standard input left open).

### A long step

One action starts the work and records that it did; a second action polls the work with `sdc-command` under its
own `--deadline`, and the binding's `timeout_seconds` is the total bound above it (at least the deadline and 2 s);
the protocol names an outcome for each state the poll can end in. A poll that never passes ends at its deadline
with the outcome `deadline`, and the evidence of the first action stays.

The example is the runbook directory `examples/long-step/` (protocol `long-step@1`, its bindings, and the four
cycles of `examples/long-step/long-step.suite.yaml`: a poll that passes on its third try ends `green`, one that
stays pending ends `deadline` after 2 s, one that exits 1 ends `red-checks`, and a guard that fails ends
`head-moved`). The poll command there is a sh stand-in that a seeded `checks-plan.txt` steers. The cycles run in
`runbook/tests/test_suite/long_step.rs`, with the built helper first on `PATH`; by hand:
`PATH="$PWD/driver/target/debug:$PATH" RUNBOOK_HOME=examples/long-step runbook test
examples/long-step/long-step.suite.yaml --confinement local`. `runbook check --for long-step` accepts the
directory as a configured runbook (`runbooks = ["examples/long-step"]` in `runbook.toml`).

### Merge on green (an example)

A step that waits for the required checks of a pull request and merges nothing itself; the repository and the
head are placeholders. `gh pr checks --required` exits 0 when every required check passed, 8 while one is
pending and 1 when one failed.

```yaml
  - action: checks.wait
    timeout_seconds: 1900
    executor:
      kind: program
      argv: [sdc-command, --deadline, 30m, --retry-on, "8", --retry-delay, 30s, --stop, 1=red-checks,
             --guard-stop, head-moved,
             --guard, sh, -c, 'test "$(gh pr view 123 --repo example/repo --json headRefOid --jq .headRefOid)" = "<head of the intake>"',
             --, gh, pr, checks, "123", --repo, example/repo, --required]
    exits: {error: [75, 126, 127]}
    evidence:
      - kind: checks_result
        subject: pull_request
        producer: checks-wait
        from: /outcome
        results: {done: green, red-checks: red, head-moved: moved, deadline: late, failed: failed, timeout: failed, "*": failed}
```

## Written in the YAML, carried out by Loom later

These constructs load today; Loom runs them once the story named lands (waves as in Runbook's
proposed order of Loom's open stories, 2026-11-21).

- `copy` (`roadmap.read` in `sdc`, `sdc-epic`, `sdc-spec`, `sdc-plan`): `story:record-copy-between-cases`, wave 1. `from` is the registration, as in Loom's guide (`../loom/website/docs/guides/bindings.md:138-142`).
- `open` with its `from: effect` entries, the child's `independence`: `story:child-case-runtime`, wave 1.
- `arguments_from` on an `open` binding (`epics.open`, `stories.open`): `story:tree-run-command`, wave 2.
- `handoff`: `story:tree-run-command`, wave 2.
- A `person`'s record produced by the approving principal, not the literal `person:owner`: `story:tree-run-command`, wave 2.
- `budget.max_runs`, counted since the case's last arrival as the driver does (D24): `story:run-caps-per-arrival`, wave 6.

`retry_when` and the feedback to the writer are not in this list: the pinned Loom (`d8944d49`) runs
them. On a step with retry records it sets `LOOM_RETRY_FEEDBACK` on the bound program's child to the
path of a `loom.retry-feedback/1` document; with no retry record it sets nothing
(`crates/loom-bindings/src/retry.rs:14-17`, `:34`, `:36` of Loom).

## Pending (`bindings/PENDING.md`)

| Loom work | Wave | What it brings |
|---|---|---|
| G1 bound program contract | 2 | argv relative to the bindings file's directory (D13); program-error exits (D36), not-called runs (D37) and the pool-wait exits are stated in the bindings, see "The exit lists" |
| G2 person actions | 3 | done at Loom `70b0e6e0`: a program on a person binding with the principal as argument (D11, D15; `scope.answer` in `bindings/sdc-intake.yaml`), `rejected` with reasons (D12); a subject with no evidence (D38) needs no member: the binding names none |
| `story:bound-agent-node` | 4 | `agent` executors for the 14 agent nodes: adapter, model, `max_turns`, `max_budget_micro_usd`, `tools.allow` (D18-D20, D22) |
| N8 | 4 | `{work}` in `handoff` (D14) |
| N3 fixture adapter | 5 | the stub of each agent node (D21) |
| G4 | 10 | initial child revisions (D30), only if step 3 shows `sdc.epic` needs them |

The exit lists (Loom `70b0e6e0`, `exits: {error: [...], not_called: [...]}`; Loom ships no default
list): each of the 52 program bindings of `bindings/*.yaml` states `exits.error`, `[75, 126, 127]`
(the former driver's `program_error`), and each of the 14 model-node bindings
(the `sdc-node` actions) also `exits.not_called: [79]`. An exit in `error` ends the run
`ProgramError`, the run log holds no result for the node, and the run is not counted against
`budget.max_runs`; an exit in `not_called` records nothing, the step is performed, and the run is
not counted either. `scripts/check-bindings.sh` refuses by name an `exits` class other than these two, a class that is not a list and an `exits` that is not a mapping, as Loom's decoder does. `sdc-node` ends 79 where it prints "not called" and writes nothing
(`questions.write` with an interview, a stage agent while a finding waits for a person); the one
"not called" that stays exit 0 is a derived tree that is only renewed (`derived_skip`): its
renewed `.derived-from` is what moves the case, and a run that records nothing leaves the case at
its revision (`spec-own-default` of `cycles/tree-suite.yaml` ended Stuck with 79 there). That
renewal of `spec.author` exits 0 and so counts as one of its `budget.max_runs`. A node
that is not called while it stays due (`retry_when`) was taken again at once at Loom `db2726ff`
(Loom counts no run of it); at `c9b4c177` (`story:writer-not-called-leaves-the-due-set`) it leaves
the due set after its not-called exit until the case is evaluated again for a reason other than its
own moves, so it starts once per evaluation and the run goes on to the next useful action or to the
handoff check; a not-called writer that moves an artifact, and a writer of evidence on that artifact,
still alternate until the step or the cycle's time bound (at `d8944d49` a set-aside action is idle in the due set, the second part of the fix; whether this alternation is gone is not observed here, the entries that show it were not run again). The test of the state of run 1 of record
`parity-loom-strong-4` is `runbook/tests/test_suite/notcalled.rs`; see `spec-person-wait` and
`cite-fault-sent-back` in `docs/cycles-suite.md`. The bindings that wait for the build pool (story `tests.run`, release
`acceptance.run`, bugfix `repro.run`, `tests.run`, `gate.run`) list `[69, 70, 75, 126, 127]`: 69 is
no slot within `--pool-wait` (nothing ran), 70 the pool ended a started job at its run limit ("ran
over its time"). The tool exits 69 and 70 in both modes; under Loom neither maps to `failed` any
more.

`scripts/check-bindings.sh --exits <root>` runs the exits check alone over `<root>/bindings/*.yaml`.
The list of the five pool bindings is this repository's: it applies when `<root>/bindings/` holds one of
the files it names (`sdc-story.yaml`, `sdc-release.yaml`, `sdc-bugfix.yaml`; a link of one of these names
that points at nothing counts), and by nothing else; where the script lies does not count, another
repository may hold a copy of it. A runbook of another repository that happens to hold a file with one of
these names sees the five lines `bindings/<file>:<action>: a binding that waits for the build pool is not in
bindings/` (`sdc-story.yaml:tests.run`, `sdc-release.yaml:acceptance.run`, `sdc-bugfix.yaml:repro.run`,
`tests.run`, `gate.run`), pinned by the test
`a_runbook_of_another_repository_with_a_file_named_sdc_story_yaml_gets_the_five_pool_lines`. For any other root,
the runbook of another repository, the option asks nothing about the build pool and checks the root's own
files by the binding itself: every `program` binding states `exits.error`: `[69, 70, 75, 126, 127]` when
its argv holds `--pool-wait` as an element, else `[75, 126, 127]` or `[69, 70, 75, 126, 127]` (the tool
exits 69 and 70 for the environment variable `SDC_POOL_WAIT` too, which wins over the flag); a model node (`sdc-node`) also states
`exits.not_called: [79]`; no code is in both classes or a key of the action's `from: exit` result table,
and `exits` is on no other executor. A binding that lacks it gets one named line
(`bindings/<file> <action>: exits.error is [], want [75,126,127] or [69,70,75,126,127]`); the line a user adds to its program
binding is `exits.error: [75, 126, 127]`. A root with no `bindings/*.yaml` is refused by one line that names
the root. A file of no bytes (`the file is empty`), a link named `*.yaml` that points at nothing, a file
that is no mapping with an `actions` list, an entry of `actions` that is no mapping or has no `action` name,
an `executor` that is present and no mapping (`executor:` with no value too), an `executor.argv` that is no
list, `evidence` that is no list of mappings, a `results` that is no mapping, and a result key read from
the exit that is no exit code (plain digits for a number from 0 to 255: `+1`, `01`, `256` and `1` with a
final newline are refused) are each refused by one line that names the file (and the action where there
is one), exit 1; the other files are still checked. Of every member the script reads from a YAML bindings
file, an absent or null value counts as absent and any other value must have the type the check needs: `false` is not an empty
list. The full `scripts/check-bindings.sh` refuses the same shapes in the bindings file of a protocol under
`FAIL <name>`, and there also an element of `executor.argv` or of `arguments_from` that is no string, an
`arguments_from` that is no list and a `budget` that is no mapping; its registrations step reads a bindings
file of any shape that `yq` can write as JSON without a message of `yq`, and fails the script when it
cannot read a file. Every member the script reads from the YAML bindings file of a protocol, with each JSON
type in it (null, false, true, a number, a string, an empty string, a list, an empty list, a mapping, an
empty mapping; for text and keys also a final newline, a leading `+`, a leading zero, a number of 26
digits), ends in exit 0 or 1 with a `FAIL` line that names the file, never in a raw message of `jq` or
`yq`; the table is in `driver/tests/check_bindings_exits_value_table.rs`, one test per member. Outside
that, and unchanged from before: YAML that `yq` cannot write as JSON (`.inf`, a key that is a list) ends
the check with the message of `yq`, exit 1, with no `FAIL` line and no file named; a bindings file of more
than one YAML document gives a raw message of `jq` in the full check; the JSON copies under
`driver/examples/` and the protocol files that the registrations step reads are not checked for the type
of their members, and a member of the wrong type there ends the check in a message of `jq` (exit 5). The
backlog story `check-bindings-refuses-every-file-by-name` holds these; three ignored cases in
`driver/tests/check_bindings_exits_unwritable_yaml_json_copies_and_protocol_ids.rs` measure them. Text that
is no YAML ends the check with the message of `yq`, which names the file. A root given as a relative path
may start with a dash. A copy of
this repository's bindings elsewhere is this repository only while it holds one of the three named files.
That this repository keeps them is held by the test `the_three_files_the_pool_list_names_exist_in_this_repository`
and by the full `scripts/check-bindings.sh`, which fails for a protocol whose `bindings/<name>.yaml` is missing.

The not-called code is 79. It is the one code that collides with no result-table key (the tables
name only 0, 1, 3, 5, 8 and 77, the exits of the check tools; 77 is the refusal of the hand-back spec check, `docs/bugfix.md`) and
with no `exits.error` code of any binding (75, 126, 127, 69, 70); Loom refuses a code that is in
both classes or in a class and a `from: exit` table key, and `scripts/check-bindings.sh` checks the
same three things before Loom does. 79 also stays clear of the codes `sdc-node` passes through from
its adapters (64 bad arguments, 75 the model's API ended) and of the shell's 124, 126, 127, 128+n.

## Checks

- `scripts/check-bindings.sh` (CI, `task check`): for each JSON file, its `bindings/<name>.yaml`
  exists, binds every action the JSON binds or `bindings/PENDING.md` has a line
  `` - `<name>.bindings.json:<line>` `<action>` ``, and binds no action the compiled protocol
  (`protocol/compiled/<name>.ir.json`) does not declare. Since step 2 also: no `executor.argv`
  or `arguments_from` entry holds a `..` segment, and every program named with a `/` is
  `tools/<file>`, an executable file under `bindings/tools/`; a program without a `/` comes from
  `PATH`. Also: each file's `protocol:` is the registration of `protocol/<name>.protocol.yaml`
  (the id with `.` and `_` as `-`, then `@<revision>`), the `from:` of every `copy` executor is the
  registration of a protocol file in `protocol/`, and a file that binds a program has
  `program_root: bindings`; a failure names the file and the value. A new revision of a protocol
  file moves its registration here and in every `copy` `from:`. Also the exits (see "The exit
  lists"): `exits.error` is `[75, 126, 127]` (`[69, 70, 75, 126, 127]` where the argv names
  `--pool-wait`), a `sdc-node` action has `exits.not_called: [79]`, and no code is in both classes
  or a key of the action's `from: exit` table. Plain text with `jq` and `yq`.
- The tools' Loom mode: `driver/tests/questions_check.rs`, module `loom_mode`, runs a check
  (`check-questions`, `check-requirements`), `list-stories`, an agent node on the stub and on dry
  (`idea.dissect`, `spec.author`) and a retried writer (`questions.write` with
  `LOOM_RETRY_FEEDBACK`) twice: in driver mode and under Loom's contract (an empty environment but
  `PATH` and `LOOM_CASE`, the arguments on standard input, a fresh workspace), and compares exit
  code, output and the files written.
- The load check, with Loom's own loader, outside this repository (no Rust of Runbook's): a scratch
  binary crate that depends on `b10x-loom-bindings` by path in an export of Loom at `8a8fad3b`
  (`git -C ../loom archive 8a8fad3b Cargo.toml Cargo.lock crates generated protocols`, Loom's
  `Cargo.lock` copied beside the crate) and on `b10x-canon` tag `0.10.0` (the run of 2026-11-21
  below used `0.9.0`). Per file it parses
  `protocol/<name>.protocol.yaml` with `b10x_canon::model::parse`, then calls
  `b10x_loom_bindings::decode` on `bindings/<name>.yaml` and
  `b10x_loom_bindings::check(&document, "<name>@<revision>", &protocol)`, and prints `ok` or the refusal.
  Build it through `slot` (`cargo build --release`; a cold build took 184 s, 44 s of it queued).
  On 2026-11-21 all nine files printed `ok`, at `8a8fad3b` and, after step 2, at Loom's
  integration head `882c1866` (which has no `resolve_programs`, so only the load is checked). Loom's `b10x-loom bindings check <file> --protocol
  <name@major>` (N2, wave 7) replaces it; `b10x-loom` 0.4.0 has no such command.

## Tool changes for Loom's program contract

Step 2 (`story:loom-tools-program-contract`). The tools moved from `driver/examples/sdc/tools/` to
`bindings/tools/`, beside the files that name them: `bindings/*.yaml` names each as
`tools/<file>`, and `driver/examples/sdc/*.bindings.json` as `{dir}/../../../bindings/tools/<file>`.
Each tool has two modes. With `LOOM_CASE` unset it runs as before, in the mode of the former driver. With
`LOOM_CASE` set it reads Loom's program contract: the arguments as one JSON object on standard
input, the case id from `LOOM_CASE`, the epic and the story from `LOOM_PARENT_CASE_NAME` and `LOOM_CASE_NAME` (the names Loom gave the cases; without them the cut of the case id), retry feedback from the `loom.retry-feedback/1` document that
`LOOM_RETRY_FEEDBACK` names, the start directory at the workspace root, and everything the driver
gave in a variable as an argv entry of the binding (`sdc-tool.sh <sub> ... --action <id>`;
`sdc-node <action> --model ... --max-turns ... --max-budget-usd ... [--operations ...] [--instructions <json>] --stub
<argv>`). The tools map these inputs onto the driver's variables and export them, so the programs
they start (`sdc-findings`, `sdc-bugfix`, `sdc-reply`, the adapters) run unchanged. The driver
crate's programs come from `PATH`, else from `driver/target/debug/` of this checkout, found from
the tool's own directory; never from `HOME`. A bad argv entry exits 64.

Status per read: done, or pending with the Loom story in `bindings/PENDING.md` ("Tool environment
under Loom"); a pending read takes the driver's default under Loom. The first column is a string to `grep -n` for in the tool: line numbers of a tool that changes go stale.

`bindings/tools/sdc-tool.sh`

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `cat > /dev/null` | standard input | read and dropped: every subcommand takes its arguments from argv | done |
| `SDC_FINDINGS:-` | `SDC_FINDINGS`, else `driver/target/debug/` of this checkout | `sdc-findings` on `PATH`, else relative to the tool; no `HOME`; not found: exit 127, the message names the binary and where it looked | done |
| `SDC_BUGFIX:-` | `SDC_BUGFIX` | `sdc-bugfix` the same way | done |
| `executor=agent:` | `SDC_ACTION` (the `aep` executor `agent:<action>`) | `--action <id>`, in every `sdc-tool.sh` argv of `bindings/*.yaml` | done |
| `aep_actor=` | `SDC_PRINCIPAL` (`AEP_ACTOR`) | `--principal <principal>` is taken; no binding states one | pending: `story:bound-program-declares-its-environment` (wave 9) or G2 |
| `stub-file)` | `SDC_STUB_FIXTURES` (`stub-file`) | `--stub-fixtures <dir>` is taken | pending: N3 (the stub overlay's binding states it) |
| `SDC_STUB_COUNTER:-` | `SDC_STUB_COUNTER` | `--stub-counter <dir>` is taken (default `.sdc`) | pending: N3 |
| `stub-tree)` | `SDC_STUB_FIXTURES` (`stub-tree`) | as for `stub-file` | pending: N3 |
| `SDC_POOL_WAIT` | `SDC_POOL_WAIT` (the step's wait bound for the build pool) | `--pool-wait <seconds>`, in the argv of story `tests.run`, release `acceptance.run` and bugfix `repro.run`, `tests.run`, `gate.run` of `bindings/*.yaml`; the environment variable wins | done |
| `report-$SDC_ACTION` | `SDC_ACTION` (report file name) | `--action` | done |
| `impl-gate)`, `story=$(cut` | `SDC_CASE` (`impl-gate`: the story id) | `LOOM_CASE`; the story is `LOOM_CASE_NAME` when Loom sets it (`LOOM_PARENT_CASE_NAME` set: its own name; only `LOOM_CASE_NAME`: an epic case, no story) | done |
| `list-stories)`, `epic=$(cut` | `SDC_CASE` (`list-stories`: the epic) | `LOOM_PARENT_CASE_NAME`, else `LOOM_CASE_NAME` (an epic case), else the cut of `LOOM_CASE`; given to `arguments_from` too | done |
| `scope-answer)` | `$2`, the `{principal}` argument (`scope-answer`) | `argv: [tools/sdc-tool.sh, scope-answer, "{principal}", --action, scope.answer]` of the person step `scope.answer` | done |

`sdc-node` (every `bindings/*.yaml` argv runs this program, which reads the same variables under Loom, from the argv entries alone; the shell script it replaced is deleted)

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `SDC_ARGS` | `SDC_ARGS` (`spec.author`'s `domain`) | standard input, one JSON object (empty input is `{}`) | done |
| `SDC_LLM_NODE` | `SDC_LLM_NODE` (the binding's `llm` object) | `--model`, `--max-turns`, `--max-budget-usd`, `--operations`, `--instructions <json>`, `--stub <argv>` | done, until the `agent` executor (`story:bound-agent-node`) |
| `SDC_REPLY` | `SDC_REPLY` | `sdc-reply` on `PATH`, else relative to the tool | done |
| `SDC_FEEDBACK`, `LOOM_RETRY_FEEDBACK` | `SDC_FEEDBACK` (failing records and their logs) | `LOOM_RETRY_FEEDBACK`, written in the driver's form to `work/.sdc/loom-feedback/<digest>/`: each record's kind, result, subject and log tail | done; the records name no action and no exit, so `in/feedback.md` reads `(from ?, exit ?)` |
| `not_called()` | `SDC_NOT_CALLED` (the not-called file) | the reason on standard output, exit 79 (`exits.not_called: [79]` of every model-node binding), no evidence | done |
| `SDC_PRINCIPAL` | `SDC_PRINCIPAL` | `--principal` is taken; no binding states one | pending: `story:bound-program-declares-its-environment` or G2 |
| `SDC_LLM_ADAPTER` | `SDC_LLM_ADAPTER` | `--adapter` is taken; the YAML names none, so the stub runs | pending: N3, N9 |
| `SDC_CLAUDE_CREDENTIALS` | `SDC_CLAUDE_CREDENTIALS` (the login file of `llm-adapter-claude-cli.sh` and of `llm-adapter-metaharness.sh`; else `CLAUDE_CONFIG_DIR`, else `HOME`) | `--claude-credentials <file>`; with none of the four (`ANTHROPIC_API_KEY` too) the claude-cli adapter exits 127 and names them | done; the runner writes it into every model node (`docs/loom-run.md`, "How each adapter gets the login under Loom") |
| `SDC_GATEWAY_FILE` | `SDC_GATEWAY_FILE` (the file with the address and the token of a gateway, read by `llm-adapter-claude-cli.sh` through `bindings/tools/gateway.sh`; `llm-adapter-metaharness.sh` exits 127 when it is set) | `--gateway-file <file>`, never with `--claude-credentials`; without it the adapter reads no gateway | done; the runner writes it into every model node when the settings name a `[gateway]` (`docs/loom-run-settings.md`) |
| `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE` | `SDC_IMPLEMENT_MODE`, `SDC_REVIEW_MODE`, `SDC_IMPLEMENT_COVERAGE` | the defaults | pending: N9 |
| `SDC_LLM_NODES` | `SDC_LLM_NODES` | the default: every node on the adapter | pending: N9 |
| `SDC_STUB_FINDINGS` | `SDC_STUB_FINDINGS` | the default: none | pending: N3 |
| `SDC_FINDINGS:-` | `SDC_FINDINGS`, else `driver/target/debug/` of this checkout | as in `sdc-tool.sh` | done |
| `cd_=${SDC_CASE_DIR:-}`, `events.jsonl` | `SDC_CASE_DIR` (`events.jsonl`: a rejection no longer current) | does not run: `LOOM_RETRY_FEEDBACK` holds only records about the current revision | pending: `story:bound-agent-node` |
| `A=$(jq -r '.approval` | `SDC_CASE_DIR` (`bindings.json`: the rejected subject's path) | `artifacts` of the `bindings/*.yaml` file that binds the rejecting approval | done |
| `SDC_LLM_RECORD`, `SDC_LLM_LOG` | `SDC_LLM_RECORD`, `SDC_LLM_LOG` | the default `work/.sdc/llm/` | pending: `story:agent-node-run-records` |
| `epic=$(cut`, `story=$(cut` | `SDC_CASE` (epic and story ids) | `LOOM_PARENT_CASE_NAME` (epic) and `LOOM_CASE_NAME` (story) when Loom sets them, else the cut of `LOOM_CASE` | done |
| `SDC_CASE_DIR`, `case_dir=` | `SDC_CASE_DIR` (implement request mode) | the directory above the workspace | pending: `story:bound-agent-node` |
| `case_dir=`, `sdc-reply review-inputs`, `schema --type review`, `check-review`, `rejections` | `case.json` (the revisions), `events.jsonl`, `evidence/`, `blobs/` (the conformance run on the change revision, the gate run on the spec revision, the earlier rejections of `change.review`) | record files of the workspace under `.sdc/records/`, one small file per fact, written by the tool that owns it: `conformance_run/<digest>.log` (`sdc-tool.sh story-tests`, tests.run) and `ess_gate/<digest>.log` (`sdc-tool.sh ess-gate`, spec.gate), each the step's log about the tree (`impl`, `spec`) whose content has that digest (`sdc-reply record`; the digest leaves out `target`, `.sdc` and `.derived-from`, like the review's diff), and `rejections/change.review/<n>.json` (`sdc-reply check-review`, on a rejection). `sdc-reply` takes both sources behind one type (`Facts`: the driver's case directory, or the workspace when `LOOM_CASE` is set); the request record's `case_dir` stays empty under Loom, and the fixture adapter, which runs in the workspace, names `$PWD` to count the rejections the same way; under `LOOM_CASE` `sdc_tools::reply::records::case_work` is the one place that decides where the workspace is: the directory the program was started in (Loom `c9b4c177` gives a bound program `PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS`, `LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME` and that start directory, no case directory and no store), and the case directory a caller names is not read, so a `work/` that a model made in the workspace decides nothing | done |
| `SDC_RUN_SETTINGS` | `SDC_RUN_SETTINGS` | the default: no rework limit | pending: N9 |
| `SDC_LLM_MODEL`, `SDC_LLM_EFFORT` | `SDC_LLM_MODEL[_<CLASS>]`, `SDC_LLM_EFFORT[_<CLASS>]` | the binding's `--model`; `--effort <level>` (`efforts.<role>.effort` of the run's settings, written by the bindings copy), else the class's default effort | the effort is done; the model of one variable for all classes stays pending: N9 |
| `SDC_REVIEW_RETRIES` | `SDC_REVIEW_RETRIES` | the default 2 | pending: N9 |

`driver/src/bin/*.rs` called through the tools

| Marker | Reads | Under Loom | Status |
|---|---|---|---|
| `sdc-bugfix.rs:745`, `:786` | `SDC_ACTION` (`story.update`'s `aep` actor) | exported by `sdc-tool.sh` from `--action` | done |
| `sdc-bugfix.rs:1381` | `PATH` | unchanged: Loom gives `PATH` | done |
| `sdc-findings.rs:1435` | `SDC_STUB_REFUSE_DECISION` (stub cycles) | the default: none | pending: N3 |
| `sdc-findings.rs:1552` | `SDC_CONSISTENCY_RULES` | unchanged: both tools pass `--rules` | done |
| `reply/adapters.rs` | `SDC_<CLASS>_ADAPTER`, `SDC_LLM_ADAPTER` | `sdc-node` takes `--adapter`; the class adapters take their defaults | pending: N9 |

Variables beyond `PATH` read under `set -u` (checked 2026-11-22, `story:tool-texts-and-paths`):

| Where | Reads | What was done |
|---|---|---|
| `llm-adapter-claude-cli.sh` | `HOME` (the login under `~/.claude`) | read as `${HOME:-}`; the login is `SDC_CLAUDE_CREDENTIALS`, else `CLAUDE_CONFIG_DIR`, else `HOME`, and with none of them (and no `ANTHROPIC_API_KEY`) the adapter exits 127 naming all four; llm-node's `--claude-credentials <file>` gives the first one |
| `llm-adapter-metaharness.sh`, `llm-adapter-dry.sh`, `llm-adapter-fixture.sh` | `METAHARNESS`, `SDC_LLM_ENDPOINT`, `SDC_LLM_DRY_LOG`, `SDC_COVERAGE_REPLIES`, `SDC_REPLY`, and for the metaharness adapter `SDC_CLAUDE_CREDENTIALS` | all read with a default (`${X:-}`); the metaharness adapter, when `HOME` has no login and the login's directory is the runner's private one (mode 700), sets `HOME` and `TMPDIR` to a home in that directory with a copy of the login for the metaharness process alone, removed on exit (the runner removes what a SIGKILL left) |
| `sdc-tool.sh` `conform-run` | `SDC_ACTION` (the report file name) | read as `${SDC_ACTION:-conform-run}`; the binding states `--action` anyway |
| `sdc-tool.sh` `ess` calls | `ESS_TOOLCHAIN`, `ESS_TOOLCHAIN_QUIET` | exported with defaults at the top of the tool: nothing to change |
| `sdc-tool.sh` `ess` calls | `ESS_TOOLCHAIN_DIR`, else `XDG_CACHE_HOME`, else `HOME` (the 0.97.1 and later dispatcher finds and installs the toolchain below it, 0.97.3 since 2026-11-22, 0.96.0 before) | with none of the three set, `spec-validate` exited 1 (`neither XDG_CACHE_HOME nor HOME is set; set ESS_TOOLCHAIN_DIR`); in Loom mode the tool now exports `ESS_TOOLCHAIN_DIR=<home>/.cache/ess/toolchains`, `<home>` from `getent passwd "$(id -u)"`, when none of the three is set; `scripts/check-bindings.sh` runs `spec-validate` under `env -i PATH LOOM_CASE` |
| `sdc-tool.sh` `slot task` (`conform-run`), `slot cargo test` (`impl-gate`) | none: `PWD` only | nothing to change |
| `ess`, `slot`, `cargo`, `aep`, `claude`, `jq`, `yq` | `HOME` | started with `env -i PATH=...` they answer `--version` (`slot status` too): they take the home directory from the passwd entry; a build through `cargo` without `HOME` was not run (*inferred*). `ess` is the exception once it hands off to a toolchain (row above). `aep` was run under `env -i PATH=...` for `plan artifact new`, `list`, `move`, `validate --strict`: it opens no file under the home directory and needs nothing |

A host whose user has no passwd entry still needs `story:bound-program-declares-its-environment`
(wave 9) or `--confinement local`.

### Where Loom's head differs from this contract

Read at Loom's integration head `882c1866` (`wave/2026-11-21-w7`) and the branches named.

- `LOOM_CASE` (`story:bound-program-case-id`) and `LOOM_RETRY_FEEDBACK`
  (`story:bound-program-retry-records`) are on Loom's branches `impl/bound-program-case-id` and
  `impl/bound-program-retry-records`, not in the head. At the head a program gets `PATH` alone.
- The head had no `program_root` and no `resolve_programs` (*inferred* for the decoder: it refuses a
  member it does not declare): a relative program path was a workspace path, so
  `tools/sdc-tool.sh` was looked for in the workspace. Loom at `af8de2c9` decodes
  `program_root` (`workspace` or `bindings`, `crates/loom-bindings/src/decode.rs:122-134`), and
  every bindings file here that binds a program names `program_root: bindings`. No Loom ref has a
  reserved-action-id check.
- A retry record names its evidence id, kind, subject, subject revision, result, producer, time
  and blob; not the action that produced it nor its exit. A log is the last 60 lines, at most
  64 KiB (the former model-node script quoted the last 80 lines of a driver log). Only records about the subject's
  current revision are given; the variable is unset when there are none and never set for
  `arguments_from`.
- A child case id is `<parent id>.<name>`, as in the driver. Loom names a repeated child
  `<id>.2`; the tools cut the epic and the story from the second and third dot-separated fields,
  so a story under a repeated epic would read `2` as its story.
- At the head a step that placed an artifact at a path takes the content digest of that path as
  the artifact's revision (`impl/bound-artifact-revisions`, merged); the tools need no change for it.
