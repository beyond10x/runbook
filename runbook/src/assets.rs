//! Where the runbook finds its assets (the directories of `LAYOUT`, laid out as in the repository)
//! and its helper binaries.
//!
//! Assets, first match wins:
//! 1. `RUNBOOK_HOME`, when set and non-empty. It is used as given; a directory without `protocol/`
//!    is an error, not a reason to look further, because the person asked for that one.
//! 2. `<directory of the executable>/../share/runbook`, when it holds `protocol/`.
//! 3. A checkout: walking up from the executable's directory, then from the current directory, the
//!    first directory that holds `protocol/` and `driver/examples/sdc/`.
//!
//! Helper binaries, first match wins: `<exe dir>/../libexec/runbook`, the executable's own directory, in a
//! checkout `driver/target/{release,debug}` and `watch/target/{release,debug}`, then `PATH`. A flat
//! `<prefix>/libexec` is not read.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Installed,
    Checkout,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Env => "RUNBOOK_HOME",
            Source::Installed => "installed (share/runbook)",
            Source::Checkout => "checkout",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assets {
    pub root: PathBuf,
    pub source: Source,
}

impl Assets {
    pub fn protocol_dir(&self) -> PathBuf {
        self.root.join("protocol")
    }
}

/// What one part of an installed runbook is: a directory, a directory that holds files with an
/// extension, or one program that must be executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Dir,
    Files(&'static str),
    Program,
}

/// One part of the layout under the asset root, and what needs it.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    /// Also the name of its `runbook doctor` line.
    pub name: &'static str,
    /// Relative to the asset root.
    pub path: &'static str,
    pub shape: Shape,
    pub by: &'static str,
}

/// Every part an installed runbook has, in the order `runbook doctor` prints them: the protocols,
/// the text of a runbook (bindings, instructions, views, cycles), the tools the bindings run, the
/// scripts and the inspector.
#[rustfmt::skip]
pub const LAYOUT: &[Part] = &[
    Part { name: "protocol/", path: "protocol", shape: Shape::Dir, by: "every runbook: the protocol files" },
    Part { name: "scripts/", path: "scripts", shape: Shape::Dir, by: "every runbook: the evaluation and check scripts" },
    Part { name: "driver/examples/sdc/", path: "driver/examples/sdc", shape: Shape::Dir, by: "every runbook: the JSON forms of the bindings, which the checks read" },
    Part { name: "bindings/", path: "bindings", shape: Shape::Dir, by: "every runbook: the bindings in Loom's format" },
    Part { name: "bindings/*.yaml", path: "bindings", shape: Shape::Files("yaml"), by: "every runbook: at least one binding file" },
    Part { name: "bindings/tools/", path: "bindings/tools", shape: Shape::Dir, by: "every runbook: the programs the bindings run" },
    Part { name: "instructions/", path: "instructions", shape: Shape::Dir, by: "every runbook: the instruction files of the model's calls" },
    Part { name: "views/", path: "views", shape: Shape::Dir, by: "every runbook and the ui: the view files of the run inspector" },
    Part { name: "cycles/", path: "cycles", shape: Shape::Dir, by: "every runbook: the finding cycles as data" },
    Part { name: "ui/", path: "ui", shape: Shape::Dir, by: "every runbook: the run inspector" },
    Part { name: "bindings/tools/sdc-tool.sh", path: "bindings/tools/sdc-tool.sh", shape: Shape::Program, by: "every runbook: the tool the story and plan nodes run" },
    Part { name: "bindings/tools/llm-adapter-claude-cli.sh", path: "bindings/tools/llm-adapter-claude-cli.sh", shape: Shape::Program, by: "every runbook: the adapter that runs the claude CLI" },
];

impl Part {
    /// Why the part is not there, in a few words.
    pub fn absent_reason(&self) -> &'static str {
        match self.shape {
            Shape::Dir => "no such directory",
            Shape::Files(_) => "no such files",
            Shape::Program => "not an executable file",
        }
    }

    /// Whether the part is there under `root`.
    pub fn present(&self, root: &Path) -> bool {
        let p = root.join(self.path);
        match self.shape {
            Shape::Dir => p.is_dir(),
            Shape::Files(ext) => std::fs::read_dir(&p)
                .map(|d| d.flatten().any(|e| e.path().is_file() && e.path().extension().is_some_and(|x| x == ext)))
                .unwrap_or(false),
            Shape::Program => is_executable(&p),
        }
    }
}

