//! The run settings file (TOML, `docs/loom-run-settings.md`) and its redacted copy.

use serde_json::Value;
use std::path::Path;

/// The key that names the path of the login file. Its value never reaches the run directory.
pub const LOGIN_KEY: &str = "login_file";

/// Reads the settings file: a TOML table, as the one form of the format. A file that is JSON is
/// refused by its content (not by its name) before anything else is read.
pub fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("--settings {}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("--settings {}: {e}", path.display()))
}

/// The settings a run reads: the `--settings` file when one is named, over the members `runbook.toml`
/// carries (a member of the file replaces the default's whole). With neither, an empty table.
pub fn merged(args: &crate::loom_run::Args) -> Result<Value, String> {
    let over = args.settings.as_deref().map(read).transpose()?;
    Ok(crate::config::merge(&args.defaults, over.as_ref()))
}

/// The settings of a TOML text as the JSON value the loop, the decider and the bindings copy read:
/// a table is an object in file order, an integer a whole number, a float a float (so no guard that
/// wants a whole number takes it). A date or a time is no value of any setting.
pub fn parse(text: &str) -> Result<Value, String> {
    read_table(text).map_err(|e| crate::config::printable(&e))
}

fn read_table(text: &str) -> Result<Value, String> {
    if looks_like_json(text) {
        return Err("this is JSON, and the settings file is TOML now: write the same members as TOML, the members that were under `loom` at the top level (the form and an example: docs/loom-run-settings.md)".to_owned());
    }
    let table = toml::de::DeTable::parse(text).map_err(|e| not_toml(&e, text))?;
    table_of(table.get_ref(), "settings")
}

/// The most of the parser's own words a refusal prints, in characters.
const MESSAGE_LIMIT: usize = 160;

/// The refusal of a syntax error: where it is (line and column, in characters) and the parser's own
/// words for the kind of error, cut to `MESSAGE_LIMIT`. Never a line, a value or a part of a value of
/// the file: the library's `Display` prints the offending line, and that line can hold the login's
/// path or a pasted login content.
fn not_toml(e: &toml::de::Error, text: &str) -> String {
    let words: String = e.message().lines().next().unwrap_or("").chars().take(MESSAGE_LIMIT).collect();
    let at = e.span().map(|span| {
        let start = span.start.min(text.len());
        let before = &text.as_bytes()[..start];
        let line_start = before.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let line = before.iter().filter(|b| **b == b'\n').count() + 1;
        let column = String::from_utf8_lossy(&before[line_start..]).chars().count() + 1;
        format!(" at line {line}, column {column}")
    });
    format!("not TOML{} ({words}; the form: docs/loom-run-settings.md)", at.unwrap_or_default())
}

/// Whether the text is the old form: an object, or an array, as JSON begins, after a byte order mark,
/// blank lines and comment lines (which TOML also has). TOML has no value that begins with `{`, and a
/// table header begins with `[` but is no JSON array of anything.
fn looks_like_json(text: &str) -> bool {
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text).trim_start();
    while rest.starts_with('#') {
        rest = rest.split_once('\n').map_or("", |(_, after)| after).trim_start();
    }
    rest.starts_with('{') || (rest.starts_with('[') && serde_json::from_str::<Value>(rest).is_ok())
}

/// A table of the file as a JSON object in file order. The reading is the library's syntax tree and
/// this walk, not its serde glue: the glue words an error with the value it met.
fn table_of(t: &toml::de::DeTable<'_>, at: &str) -> Result<Value, String> {
    let mut object = serde_json::Map::new();
    for (k, x) in t.iter() {
        let k = k.get_ref().to_string();
        let member = json_of(x.get_ref(), &format!("{at}.{}", crate::config::segment(&k)))?;
        object.insert(k, member);
    }
    Ok(Value::Object(object))
}

