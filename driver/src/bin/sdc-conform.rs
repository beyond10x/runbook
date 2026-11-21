//! sdc-conform: the conformance observation of one ended run (docs/conformance.md).
//!
//! ```text
//! sdc-conform observe <run dir> [--tables <n>] [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
//! sdc-conform check-table --ir <ir.json> --table <table.json> --tree <delivered tree>
//! sdc-conform ess-path [--pin <ess-pin.json>]
//! sdc-conform ess-install [--archive <ess-….tar.gz>] [--pin <ess-pin.json>]
//! ```
//!
//! `observe` holds the run's delivered product (`work/published/` of the root case) to the
//! conformance suite of the run's own specification (`work/spec/`): it compiles and synthesizes
//! with the pinned ess (`driver/ess-pin.json`, never the `ess` on PATH), gets `--tables` route
//! tables (default 3, `SDC_CONFORMANCE_TABLES`), each from a fresh session of the node
//! `conformance.table` (or one from `--table`), checks each against the compiled specification,
//! builds a copy of the delivered tree per distinct build under `<run>/conformance/product*/`, and
//! runs `ess verify conform run --target process` with `sdc-conform-adapter` once per table. A
//! table whose run shows its own fault (a capture that never appears, a request naming a value the
//! scenario does not carry, 401 or 403 from the product) is refused and asked again once with that
//! evidence. It writes `<run>/oracle.json` (`sdc.oracle/1`, one check `conformance`: k passed, f
//! failed, d disputed, u not measurable of n scenarios over the counted tables, and how many scenarios
//! ended the same under all of them; `held` only when k = n > 0), `<run>/conformance-detail.json` (every
//! table run's ess `--format json` output) and its working files under `<run>/conformance/`. An
//! empty suite, a missing delivered tree, no counted table (missing, refused, product not built or
//! not started) and a missing or mismatched ess are each `unknown` with a named cause and no counts,
//! never a pass. Observation only: it changes nothing a run
//! decided. Exit 0 when oracle.json is written (whatever its result), 2 when it cannot be, 64 usage.
//!
//! The node `conformance.table` (class other): a fresh model session through the LLM adapter
//! (`SDC_CONFORMANCE_ADAPTER`, else `SDC_LLM_ADAPTER`, else claude-cli) with read-only operations
//! on a copy of the delivered tree, the specification and its compiled IR; model
//! `SDC_LLM_MODEL_CONFORMANCE` (default claude-sonnet-5-5), effort `SDC_LLM_EFFORT_CONFORMANCE`,
//! else `SDC_LLM_EFFORT_OTHER`, else `SDC_LLM_EFFORT`, else the class default of
//! `tools/node-classes.json`. Its reply is checked by the program (`check-table`); an invalid reply
//! is asked again with the check's reasons at most twice, then the table is `table_refused`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value as J};

#[path = "../conformance.rs"]
#[allow(dead_code)]
mod conformance;

use conformance::{check_table, declared_missing, needs_unreachable, not_reachable, read_json, resolve_ess, s, sha256_bytes, sha256_file, table_schema, Ess};

const USAGE: &str = "usage: sdc-conform observe <run dir> [--tables <n>] [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
       sdc-conform check-table --ir <ir.json> --table <table.json> --tree <delivered tree>
       sdc-conform ess-path [--pin <ess-pin.json>]
       sdc-conform ess-install [--archive <file>] [--pin <ess-pin.json>]";
const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
const DEFAULT_TABLES: usize = 3;
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const LIMITS: [&str; 4] = [
    "The product runs on this host without confinement (Substrate is not used yet); it is started from a copy of the delivered tree and bound to 127.0.0.1 only as far as it honours the port it is given.",
    "Events are built by the adapter from the HTTP response body by the table's templates, so event checks test the table as much as the product (events_checked_from: response_body); a literal is refused, and a check of a field filled from the input is recorded unverified (`unverified_checks`).",
    "The route tables are written by a model after the run. A scenario passes only if every counted table that reached it passed it, fails only if every one failed it, and is disputed when they disagree; the figure is an instrument reading, not a ledger metric (docs/conformance.md).",
    "Several statuses may mean one outcome, so a table decides which of the product's answers count as an outcome; the check refuses only a status mapped to two outcomes without a body discriminator.",
];

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default()
}

/// The repository's `driver/` directory: from `--pin`'s default and the tools dir, found upward
/// from the binary (`driver/target/<profile>/sdc-conform`).
fn driver_dir() -> Option<PathBuf> {
    exe_dir().ancestors().find(|p| p.join("ess-pin.json").is_file() && p.join("Cargo.toml").is_file()).map(Path::to_path_buf)
}

fn default_pin() -> PathBuf {
    std::env::var_os("SDC_ESS_PIN").map(PathBuf::from).or_else(|| driver_dir().map(|d| d.join("ess-pin.json"))).unwrap_or_else(|| PathBuf::from("driver/ess-pin.json"))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name();
        if name == "target" || name == ".git" || name == "node_modules" {
            continue;
        }
        let ft = e.file_type().map_err(|e| e.to_string())?;
        let dst = to.join(&name);
        if ft.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else if ft.is_file() {
            fs::copy(e.path(), &dst).map_err(|er| format!("{}: {er}", e.path().display()))?;
        }
    }
    Ok(())
}

