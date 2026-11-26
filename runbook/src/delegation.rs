//! The free run of a delegation (`docs/model-routes.md`, sections 8 and 9): a delegation that names no protocol runs
//! the runbook `delegation-free`, one agent node (`sdc-node delegation.free` on the claude-cli adapter) whose
//! instruction is the request, on the delegation's route, in the run's workspace, writing `result.md`.
//!
//! The runbook is part of the daemon, not of a runbook home: `install` writes it under the daemon's state directory
//! when the daemon starts, and the engine reads it from there for a delegation only (`Engine::with_free_run`). A
//! submit-run that names `delegation-free` does not find it.

use std::path::{Path, PathBuf};

/// The runbook of the free run.
pub const FREE_RUNBOOK: &str = "delegation-free";
/// The one action of the free run, and the agent node it runs.
pub const FREE_ACTION: &str = "delegation.free";
/// The file a delegation's run returns: its text is the result.
pub const RESULT_FILE: &str = "result.md";
/// The most bytes of `result.md` the view `delegation-result` answers.
pub const RESULT_LIMIT: u64 = 1_048_576;
/// The event a delegation's run holds second: the route, the path and the protocol.
pub const DELEGATED_EVENT: &str = "runbook.delegated";
/// The instruction file of the free run's agent node, relative to the repository `sdc-node` reads its instructions from.
pub const INSTRUCTION: &str = "instructions/delegation.free.md";
/// Where a `result.md` that came with a delegation's workspace is moved before the run starts: a dot name, which the products
/// leave out, so that the view `delegation-result` answers only a `result.md` the run wrote.
pub const SET_ASIDE: &str = ".result.from-workspace.md";
/// The directory under the daemon's state directory the free run's home is written to.
const HOME_DIR: &str = "delegation";

// ---- classify, else build (docs/model-routes.md, section 9) ----

/// The runbook a delegation is submitted under when the daemon classifies it as its run starts: the protocol it runs is
/// picked or built then, and the run event [`RESOLVED_EVENT`] names it.
pub const CLASSIFY_RUNBOOK: &str = "delegation";
/// The runbook name of a protocol the builder wrote, in the run's own home ([`BUILT_DIR`] of the run's directory).
pub const BUILT_RUNBOOK: &str = "delegation-built";
/// The directory of the run's directory the generated protocol's home is written to.
pub const BUILT_DIR: &str = "built";
/// The run event that records the path a classified delegation took, its protocol, the classifier's confidence, the
/// classifier route and the digest of a generated protocol.
pub const RESOLVED_EVENT: &str = "runbook.delegation-resolved";
/// The file of the run's directory that holds the record of [`RESOLVED_EVENT`]: a run that goes on from its store takes
/// the same path again without a call.
pub const RESOLVED_FILE: &str = "delegation.json";
/// The directory of the run's work/ directory the classifier's answer and a generated protocol are written to: products of
/// the run.
pub const PRODUCTS_DIR: &str = "delegation";
/// The confidence at or above which the runbook the classifier picked runs (`delegation.classify_threshold`).
pub const CLASSIFY_THRESHOLD: f64 = 0.8;
/// The role of the classify and build calls on the run's relay.
pub const CLASSIFIER_ROLE: &str = "classifier";
/// How many more calls an answer that is not the JSON asked for gets, each with the validator's error (AGENTS.md: a data
/// document a model writes is validated by a program, and an invalid one is asked again at most twice).
pub const RETRIES: usize = 2;
/// What a classify or build call whose cost is not known books (the route declares no price): its reservation, as a call of
/// the decider books `max_call_micro_usd` (`docs/model-routes.md`, section 5). One USD.
pub const CALL_RESERVATION_MICRO_USD: u64 = 1_000_000;
/// The agent node every action of a generated protocol runs: the free run's.
pub const AGENT_NODE: [&str; 2] = ["sdc-node", FREE_ACTION];

