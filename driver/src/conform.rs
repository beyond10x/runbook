//! sdc-conform: the conformance observation of one ended run (docs/conformance.md).
//!
//! ```text
//! sdc-conform observe <run dir> [--tables <n>] [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
//! sdc-conform check-table --ir <ir.json> --table <table.json> --tree <delivered tree>
//! sdc-conform ess-path [--pin <ess-pin.json>]
//! sdc-conform ess-install [--archive <ess-….tar.gz>] [--pin <ess-pin.json>]
//! sdc-conform aep-path [--pin <aep-pin.json>]
//! sdc-conform aep-install [--archive <aep-….tar.gz>] [--pin <aep-pin.json>]
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
//! evidence. It writes `<run>/oracle.json` (`sdc.oracle/1`, one check `conformance`: spec
//! scenarios proven passed k of n (a lower bound), failed f (p of them refused for a precondition the
//! specification does not model), disputed d, not measurable u over the counted tables, and how many
//! scenarios ended the same under all of them; `held` only when k = n > 0), `<run>/conformance-detail.json` (every
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

use crate::conformance::{self, check_table, declared_missing, needs_unreachable, not_reachable, read_json, resolve_ess, s, sha256_bytes, sha256_file, table_schema, Ess};
use crate::{instructions, run_metrics};

/// Where an observation finds what it needs. [`Setup::default`] is what the `sdc-conform` program
/// does: everything is found from the program's own place in a checkout. `runbook run` names its
/// own (the run's workspace, the bindings' tools, the asset root, the adapter program) because its
/// binary does not sit under `driver/target/`.
#[derive(Clone, Debug, Default)]
pub struct Setup {
    /// The workspace of the root case relative to the run directory (`spec/` and `published/`
    /// inside); none: `store/IDEA-1/work` of the run, else the first `store/*/work`.
    pub work: Option<PathBuf>,
    /// The ess to run: none is the pinned binary of `driver/ess-pin.json` ([`resolve_ess`]).
    pub ess: Option<EssTool>,
    /// The tools directory (`bindings/tools`: the LLM adapters and `node-classes.json`); none:
    /// `SDC_TOOLS_DIR`, else beside the program's checkout.
    pub tools: Option<PathBuf>,
    /// The repository root that holds `instructions/`; none: found above the running program.
    pub instructions: Option<PathBuf>,
    /// The program that answers the product's requests for ess (`sdc-conform-adapter`); none:
    /// `SDC_CONFORM_ADAPTER_BIN`, else beside the running program.
    pub adapter: Option<PathBuf>,
    /// The gateway file of the run (`runbook run` with a `[gateway]`): handed to the model adapter of the
    /// table node as `SDC_GATEWAY_FILE`, the path only. None: the adapter gets the environment it always got.
    pub gateway_file: Option<PathBuf>,
    /// The most the node sessions of the whole observation may spend, USD; none: no bound but each
    /// session's own (`SDC_CONFORMANCE_MAX_BUDGET_USD`, none when unset; 2 with a bound). With a bound no session starts
    /// once what the sessions before it cost has reached it (a session that left no cost record
    /// counts at its own limit), and no session's own limit is larger than what is left of it.
    pub budget_usd: Option<f64>,
}

/// An ess that is run through a tool program: `<tool> ess <arguments>` (`bindings/tools/sdc-tool.sh`,
/// which pins the release), in `cwd`.
#[derive(Clone, Debug)]
pub struct EssTool {
    /// The program, `sdc-tool.sh`.
    pub tool: PathBuf,
    /// The directory the tool starts in (it writes its `.sdc/` there).
    pub cwd: PathBuf,
}

impl Setup {
    fn tools(&self) -> Option<PathBuf> {
        self.tools.clone().or_else(tools_dir)
    }

    fn instructions_text(&self, rel: &str, values: &[(&str, &str)]) -> Result<String, String> {
        match &self.instructions {
            Some(root) => instructions::render_in(root, rel, values),
            None => instructions::render(rel, values),
        }
    }

    fn adapter(&self) -> PathBuf {
        env("SDC_CONFORM_ADAPTER_BIN").map(PathBuf::from).or_else(|| self.adapter.clone()).unwrap_or_else(|| exe_dir().join("sdc-conform-adapter"))
    }

    /// The root case's workspace of `run`.
    fn work_of(&self, run: &Path) -> PathBuf {
        if let Some(w) = &self.work {
            return run.join(w);
        }
        let idea = run.join("store/IDEA-1");
        let root = if idea.is_dir() {
            idea
        } else {
            let mut found: Vec<PathBuf> = fs::read_dir(run.join("store")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("work").is_dir()).collect();
            found.sort();
            found.into_iter().next().unwrap_or_else(|| run.join("store/IDEA-1"))
        };
        root.join("work")
    }
}