/// Run a command with a time limit; `(exit code or None on timeout/spawn failure, stderr tail)`.
fn run_bounded(cmd: &mut Command, limit: Duration, stdout: Option<&Path>, stderr: &Path) -> (Option<i32>, String) {
    let out = stdout.and_then(|p| fs::File::create(p).ok()).map(Stdio::from).unwrap_or_else(Stdio::null);
    let err = fs::File::create(stderr).map(Stdio::from).unwrap_or_else(|_| Stdio::null());
    let mut child = match cmd.stdin(Stdio::null()).stdout(out).stderr(err).spawn() {
        Ok(c) => c,
        Err(e) => return (None, format!("cannot start: {e}")),
    };
    let end = Instant::now() + limit;
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) if Instant::now() > end => {
                let _ = child.kill();
                let _ = child.wait();
                return (None, format!("killed after {} s", limit.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return (None, e.to_string()),
        }
    };
    let mut text = String::new();
    let _ = fs::File::open(stderr).and_then(|mut f| f.read_to_string(&mut text));
    let tail: Vec<&str> = text.lines().rev().take(6).collect();
    (code, tail.into_iter().rev().collect::<Vec<_>>().join(" | "))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn first_env(names: &[&'static str]) -> Option<(String, &'static str)> {
    names.iter().find_map(|n| env(n).map(|v| (v, *n)))
}

struct Obs {
    run: PathBuf,
    rec: J,
}

/// The figure of a conformance record: `k passed, f failed, d disputed, u not measurable of n; t tables agree
/// on a of n` (`; 1 table` for one), with `, unknown: <cause>` for a measured unknown;
/// `not measured (<cause>), n scenarios` when no table was counted.
fn figure(c: &J) -> String {
    let cause = c["cause"].as_str().unwrap_or("no cause");
    let n = c["n"].as_u64();
    let (Some(k), Some(f), Some(u), Some(n)) = (c["k"].as_u64(), c["f"].as_u64(), c["u"].as_u64(), n) else {
        return match n {
            Some(n) => format!("not measured ({cause}), {n} scenarios"),
            None => format!("not measured ({cause})"),
        };
    };
    let d = c["d"].as_u64().unwrap_or(0);
    let mut text = format!("{k} passed, {f} failed, {d} disputed, {u} not measurable of {n}");
    match (c["tables_counted"].as_u64(), c["agreement"].as_u64()) {
        (Some(1), _) => text.push_str("; 1 table"),
        (Some(t), Some(a)) => text.push_str(&format!("; {t} tables agree on {a} of {n}")),
        _ => {}
    }
    if c["result"] == "unknown" {
        text.push_str(&format!(", unknown: {cause}"));
    }
    text
}

impl Obs {
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.run).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string())
    }

    /// Write oracle.json with the record so far and the result. k, f and u stay null unless a
    /// table was counted.
    fn finish(mut self, result: &str, cause: Option<&str>, why: Option<String>) -> Result<J, String> {
        self.rec["result"] = json!(result);
        self.rec["cause"] = json!(cause);
        self.rec["why"] = json!(why);
        if self.rec["tables_counted"].is_null() {
            self.rec["tables_counted"] = json!(0);
        }
        self.rec["measured"] = json!(self.rec["k"].is_u64());
        self.rec["summary"] = json!(figure(&self.rec));
        self.rec["limits"] = json!(LIMITS);
        let name = self.run.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let r = &self.rec;
        let check = json!({"id": "conformance", "result": result, "k": r["k"], "f": r["f"], "d": r["d"], "u": r["u"], "n": r["n"],
            "agreement": r["agreement"], "tables_counted": r["tables_counted"], "cause": cause});
        let doc = json!({"format": "sdc.oracle/1", "run": name, "checks": [check], "conformance": self.rec});
        let p = self.run.join("oracle.json");
        fs::write(&p, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n").map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(doc)
    }
}

fn effort() -> (String, String) {
    if let Some((v, n)) = first_env(&["SDC_LLM_EFFORT_CONFORMANCE", "SDC_LLM_EFFORT_OTHER", "SDC_LLM_EFFORT"]) {
        return (v, n.to_string());
    }
    let classes = tools_dir().map(|t| t.join("node-classes.json")).and_then(|p| read_json(&p).ok());
    match classes.as_ref().and_then(|c| c["default_efforts"]["other"].as_str()) {
        Some(e) => (e.to_string(), "node-classes.json default_efforts.other".into()),
        None => ("high".into(), "default".into()),
    }
}

fn tools_dir() -> Option<PathBuf> {
    env("SDC_TOOLS_DIR").map(PathBuf::from).or_else(|| driver_dir().map(|d| d.join("examples/sdc/tools")))
}

/// What the node is told about its previous table.
enum Feedback {
    None,
    /// The program check refused the table.
    Check(Vec<String>),
    /// The table was accepted, and its run showed faults of the table itself.
    Run(Vec<String>),
}