const CLASSIFY_PROMPT: &str = include_str!("../../instructions/delegation.classify.md");
const BUILD_PROMPT: &str = include_str!("../../instructions/delegation.build.md");
const RETRY_PROMPT: &str = include_str!("../../instructions/delegation.retry.md");
/// The most characters of a runbook's description the catalog gives the classifier.
const DESCRIPTION_LIMIT: usize = 600;

/// A runbook the daemon can run, as the classifier and the builder are told of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub title: String,
    pub description: String,
}

impl CatalogEntry {
    /// The entry of the protocol document `doc` of the runbook `id`.
    pub fn of(id: &str, doc: &serde_yaml::Value) -> CatalogEntry {
        let description: String = doc["protocol"]["description"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(DESCRIPTION_LIMIT).collect();
        CatalogEntry { id: id.to_owned(), title: crate::runbooks::title_of(doc), description }
    }
}

/// `template` with every `{{name}}` of `values` replaced by its value, in one pass: a value is never read for placeholders.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut rest = template.trim_end_matches('\n');
    let mut out = String::with_capacity(rest.len());
    while let Some(at) = rest.find("{{") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match after.find("}}").and_then(|end| values.iter().find(|(k, _)| *k == &after[..end]).map(|(_, v)| (end, v))) {
            Some((end, value)) => {
                out.push_str(value);
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn catalog_lines(catalog: &[CatalogEntry]) -> String {
    if catalog.is_empty() {
        return "(none)".to_owned();
    }
    catalog.iter().map(|e| serde_json::json!({"id": e.id, "title": e.title, "description": e.description}).to_string()).collect::<Vec<_>>().join("\n")
}

/// What the classifier is asked: the request and the catalog (`instructions/delegation.classify.md`).
pub fn classify_prompt(request: &str, catalog: &[CatalogEntry]) -> String {
    fill(CLASSIFY_PROMPT, &[("catalog", &catalog_lines(catalog)), ("request", request)])
}

/// What the builder is asked: the request, the catalog and the free run as the example (`instructions/delegation.build.md`).
pub fn build_prompt(request: &str, catalog: &[CatalogEntry]) -> String {
    let example_bindings = format!("format: loom.bindings/1\nartifacts:\n  idea: request.yaml\n  result: {RESULT_FILE}\nactions:\n  - action: {FREE_ACTION}\n    executor: {{kind: program, argv: [sdc-node, {FREE_ACTION}]}}\n    evidence:\n      - {{kind: result_written, subject: result, producer: sdc-node, from: exit, results: {{\"0\": written, timeout: failed, \"*\": failed}}}}");
    fill(BUILD_PROMPT, &[("catalog", &catalog_lines(catalog)), ("request", request), ("example_protocol", PROTOCOL.trim_end()), ("example_bindings", &example_bindings)])
}

/// What an answer that could not be used is followed by: the validator's error (`instructions/delegation.retry.md`).
pub fn retry_prompt(error: &str) -> String {
    fill(RETRY_PROMPT, &[("error", error)])
}

/// The JSON object of an answer's text: the text itself, or the inside of one fenced block.
fn object_of(text: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let t = text.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).and_then(|t| t.strip_suffix("```")).unwrap_or(t).trim();
    match serde_json::from_str::<serde_json::Value>(t) {
        Ok(serde_json::Value::Object(m)) => Ok(m),
        Ok(_) => Err("the answer is JSON but not one object".to_owned()),
        Err(e) => Err(format!("the answer is not one JSON object ({e})")),
    }
}

/// What the classifier answered: the runbook it picked (none when none fits) and how sure it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Classified {
    pub protocol: Option<String>,
    pub confidence: f64,
}

/// The classifier's answer checked against its schema: `protocol` the id of a runbook of `catalog` or null, `confidence` a
/// number from 0 to 1, `inputs` an object when given, and no other member. `Err` is the validator's error.
pub fn read_classified(text: &str, catalog: &[CatalogEntry]) -> Result<Classified, String> {
    let m = object_of(text)?;
    if let Some(k) = m.keys().find(|k| !["protocol", "confidence", "inputs"].contains(&k.as_str())) {
        return Err(format!("the member {k:?} is not one of protocol, confidence, inputs"));
    }
    let protocol = match m.get("protocol") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(p)) if catalog.iter().any(|e| &e.id == p) => Some(p.clone()),
        Some(serde_json::Value::String(p)) => return Err(format!("protocol {p:?} is no runbook of the catalog; name one of its ids, or null")),
        Some(_) => return Err("protocol is the id of a runbook as text, or null".to_owned()),
    };
    let confidence = m.get("confidence").and_then(serde_json::Value::as_f64).filter(|c| (0.0..=1.0).contains(c)).ok_or("confidence is a number from 0 to 1")?;
    if m.get("inputs").is_some_and(|i| !i.is_object()) {
        return Err("inputs is an object".to_owned());
    }
    Ok(Classified { protocol, confidence })
}

