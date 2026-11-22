//! The `replay` port of `runbook run` (`--recorded` and the options that go with it): a re-drive of
//! a recorded run, every model call and every decider call answered from the recording.
//!
//! The old launcher made the plan with `sdc-replay prepare` and passed its environment to the
//! driver. Here the plan is settings of the run, and the adapter half of `sdc-replay` (the program
//! the bindings copy and the decider call as their adapter) is used as it is:
//!
//! - [`preset`] asks `sdc-replay prepare --plan-only` (nothing is written) and puts the answer into
//!   the run settings before the decider is built: the source's autonomy and decider (on the
//!   adapter `sdc-replay`, without claim values: Loom's decisions hold none), the models the
//!   caller set, and under `replay` what the bindings copy needs (`BindingPlan`): the adapter of
//!   each model node, and the conditions per action (`max_runs`, `timeout_s`, an argv wrap).
//! - [`prepare`] writes the plan (`replay/` of the run directory: `plan.json`, `index.json`,
//!   `conditions.json`) once the run directory exists, and prints the plan's first line.
//! - [`mark`] puts `sdc-replay settings` into the run settings the run directory records.
//! - [`stopped`] and [`diverged`] read `replay/diverged.json` that the adapter writes when a call
//!   has no recorded reply (policy `stop`, a spent live cap, nothing to reuse): the run ends
//!   `Outcome::Diverged`, exit 11, cause `recording_diverged`.
//! - [`seal`] runs `sdc-replay seal` at the end: the counts into `status.json` or
//!   `run-stopped.json`, the virtual seconds into `rounds.jsonl`, each call's mode into
//!   `llm-calls.jsonl`.
//!
//! The adapter finds its plan above its call's record directory (`replay/plan.json`), because the
//! runner passes a bound program no environment. What the engine cannot give is refused by name
//! when the settings are made: the driver's switches that are environment variables of its loop
//! (`--set max_rejections`, `review_retries`; `--set unstick` is refused as a condition that no
//! longer exists) and the two caps that are options of the
//! runner (`--set max_llm_calls`, `max_wall_seconds`).

use super::ports::{Ports, Refusal, USAGE};
use super::{Args, Outcome};
use serde_json::{json, Value};
use serde_yaml::{Mapping, Value as Yaml};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The helper that holds the adapter, the plan and the seal.
const HELPER: &str = "sdc-replay";
/// The adapter of a model node the source never sent to a model.
const STUB: &str = "stub";
/// The program of a model node of the bindings copy.
const MODEL_TOOL: &str = "sdc-node";
/// The program of the other bound steps of the bindings copy.
const PROGRAM_TOOL: &str = "sdc-tool.sh";

/// The checks of the options that need no run: `--recorded` names a source that exists, and the
/// options that go with it are given only with it. The name is the skeleton's (`ports.rs` calls it
/// for every port); the port is built.
pub fn replay_not_built(ports: &Ports) -> Result<(), Refusal> {
    let replay = &ports.replay;
    match &replay.recorded {
        None => match replay.first_without_recorded() {
            Some(option) => Err((USAGE, format!("{option} needs --recorded <source>"))),
            None => Ok(()),
        },
        Some(source) if !source.is_dir() => Err((USAGE, format!("--recorded needs a run directory or a recording, not '{}'", source.display()))),
        Some(_) => Ok(()),
    }
}

fn recorded(args: &Args) -> Option<&Path> {
    args.ports.replay.recorded.as_deref()
}

/// The helper binary, found as `runbook doctor` finds the others: beside this executable, in a
/// checkout's `driver/target`, then on `PATH`.
fn helper() -> Result<PathBuf, String> {
    let lookup = crate::assets::Lookup::from_process();
    let assets = lookup.locate().ok();
    lookup.helper(HELPER, assets.as_ref()).ok_or_else(|| format!("{HELPER} is not found: build the helper binaries (`cargo build --locked` in driver/) or put {HELPER} on PATH"))
}

