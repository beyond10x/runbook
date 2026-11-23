//! Forms of a bindings copy that the rule counts as a model node, which no other case holds
//! (`docs/loom-run.md`, "A run without a model node"). Each form is run under `substrate` with every
//! cap at 0 and no ledger, where a run without a model node starts; a form that counts is refused
//! before anything is written, for the ledger. A fake `b10x-loom`: no model call, no network request.

use super::core::{code, stderr, World};

fn set(a: &mut [String], flag: &str, value: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a[i + 1] = value.to_owned();
}

fn without(a: &mut Vec<String>, flag: &str) {
    let i = a.iter().position(|x| x == flag).unwrap();
    a.drain(i..=i + 1);
}

/// The options of run `r1` with every cap at 0 and no ledger, under `substrate`.
fn zero(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    set(&mut a, "--max-llm-calls", "0");
    without(&mut a, "--spend-ledger");
    set(&mut a, "--confinement", "substrate");
    a
}

fn file(w: &World, rel: &str, text: &str) {
    let path = w.root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn one_action(executor: &str) -> String {
    format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: {executor}\n")
}

#[test]
fn more_forms_the_scan_cannot_read_count_as_a_model_node() {
    type Setup = Box<dyn Fn(&World)>;
    let forms: Vec<(&str, Setup)> = vec![
        ("an executor kind the scan does not know", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: script, argv: [tools/sdc-tool.sh, x]}")))),
        ("a program given as a list inside the argv", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [[tools/sdc-tool.sh, x], y]}")))),
        ("the tool behind an anchor that an argv takes by alias", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("x-programs:\n  node: &node [sdc-node, a.b]\n{}", one_action("{kind: program, argv: *node}"))))),
        ("an executor taken whole by alias from another action", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: &e {kind: agent, model: m}\n  - action: c.d\n    executor: *e\n");
        })),
        ("a child bindings file that is a link to a file outside the copy", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", &one_action("{kind: open, name: intake, bindings: child.yaml}"));
            file(w, "elsewhere/child.txt", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
            std::os::unix::fs::symlink(w.root.join("elsewhere/child.txt"), w.root.join("bindings/child.yaml")).unwrap();
        })),
        ("the tool's name with a trailing space", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [\"sdc-node \", a.b]}")))),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (what, setup) in &forms {
        let w = World::new("more-forms");
        setup(&w);
        let o = w.run(&zero(&w));
        if code(&o) != 2 || !stderr(&o).contains("--spend-ledger is required") {
            wrong.push(format!("{what}: exit {} ({})", code(&o), stderr(&o).lines().last().unwrap_or("")));
        }
        if w.run_dir("r1").exists() {
            wrong.push(format!("{what}: a run directory was made"));
        }
    }
    assert!(wrong.is_empty(), "forms that did not count as a model node:\n{}", wrong.join("\n"));
}