/// What the builder answered: the free run, or a protocol and its bindings as YAML texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Built {
    Free,
    Protocol { protocol: String, bindings: String },
}

/// The builder's answer checked against its schema: `{"free": true}`, or `protocol` and `bindings` as texts and no other
/// member. What the texts hold is the loader's to check ([`bind_built`] and the engine).
pub fn read_built(text: &str) -> Result<Built, String> {
    let m = object_of(text)?;
    if m.get("free") == Some(&serde_json::Value::Bool(true)) && m.len() == 1 {
        return Ok(Built::Free);
    }
    if let Some(k) = m.keys().find(|k| !["protocol", "bindings"].contains(&k.as_str())) {
        return Err(format!("the member {k:?} is not one of protocol, bindings (or the one member free: true)"));
    }
    let text = |k: &str| m.get(k).and_then(serde_json::Value::as_str).filter(|t| !t.trim().is_empty()).map(str::to_owned).ok_or_else(|| format!("{k} is the YAML text of the document"));
    Ok(Built::Protocol { protocol: text("protocol")?, bindings: text("bindings")? })
}

/// The members of an action of a generated protocol's bindings the builder writes; the daemon writes its executor's
/// arguments, exits, budget and timeout.
const BUILT_ACTION: [&str; 3] = ["action", "executor", "evidence"];
const BUILT_EVIDENCE: [&str; 5] = ["kind", "subject", "producer", "from", "results"];

/// The protocol id and revision of a protocol document's text, and its registration (`<id with . and _ as ->@<revision>`).
pub fn protocol_of(text: &str) -> Result<(String, i64, String), String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| format!("protocol: not YAML: {e}"))?;
    let id = doc["protocol"]["id"].as_str().filter(|i| !i.is_empty()).ok_or("protocol: no protocol.id")?.to_owned();
    let revision = doc["protocol"]["revision"].as_i64().ok_or("protocol: no protocol.revision as a whole number")?;
    let registration = format!("{}@{revision}", id.replace(['.', '_'], "-"));
    Ok((id, revision, registration))
}

