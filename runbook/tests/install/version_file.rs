//! What `share/runbook/VERSION` says and which bytes are installed: the commit the binary was built from, and a mark
//! when a tracked file of the copied directories differs from it. Fake checkouts with a git of their own.

use super::*;

fn version_of(prefix: &Path) -> String {
    fs::read_to_string(prefix.join("share/runbook/VERSION")).unwrap()
}

/// A clean checkout gives `VERSION` = what the binary prints; a tracked file of a copied directory that differs from
/// HEAD adds ` dirty`, and the bytes installed are those of the work tree, not of HEAD. An untracked file there and a
/// change outside the copied directories add no mark.
#[test]
fn version_names_head_and_a_tracked_change_of_a_copied_directory_adds_one_word_and_installs_the_work_tree_bytes() {
    let root = fake_checkout("fix2-dirty", None);
    let (sandbox, prefix) = sandbox_of(&root);
    let head = head12(&root);
    let clean = stub_version(&head);
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    assert_eq!(version_of(&prefix), format!("{clean}\n"), "a clean checkout");

    fs::write(root.join("protocol/untracked.txt"), "left by a run\n").unwrap();
    fs::write(root.join("README.md"), "outside the copied directories\n").unwrap();
    git_in(&root, &["add", "README.md"]);
    git_in(&root, &["commit", "-q", "-m", "readme"]);
    stubs(&root, None, &stub_version(&head12(&root)));
    let clean = stub_version(&head12(&root));
    fs::write(root.join("README.md"), "changed, and still outside\n").unwrap();
    assert_eq!(code(&from_built(&root, &prefix)), 0);
    assert_eq!(version_of(&prefix), format!("{clean}\n"), "an untracked file and a change outside the copied directories");

    fs::write(root.join("protocol/file.txt"), "edited and not committed\n").unwrap();
    let second = sandbox.join("second/prefix");
    fs::create_dir_all(&second).unwrap();
    fs::create_dir_all(sandbox.join("second/home")).unwrap();
    let o = install(&root.join("scripts/install.sh"), &second, &["--from-built", "debug"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(version_of(&second), format!("{clean} dirty\n"), "a tracked file of protocol/ differs from HEAD");
    assert_eq!(fs::read_to_string(second.join("share/runbook/protocol/file.txt")).unwrap(), "edited and not committed\n", "the bytes of the work tree");
    assert!(err(&o).contains("dirty"), "the closing line says it: {}", err(&o));
}
