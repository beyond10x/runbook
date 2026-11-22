//! The `runtime` block of `driver/examples/sdc/rule-ends.json` and the way `sdc-check-routes`
//! reads it: a file the check cannot read as a whole is refused with a message that names the
//! member, and the live tree prints one capability line per member and exits 0.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            fs::copy(&p, &q).unwrap();
        }
    }
}

/// A scratch root under `driver/target/` holding what `sdc-check-routes` reads.
fn scratch_root(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("runtime_block_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    copy_dir(&root().join("protocol/compiled"), &d.join("protocol/compiled"));
    copy_dir(&root().join("driver/examples/sdc"), &d.join("driver/examples/sdc"));
    fs::create_dir_all(d.join("docs")).unwrap();
    fs::copy(root().join("docs/reject-routes.md"), d.join("docs/reject-routes.md")).unwrap();
    d
}

fn ends_path(d: &Path) -> PathBuf {
    d.join("driver/examples/sdc/rule-ends.json")
}

fn run_routes(d: &Path) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-check-routes"))
        .arg("--root")
        .arg(d)
        .arg("--doc")
        .arg(d.join("docs/reject-routes.md"))
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into())
}

/// Runs the check on a copy of the tree whose rule ends `edit` has changed.
fn refused_after(tag: &str, edit: impl FnOnce(&mut Value)) -> (i32, String, String) {
    let d = scratch_root(tag);
    let p = ends_path(&d);
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
    edit(&mut v);
    fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let r = run_routes(&d);
    let _ = fs::remove_dir_all(&d);
    r
}

fn assert_refused(r: (i32, String, String), names: &[&str]) {
    let (code, out, err) = r;
    assert_eq!(code, 2, "exit 2 (the file cannot be read):\nstdout:\n{out}\nstderr:\n{err}");
    assert!(!out.contains("routes:") && !out.contains("capability "), "nothing is printed before the refusal:\n{out}");
    for n in names {
        assert!(err.contains(n), "the message names `{n}`:\n{err}");
    }
}

#[test]
fn the_old_key_driver_is_refused_and_called_renamed() {
    let r = refused_after("oldkey", |v| {
        let rt = v.as_object_mut().unwrap().remove("runtime").unwrap();
        v.as_object_mut().unwrap().insert("driver".into(), rt);
    });
    assert_refused(r, &["runtime", "driver", "renamed"]);
}

#[test]
fn a_file_without_a_runtime_block_is_refused() {
    let r = refused_after("noruntime", |v| {
        v.as_object_mut().unwrap().remove("runtime");
    });
    assert_refused(r, &["runtime"]);
}

#[test]
fn a_member_without_provided_by_is_refused() {
    let r = refused_after("noprov", |v| {
        v["runtime"]["provided_by"].as_object_mut().unwrap().remove("cap_per_kind");
    });
    assert_refused(r, &["cap_per_kind", "provided_by"]);
}

#[test]
fn a_provided_by_entry_for_a_member_that_is_not_there_is_refused() {
    let r = refused_after("orphan", |v| {
        v["runtime"]["provided_by"]["ghost"] = json!("loom: nothing, x.rs:1");
    });
    assert_refused(r, &["ghost", "provided_by"]);
}

#[test]
fn an_unknown_member_of_the_runtime_is_refused() {
    let r = refused_after("unknown", |v| {
        v["runtime"]["bogus_member"] = json!(true);
        v["runtime"]["provided_by"]["bogus_member"] = json!("loom: nothing, x.rs:1");
    });
    assert_refused(r, &["bogus_member"]);
}

#[test]
fn a_provided_by_value_of_no_known_form_is_refused() {
    let r = refused_after("form", |v| {
        v["runtime"]["provided_by"]["cap_per_kind"] = json!("pending: docs/loom-bindings.md:106");
    });
    assert_refused(r, &["cap_per_kind", "loom:", "runner:", "absent:"]);
}

#[test]
fn the_live_tree_prints_one_capability_line_per_member_and_exits_0() {
    let d = scratch_root("live");
    let (code, out, err) = run_routes(&d);
    let ends: Value = serde_json::from_str(&fs::read_to_string(ends_path(&d)).unwrap()).unwrap();
    let _ = fs::remove_dir_all(&d);
    let members = ends["runtime"].as_object().unwrap().len() - 1;
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out.lines().filter(|l| l.starts_with("capability ")).count(), members, "{out}");
}

fn run_with(tag: &str, edit: impl FnOnce(&mut Value)) -> (i32, String, String) {
    refused_after(tag, edit)
}

#[test]
fn the_unedited_block_is_read_and_exits_0() {
    let (code, out, err) = run_with("live2", |_| {});
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn a_provided_by_value_with_the_prefix_and_nothing_after_it_is_refused() {
    for v in ["loom: ", "runner: ", "absent: "] {
        let (code, out, err) = run_with("emptyval", |e| e["runtime"]["provided_by"]["capped_effects"] = json!(v));
        assert_eq!(code, 2, "{v:?} accepted:\n{out}{err}");
    }
}

#[test]
fn a_loom_value_with_no_file_and_line_is_refused() {
    let (code, out, err) = run_with("nofileline", |e| e["runtime"]["provided_by"]["capped_effects"] = json!("loom: nothing in particular"));
    assert_eq!(code, 2, "accepted:\n{out}{err}");
    assert!(err.contains("capped_effects") && err.contains("file:line"), "{err}");
}

#[test]
fn a_member_of_the_wrong_type_is_refused() {
    for (member, value) in [
        ("rejection_is_arrival", json!("8")),
        ("capped_effects", json!("write")),
        ("cap_per_kind", json!("false")),
        ("capped_effects", json!(["bogus"])),
        ("own_failure_fed_back", json!(-3)),
    ] {
        let (code, out, err) = run_with("wrongtype", |e| e["runtime"][member] = value.clone());
        assert_eq!(code, 2, "{member}={value} accepted (exit {code}):\n{out}{err}");
    }
}

#[test]
fn two_refusals_at_once_are_both_named() {
    let (code, out, err) = run_with("two", |e| {
        e["runtime"]["provided_by"].as_object_mut().unwrap().remove("rejection_logs_fed_back");
        e["runtime"]["provided_by"]["cap_per_kind"] = json!("nothing");
    });
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("rejection_logs_fed_back") && err.contains("cap_per_kind"), "only one is named:\n{err}");
}