/// The ess of an observation: the one `setup` names, else the pinned binary.
fn ess_of(setup: &Setup, pin: &Path) -> Result<Ess, (&'static str, String)> {
    let Some(t) = &setup.ess else { return resolve_ess(pin) };
    if !t.tool.is_file() {
        return Err(("ess_missing", format!("{} (the tool that runs the pinned ess) does not exist", t.tool.display())));
    }
    let mut ess = Ess { path: t.tool.clone(), version: String::new(), sha256: String::new(), source: "sdc-tool.sh", prefix: vec!["ess".into()], cwd: Some(t.cwd.clone()) };
    fs::create_dir_all(&t.cwd).map_err(|e| ("ess_missing", format!("{}: {e}", t.cwd.display())))?;
    let out = ess_cmd(&ess).arg("--version").stdin(Stdio::null()).output().map_err(|e| ("ess_missing", format!("{}: {e}", t.tool.display())))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        return Err(("ess_missing", format!("{} ess --version exited {:?}: {said}", t.tool.display(), out.status.code())));
    }
    ess.version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(ess)
}

const USAGE: &str = "usage: sdc-conform observe <run dir> [--tables <n>] [--table <table.json>] [--pin <ess-pin.json>] [--tools <dir>]
       sdc-conform recompute <run dir> [--pin <ess-pin.json>]
       sdc-conform summary <run dir>
       sdc-conform check-table --ir <ir.json> --table <table.json> --tree <delivered tree>
       sdc-conform ess-path [--pin <ess-pin.json>]
       sdc-conform ess-install [--archive <file>] [--pin <ess-pin.json>]
       sdc-conform aep-path [--pin <aep-pin.json>]
       sdc-conform aep-install [--archive <file>] [--pin <aep-pin.json>]
       sdc-conform prompt [--attempt <n>] [--refused <why>]... [--run-fault <why>]...";
const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
const DEFAULT_TABLES: usize = 3;

/// The number of table slots from `SDC_CONFORMANCE_TABLES`: 3 when it is not set, else a number
/// from 1 to 9.
pub fn tables_from_env() -> Result<usize, String> {
    match env("SDC_CONFORMANCE_TABLES").map(|x| x.parse::<usize>()) {
        None => Ok(DEFAULT_TABLES),
        Some(Ok(t)) if (1..=9).contains(&t) => Ok(t),
        Some(_) => Err("SDC_CONFORMANCE_TABLES is a number from 1 to 9".to_string()),
    }
}
/// Node sessions per table slot: three for the check, one retry after a faulty run, one more.
const MAX_SESSIONS: u32 = 5;
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

fn default_aep_pin() -> PathBuf {
    driver_dir().map(|d| d.join("aep-pin.json")).unwrap_or_else(|| PathBuf::from("driver/aep-pin.json"))
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
    let mut poll = Duration::from_millis(1);
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) if Instant::now() > end => {
                let _ = child.kill();
                let _ = child.wait();
                return (None, format!("killed after {} s", limit.as_secs()));
            }
            Ok(None) => {
                std::thread::sleep(poll);
                poll = (poll * 2).min(Duration::from_millis(5));
            }
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

/// The figure of a conformance record (`run_metrics::conformance_figure`).
fn figure(c: &J) -> String {
    run_metrics::conformance_figure(c)
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
        let check = json!({"id": "conformance", "result": result, "k": r["k"], "f": r["f"], "p": r["p"], "d": r["d"], "u": r["u"], "n": r["n"],
            "agreement": r["agreement"], "tables_counted": r["tables_counted"], "cause": cause});
        let doc = json!({"format": "sdc.oracle/1", "run": name, "checks": [check], "conformance": self.rec});
        let p = self.run.join("oracle.json");
        fs::write(&p, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n").map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(doc)
    }
}

fn effort(setup: &Setup) -> (String, String) {
    if let Some((v, n)) = first_env(&["SDC_LLM_EFFORT_CONFORMANCE", "SDC_LLM_EFFORT_OTHER", "SDC_LLM_EFFORT"]) {
        return (v, n.to_string());
    }
    let classes = setup.tools().map(|t| t.join("node-classes.json")).and_then(|p| read_json(&p).ok());
    match classes.as_ref().and_then(|c| c["default_efforts"]["other"].as_str()) {
        Some(e) => (e.to_string(), "node-classes.json default_efforts.other".into()),
        None => ("high".into(), "default".into()),
    }
}

fn tools_dir() -> Option<PathBuf> {
    env("SDC_TOOLS_DIR").map(PathBuf::from).or_else(|| driver_dir().map(|d| d.join("../bindings/tools")))
}

/// What the node is told about its previous table.
enum Feedback {
    None,
    /// The program check refused the table.
    Check(Vec<String>),
    /// The table was accepted, and its run showed faults of the table itself.
    Run(Vec<String>),
}

