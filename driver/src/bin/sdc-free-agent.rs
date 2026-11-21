//! One free-running agent session: the comparison run of experiments/free-agent-0.1.0.yaml
//! (docs/llm-is-not-the-runtime.md).
//!
//! ```text
//! sdc-free-agent run --run-dir <dir> --idea <text> --prompt <file> --example-spec <dir> --model <id>
//!                    --effort <level> --max-usd <usd> --max-wall-s <s> --max-turns <n>
//!                    [--claude <bin>] [--ess <bin>] [--toolchain <dir>] [--credentials <file>] [--print]
//! sdc-free-agent measure <run dir>
//! sdc-free-agent job --run-dir <dir> --rw <dir>... --cwd <dir> -- <cargo arguments>   (run by slot)
//! cargo <arguments>                                                (the shim inside the confinement)
//! ```
//!
//! `run` starts ONE `claude -p` session with the hermetic settings of the SDC's claude-cli adapter
//! (driver/examples/sdc/tools/llm-adapter-claude-cli.sh: `--setting-sources ""`, a fresh config
//! home holding only a copy of the login, `--strict-mcp-config`, `--disable-slash-commands`, no
//! auto-memory, no git status block), with the file tools and Bash, inside a bubblewrap
//! confinement: a new mount, PID, IPC and UTS namespace whose only writable directories are the
//! run's `work/` (the session's working directory), `home/` and `cargo-home/`; /usr and /etc read
//! only; no other directory of the host, so no repository checkout and no other run; an empty
//! environment plus what the session needs (PATH, HOME, the config home). The session keeps the
//! network (the model API needs it). A `cargo` that compiles or tests is the shim: it asks the
//! launcher's broker (a socket in the run directory) to run it, and the broker runs it through
//! `slot run`, inside a second confinement of the same view without network and without the login.
//! Every stream line is written to `events.jsonl` and the instant it arrived to `received.jsonl`;
//! `free-agent.json` holds the settings, caps and end. Caps: `--max-budget-usd` (the CLI ends the
//! session) and the wall time (the launcher kills the confinement); a capped run says so.
//!
//! `measure` measures the outcome by program, not by the agent's word: the session ended by itself,
//! `ess specify validate --strict-requires` on its spec, `cargo build` and `cargo test --locked` of
//! a copy of its implementation through slot in the same confinement, and the deliverables present;
//! it writes `outcome.json`. Exit 0, 1 a run that failed to start or a measurement that could not
//! run, 64 usage.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: sdc-free-agent run --run-dir <dir> --idea <text> --prompt <file> --example-spec <dir> --model <id> --effort <level> --max-usd <usd> --max-wall-s <s> --max-turns <n> [--claude <bin>] [--ess <bin>] [--toolchain <dir>] [--credentials <file>] [--print]
       sdc-free-agent measure <run dir>
       sdc-free-agent job --run-dir <dir> --rw <dir>... --cwd <dir> -- <cargo arguments>";

/// The session's tools: the SDC nodes' file tools and a shell.
const TOOLS: &str = "Read,Write,Edit,Glob,Grep,Bash";
const DISALLOWED: &str = "WebFetch,WebSearch,Task,Skill";
/// cargo subcommands the slot shim sends through the pool (as ~/.local/share/slot/slot-shim).
const POOLED: [&str; 14] = ["build", "b", "check", "c", "test", "t", "clippy", "doc", "d", "bench", "nextest", "install", "rustc", "miri"];

// ---- small helpers -----------------------------------------------------------------------------

fn civil(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn iso_ms(ms: i64) -> String {
    let (y, m, d, hh, mm, ss) = civil(ms.div_euclid(1000));
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.{:03}Z", ms.rem_euclid(1000))
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()))
}

fn canon(p: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(p).map_err(|e| format!("cannot resolve {}: {e}", p.display()))
}

fn copy_dir(from: &Path, to: &Path, skip: &[&str]) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for e in fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?.flatten() {
        let name = e.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        let ft = e.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_dir(&src, &dst, skip)?;
        } else if ft.is_file() {
            fs::copy(&src, &dst).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        }
    }
    Ok(())
}

