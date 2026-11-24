//! sdc-node: the model step of a case under Loom, for the plain nodes (`idea.dissect`,
//! `questions.write`, `requirements.normalise`, `spec.author`, `test.reproduce`, `bug.dissect`,
//! `change.fix`) and the nodes of the planning store (`vision.write`, `roadmap.write`,
//! `plan.decompose`, `context.default`, `questions.answer`), of the review (`change.review`) and
//! the implementer (`change.implement`, in session, coverage and request mode). It replaces
//! the shell script that was the model step of the removed driver and of Loom.
//!
//! ```text
//! sdc-node <node> [--tools-dir <dir>] [--model m] [--max-turns n] [--max-budget-usd u]
//!          [--operations a,b] [--instructions <json object>] [--adapter a] [--principal p]
//!          [--stub-fixtures d] [--stub-counter d] [--claude-credentials f] [--effort level] [--replay-plan p]
//!          [--stub <argv...>]
//! ```
//!
//! `--max-budget-usd` is optional: with none the request of the adapter names no money budget.
//!
//! The working directory is the case's workspace; standard input is one JSON object (empty is
//! `{}`); `LOOM_CASE` names the case, `LOOM_RETRY_FEEDBACK` a `loom.retry-feedback/1` document on a
//! retry. The program builds the prompt from the node's declared inputs, copies those into the
//! call's `in/`, lets the adapter write `out/`, and copies back only the declared output. It holds
//! no text a model reads: every sentence is a file under `instructions/`. Exit codes are the
//! script's: 0, 64 (bad arguments), 75 (the model's API ended), 79 (not called: the program prints
//! why and writes no evidence; a derived tree that is only renewed ends 0, see `derived_skip`), 126
//! (program error), 127 (missing program); an adapter's exit is passed through.

#[path = "sdc-node/call.rs"]
mod call;
#[path = "sdc-node/findings.rs"]
mod findings;
#[path = "sdc-node/implement.rs"]
mod implement;
#[path = "sdc-node/nodes.rs"]
mod nodes;
#[path = "sdc-node/ordjson.rs"]
mod ordjson;
#[path = "sdc-node/review.rs"]
mod review;
#[path = "sdc-node/rework.rs"]
mod rework;
#[path = "sdc-node/setup.rs"]
mod setup;
#[path = "sdc-node/store.rs"]
mod store;
#[path = "sdc-node/util.rs"]
mod util;
#[path = "sdc-node/yaml.rs"]
mod yaml;

use ordjson::Ov;
use setup::Setup;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use util::{die, env_set, run_cmd, Exit, Out, R};

/// The state of one run.
pub struct Ctx {
    /// The action name.
    pub node: String,
    /// What the start produced.
    pub set: Setup,
    /// The workspace: the working directory.
    pub work: PathBuf,
    /// The nearest directory above the tools that holds `instructions/`.
    pub repo_root: PathBuf,
    /// The adapter's name, `stub` for none.
    pub adapter: String,
    /// The program of the adapter.
    pub adapter_bin: PathBuf,
    /// The node's class (`tools/node-classes.json`).
    pub class: String,
    /// The reasoning effort: `--effort`, else the class default.
    pub effort: Option<String>,
    /// The rejection document of an approval of this writer's output, when the feedback has one.
    pub rejection: Option<PathBuf>,
    /// `change.implement` makes one request (`SDC_IMPLEMENT_MODE=request`) instead of a session.
    pub request_mode: bool,
    /// `change.implement` replies with a coverage document (`SDC_IMPLEMENT_COVERAGE=on`, session).
    pub coverage: bool,
}

impl Ctx {
    fn new(node: &str, set: Setup) -> Ctx {
        let work = std::env::current_dir().unwrap_or_default();
        let repo_root = set.here.ancestors().find(|d| d.join("instructions").is_dir()).map_or_else(|| PathBuf::from("/"), Path::to_path_buf);
        // The library that assembles the implement request finds its instruction files from here,
        // not from beside the binary.
        std::env::set_var("SDC_INSTRUCTIONS_ROOT", &repo_root);
        Ctx {
            node: node.to_string(),
            set,
            work,
            repo_root,
            adapter: String::new(),
            adapter_bin: PathBuf::new(),
            class: String::new(),
            effort: None,
            rejection: None,
            request_mode: false,
            coverage: false,
        }
    }

