//! The one settings format, `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`), and what a
//! run does with the file `sdc-experiment run` writes.
//!
//! `tests/fixtures/loom-run-settings.toml` is the one copy of that file kept on this side. The
//! launcher's test (`driver/tests/experiment_loom.rs`, the test that names the fixture) prints the
//! file for a record and compares it with the fixture; the tests here run the fixture. Neither
//! crate's test builds the other.

use super::core::{code, stderr, World};
use super::decider::{fixture, fixture_settings, install_decider, lines, tree_with_approval, write_settings, APPROVE};
use serde_json::{json, Value};

/// Runs with the fixture changed by `mutate` and asserts a refusal (exit 2, nothing written, no
/// call) that names `needle`.
fn refused(tag: &str, mutate: impl Fn(&mut Value), needle: &str) {
    let w = World::new(tag);
    let mut v = fixture_settings(&w, 5_000_000);
    mutate(&mut v);
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 2, "{needle}: {}", stderr(&o));
    assert!(stderr(&o).contains(needle), "{needle} not named: {}", stderr(&o));
    assert!(!w.run_dir("r1").exists(), "{needle}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{needle}: the binary was called");
    assert!(!w.ledger().exists(), "{needle}: the ledger was made");
}

#[test]
fn the_file_sdc_experiment_writes_runs_whole_with_the_model_decider_it_names() {
    // Every member of the fixture is accepted and none is left to guess: the run takes its name, its
    // caps (from the file, through `args_for`) and its decider from the file, and ends delivered.
    let w = World::new("fixture-runs");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let v = fixture(&w);
    write_settings(&w, &v);
    let o = w.run(&w.args_for(&v));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let run = v["run"].as_str().unwrap();
    assert_eq!(w.outcome(run)["outcome"], "ended");
    // decider.kind and autonomy: the model decider answered the approval, as the principal the
    // runner derives from the kind (the file names none).
    let approve = w.fake.calls_of(&["tree", "approve"]);
    assert_eq!(approve.len(), 1);
    let at = approve[0].argv.iter().position(|a| a == "--principal").unwrap();
    assert_eq!(approve[0].argv[at + 1], "decider:model");
    // decider.adapter names the fake adapter of the copy, decider.model reaches its request and
    // sdc-reply found on PATH its environment.
    let env = std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/tools/env.txt"))).unwrap();
    let asked: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/tools/adapter-request.json"))).unwrap()).unwrap();
    assert_eq!(asked["model"], "claude-opus-5-5");
    assert!(env.contains(&format!("SDC_REPLY={}", w.root.join("sdc-reply").display())), "{env}");
    // adapter and review.adapter: the model nodes of the copy run on metaharness, the review class on claude-cli.
    let bindings = std::fs::read_to_string(w.run_dir(&format!("{run}/bindings/sdc.yaml"))).unwrap();
    assert!(bindings.contains("--adapter"), "{bindings}");
    assert_eq!(lines(&w.run_dir(&format!("{run}/decisions.jsonl"))).len(), 1);
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir(&format!("{run}/settings.json"))).unwrap()).unwrap();
    assert_eq!(recorded["login_file"], "named");
    assert_eq!(recorded["autonomy"], "A5");
    assert_eq!(recorded["decider"]["kind"], "model");
}

#[test]
fn the_file_without_the_members_the_launcher_now_writes_is_stopped_where_the_decider_needs_them() {
    // Without `autonomy` the model decider is built and the run stops at the first decision for a
    // person: it answers only at A5. This is what the launcher wrote before it wrote `autonomy`.
    let w = World::new("fixture-no-autonomy");
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let mut v = fixture_settings(&w, 5_000_000);
    v.as_object_mut().unwrap().remove("autonomy");
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 8, "{}", stderr(&o));
    let reason = w.outcome("r1")["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("no autonomy level named"), "{reason}");
    // And without `decider.kind` the decider is refused by name.
    refused("fixture-no-kind", |v| {
        v["decider"].as_object_mut().unwrap().remove("kind");
    }, "decider.kind");
}