fn prompt(attempt: u32, feedback: &Feedback) -> String {
    let mut p = String::from(
        "You describe how to drive one delivered software product through its HTTP JSON interface, so that a\n\
program can hold it to the conformance suite of its specification. You change nothing; you only read.\n\
\n\
In the current directory: `product/` is the delivered source tree (read its README, its routes and its\n\
start command), `spec/` the specification it was built to, `ir.json` that specification compiled\n\
(`commands` with `input` and `outcomes`, `events`, `errors`, `views`).\n\
\n\
Reply with one route table (format sdc.conformance-table/1), exactly the reply schema:\n\
- `build`: the argv that builds the product inside its tree, or null. Rust: [\"cargo\", \"build\", \"--release\", \"--locked\", \"--offline\"].\n\
- `start.argv`: the argv that starts the built product, run in the tree. argv[0] is a path relative to the\n\
  tree (e.g. target/release/<bin>) or one of cargo, go, node, npm, deno, java, dotnet. Use the placeholders\n\
  {port} (required), {host} (127.0.0.1), {data_dir} (an empty directory), {data_file} (a file in it).\n\
  `start.from`: [{file, line}], the README or manifest (Cargo.toml, package.json, go.mod, pyproject.toml,\n\
  Procfile, Taskfile.yml, Makefile, deno.json, justfile) of the tree and the 1-based lines that state this\n\
  start command for normal use; together the cited lines must contain the program's file name and every\n\
  literal argument. Never use an argument or a header the README describes as a test, demo or mock mode.\n\
  `start.ready`: a request the product answers once it is up, e.g. {\"method\": \"GET\", \"path\": \"/\"}.\n\
- `captures` (optional): values the product prints when it starts, e.g. a link, a token or a board code\n\
  minted at start: {name, stream (stdout or stderr), regex (exactly one group), timeout_s (1-120)}.\n\
  \"{capture.<name>}\" reads one in paths, `headers`, bodies and templates. `bindings` (optional):\n\
  {input, capture} makes every input field of that name take the captured value instead of the\n\
  scenario's (e.g. the spec's link), and maps the captured value back in what the product answers.\n\
  Declare a capture only if the table uses it. If the product needs a sign-in, a token or a key,\n\
  send it in every request that needs it (a header or a body member, from a capture).\n\
- `commands`: one entry per command of ir.json: `request` {method, path, body, headers}; in `path`,\n\
  `body` and `headers`, \"{input.<field>}\" is the command's input value. An optional input field\n\
  (type Optional<...>) is often left out by a scenario: do not name it in a path or query. `outcomes`:\n\
  one entry per outcome of the command,\n\
  `status` the HTTP status the product answers for it, `events` the events the outcome emits with a\n\
  `payload` naming every field of the event, `error` {error, fields} for an outcome that declares one.\n\
  In payloads and error fields \"{response.<path>}\" reads the response body, \"{input.<field>}\" the input,\n\
  \"{lookup.<path>}\" the body of a `lookup` request (a GET made after the response, e.g. for a state the\n\
  response lacks). An outcome that is external (`condition.kind` external) needs a `forced` entry: the\n\
  request on the command's own method and route that makes the product take it (e.g. a body member\n\
  or a query parameter that simulates a lost connection).\n\
  `status` may be a list when several statuses mean one outcome (e.g. [409, 404] where the product\n\
  answers a missing entity 404 and the specification calls that case wrong-state). Two outcomes may\n\
  share a status only with a `when_body` {path, equals} on each. Payload and error fields are never\n\
  literals: fill each from the response, a lookup, a capture or the input. A field filled from the\n\
  input is recorded unverified, because the check then compares the input with itself; prefer the\n\
  response where the product echoes the value.\n\
- `views`: one entry per view: `request`, `rows` (the dotted path to the list of rows in the response\n\
  body, \"\" for the whole body), `fields` mapping each view field to its path in a row. Two views\n\
  cannot read the same request.\n\
- Describe the product as it is: map each outcome to the statuses the product gives for the case the\n\
  outcome describes. Where a command, outcome or view has no route, give an entry with\n\
  only its name and `none` {kind, why} instead of inventing a route. kind\n\
  `not_reachable_over_the_interface`: the product does it, but no HTTP client can drive or read it\n\
  (it lives only in the browser, or only an operator at the console can trigger it); its scenarios\n\
  are counted not measurable. kind `not_implemented`: the product does not do it; its scenarios fail.\n\
  Say in `why` where you looked.\n\
- `notes`: a few sentences on what you read and anything uncertain.\n",
    );
    match feedback {
        Feedback::None => {}
        Feedback::Check(invalid) => {
            p.push_str(&format!("\nThis is attempt {attempt}. Your previous table was refused by the program check:\n"));
            for why in invalid.iter().take(30) {
                p.push_str(&format!("  - {why}\n"));
            }
            p.push_str("Reply with the corrected table.\n");
        }
        Feedback::Run(evidence) => {
            p.push_str(&format!("\nThis is attempt {attempt}. Your previous table passed the program check but was refused after its run \
against the product: the run shows faults of the table itself, not of the product:\n"));
            for why in evidence.iter().take(30) {
                p.push_str(&format!("  - {why}\n"));
            }
            p.push_str("Read the product again where these point and reply with the corrected table.\n");
        }
    }
    p
}

/// The node `conformance.table` as configured for this observation.
struct Node {
    adapter: PathBuf,
    rec: J,
    model: String,
    effort: String,
    cwd: PathBuf,
    schema: PathBuf,
    /// The delivered tree, whose README or manifest the start command is checked against.
    tree: PathBuf,
}

fn node_setup(out: &Path, tree: &Path, spec: &Path, ir: &Path) -> Result<Node, (&'static str, String)> {
    let adapter_name = first_env(&["SDC_CONFORMANCE_ADAPTER", "SDC_LLM_ADAPTER"]).map(|x| x.0).unwrap_or_else(|| "claude-cli".into());
    let adapter = match adapter_name.as_str() {
        "stub" | "dry" | "fixture" => return Err(("table_missing", format!("the LLM adapter {adapter_name} has no model, so no table was written"))),
        "metaharness" | "claude-cli" => match tools_dir() {
            Some(t) => t.join(format!("llm-adapter-{adapter_name}.sh")),
            None => return Err(("table_missing", "the tools directory (driver/examples/sdc/tools) was not found".into())),
        },
        p => PathBuf::from(p),
    };
    let (model, model_source) = env("SDC_LLM_MODEL_CONFORMANCE").map(|m| (m, "SDC_LLM_MODEL_CONFORMANCE")).unwrap_or((DEFAULT_MODEL.into(), "default"));
    let (effort, effort_source) = effort();
    if !EFFORTS.contains(&effort.as_str()) {
        return Err(("table_missing", format!("effort {effort:?} ({effort_source}) is not one of {}", EFFORTS.join(", "))));
    }
    let cwd = out.join("node-cwd");
    let schema = out.join("reply-schema.json");
    let prep = || -> Result<(), String> {
        copy_tree(tree, &cwd.join("product"))?;
        copy_tree(spec, &cwd.join("spec"))?;
        fs::copy(ir, cwd.join("ir.json")).map_err(|e| e.to_string())?;
        fs::write(&schema, table_schema().to_string()).map_err(|e| e.to_string())
    };
    prep().map_err(|e| ("table_missing", e))?;
    let rec = json!({"source": "conformance.table", "adapter": adapter_name, "model": model, "model_source": model_source,
        "effort": effort, "effort_source": effort_source});
    Ok(Node { adapter, rec, model, effort, cwd, schema, tree: tree.to_path_buf() })
}

