//! A held tree (`tree hold`, Loom's `stop recorded` line) through the calls the runner and the engine
//! make: `run_tree` refuses it `TreeStopped`, the refusal is typed and named, and after `tree reopen`
//! the same tree runs. No model, no network: the bound programs are POSIX sh. The refusal is the one
//! that ends a run as `refused`, cause `tree_stopped` (`engine::refusal`).

use b10x_loom_tree::model::{TreeRefusalReason, TreeStop};
use b10x_loom_tree::{hold, reopen, verify_tree, HoldRequest, ReopenRequest, TreeTime};
use runbook::engine::calls;
use runbook::engine::refusal;
use runbook::runtime::Confinement;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-suite")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

struct Tree {
    store: PathBuf,
    catalog: loom_protocols::ProtocolCatalog,
}

fn open(tag: &str) -> Tree {
    let base =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("held-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    copy_dir(&fixtures().join("suites/bindings"), &base.join("bindings"));
    let store = base.join("store");
    let work = base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let opening = calls::Opening {
        store: &store,
        workspace: &work,
        protocols: &fixtures().join("protocol"),
        root: calls::Root::Registration("suite-demo@1"),
        bindings: &base.join("bindings/suite-demo.yaml"),
        case: "held-1",
    };
    calls::open(&opening, || Ok(())).unwrap();
    let catalog = calls::catalog(&store).unwrap();
    Tree { store, catalog }
}

fn hold_request<'a>(store: &'a Path) -> HoldRequest<'a> {
    HoldRequest {
        store,
        principal: "person:operator",
        reason: "the budget is under review",
        time: TreeTime::Host,
    }
}

#[test]
fn a_held_tree_is_refused_tree_stopped_by_name_and_runs_again_after_a_reopen() {
    let tree = open("run");
    let runner = calls::runner(Confinement::Local).unwrap();
    let settings = calls::run_settings(runner, Confinement::Local, None);

    hold(&hold_request(&tree.store)).unwrap();
    let refused = calls::run(&tree.store, &tree.catalog, &settings, None, None)
        .err()
        .expect("a held tree is not run");
    assert_eq!(refused.reason, TreeRefusalReason::TreeStopped, "{refused}");
    assert_eq!(refused.cause(), "tree_stopped");
    assert!(refused.to_string().starts_with("TreeStopped:"), "{refused}");
    assert_eq!(
        refusal::named(&refused.to_string()),
        Some(TreeRefusalReason::TreeStopped)
    );

    reopen(&ReopenRequest {
        store: &tree.store,
        principal: "person:operator",
        time: TreeTime::Host,
    })
    .unwrap();
    let ran = calls::run(&tree.store, &tree.catalog, &settings, None, None)
        .unwrap_or_else(|e| panic!("a reopened tree runs: {e}"));
    assert_eq!(ran.stop, TreeStop::Terminal, "{:?}", ran.detail);
    assert!(ran.steps > 0);
    assert!(
        verify_tree(&tree.store, &tree.catalog).is_ok(),
        "the log with its hold and reopen lines verifies"
    );
}

#[test]
fn holding_twice_and_reopening_an_open_tree_are_refused_by_name() {
    let tree = open("twice");
    let again = {
        hold(&hold_request(&tree.store)).unwrap();
        hold(&hold_request(&tree.store)).expect_err("held already")
    };
    assert_eq!(refusal::Refusal::of(&again).cause(), "tree_already_stopped");
    let request = ReopenRequest {
        store: &tree.store,
        principal: "person:operator",
        time: TreeTime::Host,
    };
    reopen(&request).unwrap();
    let not_held = reopen(&request).expect_err("not held");
    assert_eq!(refusal::Refusal::of(&not_held).cause(), "tree_not_stopped");
    let unusable = hold(&HoldRequest {
        reason: "",
        ..hold_request(&tree.store)
    })
    .expect_err("a hold needs a reason");
    assert_eq!(refusal::Refusal::of(&unusable).cause(), "stop_unusable");
}
