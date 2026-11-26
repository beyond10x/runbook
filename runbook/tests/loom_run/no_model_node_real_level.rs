//! A run without a model node on the real Loom as a library (a rig of the engine fixture, the draft case as
//! the root): what a bound program is given and reaches under `substrate` without caps, and what the
//! login leaves in the argv of a bound program and in the files of a run under `local` with caps. A login
//! file of invented bytes, sh programs, no model call, no network request.

use super::core::{code, grep_tree, stderr, BIN};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const LOGIN_TEXT: &str = "STAND-IN-LOGIN-TEXT-9917";

struct Rig {
    root: PathBuf,
    level: &'static str,
    caps: bool,
    loom: Option<PathBuf>,
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

fn list_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() && !std::fs::symlink_metadata(&p).unwrap().file_type().is_symlink() {
            list_files(&p, found);
        } else {
            found.push(p);
        }
    }
}

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Rig {
    fn new(tag: &str, level: &'static str, caps: bool, loom: Option<PathBuf>) -> Rig {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zc-rig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["runs", "xdg", "bin", "login"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::set_permissions(root.join("xdg"), std::fs::Permissions::from_mode(0o700)).unwrap();
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-runbook"), &root.join("fx"));
        std::fs::remove_file(root.join("fx/bindings/engine-demo.yaml")).unwrap();
        for tool in ["draft-check.sh", "draft-hold.sh", "draft-write.sh"] {
            std::fs::set_permissions(root.join("fx/bindings/tools").join(tool), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(root.join("idea.yaml"), "text: a note on tea\n").unwrap();
        std::fs::write(root.join("login/credentials.json"), LOGIN_TEXT).unwrap();
        super::decider::install_routing_table_at(&root.join("fx"));
        let rig = Rig { root, level, caps, loom };
        let approve = json!({"approve": true, "cost_micro_usd": 0});
        let mut v = json!({"login_file": rig.login(), "confinement": level, "decider": {"kind": "scripted", "max_call_micro_usd": 0, "answers": [approve, approve, approve, approve]}});
        if rig.loom.is_none() {
            v["in_process"] = json!(true);
        }
        std::fs::write(rig.root.join("settings.toml"), super::core::toml_text(&v)).unwrap();
        rig
    }

    fn login(&self) -> String {
        self.root.join("login/credentials.json").display().to_string()
    }

    fn tool(&self, name: &str, script: &str) {
        write_exe(&self.root.join("fx/bindings/tools").join(name), script);
    }

    fn work(&self) -> PathBuf {
        self.root.join("runs/r1/work")
    }

    fn run(&self) -> Output {
        let r = |p: &str| self.root.join(p).display().to_string();
        let mut args: Vec<String> = [
            "run", "sdc", "--run", "r1", "--runs-dir", &r("runs"), "--bindings", &r("fx/bindings"), "--protocols", &r("fx/protocol"), "--root-protocol", "engine-draft@1", "--case", "DRAFT-1", "--intake", &r("idea.yaml"),
            "--confinement", self.level, "--settings", &r("settings.toml"), "--max-wall-seconds", "50",
        ]
        .map(String::from)
        .to_vec();
        if self.caps {
            args.extend(["--spend-ledger".to_owned(), r("ledger.json"), "--cap-micro-usd".into(), "5000000".into(), "--max-llm-calls".into(), "100".into()]);
        }
        if let Some(loom) = &self.loom {
            let digest = Command::new("sha256sum").arg(loom).output().unwrap();
            let digest = String::from_utf8_lossy(&digest.stdout).split_whitespace().next().unwrap().to_owned();
            args.extend(["--loom".into(), loom.display().to_string(), "--loom-sha256".into(), digest]);
        }
        Command::new(BIN)
            .args(args)
            .env_clear().envs(crate::kept())
            .env("RUNBOOK_NO_CONFIG", "1")
            .env("PATH", format!("{}:{}", r("bin"), std::env::var("PATH").unwrap_or_default()))
            .env("HOME", "/marker-home-dir")
            .env("XDG_RUNTIME_DIR", self.root.join("xdg"))
            .env("ANTHROPIC_API_KEY", "sk-fake-token-9918")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

/// The probe a bound program runs: what it is given, what it sees of the network and of the host,
/// and what it can write.
const PROBE: &str = r#"#!/bin/sh
env > probe-env.txt 2>/dev/null
pwd > probe-cwd.txt 2>/dev/null
printf '%s\n' "$0" "$@" > probe-argv.txt 2>/dev/null
{
echo PROBE-BEGIN
echo "if=$(sed -n 's/^ *\([^ :]*\):.*/\1/p' /proc/net/dev 2>/dev/null | tr '\n' ',')"
echo "routes=$(sed 1d /proc/net/route 2>/dev/null | grep -c .)"
echo "env=$(env | sed 's/=.*//' | tr '\n' ',')"
echo "root=$(ls -a / 2>&1 | tr '\n' ',' | cut -c1-200)"
echo "resolv=$([ -e /etc/resolv.conf ] && echo yes || echo no)"
echo "etc=$(ls /etc 2>/dev/null | grep -c .)"
echo "run=$(ls /run 2>/dev/null | grep -c .)"
echo "var=$(ls /var 2>/dev/null | grep -c .)"
echo "home=$(ls /home 2>/dev/null | grep -c .)"
echo "sockets=$(find /run /tmp /var -maxdepth 3 -type s 2>/dev/null | grep -c .)"
echo "cwd=$(pwd)"
echo "here=$(ls -a . 2>&1 | tr '\n' ',' | cut -c1-160)"
echo "up=$(ls -a .. 2>&1 | tr '\n' ',' | cut -c1-160)"
echo "mounts=$(grep -c . /proc/self/mounts 2>/dev/null)"
echo "rw=$(awk '$4 ~ /^rw/ {printf "%s,", $2}' /proc/self/mounts 2>/dev/null | cut -c1-300)"
(echo '# rewritten' >> ../bindings/engine-draft.yaml) 2>/dev/null && echo copy=written || echo copy=refused
(cat ../store/tree.json > /dev/null) 2>/dev/null && echo manifest=read || echo manifest=unread
(echo '# rewritten' >> '@COPY@') 2>/dev/null && echo copyabs=written || echo copyabs=refused
(cat '@MANIFEST@' > /dev/null) 2>/dev/null && echo manifestabs=read || echo manifestabs=unread
(echo written > '@OUTSIDE@') 2>/dev/null && echo outside=written || echo outside=refused
(echo x > probe-w.txt) 2>/dev/null && echo work=written || echo work=refused
echo PROBE-END
} >&2
echo draft >> draft.txt
"#;

/// The probe a bound program runs: what it is given, what it sees of the network and of the host,
/// and what it can write. It writes files into its workspace and one block on standard error (Loom
/// keeps the output of a program in the store), and it makes no network request.
fn probe(outside: &Path) -> String {
    let run = outside.parent().unwrap().join("runs/r1");
    PROBE
        .replace("@OUTSIDE@", &outside.display().to_string())
        .replace("@COPY@", &run.join("bindings/engine-draft.yaml").display().to_string())
        .replace("@MANIFEST@", &run.join("store/tree.json").display().to_string())
}

/// The block of the probe that `store/blobs/` holds, as `name=value` lines; empty when no program
/// wrote one.
fn probe_block(store: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if store.join("blobs").is_dir() {
        list_files(&store.join("blobs"), &mut files);
    }
    for f in files {
        let text = String::from_utf8_lossy(&std::fs::read(&f).unwrap_or_default()).into_owned();
        if let Some(at) = text.find("PROBE-BEGIN") {
            return text[at..].lines().skip(1).take_while(|l| *l != "PROBE-END").map(|l| l.split_once('=').map_or((l.to_owned(), String::new()), |(k, v)| (k.to_owned(), v.to_owned()))).collect();
        }
    }
    Vec::new()
}

/// `--confinement substrate`, Loom as a library of the pinned revision, a run without a model node
/// and without caps. Where the level can be given (a cgroup this process may delegate, as inside a
/// job of the build pool), the bound program really runs under Substrate and the probe says what it
/// has: no interface beyond loopback, no route, no resolver file, no socket of the host, no write to
/// the copy of the bindings or outside the run. Where it cannot be given, the run is refused before
/// anything is written and no bound program starts: it is not run under something weaker.
#[test]
fn real_loom_under_substrate_without_caps_a_bound_program_has_no_network() {
    let rig = Rig::new("lib-substrate", "substrate", false, None);
    let outside = rig.root.join("written-outside");
    rig.tool("draft-write.sh", &probe(&outside));
    let o = rig.run();
    println!("the cgroup of the test: {}", std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default().trim());
    println!("library, substrate, caps 0: exit {}, stderr: {}", code(&o), stderr(&o).trim());
    if !rig.root.join("runs/r1").exists() {
        assert_eq!(code(&o), 2, "{}", stderr(&o));
        assert!(stderr(&o).contains("substrate"), "{}", stderr(&o));
        assert!(!rig.root.join("ledger.json").exists() && !rig.root.join("xdg/runbook").exists());
        assert!(!outside.exists(), "a bound program started");
        println!("the level cannot be given here: refused before anything was written");
        return;
    }
    let block = probe_block(&rig.root.join("runs/r1/store"));
    for (k, v) in &block {
        println!("probe {k}={v}");
    }
    assert!(!block.is_empty(), "the run started under substrate and the store holds no output of the probe");
    let get = |name: &str| block.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("?");
    assert_eq!(get("if"), "lo,", "interfaces of a bound program under substrate");
    assert_eq!(get("routes"), "0", "routes of a bound program under substrate");
    assert_eq!(get("resolv"), "no", "a resolver file is in the view of a bound program under substrate");
    assert_eq!(get("sockets"), "0", "a socket file is in the view of a bound program under substrate");
    assert_eq!(get("run"), "0", "the host's /run is in the view of a bound program under substrate");
    assert_eq!(get("etc"), "0", "the host's /etc is in the view of a bound program under substrate");
    assert_eq!(get("var"), "0", "the host's /var is in the view of a bound program under substrate");
    assert_eq!(get("manifestabs"), "unread", "a bound program under substrate read the tree manifest");
    // the workspace is read-only except `target/`: the probe cannot write into it, and the fixture's
    // writer cannot write its draft, so the run ends stuck
    let writable: Vec<&str> = get("rw").split(',').filter(|m| m.starts_with("/workspace")).collect();
    assert_eq!(get("work"), "refused", "a bound program under substrate wrote into the workspace");
    assert_eq!(writable, ["/workspace/target"], "the writable mounts of the workspace under substrate");
    assert_eq!(code(&o), 3, "the fixture's writer cannot write its draft under substrate, so the run ends stuck: {}", stderr(&o));
    // the root of the sandbox is its own: a write to a host path lands there and not on the host
    let copy = std::fs::read_to_string(rig.root.join("runs/r1/bindings/engine-draft.yaml")).unwrap();
    assert!(!copy.contains("# rewritten"), "a bound program under substrate wrote the copy of the bindings");
    assert!(!outside.exists(), "a bound program under substrate wrote outside the run");
    let other: Vec<&str> = get("env").split(',').filter(|k| !k.is_empty() && !["PATH", "PWD", "OLDPWD", "SHLVL", "_"].contains(k) && !k.starts_with("LOOM_")).collect();
    assert!(other.is_empty(), "a bound program under substrate is given more than PATH and LOOM_*: {other:?}");
    let outcome: Value = serde_json::from_str(&std::fs::read_to_string(rig.root.join("runs/r1/outcome.json")).unwrap()).unwrap();
    assert_eq!((outcome["has_model_node"].as_bool(), outcome["llm_calls"].as_u64(), outcome["spent_micro_usd"].as_u64()), (Some(false), Some(0), Some(0)), "{outcome}");
    assert!(!rig.root.join("ledger.json").exists() && !rig.root.join("xdg/runbook").exists(), "a ledger or a private directory was made");
}

/// Two assertions of a run on the real library that no other case holds:
/// the argv of a bound program holds nothing of the login,
/// and no file of the run directory holds the path of the login file. Under `local` with caps, where
/// such a run starts on this host.
#[test]
fn real_loom_under_local_with_caps_the_argv_of_a_bound_program_and_the_run_files_hold_no_path_to_the_login() {
    let rig = Rig::new("lib-local", "local", true, None);
    let outside = rig.root.join("written-outside");
    rig.tool("draft-write.sh", &probe(&outside));
    let o = rig.run();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let read = |name: &str| std::fs::read_to_string(rig.work().join(name)).unwrap_or_default();
    let env = read("probe-env.txt");
    let names: Vec<&str> = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)).filter(|k| !["PWD", "OLDPWD", "SHLVL", "_"].contains(k)).collect();
    println!("environment of the bound program: {names:?}\nargv: {}", read("probe-argv.txt").replace('\n', " "));
    let other: Vec<&&str> = names.iter().filter(|k| **k != "PATH" && !k.starts_with("LOOM_")).collect();
    assert!(other.is_empty(), "the bound program is given more than PATH and LOOM_*: {other:?}");
    assert!(!read("probe-argv.txt").is_empty(), "the probe did not run");
    let login = rig.login();
    for needle in [login.as_str(), LOGIN_TEXT, "credentials.json", "sk-fake-token-9918", "/marker-home-dir"] {
        assert!(!env.contains(needle), "{needle} is in the environment of the bound program");
        assert!(!read("probe-argv.txt").contains(needle), "{needle} is in the argv of the bound program");
    }
    for needle in [login.as_str(), LOGIN_TEXT, "sk-fake-token-9918"] {
        let mut found = Vec::new();
        grep_tree(&rig.root.join("runs/r1"), needle, &mut found);
        assert!(found.is_empty(), "files of the run directory hold {needle}: {found:?}");
        assert!(!String::from_utf8_lossy(&o.stdout).contains(needle) && !stderr(&o).contains(needle), "{needle} is on a stream of the runner");
    }
    assert!(!rig.root.join("xdg/runbook").exists(), "a private directory was made");
    // the run's own home of the Loom child: nothing of a login in it
    let home = rig.root.join("runs/r1/home");
    if home.exists() {
        let mut files = Vec::new();
        list_files(&home, &mut files);
        println!("files below the run's home: {files:?}");
    }
}

/// What the runner does on this host with `--confinement substrate` and a `b10x-loom` child, without
/// a model node and without caps. `RUNBOOK_TEST_LOOM_BIN` names a built `b10x-loom` (read only). When
/// Loom refuses the level no bound program has run; when it runs one, that program sees no interface
/// beyond loopback, cannot write the copy of the bindings and cannot write outside the run.
#[test]
#[ignore = "measurement: set RUNBOOK_TEST_LOOM_BIN to a built b10x-loom"]
fn child_loom_under_substrate_without_caps_runs_no_bound_program_with_a_network() {
    let loom = PathBuf::from(std::env::var("RUNBOOK_TEST_LOOM_BIN").expect("RUNBOOK_TEST_LOOM_BIN names a built b10x-loom"));
    let rig = Rig::new("child-substrate", "substrate", false, Some(loom));
    let outside = rig.root.join("written-outside");
    rig.tool("draft-write.sh", &probe(&outside));
    let copy_before = std::fs::read_to_string(rig.root.join("fx/bindings/engine-draft.yaml")).unwrap();
    let o = rig.run();
    let outcome = std::fs::read_to_string(rig.root.join("runs/r1/outcome.json")).unwrap_or_default();
    println!("the cgroup of the test: {}", std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default().trim());
    println!("child, substrate, caps 0: exit {}\nstderr: {}\noutcome: {outcome}", code(&o), stderr(&o).trim());
    let block = probe_block(&rig.root.join("runs/r1/store"));
    let ran = !block.is_empty() || rig.work().join("probe-env.txt").exists();
    println!("a bound program ran: {ran}");
    if !ran {
        assert_ne!(code(&o), 0, "no bound program ran and the run ended with exit 0");
        assert!(!outside.exists(), "a bound program wrote outside the run");
        assert!(!rig.root.join("ledger.json").exists() && !rig.root.join("xdg/runbook").exists());
        return;
    }
    for (k, v) in &block {
        println!("probe {k}={v}");
    }
    let get = |name: &str| block.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("?");
    assert_eq!(get("if"), "lo,", "interfaces of a bound program under substrate");
    assert_eq!(get("resolv"), "no");
    assert_eq!(get("copy"), "refused");
    assert!(!outside.exists(), "a bound program under substrate wrote outside the run");
    let copy_after = std::fs::read_to_string(rig.root.join("runs/r1/bindings/engine-draft.yaml")).unwrap_or_default();
    assert!(!copy_after.contains("# rewritten"), "a bound program under substrate wrote the copy of the bindings");
    assert!(!copy_before.contains("# rewritten"));
}