#[test]
fn a_member_that_repeats_an_option_must_agree_with_it_and_is_refused_by_name() {
    refused("sha", |v| v["loom_sha256"] = json!("00".repeat(32)), "loom_sha256");
    refused("conf", |v| v["confinement"] = json!("substrate"), "confinement is");
    refused("cap", |v| v["cap_micro_usd"] = json!(1), "cap_micro_usd");
    refused("calls", |v| v["max_llm_calls"] = json!(150), "max_llm_calls");
    refused("wall", |v| v["max_wall_seconds"] = json!(3600), "max_wall_seconds");
    refused("calls-text", |v| v["max_llm_calls"] = json!("100"), "max_llm_calls");
    refused("run", |v| v["run"] = json!("another"), "is not the run named by --run");
    refused("format", |v| v["format"] = json!("runbook.loom-run-settings/2"), "is not runbook.loom-run-settings/1");
    refused("bin", |v| v["loom"] = json!({"bin": "/opt/loom/b10x-loom"}), "loom.bin");
    refused("loom-key", |v| v["loom"] = json!({"extra": 1}), "loom.extra");
}

#[test]
fn the_login_file_has_one_place_and_the_deciders_program_is_the_runners() {
    refused(
        "login-old-place",
        |v| v["loom"] = json!({"login_file": "/somewhere/else"}),
        "loom.login_file",
    );
    refused("program", |v| v["decider"]["program"] = json!("/opt/decider"), "decider.program");
    refused("principal", |v| v["decider"]["principal"] = json!("person:owner"), "decider.principal");
}

#[test]
fn a_scripted_decider_takes_the_principal_of_its_answers_from_the_settings() {
    // Loom prints `operator` for every decision. `decider.principal` is the default of the answers,
    // an answer's own `principal` wins, and an answer with neither keeps what Loom printed.
    use super::fake::{decision_json, tree_run_json};
    let w = World::new("scripted-principal");
    let three = format!("[{},{},{}]", decision_json("a.one", "ApproveAction", "operator"), decision_json("a.two", "Approve", "operator"), decision_json("a.three", "ApproveAction", "operator"));
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &three), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": [
        {"approve": true, "cost_micro_usd": 0},
        {"approve": true, "principal": "person:other", "cost_micro_usd": 0},
        {"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let principal_of = |c: &super::fake::Call| c.argv[c.argv.iter().position(|a| a == "--principal").unwrap() + 1].clone();
    let principals: Vec<String> = w.fake.calls_of(&["tree", "approve"]).iter().map(principal_of).collect();
    assert_eq!(principals, ["person:owner", "person:other", "person:owner"]);
    // Without `decider.principal` an answer without one keeps the principal Loom printed.
    let w = World::new("scripted-no-principal");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{}]", decision_json("a.one", "ApproveAction", "operator"))), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(principal_of(&w.fake.calls_of(&["tree", "approve"])[0]), "operator");
}

#[test]
fn a_decider_kind_person_is_refused_by_name_before_any_run_directory_or_call() {
    // `cycles/suite.yaml` autopilot-person: a run is not started with a person as its decider.
    refused("kind-person", |v| v["decider"]["kind"] = json!("person"), "person");
    refused("kind-person-no-autonomy", |v| {
        v["decider"]["kind"] = json!("person");
        v.as_object_mut().unwrap().remove("autonomy");
    }, "person");
}