/// One value of the file as the JSON value the settings readers take. An error names the member and
/// the kind of value, never the value.
fn json_of(v: &toml::de::DeValue<'_>, at: &str) -> Result<Value, String> {
    use toml::de::DeValue;
    Ok(match v {
        DeValue::String(s) => Value::String(s.to_string()),
        DeValue::Integer(n) => match i64::from_str_radix(n.as_str(), n.radix()) {
            Ok(n) => Value::from(n),
            Err(_) => return Err(format!("{at}: not TOML: an integer must fit 64 bits")),
        },
        DeValue::Float(f) => {
            let x: f64 = f.as_str().parse().map_err(|_| format!("{at}: not TOML: a float the settings cannot read"))?;
            if x.is_infinite() && !f.as_str().contains("inf") {
                return Err(format!("{at}: not TOML: a float too large for 64 bits"));
            }
            Value::Number(serde_json::Number::from_f64(x).ok_or_else(|| format!("{at}: a float that is not finite is not a number a setting takes"))?)
        }
        DeValue::Boolean(b) => Value::Bool(*b),
        DeValue::Datetime(_) => return Err(format!("{at}: a date or a time is not a value of any setting")),
        DeValue::Array(items) => Value::Array(items.iter().enumerate().map(|(i, x)| json_of(x.get_ref(), &format!("{at}[{i}]"))).collect::<Result<_, _>>()?),
        DeValue::Table(t) => table_of(t, at)?,
    })
}

/// The format name a settings file may carry in `format`.
pub const FORMAT: &str = "runbook.loom-run-settings/1";

/// The keys of the file's top level that only repeat an option; `normalise` checks and takes them out.
const OPTION_KEYS: [&str; 6] = ["format", "run", "loom_sha256", "in_process", "confinement", "cap_micro_usd"];

/// The members the JSON form kept under `loom`, and the place each has now.
const OLD_PLACES: [(&str, &str); 5] =
    [("sha256", "loom_sha256"), ("in_process", "in_process"), ("confinement", "confinement"), ("cap_micro_usd", "cap_micro_usd"), ("login_file", "login_file")];

fn same_number(v: &Value, want: u64, what: &str, option: &str) -> Result<(), String> {
    match v.as_u64() {
        Some(n) if n == want => Ok(()),
        Some(n) => Err(format!("{what} is {n}, the option {option} is {want}: the option decides; make them equal")),
        None => Err(format!("{what}: a whole number")),
    }
}

/// A member that names a cap, beside an option that may be absent: a member without the option
/// names the cap (`with_named_caps` took it), a member beside the option must equal it.
fn same_named(v: &Value, option_value: Option<u64>, what: &str, option: &str) -> Result<(), String> {
    match option_value {
        Some(want) => same_number(v, want, what, option),
        None => Err(format!("{what}: a whole number")),
    }
}

/// The arguments of the run with the caps the settings name: a member `cap_micro_usd` or
/// `max_llm_calls` of the file names the cap when its option is not given. A member that is not a
/// whole number names nothing here; [`normalise`] refuses it.
pub fn with_named_caps(args: &crate::loom_run::Args, raw: &Value) -> crate::loom_run::Args {
    let mut named = args.clone();
    if named.cap_micro_usd.is_none() {
        named.cap_micro_usd = raw.get("cap_micro_usd").and_then(Value::as_u64);
    }
    if named.max_llm_calls.is_none() {
        named.max_llm_calls = raw.get("max_llm_calls").and_then(Value::as_u64);
    }
    named
}

/// The refusal of a `loom` member of the JSON form: every member is named with its new place.
fn old_place(loom: &Value) -> String {
    let Some(loom) = loom.as_object() else {
        return "loom: there is no `loom` table in the TOML form; its members are at the top level (docs/loom-run-settings.md)".to_owned();
    };
    let moved: Vec<String> = loom
        .keys()
        .map(|key| match OLD_PLACES.iter().find(|(old, _)| old == key) {
            Some((old, new)) => format!("loom.{old} is now `{new}`"),
            None if key == "bin" => "loom.bin is no setting: the binary is the option --loom".to_owned(),
            None => format!("loom.{key} is no setting"),
        })
        .collect();
    format!("loom: there is no `loom` table in the TOML form: {} (docs/loom-run-settings.md)", moved.join(", "))
}

