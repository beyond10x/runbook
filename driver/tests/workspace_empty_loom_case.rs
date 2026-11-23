//! `LOOM_CASE` set to the empty string is no Loom mode
//! (`Facts::open` and `loom_workspace` both read it so). A caller that runs `sdc-reply` with an empty
//! `LOOM_CASE` reads the case directory it names, the same as with the variable unset.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const REPLY: &str = env!("CARGO_BIN_EXE_sdc-reply");

fn say(o: &Output) -> String {
    format!("{:?}\n{}\n{}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn an_empty_loom_case_reads_the_case_directory_as_an_unset_one_does() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("workspace-empty-loom-case");
    let _ = fs::remove_dir_all(&root);
    let case = root.join("case");
    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(case.join("work")).unwrap();
    fs::create_dir_all(case.join("work/aep/story")).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(case.join("work/aep/story/list.md"), "---\nid: story:list\n---\nthe case directory\n").unwrap();
    let go = |loom: Option<&str>| {
        let mut c = Command::new(REPLY);
        c.args(["coverage", "--case-dir"]).arg(&case).args(["--story", "list"]).current_dir(&elsewhere).env_remove("SDC_CASE_DIR").env_remove("LOOM_CASE");
        if let Some(v) = loom {
            c.env("LOOM_CASE", v);
        }
        c.output().unwrap()
    };
    let unset = say(&go(None));
    let empty = say(&go(Some("")));
    assert!(!unset.contains("No such file"), "an unset LOOM_CASE reads <case dir>/work, where the story is: {unset}");
    assert_eq!(empty, unset, "an empty LOOM_CASE read the current directory, not the case directory");
}
