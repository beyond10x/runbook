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
}