/// The bindings the daemon runs a generated protocol with: the builder's artifacts and actions, each action the daemon's
/// agent node as `free_bindings` (the free run's) runs it, the artifact `idea` at `request.yaml` where the agent node reads
/// the request. Refused, naming the member, when the builder names anything but the parts the daemon has: an executor that
/// is not `sdc-node delegation.free`, an evidence not read from the exit, a member the daemon does not take, a path that
/// leaves the workspace.
pub fn bind_built(registration: &str, bindings: &str, free_bindings: &str) -> Result<String, String> {
    use serde_yaml::{Mapping, Value};
    let doc: Value = serde_yaml::from_str(bindings).map_err(|e| format!("bindings: not YAML: {e}"))?;
    let doc = doc.as_mapping().ok_or("bindings: not a mapping")?;
    let free: Value = serde_yaml::from_str(free_bindings).map_err(|e| format!("the free run's bindings: {e}"))?;
    let canon = free["actions"][0].as_mapping().ok_or("the free run's bindings hold no action")?;
    let mut out = Mapping::new();
    out.insert("format".into(), "loom.bindings/1".into());
    out.insert("protocol".into(), registration.into());
    out.insert("program_root".into(), "bindings".into());
    let mut artifacts = Mapping::new();
    let mut actions = Vec::new();
    for (k, v) in doc {
        let key = k.as_str().unwrap_or("");
        match key {
            "format" if v.as_str() == Some("loom.bindings/1") => {}
            "format" => return Err("bindings.format: loom.bindings/1".to_owned()),
            // the daemon's to write
            "protocol" | "program_root" => {}
            "artifacts" => {
                for (name, path) in v.as_mapping().ok_or("bindings.artifacts: a mapping of artifact to path")? {
                    let (Some(name), Some(path)) = (name.as_str(), path.as_str()) else { return Err("bindings.artifacts: each artifact a name and a path as text".to_owned()) };
                    let p = Path::new(path);
                    let plain = |c: std::path::Component| matches!(c, std::path::Component::Normal(n) if !n.to_string_lossy().starts_with('.'));
                    if path.is_empty() || p.is_absolute() || !p.components().all(plain) {
                        return Err(format!("bindings.artifacts.{name}: a path inside the workspace, without .., and no name that starts with a dot"));
                    }
                    artifacts.insert(name.into(), path.into());
                }
            }
            "actions" => {
                for (i, a) in v.as_sequence().ok_or("bindings.actions: a list")?.iter().enumerate() {
                    let a = a.as_mapping().ok_or_else(|| format!("bindings.actions[{i}]: a mapping"))?;
                    let name = a.get("action").and_then(Value::as_str).ok_or_else(|| format!("bindings.actions[{i}].action: the action's name"))?;
                    let at = format!("bindings.actions[{i}] ({name})");
                    if let Some(k) = a.keys().filter_map(Value::as_str).find(|k| !BUILT_ACTION.contains(k)) {
                        return Err(format!("{at}.{k}: not a member a generated protocol's action has (the daemon writes the executor's exits, budget and timeout)"));
                    }
                    let argv: Vec<&str> = a.get("executor").and_then(|e| e["argv"].as_sequence()).map(|s| s.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    let program = a.get("executor").and_then(|e| e["kind"].as_str()) == Some("program");
                    if !program || argv.len() < 2 || argv[..2] != AGENT_NODE {
                        let named = if argv.is_empty() { "no program".to_owned() } else { format!("{:?}", argv.join(" ")) };
                        return Err(format!("{at}.executor: a generated protocol runs only the daemon's agent node sdc-node delegation.free, and this one names {named}"));
                    }
                    let mut evidence = Vec::new();
                    for (j, e) in a.get("evidence").and_then(Value::as_sequence).ok_or_else(|| format!("{at}.evidence: a list"))?.iter().enumerate() {
                        let e = e.as_mapping().ok_or_else(|| format!("{at}.evidence[{j}]: a mapping"))?;
                        if let Some(k) = e.keys().filter_map(Value::as_str).find(|k| !BUILT_EVIDENCE.contains(k)) {
                            return Err(format!("{at}.evidence[{j}].{k}: not a member of an evidence a generated protocol reads"));
                        }
                        if e.get("from").and_then(Value::as_str) != Some("exit") {
                            return Err(format!("{at}.evidence[{j}].from: exit (the agent node's exit status is its evidence)"));
                        }
                        evidence.push(Value::Mapping(e.clone()));
                    }
                    let mut action = canon.clone();
                    action.insert("action".into(), name.into());
                    action.insert("evidence".into(), Value::Sequence(evidence));
                    actions.push(Value::Mapping(action));
                }
            }
            _ => return Err(format!("bindings.{key}: not a part a generated protocol may name (format, artifacts, actions)")),
        }
    }
    artifacts.insert("idea".into(), "request.yaml".into());
    out.insert("artifacts".into(), Value::Mapping(artifacts));
    out.insert("actions".into(), Value::Sequence(actions));
    serde_yaml::to_string(&Value::Mapping(out)).map_err(|e| e.to_string())
}

/// `sha256:<hex>` of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("sha256:{}", sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect::<String>())
}