fn write_json(path: &Path, v: &J) -> Result<(), String> {
    fs::write(path, serde_json::to_string_pretty(v).unwrap_or_default() + "\n").map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn read_json(path: &Path) -> Result<J, String> {
    let t = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

// ---- the confinement ---------------------------------------------------------------------------

/// What the confinement of a run binds, written to `<run>/confinement.json` by `run` and read by
/// `job` and `measure`.
struct Layout {
    run: PathBuf,
    work: PathBuf,
    home: PathBuf,
    cargo_home: PathBuf,
    broker: PathBuf,
    toolchain: PathBuf,
    claude: PathBuf,
    ess: PathBuf,
    me: PathBuf,
    slot: PathBuf,
    bwrap: PathBuf,
}

impl Layout {
    fn json(&self) -> J {
        json!({"run": s(&self.run), "work": s(&self.work), "home": s(&self.home), "cargo_home": s(&self.cargo_home),
            "broker": s(&self.broker), "toolchain": s(&self.toolchain), "claude": s(&self.claude), "ess": s(&self.ess),
            "me": s(&self.me), "slot": s(&self.slot), "bwrap": s(&self.bwrap)})
    }
    fn load(run: &Path) -> Result<Layout, String> {
        let j = read_json(&run.join("confinement.json"))?;
        let p = |k: &str| PathBuf::from(j[k].as_str().unwrap_or(""));
        Ok(Layout { run: p("run"), work: p("work"), home: p("home"), cargo_home: p("cargo_home"), broker: p("broker"),
            toolchain: p("toolchain"), claude: p("claude"), ess: p("ess"), me: p("me"), slot: p("slot"), bwrap: p("bwrap") })
    }
    fn socket(&self) -> PathBuf {
        self.broker.join("cargo.sock")
    }
}

/// The view both confinements share: /usr and /etc read only, fresh /proc, /dev and /tmp, the Rust
/// toolchain at /opt/rust, new PID, IPC and UTS namespaces, killed with the launcher.
fn base_view(l: &Layout) -> Vec<String> {
    let mut a: Vec<String> = ["--die-with-parent", "--new-session", "--unshare-pid", "--unshare-ipc", "--unshare-uts",
        "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin", "/bin", "--symlink", "usr/bin", "/sbin",
        "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64", "--ro-bind", "/etc", "/etc",
        "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"].iter().map(|x| x.to_string()).collect();
    a.extend(["--ro-bind".into(), s(&l.toolchain), "/opt/rust".into()]);
    a.extend(["--bind".into(), s(&l.cargo_home), s(&l.cargo_home)]);
    a
}

fn setenv(a: &mut Vec<String>, k: &str, v: &str) {
    a.extend(["--setenv".into(), k.into(), v.into()]);
}

/// The session's confinement around `claude -p`.
fn session_bwrap(l: &Layout) -> Vec<String> {
    let mut a = vec![s(&l.bwrap)];
    a.extend(base_view(l));
    for d in [&l.work, &l.home, &l.broker] {
        a.extend(["--bind".into(), s(d), s(d)]);
    }
    // The resolver /etc/resolv.conf points to (systemd-resolved's stub), read only: the model API
    // needs names.
    if Path::new("/run/systemd/resolve").is_dir() {
        a.extend(["--ro-bind".into(), "/run/systemd/resolve".into(), "/run/systemd/resolve".into()]);
    }
    a.extend(["--ro-bind".into(), s(&l.claude), "/opt/sdc/bin/claude".into()]);
    a.extend(["--ro-bind".into(), s(&l.ess), "/opt/sdc/bin/ess".into()]);
    a.extend(["--ro-bind".into(), s(&l.me), "/opt/sdc/bin/cargo".into()]);
    a.extend(["--remount-ro".into(), "/".into(), "--chdir".into(), s(&l.work), "--clearenv".into()]);
    setenv(&mut a, "PATH", "/opt/sdc/bin:/opt/rust/bin:/usr/bin");
    setenv(&mut a, "HOME", &s(&l.home));
    setenv(&mut a, "USER", "agent");
    setenv(&mut a, "SHELL", "/usr/bin/bash");
    setenv(&mut a, "LANG", "C.UTF-8");
    setenv(&mut a, "TERM", "dumb");
    setenv(&mut a, "TMPDIR", "/tmp");
    setenv(&mut a, "CLAUDE_CONFIG_DIR", &s(&l.home.join(".claude")));
    setenv(&mut a, "CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1");
    setenv(&mut a, "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", "1");
    setenv(&mut a, "DISABLE_AUTOUPDATER", "1");
    setenv(&mut a, "CARGO_HOME", &s(&l.cargo_home));
    setenv(&mut a, "CARGO_NET_OFFLINE", "true");
    setenv(&mut a, "RUSTC", "/opt/rust/bin/rustc");
    setenv(&mut a, "SDC_REAL_CARGO", "/opt/rust/bin/cargo");
    setenv(&mut a, "SDC_FREE_AGENT_BROKER", &s(&l.socket()));
    a.push("--".into());
    a
}

/// The confinement of one cargo job (run by slot): the same view, no network, no login, writable
/// only `rw`, the cargo home and the home without its config directory.
fn job_bwrap(l: &Layout, rw: &[PathBuf], cwd: &Path, args: &[String]) -> Vec<String> {
    let mut a = vec![s(&l.bwrap)];
    a.extend(base_view(l));
    a.push("--unshare-net".into());
    a.extend(["--bind".into(), s(&l.home), s(&l.home), "--tmpfs".into(), s(&l.home.join(".claude"))]);
    for d in rw {
        a.extend(["--bind".into(), s(d), s(d)]);
    }
    let mut fifo_env = vec![];
    for k in ["MAKEFLAGS", "CARGO_MAKEFLAGS"] {
        if let Ok(v) = std::env::var(k) {
            for part in v.split_whitespace() {
                if let Some(f) = part.strip_prefix("--jobserver-auth=fifo:") {
                    if Path::new(f).exists() && !a.iter().any(|x| x == f) {
                        a.extend(["--bind".into(), f.into(), f.into()]);
                    }
                }
            }
            fifo_env.push((k, v));
        }
    }
    a.extend(["--remount-ro".into(), "/".into(), "--chdir".into(), s(cwd), "--clearenv".into()]);
    setenv(&mut a, "PATH", "/opt/rust/bin:/usr/bin");
    setenv(&mut a, "HOME", &s(&l.home));
    setenv(&mut a, "LANG", "C.UTF-8");
    setenv(&mut a, "TERM", "dumb");
    setenv(&mut a, "TMPDIR", "/tmp");
    setenv(&mut a, "CARGO_HOME", &s(&l.cargo_home));
    setenv(&mut a, "CARGO_NET_OFFLINE", "true");
    setenv(&mut a, "RUSTC", "/opt/rust/bin/rustc");
    for (k, v) in fifo_env {
        setenv(&mut a, k, &v);
    }
    if let Ok(j) = std::env::var("SLOT_JOB") {
        setenv(&mut a, "SLOT_JOB", &j);
    }
    a.push("--".into());
    a.push("/opt/rust/bin/cargo".into());
    a.extend(args.iter().cloned());
    a
}

/// The host environment the slot client gets: enough to reach the daemon, nothing of the login.
fn slot_client_env() -> Vec<(String, String)> {
    ["HOME", "XDG_RUNTIME_DIR", "XDG_STATE_HOME", "USER", "LOGNAME"].iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .chain([("PATH".to_string(), "/usr/bin:/bin".to_string())])
        .collect()
}

// ---- the broker and the shim -------------------------------------------------------------------

fn frame(w: &mut impl Write, tag: u8, data: &[u8]) -> std::io::Result<()> {
    w.write_all(&[tag])?;
    w.write_all(&(data.len() as u32).to_be_bytes())?;
    w.write_all(data)?;
    w.flush()
}

/// One shim request: run `cargo <args>` in `cwd` (under work/) through slot in the job
/// confinement, stream its output back, then its exit code.
fn serve(l: Arc<Layout>, stream: UnixStream, log: Arc<Mutex<fs::File>>) {
    let mut reader = BufReader::new(match stream.try_clone() { Ok(s) => s, Err(_) => return });
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let out = Arc::new(Mutex::new(stream));
    let refuse = |msg: &str| {
        let mut o = out.lock().unwrap();
        let _ = frame(&mut *o, 2, format!("sdc-free-agent broker: {msg}\n").as_bytes());
        let _ = frame(&mut *o, 3, &1i32.to_be_bytes());
    };
    let Ok(req) = serde_json::from_str::<J>(&line) else { return refuse("unreadable request") };
    let args: Vec<String> = req["args"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    let cwd = match fs::canonicalize(req["cwd"].as_str().unwrap_or("")) {
        Ok(c) if c.starts_with(&l.work) => c,
        _ => return refuse("cargo runs only under the working directory"),
    };
    let started = now_ms();
    let child = Command::new(&l.slot).arg("run").arg("--").arg(&l.me)
        .args(["job", "--run-dir", &s(&l.run), "--rw", &s(&l.work), "--cwd", &s(&cwd), "--"]).args(&args)
        .env_clear().envs(slot_client_env()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return refuse(&format!("cannot start slot: {e}")),
    };
    let broken = Arc::new(AtomicBool::new(false));
    let pump = |r: Box<dyn Read + Send>, tag: u8| {
        let out = out.clone();
        let broken = broken.clone();
        std::thread::spawn(move || {
            let mut r = r;
            let mut buf = [0u8; 8192];
            loop {
                match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if frame(&mut *out.lock().unwrap(), tag, &buf[..n]).is_err() {
                            broken.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
        })
    };
    let t1 = pump(Box::new(child.stdout.take().unwrap()), 1);
    let t2 = pump(Box::new(child.stderr.take().unwrap()), 2);
    let code = loop {
        if broken.load(Ordering::SeqCst) {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(1),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break 1,
        }
    };
    let _ = t1.join();
    let _ = t2.join();
    let _ = frame(&mut *out.lock().unwrap(), 3, &code.to_be_bytes());
    let rel = cwd.strip_prefix(&l.work).map(|p| p.display().to_string()).unwrap_or_default();
    let entry = json!({"at": iso_ms(started), "ms": now_ms() - started, "cwd": rel, "args": args, "exit": code});
    let _ = writeln!(log.lock().unwrap(), "{entry}");
}

fn broker(l: Arc<Layout>) -> Result<(), String> {
    let sock = l.socket();
    let listener = UnixListener::bind(&sock).map_err(|e| format!("cannot bind {}: {e}", sock.display()))?;
    let log = Arc::new(Mutex::new(fs::OpenOptions::new().create(true).append(true).open(l.broker.join("jobs.jsonl"))
        .map_err(|e| format!("cannot open jobs.jsonl: {e}"))?));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (l, log) = (l.clone(), log.clone());
            std::thread::spawn(move || serve(l, stream, log));
        }
    });
    Ok(())
}

/// `cargo` inside the confinement.
fn shim(args: Vec<String>) -> ExitCode {
    let mut skip = false;
    let mut sub = "";
    for a in &args {
        if skip {
            skip = false;
            continue;
        }
        match a.as_str() {
            "--color" | "--config" | "-Z" | "-C" | "--manifest-path" => skip = true,
            x if x.starts_with('+') || x.starts_with('-') => {}
            x => {
                sub = x;
                break;
            }
        }
    }
    let real = std::env::var("SDC_REAL_CARGO").unwrap_or_else(|_| "/opt/rust/bin/cargo".into());
    if !POOLED.contains(&sub) {
        let e = Command::new(&real).args(&args).exec();
        eprintln!("cargo shim: cannot run {real}: {e}");
        return ExitCode::from(127);
    }
    let Ok(sock) = std::env::var("SDC_FREE_AGENT_BROKER") else {
        eprintln!("cargo shim: no build pool broker (SDC_FREE_AGENT_BROKER)");
        return ExitCode::from(127);
    };
    let mut st = match UnixStream::connect(&sock) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo shim: cannot reach the build pool broker: {e}");
            return ExitCode::from(127);
        }
    };
    let cwd = std::env::current_dir().map(|p| s(&p)).unwrap_or_default();
    if writeln!(st, "{}", json!({"args": args, "cwd": cwd})).is_err() {
        return ExitCode::from(127);
    }
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    loop {
        let mut head = [0u8; 5];
        if st.read_exact(&mut head).is_err() {
            eprintln!("cargo shim: the broker closed the connection");
            return ExitCode::from(1);
        }
        let n = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
        let mut body = vec![0u8; n];
        if st.read_exact(&mut body).is_err() {
            return ExitCode::from(1);
        }
        match head[0] {
            1 => {
                let _ = out.write_all(&body);
                let _ = out.flush();
            }
            2 => {
                let _ = err.write_all(&body);
                let _ = err.flush();
            }
            _ => {
                let code = if n == 4 { i32::from_be_bytes([body[0], body[1], body[2], body[3]]) } else { 1 };
                return ExitCode::from(code.clamp(0, 255) as u8);
            }
        }
    }
}