/// Reads the file as the one format `runbook.loom-run-settings/1` (`docs/loom-run-settings.md`) and
/// returns the settings the loop, the decider and the bindings copy read. An unknown key of the top
/// level and a `loom` table are refused by name (the latter with the new place of its members). The
/// members that only repeat an option (`format`, `run`, `loom_sha256`, `in_process`, `confinement`,
/// `cap_micro_usd`, `max_llm_calls`, `max_wall_seconds`) are checked against the options and must
/// agree (a member of `cap_micro_usd` or `max_llm_calls` without its option names the cap, see
/// [`with_named_caps`]); the first six are taken out, in the order the file has the rest. A `decider.principal` of
/// a model or rule decider must be the one the kind gives (`decider:model`, `decider:<verdict>`) and
/// is taken out. Everything else is passed on untouched.
pub fn normalise(raw: &Value, args: &crate::loom_run::Args) -> Result<Value, String> {
    let mut v = raw.clone();
    let top = v.as_object_mut().ok_or("settings: a table")?;
    if let Some(loom) = top.get("loom") {
        return Err(old_place(loom));
    }
    for key in top.keys() {
        if !OPTION_KEYS.contains(&key.as_str()) && !crate::loom_run::bindings_copy::SETTINGS_KEYS.contains(&key.as_str()) {
            let known: Vec<&str> = OPTION_KEYS.iter().chain(crate::loom_run::bindings_copy::SETTINGS_KEYS.iter()).copied().collect();
            return Err(format!("settings.{key}: not a setting of the format (known: {})", known.join(", ")));
        }
    }
    if let Some(f) = top.shift_remove("format") {
        if f.as_str() != Some(FORMAT) {
            return Err(format!("format: {f} is not {FORMAT}"));
        }
    }
    if let Some(r) = top.shift_remove("run") {
        if r.as_str() != Some(args.run.as_str()) {
            return Err(format!("run: {r} is not the run named by --run"));
        }
    }
    if let Some(value) = top.shift_remove("loom_sha256") {
        match args.loom_sha256.as_deref() {
            Some(given) => {
                if value.as_str().map(|s| s.trim().to_ascii_lowercase()) != Some(given.trim().to_ascii_lowercase()) {
                    return Err("loom_sha256 is not the sha256 of --loom-sha256: the option decides; make them equal".to_owned());
                }
            }
            None => return Err("loom_sha256: Loom is a library of this process (no --loom), so there is no digest to name; take it out".to_owned()),
        }
    }
    if let Some(value) = top.shift_remove("in_process") {
        if value != Value::Bool(true) || !args.in_process() {
            return Err("in_process is true only when Loom is a library of this process (no --loom): make them equal".to_owned());
        }
    }
    if let Some(value) = top.shift_remove("confinement") {
        if value.as_str() != Some(args.confinement.as_arg()) {
            return Err(format!("confinement is {value}, the option --confinement is {}: make them equal", args.confinement.as_arg()));
        }
    }
    if let Some(value) = top.shift_remove("cap_micro_usd") {
        same_named(&value, args.cap_micro_usd, "cap_micro_usd", "--cap-micro-usd")?;
    }
    if let Some(n) = top.get("max_llm_calls") {
        same_named(n, args.max_llm_calls, "max_llm_calls", "--max-llm-calls")?;
    }
    if let Some(n) = top.get("max_wall_seconds") {
        same_number(n, args.max_wall_seconds, "max_wall_seconds", "--max-wall-seconds")?;
    }
    if let Some(decider) = top.get_mut("decider").and_then(Value::as_object_mut) {
        let kind = decider.get("kind").and_then(Value::as_str).map(str::to_owned);
        let want = match kind.as_deref() {
            Some("model") => Some("decider:model".to_owned()),
            Some("rule") => decider.get("verdict").and_then(Value::as_str).map(|v| format!("decider:{v}")),
            _ => None,
        };
        if let Some(want) = want {
            if let Some(p) = decider.remove("principal") {
                if p.as_str() != Some(want.as_str()) {
                    return Err(format!("decider.principal is {p}; a decider of kind {} acts as {want}", kind.unwrap_or_default()));
                }
            }
        }
    }
    Ok(v)
}