fn absolute(p: &Path) -> Result<String, String> {
    std::path::absolute(p).map(|p| p.display().to_string()).map_err(|e| format!("{}: {e}", p.display()))
}

/// The arguments of `sdc-replay prepare` for the options, the plan going to `run_dir`.
fn prepare_args(args: &Args, run_dir: &Path) -> Result<Vec<String>, String> {
    let r = &args.ports.replay;
    let source = r.recorded.as_deref().ok_or("--recorded is not given")?;
    let tree = args.bindings.parent().ok_or("--bindings has no parent directory: the plan reads bindings/tools of the checkout")?;
    let mut v = vec!["prepare".to_owned(), "--source".to_owned(), absolute(source)?, "--out".to_owned(), absolute(run_dir)?, "--tree".to_owned(), absolute(tree)?];
    if let Some(policy) = r.on_divergence {
        v.extend(["--on-divergence".to_owned(), format!("{policy:?}").to_lowercase()]);
    }
    if let Some(a) = &r.live_adapter {
        v.extend(["--live-adapter".to_owned(), a.clone()]);
    }
    if let Some(n) = r.live_cap {
        v.extend(["--live-cap".to_owned(), n.to_string()]);
    }
    for fault in &r.inject {
        v.extend(["--inject".to_owned(), fault.clone()]);
    }
    for set in &r.set {
        v.extend(["--set".to_owned(), set.clone()]);
    }
    if let Some(a) = &r.take_recorded {
        v.extend(["--take-recorded".to_owned(), a.clone()]);
    }
    if let Some(c) = &r.source_tree {
        v.extend(["--source-tree".to_owned(), c.clone()]);
    }
    Ok(v)
}

/// Runs the helper; its standard output is one JSON document. A failure is its last line.
fn run_helper(args: &[String]) -> Result<Value, String> {
    let out = Command::new(helper()?).args(args).stdin(Stdio::null()).output().map_err(|e| format!("cannot start {HELPER}: {e}"))?;
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        let line = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it failed and said nothing");
        return Err(format!("--recorded: {line}"));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("{HELPER} printed no JSON: {e}"))
}