#[test]
fn a_refusal_prints_no_route_and_names_the_file() {
    let (code, out, err) = run_with("order", |e| {
        e["runtime"].as_object_mut().unwrap().remove("provided_by");
    });
    assert_eq!(code, 2);
    assert!(out.is_empty(), "stdout before a refusal:\n{out}");
    assert!(err.contains("rule-ends.json"), "{err}");
}

#[test]
fn a_member_that_says_it_is_present_while_provided_by_says_absent_is_refused() {
    let (code, out, err) = run_with("contradict", |e| e["runtime"]["cap_per_kind"] = json!(true));
    assert_eq!(code, 2, "cap_per_kind true next to `absent:` accepted:\n{out}{err}");
}

#[test]
fn a_member_that_says_it_is_absent_while_provided_by_says_who_holds_it_is_refused() {
    for who in ["loom: a reason, x.rs:1", "runner: a reason"] {
        let (code, out, err) = run_with("contradict-false", |e| {
            e["runtime"]["rejection_logs_fed_back"] = json!(false);
            e["runtime"]["provided_by"]["rejection_logs_fed_back"] = json!(who);
        });
        assert_eq!(code, 2, "false next to {who:?} accepted:\n{out}{err}");
        assert!(err.contains("rejection_logs_fed_back") && err.contains("lacks"), "{err}");
    }
}

#[test]
fn an_empty_capped_effects_next_to_loom_and_a_full_one_next_to_absent_are_refused() {
    let (code, out, err) = run_with("contradict-list", |e| e["runtime"]["capped_effects"] = json!([]));
    assert_eq!(code, 2, "empty list next to `loom:` accepted:\n{out}{err}");
    let (code, out, err) = run_with("contradict-list2", |e| {
        e["runtime"]["provided_by"]["capped_effects"] = json!("absent: nothing caps, x.rs:1");
    });
    assert_eq!(code, 2, "a list next to `absent:` accepted:\n{out}{err}");
}

#[test]
fn every_refusal_is_named_on_its_own_line() {
    let (code, out, err) = run_with("lines", |e| {
        e["runtime"]["provided_by"]["capped_effects"] = json!("loom: ");
        e["runtime"]["provided_by"]["cap_per_kind"] = json!("loom: no file or line");
        e["runtime"]["own_failure_fed_back"] = json!("yes");
        e["runtime"]["rejection_is_arrival"] = json!(true);
        e["runtime"]["bogus"] = json!(true);
    });
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.is_empty(), "{out}");
    for (name, word) in [
        ("capped_effects", "nothing after"),
        ("cap_per_kind", "file:line"),
        ("own_failure_fed_back", "true or false"),
        ("rejection_is_arrival", "absent"),
        ("bogus", "not one the check knows"),
    ] {
        let hits = err.lines().filter(|l| l.contains(name) && l.contains(word)).count();
        assert_eq!(hits, 1, "`{name}` / `{word}` is named on {hits} lines:\n{err}");
    }
    assert!(err.lines().all(|l| l.starts_with("sdc-check-routes: ")), "{err}");
}

#[test]
fn a_web_address_is_not_a_file_and_line() {
    let (code, out, err) = run_with("url", |e| e["runtime"]["provided_by"]["capped_effects"] = json!("loom: see https://example.com:443"));
    assert_eq!(code, 2, "a port number read as a line:\n{out}{err}");
}

#[test]
fn value_forms_of_no_known_kind_are_refused() {
    let forms: Vec<(&str, Value)> = vec![
        ("capped_effects", json!(["Write"])),
        ("cap_per_kind", Value::Null),
        ("capped_effects", json!([])),
    ];
    for (member, v) in forms {
        let (code, out, err) = run_with("forms", |e| e["runtime"][member] = v.clone());
        assert_eq!(code, 2, "{member}={v}:\n{out}{err}");
    }
    for who in ["loom: x, a:1-2-3", "loom: x, :5", "loom: x, a:\u{661}"] {
        let (code, out, err) = run_with("whos", |e| e["runtime"]["provided_by"]["capped_effects"] = json!(who));
        assert_eq!(code, 2, "{who:?}:\n{out}{err}");
    }
    let (code, out, err) = run_with("array", |e| e["runtime"] = json!([]));
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.is_empty());
}

#[test]
fn no_stale_retry_names_a_kind_the_yaml_binding_produces() {
    // The YAML Loom runs says `questions.answer` records `answer_applied` (evidence entry, results 0 and 1);
    // the check must not call that kind one no bound action produces.
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-check-routes")).arg("--root").arg(root()).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    let stale: Vec<&str> = out.lines().filter(|l| l.starts_with("stale-retry")).collect();
    assert!(stale.is_empty(), "{stale:?}");
}
