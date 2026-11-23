//! The modes of the directories the script writes: 755 and no set-id bit below `libexec/runbook` and `share/runbook`,
//! whatever the prefix hands down and whatever an older install left. Fake checkouts; every prefix lies under the target
//! directory.

use super::*;

fn mode(p: &Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

fn own_directories(prefix: &Path) -> Vec<PathBuf> {
    let mut found = vec![prefix.join("libexec/runbook"), prefix.join("share/runbook")];
    let mut i = 0;
    while i < found.len() {
        for e in fs::read_dir(&found[i]).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                found.push(e.path());
            }
        }
        i += 1;
    }
    found
}

/// A prefix with the set-group-id bit hands it down to every directory `mkdir` makes in it, and an older install may
/// have left a directory writable by all: every directory of the product's own trees is 755 afterwards, and `bin`,
/// `libexec` and `share` (shared with other packages) are left as they are.
#[test]
fn the_directories_of_the_product_are_755_without_a_set_id_bit_under_a_setgid_prefix_and_over_a_wide_open_one() {
    let root = fake_checkout("fix2-modes", None);
    let (_, prefix) = sandbox_of(&root);
    fs::set_permissions(&prefix, fs::Permissions::from_mode(0o2755)).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let wrong: Vec<String> = own_directories(&prefix).iter().filter(|d| mode(d) != 0o755).map(|d| format!("{} {:o}", d.display(), mode(d))).collect();
    assert!(wrong.is_empty(), "a setgid prefix: {wrong:?}");

    fs::set_permissions(prefix.join("share/runbook/protocol"), fs::Permissions::from_mode(0o777)).unwrap();
    fs::set_permissions(prefix.join("libexec/runbook"), fs::Permissions::from_mode(0o2777)).unwrap();
    fs::set_permissions(prefix.join("libexec"), fs::Permissions::from_mode(0o750)).unwrap();
    let o = from_built(&root, &prefix);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let wrong: Vec<String> = own_directories(&prefix).iter().filter(|d| mode(d) != 0o755).map(|d| format!("{} {:o}", d.display(), mode(d))).collect();
    assert!(wrong.is_empty(), "directories an older install left: {wrong:?}");
    assert_eq!(mode(&prefix.join("libexec")), 0o750, "libexec is shared and keeps its mode");
}
