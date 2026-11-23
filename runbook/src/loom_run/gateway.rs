//! The `[gateway]` setting (`docs/loom-run-settings.md`, "`[gateway]`"): an address and a token for the
//! model programs of a run, instead of a login file.
//!
//! Two steps. [`check_shape`] runs in `settings::normalise` and reads nothing: the table, its members, the
//! address, an inline token and the form of a `token_file` (the path, not the file), and that no login file
//! is named beside it. [`take`] runs once the dry preset is known: it reads the `token_file`, refuses the
//! confinement and the adapters that cannot carry a gateway, and returns the gateway with the settings in
//! which the table is replaced by the text `named`, so that nothing after it, the decider, the bindings copy,
//! the events and the run directory, ever holds the token or the path of a token file.
//!
//! Every refusal names the member (`gateway.base_url`) and never a value, a part of a value or a path.

use super::{Args, Confinement};
pub use sdc_tools::gateway::{FILE_VARIABLE, OPTION};
use sdc_tools::gateway::{self, Gateway};
use serde_json::Value;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The key of the table.
pub const KEY: &str = "gateway";

const MEMBERS: [&str; 3] = ["base_url", "token", "token_file"];

/// The shape of the table, with nothing read: see the module's text.
pub fn check_shape(settings: &Value) -> Result<(), String> {
    let Some(table) = settings.get(KEY) else {
        return Ok(());
    };
    let Some(table) = table.as_object() else {
        return Err("gateway: a table with base_url and one of token and token_file".to_owned());
    };
    let old_spelling = settings.get("loom").and_then(|loom| loom.get(super::settings::LOGIN_KEY));
    if settings.get(super::settings::LOGIN_KEY).is_some() || old_spelling.is_some() {
        return Err("gateway and login_file: a run has one or the other, never both".to_owned());
    }
    if let Some(other) = table.keys().find(|k| !MEMBERS.contains(&k.as_str())) {
        return Err(format!("gateway.{other}: not a member of the table (known: {})", MEMBERS.join(", ")));
    }
    match table.get("base_url") {
        None => return Err("gateway.base_url: required".to_owned()),
        Some(Value::String(url)) => gateway::check_base_url(url).map_err(|why| format!("gateway.base_url: {why}"))?,
        Some(_) => return Err("gateway.base_url: text".to_owned()),
    }
    match (table.get("token"), table.get("token_file")) {
        (Some(_), Some(_)) => Err("gateway.token and gateway.token_file: exactly one is required (both given)".to_owned()),
        (None, None) => Err("gateway.token and gateway.token_file: exactly one is required (neither given)".to_owned()),
        (Some(Value::String(token)), None) => gateway::check_token(token).map_err(|why| format!("gateway.token: {why}")),
        (Some(_), None) => Err("gateway.token: text".to_owned()),
        (None, Some(Value::String(path))) if Path::new(path).is_absolute() => Ok(()),
        (None, Some(Value::String(_))) => Err("gateway.token_file: not an absolute path".to_owned()),
        (None, Some(_)) => Err("gateway.token_file: a path".to_owned()),
    }
}

/// The token of the file `path`: one line, an end of line after it allowed. The file is opened once, without
/// following a link and without blocking (a FIFO opens at once and is no regular file), the kind is asked of the
/// open descriptor, and it is read up to a limit; an error names the member and what is wrong, never the path or
/// the content.
fn token_of_file(path: &str) -> Result<String, String> {
    let unreadable = || "gateway.token_file: not a readable file".to_owned();
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC).open(path).map_err(|_| unreadable())?;
    if !file.metadata().map_err(|_| unreadable())?.is_file() {
        return Err(unreadable());
    }
    let mut text = String::new();
    file.take(gateway::TOKEN_LIMIT as u64 + 16).read_to_string(&mut text).map_err(|_| "gateway.token_file: not readable as text".to_owned())?;
    if text.trim().is_empty() {
        return Err("gateway.token_file: empty".to_owned());
    }
    let line = text.strip_suffix('\n').map(|t| t.strip_suffix('\r').unwrap_or(t)).unwrap_or(&text);
    if line.contains('\n') || line.contains('\r') {
        return Err("gateway.token_file: not one line".to_owned());
    }
    gateway::check_token(line).map_err(|why| format!("gateway.token_file: the token {why}"))?;
    Ok(line.to_owned())
}

