//! sdc-conform: the conformance observation of one ended run (docs/conformance.md).
//!
//! ```text
//! sdc-conform observe <run dir> [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
//! sdc-conform check-table --ir <ir.json> --table <table.json>
//! sdc-conform ess-path [--pin <ess-pin.json>]
//! sdc-conform ess-install [--archive <ess-….tar.gz>] [--pin <ess-pin.json>]
//! ```
//!
//! `observe` holds the run's delivered product (`work/published/` of the root case) to the
//! conformance suite of the run's own specification (`work/spec/`): it compiles and synthesizes
//! with the pinned ess (`driver/ess-pin.json`, never the `ess` on PATH), gets a route table from the
//! node `conformance.table` (or `--table`), checks it against the compiled specification, builds a
//! copy of the delivered tree under `<run>/conformance/product/`, and runs
//! `ess verify conform run --target process` with `sdc-conform-adapter`. It writes
//! `<run>/oracle.json` (`sdc.oracle/1`, one check `conformance`: k passed, f failed, u not
//! measurable of n scenarios; `held` only when ess says `passed` and k = n > 0; not measurable is a
//! scenario that needs what the table declares `none` of kind `not_reachable_over_the_interface`),
//! `<run>/conformance-detail.json` (ess's `--format json` output) and its working files under
//! `<run>/conformance/`. An empty suite, a missing delivered tree, a product that does not build or
//! start, a capture that does not appear, a missing or refused table and a missing or mismatched
//! ess are each `unknown` with a named cause, never a pass. Observation only: it changes nothing a run
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

const USAGE: &str = "usage: sdc-conform observe <run dir> [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
       sdc-conform check-table --ir <ir.json> --table <table.json>
       sdc-conform ess-path [--pin <ess-pin.json>]
       sdc-conform ess-install [--archive <file>] [--pin <ess-pin.json>]";
const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const LIMITS: [&str; 3] = [
    "The product runs on this host without confinement (Substrate is not used yet); it is started from a copy of the delivered tree and bound to 127.0.0.1 only as far as it honours the port it is given.",
    "Events are built by the adapter from the HTTP response body by the table's templates, so event checks test the table as much as the product (events_checked_from: response_body).",
    "The route table is written by a model after the run; a wrong table can fail or pass scenarios the product would not.",
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
    out: PathBuf,
    rec: J,
}

