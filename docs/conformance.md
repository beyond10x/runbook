# Conformance of the delivered product, observed after close

A run can pass every merge and still miss the idea: until now nobody started the delivered product
(`docs/releases/0.1.0.md`). `sdc-conform observe runs/<run>` starts it and holds it to the
conformance suite of the run's own specification, after the root case has closed. It is an
observation: it changes nothing the run decided, gates nothing, and applies to runs that ended
before it existed (`story:conformance-observe-after-close`). Gating is
`story:conformance-against-implementation`, a later, separately measured step.

```console
$ driver/target/debug/sdc-conform observe runs/<run> [--tables 3] [--table <table.json>]
conformance: 13 passed, 4 failed, 0 not measurable of 17; 3 tables agree on 16 of 17
```

A run that was never measured prints no counts: `conformance: not measured (table_missing), 32
scenarios`. `scripts/run-trial.sh` calls it after every close (off with `SDC_CONFORMANCE=off`).

## What it does

1. Resolves the pinned ess (below). The `ess` on PATH is never used.
2. `ess compile` and `ess verify conform synthesize --suite-format 5` on the root case's
   `work/spec/` into `runs/<run>/conformance/` (`ir.json`, `suite.json`): n is the suite's
   scenario count.
3. Fills N table slots (`--tables N`, else `SDC_CONFORMANCE_TABLES`, default 3; one slot with
   `--table <file>`). Each slot gets a route table (`sdc.conformance-table/1`) from a fresh session
   of the node `conformance.table`, checked against the compiled specification
   (`sdc-conform check-table`).
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
A figure that moves by six scenarios with the instrument cannot show an improvement of the
workflow. So the suite runs once per table, and the figure is taken over the counted tables:

- **passed**: the scenarios that passed under at least one counted table;
- **failed**: the scenarios that passed under none and failed under at least one, so under every
  counted table that reached them (a table reaches a scenario when it ends passed or failed, not
  not measurable or error);
- **not measurable**: the rest;
- **table agreement**: the scenarios with the same result under every counted table, of n.

`held` only when every scenario passed (k = n > 0); `contradicted` when f > 0; `unknown` with
cause `not_measurable` when f = 0 and u > 0. With no counted table the run is `unknown` with the
slots' most frequent cause, and k, f and u are null.

A table counts when the check accepted it, its run produced a report, the product started in every
scenario, ess ran every scenario, and the run showed no fault of the table itself. Such faults,
found in the adapter's log, are:

- a capture that did not appear (cause `capture_missing`);
- a request naming a value the scenario does not carry, such as an optional input left out
  (`request_unfillable`, cause `table_error`);
- the product answering 401 or 403 to a command or view: the table sends no sign-in, token or key
  the product accepts (cause `table_error`).

The run is then refused with that evidence and the node asked once more, in the same slot, with the
evidence as feedback (`retried_after_run`, `evidence`, both runs under `runs`). A retried table
that shows a fault again, or that the check refuses, leaves the slot uncounted.

**Why "at least one".** The checker holds every table to the specification and to the product's
own routes, so a fault of a table can make a scenario the product would pass fail or end in error:
a wrong path or status gets no declared outcome, a wrong template builds a wrong event, a missing
sign-in gets 401. It cannot, within those rules, make a scenario pass that the product fails: every
outcome is read from the status the product answered on the command's own route, every event and
error field from the product's response, every view row from the product's rows. A pass under one
table is therefore evidence about the product; a failure under another may be evidence about that
table.

**Where the argument stops.** It holds only as far as the checker's rules reach. These table
faults can turn a real failure into a pass:

| way | state |
|---|---|
| A forced request on another route, which answers the forced outcome's status whatever the product does on the command's route | closed: a forced request keeps the command's method and route; only a placeholder segment, the query, the body and the headers may differ |
| Two views on one request: both get the same rows, and a check of the wrong view can pass on rows of the other | closed: the check refuses it |
| A status mapped to an outcome the product means otherwise (404 "not found" read as `wrong-state`). The prompt forbids it; the checker cannot see what a status means | open |
| The union over tables of contradicting mappings: one table reads 409 as `wrong-state` and passes "refuses when done", another reads 404 as `wrong-state` and passes "unknown id"; no single reading passes both, the figure counts both | open |
| A view mapped to a route that returns more rows than the view holds (all items for "done items"): a "row present" check passes on rows the real view would not hold | open |
| An event payload or error field filled from the input or a literal instead of the response: the check compares the scenario with itself | open (`events_checked_from: response_body`) |
| `start.argv` starting something in the tree other than the product (a mock, a demo mode), or a header that switches the product into a test mode | open |

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
- `captures` (optional): values the product prints at start, each `{name, stream, regex,
  timeout_s}` with `stream` `stdout` or `stderr`, a regex with exactly one group and a timeout of
  1 to 120 s. `"{capture.<name>}"` reads one in paths, headers, bodies and templates.