const PROTOCOL: &str = "format: protocol/1
protocol:
  id: delegation-free
  revision: 1
  description: >-
    The free run of a delegation: one agent node whose instruction is the request, on the delegation's route, in the
    run's workspace, writing result.md.
artifacts:
  idea: {}
  result: {}
evidence_kinds:
  result_written: {}
claims:
  result.written:
    true_when:
      evidence: {kind: result_written, subject: result, result: written}
actions:
  delegation.free:
    effect: write
    may_produce:
      - evidence: result_written
outcomes:
  delivered:
    requires: {claim: result.written}
";

/// The bindings of the free run. `tools_dir`: the `bindings/tools` of the daemon's assets, handed to `sdc-node` when
/// the daemon knows them; without it `sdc-node` finds the tools above its own program.
fn bindings(tools_dir: Option<&Path>) -> String {
    let tools = tools_dir.map(|d| format!(", --tools-dir, {}", quoted(&d.display().to_string()))).unwrap_or_default();
    format!(
        "format: loom.bindings/1
protocol: delegation-free@1
program_root: bindings
artifacts:
  idea: request.yaml
  result: {RESULT_FILE}
actions:
  - action: {FREE_ACTION}
    executor:
      kind: program
      argv: [sdc-node, {FREE_ACTION}, --adapter, claude-cli, --max-turns, \"40\", --max-budget-usd, \"2\", --instructions, '{{\"task\":\"{INSTRUCTION}\"}}'{tools}]
    exits: {{error: [75, 126, 127], not_called: [79]}}
    budget: {{max_runs: 2}}
    timeout_seconds: 3600
    evidence:
      - kind: result_written
        subject: result
        producer: sdc-node
        from: exit
        results: {{\"0\": written, timeout: failed, \"*\": failed}}
"
    )
}

/// A YAML double-quoted scalar of `text`.
fn quoted(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

/// Writes the free run's home (`protocol/delegation-free.protocol.yaml`, `bindings/delegation-free.yaml`) under
/// `state_dir` and returns it. Written at every start, so the home is the daemon's own revision.
pub fn install(state_dir: &Path, tools_dir: Option<&Path>) -> std::io::Result<PathBuf> {
    let home = state_dir.join(HOME_DIR);
    std::fs::create_dir_all(home.join("protocol"))?;
    std::fs::create_dir_all(home.join("bindings"))?;
    std::fs::write(home.join("protocol").join(format!("{FREE_RUNBOOK}.protocol.yaml")), PROTOCOL)?;
    std::fs::write(home.join("bindings").join(format!("{FREE_RUNBOOK}.yaml")), bindings(tools_dir))?;
    Ok(home)
}

/// Move a `result.md` of the workspace (one copied in with it) to [`SET_ASIDE`]; nothing when there is none.
pub fn set_aside(workspace: &Path) -> std::io::Result<()> {
    let found = workspace.join(RESULT_FILE);
    match std::fs::symlink_metadata(&found) {
        Ok(_) => std::fs::rename(&found, workspace.join(SET_ASIDE)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The `bindings/tools` of an asset root, when it holds the tools `sdc-node` reads (`node-classes.json`).
pub fn tools_of(assets_root: &Path) -> Option<PathBuf> {
    let tools = assets_root.join("bindings/tools");
    tools.join("node-classes.json").is_file().then_some(tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bindings_are_yaml_and_name_the_tools_directory_when_given() {
        let plain: serde_yaml::Value = serde_yaml::from_str(&bindings(None)).unwrap();
        let argv = plain["actions"][0]["executor"]["argv"].as_sequence().unwrap();
        assert_eq!(argv[0].as_str(), Some("sdc-node"));
        assert!(!argv.iter().any(|a| a.as_str() == Some("--tools-dir")));
        let with: serde_yaml::Value = serde_yaml::from_str(&bindings(Some(Path::new("/opt/a b/bindings/tools")))).unwrap();
        let argv: Vec<&str> = with["actions"][0]["executor"]["argv"].as_sequence().unwrap().iter().filter_map(|a| a.as_str()).collect();
        assert_eq!(&argv[argv.len() - 2..], ["--tools-dir", "/opt/a b/bindings/tools"]);
        let protocol: serde_yaml::Value = serde_yaml::from_str(PROTOCOL).unwrap();
        assert_eq!(protocol["protocol"]["id"].as_str(), Some(FREE_RUNBOOK));
    }

    /// The prompts of the classify, build and retry calls on stub inputs are their goldens under
    /// `instructions/golden/delegation/` byte for byte (`SDC_BLESS_PROMPTS=1` writes them); the driver's check that every
    /// instruction file is reached by a golden reads them.
    #[test]
    fn the_prompts_of_classify_build_and_retry_are_their_goldens_byte_for_byte() {
        let catalog = vec![CatalogEntry { id: "answer-demo".into(), title: "Answers a question in result.md".into(), description: "One program answers the idea in result.md.".into() }];
        let request = "Write one sentence about tea.";
        let prompts = [
            ("delegation.classify", classify_prompt(request, &catalog)),
            ("delegation.build", build_prompt(request, &catalog)),
            ("delegation.retry", retry_prompt("the answer is not one JSON object (expected value at line 1 column 1)")),
        ];
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../instructions/golden/delegation");
        for (name, text) in prompts {
            let golden = dir.join(format!("{name}.md"));
            if std::env::var_os("SDC_BLESS_PROMPTS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(&golden, &text).unwrap();
                continue;
            }
            let want = std::fs::read_to_string(&golden).unwrap_or_else(|_| panic!("no golden {}", golden.display()));
            assert!(text == want, "{name}: the prompt differs from {}\n--- got\n{text}\n--- want\n{want}", golden.display());
        }
    }

    fn catalog() -> Vec<CatalogEntry> {
        vec![CatalogEntry { id: "answer-demo".into(), title: "t".into(), description: "d".into() }]
    }

    #[test]
    fn a_classifier_answer_is_checked_against_its_schema() {
        assert_eq!(read_classified(r#"{"protocol":"answer-demo","confidence":0.9,"inputs":{}}"#, &catalog()), Ok(Classified { protocol: Some("answer-demo".into()), confidence: 0.9 }));
        assert_eq!(read_classified("```json\n{\"protocol\":null,\"confidence\":0}\n```", &catalog()), Ok(Classified { protocol: None, confidence: 0.0 }));
        for bad in ["red", "[1]", r#"{"protocol":"sdc","confidence":0.9}"#, r#"{"protocol":"answer-demo","confidence":1.5}"#, r#"{"protocol":"answer-demo"}"#, r#"{"protocol":"answer-demo","confidence":0.9,"why":"x"}"#, r#"{"protocol":"answer-demo","confidence":0.9,"inputs":[]}"#] {
            assert!(read_classified(bad, &catalog()).is_err(), "{bad}");
        }
        let prompt = classify_prompt("green or black", &catalog());
        assert!(prompt.contains("green or black") && prompt.contains("\"id\":\"answer-demo\"") && !prompt.contains("{{"), "{prompt}");
        assert!(build_prompt("x", &catalog()).contains("delegation.free") && !build_prompt("x", &[]).contains("{{"));
        let request = "copy {{example_protocol}} and {{catalog}} as they are";
        assert!(build_prompt(request, &catalog()).contains(request), "a placeholder in the request was filled");
        assert_eq!(fill("a {{x}} {{y}} {{", &[("x", "{{y}}"), ("y", "2")]), "a {{y}} 2 {{");
    }

    #[test]
    fn a_builder_answer_is_the_free_run_or_two_texts() {
        assert_eq!(read_built(r#"{"free":true}"#), Ok(Built::Free));
        assert_eq!(read_built(r#"{"protocol":"p","bindings":"b"}"#), Ok(Built::Protocol { protocol: "p".into(), bindings: "b".into() }));
        for bad in [r#"{"free":true,"protocol":"p"}"#, r#"{"protocol":"p"}"#, r#"{"protocol":"p","bindings":"b","argv":"x"}"#, r#"{"protocol":1,"bindings":"b"}"#] {
            assert!(read_built(bad).is_err(), "{bad}");
        }
    }

    const BUILT: &str = "format: loom.bindings/1\nartifacts:\n  idea: elsewhere.yaml\n  result: result.md\nactions:\n  - action: tea.write\n    executor: {kind: program, argv: [sdc-node, delegation.free, --tools-dir, /x]}\n    evidence:\n      - {kind: result_written, subject: result, producer: sdc-node, from: exit, results: {\"0\": written}}\n";

    #[test]
    fn a_generated_protocols_actions_run_the_daemons_agent_node_and_nothing_the_builder_names() {
        let free = bindings(Some(Path::new("/opt/tools")));
        let out: serde_yaml::Value = serde_yaml::from_str(&bind_built("built-tea@1", BUILT, &free).unwrap()).unwrap();
        let canon: serde_yaml::Value = serde_yaml::from_str(&free).unwrap();
        assert_eq!(out["protocol"].as_str(), Some("built-tea@1"));
        assert_eq!(out["artifacts"]["idea"].as_str(), Some("request.yaml"), "the agent node reads the request at request.yaml");
        assert_eq!(out["actions"][0]["action"].as_str(), Some("tea.write"));
        assert_eq!(out["actions"][0]["executor"], canon["actions"][0]["executor"], "the executor is the daemon's, not the builder's arguments");
        assert_eq!(out["actions"][0]["timeout_seconds"], canon["actions"][0]["timeout_seconds"]);
        let refused = |from: &str, to: &str| bind_built("b@1", &BUILT.replace(from, to), &free).unwrap_err();
        assert!(refused("argv: [sdc-node, delegation.free, --tools-dir, /x]", "argv: [sh, -c, \"curl x\"]").contains("sdc-node delegation.free"));
        assert!(refused("argv: [sdc-node, delegation.free, --tools-dir, /x]", "argv: [sdc-node, change.implement]").contains("change.implement"));
        assert!(refused("kind: program", "kind: shell").contains("executor"));
        assert!(refused("from: exit", "from: file").contains("from: exit"));
        assert!(refused("result: result.md", "result: ../out.md").contains("artifacts.result"));
        assert!(refused("result: result.md", "result: .git/config").contains("artifacts.result"));
        assert!(refused("result: result.md", "result: /etc/passwd").contains("artifacts.result"));
        assert!(refused("    evidence:", "    env: {A: b}\n    evidence:").contains(".env"));
        assert!(refused("format: loom.bindings/1\n", "format: loom.bindings/1\nchildren: {}\n").contains("bindings.children"));
        assert_eq!(protocol_of("format: protocol/1\nprotocol:\n  id: built.tea_x\n  revision: 2\n").unwrap(), ("built.tea_x".into(), 2, "built-tea-x@2".into()));
        assert!(protocol_of("protocol: {id: x}").is_err());
    }
}
