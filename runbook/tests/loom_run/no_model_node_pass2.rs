//! Security review, pass 2, of `run-without-model-node-needs-no-caps`: the cases of the review.
//!
//! Programs here are sh scripts with invented text. A "model program" is a stand-in that writes a
//! marker file; "workspace code" is a hand-written Taskfile whose check writes a marker file. No
//! model call, no network, no real login file.

use super::core::{code, stderr, World, BIN};
use super::fake::tree_run_json;
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

/// Every regular file below `dir` whose bytes hold `needle`.
fn files_holding(dir: &Path, needle: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            files_holding(&path, needle, found);
        } else if meta.is_file() {
            if let Ok(bytes) = std::fs::read(&path) {
                if bytes.windows(needle.len()).any(|w| w == needle.as_bytes()) {
                    found.push(path.display().to_string());
                }
            }
        }
    }
}

// ---- claim 3: the committed bindings that run a workspace's tests ----

/// A run on a reduced directory of the committed bindings, files unchanged, with Loom as a
/// library, caps of 0 and no ledger. `bin/` is first on the PATH of the run: a stand-in seed
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

    /// Run `r1` with caps of 0 and no ledger.
    fn run(&self, runbook: &str, extra: &[&str]) -> Output {
        let mut args: Vec<String> = ["run", runbook, "--run", "r1", "--runs-dir"].iter().map(|s| (*s).to_owned()).collect();
        args.extend([self.p("runs"), "--bindings".into(), self.p("fx/bindings"), "--protocols".into(), self.p("fx/protocol"), "--intake".into(), self.p("idea.yaml")]);
        args.extend(["--confinement".into(), "local".into(), "--settings".into(), self.p("settings.json")]);
        args.extend(["--cap-micro-usd", "0", "--max-llm-calls", "0", "--max-wall-seconds", "40"].map(String::from));
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
fn claim3_the_committed_bug_fix_bindings_with_a_seed_do_not_run_without_a_ledger() {
    let c = Committed::new("bugfix", &["sdc-bugfix.yaml"]);
    let o = c.run("sdc-bugfix", &["--bugfix", &c.p("seed")]);
    let seen = c.summary("committed sdc-bugfix.yaml alone, --bugfix <seed>, caps 0, no ledger", &o);
    assert_eq!(code(&o), 2, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(!c.root.join("runs/r1").exists(), "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
}

/// The same for the story bindings as committed (`story-tests impl`), and for the whole committed
/// directory with a seed.
#[test]
fn claim3_the_committed_story_bindings_and_the_whole_directory_do_not_run_without_a_ledger() {
    let c = Committed::new("story", &["sdc-story.yaml"]);
    let o = c.run("sdc", &["--root-protocol", "sdc-story@8", "--case", "STORY-1"]);
    let seen = c.summary("committed sdc-story.yaml alone, caps 0, no ledger", &o);
    assert_eq!(code(&o), 2, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(!c.root.join("runs/r1").exists(), "{seen}");

    let all = ["sdc-bugfix.yaml", "sdc-direction.yaml", "sdc-epic.yaml", "sdc-intake.yaml", "sdc-plan.yaml", "sdc-release.yaml", "sdc-spec.yaml", "sdc-story.yaml", "sdc.yaml"];
    let c = Committed::new("whole", &all);
    let o = c.run("sdc-bugfix", &["--bugfix", &c.p("seed")]);
    let seen = c.summary("the whole committed bindings directory, --bugfix <seed>, caps 0, no ledger", &o);
    assert_eq!(code(&o), 2, "{seen}");
    assert!(stderr(&o).contains("--spend-ledger is required"), "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
}

/// The release bindings as committed hold no model node. A seed directory is the one way code that
/// no bound program wrote gets into a workspace, and it cannot be given to them: the bug-fix
/// options open the bug-fix protocol only.
#[test]
fn claim3_a_seed_directory_cannot_be_given_to_the_committed_release_bindings() {
    let c = Committed::new("release-seed", &["sdc-release.yaml"]);
    let o = c.run("sdc", &["--root-protocol", "sdc-release@4", "--case", "REL-1", "--bugfix", &c.p("seed")]);
    let seen = c.summary("committed sdc-release.yaml alone, --root-protocol sdc-release@4, --bugfix <seed>, caps 0, no ledger", &o);
    assert_ne!(code(&o), 0, "{seen}");
    assert!(c.called("sdc-bugfix").is_empty(), "the seed ran: {seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
    assert!(!c.root.join("runs/r1/work/mainline").exists(), "{seen}");
}

/// The release bindings as committed, alone, with caps of 0 and no ledger: the run opens without
/// caps. Its workspace holds what the runner wrote and nothing else, so the step that would run a
/// tree's own `task check` (`conform-run release`) has no tree to run.
#[test]
fn claim3_the_committed_release_bindings_alone_open_without_caps_and_have_no_code_tree_to_run() {
    let c = Committed::new("release", &["sdc-release.yaml"]);
    let o = c.run("sdc", &["--root-protocol", "sdc-release@4", "--case", "REL-1"]);
    let seen = c.summary("committed sdc-release.yaml alone, --root-protocol sdc-release@4, caps 0, no ledger", &o);
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(c.root.join("runs/r1/outcome.json")).unwrap_or_else(|e| panic!("no outcome.json ({e}): {seen}"))).unwrap();
    assert_eq!(outcome["has_model_node"], false, "{seen}");
    assert!(!c.root.join("ledger.json").exists(), "{seen}");
    assert!(!c.workspace_code_ran(), "{seen}");
    assert!(c.called("cargo").is_empty(), "{seen}");
    assert!(!c.called("slot").contains("task"), "{seen}");
}

// ---- Loom itself on the draft case of the engine fixture ----

struct Lib {
    root: PathBuf,
    /// The Loom binary of a run through the child process; `None`: Loom as a library.
    loom: Option<PathBuf>,
}

const LOGIN_TEXT: &str = "STAND-IN-LOGIN-TEXT-9204";
const NAME_THE_TOOL: &str = "s#\\[tools/draft-check.sh\\]#[sdc-node, draft.check]#";

impl Lib {
    fn new(tag: &str, loom: Option<PathBuf>) -> Lib {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review2-lib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["runs", "xdg", "bin", "login"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        // the root of the fixture opens a child case, and an `open` executor counts as a model node
        std::fs::remove_file(root.join("fx/bindings/engine-demo.yaml")).unwrap();
        for tool in ["draft-check.sh", "draft-hold.sh", "draft-write.sh"] {
            std::fs::set_permissions(root.join("fx/bindings/tools").join(tool), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        std::fs::write(root.join("login/credentials.json"), LOGIN_TEXT).unwrap();
        super::decider::install_routing_table_at(&root.join("fx"));
        let lib = Lib { root, loom };
        lib.settings(&json!({}));
        // the stand-in for the model tool: the marker, and the cost record of a call of 0.25 USD
        let text = format!(
            "#!/bin/sh\necho started >> '{}'\nw=.sdc/llm; mkdir -p \"$w/001-draft.check\"\nprintf '{{\"action\":\"draft.check\",\"record\":\"%s/%s/001-draft.check\",\"cost_usd\":0.25}}\\n' \"$PWD\" \"$w\" >> \"$w/calls.jsonl\"\n[ -s draft.txt ]\n",
            lib.marker().display()
        );
        write_exe(&lib.root.join("bin/sdc-node"), &text);
        lib
    }

    fn settings(&self, extra: &Value) {
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let loom = if self.loom.is_some() { json!({"confinement": "local"}) } else { json!({"in_process": true, "confinement": "local"}) };
        let mut v = json!({"loom": loom, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        std::fs::write(self.root.join("settings.json"), serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    fn marker(&self) -> PathBuf {
        self.root.join("marker-model-program")
    }

    fn tool(&self, name: &str, script: &str) {
        write_exe(&self.root.join("fx/bindings/tools").join(name), script);
    }

    fn bindings(&self, change: impl Fn(&str) -> String) {
        let path = self.root.join("fx/bindings/engine-draft.yaml");
        let text = std::fs::read_to_string(&path).unwrap();
        let changed = change(&text);
        assert_ne!(changed, text, "the bindings were not changed");
        std::fs::write(path, changed).unwrap();
    }

    fn run_dir(&self) -> PathBuf {
        self.root.join("runs/r1")
    }

    fn work(&self) -> PathBuf {
        self.run_dir().join("work")
    }

    /// Run `r1` with caps of 0 and no ledger.
    fn run(&self) -> Output {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut args = vec![
            "run".to_owned(), "sdc".into(), "--run".into(), "r1".into(), "--runs-dir".into(), r("runs"), "--bindings".into(), r("fx/bindings"), "--protocols".into(), r("fx/protocol"),
            "--root-protocol".into(), "engine-draft@1".into(), "--case".into(), "DRAFT-1".into(), "--intake".into(), r("idea.yaml"), "--confinement".into(), "local".into(),
            "--settings".into(), r("settings.json"), "--cap-micro-usd".into(), "0".into(), "--max-llm-calls".into(), "0".into(), "--max-wall-seconds".into(), "50".into(),
        ];
        if let Some(loom) = &self.loom {
            let digest = Command::new("sha256sum").arg(loom).output().unwrap();
            let digest = String::from_utf8_lossy(&digest.stdout).split_whitespace().next().unwrap().to_owned();
            args.extend(["--loom".into(), loom.display().to_string(), "--loom-sha256".into(), digest]);
        }
        Command::new(BIN)
            .args(args)
            .env_clear()
            .env("PATH", format!("{}:{}", r("bin"), std::env::var("PATH").unwrap_or_default()))
            .env("HOME", "/marker-home-dir")
            .env("XDG_RUNTIME_DIR", self.root.join("xdg"))
            .env("ANTHROPIC_API_KEY", "sk-fake-token-5517")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn outcome(&self) -> Value {
        std::fs::read_to_string(self.run_dir().join("outcome.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
    }

    fn seen(&self, o: &Output) -> String {
        let outcome = self.outcome();
        format!(
            "exit {}, model program started: {}, outcome {}, has_model_node {}, llm_calls {}, spent_micro_usd {}, cause {}, steps {}; stderr: {}",
            code(o),
            self.marker().exists(),
            outcome["outcome"],
            outcome["has_model_node"],
            outcome["llm_calls"],
            outcome["spent_micro_usd"],
            outcome["cause"],
            outcome["steps"],
            stderr(o).lines().last().unwrap_or("")
        )
    }
}

/// The first step's writer, run once: `change` is a shell snippet run in the work directory (the
/// run directory is `..`, `$run` its absolute path) after `../alt` was made, a copy of the
/// bindings that names the model tool for the step after it.
fn manifest_writer(change: &str) -> String {
    format!(
        "#!/bin/sh\nif [ ! -e ../done ]; then\n  touch ../done\n  cp -r ../bindings ../alt\n  sed -i '{NAME_THE_TOOL}' ../alt/engine-draft.yaml\n  run=$(cd .. && pwd)\n  {change}\nfi\necho draft >> draft.txt\n"
    )
}

/// Forms of the tree manifest the fix round's cases do not hold. For each: no model program
/// starts, and the run ends the way named (13: refused for the manifest; 0: the manifest still
/// names the root file of the copy for the runner and for Loom, and the run goes on).
fn manifest_forms(tag: &str, forms: &[(&str, &str, i32)]) {
    let mut wrong: Vec<String> = Vec::new();
    for (n, (what, change, exit)) in forms.iter().enumerate() {
        let lib = Lib::new(&format!("{tag}-{n}"), None);
        lib.tool("draft-write.sh", &manifest_writer(change));
        let o = lib.run();
        let manifest = std::fs::read_to_string(lib.run_dir().join("store/tree.json")).unwrap_or_default();
        if !lib.run_dir().join("done").exists() {
            wrong.push(format!("{what}: the writer did not run: {}", lib.seen(&o)));
        }
        if lib.marker().exists() {
            wrong.push(format!("{what}: the model program started: {}", lib.seen(&o)));
        }
        if code(&o) != *exit {
            wrong.push(format!("{what}: exit {} and not {exit}: {}; the manifest: {manifest}", code(&o), lib.seen(&o)));
        }
        if *exit == 13 && lib.outcome()["cause"] != "model_call_without_model_node" {
            wrong.push(format!("{what}: the cause is {}", lib.outcome()["cause"]));
        }
    }
    assert!(wrong.is_empty(), "forms of the manifest:\n{}", wrong.join("\n"));
}

#[test]
fn claim1_a_manifest_with_two_bindings_members_or_a_member_that_is_no_text_starts_no_model_program() {
    manifest_forms(
        "manifest-a",
        &[
            ("two bindings members, the first names other bindings and the last the root file", r##"sed -i "1 s#^{#{\"bindings\":\"$run/alt/engine-draft.yaml\",#" ../store/tree.json"##, 0),
            ("two bindings members, the last names other bindings", r##"t=$(cat ../store/tree.json); printf '%s' "${t%\}*}" > ../store/tree.json; printf ',"bindings":"%s"}\n' "$run/alt/engine-draft.yaml" >> ../store/tree.json"##, 13),
            ("a bindings member that is a list holding the path", r##"sed -i "s#\"bindings\": *\"\([^\"]*\)\"#\"bindings\":[\"$run/alt/engine-draft.yaml\"]#" ../store/tree.json"##, 13),
        ],
    );
}

#[test]
fn claim1_a_manifest_that_names_a_hard_link_or_goes_through_a_directory_link_starts_no_model_program() {
    manifest_forms(
        "manifest-b",
        &[
            (
                "a hard link to the root file in a directory whose tools are other programs",
                r##"rm ../alt/engine-draft.yaml; ln ../bindings/engine-draft.yaml ../alt/engine-draft.yaml; cp "$(command -v sdc-node)" ../alt/tools/draft-check.sh; sed -i "s#\"bindings\": *\"[^\"]*\"#\"bindings\":\"$run/alt/engine-draft.yaml\"#" ../store/tree.json"##,
                13,
            ),
            (
                "a path through a directory link that resolves to the root file",
                r##"ln -s bindings ../lnk; sed -i "s#\"bindings\": *\"[^\"]*\"#\"bindings\":\"$run/lnk/engine-draft.yaml\"#" ../store/tree.json"##,
                0,
            ),
        ],
    );
}

/// What a bound program of a run without a model node is given and can reach under
/// `--confinement local`, with a login file named in the settings: its environment, its working
/// directory, the files of the run, and a file outside the run directory.
#[test]
fn claim2_and_3_what_a_bound_program_of_a_run_without_a_model_node_is_given_and_reaches() {
    let lib = Lib::new("probe", None);
    let login = lib.root.join("login/credentials.json").display().to_string();
    lib.settings(&json!({"login_file": login}));
    let outside = lib.root.join("probe-outside").display().to_string();
    lib.tool(
        "draft-write.sh",
        &format!("#!/bin/sh\nenv > probe-env.txt\npwd > probe-cwd.txt\necho \"$@\" > probe-argv.txt\necho written > '{outside}' 2> probe-outside-error.txt\nls -a .. > probe-run-dir.txt\necho draft >> draft.txt\n"),
    );
    let o = lib.run();
    assert_eq!(code(&o), 0, "{}", lib.seen(&o));
    assert_eq!(lib.outcome()["has_model_node"], false);
    let read = |name: &str| std::fs::read_to_string(lib.work().join(name)).unwrap_or_default();
    let env = read("probe-env.txt");
    let names: Vec<&str> = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).filter(|k| !["PWD", "OLDPWD", "SHLVL", "_"].contains(k)).collect();
    println!("environment of the bound program: {names:?}\nworking directory: {}run directory: {}outside write: {:?}", read("probe-cwd.txt"), read("probe-run-dir.txt").replace('\n', " "), std::fs::read_to_string(&outside));
    let other: Vec<&&str> = names.iter().filter(|k| **k != "PATH" && !k.starts_with("LOOM_")).collect();
    assert!(other.is_empty(), "the bound program is given more than PATH and LOOM_*: {other:?}\n{env}");
    for needle in ["sk-fake-token-5517", "/marker-home-dir", LOGIN_TEXT, "credentials.json"] {
        assert!(!env.contains(needle), "{needle} is in the environment of the bound program:\n{env}");
        assert!(!read("probe-argv.txt").contains(needle), "{needle} is in the argv of the bound program");
    }
    assert_eq!(read("probe-cwd.txt").trim(), lib.work().display().to_string(), "the working directory is the workspace of the run");
    // nothing of the run names the login file or holds its text, and no private directory was made
    for needle in [login.as_str(), LOGIN_TEXT] {
        let mut found = Vec::new();
        files_holding(&lib.run_dir(), needle, &mut found);
        assert!(found.is_empty(), "files of the run directory hold {needle}: {found:?}");
    }
    assert!(!lib.root.join("xdg/runbook").exists(), "a private directory was made under XDG_RUNTIME_DIR");
    // local is not confined: the program writes outside the run directory as the user
    assert_eq!(std::fs::read_to_string(&outside).unwrap_or_default().trim(), "written", "{}", read("probe-outside-error.txt"));
}

// ---- claim 1, the handoff: a `tree run` of more than one step ----

const HANDOFF_WAIT: &str = "#!/bin/sh\n[ -e ../waited ] && exit 1\ntouch ../waited\nexit 0\n";

fn loom_binary() -> PathBuf {
    PathBuf::from(std::env::var("REVIEW2_LOOM_BIN").expect("REVIEW2_LOOM_BIN names the b10x-loom binary of the pinned Loom revision"))
}

/// The control of the handoff case below, through the Loom child process: the writer rewrites the
/// copy, the next call is a `tree run --max-steps 1` of the runner, and the check before it refuses.
#[test]
#[ignore = "needs REVIEW2_LOOM_BIN, the b10x-loom binary of the pinned Loom revision"]
fn child_loom_a_copy_rewritten_by_a_bound_program_is_refused_before_the_next_step() {
    let lib = Lib::new("child-control", Some(loom_binary()));
    lib.tool("draft-write.sh", &format!("#!/bin/sh\nsed -i '{NAME_THE_TOOL}' ../bindings/engine-draft.yaml\necho draft >> draft.txt\n"));
    let o = lib.run();
    assert!(std::fs::read_to_string(lib.run_dir().join("bindings/engine-draft.yaml")).unwrap_or_default().contains("sdc-node"), "the writer did not rewrite the copy: {}", lib.seen(&o));
    assert!(!lib.marker().exists(), "the model program started: {}", lib.seen(&o));
    assert_eq!(code(&o), 13, "{}", lib.seen(&o));
}

/// A handoff's printed command is `tree run` with no bound on its steps, and the runner runs it as
/// printed. Loom reads the bindings again before every step of one call. The writer that runs as
/// the first step of the handoff's call rewrites the copy so that the next action is the model
/// tool; the runner's check ran before the call. No model program may start.
#[test]
#[ignore = "needs REVIEW2_LOOM_BIN, the b10x-loom binary of the pinned Loom revision"]
fn child_loom_a_copy_rewritten_during_the_tree_run_of_a_handoff_starts_no_model_program() {
    let lib = Lib::new("child-handoff", Some(loom_binary()));
    lib.bindings(|t| t.replace("      argv: [tools/draft-write.sh]\n", "      argv: [tools/draft-write.sh]\n    handoff: \"decide, then run again\"\n    handoff_when: [tools/draft-wait.sh]\n"));
    lib.tool("draft-wait.sh", HANDOFF_WAIT);
    lib.tool("draft-write.sh", &format!("#!/bin/sh\nsed -i '{NAME_THE_TOOL}' ../bindings/engine-draft.yaml\necho draft >> draft.txt\n"));
    let o = lib.run();
    let events = std::fs::read_to_string(lib.run_dir().join("events.jsonl")).unwrap_or_default();
    assert!(events.contains("Handoff"), "the run took no handoff: {}\n{events}", lib.seen(&o));
    assert!(std::fs::read_to_string(lib.run_dir().join("bindings/engine-draft.yaml")).unwrap_or_default().contains("sdc-node"), "the writer did not rewrite the copy: {}", lib.seen(&o));
    assert!(!lib.marker().exists(), "the model program started in the tree run of the handoff: {}", lib.seen(&o));
}

// ---- claim 6: forms the scan cannot classify ----

const NO_MODEL: &str = "format: loom.bindings/1
protocol: sdc@8
actions:
  - action: check.run
    executor:
      kind: program
      argv: [tools/sdc-tool.sh, check-idea]
  - action: scope.approve
    executor: {kind: person, principal: person:owner}
";

fn zero(w: &World) -> Vec<String> {
    let mut a = w.args("r1", 0);
    let i = a.iter().position(|x| x == "--max-llm-calls").unwrap();
    a[i + 1] = "0".to_owned();
    let i = a.iter().position(|x| x == "--spend-ledger").unwrap();
    a.drain(i..=i + 1);
    a
}

fn file(w: &World, rel: &str, text: &str) {
    let path = w.root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn one_action(executor: &str) -> String {
    format!("format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: {executor}\n")
}

#[test]
fn claim6_five_more_forms_count_as_a_model_node() {
    type Setup = Box<dyn Fn(&World)>;
    let forms: Vec<(&str, Setup)> = vec![
        ("an executor kind the scan does not know", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: script, argv: [tools/sdc-tool.sh, x]}")))),
        ("a program given as a list inside the argv", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [[tools/sdc-tool.sh, x], y]}")))),
        ("the tool behind an anchor that an argv takes by alias", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &format!("x-programs:\n  node: &node [sdc-node, a.b]\n{}", one_action("{kind: program, argv: *node}"))))),
        ("an executor taken whole by alias from another action", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", "format: loom.bindings/1\nprotocol: sdc@8\nactions:\n  - action: a.b\n    executor: &e {kind: agent, model: m}\n  - action: c.d\n    executor: *e\n");
        })),
        ("a child bindings file that is a link to a file outside the copy", Box::new(|w: &World| {
            file(w, "bindings/sdc.yaml", &one_action("{kind: open, name: intake, bindings: child.yaml}"));
            file(w, "elsewhere/child.txt", &one_action("{kind: program, argv: [sdc-node, a.b]}"));
            std::os::unix::fs::symlink(w.root.join("elsewhere/child.txt"), w.root.join("bindings/child.yaml")).unwrap();
        })),
        ("the tool's name with a trailing space", Box::new(|w: &World| file(w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [\"sdc-node \", a.b]}")))),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (what, setup) in &forms {
        let w = World::new("review2-forms");
        setup(&w);
        let o = w.run(&zero(&w));
        if code(&o) != 2 || !stderr(&o).contains("--spend-ledger is required") {
            wrong.push(format!("{what}: exit {} ({})", code(&o), stderr(&o).lines().last().unwrap_or("")));
        }
        if w.run_dir("r1").exists() {
            wrong.push(format!("{what}: a run directory was made"));
        }
    }
    assert!(wrong.is_empty(), "forms that did not count as a model node:\n{}", wrong.join("\n"));
}

/// The tool's name in another letter case is another program name on a file system that tells the
/// cases apart: the scan reads names as they are written. What the run does is printed for the
/// report; the case holds only that the decision is stated.
#[test]
fn claim6_the_tool_in_another_letter_case_is_decided_and_the_decision_is_stated() {
    let w = World::new("review2-case");
    file(&w, "bindings/sdc.yaml", &one_action("{kind: program, argv: [SDC-NODE, a.b]}"));
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&zero(&w));
    let stated = std::fs::read_to_string(w.run_dir("r1").join("outcome.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| v["has_model_node"].clone());
    println!("argv [SDC-NODE, a.b]: exit {}, has_model_node {stated:?}, stderr: {}", code(&o), stderr(&o).lines().last().unwrap_or(""));
    assert!(code(&o) == 2 || stated == Some(Value::Bool(false)), "exit {}: {}", code(&o), stderr(&o));
}

// ---- claim 2: the fake world, whose settings name a login file ----

/// A run without a model node, settings that name a login file: no private directory, no link,
/// and no file of the run directory holds the path of the login file or its text; no call of
/// Loom carries either in its argv or its environment.
#[test]
fn claim2_a_run_without_a_model_node_leaves_no_trace_of_the_login_file() {
    let w = World::new("review2-login");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    w.fake.answer("tree_run", "1", &tree_run_json("BudgetSpent", 1, Some("the run took its 1 steps"), "[]"), 5);
    w.fake.answer("tree_run", "2", &tree_run_json("Terminal", 1, Some("operating"), "[]"), 0);
    let o = w.run(&zero(&w));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(w.outcome("r1")["has_model_node"], false);
    assert!(!w.root.join("xdg/runbook").exists(), "a private directory was made");
    let login = w.login_path().display().to_string();
    for needle in [login.as_str(), "LOGIN-FILE-MARKER-4711", "loginhome"] {
        let mut found = Vec::new();
        files_holding(&w.run_dir("r1"), needle, &mut found);
        assert!(found.is_empty(), "files of the run directory hold {needle}: {found:?}");
        for call in w.fake.calls() {
            assert!(!call.argv.iter().any(|a| a.contains(needle)), "{needle} is in the argv of a call of Loom: {:?}", call.argv);
            assert!(!call.env.iter().any(|(k, v)| k.contains(needle) || v.contains(needle)), "{needle} is in the environment of a call of Loom");
        }
        assert!(!String::from_utf8_lossy(&o.stdout).contains(needle) && !stderr(&o).contains(needle), "{needle} is on a stream of the runner");
    }
}

// ---- claim 5: the manifest check before an approval, which no committed case holds ----

/// The manifest is rewritten while Loom answers the `tree status` of a step (the runner's check of
/// the status ran before it); the next path is the approval. It must be refused for the manifest,
/// and the approval must not be sent to Loom.
#[test]
fn claim5_a_manifest_rewritten_after_the_status_of_a_step_is_refused_before_the_approval() {
    let w = World::new("review2-manifest-approve");
    file(&w, "bindings/sdc.yaml", NO_MODEL);
    let approval = super::fake::decision_json("scope.approve", "ApproveAction", "person:owner");
    w.fake.answer("tree_run", "1", &tree_run_json("AwaitingPerson", 2, None, &format!("[{approval}]")), 3);
    w.fake.hook("tree_status", "1", "printf '{\"bindings\":\"%s/bindings/none.yaml\"}' \"$PWD\" > \"$store/tree.json\"\n");
    w.settings(&json!({"decider": {"kind": "scripted", "answers": [{"approve": true, "cost_micro_usd": 0}]}}));
    let o = w.run(&zero(&w));
    let outcome = w.outcome("r1");
    assert!(w.fake.calls_of(&["tree", "approve"]).is_empty(), "the approval was sent to Loom: exit {}, {outcome}", code(&o));
    assert_eq!(code(&o), 13, "{}", stderr(&o));
    let reason = outcome["reason"].as_str().unwrap_or("");
    assert!(reason.contains("tree manifest") && reason.contains("tree approve"), "{outcome}");
}

// ---- claim 3: the whole committed directory, with caps ----

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .expect("git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// A measurement, not a guard: the committed bug-fix cycle (`bindings/`, `protocol/`, the helpers
/// of `driver/`, the stub adapter, the invented ticket of the committed fixture) on a repository
/// that holds one more test, which writes a marker with its working directory and the names of
/// its environment. With caps and a ledger, as the committed slow-lane case runs it. No model call.
#[test]
#[ignore = "needs driver/target/debug built (`cargo build --locked --bins` in driver/); the stub adapter, no model call"]
fn claim3_with_caps_the_whole_committed_directory_runs_the_code_of_a_ticket_repository() {
    let root = repo();
    assert!(root.join("driver/target/debug/sdc-bugfix").is_file(), "driver/ is not built");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("review2-with-caps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let input = scratch.join("input");
    let fixture = root.join("driver/examples/sdc/fixtures/bugfix-pager");
    copy_tree(&fixture.join("crate"), &input.join("repo"));
    let marker = scratch.join("marker-workspace-code");
    std::fs::create_dir_all(input.join("repo/tests")).unwrap();
    let test = format!(
        "use std::io::Write;\n\n#[test]\nfn marker() {{\n    let mut names: Vec<String> = std::env::vars_os()\n        .map(|(k, _)| k.to_string_lossy().into_owned())\n        .collect();\n    names.sort();\n    let cwd = std::env::current_dir().unwrap();\n    let mut f = std::fs::OpenOptions::new()\n        .create(true)\n        .append(true)\n        .open(\"{}\")\n        .unwrap();\n    writeln!(f, \"cwd {{}} env {{}}\", cwd.display(), names.join(\" \")).unwrap();\n}}\n",
        marker.display()
    );
    std::fs::write(input.join("repo/tests/zz_marker.rs"), test).unwrap();
    git(&input.join("repo"), &["init", "-q"]);
    git(&input.join("repo"), &["add", "."]);
    git(&input.join("repo"), &["commit", "-q", "-m", "fixture"]);
    let head = git(&input.join("repo"), &["rev-parse", "HEAD"]);
    let ticket = std::fs::read_to_string(fixture.join("ticket.yaml")).unwrap().replace("0000000000000000000000000000000000000000", &head);
    std::fs::write(input.join("ticket.yaml"), ticket).unwrap();
    std::fs::write(scratch.join("idea.yaml"), "text: a ticket stands in for the report\n").unwrap();
    let answers: Vec<Value> = (0..3).map(|_| json!({"approve": true, "cost_micro_usd": 0})).collect();
    let settings = json!({"decider": {"kind": "scripted", "principal": "person:owner", "max_call_micro_usd": 0, "answers": answers}});
    std::fs::write(scratch.join("settings.json"), serde_json::to_string_pretty(&settings).unwrap()).unwrap();
    let p = |rel: &str| scratch.join(rel).display().to_string();
    let mut args: Vec<String> = ["run", "sdc-bugfix", "--run", "r1", "--runs-dir"].map(String::from).to_vec();
    args.push(p("runs"));
    args.extend(["--bindings".into(), root.join("bindings").display().to_string(), "--protocols".into(), root.join("protocol").display().to_string()]);
    args.extend(["--intake".into(), p("idea.yaml"), "--confinement".into(), "local".into(), "--settings".into(), p("settings.json"), "--spend-ledger".into(), p("ledger.json")]);
    args.extend(["--cap-micro-usd", "1000000", "--max-llm-calls", "100", "--max-wall-seconds", "50"].map(String::from));
    args.extend(["--bugfix-ticket".into(), p("input/ticket.yaml")]);
    let started = std::time::Instant::now();
    let o = Command::new(BIN)
        .args(&args)
        .env_clear()
        .env("PATH", format!("{}:{}", root.join("driver/target/debug").display(), std::env::var("PATH").unwrap_or_default()))
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("RUNBOOK_HOME", &root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let run = scratch.join("runs/r1");
    let outcome: Value = std::fs::read_to_string(run.join("outcome.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let records = std::fs::read_to_string(run.join("work/.sdc/llm/calls.jsonl")).unwrap_or_default();
    let ledger = std::fs::read_to_string(scratch.join("ledger.json")).unwrap_or_default();
    let written = std::fs::read_to_string(&marker).unwrap_or_default();
    println!(
        "the whole committed directory, --bugfix-ticket, caps 1000000 micro-USD and 100 calls, a ledger, the stub adapter: {:?}\n  exit {}; stderr: {}\n  outcome {}, reason {}, has_model_node {}, because {}, llm_calls {}, spent_micro_usd {}, steps {}\n  cost records of the workspace: {} lines\n  ledger: {}\n  the marker of the repository's own test, one line a run of it:\n{}",
        started.elapsed(),
        code(&o),
        stderr(&o).lines().last().unwrap_or(""),
        outcome["outcome"],
        outcome["reason"],
        outcome["has_model_node"],
        outcome["because"],
        outcome["llm_calls"],
        outcome["spent_micro_usd"],
        outcome["steps"],
        records.lines().count(),
        ledger.replace('\n', " "),
        written.replace(&scratch.display().to_string(), "<root>")
    );
    assert_eq!(outcome["has_model_node"], true, "{outcome}");
    assert!(!written.is_empty(), "the test of the ticket's repository did not run: exit {}, {outcome}", code(&o));
}