/// The settings of a re-drive: the plan's settings put into `settings`. A member the caller's
/// settings name wins, except the decider's adapter (a re-drive answers every decision from the
/// recording) and what a re-drive cannot take from the caller: an `adapter` (every model node has
/// its own) and a `replay` member. Without `--recorded` the settings are returned as they are.
pub fn preset(args: &Args, run_dir: &Path, mut settings: Value) -> Result<Value, String> {
    if recorded(args).is_none() {
        return Ok(settings);
    }
    let mut command = prepare_args(args, run_dir)?;
    command.push("--plan-only".to_owned());
    let plan = run_helper(&command)?;
    let planned = plan["settings"].as_object().ok_or("the plan names no settings")?;
    let top = settings.as_object_mut().ok_or("settings: an object")?;
    for key in ["adapter", "replay"] {
        if top.contains_key(key) {
            return Err(format!("settings.{key}: --recorded decides it (every model node runs on the adapter of the re-drive); take it out of the settings"));
        }
    }
    for (key, value) in planned {
        match (key.as_str(), top.get_mut(key)) {
            ("decider", Some(own)) => {
                // A model decider of the caller still answers from the recording; a rule or scripted one decides itself.
                if own.get("kind").and_then(Value::as_str) == Some("model") {
                    own["adapter"] = value["adapter"].clone();
                    own["without_claims"] = json!(true);
                }
            }
            ("models", Some(own)) => {
                for (role, set) in value.as_object().into_iter().flatten() {
                    if let Some(own) = own.as_object_mut() {
                        own.entry(role.clone()).or_insert_with(|| set.clone());
                    }
                }
            }
            (_, Some(_)) => {}
            (_, None) => {
                top.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(settings)
}

/// Writes the plan into the run directory and prints its first line. Called once the run
/// directory exists.
pub fn prepare(args: &Args, run_dir: &Path) -> Result<(), String> {
    if recorded(args).is_none() {
        return Ok(());
    }
    run_helper(&prepare_args(args, run_dir)?)?;
    let plan = read(&run_dir.join("replay/plan.json")).ok_or("the plan was not written")?;
    if let Some(line) = plan["first_line"].as_str() {
        println!("{line}");
    }
    Ok(())
}

fn read(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The recorded settings of a re-drive: `replay` is what `sdc-replay settings` says (the source,
/// the trees, the policy, the injections, the conditions).
pub fn mark(args: &Args, run_dir: &Path, mut recorded_settings: Value) -> Value {
    if recorded(args).is_none() {
        return recorded_settings;
    }
    let said = absolute(run_dir).and_then(|r| run_helper(&["settings".to_owned(), "--out".to_owned(), r]));
    if let Some(o) = recorded_settings.as_object_mut() {
        match said {
            Ok(s) => {
                o.insert("replay".to_owned(), s);
            }
            Err(e) => {
                o.insert("replay".to_owned(), json!({"error": e}));
            }
        }
    }
    recorded_settings
}

/// The outcome a divergence of the recording ends the run with, when the adapter wrote
/// `replay/diverged.json`.
pub fn stopped(run_dir: &Path) -> Option<Outcome> {
    let d = read(&run_dir.join("replay/diverged.json"))?;
    let reason = d["summary"].as_str().or_else(|| d["difference"].as_str()).unwrap_or("a call has no recorded reply").to_owned();
    Some(Outcome::Diverged { reason })
}

/// `outcome`, or the divergence when the adapter recorded one: a call that failed because the
/// recording had no reply may have ended the run in any other way (a failed step, a decider that
/// could not decide). A signal stays a signal.
pub fn diverged(run_dir: &Path, outcome: Outcome) -> Outcome {
    if outcome == Outcome::StoppedSignal {
        return outcome;
    }
    stopped(run_dir).unwrap_or(outcome)
}

/// Seals the run directory: the counts, the virtual clock and each call's mode. A failure is a
/// line, the run's outcome is as it was.
pub fn seal(args: &Args, run_dir: &Path) {
    if recorded(args).is_none() {
        return;
    }
    let sealed = absolute(run_dir).and_then(|r| run_helper(&["seal".to_owned(), r]));
    if let Err(e) = sealed {
        eprintln!("runbook run: replay: not sealed ({e})");
    }
}

/// What the bindings copy takes from the plan (`settings.replay`).
#[derive(Debug, Default)]
pub struct BindingPlan {
    adapter: String,
    nodes: Option<Vec<String>>,
    actions: Vec<(String, Conditions)>,
    principal: Option<String>,
}

#[derive(Debug, Default)]
struct Conditions {
    max_runs: Option<u64>,
    timeout_s: Option<u64>,
    wrap: Vec<String>,
}

/// Reads `settings.replay`: `None` when there is none.
pub fn binding_plan(settings: &Value) -> Result<Option<BindingPlan>, String> {
    let Some(replay) = settings.get("replay") else { return Ok(None) };
    let adapter = replay.get("adapter").and_then(Value::as_str).filter(|a| !a.is_empty()).ok_or("settings.replay.adapter: a path")?.to_owned();
    let nodes = match replay.get("nodes") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(a.iter().map(|n| n.as_str().map(str::to_owned).ok_or("settings.replay.nodes: names")).collect::<Result<Vec<_>, _>>()?),
        Some(_) => return Err("settings.replay.nodes: a list of node names, or null for all".to_owned()),
    };
    let mut actions = Vec::new();
    for (name, c) in replay.get("actions").and_then(Value::as_object).into_iter().flatten() {
        let number = |k: &str| -> Result<Option<u64>, String> {
            match c.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("settings.replay.actions.{name}.{k}: a whole number")),
            }
        };
        let wrap = c.get("wrap").and_then(Value::as_array).map(|w| w.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        actions.push((name.clone(), Conditions { max_runs: number("max_runs")?, timeout_s: number("timeout_s")?, wrap }));
    }
    let principal = replay.get("principal").and_then(Value::as_str).filter(|p| !p.is_empty()).map(str::to_owned);
    Ok(Some(BindingPlan { adapter, nodes, actions, principal }))
}

fn key(name: &str) -> Yaml {
    Yaml::String(name.to_owned())
}

impl BindingPlan {
    /// Applies the plan to one bindings document of the copy (`base` is the copy's directory):
    /// the adapter of every model node (this program for a node the source sent to a model, the
    /// stub for the others), and per action the changed conditions. True when anything changed.
    pub fn apply(&self, doc: &mut Yaml, base: &Path) -> bool {
        let mut changed = false;
        let Some(actions) = doc.get_mut("actions").and_then(Yaml::as_sequence_mut) else { return false };
        for action in actions {
            let Some(name) = action.get("action").and_then(Yaml::as_str).map(str::to_owned) else { continue };
            if let Some(argv) = action.get_mut("executor").and_then(|e| e.get_mut("argv")).and_then(Yaml::as_sequence_mut) {
                let tool = argv.first().and_then(Yaml::as_str).and_then(|t| t.rsplit('/').next()).map(str::to_owned);
                if tool.as_deref() == Some(MODEL_TOOL) {
                    let replayed = self.nodes.as_ref().is_none_or(|n| n.contains(&name));
                    changed |= super::bindings_copy::set_option(argv, "--adapter", if replayed { &self.adapter } else { STUB });
                }
                // The source's principal (the driver's SDC_PRINCIPAL, which the aep store records as the actor of a move):
                // the runner passes a bound program no environment, so the tools take it as an option.
                if let Some(principal) = &self.principal {
                    match tool.as_deref() {
                        Some(MODEL_TOOL) => changed |= super::bindings_copy::set_option(argv, "--principal", principal),
                        Some(PROGRAM_TOOL) if !argv.iter().any(|a| a.as_str() == Some("--principal")) => {
                            argv.push(key("--principal"));
                            argv.push(key(principal));
                            changed = true;
                        }
                        _ => {}
                    }
                }
            }
            let Some((_, c)) = self.actions.iter().find(|(n, _)| *n == name) else { continue };
            let Some(map) = action.as_mapping_mut() else { continue };
            if let Some(n) = c.max_runs {
                let budget = map.entry(key("budget")).or_insert_with(|| Yaml::Mapping(Mapping::new()));
                if let Some(b) = budget.as_mapping_mut() {
                    b.insert(key("max_runs"), Yaml::Number(n.into()));
                    changed = true;
                }
            }
            if let Some(t) = c.timeout_s {
                map.insert(key("timeout_seconds"), Yaml::Number(t.into()));
                changed = true;
            }
            if !c.wrap.is_empty() {
                let Some(argv) = map.get_mut("executor").and_then(|e| e.get_mut("argv")).and_then(Yaml::as_sequence_mut) else { continue };
                // The wrapped program is started by the wrapper, from the workspace: a path of the copy is made absolute.
                if let Some(first) = argv.first().and_then(Yaml::as_str).filter(|f| !Path::new(f).is_absolute() && f.contains('/')).map(str::to_owned) {
                    if base.join(&first).exists() {
                        argv[0] = key(&base.join(&first).display().to_string());
                    }
                }
                let mut wrapped: Vec<Yaml> = c.wrap.iter().map(|w| key(w)).collect();
                wrapped.append(argv);
                *argv = wrapped;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> BindingPlan {
        binding_plan(&json!({"replay": {
            "adapter": "/x/sdc-replay",
            "nodes": ["change.implement"],
            "actions": {"change.implement": {"max_runs": 2, "timeout_s": 9}, "tests.run": {"wrap": ["/x/sdc-replay", "program", "--action", "tests.run", "--"]}}
        }})).unwrap().unwrap()
    }

    fn doc() -> Yaml {
        serde_yaml::from_str(
            "actions:\n  - action: change.implement\n    executor: {kind: program, argv: [sdc-node, change.implement, --model, m]}\n    budget: {max_runs: 3}\n    timeout_seconds: 1500\n  - action: idea.dissect\n    executor: {kind: program, argv: [sdc-node, idea.dissect]}\n  - action: tests.run\n    executor: {kind: program, argv: [tools/sdc-tool.sh, story-tests]}\n",
        )
        .unwrap()
    }

    #[test]
    fn a_node_the_source_sent_to_a_model_gets_the_adapter_and_the_others_the_stub() {
        let mut d = doc();
        let scratch = crate::assets::testdir::Dir::new("replay-apply");
        assert!(plan().apply(&mut d, scratch.path()));
        let argv = |i: usize| -> Vec<String> { d["actions"][i]["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect() };
        assert!(argv(0).windows(2).any(|w| w == ["--adapter", "/x/sdc-replay"]), "{:?}", argv(0));
        assert!(argv(1).windows(2).any(|w| w == ["--adapter", "stub"]), "{:?}", argv(1));
    }

    #[test]
    fn the_conditions_go_into_the_budget_the_timeout_and_the_argv() {
        let mut d = doc();
        let scratch = crate::assets::testdir::Dir::new("replay-conditions");
        std::fs::create_dir_all(scratch.path().join("tools")).unwrap();
        std::fs::write(scratch.path().join("tools/sdc-tool.sh"), "").unwrap();
        plan().apply(&mut d, scratch.path());
        assert_eq!((d["actions"][0]["budget"]["max_runs"].as_u64(), d["actions"][0]["timeout_seconds"].as_u64()), (Some(2), Some(9)));
        let wrapped: Vec<&str> = d["actions"][2]["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        let tool = scratch.path().join("tools/sdc-tool.sh").display().to_string();
        assert_eq!(&wrapped[..6], ["/x/sdc-replay", "program", "--action", "tests.run", "--", tool.as_str()]);
        assert_eq!(wrapped[6], "story-tests");
    }

    #[test]
    fn no_replay_member_is_no_plan_and_a_bad_one_is_refused_by_name() {
        assert!(binding_plan(&json!({})).unwrap().is_none());
        assert!(binding_plan(&json!({"replay": {"adapter": ""}})).unwrap_err().contains("settings.replay.adapter"));
        assert!(binding_plan(&json!({"replay": {"adapter": "a", "nodes": "x"}})).unwrap_err().contains("settings.replay.nodes"));
        assert!(binding_plan(&json!({"replay": {"adapter": "a", "actions": {"x": {"max_runs": "two"}}}})).unwrap_err().contains("max_runs"));
    }

    #[test]
    fn a_divergence_ends_the_run_and_a_signal_stays_a_signal() {
        let scratch = crate::assets::testdir::Dir::new("replay-diverged");
        let run = scratch.path();
        assert!(stopped(run).is_none());
        assert_eq!(diverged(run, Outcome::Stuck), Outcome::Stuck);
        std::fs::create_dir_all(run.join("replay")).unwrap();
        std::fs::write(run.join("replay/diverged.json"), json!({"summary": "model call at c/n@1.2: the source made no call"}).to_string()).unwrap();
        let expected = Outcome::Diverged { reason: "model call at c/n@1.2: the source made no call".to_owned() };
        assert_eq!(diverged(run, Outcome::Stuck), expected);
        assert_eq!(diverged(run, Outcome::StoppedDecider { reason: "x".into() }), expected);
        assert_eq!(diverged(run, Outcome::StoppedSignal), Outcome::StoppedSignal);
    }

    #[test]
    fn options_without_recorded_are_refused_and_a_missing_source_too() {
        let mut ports = Ports::default();
        assert!(replay_not_built(&ports).is_ok());
        ports.replay.live_cap = Some(1);
        assert_eq!(replay_not_built(&ports).unwrap_err().1, "--live-cap needs --recorded <source>");
        ports.replay.recorded = Some("/nonexistent-recording".into());
        assert!(replay_not_built(&ports).unwrap_err().1.contains("--recorded needs a run directory or a recording"));
    }
}
