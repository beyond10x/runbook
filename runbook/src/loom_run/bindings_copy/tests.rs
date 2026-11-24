//! Tests of the bindings copy against the committed `bindings/` of this repository.

use super::*;
use crate::assets::testdir::Dir;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings")
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    copy_dirs(&[(src, dst)], Links::Keep)
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "yaml")).collect();
    files.sort();
    files
}

fn load(file: &Path) -> Yaml {
    serde_yaml::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn actions(dir: &Path) -> Vec<(String, Yaml)> {
    let mut out = Vec::new();
    for file in yaml_files(dir) {
        let doc = load(&file);
        for action in doc["actions"].as_sequence().unwrap() {
            out.push((format!("{}:{}", file.file_name().unwrap().to_string_lossy(), action["action"].as_str().unwrap()), action.clone()));
        }
    }
    out
}

fn model_settings() -> Value {
    json!({"autonomy": "A5", "decider": {"kind": "model", "model": "claude-opus-5-5", "reply_bin": "y"}})
}

fn argv_of(action: &Yaml) -> Vec<String> {
    action["executor"]["argv"].as_sequence().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect()
}

fn option_of(argv: &[String], name: &str) -> Option<String> {
    let stub = argv.iter().position(|a| a == "--stub").unwrap_or(argv.len());
    (2..stub).step_by(2).find(|i| argv[*i] == name).map(|i| argv[i + 1].clone())
}

fn make(tag: &str, settings: &Value) -> (Dir, PathBuf, Result<PathBuf, String>) {
    let dir = Dir::new(tag);
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let result = materialise(&committed(), &dst, "sdc", settings, None);
    (dir, dst, result)
}

/// The action as the copy holds it: the tools directory of the copy handed to a model node.
fn with_tools_dir(was: &Yaml, dst: &Path) -> Yaml {
    let mut is = was.clone();
    if is["executor"]["kind"].as_str() == Some("program") && argv_of(was)[0].ends_with("sdc-node") {
        let mut argv = argv_of(was);
        argv.insert(2, dst.join("tools").display().to_string());
        argv.insert(2, "--tools-dir".to_owned());
        is["executor"]["argv"] = Yaml::Sequence(argv.into_iter().map(Yaml::String).collect());
    }
    is
}

/// The copy is the committed bindings but for the tools directory of the model nodes: every file
/// that holds no binding is the same bytes.
fn assert_only_the_tools_dir_differs(dst: &Path) {
    let (was, now) = (digest_tree(&committed()), digest_tree(dst));
    assert_eq!(was.len(), now.len());
    for ((a, x), (b, y)) in was.iter().zip(&now) {
        assert_eq!(a, b);
        assert!(a.ends_with(".yaml") && !a.contains('/') || x == y, "{a} differs");
    }
    for ((name, was), (_, is)) in actions(&committed()).iter().zip(&actions(dst)) {
        assert_eq!(&with_tools_dir(was, dst), is, "{name}");
    }
}

fn digest_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.strip_prefix(dir).unwrap().display().to_string(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn no_settings_copy_every_file_but_the_tools_dir_of_the_model_nodes_and_keep_the_68_bindings() {
    let (dir, dst, root) = make("r2-copy-plain", &json!({}));
    assert_eq!(root.unwrap(), dst.join("sdc.yaml"));
    assert_only_the_tools_dir_differs(&dst);
    assert_eq!(actions(&dst).len(), 68);
    assert_eq!(actions(&committed()).len(), 68);
    assert_eq!(digest_tree(&dir.path().join("run/instructions")), digest_tree(&committed().parent().unwrap().join("instructions")));
}

#[test]
fn rewriting_keeps_all_68_bindings_and_changes_only_the_rewritten_members() {
    let settings = json!({
        "autonomy": "A5",
        "decider": {"kind": "model", "model": "claude-opus-5-5", "reply_bin": "y"},
        "adapter": "metaharness",
        "models": {
            "writer": {"model": "claude-sonnet-5-5", "max_turns": 7, "max_budget_usd": 1.5},
            "implement": {"model": "claude-sonnet-5-5"}, "review": {"model": "claude-sonnet-5-5"}, "other": {"model": "claude-sonnet-5-5"},
        },
    });
    let (_dir, dst, root) = make("r2-copy-rewrite", &settings);
    root.unwrap();
    let before = actions(&committed());
    let after = actions(&dst);
    assert_eq!(before.len(), 68);
    assert_eq!(after.len(), 68);
    let (mut people, mut nodes) = (0, 0);
    for ((name, was), (name_after, is)) in before.iter().zip(&after) {
        assert_eq!(name, name_after);
        let mut expect = was.clone();
        let kind = was["executor"]["kind"].as_str().unwrap();
        if kind == "person" {
            people += 1;
            expect["executor"]["principal"] = "decider:model".into();
            for e in expect.get_mut("evidence").and_then(Yaml::as_sequence_mut).into_iter().flatten() {
                e["producer"] = "decider:model".into();
            }
        }
        if kind == "program" && argv_of(was)[0].ends_with("sdc-node") {
            nodes += 1;
            let action = name.split(':').nth(1).unwrap();
            let writer = ["spec.author", "plan.decompose"].contains(&action);
            let mut argv = argv_of(was);
            let replace = |argv: &mut Vec<String>, option: &str, value: &str| {
                let i = (2..argv.iter().position(|a| a == "--stub").unwrap()).step_by(2).find(|i| argv[*i] == option);
                match i {
                    Some(i) => argv[i + 1] = value.to_owned(),
                    None => {
                        argv.insert(2, value.to_owned());
                        argv.insert(2, option.to_owned());
                    }
                }
            };
            replace(&mut argv, "--tools-dir", &dst.join("tools").display().to_string());
            replace(&mut argv, "--model", "claude-sonnet-5-5");
            if writer {
                replace(&mut argv, "--max-turns", "7");
                replace(&mut argv, "--max-budget-usd", "1.5");
            }
            replace(&mut argv, "--adapter", "metaharness");
            expect["executor"]["argv"] = Yaml::Sequence(argv.into_iter().map(Yaml::String).collect());
        }
        assert_eq!(&expect, is, "{name}: something other than the rewritten members changed");
    }
    assert_eq!(people, 5, "every person step: the answers file declares scope.answer");
    assert_eq!(nodes, 14, "{nodes} model nodes");
}

#[test]
fn the_principal_of_person_steps_is_rewritten_in_executor_and_evidence_and_scope_answer_follows_the_answers_file() {
    let (_dir, dst, root) = make("r2-copy-principal", &model_settings());
    root.unwrap();
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() != Some("person") {
            continue;
        }
        assert_eq!(action["executor"]["principal"].as_str(), Some("decider:model"), "{name}: declared in tools/answers.yaml or a recorded result");
        for e in action["evidence"].as_sequence().into_iter().flatten() {
            assert_eq!(e["producer"].as_str(), Some("decider:model"), "{name}");
        }
    }
    // Without the declaration the step whose outcome is a person's text keeps its person; the
    // steps that record a result do not need one.
    let dir = Dir::new("r2-copy-undeclared");
    let src = dir.path().join("src/bindings");
    copy_dir(&committed(), &src).unwrap();
    std::fs::remove_file(src.join(crate::loom_run::answers::FILE)).unwrap();
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    materialise(&src, &dst, "sdc", &model_settings(), None).unwrap();
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() != Some("person") {
            continue;
        }
        let expect = if name.ends_with(":scope.answer") { "person:owner" } else { "decider:model" };
        assert_eq!(action["executor"]["principal"].as_str(), Some(expect), "{name}");
    }
    for autonomy in ["A0", "A1"] {
        let mut s = model_settings();
        s["autonomy"] = json!(autonomy);
        let (_d, d, r) = make("r2-copy-reserved", &s);
        r.unwrap();
        assert_only_the_tools_dir_differs(&d);
    }
}

