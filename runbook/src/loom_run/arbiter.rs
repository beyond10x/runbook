//! The decider of `runbook run` as the arbiter of the daemon's runs (`runbook::engine::arbiter`).
//!
//! One decider, one routing and one answer shape for both: the verdict comes from
//! `NativeDecider::decide` (`loom_run::decider`: the autonomy level, the holder table, the rule or the
//! model call), the run's copy of the bindings from `bindings_copy::materialise`, and the result a
//! person action is answered with from `runbook::engine::shape`, the functions `run_loop` calls too.
//! What differs is where the answer goes: `runbook run` hands it to its host, the daemon's engine
//! applies it as it applies a client's answer.
//!
//! A decision the decider does not take is `Judgement::Open` and stays for a client: one the level
//! leaves to a person, a table of who decides that cannot be read, a decider that cannot decide or
//! fails, a verdict Loom would refuse, a decision the decider answered before and Loom prints again.

use crate::loom_run::bindings_copy;
use crate::loom_run::decide::{Decider, DecisionRequest, Verdict};
use crate::loom_run::decider::NativeDecider;
use crate::loom_run::private::PrivateDir;
use runbook::engine::arbiter::{Arbiter, Judgement, Opened, Opening, RunArbiter};
use runbook::engine::shape::{self, Unshaped};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// The directory of the run's copy of the bindings, inside the run's directory.
const COPY: &str = "bindings";
/// The directories the PATH of a decider program names after the run's own tools.
const SYSTEM_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// The kinds a daemon can be configured with.
const KINDS: [&str; 2] = ["rule", "model"];

/// What the file of the daemon gives every run besides the tables of the kinds: the same members
/// `runbook run` reads (`login_file`, `cap_micro_usd`, `max_llm_calls`), none of them required.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// The path of the login the model's adapter uses, as `runbook run` is given it.
    pub login_file: Option<String>,
    /// What the decider of one run may spend in all; none: no limit.
    pub cap_micro_usd: Option<u64>,
    /// How many model calls the decider of one run may make; none: no limit.
    pub max_llm_calls: Option<u64>,
}

/// The deciders the operator configured, by kind name.
#[derive(Debug)]
pub struct DaemonArbiter {
    kinds: BTreeMap<String, Value>,
    limits: Limits,
}

impl DaemonArbiter {
    /// The kinds `configured` names, each with its table (the members of a `decider` table without `kind`,
    /// or with the kind that is its name). A table the decider's reader refuses is refused here by name, so
    /// the daemon does not start with it.
    pub fn new(configured: Map<String, Value>, limits: Limits) -> Result<DaemonArbiter, String> {
        let mut kinds = BTreeMap::new();
        for (name, table) in configured {
            if !KINDS.contains(&name.as_str()) {
                return Err(format!("decider kind {name}: not a kind a daemon takes ({})", KINDS.join(", ")));
            }
            let mut table = table.as_object().cloned().ok_or_else(|| format!("deciders.{name}: a table"))?;
            match table.get("kind") {
                None => {
                    table.insert("kind".to_owned(), json!(name));
                }
                Some(Value::String(k)) if *k == name => {}
                Some(_) => return Err(format!("deciders.{name}.kind: it is the kind {name} or it is absent")),
            }
            let table = Value::Object(table);
            NativeDecider::parse(&json!({"autonomy": "A5", "decider": table}), &table).map_err(|e| format!("deciders.{name}: {e}"))?;
            kinds.insert(name, table);
        }
        Ok(DaemonArbiter { kinds, limits })
    }
}

/// The name of a run's private directory under `$XDG_RUNTIME_DIR/runbook/`: the daemon's process, so
/// that a directory a killed daemon left does not refuse the run's resume, and the run's own name
/// (the last part of its directory) reduced to characters a path part may hold.
fn private_name(run_dir: &Path) -> String {
    let run: String = run_dir.file_name().and_then(|n| n.to_str()).unwrap_or("run").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    format!("daemon-{}-{run}", std::process::id())
}

