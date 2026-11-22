//! The inputs and the task of each node (lines 418-599 of `llm-node.sh`): the seven plain nodes.

use crate::util::{cp_contents, cp_r, die, run_to_file, R};
use crate::{yaml, Ctx};
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The inputs copied into the call's `in/` and listed in the prompt, in the order they were given.
pub struct Inputs<'a> {
    work: &'a Path,
    cwd: &'a Path,
    /// The entries of the prompt's list of inputs.
    pub list: Vec<String>,
}

impl<'a> Inputs<'a> {
    /// An empty list for the call whose directory is `cwd`.
    pub fn new(work: &'a Path, cwd: &'a Path) -> Inputs<'a> {
        Inputs { work, cwd, list: Vec::new() }
    }

    /// Copies one declared input from work/ into in/ (a missing one is listed as absent). A copy
    /// that fails is not an error: the entry is listed all the same.
    pub fn input(&mut self, p: &str) {
        if self.work.join(p).exists() {
            let dst = self.cwd.join("in").join(p);
            if let Some(parent) = dst.parent() {
                if fs::create_dir_all(parent).is_ok() {
                    let _ = cp_r(&self.work.join(p), &dst);
                }
            }
            self.list.push(format!("in/{p}"));
        } else {
            self.list.push(format!("in/{p} (absent)"));
        }
    }

    /// An output edited in place starts as a copy of the current artifact.
    pub fn seed(&self, p: &str) {
        if self.work.join(p).exists() {
            let dst = self.cwd.join("out").join(p);
            if let Some(parent) = dst.parent() {
                if fs::create_dir_all(parent).is_ok() {
                    let _ = cp_r(&self.work.join(p), &dst);
                }
            }
        }
    }

    /// One input note per line of a rendered file.
    pub fn notes(&mut self, text: &str) {
        self.list.extend(text.split('\n').map(str::to_string));
    }
}

/// What a node's inputs and task come to.
pub struct Built {
    /// The declared output artifact.
    pub out: &'static str,
    /// The response schema of a data-document node.
    pub schema: Option<PathBuf>,
    /// The task text.
    pub task: String,
}

/// The case's commands as the programs will run them (`sdc-bugfix checks`), as in/checks.yaml.
fn checks_input(c: &Ctx, cwd: &Path, inp: &mut Inputs) -> R<Option<Value>> {
    let target = cwd.join("in/checks.yaml");
    if run_to_file(Command::new(c.set.here.join("sdc-tool.sh")).args(["bugfix", "checks"]), &target) != 0 {
        return Err(die(
            format!("llm-node: {} could not run: the case's checks.yaml cannot be read (sdc-bugfix checks)", c.node),
            126,
        ));
    }
    inp.list.push("in/checks.yaml".to_string());
    Ok(yaml::load(&target))
}

/// The gate commands of the checks, one line each.
fn gate(checks: Option<&Value>) -> String {
    let Some(items) = checks.and_then(|d| yaml::get(d, "gate")).and_then(Value::as_sequence) else { return String::new() };
    let lines: Vec<String> = items
        .iter()
        .map(|g| format!("  - {}: `{}`", yaml::str_or_empty(g, "name"), yaml::join_list(yaml::get(g, "argv"))))
        .collect();
    lines.join("\n")
}

/// Reads the inputs of `c.node` and renders its task.
pub fn build(c: &Ctx, rec: &Path, cwd: &Path, inp: &mut Inputs) -> R<Built> {
    let work = &c.work;
    let plain = |inp: &mut Inputs, inputs: &[&str], out: &'static str, seed: Option<&str>| -> R<Built> {
        for i in inputs {
            inp.input(i);
        }
        if let Some(s) = seed {
            inp.seed(s);
        }
        Ok(Built { out, schema: None, task: c.render_part("task", &[])? })
    };
    match c.node.as_str() {
        "idea.dissect" => plain(inp, &["idea.yaml"], "dissection.yaml", None),
        "questions.write" => plain(inp, &["idea.yaml", "dissection.yaml"], "questions.yaml", None),
        "requirements.normalise" => plain(inp, &["idea.yaml", "dissection.yaml", "questions.yaml"], "requirements.yaml", None),
        "spec.author" => {
            for i in ["idea.yaml", "requirements.yaml", "context.yaml", "spec"] {
                inp.input(i);
            }
            inp.seed("spec");
            let example = cwd.join("in/example-spec");
            if fs::create_dir_all(&example).is_ok() {
                let _ = cp_contents(&c.fx().join("billing-spec"), &example);
            }
            let notes = c.render_part("inputs", &[])?;
            inp.notes(&notes);
            let system = c.set.args.field("domain").map_or_else(|| "system".to_string(), |d| d.raw());
            Ok(Built { out: "spec", schema: None, task: c.render_part("task", &[("system", &system)])? })
        }
        "bug.dissect" => {
            inp.input("bug.yaml");
            inp.input("impl");
            let schema = rec.join("schema.json");
            let ok = run_to_file(Command::new(c.set.here.join("sdc-tool.sh")).args(["bugfix", "schema", "dissection"]), &schema) == 0;
            let compact = fs::read_to_string(&schema).ok().and_then(|t| crate::ordjson::Ov::parse(&t).ok()).map(|v| v.compact()).unwrap_or_default();
            Ok(Built {
                out: "bug-dissection.json",
                schema: ok.then_some(schema),
                task: c.render_part("task", &[("schema", &compact)])?,
            })
        }
        "test.reproduce" => {
            for i in ["bug.yaml", "bug-dissection.json", "impl"] {
                inp.input(i);
            }
            let checks = checks_input(c, cwd, inp)?;
            let ck = |k: &str| yaml::text(checks.as_ref().and_then(|d| yaml::get(d, k)));
            let join = |k: &str| yaml::join_list(checks.as_ref().and_then(|d| yaml::get(d, k)));
            let task = c.render_part(
                "task",
                &[("test_file", &ck("test_file")), ("test_name", &ck("test_name")), ("repro_command", &join("repro")), ("test_command", &join("test"))],
            )?;
            Ok(Built { out: "repro", schema: None, task })
        }
        "change.fix" => {
            for i in ["bug.yaml", "bug-dissection.json", "repro", "impl"] {
                inp.input(i);
            }
            inp.seed("impl");
            let checks = checks_input(c, cwd, inp)?;
            let repro = fs::read_to_string(work.join("repro/repro.json")).ok().and_then(|t| crate::ordjson::Ov::parse(&t).ok());
            let from_repro = |k: &str| repro.as_ref().and_then(|r| r.field(k)).map(|v| v.raw()).unwrap_or_default();
            let repro_file = from_repro("file");
            // A run seeded from a ticket works on a repository that may have dependencies of its own.
            let mut stdlib = c.render_part("stdlib", &[])?;
            if c.has_ticket() {
                stdlib.clear();
            }
            let mut gate = gate(checks.as_ref());
            if gate.is_empty() {
                gate = c.render_part("no_gate", &[])?;
            }
            let test_command = yaml::join_list(checks.as_ref().and_then(|d| yaml::get(d, "test")));
            let task = c.render_part(
                "task",
                &[("repro_test", &from_repro("test")), ("repro_file", &repro_file), ("stdlib", &stdlib), ("test_command", &test_command), ("gate", &gate)],
            )?;
            Ok(Built { out: "impl", schema: None, task })
        }
        other => Err(die(format!("llm-node: no agent node {other}"), 64)),
    }
}