#[test]
fn below_a5_only_the_person_steps_the_table_gives_the_decider_get_its_principal_and_no_table_leaves_every_step_a_person() {
    let table = decider::Routing::load(committed().parent().unwrap()).unwrap();
    for level in ["A2", "A3", "A4"] {
        let mut s = model_settings();
        s["autonomy"] = json!(level);
        let (_d, dst, r) = make("r2-copy-routed", &s);
        r.unwrap();
        let (mut taken, mut kept) = (0, 0);
        for (name, action) in actions(&dst) {
            if action["executor"]["kind"].as_str() != Some("person") {
                continue;
            }
            let short = name.split_once(':').unwrap().1;
            let got = action["executor"]["principal"].as_str().unwrap();
            if got == "decider:model" {
                assert!(table.decider_takes(level, "approval", short), "{level} {name}: the table does not give it to the decider");
                taken += 1;
            } else {
                assert!(!got.starts_with("decider:") && !got.starts_with("rule:"), "{level} {name}: {got}");
                kept += 1;
            }
            if short == "release.publish" || short == "change.merge" {
                assert_ne!(got, "decider:model", "{level} {name}: a person's below A5");
            }
        }
        assert!(taken > 0 && kept > 0, "{level}: {taken} taken, {kept} kept");
    }
    // The table is read beside the committed bindings; a source whose root has none is a person's.
    let dir = Dir::new("r2-copy-no-table");
    let src = dir.path().join("src/bindings");
    copy_dir(&committed(), &src).unwrap();
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let mut s = model_settings();
    s["autonomy"] = json!("A4");
    materialise(&src, &dst, "sdc", &s, None).unwrap();
    assert_only_the_tools_dir_differs(&dst);
}

