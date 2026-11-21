# Conformance of the delivered product, observed after close

A run can pass every merge and still miss the idea: until now nobody started the delivered product
(`docs/releases/0.1.0.md`). `sdc-conform observe runs/<run>` starts it and holds it to the
conformance suite of the run's own specification, after the root case has closed. It is an
observation: it changes nothing the run decided, gates nothing, and applies to runs that ended
before it existed (`story:conformance-observe-after-close`). Gating is
`story:conformance-against-implementation`, a later, separately measured step.

```console
$ driver/target/debug/sdc-conform observe runs/<run> [--tables 3] [--table <table.json>]
conformance: spec scenarios proven passed: 11 of 17 (lower bound); 2 failed; 4 not measurable; tables agree on 16 of 17
```

A run that was never measured prints no counts: `conformance: not measured (table_missing), 32
scenarios`. `scripts/run-trial.sh` calls it after a close when `SDC_CONFORMANCE=observe` (off by default since 0.1.4, for its cost; an experiment record turns it on with the launch setting `conformance: observe`; skipped on a recorded
re-drive, whose recording holds no table call).
Writing the tables costs about 1 to 3 USD of node cost per observed product (0.82 to 2.99 USD on
the six 0.1.0 baseline products, below).

## What the figure claims

It judges the delivered product against **its own run's specification**: the contract the run
wrote and then implemented. `passed` is a lower bound and nothing else is claimed: a scenario counts
passed only when every counted table that reached it passed it, and what the tables could not reach
stays outside k. `held` is said only when k = n. A run without an observation shows `not observed`,
never 0. A specification that forgot something the product needs, such as a sign-in, is the
workflow's fault upstream, and the figure shows it: the product refuses what the contract says
succeeds, so it does not conform to that contract (`precondition_not_in_spec`, below).

## What it does

1. Resolves the pinned ess (below). The `ess` on PATH is never used.
2. `ess compile` and `ess verify conform synthesize --suite-format 5` on the root case's
   `work/spec/` into `runs/<run>/conformance/` (`ir.json`, `suite.json`): n is the suite's
   scenario count.
3. Fills N table slots (`--tables N`, else `SDC_CONFORMANCE_TABLES`, default 3; one slot with
   `--table <file>`). Each slot gets a route table (`sdc.conformance-table/1`) from a fresh session
   of the node `conformance.table`, checked against the compiled specification and the delivered
   tree (`sdc-conform check-table --ir … --table … --tree …`).
4. Per distinct `build` of the tables, copies the delivered tree (`work/published/`, else
   `work/release/`) to `runs/<run>/conformance/product/` (`product-2/`, …) and runs `build.argv`
   there (offline; the caller's `SDC_*`, `ANTHROPIC_*`, `CLAUDE*`, `ESS_*`, `GH_*` and `GITHUB_*`
   variables removed).
5. Per table, runs `ess verify conform run --suite … --target process --report-format 2 --strict
   --format json` with `sdc-conform-adapter` as the adapter (`conformance/table-<i>/run-<r>/`). A
   run that shows a fault of the table itself is refused and the slot asked again once (below).
6. Combines the counted tables into one figure and writes `runs/<run>/oracle.json` and
   `runs/<run>/conformance-detail.json` (`sdc.conformance-detail/1`: every table run's
   `--format json` output, `ess-conformance-run/2`).

## Several tables, one figure

One table is one model's reading of the product, and readings differ: on `trial-0.1.0-mixed-2`
one table signed in and got 7 passed, 2 failed, 7 not measurable, another did not and got 1, 8, 7.
So the suite runs once per table, and the figure is taken over the counted tables. A table
**reaches** a scenario when the scenario ends passed or failed under it (not not measurable, not
error):

- **passed**: every counted table that reached the scenario passed it;
- **failed**: every counted table that reached it failed it;
- **disputed**: one counted table passed it and another failed it;
- **not measurable**: no counted table reached it;
- **table agreement**: the scenarios with the same result under every counted table, of n.

A failure is not hidden by an unreachable declaration: a scenario that failed under one counted
table that reached it and that the other tables declared unreachable counts failed (it was reached
and it failed; `strong-2`'s `PushChange/superseded`, under `failed`, `not_measurable`,
`not_measurable`). Not measurable is only what no counted table reached.