/// Fills, for a model decider with none, `decider.reply_bin` (`sdc-reply` found in `path`) before
/// the decider is built: the decider does not read it, the scripted adapter of a test may, so the
/// adapter keeps getting it as `SDC_REPLY`. A `decider.program` in the file is refused: the decider
/// runs inside the runner and starts no program of the bindings copy but the adapter.
pub fn fill_decider(settings: &mut Value, path: Option<&std::ffi::OsStr>) -> Result<(), String> {
    let Some(decider) = settings.get_mut("decider").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let kind = decider.get("kind").and_then(Value::as_str).unwrap_or("").to_owned();
    if kind != "model" && kind != "rule" {
        return Ok(());
    }
    if decider.contains_key("program") {
        return Err("decider.program: not a setting; the decider runs inside the runner and starts only the adapter program (decider.adapter)".to_owned());
    }
    if kind == "model" && !decider.contains_key("reply_bin") {
        let found = path.into_iter().flat_map(std::env::split_paths).map(|d| d.join("sdc-reply")).find(|p| p.is_file());
        if let Some(found) = found {
            decider.insert("reply_bin".to_owned(), Value::String(found.display().to_string()));
        }
    }
    Ok(())
}

/// The redacted settings with what the command was given added, as `RunStarted` carries them (the
/// members `sdc-experiment check --runs-dir` compares): `loom` (`sha256` of the binary, or
/// `in_process` and the pinned `commit` of the library; `confinement`, `cap_micro_usd`),
/// `max_llm_calls`, `max_wall_seconds`, `case`, `root_protocol`, and `config` when a `runbook.toml` gave a
/// value (its name and how it was found, never its path). The options win over a member of the same name
/// in the file. The path of the binary is not in them.
pub fn with_run(redacted: &Value, args: &crate::loom_run::Args, loom: &crate::loom_run::host::Identity) -> Value {
    use crate::loom_run::host::Identity;
    let mut v = redacted.clone();
    if let Some(o) = v.as_object_mut() {
        let mut which = match loom {
            Identity::Binary { sha256 } => serde_json::json!({"sha256": sha256}),
            Identity::Library { commit } => serde_json::json!({"in_process": true, "commit": commit}),
        };
        if let Some(w) = which.as_object_mut() {
            w.insert("confinement".to_owned(), Value::from(args.confinement.as_arg()));
            w.insert("cap_micro_usd".to_owned(), Value::from(args.cap_micro_usd));
        }
        o.insert("loom".to_owned(), which);
        o.insert("max_llm_calls".to_owned(), Value::from(args.max_llm_calls));
        o.insert("max_wall_seconds".to_owned(), Value::from(args.max_wall_seconds));
        o.insert("case".to_owned(), Value::from(args.case.clone()));
        o.insert("root_protocol".to_owned(), Value::from(args.root_protocol.clone()));
        if let Some(config) = &args.config {
            o.insert("config".to_owned(), Value::from(config.clone()));
        }
    }
    v
}