#[test]
fn models_turns_budgets_and_adapter_land_on_the_model_nodes_of_their_role() {
    let settings = json!({
        "adapter": "claude-cli",
        "models": {"review": {"model": "m-review", "max_turns": 9}, "other": {"max_budget_usd": 0.25}},
    });
    let (_dir, dst, root) = make("r2-copy-roles", &settings);
    root.unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() != Some("program") || !argv_of(&action)[0].ends_with("sdc-node") {
            continue;
        }
        let argv = argv_of(&action);
        let node = name.split(':').nth(1).unwrap().to_owned();
        assert_eq!(option_of(&argv, "--adapter").as_deref(), Some("claude-cli"), "{name}");
        if ["spec.review", "change.review"].contains(&node.as_str()) {
            assert_eq!(option_of(&argv, "--model").as_deref(), Some("m-review"), "{name}");
            assert_eq!(option_of(&argv, "--max-turns").as_deref(), Some("9"), "{name}");
            seen.insert("review");
        } else {
            assert_ne!(option_of(&argv, "--model").as_deref(), Some("m-review"), "{name}");
        }
        if ["idea.dissect", "questions.write", "vision.write"].contains(&node.as_str()) {
            assert_eq!(option_of(&argv, "--max-budget-usd").as_deref(), Some("0.25"), "{name}");
            seen.insert("other");
        }
        assert_eq!(argv.iter().position(|a| a == "--stub"), argv.iter().rposition(|a| a == "--stub"));
    }
    assert_eq!(seen.len(), 2);
}

#[test]
fn an_option_that_exists_is_replaced_and_one_that_does_not_is_inserted_before_the_stub() {
    let mut argv: Vec<Yaml> = ["tools/sdc-node", "n", "--model", "a", "--stub", "stub-x", "y"].iter().map(|s| Yaml::String((*s).to_owned())).collect();
    assert!(set_option(&mut argv, "--model", "b"));
    assert!(!set_option(&mut argv, "--model", "b"), "an unchanged value reports no change");
    assert!(set_option(&mut argv, "--adapter", "claude-cli"));
    let words: Vec<&str> = argv.iter().map(|a| a.as_str().unwrap()).collect();
    assert_eq!(words, ["tools/sdc-node", "n", "--adapter", "claude-cli", "--model", "b", "--stub", "stub-x", "y"]);
    let mut bare: Vec<Yaml> = ["tools/sdc-node", "n"].iter().map(|s| Yaml::String((*s).to_owned())).collect();
    assert!(set_option(&mut bare, "--model", "m"));
    assert_eq!(bare.len(), 4);
}

