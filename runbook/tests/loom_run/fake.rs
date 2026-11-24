//! A fake `b10x-loom` for the tests of `runbook run`: a shell script the test writes at run
//! time. It records the argv and the environment of every call and answers from a scripted list.
//!
//! Layout of a fake's directory `D`:
//! - `D/b10x-loom`            the script (its sha256 is `Fake::sha256`)
//! - `D/calls/count`          the number of calls so far (a clock for tests: see `calls_so_far`)
//! - `D/calls/<n>.argv`       one argument per line
//! - `D/calls/<n>.env`        the environment, `env` output
//! - `D/script/<sub>.<k>.out|err|exit|sh`  the k-th answer to subcommand `<sub>` (`tree_run`,
//!   `tree_approve`, `tree_open`, `tree_status`, `tree_verify`, `protocols_add`); `<sub>.default.*`
//!   answers every call without an answer of its own; no answer at all: exit 99.
//!   A `.sh` file is sourced first, in the runner's run directory as working directory (the hook
//!   writes cost records into `work/.sdc/llm`, sleeps, and so on).

use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SCRIPT: &str = r#"#!/bin/sh
D='@DIR@'
mkdir -p "$D/calls" "$D/seq"
n=$(cat "$D/calls/count" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$D/calls/count"
printf '%s\n' "$@" > "$D/calls/$n.argv"
env > "$D/calls/$n.env"
sub="$1_$2"
k=$(cat "$D/seq/$sub" 2>/dev/null || echo 0)
k=$((k + 1))
echo "$k" > "$D/seq/$sub"
pick() {
  if [ -e "$D/script/$sub.$k.$1" ]; then echo "$D/script/$sub.$k.$1"
  elif [ -e "$D/script/$sub.default.$1" ]; then echo "$D/script/$sub.default.$1"
  fi
}
if [ -z "$(pick out)$(pick err)$(pick exit)$(pick sh)" ]; then
  echo "fake b10x-loom: no answer scripted for $sub call $k" >&2
  exit 99
fi
store=""; bindings=""; prev=""
for a in "$@"; do [ "$prev" = --store ] && store=$a; [ "$prev" = --bindings ] && bindings=$a; prev=$a; done
hook=$(pick sh)
[ -n "$hook" ] && . "$hook"
f=$(pick out); [ -n "$f" ] && sed "s|@STORE@|$store|g" "$f"
f=$(pick err); [ -n "$f" ] && cat "$f" >&2
f=$(pick exit); [ -n "$f" ] && exit "$(cat "$f")"
exit 0
"#;

/// One recorded call.
pub struct Call {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub struct Fake {
    pub dir: PathBuf,
}

impl Fake {
    pub fn new(dir: &Path) -> Fake {
        std::fs::create_dir_all(dir.join("script")).unwrap();
        let script = SCRIPT.replace("@DIR@", &dir.display().to_string());
        let bin = dir.join("b10x-loom");
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        Fake { dir: dir.to_path_buf() }
    }

    pub fn bin(&self) -> PathBuf {
        self.dir.join("b10x-loom")
    }

    pub fn sha256(&self) -> String {
        Sha256::digest(std::fs::read(self.bin()).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The `k`-th (1-based, or `default`) answer to `sub`: stdout, exit code.
    pub fn answer(&self, sub: &str, k: &str, out: &str, exit: i32) -> &Fake {
        let base = self.dir.join("script").join(format!("{sub}.{k}"));
        if !out.is_empty() {
            std::fs::write(base.with_extension_str("out"), out).unwrap();
        }
        std::fs::write(base.with_extension_str("exit"), exit.to_string()).unwrap();
        self
    }

    /// stderr of an answer.
    pub fn stderr(&self, sub: &str, k: &str, text: &str) -> &Fake {
        std::fs::write(self.dir.join("script").join(format!("{sub}.{k}.err")), text).unwrap();
        self
    }

    /// A shell snippet sourced by the answer.
    pub fn hook(&self, sub: &str, k: &str, snippet: &str) -> &Fake {
        std::fs::write(self.dir.join("script").join(format!("{sub}.{k}.sh")), snippet).unwrap();
        self
    }

    /// The answers of a run that opens fine and registers every protocol.
    pub fn open_ok(&self) -> &Fake {
        self.answer("protocols_add", "default", "installed\n", 0);
        self.answer("tree_open", "default", "{\"root\":\"r\"}\n", 0);
        self.hook("tree_open", "default", "mkdir -p \"$store\" && printf '{\"bindings\":\"%s\"}\\n' \"$bindings\" > \"$store/tree.json\"\n");
        self.answer("tree_status", "default", "{\"cases\":[],\"format\":\"loom.tree-status/1\"}\n", 0);
        self.answer("tree_verify", "default", "{\"cases\":1}\n", 0);
        self
    }

    pub fn calls_so_far(&self) -> u64 {
        std::fs::read_to_string(self.dir.join("calls/count")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0)
    }

    pub fn calls(&self) -> Vec<Call> {
        (1..=self.calls_so_far())
            .map(|n| {
                let argv = std::fs::read_to_string(self.dir.join(format!("calls/{n}.argv"))).unwrap_or_default().lines().map(str::to_owned).collect();
                let env = std::fs::read_to_string(self.dir.join(format!("calls/{n}.env")))
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                    .collect();
                Call { argv, env }
            })
            .collect()
    }

    /// The calls of one subcommand, for example `["tree", "run"]`.
    pub fn calls_of(&self, words: &[&str]) -> Vec<Call> {
        self.calls().into_iter().filter(|c| c.argv.len() >= words.len() && c.argv.iter().zip(words).all(|(a, w)| a == w)).collect()
    }
}

trait WithExt {
    fn with_extension_str(&self, ext: &str) -> PathBuf;
}

impl WithExt for PathBuf {
    /// Appends `.ext` (the base names hold dots already: `tree_run.1`).
    fn with_extension_str(&self, ext: &str) -> PathBuf {
        PathBuf::from(format!("{}.{ext}", self.display()))
    }
}

/// A `tree run` answer: the JSON `b10x-loom` prints and the exit code.
pub fn tree_run_json(stop: &str, steps: i64, detail: Option<&str>, decisions: &str) -> String {
    let detail = detail.map_or("null".to_owned(), |d| format!("\"{d}\""));
    format!("{{\"case\":\"root\",\"decisions\":{decisions},\"detail\":{detail},\"steps\":{steps},\"stop\":\"{stop}\"}}\n")
}

/// One decision as Loom prints it, for the store `store`.
pub fn decision_json(action: &str, kind: &str, principal: &str) -> String {
    let action_words = if kind == "ApproveAction" { format!(",\"--action\",\"{action}\"") } else { String::new() };
    format!(
        "{{\"action\":\"{action}\",\"capability\":\"owner.approve\",\"case\":\"root\",\"command\":[\"b10x-loom\",\"tree\",\"approve\",\"--store\",\"@STORE@\",\"--case\",\"root\",\"--capability\",\"owner.approve\",\"--principal\",\"{principal}\"{action_words}],\"decides\":\"{action} of case root at revision 1\",\"kind\":\"{kind}\",\"path\":\"/work/x.yaml\",\"revision\":\"t1\",\"subject\":\"x\"}}"
    )
}
