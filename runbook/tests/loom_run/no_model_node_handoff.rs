//! The handoff of a run without a model node under `substrate` with caps of 0: the bound of one step
//! every `tree run` call keeps, and the printed commands the runner does not run. A fake `b10x-loom`,
//! no model call, no network request.

use super::core::{code, World};
use super::fake::tree_run_json;
use super::no_model_node_levels_and_caps::{argvs, last_line, zero, NO_MODEL};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

/// A world whose first `tree run` stops at a handoff that prints `printed` (the words of the
/// command, as JSON), under `substrate` with caps of 0.
fn handoff_world(tag: &str, command: &str) -> (World, Output) {
    let w = World::new(tag);
    std::fs::write(w.root.join("bindings/sdc.yaml"), NO_MODEL).unwrap();
    let handoff = format!("{{\"action\":\"amend.upstream\",\"case\":\"root\",\"command\":{command},\"decides\":\"Add a second stop, then run again.\",\"kind\":\"Handoff\"}}");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{handoff}]")), 3);
    w.fake.answer("tree_run", "2", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "3", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&zero(&w, "substrate"));
    (w, o)
}

/// Every way the printed `tree run` can carry a bound of its own (`--max-steps=9`, the option
/// twice, both forms mixed, the option as the last word) ends as one bound of one step.
#[test]
fn every_form_of_a_bound_in_the_printed_command_becomes_one_step() {
    let forms = [
        r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","substrate","--max-steps=9"]"#,
        r#"["b10x-loom","tree","run","--max-steps","5","--store","@STORE@","--max-steps","7","--confinement","substrate"]"#,
        r#"["b10x-loom","tree","run","--max-steps=3","--store","@STORE@","--confinement","substrate","--max-steps","4"]"#,
        r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","substrate","--max-steps"]"#,
        r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","substrate","--max-steps=","--max-steps=0"]"#,
    ];
    let wrong: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = forms
            .iter()
            .enumerate()
            .map(|(n, printed)| {
                s.spawn(move || {
                    let (w, o) = handoff_world(&format!("zc-bound-{n}"), printed);
                    let runs = w.fake.calls_of(&["tree", "run"]);
                    let mut wrong = Vec::new();
                    if runs.len() < 2 {
                        return vec![format!("{printed}: the printed command was not run: exit {} ({})", code(&o), last_line(&o))];
                    }
                    for call in &runs {
                        let a = &call.argv;
                        let bounds: Vec<usize> = a.iter().enumerate().filter(|(_, x)| x.starts_with("--max-steps")).map(|(i, _)| i).collect();
                        if bounds.len() != 1 || a[bounds[0]] != "--max-steps" || a.get(bounds[0] + 1).map(String::as_str) != Some("1") {
                            wrong.push(format!("{printed}: a tree run call is not bounded to one step: {a:?}"));
                        }
                    }
                    wrong
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A printed command that is no plain `b10x-loom tree run` is not run: another sub-command spelled
/// to look like it, a word that holds a line break, an option before the sub-command, an absolute
/// path to another program, an environment prefix, the command as one text. Each printed command
/// carries a word no other call has; no call of Loom holds it and the other program is not started.
#[test]
fn a_printed_command_that_is_no_plain_tree_run_is_not_run_in_a_run_without_a_model_node() {
    let forms: Vec<(&str, String)> = vec![
        ("the sub-command in another letter case", r#"["b10x-loom","tree","Run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("the sub-command with a trailing space", r#"["b10x-loom","tree","run ","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("the two words of the sub-command as one word", r#"["b10x-loom","tree run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("a line break inside a word of the sub-command", r#"["b10x-loom","tree","run\n--max-steps\n9","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("an option before the sub-command", r#"["b10x-loom","--printed-word","tree","run","--store","@STORE@"]"#.to_owned()),
        ("another sub-command of tree", r#"["b10x-loom","tree","runs","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("the sub-command after another one", r#"["b10x-loom","protocols","tree","run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("an absolute path to another program", r#"["@OTHER@","tree","run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("an environment prefix", r#"["env","PRINTED_WORD=1","b10x-loom","tree","run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("a variable before the program", r#"["PRINTED_WORD=1","b10x-loom","tree","run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("the program name with a path", r#"["./b10x-loom","tree","run","--store","@STORE@","--printed-word"]"#.to_owned()),
        ("the command as one text over two lines", r#""b10x-loom tree run\n--store @STORE@ --printed-word""#.to_owned()),
        ("the command as one word", r#"["b10x-loom tree run --store @STORE@ --printed-word"]"#.to_owned()),
        ("no sub-command", r#"["b10x-loom"]"#.to_owned()),
    ];
    let wrong: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = forms
            .iter()
            .enumerate()
            .map(|(n, (what, printed))| {
                s.spawn(move || {
                    let other = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zc-other-{n}-{}", std::process::id()));
                    let started = PathBuf::from(format!("{}.started", other.display()));
                    let _ = std::fs::remove_file(&started);
                    std::fs::write(&other, format!("#!/bin/sh\necho started > '{}'\n", started.display())).unwrap();
                    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o755)).unwrap();
                    let (w, o) = handoff_world(&format!("zc-noplain-{n}"), &printed.replace("@OTHER@", &other.display().to_string()));
                    let mut wrong = Vec::new();
                    if code(&o) == 0 {
                        wrong.push(format!("{what}: the run ended with exit 0"));
                    }
                    if w.fake.calls_of(&["tree", "run"]).len() != 1 {
                        wrong.push(format!("{what}: {} calls of tree run, 1 expected (the one that printed the command)", w.fake.calls_of(&["tree", "run"]).len()));
                    }
                    for a in argvs(&w) {
                        if a.iter().any(|x| x.contains("printed-word") || x.contains("PRINTED_WORD")) {
                            wrong.push(format!("{what}: the printed command reached Loom: {a:?}"));
                        }
                    }
                    if started.exists() {
                        wrong.push(format!("{what}: the other program was started"));
                    }
                    println!("{what}: exit {}, {}", code(&o), last_line(&o));
                    let _ = std::fs::remove_file(&other);
                    let _ = std::fs::remove_file(&started);
                    wrong
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A measurement, not a guard the unit claims: what the runner runs when the printed command of a
/// handoff names another level, another store or ends its options with `--`. The pinned Loom prints
/// the level and the store of its own call (`loom-tree/src/run.rs`, `handoff_command`), so only
/// another Loom reaches this. The case asserts what a runner that trusts no printed word would
/// hold: every `tree run` call of a run opened under `substrate` without caps carries the level and
/// the store of the run, and its bound stands before any `--`.
#[test]
#[ignore = "measurement, not a guard: the printed level and store of a handoff are run as printed"]
fn measured_the_handoff_call_keeps_the_printed_level_and_store() {
    let forms = [
        ("another level", r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","local"]"#),
        ("the level twice, the last another", r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","substrate","--confinement","local"]"#),
        ("another store", r#"["b10x-loom","tree","run","--store","/nonexistent/zc-other-store","--confinement","substrate"]"#),
        ("the options ended by --", r#"["b10x-loom","tree","run","--store","@STORE@","--confinement","substrate","--"]"#),
    ];
    let mut wrong = Vec::new();
    for (n, (what, printed)) in forms.iter().enumerate() {
        let (w, o) = handoff_world(&format!("zc-printed-{n}"), printed);
        let store = w.run_dir("r1/store").display().to_string();
        for call in w.fake.calls_of(&["tree", "run"]) {
            let a = &call.argv;
            let levels: Vec<&String> = a.iter().enumerate().filter(|(i, _)| *i > 0 && a[i - 1] == "--confinement").map(|(_, x)| x).collect();
            let stores: Vec<&String> = a.iter().enumerate().filter(|(i, _)| *i > 0 && a[i - 1] == "--store").map(|(_, x)| x).collect();
            let end = a.iter().position(|x| x == "--").unwrap_or(a.len());
            let bound = a.iter().position(|x| x == "--max-steps").unwrap_or(usize::MAX);
            if levels != ["substrate"] || stores != [&store] || bound > end {
                wrong.push(format!("{what}: exit {}, call {a:?}", code(&o)));
            }
        }
    }
    assert!(wrong.is_empty(), "tree run calls of a run opened under substrate without caps that do not carry the run's level, store and bound:\n{}", wrong.join("\n"));
}
