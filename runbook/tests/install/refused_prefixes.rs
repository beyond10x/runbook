//! Prefixes the script refuses because installing into them would copy the checkout into itself or replace it: the
//! checkout, a directory that holds it, and its git directory. Exit 2 and nothing written.

use super::*;

#[test]
fn a_prefix_that_is_the_checkout_holds_it_or_is_inside_its_git_directory_is_refused_and_nothing_is_written() {
    let root = fake_checkout("fix2-prefix-holds", None);
    let (sandbox, _) = sandbox_of(&root);
    let before_root = listing(&root);
    let before_sandbox = listing(&sandbox);
    for (case, prefix) in [("the checkout", root.clone()), ("a directory that holds it", sandbox.clone()), ("its git directory", root.join(".git")), ("inside its git directory", root.join(".git/objects"))] {
        let o = from_built(&root, &prefix);
        assert_eq!(code(&o), 2, "{case}: {}", err(&o));
        assert!(err(&o).contains("checkout") || err(&o).contains("git directory"), "{case}: {}", err(&o));
    }
    assert_eq!(listing(&root), before_root, "the checkout changed");
    assert_eq!(listing(&sandbox), before_sandbox, "the directory that holds it changed");
}