/// Everything the lookup reads from the process, so a test can give it a temporary world.
#[derive(Debug, Clone, Default)]
pub struct Lookup {
    pub home: Option<PathBuf>,
    pub exe: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub path: Option<OsString>,
    /// The checkout walk stops at this directory (`None`: at `/`), so a test's temporary world
    /// inside a real checkout does not find that checkout.
    pub ceiling: Option<PathBuf>,
}

impl Lookup {
    pub fn from_process() -> Lookup {
        Lookup {
            home: std::env::var_os("RUNBOOK_HOME").filter(|v| !v.is_empty()).map(PathBuf::from),
            exe: std::env::current_exe().ok().map(|p| p.canonicalize().unwrap_or(p)),
            cwd: std::env::current_dir().ok(),
            path: std::env::var_os("PATH"),
            ceiling: None,
        }
    }

    fn exe_dir(&self) -> Option<&Path> {
        self.exe.as_deref().and_then(Path::parent)
    }

    pub fn locate(&self) -> Result<Assets, String> {
        if let Some(h) = &self.home {
            if h.join("protocol").is_dir() {
                return Ok(Assets { root: h.clone(), source: Source::Env });
            }
            return Err(format!("RUNBOOK_HOME={} holds no protocol/ directory", h.display()));
        }
        if let Some(d) = self.exe_dir() {
            let share = d.join("..").join("share").join("runbook");
            if share.join("protocol").is_dir() {
                return Ok(Assets { root: share, source: Source::Installed });
            }
        }
        for start in [self.exe_dir(), self.cwd.as_deref()].into_iter().flatten() {
            let below_ceiling = |d: &&Path| self.ceiling.as_deref().is_none_or(|c| d.starts_with(c));
            if let Some(root) = start.ancestors().take_while(below_ceiling).find(|d| is_checkout(d)) {
                return Ok(Assets { root: root.to_path_buf(), source: Source::Checkout });
            }
        }
        Err("no assets: set RUNBOOK_HOME, install them at <prefix>/share/runbook, or run inside a checkout".into())
    }

    /// The helper binary `name`, or `None`.
    pub fn helper(&self, name: &str, assets: Option<&Assets>) -> Option<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(d) = self.exe_dir() {
            dirs.push(d.join("..").join("libexec").join("runbook"));
            dirs.push(d.to_path_buf());
        }
        if let Some(a) = assets.filter(|a| a.source == Source::Checkout) {
            for krate in ["driver", "watch"] {
                for profile in ["release", "debug"] {
                    dirs.push(a.root.join(krate).join("target").join(profile));
                }
            }
        }
        dirs.iter().map(|d| d.join(name)).find(|p| is_executable(p)).or_else(|| self.which(name))
    }

    /// The directory of the helpers of an installed prefix, `<prefix>/libexec/runbook`, or `None` when the
    /// executable does not run from an installed layout. The layout is installed when all of these hold: the
    /// executable's directory is named `bin`, `<prefix>/libexec/runbook` is a directory and
    /// `<prefix>/share/runbook/protocol` is a directory. `libexec/runbook` belongs to this product alone, so
    /// nothing another package keeps in `<prefix>/libexec` is ever found through it.
    pub fn installed_helpers(&self) -> Option<PathBuf> {
        let bin = self.exe_dir()?;
        if bin.file_name()? != "bin" {
            return None;
        }
        let prefix = bin.parent()?;
        let helpers = prefix.join("libexec").join("runbook");
        (helpers.is_dir()
            && prefix
                .join("share")
                .join("runbook")
                .join("protocol")
                .is_dir())
        .then_some(helpers)
    }

    /// The `PATH` for this process and the programs it starts, when it runs from an installed prefix:
    /// `<prefix>/libexec/runbook` in front of the current `PATH`, so a bound tool that starts a helper
    /// (`sdc-reply`, `sdc-findings`) finds it there: the install puts the helpers in `libexec/runbook`, not in
    /// `bin`, so a `PATH` of `<prefix>/bin` alone does not reach them. The new `PATH` has no empty entry (an
    /// empty entry is the working directory): empty entries of the current one are dropped, and with `PATH` unset
    /// or empty the rest is `/usr/bin:/bin`. `None` when there is nothing to change: no installed layout, or the
    /// helpers directory already on `PATH`.
    pub fn path_with_libexec(&self) -> Option<OsString> {
        let helpers = self.installed_helpers()?;
        let current = self.path.clone().unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&current)
            .filter(|d| !d.as_os_str().is_empty())
            .collect();
        if dirs.contains(&helpers) {
            return None;
        }
        if dirs.is_empty() {
            dirs = vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")];
        }
        dirs.insert(0, helpers);
        std::env::join_paths(dirs).ok()
    }

    /// `name` on `PATH`.
    pub fn which(&self, name: &str) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        std::env::split_paths(path).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(name)).find(|p| is_executable(p))
    }
}

