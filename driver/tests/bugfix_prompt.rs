//! The bug-fix writers' prompts (`driver/examples/sdc/tools/llm-node.sh`, `bug.dissect`,
//! `test.reproduce`, `change.fix`). A run seeded without a ticket keeps its prompt byte for byte
//! (`tests/fixtures/bugfix-prompt/<node>.md`); a run seeded from a ticket (`bug.yaml` carries
//! `ticket`) drops "Standard library only" and carries the ticket's constraints.
//!
//! The adapter is `true`: nothing is called, the prompt is written before the adapter runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const NODES: [&str; 3] = ["bug.dissect", "test.reproduce", "change.fix"];

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}

/// A case's work/ as it stands before `change.fix` on the pager seed, with `bug_yaml` as the
/// report; each node's prompt.md.
fn prompts(name: &str, bug_yaml: &str) -> Vec<(String, String)> {
    let fx = manifest().join("examples/sdc/fixtures/bugfix-pager");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bugfix-prompt").join(name);
    let _ = fs::remove_dir_all(&dir);
    let mut out = Vec::new();
    for node in NODES {
        let work = dir.join(node);
        copy_dir(&fx.join("crate"), &work.join("mainline"));
        copy_dir(&fx.join("crate"), &work.join("impl"));
        copy_dir(&fx.join("stub-repro"), &work.join("repro"));
        fs::create_dir_all(work.join("impl/tests")).unwrap();
        fs::copy(fx.join("stub-repro/last_page.rs"), work.join("impl/tests/last_page.rs")).unwrap();
        fs::copy(fx.join("stub-dissection.json"), work.join("bug-dissection.json")).unwrap();
        fs::write(work.join("bug.yaml"), bug_yaml).unwrap();
        let mut c = Command::new("bash");
        c.arg(manifest().join("examples/sdc/tools/llm-node.sh")).arg(node).current_dir(&work);
        for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("SDC_")) {
            c.env_remove(k);
        }
        let o = c
            .env("SDC_LLM_ADAPTER", "true")
            .env("SDC_BUGFIX", env!("CARGO_BIN_EXE_sdc-bugfix"))
            .output()
            .unwrap();
        let p = work.join(format!(".sdc/llm/001-{node}/prompt.md"));
        let text = fs::read_to_string(&p).unwrap_or_else(|_| {
            panic!("{node}: no prompt.md; stdout {} stderr {}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
        });
        out.push((node.to_string(), text));
    }
    out
}

#[test]
fn a_seed_run_without_a_ticket_keeps_its_prompts_byte_for_byte() {
    let bug = fs::read_to_string(manifest().join("examples/sdc/fixtures/bugfix-pager/bug.yaml")).unwrap();
    let golden = manifest().join("tests/fixtures/bugfix-prompt");
    for (node, text) in prompts("seed", &bug) {
        let g = golden.join(format!("{node}.md"));
        let want = fs::read_to_string(&g).unwrap_or_else(|_| panic!("no {}", g.display()));
        assert_eq!(text, want, "{node}: the prompt of a run without a ticket changed");
    }
}

#[test]
fn a_ticket_run_drops_standard_library_only_and_carries_the_constraints() {
    let bug = "title: The last partial page is never shown
text: Invented report.
story: story:ticket-pager-7
ticket:
  id: PAGER-7
  kind: bug
constraints:
- Keep the public API of the crate unchanged.
- Add no dependency.
";
    let p = prompts("ticket", bug);
    let get = |n: &str| p.iter().find(|(k, _)| k == n).map(|(_, t)| t.clone()).unwrap();
    let fix = get("change.fix");
    assert!(!fix.contains("Standard library only"), "{fix}");
    assert!(fix.contains("keep the public API.\n"), "{fix}");
    for node in ["test.reproduce", "change.fix"] {
        let t = get(node);
        assert!(t.contains("Constraints of the ticket (in/bug.yaml `constraints`), each one binding:\n- Keep the public API of the crate unchanged.\n- Add no dependency.\n"), "{node}: {t}");
    }
    assert!(!get("bug.dissect").contains("Constraints of the ticket"));
}