`held` only when every scenario passed (k = n > 0); `contradicted` when f > 0; otherwise `unknown`
with cause `disputed` when d > 0, else `not_measurable`. With no counted table the run is
`unknown` with the slots' most frequent cause, and k, f, d and u are null.

An earlier rule counted a scenario passed when at least one table passed it, on the argument that
a table fault can turn a pass into a failure but not a failure into a pass. Mapping several
statuses to one outcome refutes it: a table that reads 404 as `wrong-state` passes a scenario that
a table reading only 409 fails, and which reading describes the product is a choice of the
measurer, not an observation. A scenario the tables disagree on is now reported as disputed and
counted neither way.

A table counts when the check accepted it, its run produced a report, the product started in every
scenario, ess ran every scenario, and the run showed no fault of the table itself. Such faults,
found in the adapter's log, are:

- a capture that did not appear (cause `capture_missing`);
- a request naming a value the scenario does not carry, such as an optional input left out
  (`request_unfillable`, cause `table_error`);
- the product answering 401 or 403 to a command or view: the table sends no sign-in, token or key
  the product accepts (cause `table_error`).

The run is then refused with that evidence and the node asked once more, in the same slot, with the
evidence as feedback (`retried_after_run`, `evidence`, every run under `runs`). A slot refused after
that retry, by the check or by its run, gets one more fresh session with the reasons
(`one_more_session`); at most five node sessions per slot (three for the check, the retry, the one
more). A table refused after that leaves the slot uncounted.

**A precondition the specification does not model is a deviation, not a table fault.** When every
slot ended refused after its retry with the product's 401 or 403 as its only fault, every slot's
last run was refused in the same scenarios, none of those scenarios had a request answered with
success before its refusal, and the product did answer some other request with success, then no
command of those scenarios could satisfy the product: it wants something (a signed-in session)
that only a request the scenarios never send provides. The slots count after all
(`precondition_not_in_spec`: the ids per slot, the refusal kept under `refused_after_run`), and
those scenarios count `failed` with cause `precondition_not_in_spec`, listed apart in the text: "f
failed, of which p refused for a precondition the specification does not model". A product that
accepts no request at all wants a key or token from outside the scenarios (a value it prints at
start, a key in its README): that stays the table's fault (`table_error`). A refused scenario with
a success before its refusal is a session the table did not carry: the table's fault too.

**How a table could pass what the product fails**, and what the checker does about it:

| way | state |
|---|---|
| A forced request on another route, which answers the forced outcome's status whatever the product does on the command's route | closed: a forced request keeps the command's method and route; only a placeholder segment, the query, the body and the headers may differ |
| Two views on one request: both get the same rows, and a check of the wrong view can pass on rows of the other | closed: the check refuses it |
| An event payload or error field written into the table as a literal | closed: the check refuses it |
| An event payload or error field filled from the input: the check compares the scenario's input with itself | recorded: such a check is `unverified` (`unverified_checks`); a scenario whose every check is unverified counts not measurable |
| `start.argv` starting something other than the product's normal start (a mock, a demo mode), or a header that switches the product into a test mode | closed as far as the tree says so: `start.from` cites the README or manifest line that states the start command, the check verifies the program and every literal argument are on the cited lines, and refuses an argument or header a README line describes as a test, demo or mock mode |
| A status mapped to an outcome the product means otherwise (404 "not found" read as `wrong-state`) | closed for an absent entity: the adapter, not the table, reads a refusal of an id the scenario never created as the command's unknown-instance answer, and never reads 404 for an id the scenario created as `wrong-state` (below); the check refuses 404 on a `wrong_state` outcome. Other statuses: several may mean one outcome, and tables that read one differently make the scenario disputed |
| A view mapped to a route that returns more rows than the view holds (all items for "done items"): a "row present" check passes on rows the real view would not hold | open |

## The route table and the node that writes it