fn is_checkout(dir: &Path) -> bool {
    dir.join("protocol").is_dir() && dir.join("driver").join("examples").join("sdc").is_dir()
}

pub fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(test)]
pub mod testdir {
    use std::path::{Path, PathBuf};

    /// A scratch directory under the build's target directory, removed on drop.
    pub struct Dir(pub PathBuf);

    impl Dir {
        pub fn new(tag: &str) -> Dir {
            let exe = std::env::current_exe().unwrap();
            let base = exe.parent().unwrap().parent().unwrap().join("runbook-test-tmp");
            let d = base.join(format!("{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Dir(d)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
        pub fn dir(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            p
        }
        pub fn exe(&self, rel: &str) -> PathBuf {
            use std::os::unix::fs::PermissionsExt;
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testdir::Dir;
    use super::*;

    fn lookup(t: &Dir) -> Lookup {
        Lookup {
            home: None,
            exe: Some(t.dir("prefix/bin").join("runbook")),
            cwd: Some(t.dir("elsewhere")),
            path: None,
            ceiling: Some(t.path().to_path_buf()),
        }
    }

    fn make_checkout(t: &Dir, rel: &str) -> PathBuf {
        let root = t.dir(rel);
        std::fs::create_dir_all(root.join("protocol")).unwrap();
        std::fs::create_dir_all(root.join("driver/examples/sdc")).unwrap();
        root
    }

    #[test]
    fn env_home_wins_over_installed_and_checkout() {
        let t = Dir::new("env-wins");
        let home = t.dir("home/protocol").parent().unwrap().to_path_buf();
        t.dir("prefix/share/runbook/protocol");
        make_checkout(&t, "prefix/bin/co");
        let mut l = lookup(&t);
        l.home = Some(home.clone());
        assert_eq!(l.locate().unwrap(), Assets { root: home, source: Source::Env });
    }

    #[test]
    fn env_home_without_protocol_is_an_error_not_a_fallthrough() {
        let t = Dir::new("env-bad");
        t.dir("prefix/share/runbook/protocol");
        let mut l = lookup(&t);
        l.home = Some(t.dir("empty"));
        assert!(l.locate().unwrap_err().contains("RUNBOOK_HOME"));
    }

    #[test]
    fn installed_share_beats_checkout() {
        let t = Dir::new("installed");
        t.dir("prefix/share/runbook/protocol");
        make_checkout(&t, "prefix");
        let a = lookup(&t).locate().unwrap();
        assert_eq!(a.source, Source::Installed);
        assert!(a.root.ends_with("prefix/bin/../share/runbook"), "{:?}", a.root);
    }

    #[test]
    fn checkout_is_found_from_the_executable_then_from_the_cwd() {
        let t = Dir::new("checkout");
        let from_exe = make_checkout(&t, "a");
        let from_cwd = make_checkout(&t, "b");
        let mut l = lookup(&t);
        l.exe = Some(from_exe.join("runbook/target/debug/runbook"));
        l.cwd = Some(from_cwd.join("docs"));
        assert_eq!(l.locate().unwrap(), Assets { root: from_exe.clone(), source: Source::Checkout });
        l.exe = Some(t.path().join("prefix/bin/runbook"));
        assert_eq!(l.locate().unwrap(), Assets { root: from_cwd, source: Source::Checkout });
    }

    #[test]
    fn a_directory_with_only_protocol_is_not_a_checkout() {
        let t = Dir::new("not-checkout");
        t.dir("x/protocol");
        let mut l = lookup(&t);
        l.cwd = Some(t.path().join("x"));
        assert!(l.locate().is_err());
    }

    #[test]
    fn helpers_are_found_in_libexec_then_beside_the_exe_then_checkout_then_path() {
        let t = Dir::new("helpers");
        let root = make_checkout(&t, "co");
        let a = Assets { root: root.clone(), source: Source::Checkout };
        let mut l = lookup(&t);
        l.path = Some(t.dir("pathdir").into_os_string());
        let on_path = t.exe("pathdir/sdc-x");
        assert_eq!(l.helper("sdc-x", Some(&a)), Some(on_path));
        let debug = t.exe("co/driver/target/debug/sdc-x");
        assert_eq!(l.helper("sdc-x", Some(&a)), Some(debug));
        let release = t.exe("co/driver/target/release/sdc-x");
        assert_eq!(l.helper("sdc-x", Some(&a)), Some(release));
        let beside = t.exe("prefix/bin/sdc-x");
        assert_eq!(l.helper("sdc-x", Some(&a)), Some(beside.clone()));
        let flat = t.exe("prefix/libexec/sdc-x");
        assert_eq!(
            l.helper("sdc-x", Some(&a)),
            Some(beside.clone()),
            "a flat libexec is not read"
        );
        let libexec = t.exe("prefix/libexec/runbook/sdc-x");
        let got = l.helper("sdc-x", Some(&a)).unwrap();
        assert_eq!(got.canonicalize().unwrap(), libexec.canonicalize().unwrap());
        assert_ne!(got.canonicalize().unwrap(), flat.canonicalize().unwrap());
    }

    #[test]
    fn checkout_target_directories_count_only_for_a_checkout() {
        let t = Dir::new("helpers-nocheckout");
        let root = make_checkout(&t, "co");
        t.exe("co/driver/target/debug/sdc-x");
        let a = Assets { root, source: Source::Env };
        assert_eq!(lookup(&t).helper("sdc-x", Some(&a)), None);
    }

    #[test]
    fn libexec_goes_in_front_of_the_path_only_from_an_installed_layout() {
        let t = Dir::new("libexec-path");
        let mut l = lookup(&t);
        l.path = Some("/usr/bin:/bin".into());
        assert_eq!(l.path_with_libexec(), None, "no libexec");
        t.dir("prefix/libexec");
        t.dir("prefix/share/runbook/protocol");
        assert_eq!(
            l.path_with_libexec(),
            None,
            "a flat libexec is not the helpers' directory"
        );
        let libexec = t.dir("prefix/libexec/runbook");
        std::fs::remove_dir_all(t.path().join("prefix/share/runbook/protocol")).unwrap();
        assert_eq!(
            l.path_with_libexec(),
            None,
            "libexec/runbook without share/runbook/protocol is not an install"
        );
        t.dir("prefix/share/runbook/protocol");
        let mut want = libexec.clone().into_os_string();
        want.push(":/usr/bin:/bin");
        l.path = Some("/usr/bin:/bin".into());
        assert_eq!(
            l.path_with_libexec(),
            Some(want.clone()),
            "in front of the given PATH"
        );
        l.path = Some(std::env::join_paths([PathBuf::from("/usr/bin"), libexec.clone()]).unwrap());
        assert_eq!(l.path_with_libexec(), None, "already on PATH");
        for unset in [None, Some(OsString::new()), Some(OsString::from(":"))] {
            l.path = unset.clone();
            assert_eq!(l.path_with_libexec(), Some(want.clone()), "PATH {unset:?}");
        }
        l.path = Some("/usr/local/bin::/bin:".into());
        let mut want = libexec.into_os_string();
        want.push(":/usr/local/bin:/bin");
        assert_eq!(
            l.path_with_libexec(),
            Some(want),
            "empty entries are dropped"
        );
    }

    #[test]
    fn the_executable_directory_of_an_installed_layout_is_named_bin() {
        let t = Dir::new("libexec-bin-name");
        t.dir("prefix/libexec/runbook");
        t.dir("prefix/share/runbook/protocol");
        let mut l = lookup(&t);
        l.path = Some("/usr/bin".into());
        assert!(l.path_with_libexec().is_some());
        l.exe = Some(t.dir("prefix/debug").join("runbook"));
        assert_eq!(
            l.installed_helpers(),
            None,
            "<x>/debug/runbook with <x>/libexec/runbook is no install"
        );
        assert_eq!(l.path_with_libexec(), None);
    }

    #[test]
    fn a_file_without_the_execute_bit_is_not_a_helper() {
        let t = Dir::new("not-exec");
        std::fs::write(t.dir("bin").join("sdc-x"), "").unwrap();
        let mut l = lookup(&t);
        l.path = Some(t.path().join("bin").into_os_string());
        assert_eq!(l.which("sdc-x"), None);
    }
}