/// `job`: run by slot on the host; execs the job confinement.
fn job(args: &[String]) -> Result<ExitCode, String> {
    let (mut run, mut rw, mut cwd, mut rest) = (None, vec![], None, vec![]);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--run-dir" => run = it.next().map(PathBuf::from),
            "--rw" => rw.extend(it.next().map(PathBuf::from)),
            "--cwd" => cwd = it.next().map(PathBuf::from),
            "--" => {
                rest = it.cloned().collect();
                break;
            }
            x => return Err(format!("job: unknown argument {x}")),
        }
    }
    let (Some(run), Some(cwd)) = (run, cwd) else { return Err("job needs --run-dir and --cwd".into()) };
    let l = Layout::load(&run)?;
    if !rw.iter().any(|d| cwd.starts_with(d)) {
        return Err("job: the working directory is not under a writable directory".into());
    }
    let a = job_bwrap(&l, &rw, &cwd, &rest);
    let e = Command::new(&a[0]).args(&a[1..]).env_clear().exec();
    Err(format!("cannot run bwrap: {e}"))
}

// ---- run ---------------------------------------------------------------------------------------

/// Removes the copied login when the run ends, whatever the end.
struct Login(PathBuf);

impl Drop for Login {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Default)]
struct RunArgs {
    run_dir: Option<PathBuf>,
    idea: Option<String>,
    prompt: Option<PathBuf>,
    example: Option<PathBuf>,
    model: Option<String>,
    effort: Option<String>,
    max_usd: Option<f64>,
    max_wall: Option<u64>,
    max_turns: Option<u64>,
    claude: Option<PathBuf>,
    ess: Option<PathBuf>,
    toolchain: Option<PathBuf>,
    credentials: Option<PathBuf>,
    print: bool,
}