The node `conformance.table` (class other) is a fresh model session started after the run, through
the LLM adapter contract of `llm-node.sh` (`SDC_CONFORMANCE_ADAPTER`, else `SDC_LLM_ADAPTER`, else
`claude-cli`). Its working directory holds copies of the delivered tree (`product/`), the
specification (`spec/`) and the compiled IR (`ir.json`); its operations are `dir.list`,
`file.read` and `search` only. It has no stake in the implementation and the implementing node gets
no new obligation. Model `SDC_LLM_MODEL_CONFORMANCE` (default `claude-sonnet-5-5`); effort
`SDC_LLM_EFFORT_CONFORMANCE`, else `SDC_LLM_EFFORT_OTHER`, else `SDC_LLM_EFFORT`, else the class
default in `tools/node-classes.json`; both and their sources are recorded in `oracle.json`
(`conformance.table`). Limits: `SDC_CONFORMANCE_MAX_TURNS` (40), `SDC_CONFORMANCE_MAX_BUDGET_USD`
(2.00), `SDC_CONFORMANCE_NODE_TIMEOUT` (1800 s).

The reply is constrained by a response schema and then checked by the program; a refused reply is
asked again with the check's reasons at most twice, then the slot is uncounted (`table_refused`).
Records: `runs/<run>/conformance/table-<i>/node/NNN/` (prompt, request with `slot` and `attempt`,
response, `refused.json`); attempts are numbered across the slot, the retry after a run included.

A table names:

- `build.argv` (or null) and `start.argv` with `{port}` (required), `{host}`, `{data_dir}`,
  `{data_file}`; `start.ready`, a request the product answers once it is up. argv[0] is a path
  relative to the delivered tree or one of `cargo`, `go`, `node`, `npm`, `deno`, `java`, `dotnet`.
  `start.from`: `[{file, line}]`, the README or manifest (`Cargo.toml`, `package.json`, `go.mod`,
  `pyproject.toml`, `Procfile`, `Taskfile.yml`, `Makefile`, `deno.json`, `justfile`) of the
  delivered tree and the 1-based lines that state this start command for normal use. `start.args`
  (optional): where the README leaves a start value to the operator (`<partner-account-id>`) and
  the specification has an input field for it, `{name, kind: from_scenario, field, nth,
  when_absent}` fills `{arg.<name>}` in `start.argv`: the adapter reads the suite (`--suite`) and
  starts the product per scenario with the `nth` (default 0) distinct literal value the scenario
  sends in that field, else `when_absent`, which must be on the cited lines. No script, no write
  into the product's data file.
- `captures` (optional): values the product prints at start, each `{name, stream, regex,
  timeout_s}` with `stream` `stdout` or `stderr`, a regex with exactly one group and a timeout of
  1 to 120 s. `"{capture.<name>}"` reads one in paths, headers, bodies and templates.
- `bindings` (optional): `{input, capture}` makes every input field of that name take the captured
  value instead of the scenario's (the spec's link becomes the link the product minted), and maps
  the captured value back to the scenario's value in events, error fields and view rows.
- Per command: `request` (method, path, body, headers; `"{input.<field>}"` fills in the input),
  `forced` requests for external outcomes, and per outcome its HTTP `status` (one, or a list when
  several statuses mean one outcome), the `events` it emits
  (payload templates over `{response.<path>}`, `{input.<field>}`, `{lookup.<path>}`,
  `{capture.<name>}`), its `error` and an optional `lookup` request and `when_body` discriminator.
- Per view: `request`, `rows` (path to the row list), `fields` (path of each view field in a row).
- `none` `{kind, why}` for a command, outcome or view without a route. Kind
  `not_reachable_over_the_interface`: the product does it, but no HTTP client can drive or read it
  (it lives only in the browser); its scenarios count as not measurable. Kind `not_implemented`:
  the product does not do it; its scenarios fail.