#[test]
fn a_stand_in_names_its_principal_the_run_directory_labels_it_and_no_principal_is_invented() {
    // `cycles/suite.yaml` principals: the scripted stand-in is `person:stand-in` in the approval and in
    // the decision line, its run settings say what kind of decider it is, nothing is recorded as
    // `human`; a decision that prints no principal and an answer that names none runs with none.
    use super::fake::{decision_json, tree_run_json};
    let w = World::new("stand-in-principal");
    let d = decision_json("a.one", "ApproveAction", "operator");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{d}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "principal": "person:stand-in", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let argv = &w.fake.calls_of(&["tree", "approve"])[0].argv;
    assert_eq!(argv[argv.iter().position(|a| a == "--principal").unwrap() + 1], "person:stand-in");
    let dir = w.run_dir("r1");
    let decided = lines(&dir.join("decisions.jsonl"));
    assert_eq!(decided[0]["principal"], "person:stand-in");
    let first = lines(&dir.join("run-settings.jsonl")).remove(0);
    assert_eq!(first["decider"]["kind"], "scripted");
    assert_eq!(first["decider"]["principal"], "person:stand-in");
    let mut hits = Vec::new();
    super::core::grep_tree(&dir, "human", &mut hits);
    assert!(hits.is_empty(), "something records `human`: {hits:?}");
    // No principal in the printed command and none in the answer: the approval goes out without one.
    let w = World::new("no-principal");
    let bare = decision_json("a.one", "ApproveAction", "operator").replace(",\"--principal\",\"operator\"", "");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 0, None, &format!("[{bare}]")), 3);
    w.fake.answer("tree_approve", "default", "{\"decision\":\"Allow\"}\n", 0);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    w.settings(&json!({"decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&w.args("r1", 1_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!w.fake.calls_of(&["tree", "approve"])[0].argv.iter().any(|a| a == "--principal"));
    assert_eq!(lines(&w.run_dir("r1/decisions.jsonl"))[0]["principal"], "");
}

// ---- the TOML form ----

use std::sync::atomic::{AtomicUsize, Ordering};

/// A tag per call that names no member, so a path in a message never matches a needle by accident.
fn tag(prefix: &str) -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    format!("{prefix}{}", N.fetch_add(1, Ordering::Relaxed))
}

/// The fixture as TOML text for run `r1`.
fn base_text(w: &World) -> String {
    super::core::toml_text(&fixture_settings(w, 5_000_000))
}

/// `text` with the line `key = raw` put into `table` (`None`: the top level), a line of that key
/// already there taken out, a table that is not there appended.
fn member(text: &str, table: Option<&str>, key: &str, raw: &str) -> String {
    let line = format!("{key} = {raw}");
    let header = table.map(|t| format!("[{t}]"));
    let drop = format!("{key} = ");
    let mut out: Vec<String> = Vec::new();
    let mut here = table.is_none();
    let mut placed = false;
    for l in text.lines() {
        if l.starts_with('[') {
            if here && !placed {
                out.push(line.clone());
                placed = true;
            }
            here = header.as_deref() == Some(l);
            out.push(l.to_owned());
        } else if !(here && l.starts_with(&drop)) {
            out.push(l.to_owned());
        }
    }
    if !placed {
        match (here, header) {
            (true, _) => out.push(line),
            (false, Some(h)) => out.extend([String::new(), h, line]),
            (false, None) => unreachable!("the top level is always a place"),
        }
    }
    out.join("\n") + "\n"
}

/// Runs with the settings file `text` under the file name `name` and returns the output.
fn run_text(w: &World, name: &str, text: &str) -> std::process::Output {
    std::fs::write(w.root.join(name), text).unwrap();
    let mut args = w.args("r1", 5_000_000);
    let i = args.iter().position(|a| a == "--settings").unwrap();
    args[i + 1] = w.root.join(name).display().to_string();
    w.run(&args)
}

/// A refusal before anything starts: exit 2, no run directory, no ledger, no call; `needles` named.
fn assert_refused(w: &World, o: &std::process::Output, what: &str, needles: &[&str]) {
    assert_eq!(code(o), 2, "{what}: {}", stderr(o));
    for needle in needles {
        assert!(stderr(o).contains(needle), "{what}: {needle} not named: {}", stderr(o));
    }
    assert!(!w.run_dir("r1").exists(), "{what}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{what}: the binary was called");
    assert!(!w.ledger().exists(), "{what}: the ledger was made");
}

/// What a refused value of `table` ends in: the members of `models` and `review` are read when the
/// bindings copy is made, after the run directory exists, so they end the run (any non-zero exit);
/// every other member is refused before anything starts (exit 2).
fn assert_code(w: &World, table: Option<&str>, o: &std::process::Output, what: &str) {
    if table.is_some_and(|t| t.starts_with("models") || t.starts_with("review")) {
        assert_ne!(code(o), 0, "{what}: {}", stderr(o));
    } else {
        assert_eq!(code(o), 2, "{what}: {}", stderr(o));
        assert_eq!(w.fake.calls_so_far(), 0, "{what}: the binary was called");
        assert!(!w.ledger().exists(), "{what}: the ledger was made");
    }
}

#[test]
fn a_toml_file_of_the_fixture_is_read_whatever_its_extension() {
    let w = World::new(&tag("ext"));
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let v = fixture_settings(&w, 5_000_000);
    let text = super::core::toml_text(&v);
    let o = run_text(&w, "run-settings.json", &text);
    assert_eq!(code(&o), 0, "TOML in a file named .json is read as TOML: {}", stderr(&o));
}

#[test]
fn a_json_settings_file_is_refused_by_its_content_with_the_toml_form_and_the_document_named() {
    let w = World::new(&tag("json"));
    let json_text = serde_json::to_string_pretty(&fixture_settings(&w, 5_000_000)).unwrap();
    for (what, name, text) in [
        ("pretty JSON in a .json file", "settings.json", json_text.clone()),
        ("pretty JSON in a .toml file", "settings.toml", json_text.clone()),
        ("compact JSON", "settings.toml", serde_json::to_string(&fixture_settings(&w, 5_000_000)).unwrap()),
        ("white space before the brace", "settings.toml", format!("\n  \t{json_text}")),
        ("an empty object", "settings.json", "{}".to_owned()),
        ("a cut-off object", "settings.toml", "{\"run\": \"r1\",".to_owned()),
        ("an array", "settings.toml", "[1, 2]".to_owned()),
    ] {
        let o = run_text(&w, name, &text);
        assert_refused(&w, &o, what, &["TOML", "docs/loom-run-settings.md"]);
        assert!(stderr(&o).contains("--settings"), "{what}: the option is not named: {}", stderr(&o));
    }
}

#[test]
fn a_member_in_its_old_place_is_refused_with_the_new_place_named() {
    for (old, new) in [
        ("sha256", "loom_sha256"),
        ("in_process", "in_process"),
        ("confinement", "confinement"),
        ("cap_micro_usd", "cap_micro_usd"),
        ("login_file", "login_file"),
    ] {
        let w = World::new(&tag("old"));
        let mut v = fixture_settings(&w, 5_000_000);
        let top = v.as_object_mut().unwrap();
        top.remove(new);
        top.insert("loom".into(), json!({old: json!("x")}));
        write_settings(&w, &v);
        let o = w.run(&w.args("r1", 5_000_000));
        assert_refused(&w, &o, old, &[&format!("loom.{old}"), &format!("`{new}`")]);
    }
    let w = World::new(&tag("old"));
    let o = run_text(&w, "settings.toml", &member(&base_text(&w), None, "loom", "\"local\""));
    assert_refused(&w, &o, "loom as a value", &["loom"]);
}

#[test]
fn an_unknown_key_is_refused_by_name_at_the_top_and_in_every_table() {
    for (table, name) in [
        (None, "settings.colour"),
        (Some("decider"), "decider.colour"),
        (Some("models.writer"), "settings.models.writer.colour"),
        (Some("efforts.writer"), "settings.efforts.writer.colour"),
        (Some("review"), "settings.review.colour"),
    ] {
        let w = World::new(&tag("unknown"));
        let o = run_text(&w, "settings.toml", &member(&base_text(&w), table, "colour", "\"blue\""));
        assert_code(&w, table, &o, name);
        assert!(stderr(&o).contains(name), "{name} not named: {}", stderr(&o));
    }
    let w = World::new(&tag("unknown"));
    let o = run_text(&w, "settings.toml", &member(&base_text(&w), Some("efforts.poet"), "effort", "\"low\""));
    assert!(stderr(&o).contains("settings.efforts.poet"), "{}", stderr(&o));
    // the replay table is the plan's of --recorded, never a file's
    let o = run_text(&w, "settings.toml", &member(&base_text(&w), Some("replay"), "adapter", "\"/bin/true\""));
    assert_refused(&w, &o, "[replay]", &["settings.replay"]);
}

/// The forms of a value that is not a whole number of at least 0, the ones TOML has and JSON lacks
/// among them: a float, a date, a time, an integer over 64 bits.
const NOT_WHOLE: [(&str, &str); 10] = [
    ("float 1.0", "1.0"),
    ("fraction", "2.5"),
    ("exponent", "1e3"),
    ("text", "\"1\""),
    ("date", "1979-05-27"),
    ("date and time", "1979-05-27T07:32:00Z"),
    ("time", "07:32:00"),
    ("negative", "-1"),
    ("over 64 bits", "18446744073709551616"),
    ("over i64", "9223372036854775808"),
];

#[test]
fn a_member_that_is_a_whole_number_is_refused_as_any_other_form_of_value() {
    // a cap member and the whole-number members of the decider and of a model role
    for (table, key) in [
        (None, "cap_micro_usd"),
        (None, "max_llm_calls"),
        (None, "max_wall_seconds"),
        (Some("decider"), "max_turns"),
        (Some("decider"), "max_decider_calls"),
        (Some("decider"), "retries"),
        (Some("decider"), "timeout_seconds"),
        (Some("models.writer"), "max_turns"),
    ] {
        for (what, raw) in NOT_WHOLE {
            let w = World::new(&tag("whole"));
            let o = run_text(&w, "settings.toml", &member(&base_text(&w), table, key, raw));
            assert_code(&w, table, &o, &format!("{key} = {what}"));
            assert!(stderr(&o).contains(key), "{key} = {what}: the member is not named: {}", stderr(&o));
        }
    }
}

#[test]
fn a_value_of_the_wrong_kind_is_refused_by_name_for_text_numbers_and_paths() {
    for (table, key, raw) in [
        (None, "run", "1979-05-27"),
        (None, "autonomy", "1979-05-27"),
        (None, "adapter", "5"),
        (None, "login_file", "1"),
        (None, "login_file", "1.5"),
        (None, "login_file", "1979-05-27"),
        (None, "login_file", "[\"/a\"]"),
        (None, "confinement", "1"),
        (None, "confinement", "1979-05-27"),
        (None, "loom_sha256", "1979-05-27"),
        (Some("decider"), "model", "1979-05-27"),
        (Some("decider"), "without_claims", "\"true\""),
        (Some("decider"), "max_budget_usd", "\"1\""),
        (Some("decider"), "max_budget_usd", "1979-05-27"),
        (Some("decider"), "max_budget_usd", "0"),
        (Some("models.writer"), "model", "1979-05-27"),
        (Some("models.writer"), "max_budget_usd", "\"1\""),
        (Some("models.writer"), "max_budget_usd", "-1"),
        (Some("models.writer"), "max_budget_usd", "1979-05-27"),
    ] {
        let w = World::new(&tag("kind"));
        let o = run_text(&w, "settings.toml", &member(&base_text(&w), table, key, raw));
        if key == "adapter" {
            // the adapter is read when the bindings copy is made, after the run directory exists
            assert_ne!(code(&o), 0, "{key} = {raw}: {}", stderr(&o));
        } else {
            assert_code(&w, table, &o, &format!("{key} = {raw}"));
        }
        assert!(stderr(&o).contains(key), "{key} = {raw}: the member is not named: {}", stderr(&o));
    }
}

#[test]
fn the_confinement_of_the_file_never_lowers_the_option_and_the_option_has_no_default() {
    // a level the option does not name, lower or higher, is a refusal, not a choice
    for level in ["none", "substrate", "local", "linux"] {
        if level == "local" {
            continue;
        }
        let w = World::new(&tag("level"));
        let o = run_text(&w, "settings.toml", &member(&base_text(&w), None, "confinement", &format!("\"{level}\"")));
        assert_refused(&w, &o, level, &["confinement is"]);
    }
    // the file alone does not give the level: the option is required
    let w = World::new(&tag("level"));
    let mut args = w.args("r1", 5_000_000);
    let i = args.iter().position(|a| a == "--confinement").unwrap();
    args.drain(i..=i + 1);
    let o = w.run(&args);
    assert_eq!(code(&o), 64, "{}", stderr(&o));
    assert!(stderr(&o).contains("--confinement"), "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
}

#[test]
fn a_login_file_is_a_path_and_its_content_never_a_setting() {
    let w = World::new(&tag("login"));
    let marker = "{\"token\": \"LOGIN-CONTENT-IN-THE-SETTINGS-1234\"}";
    let o = run_text(&w, "settings.toml", &member(&base_text(&w), None, "login_file", &format!("'{marker}'")));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("login_file"), "{}", stderr(&o));
    assert!(!stderr(&o).contains("LOGIN-CONTENT-IN-THE-SETTINGS-1234"), "the refusal prints the value: {}", stderr(&o));
    let o = run_text(&w, "settings.toml", &member(&base_text(&w), None, "login_file", "\"relative/login.json\""));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("login_file"), "{}", stderr(&o));
}

#[test]
fn the_login_path_is_replaced_in_the_record_and_the_record_stays_json() {
    let w = World::new(&tag("record"));
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let v = fixture_settings(&w, 5_000_000);
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let text = std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap();
    assert!(text.starts_with('{'), "{text}");
    let recorded: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(recorded["login_file"], "named");
    assert!(!text.contains(&w.login_path().display().to_string()), "the login path is in the record: {text}");
    assert_eq!(recorded["loom"]["confinement"], "local");
    assert_eq!(recorded["loom"]["cap_micro_usd"], 5_000_000);
    assert!(recorded.get("loom_sha256").is_none() && recorded.get("confinement").is_none(), "a TOML place is in the record: {text}");
    let first = lines(&w.run_dir("r1/run-settings.jsonl")).remove(0);
    assert_eq!(first["format"], "sdc.run-settings/1");
    assert_eq!(first["loom"]["confinement"], "local");
    assert!(first.get("loom_sha256").is_none(), "{first}");
}