fn parse_run(args: &[String]) -> Result<RunArgs, String> {
    let mut a = RunArgs::default();
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut v = || it.next().cloned().ok_or_else(|| format!("{x} needs a value"));
        match x.as_str() {
            "--run-dir" => a.run_dir = Some(PathBuf::from(v()?)),
            "--idea" => a.idea = Some(v()?),
            "--prompt" => a.prompt = Some(PathBuf::from(v()?)),
            "--example-spec" => a.example = Some(PathBuf::from(v()?)),
            "--model" => a.model = Some(v()?),
            "--effort" => a.effort = Some(v()?),
            "--max-usd" => a.max_usd = Some(v()?.parse().map_err(|_| "--max-usd needs a number")?),
            "--max-wall-s" => a.max_wall = Some(v()?.parse().map_err(|_| "--max-wall-s needs seconds")?),
            "--max-turns" => a.max_turns = Some(v()?.parse().map_err(|_| "--max-turns needs a number")?),
            "--claude" => a.claude = Some(PathBuf::from(v()?)),
            "--ess" => a.ess = Some(PathBuf::from(v()?)),
            "--toolchain" => a.toolchain = Some(PathBuf::from(v()?)),
            "--credentials" => a.credentials = Some(PathBuf::from(v()?)),
            "--print" => a.print = true,
            y => return Err(format!("unknown argument {y}")),
        }
    }
    Ok(a)
}

