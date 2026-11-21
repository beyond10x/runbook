# Conformance of the delivered product, observed after close

A run can pass every merge and still miss the idea: until now nobody started the delivered product
(`docs/releases/0.1.0.md`). `sdc-conform observe runs/<run>` starts it and holds it to the
conformance suite of the run's own specification, after the root case has closed. It is an
observation: it changes nothing the run decided, gates nothing, and applies to runs that ended
before it existed (`story:conformance-observe-after-close`). Gating is
`story:conformance-against-implementation`, a later, separately measured step.

```console
$ driver/target/debug/sdc-conform observe runs/<run> [--table <table.json>]
conformance: 13 passed, 4 failed, 0 not measurable of 17
```

`scripts/run-trial.sh` calls it after every close (off with `SDC_CONFORMANCE=off`).

## What it does

1. Resolves the pinned ess (below). The `ess` on PATH is never used.
2. `ess compile` and `ess verify conform synthesize --suite-format 5` on the root case's
   `work/spec/` into `runs/<run>/conformance/` (`ir.json`, `suite.json`): n is the suite's
   scenario count.
3. Gets a route table (`sdc.conformance-table/1`) from the node `conformance.table`, or from
   `--table`, and checks it against the compiled specification (`sdc-conform check-table`).
4. Copies the delivered tree (`work/published/`, else `work/release/`) to
   `runs/<run>/conformance/product/` and runs the table's `build.argv` there (offline; the caller's
   `SDC_*`, `ANTHROPIC_*`, `CLAUDE*`, `ESS_*`, `GH_*` and `GITHUB_*` variables removed).
5. Runs `ess verify conform run --suite … --target process --report-format 2 --strict --format json`
   with `sdc-conform-adapter` as the adapter.
6. Writes `runs/<run>/oracle.json` and `runs/<run>/conformance-detail.json` (ess's `--format json`
   output, `ess-conformance-run/2`).

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
asked again with the check's reasons at most twice, then the observation is `unknown`
(`table_refused`). Records: `runs/<run>/conformance/table-node/NNN/` (prompt, request, response,
`refused.json`).

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
neither a `forced` request nor a `none`; with a `none` without a kind; or with
a capture that is malformed, bound to no input field of the specification, or declared and never
used.

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
publishes none an HTTP client can observe. Starts, unmapped statuses and unfillable templates go to
`runs/<run>/conformance/adapter-log.jsonl` and `oracle.json` `conformance.adapter_notes`.

## oracle.json

`sdc.oracle/1`: `checks` holds one entry `{id: conformance, result, k, f, u, n, cause}` (the shape
`sdc-run-compare`'s oracle check reads), `conformance` the record: `result`, `k`, `f`, `u`, `n`,
`measured`, `summary` (`k passed, f failed, u not measurable of n`), `cause`, `why`,
`conformance_status`, `counts`, `scenarios` (id, status, ess_status), `failed_checks` (scenario,
about, code, expected, observed), `declared_missing`, `adapter_notes`, `ess` (version, sha256,
source), `suite` (sha256), `table` (source, model, effort, attempts, cost, refusals), `product`
(tree, PROVENANCE), `build`, `limits`. No absolute path.

Each scenario is `passed`, `error`, `not_measurable` (it did not pass, none of its checks failed,
and it executes, expects, forces or reads something the table declares `none` of kind
`not_reachable_over_the_interface`), or else `failed` (ess `failed`, `unsupported` or `skipped`). `held` only when ess's report says
`conformance_status: passed` and k = n > 0, so u > 0 is never `held`; `contradicted` when f > 0;
`unknown` with cause `not_measurable` when f = 0 and u > 0 (a measured figure: `measured` true);
otherwise `unknown` with a cause, never a pass:

| cause | when |
|---|---|
| `ess_missing`, `ess_pin_mismatch`, `ess_pin_unreadable` | the pinned binary is absent, or its sha256 or `--version` differs |
| `suite_failed` | no spec, or compile or synthesize failed |
| `suite_empty` | the spec synthesizes no scenario (ess would say `passed` with 0 scenarios and `inconclusive` under `--strict`) |
| `no_delivered_tree` | the root case has neither `work/published/` nor `work/release/` |
| `table_missing` | no reply from the node, a model-less adapter (`stub`, `dry`), or an unreadable `--table` |
| `table_refused` | the check refused the table (after two retries of the node) |
| `product_build_failed` | `build.argv` exited non-zero or timed out (`SDC_CONFORMANCE_BUILD_TIMEOUT`, 900 s) |
| `product_not_started` | the product did not answer `start.ready` in a scenario |
| `capture_missing` | a capture did not appear on its stream within its timeout in a scenario |
| `not_measurable` | no scenario failed, and u > 0 needed what the interface cannot reach |
| `adapter_refused` | ess wrote no report (exit 2: the adapter could not start or the handshake failed) |
| `scenario_error` | a scenario ended `error` (ess `inconclusive`) |
| `suite_mismatch` | ess ran another number of scenarios than the suite holds |

## Where the figure shows

- `sdc-run-metrics`: `metrics.conformance` (`k passed, f failed, u not measurable of n`, with
  `, unknown: <cause>` for an unknown, or `not observed`), `conformance_k`, `_f`, `_u`, `_n`,
  `_result`, `_cause`, `_measured`; the text form's line `conformance: …` (with `(held)` or
  `(contradicted)`).
- `sdc-run-compare`: the row `conformance`, from the same numbers.
- The progress ledger (`sdc-experiment progress`, `docs/progress.md`): per entry k, f, u and n
  summed over the measured runs (held, contradicted, not measurable only), the observed runs with
  no figure by cause, per run its figure, and a summary column; from the run summaries
  `sdc-experiment report` writes, so only runs reported after observation carry it.

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
- The table is a model's reading of the product; a wrong table can fail scenarios the product
  would pass, or pass scenarios it would fail. Its records are kept for review.
- Only products with an HTTP JSON surface; a command-line product needs another adapter.
- A capture reads only what the product prints. A value it mints and keeps to itself cannot be
  bound; the node declares what needs it `none`, and the kind decides whether its scenarios fail
  or are not measurable. The kind is the node's reading, checked only for presence.

## First figures (2026-11-21, on copies of the runs in a scratch directory)

The six closed runs of the 0.1.0 baseline, table node Sonnet at effort high:

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