#[test]
fn a_login_file_is_named_in_the_copy_only_by_the_link_it_is_given_and_never_in_a_result() {
    let dir = Dir::new("r2-copy-login");
    let login = dir.path().join("private-login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let secret = login.display().to_string();
    let settings = json!({"login_file": secret, "adapter": "claude-cli"});
    let link = dir.path().join("private/login");
    let root = materialise(&committed(), &dst, "sdc", &settings, Some(&link)).unwrap();
    assert!(!root.display().to_string().contains(&secret));
    assert!(std::fs::symlink_metadata(dir.path().join("run/login")).is_err(), "the copy makes no link of its own");
    let without_link = materialise(&committed(), &dir.path().join("run4/bindings"), "sdc", &settings, None).unwrap_err();
    assert!(!without_link.contains(&secret) && without_link.contains("link"), "{without_link}");
    let mut with_option = 0;
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("sdc-node") {
            assert_eq!(option_of(&argv_of(&action), "--claude-credentials"), Some(link.display().to_string()), "{name}");
            with_option += 1;
        }
    }
    assert_eq!(with_option, 14);
    for (name, bytes) in digest_tree(dir.path().join("run").as_path()) {
        assert!(!String::from_utf8_lossy(&bytes).contains(&secret), "{name} holds the login path");
    }
    // without a login file there is no option and no link
    let (dir2, dst2, root2) = make("r2-copy-nologin", &json!({"adapter": "claude-cli"}));
    root2.unwrap();
    assert!(!dir2.path().join("run/login").exists());
    for (_, action) in actions(&dst2) {
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("sdc-node") {
            assert_eq!(option_of(&argv_of(&action), "--claude-credentials"), None);
        }
    }
    // a login file that is not there is refused without its path
    let gone = materialise(&committed(), &dir.path().join("run2/bindings"), "sdc", &json!({"login_file": dir.path().join("nope/x.json").display().to_string()}), None).unwrap_err();
    assert!(!gone.contains("nope"), "{gone}");
    let relative = materialise(&committed(), &dir.path().join("run3/bindings"), "sdc", &json!({"login_file": "x.json"}), None).unwrap_err();
    assert!(!relative.contains("x.json"), "{relative}");
}