fn claude_argv(prompt: &str, model: &str, effort: &str, max_usd: f64, max_turns: u64) -> Vec<String> {
    let mut v: Vec<String> = vec!["/opt/sdc/bin/claude".into(), "-p".into(), prompt.into(), "--model".into(), model.into(),
        "--max-turns".into(), max_turns.to_string(), "--max-budget-usd".into(), format!("{max_usd}"),
        "--allowedTools".into(), TOOLS.into(), "--disallowedTools".into(), DISALLOWED.into(), "--tools".into(), TOOLS.into(),
        "--setting-sources".into(), String::new(), "--strict-mcp-config".into(), "--disable-slash-commands".into(),
        "--permission-mode".into(), "acceptEdits".into(), "--output-format".into(), "stream-json".into(), "--verbose".into(),
        "--include-partial-messages".into(), "--no-session-persistence".into()];
    if !effort.is_empty() {
        v.extend(["--effort".into(), effort.into()]);
    }
    v
}

/// The confinement and the command line in words, with the run directory written `<run>` and the
/// login's home `$HOME`: what the record and the report state.
fn describe(l: &Layout, argv: &[String]) -> J {
    let home = std::env::var("HOME").unwrap_or_default();
    let clean = |x: &str| {
        let x = x.replace(&s(&l.run), "<run>");
        if home.is_empty() { x } else { x.replace(&home, "$HOME") }
    };
    let shown: Vec<String> = argv.iter().enumerate().map(|(i, x)| if i > 0 && argv[i - 1] == "-p" { "<prompt.md>".into() } else { clean(x) }).collect();
    json!({
        "summary": "bubblewrap: new mount, PID, IPC and UTS namespaces, killed with the launcher; writable only <run>/work (the working directory), <run>/home (HOME and the config home with a copy of the login, removed at the end) and <run>/cargo-home; /usr, /etc and the resolver stub /run/systemd/resolve read only; the Rust toolchain, claude, ess and the cargo shim read only under /opt; the rest of the root is an empty read-only tmpfs: nothing else of the host (no repository checkout, no other run); empty environment plus PATH, HOME, the config home and cargo's; the session keeps the network for the model API; cargo build/test/check/clippy/doc go through the broker to slot run, in a confinement of the same view without network and without the login",
        "argv": shown,
    })
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let a = parse_run(args)?;
    let need = |o: &Option<String>, k: &str| o.clone().ok_or_else(|| format!("run needs {k}"));
    let run_dir = a.run_dir.clone().ok_or("run needs --run-dir")?;
    let idea = need(&a.idea, "--idea")?;
    let model = need(&a.model, "--model")?;
    let effort = need(&a.effort, "--effort")?;
    let max_usd = a.max_usd.ok_or("run needs --max-usd")?;
    let max_wall = a.max_wall.ok_or("run needs --max-wall-s")?;
    let max_turns = a.max_turns.ok_or("run needs --max-turns")?;
    let prompt_path = a.prompt.clone().ok_or("run needs --prompt")?;
    let example = a.example.clone().ok_or("run needs --example-spec")?;
    let prompt = fs::read_to_string(&prompt_path).map_err(|e| format!("cannot read {}: {e}", prompt_path.display()))?;
    let home_env = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    let find = |o: &Option<PathBuf>, name: &str| -> Result<PathBuf, String> {
        canon(&o.clone().or_else(|| which(name)).ok_or_else(|| format!("{name} is not on PATH"))?)
    };
    if run_dir.exists() {
        return Err(format!("refused: {} exists; every run starts in a fresh directory", run_dir.display()));
    }
    let parent = run_dir.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let toolchain = canon(&a.toolchain.clone().unwrap_or_else(|| PathBuf::from(&home_env).join(".rustup/toolchains/stable-x86_64-unknown-linux-gnu")))?;
    let mut l = Layout {
        run: PathBuf::new(), work: PathBuf::new(), home: PathBuf::new(), cargo_home: PathBuf::new(), broker: PathBuf::new(),
        toolchain, claude: find(&a.claude, "claude")?, ess: find(&a.ess, "ess")?,
        me: canon(&std::env::current_exe().map_err(|e| e.to_string())?)?, slot: find(&None, "slot")?,
        bwrap: find(&None, "bwrap")?,
    };
    if a.print {
        l.run = PathBuf::from("<run>");
        let argv = claude_argv(&prompt, &model, &effort, max_usd, max_turns);
        println!("{}", serde_json::to_string_pretty(&describe(&l, &argv)).unwrap_or_default());
        return Ok(ExitCode::SUCCESS);
    }
    fs::create_dir(&run_dir).map_err(|e| format!("cannot create {}: {e}", run_dir.display()))?;
    l.run = canon(&run_dir)?;
    l.work = l.run.join("work");
    l.home = l.run.join("home");
    l.cargo_home = l.run.join("cargo-home");
    l.broker = l.run.join("broker");
    for d in [&l.work, &l.home.join(".claude"), &l.cargo_home, &l.broker, &l.work.join("in")] {
        fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    let idea_doc = serde_yaml::to_string(&json!({"text": idea})).map_err(|e| e.to_string())?;
    fs::write(l.work.join("in/idea.yaml"), &idea_doc).map_err(|e| e.to_string())?;
    copy_dir(&example, &l.work.join("in/example-spec"), &[])?;
    let creds = a.credentials.clone().unwrap_or_else(|| PathBuf::from(&home_env).join(".claude/.credentials.json"));
    let login = Login(l.home.join(".claude/.credentials.json"));
    fs::copy(&creds, &login.0).map_err(|e| format!("cannot copy the login {}: {e}", creds.display()))?;
    fs::set_permissions(&login.0, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    write_json(&l.run.join("confinement.json"), &l.json())?;
    fs::write(l.run.join("prompt.md"), &prompt).map_err(|e| e.to_string())?;
    let argv = claude_argv(&prompt, &model, &effort, max_usd, max_turns);
    let started = now_ms();
    let name = l.run.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut meta = json!({
        "format": "sdc.free-agent-run/1", "run": name, "end": "running", "capped": null, "exit": null,
        "model": model, "effort": effort, "adapter": "claude-cli", "tools": TOOLS.split(',').collect::<Vec<_>>(),
        "caps": {"max_usd": max_usd, "max_wall_s": max_wall, "max_turns": max_turns},
        "idea_sha256": sha256_hex(idea.trim().as_bytes()), "prompt_sha256": sha256_hex(prompt.as_bytes()),
        "started_at": iso_ms(started), "ended_at": null,
        "confinement": describe(&l, &argv),
    });
    write_json(&l.run.join("free-agent.json"), &meta)?;
    broker(Arc::new(Layout::load(&l.run)?))?;
    let mut full = session_bwrap(&l);
    full.extend(argv);
    let stderr = fs::File::create(l.run.join("stderr.txt")).map_err(|e| e.to_string())?;
    let mut child: Child = Command::new(&full[0]).args(&full[1..]).env_clear().stdin(Stdio::null())
        .stdout(Stdio::piped()).stderr(stderr).spawn().map_err(|e| format!("cannot start bwrap: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let child = Arc::new(Mutex::new(child));
    let walled = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    {
        let (child, walled, done) = (child.clone(), walled.clone(), done.clone());
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(max_wall);
            while Instant::now() < deadline {
                if done.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            walled.store(true, Ordering::SeqCst);
            let _ = child.lock().unwrap().kill();
        });
    }
    let mut events = fs::File::create(l.run.join("events.jsonl")).map_err(|e| e.to_string())?;
    let mut received = fs::File::create(l.run.join("received.jsonl")).map_err(|e| e.to_string())?;
    let mut result = J::Null;
    for (i, line) in BufReader::new(stdout).lines().enumerate() {
        let Ok(line) = line else { break };
        let at = now_ms();
        let _ = writeln!(events, "{line}");
        let _ = writeln!(received, "{}", json!({"index": i, "received_at": iso_ms(at)}));
        if line.starts_with("{\"type\":\"result\"") || line.contains("\"type\":\"result\"") {
            if let Ok(v) = serde_json::from_str::<J>(&line) {
                if v["type"] == "result" {
                    result = v;
                }
            }
        }
    }
    let status = child.lock().unwrap().wait().map_err(|e| e.to_string())?;
    done.store(true, Ordering::SeqCst);
    drop(login);
    let ended = now_ms();
    let subtype = result["subtype"].as_str().unwrap_or("").to_string();
    let cost = result["total_cost_usd"].as_f64();
    let capped = if walled.load(Ordering::SeqCst) {
        Some("wall")
    } else if subtype.contains("budget") || cost.is_some_and(|c| c >= max_usd) {
        Some("usd")
    } else {
        None
    };
    let end = match (capped, subtype.as_str(), status.code()) {
        (Some(_), _, _) => "capped".to_string(),
        (None, "success", Some(0)) if result["is_error"] != true => "completed".to_string(),
        (None, _, _) if result["is_error"] == true => format!("failed: error ({})", result["api_error_status"].as_str().map(String::from).or_else(|| result["api_error_status"].as_i64().map(|n| n.to_string())).unwrap_or_else(|| "no status".into())),
        (None, "", _) => "failed: no result".to_string(),
        (None, st, _) => format!("failed: {st}"),
    };
    meta["end"] = json!(end);
    meta["capped"] = json!(capped);
    meta["exit"] = json!(status.code());
    meta["ended_at"] = json!(iso_ms(ended));
    meta["result_subtype"] = json!(subtype);
    meta["cost_usd"] = json!(cost);
    write_json(&l.run.join("free-agent.json"), &meta)?;
    let _ = fs::remove_file(l.socket());
    println!("{name}: {end}; cost {} USD; {} s", cost.map_or("-".into(), |c| format!("{c:.4}")), (ended - started) / 1000);
    Ok(ExitCode::SUCCESS)
}

// ---- measure -----------------------------------------------------------------------------------

fn nonempty(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// Runs `cargo <args>` on the copy through slot in the job confinement; its output and exit.
fn pooled_cargo(l: &Layout, dir: &Path, args: &[&str]) -> (Option<i32>, String) {
    let o = Command::new(&l.slot).arg("run").arg("--").arg(&l.me)
        .args(["job", "--run-dir", &s(&l.run), "--rw", &s(dir), "--cwd", &s(dir), "--"]).args(args)
        .env_clear().envs(slot_client_env()).stdin(Stdio::null()).output();
    match o {
        Ok(o) => (o.status.code(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
        Err(e) => (None, format!("cannot start slot: {e}")),
    }
}

fn measure(args: &[String]) -> Result<ExitCode, String> {
    let [run] = args else { return Err("measure needs one run directory".into()) };
    let run = canon(Path::new(run))?;
    let l = Layout::load(&run)?;
    let meta = read_json(&run.join("free-agent.json"))?;
    if meta["end"] == "running" {
        return Err("the run has not ended".into());
    }
    let w = &l.work;
    let m = run.join("measure");
    let _ = fs::remove_dir_all(&m);
    fs::create_dir_all(&m).map_err(|e| e.to_string())?;
    let requirements = fs::read_to_string(w.join("requirements.yaml")).ok()
        .and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok())
        .and_then(|y| y.get("requirements").and_then(|r| r.as_sequence()).map(|r| r.len()))
        .unwrap_or(0);
    let plan = fs::read_to_string(w.join("plan.md")).unwrap_or_default();
    let stories = plan.lines().filter(|x| x.trim_start().trim_start_matches(['-', '*', ' ']).to_lowercase().starts_with("covers:")).count();
    let mut d = BTreeMap::new();
    d.insert("requirements", requirements > 0);
    d.insert("spec", nonempty(&w.join("spec/system.yaml")));
    d.insert("plan", stories > 0);
    d.insert("impl", nonempty(&w.join("impl/Cargo.toml")));
    d.insert("review", nonempty(&w.join("review.md")));
    // The spec, with the pinned ess on the host (a program reading YAML).
    let (spec_exit, spec_out) = if d["spec"] {
        match Command::new(&l.ess).args(["specify", "validate", "--path", "spec", "--strict-requires"]).current_dir(w).output() {
            Ok(o) => (o.status.code(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
            Err(e) => (None, format!("cannot run ess: {e}")),
        }
    } else {
        (None, "no spec/system.yaml".into())
    };
    fs::write(m.join("ess-validate.txt"), &spec_out).map_err(|e| e.to_string())?;
    let ess_version = Command::new(&l.ess).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    // The implementation: a copy, built and tested through slot in the job confinement.
    let (mut build_exit, mut test_exit, mut passed, mut failed) = (None, None, 0u64, 0u64);
    if d["impl"] {
        let copy = m.join("impl");
        copy_dir(&w.join("impl"), &copy, &["target"])?;
        let (b, bo) = pooled_cargo(&l, &copy, &["build", "--locked", "--offline"]);
        fs::write(m.join("build.txt"), &bo).map_err(|e| e.to_string())?;
        build_exit = b;
        let (t, to) = pooled_cargo(&l, &copy, &["test", "--locked", "--offline"]);
        fs::write(m.join("test.txt"), &to).map_err(|e| e.to_string())?;
        test_exit = t;
        for line in to.lines().filter(|x| x.starts_with("test result:")) {
            let num = |key: &str| line.split(';').find_map(|p| p.trim().trim_start_matches("test result: ok.").trim_start_matches("test result: FAILED.")
                .trim().strip_suffix(key).and_then(|n| n.trim().parse::<u64>().ok())).unwrap_or(0);
            passed += num("passed");
            failed += num("failed");
        }
    }
    let ended_by_itself = meta["end"] == "completed";
    let spec_valid = spec_exit == Some(0);
    let builds = build_exit == Some(0);
    let tests_pass = test_exit == Some(0) && passed > 0 && failed == 0;
    let delivered = ended_by_itself && d.values().all(|x| *x) && spec_valid && builds && tests_pass;
    let outcome = json!({
        "format": "sdc.free-agent-outcome/1", "measured_at": iso_ms(now_ms()),
        "ended_by_itself": ended_by_itself, "end": meta["end"], "capped": meta["capped"],
        "deliverables": d, "requirements": requirements, "stories": stories,
        "spec_valid": spec_valid, "spec_validate_exit": spec_exit, "ess_version": ess_version,
        "builds": builds, "build_exit": build_exit,
        "tests_pass": tests_pass, "test_exit": test_exit, "tests_passed": passed, "tests_failed": failed,
        "conformance": "not measured",
        "delivered": delivered,
        "rule": "delivered: ended by itself (not capped, no error), requirements, spec, plan with at least one covers line, impl and review present, ess specify validate --strict-requires passes, cargo build --locked and cargo test --locked of a copy pass with at least one test and none failed",
    });
    write_json(&run.join("outcome.json"), &outcome)?;
    println!("{}: ended by itself {ended_by_itself}, spec valid {spec_valid}, builds {builds}, tests {passed} passed {failed} failed, delivered {delivered}",
        run.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let mut argv = std::env::args();
    let me = argv.next().unwrap_or_default();
    let args: Vec<String> = argv.collect();
    if Path::new(&me).file_name().is_some_and(|n| n == "cargo") {
        return shim(args);
    }
    let r = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("measure") => measure(&args[1..]),
        Some("job") => job(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(64);
        }
    };
    match r {
        Ok(c) => c,
        Err(m) => {
            eprintln!("sdc-free-agent: {m}");
            ExitCode::from(1)
        }
    }
}
