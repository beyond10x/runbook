//! The bindings committed in the repository, unchanged, run by `runbook run` over Loom as a library
//! under `local` (`docs/loom-run.md`, "A run without a model node"): the bug-fix and story bindings
//! hold a model node and need their caps; the release bindings hold none, so under `local` they need
//! caps too, and with them they start and run no code of a ticket's repository.
//!
//! Programs here are sh scripts with invented text. A "model program" is a stand-in that writes a
//! marker file; "workspace code" is a hand-written Taskfile whose check writes a marker file. No
//! model call, no network, no real login file.

use super::core::{code, stderr, BIN};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// A run on a reduced directory of the committed bindings, files unchanged, with Loom as a
/// library. `bin/` is first on the PATH of the run: a stand-in seed
/// helper that copies the seed directory as the real one does, stand-ins for `ess`, `slot`,
/// `cargo` and `sdc-reply` that say they were called, and the host's own `task`.
struct Committed {
    root: PathBuf,
}

const SEED_HELPER: &str = r#"#!/bin/sh
echo "$@" >> "$(dirname "$0")/../called-sdc-bugfix"
[ "$1" = seed ] || exit 0
case $2 in
  --from) cp -r "$3" mainline; cp -r "$3" impl; printf 'text: %s\n' "$5" > bug.yaml ;;
  *) exit 65 ;;
esac
echo 'format: sdc.checks/1' > checks.yaml
"#;