The check refuses a table that does not cover every command, outcome and view of the compiled
specification; that maps one (command, method, path, status) to two outcomes unless each carries a
`when_body`; whose event payloads or error fields differ from the declared ones or hold a literal
(a value with no placeholder); whose placeholders read nothing; whose argv reaches outside the delivered tree; with an external outcome that has
neither a `forced` request nor a `none`; with a forced request on another method or route than the
command's; with two views on one request; with a `none` without a kind; or with a capture that is
malformed, bound to no input field of the specification, or declared and never used. It refuses a `start.from` that is missing, cites a file that is no
README or manifest or a line that does not exist or describes a test, demo or mock mode, or whose
cited lines do not together contain the file name of `argv[0]` and every argument without a
placeholder (as whole words; an argument with a placeholder stands for a value the README gives, and so
does a literal at the place of an operator value of a cited usage line: `<partner-id>` takes any
value, `<a|b>` one of its alternatives);
it refuses `cargo run` and `go run`, which compile at every start: the table starts the binary the
build made and cites the manifest line naming it; and it refuses a start argument or a header of a command, lookup, view or
`start.ready` request that a README line of the tree root describes as a test, demo or mock mode
(`test mode`, `for tests`, `demo`, `mock`, `fake`, …). Forced requests are exempt: forcing an
external outcome is what they are for.

## The adapter

`sdc-conform-adapter` speaks `ess-conformance-target/1` (ESS `docs/design/conformance-target-process.md`
at tag `0.97.0`). Each scenario gets a fresh product process from `start.argv`, run in the tree
copy, on a free port on 127.0.0.1, with an empty data directory and no environment but `PATH`,
`LANG`, `HOST`, `PORT`, and `HOME` and `TMPDIR` set to the scenario's directory; `end_scenario`
kills its process group. The product's stdout and stderr go to the scenario's `product.log`; each
capture is read from them within its `timeout_s` of the start, and one that does not appear answers
`begin_scenario` `unavailable` (the scenario ends `error`, logged as `capture`). Start waits up to
20 s for `start.ready`; each request waits at most
`min(SDC_CONFORMANCE_TARGET_TIMEOUT, 20)` s, and ess waits `SDC_CONFORMANCE_TARGET_TIMEOUT` (30) s
per answer. A status the table does not map is answered as no declared outcome. Every request a command or
view sends is logged (`request`: method, path, status). A product that does
not start is answered `unavailable` (the scenario ends `error`), never `unsupported`. Events are
built by the adapter from the response body (`events_checked_from: response_body`): the product
publishes none an HTTP client can observe. Starts, captures, unmapped statuses, unreadable views,
unfillable requests (`request_unfillable`) and unfillable templates go to the table run's
`adapter-log.jsonl` and, for the counted tables, `oracle.json` `conformance.adapter_notes`.

**An absent entity is decided by the adapter, not the table.** ESS answers a command on an
instance no record carries with the command's not-found outcome when it declares exactly one,
otherwise with its `wrong_state` outcome (ESS 0.97.0
`docs/design/typed-literals-and-unknown-instances.md`, § 2 "An unknown instance is answered with
the command's `wrong_state` outcome" and § 2a "A declared not-found outcome comes first"; the
error carries no field, "because an instance that does not exist is in none"). ESS does not call
absence a state: the instance "is in none", and the rule is the unknown-instance answer. The
adapter reads the compiled specification (`--ir`) for the input field that names the instance a
command moves or updates, and remembers every value the events of the scenario carried (the
identities the scenario created, as ess captures them). A refusal of a command on an id no event
of the scenario carried, that is 404 or a status the table maps to that answer, is answered with
that answer and the error by name with no fields (logged `unknown_instance`). A status the table
maps to another outcome keeps that reading, because input guards answer first. For an id the
scenario created, 404 is never `wrong_state`: the check refuses 404 on a `wrong_state` outcome, so
a product that answers 404 for a removed or finished entity fails that scenario.

## oracle.json

