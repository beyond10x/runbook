//! `sdc-reply` and the fixture adapter read the workspace where Loom starts them, not a directory they
//! derive from it. Loom (c9b4c177) starts a bound program in the workspace root and gives it PATH,
//! LOOM_CASE, LOOM_TIMEOUT_SECONDS and, for an opened case, LOOM_CASE_NAME and LOOM_PARENT_CASE_NAME:
//! no case directory, no store. A suite keeps the workspace in `<scratch>/work/<n>` and the store in
//! `<scratch>/store/<n>`; a workspace is written by a model, so a `work/` directory in it, with a
//! `case.json` look-alike, is ordinary. The facts, the rejection count and the node's inputs are those
//! of the real case in every layout.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REPLY: &str = env!("CARGO_BIN_EXE_sdc-reply");

#[derive(Clone, Copy, Debug)]
enum Decoy {
    None,
    EmptyWork,
    CaseLookAlike,
}

/// A workspace `<root>/work/1` (its store `<root>/store/1` beside it) that holds a story, a tree to
/// review with a conformance record, and one rejection of `change.review`.
fn workspace(name: &str, decoy: Decoy) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("workspace-layout").join(name);
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work/1");
    fs::create_dir_all(root.join("store/1")).unwrap();
    put(&work, "aep/story/list.md", "---\nformat: aep.planning-md/3\nid: story:list\nkind: story\nstatus: active\ntitle: List\nsummary: covers billing.invoice.CreateInvoice\n---\n# List\n");
    put(&work, "impl/README.md", "# impl\n");
    put(&work, "mainline/README.md", "# mainline\n");
    put(&root, "conformance.log", "12 passed\n");
    let o = Command::new(REPLY)
        .args(["record", "--work", ".", "--kind", "conformance_run", "--tree", "impl", "--log"])
        .arg(root.join("conformance.log"))
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(o.status.success(), "sdc-reply record: {}", String::from_utf8_lossy(&o.stderr));
    put(&work, ".sdc/records/rejections/change.review/001.json", r#"{"format":"sdc.rejection/1","real":true}"#);
    match decoy {
        Decoy::None => {}
        Decoy::EmptyWork => fs::create_dir_all(work.join("work")).unwrap(),
        Decoy::CaseLookAlike => {
            put(&work, "work/case.json", r#"{"id":"decoy.case","artifacts":{"impl":{"revision":"decoy"},"spec":{"revision":"decoy"}}}"#);
            put(&work, "work/bindings.json", "{}");
            put(&work, "work/events.jsonl", "");
            for n in ["900", "901", "902"] {
                put(&work, &format!("work/.sdc/records/rejections/change.review/{n}.json"), r#"{"format":"sdc.rejection/1","decoy":true}"#);
            }
        }
    }
    work
}

fn put(base: &Path, rel: &str, text: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn reply(work: &Path, case_dir: &Path, args: &[&str]) -> Output {
    let mut c = Command::new(REPLY);
    c.args(&args[..1]).arg("--case-dir").arg(case_dir).args(&args[1..]);
    c.current_dir(work).env_remove("SDC_CASE_DIR").env_remove("SDC_REVIEW_MODE").env("LOOM_CASE", "sdc.billing.list");
    c.output().unwrap()
}

fn out(o: &Output) -> String {
    format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// The case directories a caller may name: the workspace, its parent (the old `$PWD/..`), a path that
/// does not exist. Under Loom none of them is read.
fn named(work: &Path) -> [PathBuf; 3] {
    [work.to_path_buf(), work.join(".."), work.join("../nowhere")]
}

#[test]
fn the_rejections_counted_are_those_of_the_workspace_whatever_work_holds() {
    for decoy in [Decoy::None, Decoy::EmptyWork, Decoy::CaseLookAlike] {
        let work = workspace(&format!("rejections-{decoy:?}"), decoy);
        for case_dir in named(&work) {
            let o = reply(&work, &case_dir, &["rejections", "--action", "change.review"]);
            assert!(o.status.success(), "{decoy:?} {}: {}", case_dir.display(), out(&o));
            let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
            assert_eq!(v.as_array().map(Vec::len), Some(1), "{decoy:?} {}: {v}", case_dir.display());
            assert_eq!(v[0]["real"], true, "{decoy:?} {}: {v}", case_dir.display());
        }
    }
}

#[test]
fn the_inputs_of_the_review_are_those_of_the_workspace_whatever_work_holds() {
    for decoy in [Decoy::None, Decoy::EmptyWork, Decoy::CaseLookAlike] {
        let work = workspace(&format!("review-inputs-{decoy:?}"), decoy);
        for (i, case_dir) in named(&work).iter().enumerate() {
            let call = work.join(format!("call{i}"));
            fs::create_dir_all(&call).unwrap();
            let cwd = call.display().to_string();
            let o = reply(&work, case_dir, &["review-inputs", "--cwd", &cwd, "--story", "list"]);
            assert!(o.status.success(), "{decoy:?} {}: {}", case_dir.display(), out(&o));
            let listed = fs::read_to_string(call.join("in/story.md")).unwrap_or_default();
            assert!(listed.contains("covers billing.invoice.CreateInvoice") || out(&o).contains("story"), "{decoy:?} {}: {}", case_dir.display(), out(&o));
        }
    }
}

#[test]
fn the_fixture_adapter_takes_the_round_of_the_rejections_of_the_workspace() {
    let adapter = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools/llm-adapter-fixture.sh");
    let rounds = r#"{"rounds":[{"decision":"approved","summary":"round0"},{"decision":"rejected","summary":"round1","findings":[]},{"decision":"rejected","summary":"round2","findings":[]}]}"#;
    for decoy in [Decoy::None, Decoy::EmptyWork, Decoy::CaseLookAlike] {
        let work = workspace(&format!("adapter-{decoy:?}"), decoy);
        let call = work.join(".sdc/llm/001-change.review/cwd");
        fs::create_dir_all(call.join("in")).unwrap();
        let request = work.join("request.json");
        fs::write(&request, serde_json::json!({"cwd": call, "node": "change.review", "model": "m", "review_attempt": 1, "case_dir": ""}).to_string()).unwrap();
        let bins = Path::new(REPLY).parent().unwrap().display().to_string();
        let o = Command::new("bash")
            .arg(&adapter)
            .arg(&request)
            .current_dir(&work)
            .env("PATH", format!("{bins}:{}", std::env::var("PATH").unwrap_or_default()))
            .env("LOOM_CASE", "sdc.billing.list")
            .env("SDC_REPLY", REPLY)
            .env("SDC_REVIEW_REPLIES", rounds)
            .output()
            .unwrap();
        assert!(o.status.success(), "{decoy:?}: {}", out(&o));
        let r: serde_json::Value = serde_json::from_str(&fs::read_to_string(call.join("out/review.json")).unwrap()).unwrap();
        assert_eq!(r["summary"], "round1", "{decoy:?}: one rejection of the workspace is on record: {r}");
    }
}
