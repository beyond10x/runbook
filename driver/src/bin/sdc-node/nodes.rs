//! The inputs and the task of each node (lines 418-599 of `llm-node.sh`).

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
    /// The operations offered when the binding names none.
    pub ops: &'static [&'static str],
    /// The turn cap when the binding names none.
    pub turns: u32,
    /// What the review node adds.
    pub review: Option<crate::review::ReviewInputs>,
}

const WRITER_OPS: &[&str] = &["dir.list", "file.edit", "file.read", "file.write", "search"];

impl Built {
    /// A node with the default operations and turn cap.
    pub fn new(out: &'static str, schema: Option<PathBuf>, task: String) -> Built {
        Built { out, schema, task, ops: WRITER_OPS, turns: 30, review: None }
    }
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

/// `jq -r '.commands | keys[]'` of the file: the sorted names of an object, the indexes of an
/// array, nothing where jq refuses.
fn command_names(ir: &Path) -> String {
    use crate::ordjson::Ov;
    let Some(commands) = fs::read_to_string(ir).ok().and_then(|t| Ov::parse(&t).ok()).and_then(|d| match d {
        Ov::Obj(_) => d.get("commands").cloned(),
        _ => None,
    }) else {
        return String::new();
    };
    match commands {
        Ov::Obj(m) => {
            let mut keys: Vec<String> = m.into_iter().map(|(k, _)| k).collect();
            keys.sort();
            keys.iter().map(|k| format!("{k}\n")).collect()
        }
        Ov::Arr(a) => (0..a.len()).map(|i| format!("{i}\n")).collect(),
        _ => String::new(),
    }
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
        Ok(Built::new(out, None, c.render_part("task", &[])?))
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
            Ok(Built::new("spec", None, c.render_part("task", &[("system", &system)])?))
        }
        "bug.dissect" => {
            inp.input("bug.yaml");
            inp.input("impl");
            let schema = rec.join("schema.json");
            let ok = run_to_file(Command::new(c.set.here.join("sdc-tool.sh")).args(["bugfix", "schema", "dissection"]), &schema) == 0;
            let compact = fs::read_to_string(&schema).ok().and_then(|t| crate::ordjson::Ov::parse(&t).ok()).map(|v| v.compact()).unwrap_or_default();
            Ok(Built::new("bug-dissection.json", ok.then_some(schema), c.render_part("task", &[("schema", &compact)])?))
        }
        "questions.answer" => plain(inp, &["idea.yaml", "dissection.yaml", "questions.yaml"], "questions.yaml", Some("questions.yaml")),
        "context.default" => plain(inp, &["idea.yaml", "requirements.yaml"], "context.yaml", None),
        "vision.write" => plain(inp, &["idea.yaml", "requirements.yaml", "context.yaml"], "aep/vision/product.md", None),
        "roadmap.write" => plain(inp, &["idea.yaml", "requirements.yaml", "context.yaml", "aep/vision/product.md"], "aep/initiative/roadmap.md", None),
        "plan.decompose" => {
            inp.input("requirements.yaml");
            inp.input("spec");
            // The command names of the compiled specification, one per line; a file that cannot be
            // read leaves the list empty.
            let _ = fs::write(cwd.join("in/commands.txt"), command_names(&work.join(".sdc/gate/ir.json")));
            let notes = c.render_part("inputs", &[])?;
            inp.notes(&notes);
            Ok(Built::new("aep", None, c.render_part("task", &[("epic", &c.names().0)])?))
        }
        "change.review" => crate::review::inputs(c, rec, cwd, inp),
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
            Ok(Built::new("repro", None, task))
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
            Ok(Built::new("impl", None, task))
        }
        other => Err(die(format!("llm-node: no agent node {other}"), 64)),
    }
}