    /// The fixtures of the examples.
    pub fn fx(&self) -> PathBuf {
        self.set.root.join("driver/examples/sdc/fixtures")
    }

    /// The instruction file `rel`, filled. A file that is missing, or names a placeholder without a
    /// value, is a program error.
    pub fn render(&self, rel: &str, vals: &[(&str, &str)]) -> R<String> {
        sdc_tools::instructions::render_in(&self.repo_root, rel, vals)
            .map_err(|e| die(format!("llm-node: {} could not run: {e}", self.node), 126))
    }

    /// The file the binding's `llm.instructions` names for `part`, filled.
    pub fn render_part(&self, part: &str, vals: &[(&str, &str)]) -> R<String> {
        let file = self.set.cfg.instructions.as_ref().and_then(|i| i.field(part)).map(Ov::raw).unwrap_or_default();
        if file.is_empty() {
            return Err(die(format!("llm-node: {} could not run: the binding's llm object names no instructions.{part}", self.node), 126));
        }
        self.render(&file, vals)
    }

    /// The epic and the story of the case: cuts of the case id, or the names Loom gave the cases.
    pub fn names(&self) -> (String, String) {
        let cut = |n: usize| -> String {
            let case = self.set.case.lines().next().unwrap_or("");
            if case.contains('.') { case.split('.').nth(n - 1).unwrap_or("").to_string() } else { case.to_string() }
        };
        let (mut epic, mut story) = (cut(2), cut(3));
        if let Some(parent) = env_set("LOOM_PARENT_CASE_NAME") {
            epic = parent;
            story = env_set("LOOM_CASE_NAME").unwrap_or(story);
        } else if let Some(name) = env_set("LOOM_CASE_NAME") {
            epic = name;
            story = String::new();
        }
        (epic, story)
    }

    /// The review reads one document of facts (`SDC_REVIEW_MODE=facts`).
    pub fn facts(&self) -> bool {
        self.node == "change.review" && env_set("SDC_REVIEW_MODE").as_deref() == Some("facts")
    }

    /// The bug report names a ticket.
    pub fn has_ticket(&self) -> bool {
        yaml::load(&self.work.join("bug.yaml")).is_some_and(|d| yaml::truthy(&d, "ticket"))
    }

    /// Runs the stub of the binding: `sdc-tool.sh` with the stub's argv.
    pub fn run_stub(&self, out: &mut Out) -> i32 {
        let args = self.set.cfg.stub.clone().unwrap_or_default();
        run_cmd(Command::new(self.set.here.join("sdc-tool.sh")).args(args), out)
    }

    /// The adapter of a schema-reply class (the library's `adapters`); a refusal is a program error.
    fn schema_adapter(&self, class: &str) -> R<String> {
        let (found, refused) = sdc_tools::reply::adapters::adapters(&self.set.here, &|k| std::env::var(k).ok());
        match found[class]["adapter"].as_str().filter(|a| !a.is_empty()) {
            Some(a) if refused.is_empty() => Ok(a.to_string()),
            _ => {
                let why: Vec<String> = refused.iter().map(|r| format!("refused: {r}")).collect();
                Err(die(format!("llm-node: {} could not run: {}", self.node, why.join("\n")), 126))
            }
        }
    }