/// One slot's table from the node: at most `tries` fresh sessions, each reply checked by the
/// program and refused ones asked again with the reasons. `attempts` and `cost` count across the
/// slot. `Err((cause, why, last refusals))`.
#[allow(clippy::too_many_arguments)]
fn obtain(node: &Node, dir: &Path, slot: usize, attempts: &mut u32, cost: &mut f64, tries: u32, first: Feedback, ir_doc: &J)
    -> Result<J, (&'static str, String, Vec<String>)> {
    let mut feedback = first;
    let mut invalid = vec![];
    for _ in 0..tries {
        *attempts += 1;
        let attempt = *attempts;
        let rec = dir.join("node").join(format!("{attempt:03}"));
        fs::create_dir_all(&rec).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        fs::write(rec.join("prompt.md"), prompt(attempt, &feedback)).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        let req = json!({"model": node.model, "effort": node.effort, "prompt_file": rec.join("prompt.md"), "cwd": node.cwd, "record_dir": rec,
            "node": "conformance.table", "slot": slot, "attempt": attempt, "json_schema": node.schema,
            "operations": ["dir.list", "file.read", "search"],
            "max_turns": env("SDC_CONFORMANCE_MAX_TURNS").and_then(|x| x.parse::<u64>().ok()).unwrap_or(40),
            "max_budget_usd": env("SDC_CONFORMANCE_MAX_BUDGET_USD").and_then(|x| x.parse::<f64>().ok()).unwrap_or(2.0)});
        fs::write(rec.join("request.json"), serde_json::to_string_pretty(&req).unwrap_or_default()).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        let limit = Duration::from_secs(env("SDC_CONFORMANCE_NODE_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(1800));
        let (code, tail) = run_bounded(Command::new(&node.adapter).arg(rec.join("request.json")), limit, Some(&rec.join("response.json")), &rec.join("adapter-stderr.txt"));
        let resp = read_json(&rec.join("response.json")).unwrap_or(J::Null);
        *cost += resp["cost_usd"].as_f64().unwrap_or(0.0);
        let reply = fs::read_to_string(rec.join("structured-output.json")).ok();
        invalid = match reply.as_deref().map(serde_json::from_str::<J>) {
            None => vec![format!("no reply (adapter exit {code:?}: {tail})")],
            Some(Err(e)) => vec![format!("the reply is not JSON: {e}")],
            Some(Ok(t)) => {
                let p = check_table(&t, ir_doc, &node.tree);
                if p.is_empty() {
                    return Ok(t);
                }
                p
            }
        };
        fs::write(rec.join("refused.json"), serde_json::to_string_pretty(&invalid).unwrap_or_default()).ok();
        feedback = Feedback::Check(invalid.clone());
    }
    let missing = invalid.len() == 1 && invalid[0].starts_with("no reply");
    if missing {
        Err(("table_missing", invalid[0].clone(), invalid))
    } else {
        let why = format!("refused after {} attempt(s): {}", tries, invalid.join("; "));
        Err(("table_refused", why, invalid))
    }
}

fn ess_cmd(ess: &Ess) -> Command {
    let mut c = Command::new(&ess.path);
    c.env_remove("ESS_BIN");
    c
}

/// What every table run of one observation shares.
struct Env<'a> {
    ess: &'a Ess,
    run: PathBuf,
    out: PathBuf,
    suite: PathBuf,
    suite_doc: J,
    n: usize,
    tree: PathBuf,
    ir_doc: J,
}

impl Env<'_> {
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.run).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string())
    }
}