#[test]
fn an_effort_lands_on_the_model_nodes_of_its_class_and_on_no_other_node() {
    let settings = json!({"efforts": {"review": {"effort": "max"}, "other": {"effort": "low"}}});
    let (_dir, dst, root) = make("r2-copy-efforts", &settings);
    root.unwrap();
    let classes: Value = serde_json::from_str(&std::fs::read_to_string(dst.join("tools/node-classes.json")).unwrap()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for (name, action) in actions(&dst) {
        let argv = argv_of_any(&action);
        if action["executor"]["kind"].as_str() != Some("program") || !argv[0].ends_with("sdc-node") {
            assert!(!argv.iter().any(|a| a == "--effort"), "{name}: a node that is no model node was given an effort");
            continue;
        }
        let node = name.split(':').nth(1).unwrap();
        let want = match class_of(&classes, node).as_str() {
            "review" => Some("max"),
            "other" => Some("low"),
            _ => None,
        };
        assert_eq!(option_of(&argv, "--effort").as_deref(), want, "{name}");
        seen.extend(want);
    }
    assert_eq!(seen.len(), 2);
}

/// The argv of a program action, empty for another kind.
fn argv_of_any(action: &Yaml) -> Vec<String> {
    action["executor"]["argv"].as_sequence().map(|s| s.iter().filter_map(|a| a.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

#[test]
fn unknown_settings_are_refused_by_name_before_anything_is_written() {
    for (settings, name) in [
        (json!({"colour": "blue"}), "settings.colour"),
        (json!({"models": {"writer": {"temperature": 1}}}), "settings.models.writer.temperature"),
        (json!({"models": {"poet": {"model": "m"}}}), "settings.models.poet"),
        (json!({"review": {"mode": "facts"}}), "settings.review.mode"),
        (json!({"adapter": "carrier-pigeon"}), "settings.adapter"),
        (json!({"models": {"writer": {"max_turns": 0}}}), "settings.models.writer.max_turns"),
        (json!({"models": {"writer": {"max_budget_usd": -1}}}), "settings.models.writer.max_budget_usd"),
        (json!({"models": {"writer": {"model": "two words"}}}), "settings.models.writer.model"),
        (json!({"efforts": {"writer": {"effort": "bogus"}}}), "settings.efforts.writer.effort"),
        (json!({"efforts": {"poet": {"effort": "low"}}}), "settings.efforts.poet"),
        (json!({"efforts": {"review": {"model": "m"}}}), "settings.efforts.review.model"),
    ] {
        let (dir, dst, result) = make("r2-copy-unknown", &settings);
        let error = result.unwrap_err();
        assert!(error.contains(name), "{settings}: {error}");
        assert!(!dst.exists(), "{settings}: the copy was started");
        assert!(!dir.path().join("run/instructions").exists());
    }
}

#[test]
fn review_adapter_other_than_claude_cli_is_not_expressible_and_says_why() {
    let (_d, _p, ok) = make("r2-copy-review-ok", &json!({"review": {"adapter": "claude-cli"}}));
    ok.unwrap();
    let (_d, _p, err) = make("r2-copy-review-no", &json!({"review": {"adapter": "metaharness"}}));
    assert!(err.unwrap_err().contains("only claude-cli is expressible under Loom"));
}

#[test]
fn the_committed_bindings_are_never_written_and_a_read_only_source_is_copied() {
    let dir = Dir::new("r2-copy-readonly");
    let src = dir.path().join("src/bindings");
    copy_dir(&committed(), &src).unwrap();
    copy_dir(&committed().parent().unwrap().join("instructions"), &dir.path().join("src/instructions")).unwrap();
    let frozen = digest_tree(&src);
    let mut stack = vec![src.clone()];
    let mut dirs = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p.clone());
                dirs.push(p);
            } else {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o444)).unwrap();
            }
        }
    }
    dirs.push(src.clone());
    for d in &dirs {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let mut settings = model_settings();
    settings["adapter"] = json!("claude-cli");
    let root = materialise(&src, &dst, "sdc", &settings, None);
    for d in &dirs {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    root.unwrap();
    assert_eq!(digest_tree(&src), frozen, "the source changed");
    assert_eq!(actions(&dst).len(), 68);
    assert!(std::fs::metadata(dst.join("sdc.yaml")).unwrap().permissions().mode() & 0o200 != 0, "the copy is the run's to write");
}

#[test]
fn an_existing_destination_is_refused_and_the_source_untouched() {
    let (dir, _dst, _r) = make("r2-copy-same", &json!({}));
    let before = digest_tree(&committed());
    let result = materialise(&committed(), &dir.path().join("run/bindings"), "sdc", &json!({}), None);
    assert!(result.unwrap_err().ends_with(": exists"));
    assert_eq!(digest_tree(&committed()), before);
}

#[test]
fn a_missing_root_file_is_refused_and_the_instructions_come_along() {
    let dir = Dir::new("r2-copy-root");
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let error = materialise(&committed(), &dst, "nonexistent", &json!({}), None).unwrap_err();
    assert!(error.contains("the root bindings file is missing"));
    assert!(dir.path().join("run/instructions/decider").is_dir());
}

#[test]
fn the_fixtures_of_the_driver_come_along_and_a_link_in_them_stays_a_link() {
    let dir = Dir::new("r2-copy-links");
    let checkout = dir.path().join("checkout");
    std::fs::create_dir_all(checkout.join("bindings")).unwrap();
    std::fs::write(checkout.join("bindings/sdc.yaml"), "actions: []\n").unwrap();
    std::fs::create_dir_all(checkout.join("driver/examples/sdc/fixtures/real-dir")).unwrap();
    std::fs::write(checkout.join("driver/examples/sdc/fixtures/real-dir/a.txt"), "a\n").unwrap();
    std::fs::write(checkout.join("driver/examples/sdc/rule-ends.json"), "{}\n").unwrap();
    std::os::unix::fs::symlink("real-dir", checkout.join("driver/examples/sdc/fixtures/dir-link")).unwrap();
    std::os::unix::fs::symlink("nowhere", checkout.join("driver/examples/sdc/fixtures/broken-link")).unwrap();
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    materialise(&checkout.join("bindings"), &dst, "sdc", &json!({}), None).unwrap();
    let fixtures = dir.path().join("run/driver/examples/sdc/fixtures");
    assert!(dir.path().join("run/driver/examples/sdc/rule-ends.json").is_file());
    assert_eq!(std::fs::read_link(fixtures.join("dir-link")).unwrap(), Path::new("real-dir"));
    assert_eq!(std::fs::read_link(fixtures.join("broken-link")).unwrap(), Path::new("nowhere"));
    assert_eq!(std::fs::read_to_string(fixtures.join("dir-link/a.txt")).unwrap(), "a\n");
}

/// Slow lane: `cargo test --locked --bin runbook -- --ignored loads_in_loom` with
/// `RUNBOOK_LOOM_BIN` naming the pinned `b10x-loom`. The committed tests never need that binary.
#[test]
#[ignore = "slow lane: needs b10x-loom at RUNBOOK_LOOM_BIN"]
fn the_rewritten_copy_loads_in_loom_tree_open() {
    let Some(loom) = std::env::var_os("RUNBOOK_LOOM_BIN") else {
        panic!("RUNBOOK_LOOM_BIN names no b10x-loom");
    };
    let dir = Dir::new("r2-copy-loom");
    let home = dir.dir("home");
    let repo = committed().parent().unwrap().to_path_buf();
    let run = |args: &[String]| {
        let out = std::process::Command::new(&loom)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    for entry in std::fs::read_dir(repo.join("protocol")).unwrap().filter_map(Result::ok) {
        let file = entry.path();
        if !file.to_string_lossy().ends_with(".protocol.yaml") {
            continue;
        }
        let doc = load(&file);
        let id = doc["protocol"]["id"].as_str().unwrap().replace('.', "-");
        let revision = doc["protocol"]["revision"].as_u64().unwrap();
        run(&["protocols".into(), "add".into(), format!("{id}@{revision}"), "--file".into(), file.display().to_string()]);
    }
    let settings = json!({
        "autonomy": "A5",
        "decider": {"kind": "model", "model": "claude-opus-5-5", "reply_bin": "y"},
        "adapter": "claude-cli",
        "models": {"writer": {"model": "claude-sonnet-5-5", "max_turns": 7, "max_budget_usd": 1.5}},
    });
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let root = materialise(&committed(), &dst, "sdc", &settings, None).unwrap();
    let work = dir.dir("run/work");
    std::fs::copy(repo.join("driver/examples/sdc/fixtures/idea.yaml"), work.join("idea.yaml")).unwrap();
    let store = dir.path().join("run/store");
    let revision = load(&repo.join("protocol/sdc.protocol.yaml"))["protocol"]["revision"].as_u64().unwrap();
    let opened = run(&[
        "tree".into(), "open".into(), "--protocol".into(), format!("sdc@{revision}"), "--at".into(), "2026-11-20T12:00:00Z".into(),
        "--store".into(), store.display().to_string(), "--bindings".into(), root.display().to_string(),
        "--workspace".into(), work.display().to_string(), "--case".into(), "IDEA-1".into(),
    ]);
    assert!(opened.contains("IDEA-1"), "{opened}");
    run(&["tree".into(), "verify".into(), "--store".into(), store.display().to_string()]);
}

#[test]
fn an_effort_and_a_replay_plan_land_on_the_same_nodes_and_the_effort_changes_nothing_else_in_argv() {
    let replay = json!({"adapter": "/x/sdc-replay", "plan": "/x/run/replay/plan.json", "nodes": ["change.implement", "idea.dissect", "change.review"], "actions": {}});
    let (_a, was_dst, was) = make("adv-seam-replay-only", &json!({"replay": replay}));
    was.unwrap();
    let efforts = json!({"review": {"effort": "max"}, "other": {"effort": "low"}, "implement": {"effort": "xhigh"}});
    let (_b, is_dst, is) = make("adv-seam-both", &json!({"efforts": efforts, "replay": replay}));
    is.unwrap();
    let classes: Value = serde_json::from_str(&std::fs::read_to_string(is_dst.join("tools/node-classes.json")).unwrap()).unwrap();
    let (was_actions, is_actions) = (actions(&was_dst), actions(&is_dst));
    assert_eq!(was_actions.len(), is_actions.len());
    let (mut with_effort, mut stubbed) = (0, 0);
    for ((name, was), (_, is)) in was_actions.iter().zip(&is_actions) {
        let (mut was, mut is) = (argv_of_any(was), argv_of_any(is));
        let own = |argv: &mut Vec<String>, dst: &Path| argv.iter_mut().for_each(|a| *a = a.replace(&dst.display().to_string(), "<copy>"));
        own(&mut was, &was_dst);
        own(&mut is, &is_dst);
        if !is.first().is_some_and(|t| t.ends_with("sdc-node")) {
            assert_eq!(was, is, "{name}: a node that is no model node changed");
            continue;
        }
        let node = name.split(':').nth(1).unwrap();
        let want = match class_of(&classes, node).as_str() {
            "review" => Some("max"),
            "other" => Some("low"),
            "implement" => Some("xhigh"),
            _ => None,
        };
        assert_eq!(option_of(&is, "--effort").as_deref(), want, "{name}");
        if let Some(i) = is.iter().position(|a| a == "--effort") {
            is.drain(i..i + 2);
            with_effort += 1;
        }
        assert_eq!(was, is, "{name}: the effort changed something else in the argv");
        let adapter = option_of(&was, "--adapter");
        if adapter.as_deref() == Some("stub") {
            stubbed += 1;
        } else {
            assert_eq!(adapter.as_deref(), Some("/x/sdc-replay"), "{name}");
        }
    }
    assert!(with_effort >= 5 && stubbed >= 1, "{with_effort} nodes with an effort, {stubbed} on the stub");
}

#[test]
fn the_levels_the_copy_accepts_are_the_levels_node_classes_json_lists() {
    let classes: Value = serde_json::from_str(&std::fs::read_to_string(committed().join("tools/node-classes.json")).unwrap()).unwrap();
    let listed: Vec<&str> = classes["efforts"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    for level in &listed {
        efforts(&json!({"efforts": {"writer": {"effort": level}}})).unwrap_or_else(|e| panic!("{level} is listed in node-classes.json and refused: {e}"));
    }
    assert_eq!(EFFORTS.to_vec(), listed);
}

#[test]
fn a_gateway_file_is_named_in_every_model_node_of_the_copy_and_the_token_is_in_no_file_of_it() {
    const TOKEN: &str = "tok-copy-3b9d70e4";
    let dir = Dir::new("copy-gateway");
    let file = dir.path().join("private/gateway");
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    // the settings the copy is given hold the text `named`; a table with the token that slipped in is not copied anywhere
    let settings = json!({"gateway": {"base_url": "http://127.0.0.1:8080", "token": TOKEN}, "adapter": "claude-cli"});
    materialise_with(&committed(), &dst, "sdc", &settings, None, Some(&file)).unwrap();
    let mut with_option = 0;
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("sdc-node") {
            let argv = argv_of(&action);
            assert_eq!(option_of(&argv, "--gateway-file"), Some(file.display().to_string()), "{name}");
            assert_eq!(option_of(&argv, "--claude-credentials"), None, "{name}");
            with_option += 1;
        }
    }
    assert_eq!(with_option, 14);
    for (name, bytes) in digest_tree(dir.path().join("run").as_path()) {
        assert!(!String::from_utf8_lossy(&bytes).contains(TOKEN), "{name} holds the token");
    }
    // without a gateway file there is no option
    let (_dir, dst, root) = make("copy-no-gateway", &json!({"adapter": "claude-cli"}));
    root.unwrap();
    for (_, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("sdc-node") {
            assert_eq!(option_of(&argv_of(&action), "--gateway-file"), None);
        }
    }
}

#[test]
fn under_a_gateway_the_model_of_each_role_is_written_into_every_model_node_of_the_role_the_review_step_among_them() {
    // a gateway that routes by model name: the four roles name its route, and every model node carries it
    let dir = Dir::new("copy-gateway-model");
    let file = dir.path().join("private/gateway");
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let models: serde_json::Map<String, Value> = ROLES.iter().map(|role| ((*role).to_owned(), json!({"model": "qwen"}))).collect();
    let settings = json!({"gateway": "named", "adapter": "claude-cli", "models": models});
    materialise_with(&committed(), &dst, "sdc", &settings, None, Some(&file)).unwrap();
    let (mut nodes, mut review) = (0, 0);
    for (name, action) in actions(&dst) {
        let argv = argv_of_any(&action);
        if argv.first().is_some_and(|tool| tool.ends_with("sdc-node")) {
            assert_eq!(option_of(&argv, "--model").as_deref(), Some("qwen"), "{name}");
            assert_eq!(option_of(&argv, "--gateway-file"), Some(file.display().to_string()), "{name}");
            nodes += 1;
            review += usize::from(name.ends_with(":change.review"));
        }
    }
    assert_eq!(nodes, 14);
    // the review step is the model node change.review (spec.review is a check of sdc-tool.sh, no model)
    assert_eq!(review, 1, "the review step is among them");
}

#[test]
fn a_gateway_file_and_a_login_together_are_refused_by_name_and_without_a_path() {
    let dir = Dir::new("copy-gateway-login");
    let login = dir.path().join("private-login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    let secret = login.display().to_string();
    let link = dir.path().join("private/login");
    let file = dir.path().join("private/gateway");
    let e = materialise_with(&committed(), &dir.path().join("a/bindings"), "sdc", &json!({"login_file": secret}), Some(&link), Some(&file)).unwrap_err();
    assert!(e.contains("gateway") && e.contains("login_file") && !e.contains(&secret), "{e}");
    let e = materialise_with(&committed(), &dir.path().join("b/bindings"), "sdc", &json!({"login_file": secret, "gateway": "named"}), None, Some(&file)).unwrap_err();
    assert!(e.contains("gateway") && e.contains("login_file") && !e.contains(&secret), "{e}");
    assert!(!dir.path().join("a").exists() && !dir.path().join("b").exists(), "a copy was made");
}

#[test]
fn the_committed_bindings_hold_a_model_node_and_a_copy_of_them_still_does() {
    let why = model_node_in(&committed()).expect("the committed bindings name sdc-node");
    assert!(why.contains(MODEL_TOOL), "{why}");
    let (_dir, dst, copy) = make("model-node-in-copy", &json!({}));
    copy.unwrap();
    assert!(model_node_in(&dst).is_some());
}

#[test]
fn model_node_in_counts_what_names_the_tool_and_what_it_cannot_read_and_nothing_else() {
    let write = |tag: &str, text: &str| {
        let dir = Dir::new(tag);
        std::fs::write(dir.path().join("x.yaml"), text).unwrap();
        dir
    };
    let program = |argv: &str| format!("actions:\n  - action: a.b\n    executor: {{kind: program, argv: {argv}}}\n");
    for (what, text) in [
        ("tool as first word", program("[sdc-node, x]")),
        ("tool by path", program("[/opt/bin/sdc-node, x]")),
        ("tool in a shell word", program("[sh, -c, 'sdc-node x']")),
        ("argv not a list", program("sdc-node")),
        ("argv with a number", program("[tools/t.sh, 7]")),
        ("empty argv", program("[]")),
        ("unknown kind", "actions:\n  - action: a.b\n    executor: {kind: agent}\n".to_owned()),
        ("no executor", "actions:\n  - action: a.b\n".to_owned()),
        ("actions not a list", "actions: {a: b}\n".to_owned()),
        ("not yaml", "a: [unclosed\n".to_owned()),
    ] {
        let dir = write("model-node-in-yes", &text);
        assert!(model_node_in(dir.path()).is_some(), "{what}");
    }
    for (what, text) in [
        ("tool script", program("[tools/sdc-tool.sh, check-idea]")),
        ("echo", program("[sh, -c, 'echo hi']")),
        ("person", "actions:\n  - action: a.b\n    executor: {kind: person, principal: person:owner}\n".to_owned()),
        ("no actions", "format: loom.bindings/1\n".to_owned()),
    ] {
        let dir = write("model-node-in-no", &text);
        assert_eq!(model_node_in(dir.path()), None, "{what}");
    }
    let dir = Dir::new("model-node-in-unreadable");
    assert!(model_node_in(&dir.path().join("absent")).is_some(), "a directory that cannot be read counts");
}
