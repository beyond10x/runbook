//! The daemon with several runbook homes. The guarantee, "no path of the state directory or of a
//! runbook home leaves the engine toward a client", with a configured runbook directory, a second one and the
//! built-in home.

use super::state::scratch;
use runbook::commands::{Config, Daemon, SubmitRun};
use runbook::engine::Engine;
use runbook::runtime::Confinement;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

struct Homes {
    base: PathBuf,
    d: Daemon,
}

impl Homes {
    fn new(tag: &str, spoil: &dyn Fn(&Path, &Path)) -> Homes {
        let base = std::fs::canonicalize(scratch(&format!("homes-{tag}"))).unwrap();
        let books = base.join("HOMESBOOKSONE");
        let again = base.join("HOMESBOOKSTWO");
        let home = base.join("HOMESHOME");
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &books);
        // the second configured directory and the built-in home hold other names (a name in two is refused by its name)
        for dir in [&again, &home] {
            std::fs::create_dir_all(dir.join("protocol")).unwrap();
            std::fs::create_dir_all(dir.join("bindings/tools")).unwrap();
        }
        std::fs::write(again.join("protocol/other.protocol.yaml"), "protocol:\n  id: other\n  revision: 1\n").unwrap();
        std::fs::write(again.join("bindings/other.yaml"), "format: loom.bindings/1\n").unwrap();
        std::fs::write(home.join("protocol/third.protocol.yaml"), "protocol:\n  id: third\n  revision: 1\n").unwrap();
        std::fs::write(home.join("bindings/third.yaml"), "format: loom.bindings/1\n").unwrap();
        spoil(&books, &again);
        std::fs::create_dir_all(base.join("HOMESCTX")).unwrap();
        let state = base.join("HOMESSTATE");
        let engine = Arc::new(Engine::with_levels(home, state.clone(), &[Confinement::Local]).with_runbook_dirs(vec![books, again]));
        let cfg = Config { state_dir: state, confinement_floor: Confinement::Local, context_roots: vec![base.join("HOMESCTX")], runbook_dir: None, principal: super::PRINCIPAL.into() };
        Homes { base, d: Daemon::start(cfg, engine).unwrap() }
    }

    /// What a client reads of the run `engine-demo` that refuses to start.
    fn seen(&self) -> String {
        let o = self.d.submit_run(SubmitRun { runbook_id: "engine-demo".into(), intake: "a note on tea".into(), confinement: Some(Confinement::Local), ..SubmitRun::default() }).unwrap();
        let Some(run) = o.run_id.clone() else { return format!("{o:?}") };
        let limit = Instant::now() + Duration::from_secs(40);
        loop {
            let state = self.d.run_detail(&run)[0]["state"].as_str().unwrap().to_string();
            if matches!(state.as_str(), "Ended" | "Cancelled" | "FailedToStart") {
                break;
            }
            assert!(Instant::now() < limit, "the run did not end: {state}");
            std::thread::sleep(Duration::from_millis(10));
        }
        format!("{o:?} {} {:?} {:?}", self.d.run_detail(&run)[0], self.d.events_after(&run, 0), self.d.decisions_of_run(&run))
    }
}

impl Drop for Homes {
    fn drop(&mut self) {
        self.d.stop();
        let _ = &self.base;
    }
}

#[test]
fn a_refusal_in_a_configured_directory_names_no_path_of_any_home() {
    let kinds = ["bindings-not-yaml", "protocol-not-yaml", "unreadable-tool", "link-into-the-other-directory", "link-out", "too-deep"];
    let mut bad = Vec::new();
    for kind in kinds {
        let spoil = |books: &Path, again: &Path| match kind {
            "bindings-not-yaml" => std::fs::write(books.join("bindings/engine-demo.yaml"), "a: [unclosed\n").unwrap(),
            "protocol-not-yaml" => std::fs::write(books.join("protocol/zz.protocol.yaml"), "protocol: [unclosed\n").unwrap(),
            "unreadable-tool" => {
                std::fs::write(books.join("bindings/tools/locked.sh"), "x").unwrap();
                std::fs::set_permissions(books.join("bindings/tools/locked.sh"), std::fs::Permissions::from_mode(0o000)).unwrap();
            }
            "link-into-the-other-directory" => std::os::unix::fs::symlink(again.join("bindings/other.yaml"), books.join("bindings/tools/other-link.yaml")).unwrap(),
            "link-out" => std::os::unix::fs::symlink("/etc/hostname", books.join("bindings/tools/leak.sh")).unwrap(),
            "too-deep" => {
                let mut deep = books.join("bindings/tools");
                for _ in 0..30 {
                    deep = deep.join("d");
                }
                std::fs::create_dir_all(&deep).unwrap();
            }
            other => panic!("{other}"),
        };
        let homes = Homes::new(kind, &spoil);
        let seen = homes.seen();
        assert!(seen.contains("FailedToStart") || seen.contains("refus") || seen.contains("Refus"), "{kind}: the run did not fail to start, the case measures nothing: {seen}");
        for name in ["HOMESBOOKSONE", "HOMESBOOKSTWO", "HOMESHOME", "HOMESSTATE"] {
            if seen.contains(name) {
                let at = seen.find(name).unwrap();
                bad.push(format!("{kind}: names {name}: {}", &seen[at.saturating_sub(60)..(at + 80).min(seen.len())]));
            }
        }
    }
    assert!(bad.is_empty(), "{} refusals name a path of a home or of the state directory:\n{}", bad.len(), bad.join("\n"));
}