/// A built copy of the delivered tree, one per distinct `build` of the tables.
type Built = Result<(PathBuf, J), (&'static str, String, J)>;

fn build_product(cx: &Env, build: &J, index: usize) -> Built {
    let product = cx.out.join(if index == 1 { "product".to_string() } else { format!("product-{index}") });
    let mut rec = json!({"product": cx.rel(&product)});
    if let Err(e) = copy_tree(&cx.tree, &product) {
        return Err(("product_build_failed", e, rec));
    }
    let Some(argv) = build["argv"].as_array() else { return Ok((product, rec)) };
    let argv: Vec<String> = argv.iter().map(|a| s(a).to_string()).collect();
    let prog = if argv[0].contains('/') { product.join(&argv[0]) } else { PathBuf::from(&argv[0]) };
    let limit = Duration::from_secs(env("SDC_CONFORMANCE_BUILD_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(900));
    let mut cmd = Command::new(prog);
    cmd.args(&argv[1..]).current_dir(&product).env("CARGO_NET_OFFLINE", "true");
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().to_string();
        if ["SDC_", "ANTHROPIC_", "CLAUDE", "ESS_", "GH_", "GITHUB_"].iter().any(|p| k.starts_with(p)) {
            cmd.env_remove(&k);
        }
    }
    let log = product.with_extension("build.log");
    let (code, tail) = run_bounded(&mut cmd, limit, Some(&product.with_extension("build.stdout")), &log);
    rec["argv"] = json!(argv);
    rec["exit"] = json!(code);
    if code != Some(0) {
        return Err(("product_build_failed", format!("{} exit {code:?}: {tail}", argv.join(" ")), rec));
    }
    Ok((product, rec))
}

/// One table run against the product.
struct TableRun {
    /// `{run, sha256, k, f, u, e, n, conformance_status, counts, cause, why, evidence, …}`.
    rec: J,
    /// Scenario id → passed, failed, not_measurable or error.
    statuses: std::collections::BTreeMap<String, String>,
    /// A run-level end that makes the run count for nothing.
    cause: Option<(&'static str, String)>,
    /// Faults of the table itself the run shows, and their kind.
    evidence: Vec<String>,
    kind: &'static str,
    failed_checks: Vec<J>,
    notes: Vec<J>,
    detail: J,
    /// Checks of fields the table fills from the request's own input: `{scenario, about, fields}`.
    unverified: Vec<J>,
}

/// What the adapter log shows as the table's own fault: a capture that never appeared, a request
/// naming a value the scenario does not carry, the product refusing requests as unauthenticated.
fn table_evidence(events: &[J], n: usize) -> (Vec<String>, &'static str) {
    use std::collections::BTreeMap;
    let mut out = vec![];
    let mut caps: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for e in events.iter().filter(|e| e["event"] == "capture" && e["ok"] == false) {
        let c = caps.entry(s(&e["name"]).to_string()).or_insert((0, s(&e["why"]).to_string()));
        c.0 += 1;
    }
    for (name, (m, why)) in &caps {
        out.push(format!("capture {name}: {why}, in {m} of {n} scenarios"));
    }
    let mut unfillable: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in events.iter().filter(|e| e["event"] == "request_unfillable") {
        let about = if e["command"].is_string() { s(&e["command"]).to_string() } else { s(&e["view"]).to_string() };
        *unfillable.entry((about, s(&e["why"]).to_string())).or_default() += 1;
    }
    for ((about, why), m) in &unfillable {
        out.push(format!("the request of {about} cannot be filled in {m} scenario(s): {why} (the scenario does not carry that value)"));
    }
    let auth: Vec<&J> = events.iter()
        .filter(|e| (e["event"] == "unmapped_status" || e["event"] == "view_unreadable") && matches!(e["status"].as_u64(), Some(401 | 403)))
        .collect();
    if let Some(first) = auth.first() {
        let mut about: Vec<String> = auth.iter().map(|e| if e["command"].is_string() { s(&e["command"]).to_string() } else { s(&e["view"]).to_string() }).collect();
        about.sort();
        about.dedup();
        out.push(format!("the product answered {} to {} request(s) of {} (e.g. {}): the table sends no sign-in, token or key the product accepts",
            first["status"], auth.len(), about.join(", "), s(&first["body"])));
    }
    let kind = if caps.is_empty() { "table_error" } else { "capture_missing" };
    (out, kind)
}

/// The checks of a table run that compare a field the table fills from the request's own input:
/// a payload check (`payload of <event>`) or an error check (`error <error>`) whose compared fields
/// include one the table builds from `{input.…}`. `{scenario, about, fields}` for each check ess
/// reported.
fn unverified_checks(table: &J, suite: &J, det: &J) -> Vec<J> {
    let mut out = vec![];
    for sc in det["scenarios"].as_array().into_iter().flatten() {
        let id = s(&sc["scenario"]);
        let reported = |about: &str| sc["checks"].as_array().into_iter().flatten().any(|c| c["about"] == about);
        let (mut command, mut outcome) = (String::new(), String::new());
        for st in suite["scenarios"][id]["steps"].as_array().into_iter().flatten() {
            match s(&st["step"]) {
                "execute_command" | "execute_command_without_input" => {
                    command = s(&st["command"]).to_string();
                    outcome.clear();
                }
                "expect_outcome" => outcome = s(&st["outcome"]["outcome"]).to_string(),
                step @ ("expect_event" | "expect_event_values" | "expect_error") => {
                    let (about, event, compared) = if step == "expect_error" {
                        (format!("error {}", s(&st["error"])), None, &st["fields"])
                    } else {
                        let values = &st["payload"];
                        let compared = if values.as_object().is_some_and(|m| !m.is_empty()) { values } else { &st["shape"] };
                        (format!("payload of {}", s(&st["event"])), Some(s(&st["event"])), compared)
                    };
                    let filled = conformance::input_filled(table, &command, &outcome, event);
                    let fields: Vec<&String> = compared.as_object().into_iter().flatten().map(|(k, _)| k).filter(|k| filled.contains(*k)).collect();
                    if !fields.is_empty() && reported(&about) {
                        out.push(json!({"scenario": id, "about": about, "fields": fields}));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn run_table(cx: &Env, table: &J, dir: &Path, number: usize, products: &mut Vec<(String, Built)>) -> TableRun {
    let mut t = TableRun { rec: json!({"run": number}), statuses: Default::default(), cause: None, evidence: vec![], kind: "table_error",
        failed_checks: vec![], notes: vec![], detail: J::Null, unverified: vec![] };
    let fail = |mut t: TableRun, cause: &'static str, why: String| {
        t.rec["cause"] = json!(cause);
        t.rec["why"] = json!(why);
        t.cause = Some((cause, why));
        t
    };
    if let Err(e) = fs::create_dir_all(dir) {
        return fail(t, "adapter_refused", e.to_string());
    }
    let table_path = dir.join("table.json");
    let _ = fs::write(&table_path, serde_json::to_string_pretty(table).unwrap_or_default() + "\n");
    t.rec["sha256"] = json!(sha256_bytes(table.to_string().as_bytes()));
    let problems = check_table(table, &cx.ir_doc, &cx.tree);
    if !problems.is_empty() {
        t.rec["refused"] = json!(problems);
        return fail(t, "table_refused", problems.join("; "));
    }
    t.rec["declared_missing"] = json!(declared_missing(table));
    let key = table["build"].to_string();
    if !products.iter().any(|(k, _)| *k == key) {
        let built = build_product(cx, &table["build"], products.len() + 1);
        products.push((key.clone(), built));
    }
    let product = match &products.iter().find(|(k, _)| *k == key).expect("built above").1 {
        Ok((p, rec)) => {
            t.rec["build"] = rec.clone();
            p.clone()
        }
        Err((cause, why, rec)) => {
            t.rec["build"] = rec.clone();
            let (cause, why) = (*cause, why.clone());
            return fail(t, cause, why);
        }
    };

    let adapter = env("SDC_CONFORM_ADAPTER_BIN").map(PathBuf::from).unwrap_or_else(|| exe_dir().join("sdc-conform-adapter"));
    let report = dir.join("report.json");
    let detail = dir.join("detail.json");
    let log = dir.join("adapter-log.jsonl");
    let n = cx.n;
    let timeout: u64 = env("SDC_CONFORMANCE_TARGET_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(30);
    let mut cmd = ess_cmd(cx.ess);
    cmd.args(["verify", "conform", "run", "--suite"]).arg(&cx.suite)
        .args(["--target", "process", "--target-dir"]).arg(dir)
        .args(["--target-timeout", &timeout.to_string(), "--report-format", "2", "--strict", "--format", "json", "--report-out"]).arg(&report)
        .arg("--").arg(&adapter).arg("--table").arg(&table_path).arg("--tree").arg(&product)
        .arg("--work").arg(dir.join("scenarios")).arg("--log").arg(&log)
        .args(["--start-timeout", "20", "--request-timeout", &timeout.min(20).to_string()]);
    let limit = Duration::from_secs((n as u64 + 1) * (2 * timeout + 25) + 60);
    let (code, tail) = run_bounded(&mut cmd, limit, Some(&detail), &dir.join("ess-run.stderr"));
    t.rec["ess_exit"] = json!(code);
    let Ok(rep) = read_json(&report) else {
        return fail(t, "adapter_refused", format!("ess exit {code:?}, no report: {tail}"));
    };
    let det = read_json(&detail).unwrap_or(J::Null);
    let mut ran: Vec<(String, String)> = det["scenarios"].as_array().into_iter().flatten().map(|sc| (s(&sc["scenario"]).to_string(), s(&sc["status"]).to_string())).collect();
    if ran.is_empty() {
        for (status, ids) in rep["outcomes"].as_object().into_iter().flatten() {
            for id in ids.as_array().into_iter().flatten() {
                ran.push((s(id).to_string(), status.clone()));
            }
        }
    }
    let nr = not_reachable(table);
    // A failed check is an observed contradiction: such a scenario fails even when it also needs
    // what the interface cannot reach.
    let contradicts = |id: &str| det["scenarios"].as_array().into_iter().flatten()
        .filter(|sc| sc["scenario"] == id)
        .any(|sc| sc["checks"].as_array().into_iter().flatten().any(|c| c["status"] == "failed"));
    let unverified = unverified_checks(table, &cx.suite_doc, &det);
    // A scenario whose every check compares the input with itself observed nothing of the product.
    let all_unverified = |id: &str| det["scenarios"].as_array().into_iter().flatten().find(|sc| sc["scenario"] == id).is_some_and(|sc| {
        let checks = sc["checks"].as_array().map(Vec::as_slice).unwrap_or_default();
        !checks.is_empty() && checks.iter().all(|c| unverified.iter().any(|u| u["scenario"] == id && u["about"] == c["about"]))
    });
    for (id, st) in &ran {
        let mine = match st.as_str() {
            "passed" if all_unverified(id) => "not_measurable",
            "passed" => "passed",
            "error" => "error",
            _ if !contradicts(id) && needs_unreachable(&cx.suite_doc["scenarios"][id.as_str()], &nr) => "not_measurable",
            _ => "failed",
        };
        t.statuses.insert(id.clone(), mine.to_string());
    }
    for sc in det["scenarios"].as_array().into_iter().flatten() {
        let mine = t.statuses.get(s(&sc["scenario"])).map(String::as_str).unwrap_or("");
        if mine == "failed" || mine == "error" {
            for c in sc["checks"].as_array().into_iter().flatten().filter(|c| c["status"] != "passed") {
                t.failed_checks.push(json!({"scenario": sc["scenario"], "status": c["status"], "about": c["about"], "code": c["code"],
                    "expected": c["diagnostic"]["expected"], "observed": c["diagnostic"]["observed"]}));
            }
        }
    }
    let count = |st: &str| t.statuses.values().filter(|x| *x == st).count();
    let (k, f, u, e) = (count("passed"), count("failed"), count("not_measurable"), count("error"));
    let total = rep["counts"]["total"].as_u64().unwrap_or(0) as usize;
    t.rec["k"] = json!(k);
    t.rec["f"] = json!(f);
    t.rec["u"] = json!(u);
    t.rec["e"] = json!(e);
    t.rec["n"] = json!(n);
    t.rec["conformance_status"] = rep["conformance_status"].clone();
    t.rec["counts"] = rep["counts"].clone();
    t.rec["unverified"] = json!(unverified.len());
    t.unverified = unverified;
    t.detail = det;
    let events: Vec<J> = fs::read_to_string(&log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<J>(l).ok()).collect();
    t.notes = events.iter().filter(|e| e["event"] != "start" && e["event"] != "request").cloned().collect();
    let not_started: Vec<&J> = events.iter().filter(|e| e["event"] == "start" && e["ok"] == false).collect();
    if let Some(first) = not_started.first() {
        let why = format!("{} of {n} scenarios: {}", not_started.len(), s(&first["why"]));
        return fail(t, "product_not_started", why);
    }
    if total != n {
        return fail(t, "suite_mismatch", format!("ess ran {total} scenarios, the suite holds {n}"));
    }
    let (evidence, kind) = table_evidence(&events, n);
    t.rec["evidence"] = json!(evidence);
    t.evidence = evidence;
    t.kind = kind;
    t
}

fn observe(run: &Path, table_file: Option<PathBuf>, tables: usize, pin: &Path) -> Result<J, String> {
    use std::collections::BTreeMap;
    let run = fs::canonicalize(run).map_err(|e| format!("{}: {e}", run.display()))?;
    let out = run.join("conformance");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let _ = fs::remove_file(run.join("conformance-detail.json"));
    let mut o = Obs { run: run.clone(), rec: json!({"events_checked_from": "response_body",
        "k": null, "f": null, "u": null, "n": null, "agreement": null, "tables_counted": 0}) };

    let idea = run.join("store/IDEA-1");
    let root = if idea.is_dir() { idea } else {
        let mut found: Vec<PathBuf> = fs::read_dir(run.join("store")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("work").is_dir()).collect();
        found.sort();
        found.into_iter().next().unwrap_or_else(|| run.join("store/IDEA-1"))
    };
    let work = root.join("work");
    let tree = ["published", "release"].iter().map(|d| work.join(d)).find(|p| p.is_dir());
    let spec = work.join("spec");

    let ess = match resolve_ess(pin) {
        Ok(e) => e,
        Err((cause, why)) => return o.finish("unknown", Some(cause), Some(why)),
    };
    o.rec["ess"] = json!({"version": ess.version, "sha256": ess.sha256, "source": ess.source});
    if !spec.is_dir() {
        let why = format!("{} has no spec/", o.rel(&work));
        return o.finish("unknown", Some("suite_failed"), Some(why));
    }
    let ir = out.join("ir.json");
    let (code, tail) = run_bounded(ess_cmd(&ess).args(["compile", "--path"]).arg(&spec).arg("--out").arg(&ir), Duration::from_secs(120), None, &out.join("compile.stderr"));
    if code != Some(0) {
        return o.finish("unknown", Some("suite_failed"), Some(format!("ess compile exit {code:?}: {tail}")));
    }
    let suite = out.join("suite.json");
    let (code, tail) = run_bounded(ess_cmd(&ess).args(["verify", "conform", "synthesize", "--suite-format", "5", "--path"]).arg(&spec).arg("--out").arg(&suite),
        Duration::from_secs(120), None, &out.join("synthesize.stderr"));
    if code != Some(0) {
        return o.finish("unknown", Some("suite_failed"), Some(format!("ess verify conform synthesize exit {code:?}: {tail}")));
    }
    let suite_doc = read_json(&suite)?;
    let n = suite_doc["scenarios"].as_object().map(|m| m.len()).unwrap_or(0);
    o.rec["n"] = json!(n);
    o.rec["suite"] = json!({"sha256": sha256_file(&suite)?, "scenarios": n});
    if n == 0 {
        return o.finish("unknown", Some("suite_empty"), Some("the specification synthesizes no scenario".into()));
    }
    let Some(tree) = tree else {
        let why = format!("{} has neither published/ nor release/", o.rel(&work));
        return o.finish("unknown", Some("no_delivered_tree"), Some(why));
    };
    o.rec["product"] = json!({"tree": o.rel(&tree), "provenance": fs::read_to_string(tree.join("PROVENANCE")).ok().map(|x| x.trim().to_string())});
    let ir_doc = read_json(&ir)?;

    let node = match &table_file {
        Some(_) => None,
        None => match node_setup(&out, &tree, &spec, &ir) {
            Ok(nd) => Some(nd),
            Err((cause, why)) => return o.finish("unknown", Some(cause), Some(why)),
        },
    };
    let slots = if table_file.is_some() { 1 } else { tables };
    o.rec["tables_written"] = json!(slots);
    let cx = Env { ess: &ess, run: run.clone(), out: out.clone(), suite: suite.clone(), suite_doc, n, tree: tree.clone(), ir_doc };
    let mut products: Vec<(String, Built)> = vec![];
    let mut recs: Vec<J> = vec![];
    let mut counted: Vec<(usize, TableRun)> = vec![];
    let mut details: Vec<J> = vec![];
    for slot in 1..=slots {
        let sdir = out.join(format!("table-{slot}"));
        let mut rec = match &node {
            Some(nd) => nd.rec.clone(),
            None => json!({"source": "file"}),
        };
        rec["slot"] = json!(slot);
        rec["retried_after_run"] = json!(false);
        let (mut attempts, mut cost) = (0u32, 0.0f64);
        let first = match (&table_file, &node) {
            (Some(f), _) => read_json(f).map_err(|e| ("table_missing", e, vec![])),
            (None, Some(nd)) => obtain(nd, &sdir, slot, &mut attempts, &mut cost, 3, Feedback::None, &cx.ir_doc),
            (None, None) => unreachable!("a node exists without a table file"),
        };
        let mut runs: Vec<J> = vec![];
        let mut ended: Option<(&'static str, String)> = None;
        let mut last: Option<TableRun> = None;
        match first {
            Err((cause, why, refusals)) => {
                rec["refused"] = json!(refusals);
                ended = Some((cause, why));
            }
            Ok(table) => {
                let mut r = run_table(&cx, &table, &sdir.join("run-1"), 1, &mut products);
                details.push(json!({"table": slot, "run": 1, "ess": r.detail.clone()}));
                runs.push(r.rec.clone());
                if r.cause.is_none() && !r.evidence.is_empty() {
                    rec["evidence"] = json!(r.evidence);
                    if let Some(nd) = &node {
                        rec["retried_after_run"] = json!(true);
                        match obtain(nd, &sdir, slot, &mut attempts, &mut cost, 1, Feedback::Run(r.evidence.clone()), &cx.ir_doc) {
                            Ok(t2) => {
                                r = run_table(&cx, &t2, &sdir.join("run-2"), 2, &mut products);
                                details.push(json!({"table": slot, "run": 2, "ess": r.detail.clone()}));
                                runs.push(r.rec.clone());
                            }
                            Err((cause, why, refusals)) => {
                                rec["refused"] = json!(refusals);
                                ended = Some((cause, format!("asked again after its run: {why}")));
                            }
                        }
                    }
                }
                if ended.is_none() {
                    ended = match (&r.cause, r.evidence.is_empty()) {
                        (Some((c, w)), _) => Some((c, w.clone())),
                        (None, false) => Some((r.kind, format!("refused after its run: {}", r.evidence.join("; ")))),
                        (None, true) => None,
                    };
                }
                last = Some(r);
            }
        }
        if node.is_some() {
            rec["attempts"] = json!(attempts);
        }
        rec["cost_usd"] = json!((cost * 1e6).round() / 1e6);
        if let Some(r) = &last {
            for key in ["sha256", "k", "f", "u", "e", "n", "unverified", "conformance_status", "declared_missing", "build"] {
                rec[key] = r.rec[key].clone();
            }
        }
        rec["runs"] = J::Array(runs);
        rec["counted"] = json!(ended.is_none());
        rec["cause"] = json!(ended.as_ref().map(|x| x.0));
        rec["why"] = json!(ended.as_ref().map(|x| x.1.clone()));
        if let (None, Some(r)) = (&ended, last) {
            counted.push((slot, r));
        }
        recs.push(rec);
    }
    let total_cost: f64 = recs.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
    o.rec["table_cost_usd"] = json!((total_cost * 1e6).round() / 1e6);
    o.rec["tables"] = J::Array(recs.clone());
    let detail_doc = json!({"format": "sdc.conformance-detail/1", "runs": details});
    fs::write(run.join("conformance-detail.json"), serde_json::to_string_pretty(&detail_doc).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;

    if counted.is_empty() {
        // The most frequent cause of the slots, the earliest slot's on a tie.
        let cause_of = |r: &J| r["cause"].as_str().unwrap_or("table_missing").to_string();
        let mut causes: BTreeMap<String, usize> = BTreeMap::new();
        for r in &recs {
            *causes.entry(cause_of(r)).or_default() += 1;
        }
        let mut best: Option<(String, usize)> = None;
        for r in &recs {
            let c = cause_of(r);
            let m = causes[&c];
            if best.as_ref().is_none_or(|b| m > b.1) {
                best = Some((c, m));
            }
        }
        let cause = best.map(|b| b.0).unwrap_or_else(|| "table_missing".into());
        let why = recs.iter().find(|r| cause_of(r) == cause).map(|r| s(&r["why"]).to_string());
        return o.finish("unknown", Some(&cause), why);
    }

    // The figure over the counted tables. A table reaches a scenario when it passed or failed it.
    // The scenario passes when every table that reached it passed it, fails when every one failed
    // it, is disputed when they disagree, and is not measurable when none reached it.
    let ids: Vec<String> = cx.suite_doc["scenarios"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let mut scenarios = vec![];
    let (mut k, mut f, mut d, mut u, mut agree) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for id in &ids {
        let under: Vec<J> = (1..=slots).map(|slot| counted.iter().find(|(s, _)| *s == slot)
            .map_or(J::Null, |(_, r)| json!(r.statuses.get(id).cloned().unwrap_or_else(|| "error".into())))).collect();
        let got: Vec<&str> = under.iter().filter_map(J::as_str).collect();
        let status = match (got.contains(&"passed"), got.contains(&"failed")) {
            (true, true) => "disputed",
            (true, false) => "passed",
            (false, true) => "failed",
            (false, false) => "not_measurable",
        };
        match status {
            "passed" => k += 1,
            "failed" => f += 1,
            "disputed" => d += 1,
            _ => u += 1,
        }
        if got.windows(2).all(|w| w[0] == w[1]) {
            agree += 1;
        }
        scenarios.push(json!({"id": id, "status": status, "under": under}));
    }
    let mut failed_checks = vec![];
    let mut notes = vec![];
    let mut unverified = vec![];
    for (slot, r) in &counted {
        for c in &r.unverified {
            let mut c = c.clone();
            c["table"] = json!(slot);
            unverified.push(c);
        }
        for c in &r.failed_checks {
            let failed = scenarios.iter().any(|x| x["id"] == c["scenario"] && (x["status"] == "failed" || x["status"] == "disputed"));
            if failed {
                let mut c = c.clone();
                c["table"] = json!(slot);
                failed_checks.push(c);
            }
        }
        for e in &r.notes {
            let mut e = e.clone();
            e["table"] = json!(slot);
            notes.push(e);
        }
    }
    o.rec["k"] = json!(k);
    o.rec["f"] = json!(f);
    o.rec["d"] = json!(d);
    o.rec["u"] = json!(u);
    o.rec["agreement"] = json!(agree);
    o.rec["tables_counted"] = json!(counted.len());
    o.rec["scenarios"] = J::Array(scenarios);
    o.rec["failed_checks"] = J::Array(failed_checks);
    o.rec["adapter_notes"] = J::Array(notes);
    o.rec["unverified_checks"] = J::Array(unverified);
    if k == n {
        return o.finish("held", None, None);
    }
    if f > 0 {
        return o.finish("contradicted", None, None);
    }
    if d > 0 {
        return o.finish("unknown", Some("disputed"), Some(format!("{d} of {n} scenarios passed under one counted table and failed under another")));
    }
    o.finish("unknown", Some("not_measurable"), Some(format!("{u} of {n} scenarios passed and failed under no counted table: they need what the product's interface cannot reach, or ended error under every counted table")))
}

fn ess_install(archive: Option<PathBuf>, pin_path: &Path) -> Result<String, String> {
    let pin = read_json(pin_path)?;
    let target = conformance::target_triple();
    let p = &pin["platforms"][&target];
    let asset = s(&p["asset"]).to_string();
    if asset.is_empty() {
        return Err(format!("the pin names no archive for {target}"));
    }
    let dest = conformance::cache_path(&pin);
    let dir = dest.parent().ok_or("no cache directory")?.to_path_buf();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive = match archive {
        Some(a) => a,
        None => {
            let a = dir.join(&asset);
            let url = format!("https://github.com/beyond10x/ess/releases/download/{}/{asset}", s(&pin["version"]));
            let st = Command::new("curl").args(["-fsSL", "-o"]).arg(&a).arg(&url).status().map_err(|e| format!("curl: {e}"))?;
            if !st.success() {
                return Err(format!("curl {url}: {st}"));
            }
            a
        }
    };
    let got = sha256_file(&archive)?;
    if got != s(&p["archive_sha256"]) {
        return Err(format!("{} has sha256 {got}, the pin {}", archive.display(), s(&p["archive_sha256"])));
    }
    let unpack = dir.join("unpack");
    let _ = fs::remove_dir_all(&unpack);
    fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;
    let st = Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(&unpack).arg(s(&p["member"])).status().map_err(|e| format!("tar: {e}"))?;
    if !st.success() {
        return Err(format!("tar: {st}"));
    }
    let bin = unpack.join(s(&p["member"]));
    let got = sha256_file(&bin)?;
    if got != s(&p["binary_sha256"]) {
        return Err(format!("the unpacked binary has sha256 {got}, the pin {}", s(&p["binary_sha256"])));
    }
    fs::rename(&bin, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let _ = fs::remove_dir_all(&unpack);
    Ok(dest.display().to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pin = arg(&args, "--pin").map(PathBuf::from).unwrap_or_else(default_pin);
    match args.first().map(String::as_str) {
        Some("observe") if args.len() >= 2 && !args[1].starts_with("--") => {
            let tables = arg(&args, "--tables").or_else(|| env("SDC_CONFORMANCE_TABLES")).map(|x| x.parse::<usize>());
            let tables = match tables {
                None => DEFAULT_TABLES,
                Some(Ok(t)) if (1..=9).contains(&t) => t,
                Some(_) => {
                    eprintln!("sdc-conform: --tables (or SDC_CONFORMANCE_TABLES) is a number from 1 to 9");
                    return ExitCode::from(64);
                }
            };
            match observe(Path::new(&args[1]), arg(&args, "--table").map(PathBuf::from), tables, &pin) {
                Ok(doc) => {
                    println!("conformance: {}", s(&doc["conformance"]["summary"]));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-conform: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("check-table") => {
            let (Some(ir), Some(t), Some(tree)) = (arg(&args, "--ir"), arg(&args, "--table"), arg(&args, "--tree")) else {
                eprintln!("{USAGE}");
                return ExitCode::from(64);
            };
            match (read_json(Path::new(&ir)), read_json(Path::new(&t))) {
                (Ok(ir), Ok(t)) => {
                    let p = check_table(&t, &ir, Path::new(&tree));
                    println!("{}", json!({"accepted": p.is_empty(), "problems": p, "declared_missing": declared_missing(&t)}));
                    if p.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("sdc-conform: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("ess-path") => match resolve_ess(&pin) {
            Ok(e) => {
                println!("{}", e.path.display());
                ExitCode::SUCCESS
            }
            Err((cause, why)) => {
                eprintln!("sdc-conform: {cause}: {why}");
                ExitCode::from(1)
            }
        },
        Some("ess-install") => match ess_install(arg(&args, "--archive").map(PathBuf::from), &pin) {
            Ok(p) => {
                println!("{p}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-conform: {e}");
                ExitCode::from(1)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}