- `bindings` (optional): `{input, capture}` makes every input field of that name take the captured
  value instead of the scenario's (the spec's link becomes the link the product minted), and maps
  the captured value back to the scenario's value in events, error fields and view rows.
- Per command: `request` (method, path, body, headers; `"{input.<field>}"` fills in the input),
  `forced` requests for external outcomes, and per outcome its HTTP `status`, the `events` it emits
  (payload templates over `{response.<path>}`, `{input.<field>}`, `{lookup.<path>}`,
  `{capture.<name>}`), its `error` and an optional `lookup` request and `when_body` discriminator.
- Per view: `request`, `rows` (path to the row list), `fields` (path of each view field in a row).
- `none` `{kind, why}` for a command, outcome or view without a route. Kind
  `not_reachable_over_the_interface`: the product does it, but no HTTP client can drive or read it
  (it lives only in the browser); its scenarios count as not measurable. Kind `not_implemented`:
  the product does not do it; its scenarios fail.

The check refuses a table that does not cover every command, outcome and view of the compiled
specification; that maps one (command, method, path, status) to two outcomes unless each carries a
`when_body`; whose event payloads or error fields differ from the declared ones; whose placeholders
read nothing; whose argv reaches outside the delivered tree; with an external outcome that has
neither a `forced` request nor a `none`; with a forced request on another method or route than the
command's; with two views on one request; with a `none` without a kind; or with a capture that is
malformed, bound to no input field of the specification, or declared and never used.

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
per answer. A status the table does not map is answered as no declared outcome. A product that does
not start is answered `unavailable` (the scenario ends `error`), never `unsupported`. Events are
built by the adapter from the response body (`events_checked_from: response_body`): the product
publishes none an HTTP client can observe. Starts, captures, unmapped statuses, unreadable views,
unfillable requests (`request_unfillable`) and unfillable templates go to the table run's
`adapter-log.jsonl` and, for the counted tables, `oracle.json` `conformance.adapter_notes`.

## oracle.json