`sdc.oracle/1`: `checks` holds one entry `{id: conformance, result, k, f, p, d, u, n, agreement,
tables_counted, cause}` (the shape `sdc-run-compare`'s oracle check reads), `conformance` the
record: `result`, `k`, `f`, `p` (of f, refused for a precondition the specification does not
model), `d`, `u`, `n`, `agreement`, `tables_written`, `tables_counted`, `node_sessions`, `measured`,
`summary`, `cause`, `why`, `table_cost_usd` (all slots), `tables` (per slot: source, model,
effort, attempts, `cost_usd`, `retried_after_run`, `one_more_session`, `evidence`, `counted`, `cause`, `why`, the
final run's `k`, `f`, `u`, `e` (errors), `unverified` (checks), `n`, `conformance_status`, `declared_missing`, `build`,
`precondition_not_in_spec` and `refused_after_run` when that rule counted the slot,
and `runs`, one record per table run), `scenarios` (id, status over the counted tables, `under`:
the status per slot, null for an uncounted slot, `cause` `precondition_not_in_spec` where it
applies), `recomputed` (after `sdc-conform recompute`), `failed_checks` (scenario, about, code,
expected, observed, table) of the failed and disputed scenarios, `unverified_checks`
(scenario, about, fields, table), `adapter_notes` (with table), `ess` (version,
sha256, source), `suite` (sha256), `product` (tree, PROVENANCE), `limits`. k, f, d, u and
agreement are null when no table was counted. No absolute path.

Under one table a scenario is `passed`, `error`, `not_measurable` (it passed only on unverified checks, or it did
not pass, none of its checks failed, and it executes, expects, forces or reads something the table declares `none` of
kind `not_reachable_over_the_interface`), or else `failed` (ess `failed`, `unsupported` or
`skipped`). The run's figure combines the counted tables as above; a run without one is `unknown`
with a cause, never a pass:

| cause | when |
|---|---|
| `ess_missing`, `ess_pin_mismatch`, `ess_pin_unreadable` | the pinned binary is absent, or its sha256 or `--version` differs |
| `suite_failed` | no spec, or compile or synthesize failed |
| `suite_empty` | the spec synthesizes no scenario (ess would say `passed` with 0 scenarios and `inconclusive` under `--strict`) |
| `no_delivered_tree` | the root case has neither `work/published/` nor `work/release/` |
| `table_missing` | no reply from the node, a model-less adapter (`stub`, `dry`), or an unreadable `--table` |
| `table_refused` | the check refused the table (after two retries of the node, or on the retry after a run) |
| `table_error` | the table's run showed its own fault (an unfillable request, 401 or 403), also after the retry, and the precondition rule above did not apply |
| `capture_missing` | a capture did not appear on its stream within its timeout, also after the retry |
| `product_build_failed` | `build.argv` exited non-zero or timed out (`SDC_CONFORMANCE_BUILD_TIMEOUT`, 900 s) |
| `product_not_started` | the product did not answer `start.ready` in a scenario |
| `adapter_refused` | ess wrote no report (exit 2: the adapter could not start or the handshake failed) |
| `suite_mismatch` | ess ran another number of scenarios than the suite holds |

These are slot causes; the run takes the most frequent one when no slot is counted. With a counted
table the causes are `disputed` (no scenario failed, d > 0) and `not_measurable` (no scenario
failed or is disputed, u > 0).

## Where the figure shows

The figure goes into a release as an observation: what the remaining choice of the table node can
do is bounded (below), so `passed` is published as a lower bound and nothing else is claimed. It
gates nothing.

One text form everywhere (`run_metrics::conformance_figure`): `spec scenarios proven passed: k of
n (lower bound); f failed[, of which p refused for a precondition the specification does not
model][; d disputed]; u not measurable; tables agree on a of n` (`; 1 table` for one), with
` (held)` only when k = n; `not measured (<cause>), n scenarios` (or `not measured (<cause>)` when
the suite was never counted) for a run with no counted table; `not observed` without
`oracle.json`, never 0. Never a percentage without n.

- `sdc-run-metrics`: `metrics.conformance` (that text), `conformance_k`, `_f`, `_p`, `_d`, `_u`,
  `_n`, `_agreement`, `_tables`, `_result`, `_cause`, `_measured`; the text form's line
  `conformance: …`.
- `sdc-run-compare`: the row `conformance`, the same text.
- The progress ledger (`sdc-experiment progress`, always; `docs/progress.*` and `sdc-experiment
  check --all` include it): per run the committed summary `experiments/<id>/conformance/<run>.json`
  when there is one, else the run summary's `conformance_*` figures; per entry k, f, p, d, u and n
  summed over the measured runs, the agreement summed over them, held runs, the observed runs with
  no figure by cause, the runs not observed, and the node cost of the tables; per run its figure;
  a summary column.
- `sdc-conform summary <run>` prints that committed summary (`sdc.conformance-summary/1`): counts,
  result and cause, scenario causes counted, table agreement, per slot its counts, end, sessions and
  node cost, the ess version and the suite's sha256. No request, no response body, no `why`, no
  scenario id: nothing the product or a model wrote.
- `sdc-conform recompute <run>` takes the figure again from the table runs the run stored
  (`conformance/table-<slot>/run-<n>/`: report, ess detail, adapter log, table) under the current
  rules, with no node session, no build and no ess run; slots, sessions and costs stay those of
  the stored record, which is kept under `recomputed.previous_summary`.

## The ess pin

`driver/ess-pin.json` pins ess 0.97.0 (tag `0.97.0`, commit `58242e0bc7`): per platform the
release archive's sha256 (as in the release's `SHA256SUMS`) and the unpacked binary's. The binary is
`ESS_BIN` when set, else `$SDC_ESS_CACHE`, `$XDG_CACHE_HOME/sdc/ess` or `$HOME/.cache/sdc/ess`,
then `<version>/<target>/ess`; it is refused unless its sha256 and `--version` match the pin.
`sdc-conform ess-install [--archive <file>]` downloads (or takes) the archive, checks its sha256,
unpacks the binary and checks it. Canon is pinned by version and commit in `driver/canon-pin.json`:
the scripts take `CANON` or the cached pinned install and refuse a different `canon --version`.