/// The table node's instruction text: `instructions/conformance.table.md`, and from the second
/// attempt on the refusal of the previous table or the faults its run showed.
fn prompt(setup: &Setup, attempt: u32, feedback: &Feedback) -> Result<String, String> {
    let mut p = setup.instructions_text("instructions/conformance.table.md", &[])? + "\n";
    let attempt = attempt.to_string();
    let listed = |items: &[String]| items.iter().take(30).map(|why| format!("  - {why}\n")).collect::<String>();
    match feedback {
        Feedback::None => {}
        Feedback::Check(invalid) => {
            p.push('\n');
            p.push_str(&setup.instructions_text("instructions/conformance.table.refused.md", &[("attempt", &attempt), ("reasons", &listed(invalid))])?);
            p.push('\n');
        }
        Feedback::Run(evidence) => {
            p.push('\n');
            p.push_str(&setup.instructions_text("instructions/conformance.table.run-fault.md", &[("attempt", &attempt), ("faults", &listed(evidence))])?);
            p.push('\n');
        }
    }
    Ok(p)
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
    setup: Setup,
    /// The money budget the environment names, `None` when it names none.
    budget: Option<f64>,
    /// What the node sessions of this observation cost so far, USD (a session without a cost
    /// record at its own limit), and how many of them left no cost record.
    meter: Meter,
}

#[derive(Default)]
struct Meter {
    spent: std::cell::Cell<f64>,
    unknown: std::cell::Cell<u32>,
}

/// `SDC_CONFORMANCE_MAX_BUDGET_USD`: unset or empty names no budget; a value that is set must be a
/// finite number of at least 0, or the observation is refused (a named cap never becomes none). A
/// value that is no text is named and cannot be read: refused, no byte of it shown.
fn budget_named() -> Result<Option<f64>, String> {
    const NAME: &str = "SDC_CONFORMANCE_MAX_BUDGET_USD";
    match std::env::var_os(NAME).map(std::ffi::OsString::into_string) {
        None => Ok(None),
        Some(Err(_)) => Err(format!("{NAME} is set to bytes that are no text: it must be a finite number of at least 0")),
        Some(Ok(raw)) if raw.is_empty() => Ok(None),
        Some(Ok(raw)) => crate::named::money(NAME, &raw).map(Some),
    }
}

fn node_setup(setup: &Setup, out: &Path, tree: &Path, spec: &Path, ir: &Path, budget: Option<f64>) -> Result<Node, (&'static str, String)> {
    let adapter_name = first_env(&["SDC_CONFORMANCE_ADAPTER", "SDC_LLM_ADAPTER"]).map(|x| x.0).unwrap_or_else(|| "claude-cli".into());
    let adapter = match adapter_name.as_str() {
        "stub" | "dry" | "fixture" => return Err(("table_missing", format!("the LLM adapter {adapter_name} has no model, so no table was written"))),
        "metaharness" | "claude-cli" => match setup.tools() {
            Some(t) => t.join(format!("llm-adapter-{adapter_name}.sh")),
            None => return Err(("table_missing", "the tools directory (bindings/tools) was not found".into())),
        },
        p => PathBuf::from(p),
    };
    let (model, model_source) = env("SDC_LLM_MODEL_CONFORMANCE").map(|m| (m, "SDC_LLM_MODEL_CONFORMANCE")).unwrap_or((DEFAULT_MODEL.into(), "default"));
    let (effort, effort_source) = effort(setup);
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
    Ok(Node { adapter, rec, model, effort, cwd, schema, tree: tree.to_path_buf(), setup: setup.clone(), budget, meter: Meter::default() })
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
        // The money budget of a session: what the environment names; under a bound of the whole observation
        // no more than what is left of it (2 USD when the environment names none); with no bound and none
        // named, none.
        let session_usd = match node.setup.budget_usd {
            Some(budget) => {
                let left = budget - node.meter.spent.get();
                if left < 1e-6 {
                    return Err(("table_missing", "no session was started: what the observation may spend is used".to_string(), invalid));
                }
                Some(node.budget.unwrap_or(2.0).min(left))
            }
            None => node.budget,
        };
        *attempts += 1;
        let attempt = *attempts;
        let rec = dir.join("node").join(format!("{attempt:03}"));
        fs::create_dir_all(&rec).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        fs::write(rec.join("prompt.md"), prompt(&node.setup, attempt, &feedback).map_err(|e| ("table_missing", e, vec![]))?).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        let mut req = json!({"model": node.model, "effort": node.effort, "prompt_file": rec.join("prompt.md"), "cwd": node.cwd, "record_dir": rec,
            "node": "conformance.table", "slot": slot, "attempt": attempt, "json_schema": node.schema,
            "operations": ["dir.list", "file.read", "search"],
            "max_turns": env("SDC_CONFORMANCE_MAX_TURNS").and_then(|x| x.parse::<u64>().ok()).unwrap_or(40)});
        // A money budget is named by the environment or by a bound of the observation: with neither the request names none.
        if let Some(budget) = session_usd {
            req["max_budget_usd"] = json!(budget);
        }
        fs::write(rec.join("request.json"), serde_json::to_string_pretty(&req).unwrap_or_default()).map_err(|e| ("table_missing", e.to_string(), vec![]))?;
        let limit = Duration::from_secs(env("SDC_CONFORMANCE_NODE_TIMEOUT").and_then(|x| x.parse().ok()).unwrap_or(1800));
        let mut adapter = Command::new(&node.adapter);
        adapter.arg(rec.join("request.json"));
        if let Some(file) = &node.setup.gateway_file {
            adapter.env(crate::gateway::FILE_VARIABLE, file);
        }
        let (code, tail) = run_bounded(&mut adapter, limit, Some(&rec.join("response.json")), &rec.join("adapter-stderr.txt"));
        let resp = read_json(&rec.join("response.json")).unwrap_or(J::Null);
        match resp["cost_usd"].as_f64() {
            Some(c) => {
                *cost += c;
                node.meter.spent.set(node.meter.spent.get() + c.max(0.0));
            }
            None => {
                node.meter.unknown.set(node.meter.unknown.get() + 1);
                node.meter.spent.set(node.meter.spent.get() + session_usd.unwrap_or(0.0));
            }
        }
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
    c.args(&ess.prefix);
    if let Some(d) = &ess.cwd {
        c.current_dir(d);
    }
    c.env_remove("ESS_BIN");
    c
}