/// The settings as written into the run directory: `login_file` replaced by the text `named`.
pub fn redacted(settings: &Value) -> Value {
    let mut v = settings.clone();
    if let Some(o) = v.as_object_mut() {
        if o.contains_key(LOGIN_KEY) {
            o.insert(LOGIN_KEY.to_owned(), Value::String("named".to_owned()));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::testdir::Dir;
    use crate::loom_run::{Args, Confinement};
    use std::path::PathBuf;

    const DOC: &str = include_str!("../../../docs/loom-run-settings.md");
    const SHA: &str = "d80830ea5b1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef01234567";

    /// The first ```toml block of the page: the one complete example.
    fn example() -> String {
        let start = DOC.find("```toml\n").expect("the page has a toml example") + "```toml\n".len();
        let end = start + DOC[start..].find("```").expect("the example is closed");
        DOC[start..end].to_owned()
    }

    fn args() -> Args {
        Args {
            run: "lx.low.1".into(),
            runs_dir: PathBuf::from("runs"),
            loom: Some(PathBuf::from("b10x-loom")),
            loom_sha256: Some(SHA.into()),
            bindings: PathBuf::from("bindings"),
            protocols: PathBuf::from("protocols"),
            root_protocol: "sdc@8".into(),
            case: "IDEA-1".into(),
            intake: "text: x\n".into(),
            confinement: Confinement::Local,
            settings: Some(PathBuf::from("settings.toml")),
            defaults: Default::default(),
            spend_ledger: Some(PathBuf::from("ledger.json")),
            cap_micro_usd: Some(5_000_000),
            max_llm_calls: Some(100),
            max_wall_seconds: 3600,
            ports: Default::default(),
            config: None,
        }
    }

    #[test]
    fn a_refusal_of_the_reader_is_masked_by_parse_itself_for_a_form_that_only_the_whole_path_has() {
        // the keys `eyJa` and `eyJb` and the member `c`: no one segment of the path is a web token, the path is
        let head = ["ey", "Ja.eyJb"].concat();
        let e = parse(&format!("[{head}]\nc = 1979-05-27\n")).unwrap_err();
        assert!(e.contains("a date or a time") && !e.contains(&head) && e.contains("<a key or token>"), "{e}");
    }

    #[test]
    fn the_example_of_the_page_is_read_by_the_reader_and_taken_by_the_decider_and_the_bindings_copy() {
        let given = normalise(&parse(&example()).unwrap(), &args()).unwrap();
        let top = given.as_object().unwrap();
        for gone in ["format", "run", "loom_sha256", "in_process", "confinement", "cap_micro_usd"] {
            assert!(!top.contains_key(gone), "{gone} is in the settings the run reads");
        }
        assert_eq!(given["decider"]["max_decider_calls"], 40);
        assert_eq!(given["decider"]["max_budget_usd"], 0.25);
        assert_eq!(given["models"]["writer"]["max_budget_usd"], 2.5);
        assert_eq!(given["max_wall_seconds"], 3600);

        let dir = Dir::new("settings-example");
        let login = dir.path().join("login.json");
        std::fs::write(&login, "{}").unwrap();
        let mut settings = given.clone();
        settings[LOGIN_KEY] = Value::String(login.display().to_string());
        let mut filled = settings.clone();
        fill_decider(&mut filled, None).unwrap();
        crate::loom_run::decider::from_settings(&filled).expect("the decider takes the example");
        let committed = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bindings");
        let dst = dir.path().join("run/bindings");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        crate::loom_run::bindings_copy::materialise(&committed, &dst, "sdc", &settings, Some(&dir.path().join("login-link"))).expect("the bindings copy takes the example");
    }

    #[test]
    fn a_toml_number_is_a_whole_number_only_as_an_integer() {
        let v = parse("a = 3\nb = 3.0\nc = 1e3\nd = \"3\"\ne = -3\nf = true\n").unwrap();
        assert_eq!(v["a"].as_u64(), Some(3));
        assert_eq!(v["b"].as_u64(), None);
        assert!(v["b"].is_f64());
        assert_eq!(v["c"].as_u64(), None);
        assert_eq!(v["d"].as_u64(), None);
        assert_eq!(v["e"].as_u64(), None);
        assert_eq!(v["f"], Value::Bool(true));
        let big = parse("a = 18446744073709551616\n").unwrap_err();
        assert!(big.contains("not TOML"), "{big}");
        let big = parse("a = 9223372036854775808\n").unwrap_err();
        assert!(big.contains("not TOML"), "{big}");
        let top = parse("a = 9223372036854775807\n").unwrap();
        assert_eq!(top["a"].as_u64(), Some(i64::MAX as u64));
    }

    #[test]
    fn a_date_or_a_time_names_the_member_and_is_no_value() {
        for raw in ["1979-05-27", "1979-05-27T07:32:00Z", "1979-05-27T07:32:00", "07:32:00"] {
            let e = parse(&format!("[decider]\nretries = {raw}\n")).unwrap_err();
            assert!(e.contains("settings.decider.retries") && e.contains("date or a time"), "{raw}: {e}");
        }
        let e = parse("list = [1, 1979-05-27]\n").unwrap_err();
        assert!(e.contains("settings.list[1]"), "{e}");
    }

    #[test]
    fn the_order_of_the_file_is_the_order_of_the_settings() {
        let v = parse("zeta = 1\nalpha = 2\n[mid]\nb = 1\na = 2\n").unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["zeta", "alpha", "mid"]);
        let keys: Vec<&String> = v["mid"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["b", "a"]);
    }

    #[test]
    fn the_members_taken_out_leave_the_others_in_their_order() {
        let v = parse("format = \"runbook.loom-run-settings/1\"\nautonomy = \"A5\"\nrun = \"lx.low.1\"\nadapter = \"x\"\nconfinement = \"local\"\nmax_llm_calls = 100\n").unwrap();
        let given = normalise(&v, &args()).unwrap();
        let keys: Vec<&String> = given.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["autonomy", "adapter", "max_llm_calls"]);
    }

    #[test]
    fn a_json_text_is_refused_by_its_content_and_a_table_header_is_not_json() {
        for text in ["{}", "  \n{\"run\": \"r1\"}", "{\"run\": ", "[1, 2]"] {
            let e = parse(text).unwrap_err();
            assert!(e.contains("JSON") && e.contains("TOML") && e.contains("docs/loom-run-settings.md"), "{text}: {e}");
        }
        assert!(parse("[decider]\nkind = \"rule\"\nverdict = \"approve\"\n").is_ok());
        assert!(parse("").is_ok());
    }

    #[test]
    fn a_loom_table_names_each_member_with_its_new_place() {
        let e = normalise(&parse("[loom]\nsha256 = \"x\"\nlogin_file = \"/a\"\nbin = \"/b\"\nodd = 1\n").unwrap(), &args()).unwrap_err();
        for part in ["loom.sha256 is now `loom_sha256`", "loom.login_file is now `login_file`", "loom.bin is no setting", "loom.odd is no setting"] {
            assert!(e.contains(part), "{part}: {e}");
        }
        let e = normalise(&parse("loom = \"local\"\n").unwrap(), &args()).unwrap_err();
        assert!(e.contains("no `loom` table"), "{e}");
    }

    #[test]
    fn a_path_with_a_quote_a_backslash_and_a_letter_outside_ascii_reads_back_as_written() {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/loom-run-settings-odd-path.toml")).unwrap();
        let v = parse(&text).unwrap();
        assert_eq!(v[LOGIN_KEY], "/opt/we\"ird\\dir/ünï/login.json");
        let given = normalise(&v, &args()).unwrap();
        assert_eq!(given[LOGIN_KEY], "/opt/we\"ird\\dir/ünï/login.json");
    }

    #[test]
    fn a_syntax_error_names_the_line_the_column_and_the_parsers_words_and_none_of_the_text() {
        let e = parse("a = 1\nb = 2\nsecret_member = SECRET-VALUE-77 more\n").unwrap_err();
        assert!(e.starts_with("not TOML at line 3, column 17"), "{e}");
        assert!(!e.contains("SECRET-VALUE-77") && !e.contains("secret_member"), "{e}");
        let e = parse("\"\u{fc}n\u{ef}\" = oops\n").unwrap_err();
        assert!(e.starts_with("not TOML at line 1, column 9"), "{e}");
        let e = parse("a = 1\na = 2\n").unwrap_err();
        assert!(e.contains("line 2") && e.contains("duplicate key"), "{e}");
        let e = parse(&format!("a = {}\n", "x".repeat(1 << 20))).unwrap_err();
        assert!(e.len() < 1024, "{} bytes", e.len());
    }

    #[test]
    fn a_json_text_after_a_mark_blank_lines_or_comment_lines_is_still_json() {
        for text in ["\u{feff}{}", "\u{feff}\n\n{}", "# c\n{}", "\u{feff}# c\n\n# d\n  {\"a\": 1}", "# c\n[1, 2]", "#c\r\n{\"a\": 1}"] {
            let e = parse(text).unwrap_err();
            assert!(e.contains("JSON") && e.contains("docs/loom-run-settings.md"), "{text:?}: {e}");
        }
        assert!(parse("# only a comment\n").is_ok());
        assert!(parse("# a comment\n[decider]\nkind = \"rule\"\nverdict = \"approve\"\n").is_ok());
    }

    #[test]
    fn a_whole_number_has_the_spellings_of_a_toml_integer_and_no_float_is_one() {
        let v = parse("hex = 0x10\noctal = 0o20\nbinary = 0b10\nunderscored = 1_000\nplus = +5\nplain = 16\n").unwrap();
        for (member, want) in [("hex", 16), ("octal", 16), ("binary", 2), ("underscored", 1000), ("plus", 5), ("plain", 16)] {
            assert_eq!(v[member].as_u64(), Some(want), "{member}");
        }
        for raw in ["1.0", "1e3", "-0.0", "0.5"] {
            let v = parse(&format!("x = {raw}\n")).unwrap();
            assert!(v["x"].is_f64() && v["x"].as_u64().is_none(), "{raw}: {}", v["x"]);
        }
        for raw in ["inf", "+inf", "-inf", "nan", "-nan", "1e400"] {
            let e = parse(&format!("x = {raw}\n")).unwrap_err();
            assert!(e.contains("settings.x"), "{raw}: {e}");
        }
    }
}