/// The adapter a settings file gives a model decider: its own, else the default `metaharness`.
fn decider_adapter(settings: &Value) -> Option<String> {
    let decider = settings.get("decider")?;
    (decider.get("kind").and_then(Value::as_str) == Some("model")).then(|| decider.get("adapter").and_then(Value::as_str).unwrap_or("metaharness").to_owned())
}

/// Reads the gateway of `settings` (already shape-checked) and returns the settings with the table replaced
/// by `named`, and the gateway: `None` when no table is named, and with `--dry`, which only checks the shape
/// and uses nothing.
pub fn take(args: &Args, mut settings: Value) -> Result<(Value, Option<Gateway>), String> {
    if settings.get(KEY).is_none() {
        return Ok((settings, None));
    }
    check_shape(&settings)?;
    let table = settings[KEY].clone();
    settings[KEY] = Value::String("named".to_owned());
    if args.ports.dry {
        return Ok((settings, None));
    }
    if args.confinement != Confinement::Local {
        return Err(format!("gateway: not available at --confinement {}: a gateway works at --confinement local only (docs/loom-run-settings.md, \"[gateway]\")", args.confinement.as_arg()));
    }
    if settings.get("adapter").and_then(Value::as_str) == Some("metaharness") {
        return Err("gateway and adapter: the adapter metaharness cannot take a gateway (it removes ANTHROPIC_* and starts its own proxy); name adapter = \"claude-cli\"".to_owned());
    }
    if decider_adapter(&settings).as_deref() == Some("metaharness") {
        return Err("gateway and decider.adapter: the adapter metaharness (the model decider's default) cannot take a gateway; name decider.adapter = \"claude-cli\"".to_owned());
    }
    let base = table["base_url"].as_str().unwrap_or_default();
    let token = match (table.get("token").and_then(Value::as_str), table.get("token_file").and_then(Value::as_str)) {
        (Some(token), _) => token.to_owned(),
        (None, Some(path)) => token_of_file(path)?,
        (None, None) => return Err("gateway.token and gateway.token_file: exactly one is required (neither given)".to_owned()),
    };
    Ok((settings, Some(Gateway::new(base, &token)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TOKEN: &str = "tok-unit-8d2c5a71";

    fn args(confinement: Confinement, dry: bool) -> Args {
        let mut a = Args {
            run: "r".into(),
            runs_dir: "runs".into(),
            loom: None,
            loom_sha256: None,
            bindings: "bindings".into(),
            protocols: "protocols".into(),
            root_protocol: "sdc@8".into(),
            case: "IDEA-1".into(),
            intake: "x".into(),
            confinement,
            settings: "settings.toml".into(),
            spend_ledger: "ledger.json".into(),
            cap_micro_usd: 1,
            max_llm_calls: 1,
            max_wall_seconds: 1,
            ports: Default::default(),
        };
        a.ports.dry = dry;
        a
    }

    fn table() -> Value {
        json!({"base_url": "http://127.0.0.1:8080", "token": TOKEN})
    }

    #[test]
    fn the_shape_is_checked_with_nothing_read_and_no_refusal_holds_a_value() {
        assert_eq!(check_shape(&json!({})), Ok(()));
        assert_eq!(check_shape(&json!({"gateway": table()})), Ok(()));
        assert_eq!(check_shape(&json!({"gateway": {"base_url": "https://gw.example.test/v1", "token_file": "/nonexistent/holder"}})), Ok(()));
        let bad = [
            (json!({"gateway": "http://127.0.0.1:8080"}), "gateway"),
            (json!({"gateway": {"token": TOKEN}}), "gateway.base_url"),
            (json!({"gateway": {"base_url": "ftp://h", "token": TOKEN}}), "gateway.base_url"),
            (json!({"gateway": {"base_url": 5, "token": TOKEN}}), "gateway.base_url"),
            (json!({"gateway": {"base_url": "http://h"}}), "gateway.token_file"),
            (json!({"gateway": {"base_url": "http://h", "token": TOKEN, "token_file": "/x"}}), "gateway.token_file"),
            (json!({"gateway": {"base_url": "http://h", "token_file": "relative"}}), "gateway.token_file"),
            (json!({"gateway": {"base_url": "http://h", "token": " "}}), "gateway.token"),
            (json!({"gateway": {"base_url": "http://h", "token": TOKEN, "model": "m"}}), "gateway.model"),
            (json!({"gateway": table(), "login_file": "/nonexistent/login"}), "login_file"),
        ];
        for (settings, member) in bad {
            let e = check_shape(&settings).unwrap_err();
            assert!(e.contains(member) && !e.contains(TOKEN) && !e.contains("/nonexistent") && !e.contains("relative"), "{settings}: {e}");
        }
    }

    #[test]
    fn the_example_of_the_page_is_a_table_the_reader_takes_and_the_run_settings_record_only_that_one_was_named() {
        const DOC: &str = include_str!("../../../docs/loom-run-settings.md");
        let start = DOC.find("[gateway]\nbase_url").expect("the page has the gateway example");
        let end = start + DOC[start..].find("```").expect("the example is closed");
        let given = crate::loom_run::settings::parse(&DOC[start..end]).unwrap();
        assert_eq!(given["gateway"]["base_url"], "http://127.0.0.1:8080");
        let given = crate::loom_run::settings::normalise(&given, &args(Confinement::Local, false)).unwrap();
        let (taken, gateway) = take(&args(Confinement::Local, false), given).unwrap();
        assert_eq!(gateway.unwrap().token(), "placeholder");
        assert_eq!(crate::loom_run::settings::redacted(&taken)["gateway"], "named");
        let table = json!({"gateway": table()});
        assert_eq!(crate::loom_run::settings::redacted(&table)["gateway"], "named", "a table that was not taken is redacted too");
    }

    #[test]
    fn a_gateway_with_a_login_file_in_either_place_is_refused_before_the_old_place_of_the_login_is() {
        for settings in [json!({"gateway": table(), "login_file": "/nonexistent/login"}), json!({"gateway": table(), "loom": {"login_file": "/nonexistent/login"}})] {
            let e = crate::loom_run::settings::normalise(&settings, &args(Confinement::Local, false)).unwrap_err();
            assert!(e.contains("gateway") && e.contains("login_file") && !e.contains("nonexistent") && !e.contains(TOKEN), "{e}");
        }
    }

    #[test]
    fn take_replaces_the_table_by_named_and_returns_the_gateway() {
        let (settings, gateway) = take(&args(Confinement::Local, false), json!({"gateway": table(), "autonomy": "A5"})).unwrap();
        assert_eq!(settings, json!({"gateway": "named", "autonomy": "A5"}));
        let gateway = gateway.unwrap();
        assert_eq!((gateway.base_url(), gateway.token()), ("http://127.0.0.1:8080", TOKEN));
        let (settings, none) = take(&args(Confinement::Local, false), json!({"autonomy": "A5"})).unwrap();
        assert!((settings, none.is_none()) == (json!({"autonomy": "A5"}), true));
    }

    #[test]
    fn a_dry_run_checks_the_shape_and_reads_nothing() {
        let named = json!({"gateway": {"base_url": "http://h", "token_file": "/nonexistent/holder"}, "adapter": "metaharness"});
        let (settings, gateway) = take(&args(Confinement::Substrate, true), named).unwrap();
        assert_eq!(settings["gateway"], "named");
        assert!(gateway.is_none());
        assert!(take(&args(Confinement::Local, true), json!({"gateway": {"base_url": "ftp://h", "token": TOKEN}})).is_err());
    }

    #[test]
    fn the_substrate_level_and_the_metaharness_adapter_are_refused_by_name() {
        let e = take(&args(Confinement::Substrate, false), json!({"gateway": table()})).unwrap_err();
        assert!(e.contains("gateway") && e.contains("substrate") && !e.contains(TOKEN), "{e}");
        let e = take(&args(Confinement::Local, false), json!({"gateway": table(), "adapter": "metaharness"})).unwrap_err();
        assert!(e.contains("adapter") && e.contains("metaharness") && !e.contains(TOKEN), "{e}");
        for decider in [json!({"kind": "model", "model": "m"}), json!({"kind": "model", "model": "m", "adapter": "metaharness"})] {
            let e = take(&args(Confinement::Local, false), json!({"gateway": table(), "decider": decider})).unwrap_err();
            assert!(e.contains("decider.adapter") && !e.contains(TOKEN), "{e}");
        }
        for ok in [json!({"kind": "model", "model": "m", "adapter": "claude-cli"}), json!({"kind": "rule", "verdict": "approve"}), json!({"kind": "model", "model": "m", "adapter": "/some/adapter"})] {
            assert!(take(&args(Confinement::Local, false), json!({"gateway": table(), "decider": ok, "adapter": "claude-cli"})).is_ok());
        }
    }

    #[test]
    fn a_token_file_is_read_as_one_line_and_each_fault_is_named_without_its_path_or_content() {
        let dir = crate::assets::testdir::Dir::new("gateway-token-file");
        let write = |name: &str, text: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, text).unwrap();
            p.display().to_string()
        };
        let table = |path: &str| json!({"gateway": {"base_url": "http://127.0.0.1:1", "token_file": path}});
        let ok = write("ok", &format!("{TOKEN}\n"));
        assert_eq!(take(&args(Confinement::Local, false), table(&ok)).unwrap().1.unwrap().token(), TOKEN);
        let crlf = write("crlf", &format!("{TOKEN}\r\n"));
        assert_eq!(take(&args(Confinement::Local, false), table(&crlf)).unwrap().1.unwrap().token(), TOKEN);
        let bare = write("bare", TOKEN);
        assert_eq!(take(&args(Confinement::Local, false), table(&bare)).unwrap().1.unwrap().token(), TOKEN);
        for (name, text, word) in [("empty", "", "empty"), ("blank", "\n  \n", "empty"), ("two", "first-secret\nsecond-secret\n", "one line"), ("inner", "a b\n", "visible")] {
            let path = write(name, text);
            let e = take(&args(Confinement::Local, false), table(&path)).unwrap_err();
            assert!(e.contains("gateway.token_file") && e.contains(word), "{name}: {e}");
            assert!(!e.contains(&path) && !e.contains("secret"), "{name}: {e}");
        }
        let e = take(&args(Confinement::Local, false), table(&dir.path().join("absent").display().to_string())).unwrap_err();
        assert!(e.contains("gateway.token_file") && !e.contains("absent"), "{e}");
        let e = take(&args(Confinement::Local, false), table(&dir.path().display().to_string())).unwrap_err();
        assert!(e.contains("gateway.token_file"), "{e}");
    }

    /// The bytes this thread has asked the kernel to read so far (`rchar`).
    fn bytes_read_by_this_thread() -> u64 {
        let io = std::fs::read_to_string("/proc/thread-self/io").unwrap();
        io.lines().find_map(|l| l.strip_prefix("rchar: ")).unwrap().trim().parse().unwrap()
    }

    #[test]
    fn a_token_file_far_over_the_read_bound_is_refused_after_a_few_thousand_bytes_are_read() {
        let dir = crate::assets::testdir::Dir::new("gateway-token-far-over");
        let big = dir.path().join("big");
        std::fs::File::create(&big).unwrap().set_len(256 << 20).unwrap();
        let before = bytes_read_by_this_thread();
        let e = token_of_file(&big.display().to_string()).unwrap_err();
        let read = bytes_read_by_this_thread() - before;
        assert!(e.contains("gateway.token_file"), "{e}");
        assert!(read < 64 * 1024, "{read} bytes of a file of 256 MiB were read");
    }
}