/// What every table run of one observation shares.
struct Env<'a> {
    ess: &'a Ess,
    adapter: PathBuf,
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
    /// When the run's only fault is the product refusing 401/403: the refused scenarios, if none of
    /// them had a request answered with success before its refusal and the product answered some
    /// other request with success (`precondition_refusals`).
    precondition: Option<std::collections::BTreeSet<String>>,
}

/// The scenarios the product refused 401/403 although nothing the scenario sent before had been
/// answered with success, when the product answered some other request with success: a
/// precondition (a sign-in) that no request of the scenario can meet. None when a refused scenario
/// had a request answered 2xx before its refusal (a session the table did not carry) or the product
/// accepted no request at all (a key the table did not send).
fn precondition_refusals(events: &[J]) -> Option<std::collections::BTreeSet<String>> {
    use std::collections::BTreeSet;
    let (mut refused, mut ok_before, mut any_ok) = (BTreeSet::new(), BTreeSet::new(), false);
    for e in events {
        let sc = s(&e["scenario"]).to_string();
        let status = e["status"].as_u64();
        if e["event"] == "request" && matches!(status, Some(200..=299)) {
            any_ok = true;
            if !refused.contains(&sc) {
                ok_before.insert(sc);
            }
        } else if (e["event"] == "unmapped_status" || e["event"] == "view_unreadable") && matches!(status, Some(401 | 403)) {
            if ok_before.contains(&sc) {
                return None;
            }
            refused.insert(sc);
        }
    }
    (any_ok && !refused.is_empty()).then_some(refused)
}

