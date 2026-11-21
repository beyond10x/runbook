# Conformance of the delivered product, observed after close

A run can pass every merge and still miss the idea: until now nobody started the delivered product
(`docs/releases/0.1.0.md`). `sdc-conform observe runs/<run>` starts it and holds it to the
conformance suite of the run's own specification, after the root case has closed. It is an
observation: it changes nothing the run decided, gates nothing, and applies to runs that ended
before it existed (`story:conformance-observe-after-close`). Gating is
`story:conformance-against-implementation`, a later, separately measured step.

```console
$ driver/target/debug/sdc-conform observe runs/<run> [--table <table.json>]
conformance: 13 of 17 scenarios
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
- Per command: `request` (method, path, body; `"{input.<field>}"` fills in the input), `forced`
  requests for external outcomes, and per outcome its HTTP `status`, the `events` it emits (payload
  templates over `{response.<path>}`, `{input.<field>}`, `{lookup.<path>}`), its `error` and an
  optional `lookup` request and `when_body` discriminator.
- Per view: `request`, `rows` (path to the row list), `fields` (path of each view field in a row).
- `none` with a reason for a command, outcome or view the product has no way to do: its
  scenarios are answered `unsupported` and fail.

The check refuses a table that does not cover every command, outcome and view of the compiled
specification; that maps one (command, method, path, status) to two outcomes unless each carries a
`when_body`; whose event payloads or error fields differ from the declared ones; whose placeholders
read nothing; or whose argv reaches outside the delivered tree.

## The adapter

`sdc-conform-adapter` speaks `ess-conformance-target/1` (ESS `docs/design/conformance-target-process.md`
at tag `0.97.0`). Each scenario gets a fresh product process from `start.argv`, run in the tree
copy, on a free port on 127.0.0.1, with an empty data directory and no environment but `PATH`,
`LANG`, `HOST`, `PORT`, and `HOME` and `TMPDIR` set to the scenario's directory; `end_scenario`
kills its process group. Start waits up to 20 s for `start.ready`; each request waits at most
`min(SDC_CONFORMANCE_TARGET_TIMEOUT, 20)` s, and ess waits `SDC_CONFORMANCE_TARGET_TIMEOUT` (30) s
per answer. A status the table does not map is answered as no declared outcome. A product that does
not start is answered `unavailable` (the scenario ends `error`), never `unsupported`. Events are
built by the adapter from the response body (`events_checked_from: response_body`): the product
publishes none an HTTP client can observe. Starts, unmapped statuses and unfillable templates go to
`runs/<run>/conformance/adapter-log.jsonl` and `oracle.json` `conformance.adapter_notes`.

## oracle.json

`sdc.oracle/1`: `checks` holds one entry `{id: conformance, result, k, n, cause}` (the shape
`sdc-run-compare`'s oracle check reads), `conformance` the record: `result`, `k`, `n`, `summary`
(`k of n scenarios`), `cause`, `why`, `conformance_status`, `counts`, `scenarios` (id, status),
`failed_checks` (scenario, about, code, expected, observed), `declared_missing`, `adapter_notes`,
`ess` (version, sha256, source), `suite` (sha256), `table` (source, model, effort, attempts,
cost, refusals), `product` (tree, PROVENANCE), `build`, `limits`. No absolute path.

`held` only when ess's report says `conformance_status: passed` and k = n > 0; `contradicted`
when ess ran every scenario and one failed or was unsupported; otherwise `unknown` with a cause,
never a pass:

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
| `adapter_refused` | ess wrote no report (exit 2: the adapter could not start or the handshake failed) |
| `scenario_error` | a scenario ended `error` (ess `inconclusive`) |
| `suite_mismatch` | ess ran another number of scenarios than the suite holds |

## Where the figure shows

- `sdc-run-metrics`: `metrics.conformance` (`k of n scenarios`, an unknown's cause, or
  `not observed`), `conformance_k`, `conformance_n`, `conformance_result`, `conformance_cause`; the
  text form's line `conformance: …`.
- `sdc-run-compare`: the row `conformance`.
- The progress ledger (`sdc-experiment progress`, `docs/progress.md`): per entry the summed
  `k of n` over the observed runs, per run its figure, and a summary column; from the run summaries
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
- A table can only fill requests from command inputs and responses. A value the product mints at
  start and never returns through a command (a secret list link printed on stdout and kept in the
  data file, `trial-0.1.0-mixed-1`) cannot be named: the node declares those commands `none`, and
  their scenarios count as failed.

## First figures (2026-11-21, on copies of the runs in a scratch directory)

| run | figure | table node |
|---|---|---|
| `trial-0.1.0-strong-1` | 13 of 17 scenarios, contradicted: the four failures answer an unknown id 404 where the spec says `wrong-state`, and check existence before the forced `unreachable` | Sonnet, effort high, 1 attempt, 0.27 USD |
| `trial-0.1.0-mixed-1` | 1 of 19 scenarios, contradicted: every list command needs the link minted at start (declared `none`), the offline copy lives only in the browser, and `OpenList` with the spec's link answers `unknown-link` | Sonnet, effort high, 1 attempt, 0.22 USD |

A hand-written adapter for `trial-0.1.0-strong-1` (the feasibility probe) got the same 13 of 17
with the same four failures.
- Tests (`driver/tests/conformance.rs`) need the pinned ess installed.