    /// Everything before the call's record directory. `Some(code)` ends the run there.
    fn before_call(&mut self) -> R<Option<i32>> {
        // aep records who moved an artifact: the acting principal, the run's principal when it is a
        // <kind>:<name> principal and no person (no person takes a tool step), else agent:<node>.
        let principal = env_set("SDC_PRINCIPAL").unwrap_or_default();
        let named = regex::Regex::new(r"\A[a-z][a-z-]*:[^ \t\n\r\x0b\x0c]+\z").is_ok_and(|r| r.is_match(&principal));
        let actor = if named && !principal.starts_with("human:") && !principal.starts_with("person:") { principal } else { format!("agent:{}", self.node) };
        std::env::set_var("AEP_ACTOR", actor);

        // questions.write never rewrites an interview that exists unless a check found it invalid.
        let interview = fs::metadata(self.work.join("questions.yaml")).is_ok_and(|m| m.len() > 0);
        if self.node == "questions.write" && interview && self.set.feedback.is_none() {
            let msg = self.render("instructions/shared/node.not-called.questions.md", &[])?;
            Out::inherit().line(msg.as_bytes());
            return Ok(Some(util::NOT_CALLED));
        }
        self.adapter = env_set("SDC_LLM_ADAPTER").unwrap_or_else(|| "stub".to_string());
        // The review's reply is a data document under a response schema, so its class has its own
        // adapter (the library's `adapters`).
        if self.node == "change.review" {
            self.adapter = self.schema_adapter("review")?;
        }
        let mut modes = Vec::new();
        for (var, default, allowed) in [
            ("SDC_IMPLEMENT_MODE", "session", ["session", "request"]),
            ("SDC_REVIEW_MODE", "session", ["session", "facts"]),
            ("SDC_IMPLEMENT_COVERAGE", "off", ["on", "off"]),
        ] {
            let v = env_set(var).unwrap_or_else(|| default.to_string());
            if !allowed.contains(&v.as_str()) {
                return Err(die(format!("llm-node: {} could not run: {var}={v} is neither {} nor {}", self.node, allowed[0], allowed[1]), 126));
            }
            modes.push(v);
        }
        // change.implement as one request: its reply is the files under a response schema, so its
        // class has its own adapter too. The coverage document belongs to the session implementer.
        self.request_mode = self.node == "change.implement" && modes[0] == "request";
        self.coverage = self.node == "change.implement" && !self.request_mode && modes[2] == "on";
        if self.request_mode {
            self.adapter = self.schema_adapter("implement")?;
        }
        if let Some(nodes) = env_set("SDC_LLM_NODES") {
            if !format!(" {nodes} ").contains(&format!(" {} ", self.node)) {
                self.adapter = "stub".to_string();
            }
        }
        if let Some(code) = self.wait_for_person()? {
            return Ok(Some(code));
        }
        if let Some(copy) = self.set.copy.take() {
            setup::write_copy(&self.work, copy)?;
        }
        self.rejection = self.find_rejection();

        let classes = fs::read_to_string(self.set.here.join("node-classes.json")).ok().and_then(|t| Ov::parse(&t).ok());
        let class = classes
            .as_ref()
            .and_then(|c| c.get("classes"))
            .and_then(|c| match c {
                Ov::Obj(m) => m.iter().find(|(_, v)| matches!(v, Ov::Arr(a) if a.contains(&Ov::Str(self.node.clone())))).map(|(k, _)| k.clone()),
                _ => None,
            })
            .unwrap_or_else(|| if classes.is_some() { "other".to_string() } else { String::new() });
        self.effort = self.set.effort.clone().or_else(|| classes.as_ref().and_then(|c| c.get("default_efforts")).and_then(|d| d.field(&class)).map(Ov::raw));
        self.class = class;

        if self.adapter == "stub" {
            let mut code = self.run_stub(&mut Out::inherit());
            if code == 0 && !self.coverage_stub() {
                code = 126;
            }
            if code == 0 {
                self.post("", &mut Out::inherit())?;
            }
            self.stub_rework(code)?;
            return Ok(Some(code));
        }
        if let Some(code) = self.derived_skip()? {
            return Ok(Some(code));
        }
        self.adapter_bin = match self.adapter.as_str() {
            "metaharness" | "claude-cli" | "dry" => self.set.here.join(format!("llm-adapter-{}.sh", self.adapter)),
            other => PathBuf::from(other),
        };
        Ok(None)
    }
}

fn run(argv: &[String]) -> R<i32> {
    let Some(node) = argv.get(1) else {
        return Err(die("usage: sdc-node <node> [--tools-dir <dir>] [options] [--stub <argv>...]", 64));
    };
    let set = setup::setup(node, &argv[2..])?;
    let mut ctx = Ctx::new(node, set);
    if let Some(code) = ctx.before_call()? {
        return Ok(code);
    }
    call::run(&ctx)
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let code = match run(&argv) {
        Ok(code) | Err(Exit(code)) => code,
    };
    std::process::exit(code);
}