impl Obs {
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.run).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string())
    }

    /// Write oracle.json with the record so far and the result.
    fn finish(mut self, result: &str, cause: Option<&str>, why: Option<String>) -> Result<J, String> {
        let n = self.rec["n"].as_u64();
        let num = |x: &str| self.rec[x].as_u64().unwrap_or(0);
        let (k, f, u) = (num("k"), num("f"), num("u"));
        self.rec["k"] = json!(k);
        self.rec["f"] = json!(f);
        self.rec["u"] = json!(u);
        self.rec["result"] = json!(result);
        self.rec["cause"] = json!(cause);
        self.rec["why"] = json!(why);
        self.rec["measured"] = json!(result != "unknown" || cause == Some("not_measurable"));
        self.rec["summary"] = json!(match (n, cause) {
            (Some(n), None) => format!("{k} passed, {f} failed, {u} not measurable of {n}"),
            (Some(n), Some(c)) => format!("{k} passed, {f} failed, {u} not measurable of {n}, {result}: {c}"),
            (None, Some(c)) => format!("{result}: {c}"),
            (None, None) => result.to_string(),
        });
        self.rec["limits"] = json!(LIMITS);
        let name = self.run.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let doc = json!({"format": "sdc.oracle/1", "run": name,
            "checks": [{"id": "conformance", "result": result, "k": k, "f": f, "u": u, "n": n, "cause": cause}],
            "conformance": self.rec});
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

fn prompt(attempt: u32, invalid: &[String]) -> String {
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
  `start.ready`: a request the product answers once it is up, e.g. {\"method\": \"GET\", \"path\": \"/\"}.\n\
- `captures` (optional): values the product prints when it starts, e.g. a link, a token or a board code\n\
  minted at start: {name, stream (stdout or stderr), regex (exactly one group), timeout_s (1-120)}.\n\
  \"{capture.<name>}\" reads one in paths, `headers`, bodies and templates. `bindings` (optional):\n\
  {input, capture} makes every input field of that name take the captured value instead of the\n\
  scenario's (e.g. the spec's link), and maps the captured value back in what the product answers.\n\
  Declare a capture only if the table uses it.\n\
- `commands`: one entry per command of ir.json: `request` {method, path, body, headers}; in `path`,\n\
  `body` and `headers`, \"{input.<field>}\" is the command's input value. `outcomes`: one entry per outcome of the command,\n\
  `status` the HTTP status the product answers for it, `events` the events the outcome emits with a\n\
  `payload` naming every field of the event, `error` {error, fields} for an outcome that declares one.\n\
  In payloads and error fields \"{response.<path>}\" reads the response body, \"{input.<field>}\" the input,\n\
  \"{lookup.<path>}\" the body of a `lookup` request (a GET made after the response, e.g. for a state the\n\
  response lacks). An outcome that is external (`condition.kind` external) needs a `forced` entry: the\n\
  request that makes the product take it (e.g. a body member that simulates a lost connection).\n\
  Two outcomes may share a status only with a `when_body` {path, equals} on each.\n\
- `views`: one entry per view: `request`, `rows` (the dotted path to the list of rows in the response\n\
  body, \"\" for the whole body), `fields` mapping each view field to its path in a row.\n\
- Describe the product as it is. Where it answers a case with a status the specification does not\n\
  declare (for example 404 for an unknown id where the specification says wrong-state), do not map\n\
  that status to an outcome to make a check pass: map the outcome to the status the product gives for\n\
  the case the outcome describes. Where a command, outcome or view has no route, give an entry with\n\
  only its name and `none` {kind, why} instead of inventing a route. kind\n\
  `not_reachable_over_the_interface`: the product does it, but no HTTP client can drive or read it\n\
  (it lives only in the browser, or only an operator at the console can trigger it); its scenarios\n\
  are counted not measurable. kind `not_implemented`: the product does not do it; its scenarios fail.\n\
  Say in `why` where you looked.\n\
- `notes`: a few sentences on what you read and anything uncertain.\n",
    );
    if attempt > 1 {
        p.push_str(&format!("\nThis is attempt {attempt}. Your previous table was refused by the program check:\n"));
        for why in invalid.iter().take(30) {
            p.push_str(&format!("  - {why}\n"));
        }
        p.push_str("Reply with the corrected table.\n");
    }
    p
}

/// The table from the node `conformance.table`: `Ok(table)` accepted, `Err((cause, why))`.
fn table_node(o: &mut Obs, tree: &Path, spec: &Path, ir: &Path) -> Result<J, (&'static str, String)> {
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
    let base = o.out.join("table-node");
    let cwd = base.join("cwd");
    let prep = || -> Result<(), String> {
        copy_tree(tree, &cwd.join("product"))?;
        copy_tree(spec, &cwd.join("spec"))?;
        fs::copy(ir, cwd.join("ir.json")).map_err(|e| e.to_string())?;
        fs::write(base.join("reply-schema.json"), table_schema().to_string()).map_err(|e| e.to_string())
    };
    prep().map_err(|e| ("table_missing", e))?;
    let ir_doc = read_json(ir).map_err(|e| ("suite_failed", e))?;
    let (mut invalid, mut cost) = (Vec::<String>::new(), 0.0f64);
    o.rec["table"] = json!({"source": "conformance.table", "adapter": adapter_name, "model": model, "model_source": model_source,
        "effort": effort, "effort_source": effort_source});
    for attempt in 1..=3u32 {
        let rec = base.join(format!("{attempt:03}"));
        fs::create_dir_all(&rec).map_err(|e| ("table_missing", e.to_string()))?;
        fs::write(rec.join("prompt.md"), prompt(attempt, &invalid)).map_err(|e| ("table_missing", e.to_string()))?;
        let req = json!({"model": model, "effort": effort, "prompt_file": rec.join("prompt.md"), "cwd": cwd, "record_dir": rec,
            "node": "conformance.table", "attempt": attempt, "json_schema": base.join("reply-schema.json"),
            "operations": ["dir.list", "file.read", "search"],
            "max_turns": env("SDC_CONFORMANCE_MAX_TURNS").and_then(|x| x.parse::<u64>().ok()).unwrap_or(40),
            "max_budget_usd": env("SDC_CONFORMANCE_MAX_BUDGET_USD").and_then(|x| x.parse::<f64>().ok()).unwrap_or(2.0)});
        fs::write(rec.join("request.json"), serde_json::to_string_pretty(&req).unwrap_or_default()).map_err(|e| ("table_missing", e.to_string()))?;
        let limit = Duration::from_secs(env("SDC_CONFORMANCE_NODE_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(1800));
        let (code, tail) = run_bounded(Command::new(&adapter).arg(rec.join("request.json")), limit, Some(&rec.join("response.json")), &rec.join("adapter-stderr.txt"));
        let resp = read_json(&rec.join("response.json")).unwrap_or(J::Null);
        cost += resp["cost_usd"].as_f64().unwrap_or(0.0);
        o.rec["table"]["attempts"] = json!(attempt);
        o.rec["table"]["cost_usd"] = json!((cost * 1e6).round() / 1e6);
        let reply = fs::read_to_string(rec.join("structured-output.json")).ok();
        invalid = match reply.as_deref().map(serde_json::from_str::<J>) {
            None => vec![format!("no reply (adapter exit {code:?}: {tail})")],
            Some(Err(e)) => vec![format!("the reply is not JSON: {e}")],
            Some(Ok(t)) => {
                let p = check_table(&t, &ir_doc);
                if p.is_empty() {
                    return Ok(t);
                }
                p
            }
        };
        fs::write(rec.join("refused.json"), serde_json::to_string_pretty(&invalid).unwrap_or_default()).ok();
    }
    o.rec["table"]["refused"] = json!(invalid);
    let missing = invalid.len() == 1 && invalid[0].starts_with("no reply");
    Err(if missing { ("table_missing", invalid[0].clone()) } else { ("table_refused", format!("refused after 2 retries: {}", invalid.join("; "))) })
}

fn ess_cmd(ess: &Ess) -> Command {
    let mut c = Command::new(&ess.path);
    c.env_remove("ESS_BIN");
    c
}

fn observe(run: &Path, table_file: Option<PathBuf>, pin: &Path) -> Result<J, String> {
    let run = fs::canonicalize(run).map_err(|e| format!("{}: {e}", run.display()))?;
    let out = run.join("conformance");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let _ = fs::remove_file(run.join("conformance-detail.json"));
    let mut o = Obs { run: run.clone(), out: out.clone(), rec: json!({"events_checked_from": "response_body", "k": 0, "n": null}) };

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

    let table = match table_file {
        Some(f) => {
            let t = read_json(&f);
            o.rec["table"] = json!({"source": "file"});
            match t {
                Ok(t) => t,
                Err(e) => return o.finish("unknown", Some("table_missing"), Some(e)),
            }
        }
        None => match table_node(&mut o, &tree, &spec, &ir) {
            Ok(t) => t,
            Err((cause, why)) => return o.finish("unknown", Some(cause), Some(why)),
        },
    };
    let table_path = out.join("table.json");
    fs::write(&table_path, serde_json::to_string_pretty(&table).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
    o.rec["table"]["sha256"] = json!(sha256_bytes(table.to_string().as_bytes()));
    let problems = check_table(&table, &read_json(&ir)?);
    if !problems.is_empty() {
        o.rec["table"]["refused"] = json!(problems);
        return o.finish("unknown", Some("table_refused"), Some(problems.join("; ")));
    }
    o.rec["declared_missing"] = json!(declared_missing(&table));

    let product = out.join("product");
    copy_tree(&tree, &product)?;
    if let Some(argv) = table["build"]["argv"].as_array() {
        let argv: Vec<String> = argv.iter().map(|a| s(a).to_string()).collect();
        let prog = if argv[0].contains('/') { product.join(&argv[0]) } else { PathBuf::from(&argv[0]) };
        let limit = Duration::from_secs(env("SDC_CONFORMANCE_BUILD_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(900));
        let mut build = Command::new(prog);
        build.args(&argv[1..]).current_dir(&product).env("CARGO_NET_OFFLINE", "true");
        for (k, _) in std::env::vars_os() {
            let k = k.to_string_lossy().to_string();
            if ["SDC_", "ANTHROPIC_", "CLAUDE", "ESS_", "GH_", "GITHUB_"].iter().any(|p| k.starts_with(p)) {
                build.env_remove(&k);
            }
        }
        let (code, tail) = run_bounded(&mut build, limit, Some(&out.join("build.stdout")), &out.join("build.log"));
        o.rec["build"] = json!({"argv": argv, "exit": code});
        if code != Some(0) {
            return o.finish("unknown", Some("product_build_failed"), Some(format!("{} exit {code:?}: {tail}", argv.join(" "))));
        }
    }

    let adapter = env("SDC_CONFORM_ADAPTER_BIN").map(PathBuf::from).unwrap_or_else(|| exe_dir().join("sdc-conform-adapter"));
    let report = out.join("report.json");
    let detail = run.join("conformance-detail.json");
    let log = out.join("adapter-log.jsonl");
    let timeout: u64 = env("SDC_CONFORMANCE_TARGET_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(30);
    let mut cmd = ess_cmd(&ess);
    cmd.args(["verify", "conform", "run", "--suite"]).arg(&suite)
        .args(["--target", "process", "--target-dir"]).arg(&out)
        .args(["--target-timeout", &timeout.to_string(), "--report-format", "2", "--strict", "--format", "json", "--report-out"]).arg(&report)
        .arg("--").arg(&adapter).arg("--table").arg(&table_path).arg("--tree").arg(&product)
        .arg("--work").arg(out.join("scenarios")).arg("--log").arg(&log)
        .args(["--start-timeout", "20", "--request-timeout", &timeout.min(20).to_string()]);
    let limit = Duration::from_secs((n as u64 + 1) * (2 * timeout + 25) + 60);
    let (code, tail) = run_bounded(&mut cmd, limit, Some(&detail), &out.join("ess-run.stderr"));
    o.rec["ess_run"] = json!({"exit": code});
    let Ok(rep) = read_json(&report) else {
        return o.finish("unknown", Some("adapter_refused"), Some(format!("ess exit {code:?}, no report: {tail}")));
    };
    let det = read_json(&detail).unwrap_or(J::Null);
    let mut ran: Vec<(J, String)> = det["scenarios"].as_array().into_iter().flatten().map(|sc| (sc["scenario"].clone(), s(&sc["status"]).to_string())).collect();
    if ran.is_empty() {
        for (status, ids) in rep["outcomes"].as_object().into_iter().flatten() {
            for id in ids.as_array().into_iter().flatten() {
                ran.push((id.clone(), status.clone()));
            }
        }
    }
    let nr = not_reachable(&table);
    // A failed check is an observed contradiction: such a scenario fails even when it also needs
    // what the interface cannot reach.
    let contradicts = |id: &J| det["scenarios"].as_array().into_iter().flatten()
        .filter(|sc| sc["scenario"] == *id)
        .any(|sc| sc["checks"].as_array().into_iter().flatten().any(|c| c["status"] == "failed"));
    let scenarios: Vec<J> = ran.iter().map(|(id, st)| {
        let mine = match st.as_str() {
            "passed" => "passed",
            "error" => "error",
            _ if !contradicts(id) && needs_unreachable(&suite_doc["scenarios"][s(id)], &nr) => "not_measurable",
            _ => "failed",
        };
        json!({"id": id, "status": mine, "ess_status": st})
    }).collect();
    let mut failed_checks = vec![];
    for sc in det["scenarios"].as_array().into_iter().flatten() {
        let mine = scenarios.iter().find(|x| x["id"] == sc["scenario"]).map(|x| s(&x["status"])).unwrap_or("");
        if mine == "failed" || mine == "error" {
            for c in sc["checks"].as_array().into_iter().flatten().filter(|c| c["status"] != "passed") {
                failed_checks.push(json!({"scenario": sc["scenario"], "status": c["status"], "about": c["about"], "code": c["code"],
                    "expected": c["diagnostic"]["expected"], "observed": c["diagnostic"]["observed"]}));
            }
        }
    }
    let count = |st: &str| scenarios.iter().filter(|x| x["status"] == st).count();
    let (k, f, u, errors) = (count("passed"), count("failed"), count("not_measurable"), count("error"));
    let total = rep["counts"]["total"].as_u64().unwrap_or(0) as usize;
    let status = s(&rep["conformance_status"]).to_string();
    o.rec["k"] = json!(k);
    o.rec["f"] = json!(f);
    o.rec["u"] = json!(u);
    o.rec["n"] = json!(n);
    o.rec["conformance_status"] = json!(status);
    o.rec["counts"] = rep["counts"].clone();
    o.rec["scenarios"] = J::Array(scenarios);
    o.rec["failed_checks"] = J::Array(failed_checks);
    let starts: Vec<J> = fs::read_to_string(&log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<J>(l).ok()).collect();
    let not_started: Vec<&J> = starts.iter().filter(|e| e["event"] == "start" && e["ok"] == false).collect();
    let no_capture: Vec<&J> = starts.iter().filter(|e| e["event"] == "capture" && e["ok"] == false).collect();
    o.rec["adapter_notes"] = json!(starts.iter().filter(|e| e["event"] != "start").cloned().collect::<Vec<_>>());
    if let Some(first) = not_started.first() {
        return o.finish("unknown", Some("product_not_started"), Some(format!("{} of {n} scenarios: {}", not_started.len(), s(&first["why"]))));
    }
    if let Some(first) = no_capture.first() {
        return o.finish("unknown", Some("capture_missing"), Some(format!("{} of {n} scenarios: {}", no_capture.len(), s(&first["why"]))));
    }
    if total != n {
        return o.finish("unknown", Some("suite_mismatch"), Some(format!("ess ran {total} scenarios, the suite holds {n}")));
    }
    if errors > 0 {
        return o.finish("unknown", Some("scenario_error"), Some(format!("ess says {status}: {errors} scenario(s) ended error")));
    }
    if status == "passed" && k == n && n > 0 {
        return o.finish("held", None, None);
    }
    if f > 0 {
        return o.finish("contradicted", None, None);
    }
    if u > 0 {
        return o.finish("unknown", Some("not_measurable"), Some(format!("{u} of {n} scenarios need what the product's interface cannot reach")));
    }
    o.finish("unknown", Some("scenario_error"), Some(format!("ess says {status} with {k} of {n} passed")))
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
            match observe(Path::new(&args[1]), arg(&args, "--table").map(PathBuf::from), &pin) {
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
            let (Some(ir), Some(t)) = (arg(&args, "--ir"), arg(&args, "--table")) else {
                eprintln!("{USAGE}");
                return ExitCode::from(64);
            };
            match (read_json(Path::new(&ir)), read_json(Path::new(&t))) {
                (Ok(ir), Ok(t)) => {
                    let p = check_table(&t, &ir);
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