`sdc.oracle/1`: `checks` holds one entry `{id: conformance, result, k, f, u, n, agreement,
tables_counted, cause}` (the shape `sdc-run-compare`'s oracle check reads), `conformance` the
record: `result`, `k`, `f`, `u`, `n`, `agreement`, `tables_written`, `tables_counted`, `measured`,
`summary`, `cause`, `why`, `table_cost_usd` (all slots), `tables` (per slot: source, model,
effort, attempts, `cost_usd`, `retried_after_run`, `evidence`, `counted`, `cause`, `why`, the
final run's `k`, `f`, `u`, `e` (errors), `n`, `conformance_status`, `declared_missing`, `build`,
and `runs`, one record per table run), `scenarios` (id, status over the counted tables, `under`:
the status per slot, null for an uncounted slot), `failed_checks` (scenario, about, code,
expected, observed, table) of the failed scenarios, `adapter_notes` (with table), `ess` (version,
sha256, source), `suite` (sha256), `product` (tree, PROVENANCE), `limits`. k, f, u and agreement
are null when no table was counted. No absolute path.

Under one table a scenario is `passed`, `error`, `not_measurable` (it did not pass, none of its
checks failed, and it executes, expects, forces or reads something the table declares `none` of
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
| `table_error` | the table's run showed its own fault (an unfillable request, 401 or 403), also after the retry |
| `capture_missing` | a capture did not appear on its stream within its timeout, also after the retry |
| `product_build_failed` | `build.argv` exited non-zero or timed out (`SDC_CONFORMANCE_BUILD_TIMEOUT`, 900 s) |
| `product_not_started` | the product did not answer `start.ready` in a scenario |
| `adapter_refused` | ess wrote no report (exit 2: the adapter could not start or the handshake failed) |
| `suite_mismatch` | ess ran another number of scenarios than the suite holds |

These are slot causes; the run takes the most frequent one when no slot is counted. With a counted
table the only cause is `not_measurable`: no scenario failed and u > 0 passed and failed under no
counted table.

## Where the figure shows

One text form everywhere: `k passed, f failed, u not measurable of n; t tables agree on a of n`
(`; 1 table` for one), with `, unknown: <cause>` for a measured unknown; `not measured (<cause>),
n scenarios` (or `not measured (<cause>)` when the suite was never counted) for a run with no
counted table; `not observed` without `oracle.json`.

- `sdc-run-metrics`: `metrics.conformance` (that text), `conformance_k`, `_f`, `_u`, `_n`,
  `_agreement`, `_tables`, `_result`, `_cause`, `_measured`; the text form's line
  `conformance: …` (with `(held)` or `(contradicted)`).
- `sdc-run-compare`: the row `conformance`, from the same numbers.
- The progress ledger (`sdc-experiment progress`, `docs/progress.md`): per entry k, f, u and n
  summed over the measured runs (held, contradicted, not measurable only), the agreement summed
  over them (`the tables agree on a of n`), the observed runs with no figure by cause, per run its
  figure, and a summary column; from the run summaries `sdc-experiment report` writes, so only
  runs reported after observation carry it.

## The ess pin

`driver/ess-pin.json` pins ess 0.97.0 (tag `0.97.0`, commit `58242e0bc7`): per platform the
release archive's sha256 (as in the release's `SHA256SUMS`) and the unpacked binary's. The binary is
`ESS_BIN` when set, else `$SDC_ESS_CACHE`, `$XDG_CACHE_HOME/sdc/ess` or `$HOME/.cache/sdc/ess`,
then `<version>/<target>/ess`; it is refused unless its sha256 and `--version` match the pin.
`sdc-conform ess-install [--archive <file>]` downloads (or takes) the archive, checks its sha256,
unpacks the binary and checks it. Canon, by comparison, has no version pin: the scripts use the
sibling checkout (`scripts/check-protocols.sh`, `CANON_DIR`; `scripts/run-trial.sh`, `CANON`).

## Limits

- The started product is not confined: it runs on this host as the operator's user. It is started
  from a copy of the delivered tree with a cleared environment and is given 127.0.0.1 and a port,
  but nothing stops it binding another address or reaching the network. Confinement belongs to
  Substrate (`docs/layers.md`) and is not in this step. The build runs model-written code too
  (`build.rs`, macros), with the operator's environment minus the variables listed above.
- Events are built from the response body, so event checks test the table as much as the product.
- Each table is a model's reading of the product. Several tables and "at least one" remove the
  faults that fail a scenario the product passes; the open rows of the table under "Where the
  argument stops" can still pass one it fails. The records are kept for review.
- N tables cost N node sessions and N suite runs (plus one per retry).
- Only products with an HTTP JSON surface; a command-line product needs another adapter.
- A capture reads only what the product prints. A value it mints and keeps to itself cannot be
  bound; the node declares what needs it `none`, and the kind decides whether its scenarios fail
  or are not measurable. The kind is the node's reading, checked only for presence.

## First figures (2026-11-21, on copies of the runs in a scratch directory)

The six closed runs of the 0.1.0 baseline, one table each (before several tables and the retry
after a run), table node Sonnet at effort high:

| run | k / f / u of n | result | what fails | node |
|---|---|---|---|---|
| `strong-1` | 13 / 4 / 0 of 17 | contradicted | an unknown id answers 404 where the spec says `wrong-state`; existence is checked before the forced `unreachable` | 1 attempt, 0.26 USD |
| `strong-2` | 9 / 4 / 4 of 17 | contradicted | `PushChange` of a superseded or sent change answers `sent`; unknown task 404 instead of `wrong-state`; the queue lives only in the browser (u) | 1, 0.26 USD |
| `strong-3` | 4 / 13 / 0 of 20 | unknown: `scenario_error` | ids are 32 hex digits, not a Uuid; `opened_at` and `made_at` are numbers, not Timestamps; 3 errors: the table's query string needs an optional input field the scenario leaves out | 1, 0.27 USD |
| `mixed-1` | 4 / 7 / 8 of 19 | contradicted | the link printed at start is captured and bound; ids are not Uuids; `DoneItems` as mapped holds open items; unknown item 404; the offline copy lives only in the browser (u) | 1, 0.21 USD |
| `mixed-2` | 1 / 8 / 7 of 16 | contradicted | this table did not sign in (401 `NotSignedIn` on every list command); the queue and the shared service are not reachable (u) | 1, 0.28 USD |
| `mixed-3` | 9 / 5 / 0 of 14 | contradicted | `ListItems` drops a deleted item the spec keeps; unknown item 404 instead of `wrong-state`; the forced `dropped` answers 404 | 3 attempts, 0.62 USD |

The table is the node's reading and varies between runs of the node: an earlier node run on
`mixed-2` captured the partner id printed at start, bound it to `account_id`, and got 7 / 2 / 7.
A hand-written adapter for `strong-1` (the feasibility probe) got the same 13 of 17 with the same
four failures.

- Tests (`driver/tests/conformance.rs`) need the pinned ess installed.
