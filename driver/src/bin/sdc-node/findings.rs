//! The late-finding loop (docs/trial-2.md, "The late-finding loop"): the stage agents that may raise
//! a finding, the wait while a finding needs a person, the promotion of a raised finding, the
//! marker of the requirements a derived tree comes from.

use crate::util::{env_set, is_exec, run_cmd, sha16, which, Exit, Out, R};
use crate::{yaml, Ctx};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

impl Ctx {
    /// The node is a stage agent that may raise a finding.
    pub fn raises(&self) -> bool {
        const NODES: [&str; 7] = [
            "requirements.normalise", "context.default", "vision.write", "roadmap.write", "spec.author", "plan.decompose",
            "change.implement",
        ];
        NODES.contains(&self.node.as_str())
    }

    /// The tree the node derives from the requirements.
    pub fn derived(&self) -> Option<&'static str> {
        match self.node.as_str() {
            "spec.author" => Some("spec"),
            "change.implement" => Some("impl"),
            "plan.decompose" => Some("aep"),
            _ => None,
        }
    }

    /// While a finding waits for a person's decision, a stage agent is not called: whatever it
    /// wrote would be re-derived once the decision amends the requirements. `Some(NOT_CALLED)` ends the program.
    pub fn wait_for_person(&self) -> R<Option<i32>> {
        let path = self.work.join("findings.yaml");
        if !self.raises() || !path.is_file() {
            return Ok(None);
        }
        let Some(doc) = yaml::load(&path) else { return Ok(None) };
        let needs = |f: &&&Value| yaml::get(f, "status").and_then(Value::as_str) == Some("needs_person");
        let decided = |f: &Value| match yaml::get(f, "decision") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => false,
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };
        let all = yaml::findings(&doc);
        if !all.iter().filter(needs).any(|f| !decided(f)) {
            return Ok(None);
        }
        let ids: Vec<String> = all.iter().filter(needs).map(|f| yaml::text(yaml::get(f, "id"))).collect();
        let msg = self.render("instructions/shared/node.not-called.findings.md", &[("node", &self.node), ("ids", &ids.join(" "))])?;
        crate::util::Out::inherit().line(msg.as_bytes());
        Ok(Some(crate::util::NOT_CALLED))
    }

    /// After the node's output is applied: its raised findings go through the program check into
    /// findings.yaml (a stage agent's finding counts only once that check accepts it), and a
    /// derived tree records the requirements revision it was derived from (`.derived-from`).
    /// `raised` is the file of findings the node wrote, or empty.
    pub fn post(&self, raised: &str, out: &mut Out) -> R<()> {
        if !self.raises() {
            return Ok(());
        }
        let mut raised = raised.to_string();
        let mut fixture: Option<(PathBuf, Value)> = None;
        if raised.is_empty() {
            if let Some(list) = env_set("SDC_STUB_FINDINGS") {
                for f in list.replace(':', " ").split_whitespace() {
                    let p = Path::new(f);
                    if let Some(doc) = yaml::load(p).filter(|_| p.is_file()) {
                        if yaml::text(yaml::get(&doc, "node")) == self.node {
                            fixture = Some((p.to_path_buf(), doc));
                            break;
                        }
                    }
                }
            }
        }
        if let Some((path, doc)) = fixture {
            raised = path.display().to_string();
            // A fixture with `rounds:` raises rounds[n] on the node's n-th applied run (0-based), then none.
            if let Some(rounds) = yaml::get(&doc, "rounds").map(|r| r.as_sequence().cloned().unwrap_or_default()) {
                let counter = self.work.join(format!(".sdc/stub-round.{}", self.node));
                let _ = fs::create_dir_all(self.work.join(".sdc"));
                let n: usize = fs::read_to_string(&counter).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
                let _ = fs::write(&counter, format!("{}\n", n + 1));
                raised = String::new();
                if let Some(round) = rounds.get(n) {
                    let target = self.work.join(format!(".sdc/stub-findings.{}.yaml", self.node));
                    let mut m = Mapping::new();
                    m.insert("node".into(), yaml::get(&doc, "node").cloned().unwrap_or(Value::Null));
                    m.insert("findings".into(), round.clone());
                    let _ = fs::write(&target, serde_yaml::to_string(&Value::Mapping(m)).unwrap_or_default());
                    raised = target.display().to_string();
                }
            }
        }
        if !raised.is_empty() && Path::new(&raised).is_file() {
            let looked = format!("PATH, then {}/driver/target/debug/sdc-findings", self.set.root.display());
            let bin = which("sdc-findings").unwrap_or_else(|| self.set.root.join("driver/target/debug/sdc-findings"));
            if !is_exec(&bin) {
                out.err_line(&format!("llm-node: sdc-findings not found (looked at {looked}): {} could not run", self.node));
                return Err(Exit(127));
            }
            let rules = self.set.here.join("consistency-rules.json");
            run_cmd(Command::new(&bin).arg("--rules").arg(rules).arg("promote").arg(&self.node).arg(&raised), out);
        }
        if let Some(t) = self.derived() {
            if self.work.join(t).is_dir() {
                self.mark_derived(t);
            }
        }
        Ok(())
    }

    /// `<tree>/.derived-from`: the requirements digest and the findings resolved so far.
    fn mark_derived(&self, tree: &str) {
        let ids = yaml::resolved_ids(&self.work.join("findings.yaml")).join(" ");
        let line = format!("requirements {} resolved {ids}\n", sha16(&self.work.join("requirements.yaml")));
        let _ = fs::write(self.work.join(tree).join(".derived-from"), line);
    }

    /// A derived tree is not re-derived by the model when every decision taken since this node
    /// last wrote it is the default of a finding this node raised: only the requirements digest in
    /// `.derived-from` is renewed (a new revision, so the checks run again). Not on a retry and not
    /// for a rejection. `Some(0)` ends the program, with the exit of a counted run: the renewed marker is
    /// what moves the case (an exit of `exits.not_called` records nothing, the case would not move: the
    /// run of `spec-own-default` of `cycles/tree-suite.yaml` ends Stuck).
    pub fn derived_skip(&self) -> R<Option<i32>> {
        let Some(t) = self.derived() else { return Ok(None) };
        let marker = self.work.join(t).join(".derived-from");
        let findings = self.work.join("findings.yaml");
        if !self.raises() || self.set.feedback.is_some() || self.rejection.is_some() || !marker.is_file() || !findings.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&marker).unwrap_or_default();
        let first = text.lines().next().unwrap_or("");
        let Some(rest) = first.strip_prefix("requirements ").filter(|r| r.contains(" resolved")) else { return Ok(None) };
        let after = rest.split_once(" resolved").map_or("", |(_, a)| a);
        let was = format!(" {after} ");
        let resolved = yaml::resolved_ids(&findings);
        let new: Vec<&String> = resolved.iter().filter(|id| !was.contains(&format!(" {id} "))).collect();
        let own: Vec<String> = yaml::load(&findings)
            .map(|doc| {
                yaml::findings(&doc)
                    .into_iter()
                    .filter(|f| {
                        yaml::get(f, "status").and_then(Value::as_str) == Some("resolved")
                            && yaml::get(f, "raised_by").and_then(Value::as_str) == Some(&format!("llm:{}", self.node))
                            && yaml::get(f, "resolution").and_then(|r| yaml::get(r, "default_taken")) == Some(&Value::Bool(true))
                    })
                    .map(|f| yaml::text(yaml::get(f, "id")))
                    .collect()
            })
            .unwrap_or_default();
        if new.is_empty() || !new.iter().all(|id| own.contains(id)) {
            return Ok(None);
        }
        self.mark_derived(t);
        let decisions = new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
        let msg = self.render("instructions/shared/node.not-called.derived.md", &[("node", &self.node), ("tree", t), ("decisions", &decisions)])?;
        Out::inherit().line(msg.as_bytes());
        Ok(Some(0))
    }
}