/// What the adapter log shows as the table's own fault: a capture that never appeared, a request
/// naming a value the scenario does not carry, the product refusing requests as unauthenticated.
/// The third value: the product's 401/403 refusals are the only fault shown.
fn table_evidence(events: &[J], n: usize) -> (Vec<String>, &'static str, bool) {
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
    let auth_only = !auth.is_empty() && caps.is_empty() && unfillable.is_empty();
    (out, kind, auth_only)
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

fn fail(mut t: TableRun, cause: &'static str, why: String) -> TableRun {
    t.rec["cause"] = json!(cause);
    t.rec["why"] = json!(why);
    t.cause = Some((cause, why));
    t
}

fn table_run(number: usize) -> TableRun {
    TableRun { rec: json!({"run": number}), statuses: Default::default(), cause: None, evidence: vec![], kind: "table_error",
        failed_checks: vec![], notes: vec![], detail: J::Null, unverified: vec![], precondition: None }
}

fn run_table(cx: &Env, table: &J, dir: &Path, number: usize, products: &mut Vec<(String, Built)>) -> TableRun {
    let mut t = table_run(number);
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

    let adapter = cx.adapter.clone();
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
        .arg("--ir").arg(cx.out.join("ir.json")).arg("--suite").arg(&cx.suite)
        .args(["--start-timeout", "20", "--request-timeout", &timeout.min(20).to_string()]);
    let limit = Duration::from_secs((n as u64 + 1) * (2 * timeout + 25) + 60);
    let (code, tail) = run_bounded(&mut cmd, limit, Some(&detail), &dir.join("ess-run.stderr"));
    t.rec["ess_exit"] = json!(code);
    if !report.is_file() {
        return fail(t, "adapter_refused", format!("ess exit {code:?}, no report: {tail}"));
    }
    evaluate(cx, table, dir, t)
}

/// The statuses, checks and faults of one table run from what it left in `dir`: `report.json`,
/// `detail.json` and `adapter-log.jsonl` (also how `recompute` reads a stored run).
fn evaluate(cx: &Env, table: &J, dir: &Path, mut t: TableRun) -> TableRun {
    let (report, detail, log) = (dir.join("report.json"), dir.join("detail.json"), dir.join("adapter-log.jsonl"));
    let n = cx.n;
    let Ok(rep) = read_json(&report) else {
        return fail(t, "adapter_refused", format!("{} cannot be read", report.display()));
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
    let (evidence, kind, auth_only) = table_evidence(&events, n);
    t.rec["evidence"] = json!(evidence);
    t.evidence = evidence;
    t.kind = kind;
    if auth_only {
        t.precondition = precondition_refusals(&events);
    }
    t
}

/// The observation of one ended run: writes `<run>/oracle.json` and returns it. `Err` only when
/// the file cannot be written; every other failure is the file's `unknown` with a named cause.
pub fn observe(run: &Path, table_file: Option<PathBuf>, tables: usize, pin: &Path, setup: &Setup) -> Result<J, String> {
    let run = fs::canonicalize(run).map_err(|e| format!("{}: {e}", run.display()))?;
    let out = run.join("conformance");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let _ = fs::remove_file(run.join("conformance-detail.json"));
    let mut o = Obs { run: run.clone(), rec: json!({"events_checked_from": "response_body",
        "k": null, "f": null, "u": null, "n": null, "agreement": null, "tables_counted": 0}) };

    let work = setup.work_of(&run);
    let tree = ["published", "release"].iter().map(|d| work.join(d)).find(|p| p.is_dir());
    let spec = work.join("spec");

    let ess = match ess_of(setup, pin) {
        Ok(e) => e,
        Err((cause, why)) => return o.finish("unknown", Some(cause), Some(why)),
    };
    o.rec["ess"] = json!({"version": ess.version, "sha256": ess.sha256, "source": ess.source});
    if !spec.is_dir() {
        let why = format!("{} has no spec/", o.rel(&work));
        return o.finish("unknown", Some("suite_failed"), Some(why));
    }
    // A budget the environment names is read after the pin check and before `ess compile`: a refusal costs nothing.
    let budget = match table_file {
        Some(_) => None,
        None => match budget_named() {
            Ok(b) => b,
            Err(why) => return o.finish("unknown", Some("table_missing"), Some(why)),
        },
    };
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
        None => match node_setup(setup, &out, &tree, &spec, &ir, budget) {
            Ok(nd) => Some(nd),
            Err((cause, why)) => return o.finish("unknown", Some(cause), Some(why)),
        },
    };
    let slots = if table_file.is_some() { 1 } else { tables };
    o.rec["tables_written"] = json!(slots);
    let cx = Env { ess: &ess, adapter: setup.adapter(), run: run.clone(), out: out.clone(), suite: suite.clone(), suite_doc, n, tree: tree.clone(), ir_doc };
    let mut products: Vec<(String, Built)> = vec![];
    let mut recs: Vec<J> = vec![];
    let mut counted: Vec<(usize, TableRun)> = vec![];
    let mut uncounted: Vec<(usize, TableRun)> = vec![];
    let mut details: Vec<J> = vec![];
    for slot in 1..=slots {
        let sdir = out.join(format!("table-{slot}"));
        let mut rec = match &node {
            Some(nd) => nd.rec.clone(),
            None => json!({"source": "file"}),
        };
        rec["slot"] = json!(slot);
        rec["retried_after_run"] = json!(false);
        rec["one_more_session"] = json!(false);
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
                // A run that shows a fault of the table is asked again once with that evidence; a
                // slot refused after that retry (by the check or by its run) gets one more fresh
                // session; at most MAX_SESSIONS node sessions per slot.
                let mut feedback = (r.cause.is_none() && !r.evidence.is_empty()).then(|| Feedback::Run(r.evidence.clone()));
                if feedback.is_some() {
                    rec["evidence"] = json!(r.evidence);
                }
                let mut round: usize = 1;
                while let (Some(fb), Some(nd)) = (feedback.take(), &node) {
                    if round >= 3 || attempts >= MAX_SESSIONS {
                        break;
                    }
                    rec[if round == 1 { "retried_after_run" } else { "one_more_session" }] = json!(true);
                    round += 1;
                    match obtain(nd, &sdir, slot, &mut attempts, &mut cost, 1, fb, &cx.ir_doc) {
                        Ok(t2) => {
                            ended = None;
                            r = run_table(&cx, &t2, &sdir.join(format!("run-{round}")), round, &mut products);
                            details.push(json!({"table": slot, "run": round, "ess": r.detail.clone()}));
                            runs.push(r.rec.clone());
                            if r.cause.is_none() && !r.evidence.is_empty() {
                                rec["evidence"] = json!(r.evidence);
                                feedback = Some(Feedback::Run(r.evidence.clone()));
                            }
                        }
                        Err((cause, why, refusals)) => {
                            rec["refused"] = json!(refusals);
                            ended = Some((cause, format!("asked again after its run: {why}")));
                            feedback = Some(Feedback::Check(refusals));
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
        match (ended.is_none(), last) {
            (true, Some(r)) => counted.push((slot, r)),
            (false, Some(r)) => uncounted.push((slot, r)),
            _ => {}
        }
        recs.push(rec);
    }
    let total_cost: f64 = recs.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
    o.rec["node_sessions"] = json!(recs.iter().filter_map(|r| r["attempts"].as_u64()).sum::<u64>());
    o.rec["table_cost_usd"] = json!((total_cost * 1e6).round() / 1e6);
    o.rec["cost_unknown_sessions"] = json!(node.as_ref().map_or(0, |n| n.meter.unknown.get()));
    let detail_doc = json!({"format": "sdc.conformance-detail/1", "runs": details});
    fs::write(run.join("conformance-detail.json"), serde_json::to_string_pretty(&detail_doc).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
    conclude(o, &cx, slots, recs, counted, uncounted)
}

/// A slot whose every run the product answered only with 401/403 counts after all when every slot
/// ended so after its retry and refused the same scenarios, none of which had a request answered
/// with success before its refusal: the product refuses for a precondition (a sign-in) the
/// specification does not model, which no table can meet. Those scenarios count failed with cause
/// `precondition_not_in_spec` (docs/conformance.md). The ids, when the rule applies.
fn precondition_not_in_spec(recs: &mut [J], counted: &mut Vec<(usize, TableRun)>, uncounted: Vec<(usize, TableRun)>) -> Option<Vec<String>> {
    let sets: Vec<&std::collections::BTreeSet<String>> = uncounted.iter().filter_map(|(_, r)| r.precondition.as_ref()).collect();
    let same_way = recs.iter().all(|r| r["retried_after_run"] == true && r["cause"] == "table_error" && s(&r["why"]).starts_with("refused after its run"));
    if !counted.is_empty() || sets.is_empty() || sets.len() != recs.len() || !sets.windows(2).all(|w| w[0] == w[1]) || !same_way {
        return None;
    }
    let ids: Vec<String> = sets[0].iter().cloned().collect();
    for (slot, mut r) in uncounted {
        for id in &ids {
            r.statuses.insert(id.clone(), "failed".into());
        }
        let count = |st: &str| r.statuses.values().filter(|x| *x == st).count();
        if let Some(rec) = recs.iter_mut().find(|x| x["slot"] == slot) {
            rec["refused_after_run"] = json!({"cause": rec["cause"], "why": rec["why"]});
            rec["counted"] = json!(true);
            rec["cause"] = J::Null;
            rec["why"] = J::Null;
            rec["precondition_not_in_spec"] = json!(ids);
            for (key, st) in [("k", "passed"), ("f", "failed"), ("u", "not_measurable"), ("e", "error")] {
                rec[key] = json!(count(st));
            }
        }
        counted.push((slot, r));
    }
    counted.sort_by_key(|(slot, _)| *slot);
    Some(ids)
}

/// The figure over the slots: `oracle.json`.
fn conclude(mut o: Obs, cx: &Env, slots: usize, mut recs: Vec<J>, mut counted: Vec<(usize, TableRun)>, uncounted: Vec<(usize, TableRun)>) -> Result<J, String> {
    use std::collections::BTreeMap;
    let n = cx.n;
    let precondition = precondition_not_in_spec(&mut recs, &mut counted, uncounted).unwrap_or_default();
    o.rec["tables"] = J::Array(recs.clone());

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
    let (mut k, mut f, mut d, mut u, mut agree, mut p) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
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
        let mut sc = json!({"id": id, "status": status, "under": under});
        if status == "failed" && precondition.contains(id) {
            sc["cause"] = json!("precondition_not_in_spec");
            p += 1;
        }
        scenarios.push(sc);
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
    o.rec["p"] = json!(p);
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

/// The sanitized summary of a run's observation (`sdc.conformance-summary/1`), what the progress
/// ledger reads from `experiments/<id>/conformance/<run>.json`: counts, the cause, scenario causes
/// counted, table agreement, per slot its counts, end and node cost. No request, no response, no
/// `why`, no scenario id: nothing the product or a model wrote.
fn summary(run: &Path) -> Result<J, String> {
    let doc = read_json(&run.join("oracle.json"))?;
    let c = &doc["conformance"];
    if !c["result"].is_string() {
        return Err("oracle.json holds no conformance record".into());
    }
    let mut causes: std::collections::BTreeMap<String, u64> = Default::default();
    for sc in c["scenarios"].as_array().into_iter().flatten() {
        if let Some(cause) = sc["cause"].as_str() {
            *causes.entry(format!("{}:{cause}", s(&sc["status"]))).or_default() += 1;
        }
    }
    let tables: Vec<J> = c["tables"].as_array().into_iter().flatten().map(|t| json!({
        "slot": t["slot"], "counted": t["counted"], "cause": t["cause"], "k": t["k"], "f": t["f"], "u": t["u"], "e": t["e"],
        "attempts": t["attempts"], "retried_after_run": t["retried_after_run"], "one_more_session": t["one_more_session"],
        "precondition_not_in_spec": t["precondition_not_in_spec"].as_array().map(Vec::len), "cost_usd": t["cost_usd"]})).collect();
    let name = run.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    Ok(json!({"format": "sdc.conformance-summary/1", "run": name, "written_by": "sdc-conform summary",
        "result": c["result"], "cause": c["cause"], "measured": c["measured"], "summary": c["summary"],
        "k": c["k"], "f": c["f"], "p": c["p"], "d": c["d"], "u": c["u"], "n": c["n"],
        "agreement": c["agreement"], "tables_counted": c["tables_counted"], "tables_written": c["tables_written"],
        "scenario_causes": causes, "node_sessions": c["node_sessions"], "node_cost_usd": c["table_cost_usd"],
        "ess": c["ess"]["version"], "suite_sha256": c["suite"]["sha256"], "recomputed": c["recomputed"].is_object(), "tables": tables}))
}

/// The figure of an observed run again from the table runs it stored (`conformance/table-<slot>/run-<n>/`),
/// with no node session, no build and no ess run: the slots, sessions and costs of the stored
/// `oracle.json`, each slot's last run evaluated by the current rules. A slot that ended on a
/// refusal of its table, not on a run, keeps that end.
fn recompute(run: &Path, pin: &Path) -> Result<J, String> {
    let run = fs::canonicalize(run).map_err(|e| format!("{}: {e}", run.display()))?;
    let old = read_json(&run.join("oracle.json"))?;
    let prev = &old["conformance"];
    let Some(old_tables) = prev["tables"].as_array().filter(|t| !t.is_empty()) else {
        return Err("oracle.json records no table slot to recompute".into());
    };
    let out = run.join("conformance");
    let ess = resolve_ess(pin).map_err(|(c, w)| format!("{c}: {w}"))?;
    let suite = out.join("suite.json");
    let suite_doc = read_json(&suite)?;
    let n = suite_doc["scenarios"].as_object().map(|m| m.len()).unwrap_or(0);
    let tree = run.join(s(&prev["product"]["tree"]));
    let cx = Env { ess: &ess, adapter: Setup::default().adapter(), run: run.clone(), out: out.clone(), suite, suite_doc, n, tree, ir_doc: read_json(&out.join("ir.json"))? };
    let mut rec = json!({});
    for key in ["events_checked_from", "ess", "n", "suite", "product", "tables_written", "node_sessions", "table_cost_usd"] {
        rec[key] = prev[key].clone();
    }
    rec["recomputed"] = json!({"from": "the stored table runs", "node_sessions": 0, "previous_summary": prev["summary"]});
    let o = Obs { run: run.clone(), rec };
    let (mut recs, mut counted, mut uncounted) = (vec![], vec![], vec![]);
    for old_rec in old_tables {
        let mut rec = old_rec.clone();
        let slot = rec["slot"].as_u64().unwrap_or(0) as usize;
        let sdir = out.join(format!("table-{slot}"));
        let last = fs::read_dir(&sdir).into_iter().flatten().flatten()
            .filter_map(|e| e.file_name().to_string_lossy().strip_prefix("run-").and_then(|x| x.parse::<usize>().ok()))
            .max();
        let ended_on_refusal = s(&rec["why"]).starts_with("asked again after its run");
        let (Some(number), false) = (last, ended_on_refusal) else {
            recs.push(rec);
            continue;
        };
        let dir = sdir.join(format!("run-{number}"));
        let table = read_json(&dir.join("table.json"))?;
        let mut r = evaluate(&cx, &table, &dir, table_run(number));
        let last_old = rec["runs"].as_array().and_then(|x| x.last()).cloned().unwrap_or(J::Null);
        for key in ["sha256", "build", "declared_missing", "ess_exit"] {
            r.rec[key] = last_old[key].clone();
        }
        let ended = match (&r.cause, r.evidence.is_empty()) {
            (Some((c, w)), _) => Some((*c, w.clone())),
            (None, false) => Some((r.kind, format!("refused after its run: {}", r.evidence.join("; ")))),
            (None, true) => None,
        };
        for key in ["k", "f", "u", "e", "n", "unverified", "conformance_status"] {
            rec[key] = r.rec[key].clone();
        }
        if !r.evidence.is_empty() {
            rec["evidence"] = json!(r.evidence);
        }
        rec["counted"] = json!(ended.is_none());
        rec["cause"] = json!(ended.as_ref().map(|x| x.0));
        rec["why"] = json!(ended.as_ref().map(|x| x.1.clone()));
        if ended.is_none() {
            counted.push((slot, r));
        } else {
            uncounted.push((slot, r));
        }
        recs.push(rec);
    }
    let slots = recs.len();
    conclude(o, &cx, slots, recs, counted, uncounted)
}

fn ess_install(archive: Option<PathBuf>, pin_path: &Path) -> Result<String, String> {
    let pin = read_json(pin_path)?;
    install_pinned("ess", &pin, conformance::cache_path(&pin), archive)
}

fn aep_install(archive: Option<PathBuf>, pin_path: &Path) -> Result<String, String> {
    let pin = read_json(pin_path)?;
    if pin["format"] != conformance::AEP_PIN_FORMAT {
        return Err(format!("{}: format is not {}", pin_path.display(), conformance::AEP_PIN_FORMAT));
    }
    install_pinned("aep", &pin, conformance::aep_cache_path(&pin), archive)
}

/// The binary of a pinned release of `tool` unpacked to `dest`: the archive from `archive`, else
/// downloaded from the tool's release on the forge; both digests checked.
fn install_pinned(tool: &str, pin: &J, dest: PathBuf, archive: Option<PathBuf>) -> Result<String, String> {
    let target = conformance::target_triple(pin);
    let p = &pin["platforms"][&target];
    let asset = s(&p["asset"]).to_string();
    if asset.is_empty() {
        return Err(format!("the pin names no archive for {target}"));
    }
    let dir = dest.parent().ok_or("no cache directory")?.to_path_buf();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let archive = match archive {
        Some(a) => a,
        None => {
            let a = dir.join(&asset);
            let url = format!("https://github.com/beyond10x/{tool}/releases/download/{}/{asset}", s(&pin["version"]));
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

/// The `sdc-conform` program: `args` are the process arguments, the exit code is the program's.
pub fn cli() -> ExitCode {
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
            match observe(Path::new(&args[1]), arg(&args, "--table").map(PathBuf::from), tables, &pin, &Setup::default()) {
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
        Some("recompute") if args.len() >= 2 && !args[1].starts_with("--") => match recompute(Path::new(&args[1]), &pin) {
            Ok(doc) => {
                println!("conformance: {}", s(&doc["conformance"]["summary"]));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-conform: {e}");
                ExitCode::from(2)
            }
        },
        Some("summary") if args.len() >= 2 && !args[1].starts_with("--") => match summary(Path::new(&args[1])) {
            Ok(doc) => {
                println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-conform: {e}");
                ExitCode::from(2)
            }
        },
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
        Some("prompt") => {
            let all = |name: &str| -> Vec<String> { args.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect() };
            let (refused, faults) = (all("--refused"), all("--run-fault"));
            let feedback = match (refused.is_empty(), faults.is_empty()) {
                (true, true) => Feedback::None,
                (false, true) => Feedback::Check(refused),
                (true, false) => Feedback::Run(faults),
                (false, false) => {
                    eprintln!("sdc-conform: prompt: --refused and --run-fault do not go together");
                    return ExitCode::from(64);
                }
            };
            let attempt = arg(&args, "--attempt").and_then(|a| a.parse().ok()).unwrap_or(1);
            match prompt(&Setup::default(), attempt, &feedback) {
                Ok(p) => {
                    print!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
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
        Some(c @ ("aep-path" | "aep-install")) => {
            let pin = arg(&args, "--pin").map(PathBuf::from).unwrap_or_else(default_aep_pin);
            let done = if c == "aep-path" {
                conformance::resolve_aep(&pin).map(|p| p.display().to_string())
            } else {
                aep_install(arg(&args, "--archive").map(PathBuf::from), &pin)
            };
            match done {
                Ok(p) => {
                    println!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-conform: {e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounded(name: &str, script: &str, limit: Duration) -> (Option<i32>, String, Duration) {
        let dir = std::env::temp_dir().join(format!("sdc-conform-unit-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let started = Instant::now();
        let (code, tail) = run_bounded(Command::new("sh").arg("-c").arg(script), limit, None, &dir.join("stderr"));
        let took = started.elapsed();
        let _ = fs::remove_dir_all(&dir);
        (code, tail, took)
    }

    /// The digest of a file is the digest of its bytes, whatever its length relative to the read
    /// buffer, and a file that cannot be read is an error that names it.
    #[test]
    fn the_digest_of_a_file_is_the_digest_of_its_bytes() {
        let dir = std::env::temp_dir().join(format!("sdc-conform-unit-{}-digest", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for len in [0usize, 1, 65_535, 65_536, 65_537, 3 * (1 << 20) + 17] {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let file = dir.join(format!("f{len}"));
            fs::write(&file, &bytes).unwrap();
            assert_eq!(sha256_file(&file).unwrap(), sha256_bytes(&bytes), "{len} bytes");
        }
        let missing = dir.join("missing");
        let e = sha256_file(&missing).unwrap_err();
        assert!(e.contains(&missing.display().to_string()), "{e}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A child that runs past its bound is killed at the bound and reported so.
    #[test]
    fn a_child_over_its_bound_is_ended_at_the_bound() {
        let (code, tail, took) = bounded("over", "exec sleep 30", Duration::from_secs(1));
        assert_eq!(code, None);
        assert_eq!(tail, "killed after 1 s");
        assert!(took >= Duration::from_secs(1) && took < Duration::from_secs(10), "{took:?}");
    }

    /// A child that ends is answered when it ended, with its exit code and the tail of its stderr,
    /// not at the next tick of a poll; forty such children one after another take far less than
    /// the 4 s that a poll of 100 ms would add.
    #[test]
    fn a_child_that_ends_is_answered_when_it_ended() {
        let started = Instant::now();
        for i in 0..40 {
            let (code, tail, _) = bounded("quick", "echo said >&2; exit 3", Duration::from_secs(30));
            assert_eq!((code, tail.as_str()), (Some(3), "said"), "child {i}");
        }
        let took = started.elapsed();
        assert!(took < Duration::from_secs(3), "40 children that end at once took {took:?}");
    }
}