## Limits

- The started product is not confined: it runs on this host as the operator's user. It is started
  from a copy of the delivered tree with a cleared environment and is given 127.0.0.1 and a port,
  but nothing stops it binding another address or reaching the network. Confinement belongs to
  Substrate (`docs/layers.md`) and is not in this step. The build runs model-written code too
  (`build.rs`, macros), with the operator's environment minus the variables listed above.
- Events are built from the response body, so event checks test the table as much as the product.
- Each table is a model's reading of the product. Several tables make a disagreement visible
  (disputed) instead of resolving it; the open row of the table under "How a table could pass what
  the product fails" can still pass a scenario the product fails. The records are kept for review.
- N tables cost N node sessions and N suite runs (plus one per retry): about 1 to 3 USD of node
  cost per observed product with three tables.
- Only products with an HTTP JSON surface; a command-line product needs another adapter.
- A capture reads only what the product prints. A value it mints and keeps to itself cannot be
  bound; the node declares what needs it `none`, and the kind decides whether its scenarios fail
  or are not measurable. The kind is the node's reading, checked only for presence.

## State of the instrument (2026-11-21, on copies of the six 0.1.0 baseline runs in a scratch directory)

Three tables per product, table node Sonnet at effort high, the four-count rule, the checks above,
the adapter's unknown-instance reading, `start.args` and the one more session. Round cap 10 USD
of node cost; spent 8.50 USD. The figures below are `sdc-conform recompute` of those table runs
under the precondition rule (no new node session); only `mixed-2` changed. Their sanitized
summaries are `experiments/baseline-0.1.0/conformance/*.json`; the haiku runs were not observed.

| run | figure | passed, failed, not measurable per table | node sessions | node cost |
|---|---|---|---|---|
| `strong-1` | spec scenarios proven passed: 16 of 17 (lower bound); 1 failed; 0 not measurable; tables agree on 17 of 17 | 16 1 0 · 16 1 0 · 16 1 0 | 4 | 0.82 USD |
| `strong-2` | spec scenarios proven passed: 11 of 17 (lower bound); 2 failed; 4 not measurable; tables agree on 16 of 17 | 11 2 4 · 11 1 5 · 11 1 5 | 5 | 1.41 USD |
| `strong-3` | spec scenarios proven passed: 7 of 20 (lower bound); 13 failed; 0 not measurable; tables agree on 20 of 20 | 7 13 0 · 7 13 0 · 7 13 0 | 5 | 1.34 USD |
| `mixed-1` | spec scenarios proven passed: 6 of 19 (lower bound); 5 failed; 8 not measurable; tables agree on 19 of 19 | 6 5 8 · 6 5 8 · 6 5 8 | 4 | 0.89 USD |
| `mixed-2` | spec scenarios proven passed: 2 of 16 (lower bound); 8 failed, of which 8 refused for a precondition the specification does not model; 6 not measurable; tables agree on 16 of 16 | 2 8 6 · 2 8 6 · 2 8 6 | 9 | 2.99 USD |
| `mixed-3` | spec scenarios proven passed: 11 of 14 (lower bound); 2 failed; 1 not measurable; tables agree on 14 of 14 | 11 2 1 · 11 2 1 · 11 2 1 | 6 | 1.05 USD |