fn root_name(bindings: &Path) -> Result<&str, String> {
    bindings.file_stem().and_then(|s| s.to_str()).ok_or_else(|| format!("{}: the root bindings file has no name", bindings.display()))
}

impl Arbiter for DaemonArbiter {
    fn kinds(&self) -> Vec<String> {
        self.kinds.keys().cloned().collect()
    }

    fn open(&self, o: &Opening<'_>) -> Result<Opened, String> {
        let level = o.terms.autonomy.ok_or("the run has no autonomy level")?;
        let kind = o.terms.decider.as_deref().ok_or("the run has no decider kind")?;
        let table = self.kinds.get(kind).ok_or_else(|| format!("the decider kind {kind} is not configured"))?;
        // The copy is made from the settings of the run and nothing else: the login is not one of them
        // (the copy has no link to it), and the tables of the kinds are the daemon's own.
        let settings = json!({"autonomy": level.as_str(), "decider": table});
        let name = root_name(o.bindings)?;
        let dst = o.run_dir.join(COPY);
        let root = if o.resume {
            let root = dst.join(format!("{name}.yaml"));
            if !root.is_file() {
                return Err(format!("{}: the run's copy of the bindings is missing, so the run cannot go on", root.display()));
            }
            root
        } else {
            let src = o.bindings.parent().ok_or("the root bindings file has no directory")?;
            bindings_copy::materialise(src, &dst, name, &settings, None)?
        };
        let mut read = settings.clone();
        if let (Some(login), true) = (&self.limits.login_file, kind == "model") {
            read["login_file"] = json!(login);
        }
        let decider = NativeDecider::parse(&read, table)?;
        // The table of who decides is read once, from the copy, before the first action of the run.
        decider.pin_routing(o.run_dir);
        // The model's adapter is given a link in the run's private directory, never the login file's own path; the
        // link goes with the judge, when the run ends.
        let login = self.limits.login_file.as_deref().filter(|_| kind == "model").map(Path::new);
        let private = PrivateDir::create(&private_name(o.run_dir), login)?;
        let path = format!("{}:{SYSTEM_PATH}", dst.join("tools").display());
        let verdict = table.get("verdict").and_then(Value::as_str).map(str::to_owned);
        Ok(Opened {
            bindings: root,
            judge: Box::new(Judge {
                decider,
                kind: kind.to_owned(),
                verdict,
                run_dir: o.run_dir.to_path_buf(),
                workspace: o.workspace.to_path_buf(),
                private,
                bindings_dir: dst,
                env: vec![("PATH".to_owned(), path)],
                decided: HashMap::new(),
                limits: self.limits.clone(),
                spent: 0,
                calls: 0,
            }),
        })
    }
}

struct Judge {
    decider: NativeDecider,
    /// The run's private directory with the link to the login file; removed when the judge is dropped.
    private: Option<PrivateDir>,
    kind: String,
    verdict: Option<String>,
    run_dir: PathBuf,
    workspace: PathBuf,
    bindings_dir: PathBuf,
    /// The whole environment a decider program is given by its caller: nothing of the daemon's own.
    env: Vec<(String, String)>,
    decided: HashMap<String, bool>,
    limits: Limits,
    spent: u64,
    calls: u64,
}

/// The key of a decision for "asked before": case, action, revision and the text Loom gave.
fn key(decision: &Value) -> String {
    let get = |k: &str| decision.get(k).and_then(Value::as_str).unwrap_or("");
    format!("{}\u{1f}{}\u{1f}{}\u{1f}{}", get("case"), get("action"), get("revision"), get("decides"))
}

/// The name that goes in the record for an answer by `principal`: a rule by its name, the decider by its kind.
pub(crate) fn label(kind: &str, verdict: Option<&str>, principal: &str) -> String {
    if principal.starts_with("rule:") || kind != "rule" {
        return principal.to_owned();
    }
    format!("rule:{}", verdict.unwrap_or("approve"))
}

