//! Tests of the bindings copy against the committed `bindings/` of this repository.

use super::*;
use crate::assets::testdir::Dir;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings")
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
    json!({"autonomy": "A5", "decider": {"kind": "model", "model": "claude-opus-5-5", "program": "x", "reply_bin": "y"}})
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
    let result = materialise(&committed(), &dst, "sdc", settings);
    (dir, dst, result)
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
fn no_settings_copy_every_file_byte_for_byte_and_keep_the_66_bindings() {
    let (dir, dst, root) = make("r2-copy-plain", &json!({}));
    assert_eq!(root.unwrap(), dst.join("sdc.yaml"));
    assert_eq!(digest_tree(&dst), digest_tree(&committed()));
    assert_eq!(actions(&dst).len(), 66);
    assert_eq!(actions(&committed()).len(), 66);
    assert_eq!(digest_tree(&dir.path().join("run/instructions")), digest_tree(&committed().parent().unwrap().join("instructions")));
}

#[test]
fn rewriting_keeps_all_66_bindings_and_changes_only_the_rewritten_members() {
    let settings = json!({
        "autonomy": "A5",
        "decider": {"kind": "model", "model": "claude-opus-5-5", "program": "x", "reply_bin": "y"},
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
    assert_eq!(before.len(), 66);
    assert_eq!(after.len(), 66);
    let (mut people, mut nodes) = (0, 0);
    for ((name, was), (name_after, is)) in before.iter().zip(&after) {
        assert_eq!(name, name_after);
        let mut expect = was.clone();
        let kind = was["executor"]["kind"].as_str().unwrap();
        if kind == "person" && !name.ends_with(":scope.answer") {
            people += 1;
            expect["executor"]["principal"] = "decider:model".into();
            for e in expect["evidence"].as_sequence_mut().unwrap() {
                e["producer"] = "decider:model".into();
            }
        }
        if kind == "program" && argv_of(was)[0].ends_with("llm-node.sh") {
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
    assert_eq!(people, 4, "the person steps but scope.answer");
    assert_eq!(nodes, 14, "{nodes} model nodes");
}

#[test]
fn the_principal_of_person_steps_is_rewritten_in_executor_and_evidence_and_scope_answer_keeps_its_person() {
    let (_dir, dst, root) = make("r2-copy-principal", &model_settings());
    root.unwrap();
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() != Some("person") {
            continue;
        }
        let expect = if name.ends_with(":scope.answer") { "person:owner" } else { "decider:model" };
        assert_eq!(action["executor"]["principal"].as_str(), Some(expect), "{name}");
        for e in action["evidence"].as_sequence().into_iter().flatten() {
            assert_eq!(e["producer"].as_str(), Some(expect), "{name}");
        }
    }
    for autonomy in ["A0", "A3", "A4"] {
        let mut s = model_settings();
        s["autonomy"] = json!(autonomy);
        let (_d, d, r) = make("r2-copy-reserved", &s);
        r.unwrap();
        assert_eq!(digest_tree(&d), digest_tree(&committed()), "{autonomy}: the steps keep their person");
    }
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
        if action["executor"]["kind"].as_str() != Some("program") || !argv_of(&action)[0].ends_with("llm-node.sh") {
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
    let mut argv: Vec<Yaml> = ["tools/llm-node.sh", "n", "--model", "a", "--stub", "stub-x", "y"].iter().map(|s| Yaml::String((*s).to_owned())).collect();
    assert!(set_option(&mut argv, "--model", "b"));
    assert!(!set_option(&mut argv, "--model", "b"), "an unchanged value reports no change");
    assert!(set_option(&mut argv, "--adapter", "claude-cli"));
    let words: Vec<&str> = argv.iter().map(|a| a.as_str().unwrap()).collect();
    assert_eq!(words, ["tools/llm-node.sh", "n", "--adapter", "claude-cli", "--model", "b", "--stub", "stub-x", "y"]);
    let mut bare: Vec<Yaml> = ["tools/llm-node.sh", "n"].iter().map(|s| Yaml::String((*s).to_owned())).collect();
    assert!(set_option(&mut bare, "--model", "m"));
    assert_eq!(bare.len(), 4);
}

#[test]
fn a_login_file_is_named_in_the_copy_only_as_a_link_and_never_in_a_result() {
    let dir = Dir::new("r2-copy-login");
    let login = dir.path().join("private-login-dir/credentials.json");
    std::fs::create_dir_all(login.parent().unwrap()).unwrap();
    std::fs::write(&login, "{}").unwrap();
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let secret = login.display().to_string();
    let settings = json!({"login_file": secret, "adapter": "claude-cli"});
    let root = materialise(&committed(), &dst, "sdc", &settings).unwrap();
    assert!(!root.display().to_string().contains(&secret));
    let link = dir.path().join("run/login");
    assert_eq!(std::fs::read_link(&link).unwrap(), login);
    let mut with_option = 0;
    for (name, action) in actions(&dst) {
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("llm-node.sh") {
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
        if action["executor"]["kind"].as_str() == Some("program") && argv_of(&action)[0].ends_with("llm-node.sh") {
            assert_eq!(option_of(&argv_of(&action), "--claude-credentials"), None);
        }
    }
    // a login file that is not there is refused without its path
    let gone = materialise(&committed(), &dir.path().join("run2/bindings"), "sdc", &json!({"login_file": dir.path().join("nope/x.json").display().to_string()})).unwrap_err();
    assert!(!gone.contains("nope"), "{gone}");
    let relative = materialise(&committed(), &dir.path().join("run3/bindings"), "sdc", &json!({"login_file": "x.json"})).unwrap_err();
    assert!(!relative.contains("x.json"), "{relative}");
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
    let root = materialise(&src, &dst, "sdc", &settings);
    for d in &dirs {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    root.unwrap();
    assert_eq!(digest_tree(&src), frozen, "the source changed");
    assert_eq!(actions(&dst).len(), 66);
    assert!(std::fs::metadata(dst.join("sdc.yaml")).unwrap().permissions().mode() & 0o200 != 0, "the copy is the run's to write");
}

#[test]
fn an_existing_destination_is_refused_and_the_source_untouched() {
    let (dir, _dst, _r) = make("r2-copy-same", &json!({}));
    let before = digest_tree(&committed());
    let result = materialise(&committed(), &dir.path().join("run/bindings"), "sdc", &json!({}));
    assert!(result.unwrap_err().ends_with(": exists"));
    assert_eq!(digest_tree(&committed()), before);
}

#[test]
fn a_missing_root_file_is_refused_and_the_instructions_come_along() {
    let dir = Dir::new("r2-copy-root");
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let error = materialise(&committed(), &dst, "nonexistent", &json!({})).unwrap_err();
    assert!(error.contains("the root bindings file is missing"));
    assert!(dir.path().join("run/instructions/decider").is_dir());
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
        "decider": {"kind": "model", "model": "claude-opus-5-5", "program": "x", "reply_bin": "y"},
        "adapter": "claude-cli",
        "models": {"writer": {"model": "claude-sonnet-5-5", "max_turns": 7, "max_budget_usd": 1.5}},
    });
    let dst = dir.path().join("run/bindings");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let root = materialise(&committed(), &dst, "sdc", &settings).unwrap();
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