Node sessions are counted per product; the cap is five per slot (three for the check, the retry
after a run, one more). The per-table passed counts are equal on every product. The three tables
agree on every scenario of five products, and on 16 of 17 of `strong-2`.

What the unknown-instance reading changed: the `wrong-state` scenarios that send an id no step
created now pass where the product refuses it (404 "no such change"): three on `strong-1`, two on
`mixed-3`, and the same scenarios on `strong-3` and `mixed-1`. `strong-1`'s one failure left is
`SendChange/unreachable`: the product checks existence before the forced connection loss and
answers 404, where the suite expects `unreachable`. `strong-2`'s `PushChange/wrong-state` fails on
the product: it answers 200 `sent` for a change id it never held.

**Choices the table node still makes that decide a result:**

- **`none` of kind `not_reachable_over_the_interface`**, the node's judgement that no HTTP client
  can drive or read a command, outcome or view, which makes its scenarios not measurable:
  `strong-2` (`QueueChange`, `PendingChanges`: 4 scenarios, and `PushChange/superseded`, failed
  under one table and not measurable under the two that declared it unreachable), `mixed-1`
  (`ChangeOffline`, `KeepCopy`, `SendChanges`, and the views `OpenItems`, `DoneItems`, `CopyView`:
  8 scenarios), `mixed-3` (`SyncEdit/dropped`: 1).
- Routes, the status of each outcome, forced requests and view rows are still written by the node.
  On `strong-1` and `strong-3` no scenario rests on a `none`, a dispute or an unverified check,
  and three independent tables wrote the same results scenario for scenario.

What that choice can do is bounded: a `none` moves a scenario between failed and not measurable,
never into passed, and a scenario one table saw fail is never counted passed. So k is a lower bound on every
product, and the figure goes into the ledger as such.

**`mixed-2` and the sign-in.** The product refuses every list command 401 `NotSignedIn` until a
partner signs in, and the scenarios of `AddItem`, `CompleteItem`, `EditItem`, `Item` and
`SendChange` never call `SignIn`; in all nine sessions of the three slots the first request of
each of those 8 scenarios was refused, while `SignIn` itself was answered 200. A session the
specification does not model is an ambient precondition (ESS `docs/design/outcome-shapes.md`,
#152). It used to leave the product not measured; it now counts against the product: the
specification is the contract, and a product that refuses what the contract says succeeds does not
conform. The specification that forgot the sign-in is the workflow's fault upstream, which is what
the figure shows.

Earlier readings on the same copies, for comparison: before the unknown-instance reading
`strong-1` was 13 / 4 / 0 / 0 and `strong-2` had two scenarios disputed on whether 404 is
`wrong-state`. Before `start.from`, `mixed-2`'s round-1 table got 7 / 2 / 7 by starting the product
through a `node -e` script that wrote signed-in sessions into the data file.

**Questions answered on the way.** A (several statuses for one outcome): the format allowed one
status per outcome and the prompt forbade reading 404 as `wrong-state`, so the 404 failures of the
first figures were our rule. Several statuses may now mean one outcome, and 404 for an absent
entity is the adapter's decision, not the table's. B (identifiers): the adapter binds a created
entity's id from the creating response (the creation event built from `{response.<id>}`, which
ess captures). `strong-1`'s four first failures sent the suite's literal id of an instance no step
created: the request log of `SendChange/outcome/wrong-state` holds one request, `POST
/api/changes/00000000-0000-4000-8000-9abf7a29ff89/send`, answered 404 `no such change`. No binding
was missing.

- Tests (`driver/tests/conformance.rs`) need the pinned ess installed.