/// The answer as a client writes it: the word, or a JSON object when there are reasons to carry.
fn written(shaped: &shape::Shaped, approve: bool, reasons: Option<String>) -> String {
    match (shaped.result, reasons) {
        (Some(result), Some(reasons)) => json!({"result": result, "reasons": reasons}).to_string(),
        (Some(result), None) => result.to_owned(),
        (None, _) if shaped.deny || !approve => "deny".to_owned(),
        (None, _) => "approve".to_owned(),
    }
}

impl RunArbiter for Judge {
    fn judge(&mut self, decision: &Value) -> Judgement {
        let text = |k: &str| decision.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let (kind, key) = (text("kind"), key(decision));
        if self.decided.contains_key(&key) {
            return Judgement::Open("the decider answered this decision before and Loom asks it again: it waits for a person".to_owned());
        }
        if let Err(why) = runbook::engine::calls::decision_from_json(decision) {
            return Judgement::Open(why);
        }
        let want = self.decider.max_call_micro_usd();
        let mut budget = want;
        if want > 0 {
            if let Some(cap) = self.limits.cap_micro_usd {
                budget = want.min(cap.saturating_sub(self.spent));
                if budget == 0 {
                    return Judgement::Open(format!("the cap of {cap} micro USD for the decider of this run is spent"));
                }
            }
            if let Some(max) = self.limits.max_llm_calls.filter(|max| self.calls >= *max) {
                return Judgement::Open(format!("the decider of this run made its {max} model calls"));
            }
        }
        let request = DecisionRequest { decision, run_dir: &self.run_dir, workspace: &self.workspace, budget_micro_usd: budget, child_env: &self.env, login_link: self.private.as_ref().and_then(PrivateDir::link) };
        let (approve, principal, reasons) = match self.decider.decide(&request) {
            Verdict::CannotDecide { reason } => return Judgement::Open(reason),
            Verdict::Answer { cost_micro_usd: None, .. } => return Judgement::Open("the decider answered without a cost".to_owned()),
            Verdict::Answer { approve, principal, reasons, cost_micro_usd: Some(cost), model_calls } => {
                self.spent = self.spent.saturating_add(cost);
                self.calls = self.calls.saturating_add(model_calls);
                (approve, principal, reasons)
            }
        };
        let by = label(&self.kind, self.verdict.as_deref(), principal.as_deref().unwrap_or(""));
        let shaped = match shape::person_result(&self.bindings_dir, decision, &kind, approve, principal.as_deref(), reasons.as_deref()) {
            Ok(shaped) => shaped,
            Err(Unshaped::Rejected(why) | Unshaped::Failed(why)) => return Judgement::Open(why),
        };
        let carried = shaped.reasons.as_ref().map(|_| shape::clean_reasons(reasons.as_deref(), self.limits.login_file.as_deref(), &self.run_dir, None));
        self.decided.insert(key, approve);
        let answer = written(&shaped, approve, carried);
        Judgement::Answer { answer, by: by.clone(), principal: principal.unwrap_or(by) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_is_named_by_its_verdict_and_the_decider_by_its_kind() {
        assert_eq!(label("rule", Some("approve"), "decider:approve"), "rule:approve");
        assert_eq!(label("rule", Some("reject"), "decider:reject"), "rule:reject");
        assert_eq!(label("rule", Some("approve"), "rule:autopilot"), "rule:autopilot");
        assert_eq!(label("model", None, "decider:model"), "decider:model");
    }

    #[test]
    fn an_answer_is_written_as_a_client_writes_it() {
        let plain = shape::Shaped::default();
        assert_eq!(written(&plain, true, None), "approve");
        assert_eq!(written(&plain, false, None), "deny");
        let denied = shape::Shaped { deny: true, ..shape::Shaped::default() };
        assert_eq!(written(&denied, false, None), "deny");
        let done = shape::Shaped { result: Some("performed"), ..shape::Shaped::default() };
        assert_eq!(written(&done, true, None), "performed");
        let said = written(&shape::Shaped { result: Some("rejected"), ..shape::Shaped::default() }, false, Some("too long".to_owned()));
        assert_eq!(serde_json::from_str::<Value>(&said).unwrap(), json!({"result": "rejected", "reasons": "too long"}));
    }

    fn configured(kind: &str, table: Value) -> Map<String, Value> {
        let mut configured = Map::new();
        configured.insert(kind.to_owned(), table);
        configured
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/autonomy-runbook")
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// The table in `file` with every `person` of the `close` class given to `decider`.
    fn close_to_the_decider(file: &Path) {
        let text = std::fs::read_to_string(file).unwrap();
        assert!(text.contains("\"close\": \"person\""), "the fixture's table no longer gives the close to a person");
        std::fs::write(file, text.replace("\"close\": \"person\"", "\"close\": \"decider\"")).unwrap();
    }

    /// A run opened at A3 with the rule `approve` over a copy of the fixture whose table is rewritten by
    /// `before` first; the judge, the run's directory.
    fn opened_at_a3(dir: &crate::assets::testdir::Dir, name: &str, before: impl Fn(&Path)) -> (Box<dyn RunArbiter>, PathBuf) {
        let root = dir.dir(&format!("{name}-root"));
        copy_tree(&fixture_root(), &root);
        before(&root.join("tools/autonomy-levels.json"));
        let run_dir = dir.dir(&format!("{name}-run"));
        let terms = runbook::runtime::Terms { autonomy: Some(runbook::runtime::Autonomy::A3), decider: Some("rule".to_owned()) };
        let arbiter = DaemonArbiter::new(configured("rule", json!({"verdict": "approve"})), Limits::default()).unwrap();
        let opened = arbiter
            .open(&Opening { run_dir: &run_dir, workspace: &run_dir.join("w"), bindings: &root.join("bindings/autonomy-demo.yaml"), terms: &terms, resume: false })
            .unwrap_or_else(|e| panic!("{e}"));
        (opened.judge, run_dir)
    }

    fn close_of_a_case() -> Value {
        json!({"case": "c1", "kind": "Close", "outcome": "kept", "decides": "close case c1 through kept",
            "command": ["b10x-loom", "tree", "close", "--store", "/nonexistent", "--case", "c1", "--outcome", "kept"]})
    }

    /// What the code does: the judge of a run reads the table it opened with (`open` pins it). A table
    /// rewritten in the run's directory afterwards, which a bound program can do, changes who decides
    /// nothing; the close stays a person's at A3. The control: the same rewrite made before the run opens
    /// is the table it opens with, and the decider takes the close.
    #[test]
    fn a_table_rewritten_after_the_run_opened_is_not_read_by_its_judge_and_one_rewritten_before_is() {
        let dir = crate::assets::testdir::Dir::new("arbiter-pinned");
        let (mut after, run) = opened_at_a3(&dir, "after", |_| {});
        close_to_the_decider(&run.join("tools/autonomy-levels.json"));
        let second = after.judge(&close_of_a_case());
        assert!(matches!(&second, Judgement::Open(why) if why.contains("autonomy A3 gives the class close")), "a table rewritten after the run opened was read: {second:?}");
        let (mut before, _) = opened_at_a3(&dir, "before", close_to_the_decider);
        let control = before.judge(&close_of_a_case());
        assert!(matches!(&control, Judgement::Answer { by, .. } if by == "rule:approve"), "the control did not answer, so the case proves nothing: {control:?}");
    }

    #[test]
    fn a_kind_a_daemon_does_not_take_and_a_table_the_decider_refuses_are_refused_by_name() {
        let refused = |kind: &str, table: Value| DaemonArbiter::new(configured(kind, table), Limits::default()).err().unwrap();
        assert!(refused("scripted", json!({"answers": []})).contains("not a kind a daemon takes"));
        assert!(refused("rule", json!({"verdict": "maybe"})).contains("deciders.rule"));
        assert!(refused("rule", json!({"kind": "model", "verdict": "approve"})).contains("deciders.rule.kind"));
        assert_eq!(DaemonArbiter::new(configured("rule", json!({"verdict": "approve"})), Limits::default()).unwrap().kinds(), ["rule"]);
    }
}