impl Committed {
    fn new(tag: &str, files: &[&str]) -> Committed {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review2-committed-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["runs", "xdg", "bin", "fx/bindings", "fx/protocol", "seed/tests"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
        let repo = repo();
        for file in files {
            std::fs::copy(repo.join("bindings").join(file), root.join("fx/bindings").join(file)).unwrap();
        }
        copy_tree(&repo.join("bindings/tools"), &root.join("fx/bindings/tools"));
        std::fs::set_permissions(root.join("fx/bindings/tools/sdc-tool.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        copy_tree(&repo.join("instructions"), &root.join("fx/instructions"));
        for entry in std::fs::read_dir(repo.join("protocol")).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".protocol.yaml") {
                std::fs::copy(entry.path(), root.join("fx/protocol").join(&name)).unwrap();
            }
        }
        super::decider::install_routing_table_at(&root.join("fx"));
        std::fs::write(root.join("idea.yaml"), "text: the pager drops the last page\n").unwrap();
        // the hand-written workspace: its check writes a marker file
        let marker = root.join("marker-workspace-code");
        std::fs::write(root.join("seed/Taskfile.yml"), format!("version: '3'\ntasks:\n  check:\n    cmds:\n      - sh -c 'echo ran >> {}'\n", marker.display())).unwrap();
        std::fs::write(root.join("seed/Cargo.toml"), "[package]\nname = \"pager\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        std::fs::write(root.join("seed/tests/t.rs"), "#[test]\nfn the_last_page() {}\n").unwrap();
        let called = |name: &str, rest: &str| write_exe(&root.join("bin").join(name), &format!("#!/bin/sh\necho \"$PWD: $*\" >> \"$(dirname \"$0\")/../called-{name}\"\n{rest}\n"));
        called("ess", "exit 0");
        called("cargo", "exit 0");
        called("sdc-reply", "exit 0");
        write_exe(&root.join("bin/slot"), "#!/bin/sh\necho \"$*\" >> \"$(dirname \"$0\")/../called-slot\"\nexec \"$@\"\n");
        write_exe(&root.join("bin/sdc-bugfix"), SEED_HELPER);
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let settings = json!({"loom": {"in_process": true, "confinement": "local"}, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
        std::fs::write(root.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();
        Committed { root }
    }

    fn p(&self, rel: &str) -> String {
        self.root.join(rel).display().to_string()
    }

    /// Run `r1`: with `caps`, a ledger and both caps above 0; without, every cap at 0 and no ledger.
    fn run(&self, runbook: &str, caps: bool, extra: &[&str]) -> Output {
        let mut args: Vec<String> = ["run", runbook, "--run", "r1", "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        args.extend([self.p("runs"), "--bindings".into(), self.p("fx/bindings"), "--protocols".into(), self.p("fx/protocol"), "--intake".into(), self.p("idea.yaml")]);
        args.extend(["--confinement".into(), "local".into(), "--settings".into(), self.p("settings.json")]);
        if caps {
            args.extend(["--spend-ledger".to_owned(), self.p("ledger.json")]);
            args.extend(["--cap-micro-usd", "5000000", "--max-llm-calls", "100", "--max-wall-seconds", "40"].map(String::from));
        } else {
            args.extend(["--cap-micro-usd", "0", "--max-llm-calls", "0", "--max-wall-seconds", "40"].map(String::from));
        }
        args.extend(extra.iter().map(|s| (*s).to_owned()));
        Command::new(BIN)
            .args(args)
            .env_clear()
            .env("PATH", format!("{}:{}", self.p("bin"), std::env::var("PATH").unwrap_or_default()))
            .env("HOME", "/marker-home-dir")
            .env("XDG_RUNTIME_DIR", self.root.join("xdg"))
            .env("RUNBOOK_HOME", self.root.join("fx"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn called(&self, name: &str) -> String {
        std::fs::read_to_string(self.root.join(format!("called-{name}"))).unwrap_or_default()
    }

    fn workspace_code_ran(&self) -> bool {
        self.root.join("marker-workspace-code").exists()
    }

    /// What the run did, for the report: exit, last line of standard error, outcome, the files of
    /// the work directory, and which stand-ins were called.
    fn summary(&self, what: &str, o: &Output) -> String {
        let outcome: Value = std::fs::read_to_string(self.root.join("runs/r1/outcome.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut work: Vec<String> = std::fs::read_dir(self.root.join("runs/r1/work")).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        work.sort();
        let events = std::fs::read_to_string(self.root.join("runs/r1/events.jsonl")).unwrap_or_default();
        let steps = events.lines().filter(|l| l.contains("\"step_made\"") || l.contains("step")).count();
        let text = format!(
            "{what}\n  exit {}; stderr: {}\n  run directory made: {}; outcome {}, has_model_node {}, cause {}, reason {}\n  event lines that name a step: {steps}\n  work directory: {work:?}\n  workspace code ran: {}\n  seed helper: {:?}\n  ess: {:?}\n  slot: {:?}\n  cargo: {:?}\n",
            code(o),
            stderr(o).lines().last().unwrap_or(""),
            self.root.join("runs/r1").exists(),
            outcome["outcome"],
            outcome["has_model_node"],
            outcome["cause"],
            outcome["reason"],
            self.workspace_code_ran(),
            self.called("sdc-bugfix"),
            self.called("ess"),
            self.called("slot"),
            self.called("cargo"),
        );
        println!("{text}");
        text
    }
}

/// The bug-fix bindings as committed, alone in the `--bindings` directory, with a seed directory:
/// the file names the model tool, so the run needs a ledger and nothing of the seed is run.
#[test]
fn the_committed_bug_fix_bindings_with_a_seed_do_not_run_without_a_ledger() {
    let c = Committed::new("bugfix", &["sdc-bugfix.yaml"]);
    let o = c.run("sdc-bugfix", false, &["--bugfix", &c.p("seed")]);
    let seen = c.summary("committed sdc-bugfix.yaml alone, --bugfix <seed>, caps 0, no ledger", &o);
    assert_eq!(code(&o), 64, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(!c.root.join("runs/r1").exists(), "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
}

/// The same for the story bindings as committed (`story-tests impl`), and for the whole committed
/// directory with a seed.
#[test]
fn the_committed_story_bindings_and_the_whole_directory_do_not_run_without_a_ledger() {
    let c = Committed::new("story", &["sdc-story.yaml"]);
    let o = c.run("sdc", false, &["--root-protocol", "sdc-story@8", "--case", "STORY-1"]);
    let seen = c.summary("committed sdc-story.yaml alone, caps 0, no ledger", &o);
    assert_eq!(code(&o), 64, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(!c.root.join("runs/r1").exists(), "{seen}");

    let all = ["sdc-bugfix.yaml", "sdc-direction.yaml", "sdc-epic.yaml", "sdc-intake.yaml", "sdc-plan.yaml", "sdc-release.yaml", "sdc-spec.yaml", "sdc-story.yaml", "sdc.yaml"];
    let c = Committed::new("whole", &all);
    let o = c.run("sdc-bugfix", false, &["--bugfix", &c.p("seed")]);
    let seen = c.summary("the whole committed bindings directory, --bugfix <seed>, caps 0, no ledger", &o);
    assert_eq!(code(&o), 64, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
}

/// The release bindings as committed hold no model node. A seed directory is the one way code that
/// no bound program wrote gets into a workspace, and it cannot be given to them: the bug-fix
/// options open the bug-fix protocol only.
#[test]
fn a_seed_directory_cannot_be_given_to_the_committed_release_bindings() {
    let c = Committed::new("release-seed", &["sdc-release.yaml"]);
    let o = c.run("sdc", true, &["--root-protocol", "sdc-release@4", "--case", "REL-1", "--bugfix", &c.p("seed")]);
    let seen = c.summary("committed sdc-release.yaml alone, --root-protocol sdc-release@4, --bugfix <seed>, caps", &o);
    assert_ne!(code(&o), 0, "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
    assert!(!c.root.join("runs/r1/work/mainline").exists(), "{seen}");
}

/// The release bindings as committed, alone, under `local` with caps of 0 and no ledger: they hold no
/// model node, and a bound program has the network under `local`, so the run is refused by name before
/// anything is written and no program of the bindings starts.
#[test]
fn the_committed_release_bindings_alone_are_refused_under_local_without_caps() {
    let c = Committed::new("release-refused", &["sdc-release.yaml"]);
    let o = c.run("sdc", false, &["--root-protocol", "sdc-release@4", "--case", "REL-1"]);
    let seen = c.summary("committed sdc-release.yaml alone, caps 0, no ledger, local", &o);
    assert_eq!(code(&o), 2, "{seen}");
    assert!(stderr(&o).contains("--confinement local") && stderr(&o).contains("--spend-ledger"), "{seen}");
    assert!(!c.root.join("runs/r1").exists(), "{seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
    assert!(c.called("cargo").is_empty() && c.called("slot").is_empty(), "{seen}");
}

/// The release bindings as committed, alone, under `local` with caps: the run starts. Its workspace
/// holds what the runner wrote and nothing else, so the step that would run a tree's own `task check`
/// (`conform-run release`) has no tree to run.
#[test]
fn the_committed_release_bindings_alone_start_with_caps_and_have_no_code_tree_to_run() {
    let c = Committed::new("release", &["sdc-release.yaml"]);
    let o = c.run("sdc", true, &["--root-protocol", "sdc-release@4", "--case", "REL-1"]);
    let seen = c.summary("committed sdc-release.yaml alone, --root-protocol sdc-release@4, caps", &o);
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(c.root.join("runs/r1/outcome.json")).unwrap_or_else(|e| panic!("no outcome.json ({e}): {seen}"))).unwrap();
    assert_eq!(outcome["has_model_node"], false, "{seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
    assert!(c.called("cargo").is_empty(), "{seen}");
    assert!(!c.called("slot").contains("task"), "{seen}");
}
