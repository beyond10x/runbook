//! The guards of confinement, caps and the login under the TOML form of `runbook run --settings`: they
//! must not have moved when the input form of the settings file changed.

use super::core::{code, grep_tree, stderr, toml_text, World};
use super::decider::{fixture_settings, install_decider, tree_with_approval, write_settings, APPROVE};
use serde_json::{json, Value};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

fn tag(prefix: &str) -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    format!("r1{prefix}{}", N.fetch_add(1, Ordering::Relaxed))
}

fn base_text(w: &World) -> String {
    toml_text(&fixture_settings(w, 5_000_000))
}

/// `text` with the top-level lines that set one of `drop` taken out and `fragment` put first.
fn top_with(text: &str, drop: &[&str], fragment: &str) -> String {
    let mut out = String::from(fragment);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let mut top = true;
    for l in text.lines() {
        if l.starts_with('[') {
            top = false;
        }
        if top && drop.iter().any(|k| l.starts_with(&format!("{k} = "))) {
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// `text` with the line `key = raw` put into `table` (`None`: the top level).
fn member(text: &str, table: Option<&str>, key: &str, raw: &str) -> String {
    let line = format!("{key} = {raw}");
    let header = table.map(|t| format!("[{t}]"));
    let drop = format!("{key} = ");
    let mut out: Vec<String> = Vec::new();
    let mut here = table.is_none();
    let mut placed = false;
    for l in text.lines() {
        if l.starts_with('[') {
            if here && !placed {
                out.push(line.clone());
                placed = true;
            }
            here = header.as_deref() == Some(l);
            out.push(l.to_owned());
        } else if !(here && l.starts_with(&drop)) {
            out.push(l.to_owned());
        }
    }
    if !placed {
        match (here, header) {
            (true, _) => out.push(line),
            (false, Some(h)) => out.extend([String::new(), h, line]),
            (false, None) => unreachable!(),
        }
    }
    out.join("\n") + "\n"
}

fn run_bytes(w: &World, bytes: &[u8], mutate_args: impl Fn(&mut Vec<String>)) -> Output {
    std::fs::write(w.root.join("settings.toml"), bytes).unwrap();
    let mut args = w.args("r1", 5_000_000);
    mutate_args(&mut args);
    w.run(&args)
}

fn run_text(w: &World, text: &str) -> Output {
    run_bytes(w, text.as_bytes(), |_| {})
}

/// A refusal before anything starts: exit 2, no run directory, no ledger, no call; `needles` named.
fn assert_refused(w: &World, o: &Output, what: &str, needles: &[&str]) {
    assert_eq!(code(o), 2, "{what}: {}", stderr(o));
    for needle in needles {
        assert!(stderr(o).contains(needle), "{what}: {needle} not named: {}", stderr(o));
    }
    assert!(!w.run_dir("r1").exists(), "{what}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{what}: the binary was called");
    assert!(!w.ledger().exists(), "{what}: the ledger was made");
}

#[test]
fn the_confinement_of_the_file_never_sets_or_lowers_the_level_in_any_toml_form() {
    for (what, fragment, drop, needle) in [
        ("array", "confinement = [\"local\"]", true, "confinement"),
        ("number", "confinement = 1", true, "confinement"),
        ("empty", "confinement = \"\"", true, "confinement"),
        ("case", "confinement = \"Local\"", true, "confinement"),
        ("trailing space", "confinement = \"local \"", true, "confinement"),
        ("multi-line basic", "confinement = \"\"\"local\n\"\"\"", true, "confinement"),
        ("multi-line literal", "confinement = '''\nlocal\n'''", true, "confinement"),
        ("inline table", "confinement = { level = \"local\" }", true, "confinement"),
        ("dotted key", "loom.confinement = \"local\"", true, "loom.confinement"),
        ("inline loom table", "loom = { confinement = \"local\" }", true, "loom.confinement"),
        ("key with a space", "\"confinement \" = \"local\"", true, "settings.confinement "),
        ("key in another case", "Confinement = \"local\"", true, "settings.Confinement"),
        ("key twice", "confinement = \"substrate\"", false, "duplicate"),
    ] {
        let w = World::new(&tag("conf"));
        let text = top_with(&base_text(&w), if drop { &["confinement"] } else { &[] }, fragment);
        let o = run_text(&w, &text);
        assert_refused(&w, &o, what, &[needle]);
    }
    // the option is the other level: the file's "local" does not lower it
    let w = World::new(&tag("conf"));
    let o = run_bytes(&w, base_text(&w).as_bytes(), |a| {
        let i = a.iter().position(|x| x == "--confinement").unwrap();
        a[i + 1] = "substrate".into();
    });
    assert_refused(&w, &o, "option substrate, file local", &["confinement is"]);
    // the file names a level and the option is absent: a usage error, nothing written
    let w = World::new(&tag("conf"));
    let o = run_bytes(&w, base_text(&w).as_bytes(), |a| {
        let i = a.iter().position(|x| x == "--confinement").unwrap();
        a.drain(i..=i + 1);
    });
    assert_eq!(code(&o), 64, "{}", stderr(&o));
    assert!(!w.run_dir("r1").exists());
    assert_eq!(w.fake.calls_so_far(), 0);
}

#[test]
fn a_cap_that_equals_the_option_in_another_notation_is_the_option_and_nothing_else_is_read() {
    let w = World::new(&tag("capok"));
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let mut text = base_text(&w);
    text = member(&text, None, "cap_micro_usd", "0x4C4B40");
    text = member(&text, None, "max_llm_calls", "1_0_0");
    text = member(&text, None, "max_wall_seconds", "0o1130");
    let o = run_text(&w, &text);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["loom"]["cap_micro_usd"], 5_000_000);
    assert_eq!(recorded["max_wall_seconds"], 600);
    assert_eq!(recorded["max_llm_calls"], 100);
}

#[test]
fn an_option_bound_cap_is_refused_for_every_value_that_is_not_the_option() {
    let values = [
        ("float", "100.0"),
        ("exponent", "1e3"),
        ("inf", "inf"),
        ("plus inf", "+inf"),
        ("nan", "nan"),
        ("minus nan", "-nan"),
        ("text", "\"100\""),
        ("bool", "true"),
        ("date", "1979-05-27"),
        ("array", "[100]"),
        ("inline table", "{ a = 1 }"),
        ("negative", "-100"),
        ("u64 max", "18446744073709551615"),
        ("i64 max", "9223372036854775807"),
        ("hex that is another number", "0x10"),
        ("underscored that is another number", "1_000"),
        ("zero", "0"),
        ("plus sign, another number", "+1"),
    ];
    for key in ["cap_micro_usd", "max_llm_calls", "max_wall_seconds"] {
        for (what, raw) in values {
            let w = World::new(&tag("cap"));
            let o = run_text(&w, &member(&base_text(&w), None, key, raw));
            assert_refused(&w, &o, &format!("{key} = {what}"), &[key]);
        }
    }
}

#[test]
fn a_decider_or_model_limit_is_refused_for_every_value_that_is_no_finite_whole_or_positive_number() {
    let bad_whole = ["inf", "+inf", "-inf", "nan", "true", "[5]", "{ a = 1 }", "1e400", "0.5", "-0", "0"];
    for (table, key, min_one) in [
        (Some("decider"), "max_decider_calls", true),
        (Some("decider"), "max_turns", true),
        (Some("decider"), "timeout_seconds", true),
        (Some("decider"), "retries", false),
        (Some("models.writer"), "max_turns", true),
    ] {
        for raw in bad_whole {
            if raw == "0" && !min_one || raw == "-0" && !min_one {
                continue;
            }
            let w = World::new(&tag("lim"));
            let o = run_text(&w, &member(&base_text(&w), table, key, raw));
            if table.is_some_and(|t| t.starts_with("models")) {
                assert_ne!(code(&o), 0, "{key} = {raw}: {}", stderr(&o));
            } else {
                assert_refused(&w, &o, &format!("{key} = {raw}"), &[key]);
            }
        }
    }
    let bad_usd = ["inf", "+inf", "-inf", "nan", "-nan", "1e400", "0", "0.0", "-0.0", "-1", "true", "\"1\"", "[1]", "{ a = 1 }"];
    for (table, key) in [(Some("decider"), "max_budget_usd"), (Some("models.writer"), "max_budget_usd")] {
        for raw in bad_usd {
            let w = World::new(&tag("usd"));
            let o = run_text(&w, &member(&base_text(&w), table, key, raw));
            if table.is_some_and(|t| t.starts_with("models")) {
                assert_ne!(code(&o), 0, "{key} = {raw}: {}", stderr(&o));
            } else {
                assert_refused(&w, &o, &format!("{key} = {raw}"), &[key]);
            }
        }
    }
}

#[test]
fn json_in_any_dress_is_refused_by_content_and_a_table_that_looks_like_json_is_no_setting() {
    let w = World::new(&tag("json"));
    let json = serde_json::to_string_pretty(&fixture_settings(&w, 5_000_000)).unwrap();
    for (what, bytes, needles) in [
        ("after a BOM", format!("\u{feff}{json}"), vec!["--settings"]),
        ("after blank lines", format!("\n\n\n{json}"), vec!["TOML", "docs/loom-run-settings.md"]),
        ("after a comment line", format!("# the settings\n{json}"), vec!["--settings"]),
        ("an empty object", "{}".to_owned(), vec!["TOML", "docs/loom-run-settings.md"]),
        ("an object that is also TOML-ish", "a = {b = 1}".to_owned(), vec!["settings.a"]),
        ("an inline table at the top", "{ run = \"r1\" }".to_owned(), vec!["TOML"]),
    ] {
        let o = run_bytes(&w, bytes.as_bytes(), |_| {});
        assert_refused(&w, &o, what, &needles);
    }
}

#[test]
fn a_json_file_is_refused_as_json_and_not_only_as_a_toml_syntax_error() {
    // The refusal by content says what the file is. A TOML parse error also names `TOML` and the
    // page, so only the word JSON tells the two apart: without the sniff a person reads a syntax
    // error of a line of their old file instead of the way to convert it.
    let w = World::new(&tag("jsonword"));
    let json = serde_json::to_string_pretty(&fixture_settings(&w, 5_000_000)).unwrap();
    for (what, text) in [("pretty", json.clone()), ("compact", json.replace(['\n', ' '], "")), ("empty object", "{}".to_owned()), ("blank lines first", format!("\n\n{json}"))] {
        let o = run_text(&w, &text);
        assert_refused(&w, &o, what, &["JSON", "docs/loom-run-settings.md"]);
        assert!(!stderr(&o).contains("TOML parse error"), "{what}: a syntax error stands in for the refusal: {}", stderr(&o));
    }
}

#[test]
fn a_file_that_is_no_text_or_no_file_is_refused_before_anything_starts() {
    let w = World::new(&tag("bin"));
    let o = run_bytes(&w, &[0, 1, 2, 0xff, 0xfe, 0x80], |_| {});
    assert_refused(&w, &o, "binary", &["--settings"]);
    let o = run_bytes(&w, b"a = \"\xff\"\n", |_| {});
    assert_refused(&w, &o, "invalid UTF-8 in a string", &["--settings"]);
    let dir = w.root.join("a-directory");
    std::fs::create_dir_all(&dir).unwrap();
    let o = run_bytes(&w, b"", |a| {
        let i = a.iter().position(|x| x == "--settings").unwrap();
        a[i + 1] = dir.display().to_string();
    });
    assert_refused(&w, &o, "a directory", &["--settings"]);
    // a link to a JSON file outside the world is read through the link and refused by content
    let outside = w.root.parent().unwrap().join(format!("outside-{}.json", std::process::id()));
    std::fs::write(&outside, b"{\"run\": \"r1\"}").unwrap();
    let link = w.root.join("link.toml");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let o = run_bytes(&w, b"", |a| {
        let i = a.iter().position(|x| x == "--settings").unwrap();
        a[i + 1] = link.display().to_string();
    });
    let _ = std::fs::remove_file(&outside);
    assert_refused(&w, &o, "a link to JSON", &["TOML"]);
}

#[test]
fn an_old_place_in_any_toml_spelling_is_refused_with_the_new_place_named_and_never_read() {
    for (fragment, drop, needles) in [
        ("loom.sha256 = \"00\"", "loom_sha256", vec!["loom.sha256 is now `loom_sha256`"]),
        ("loom.in_process = true", "in_process", vec!["loom.in_process is now `in_process`"]),
        ("loom.confinement = \"local\"", "confinement", vec!["loom.confinement is now `confinement`"]),
        ("loom.cap_micro_usd = 5000000", "cap_micro_usd", vec!["loom.cap_micro_usd is now `cap_micro_usd`"]),
        ("loom.login_file = \"/x\"", "none", vec!["loom.login_file is now `login_file`"]),
        ("loom.bin = \"/x\"", "none", vec!["loom.bin is no setting"]),
        ("loom = {}", "none", vec!["no `loom` table"]),
        ("loom = []", "none", vec!["no `loom` table"]),
        ("loom = 5", "none", vec!["no `loom` table"]),
        ("[[loom]]\nbin = \"/x\"", "none", vec!["no `loom` table"]),
    ] {
        let w = World::new(&tag("old"));
        let mut text = base_text(&w);
        if fragment.starts_with("[[") {
            text.push_str(&format!("\n{fragment}\n"));
        } else {
            text = top_with(&text, &[drop], fragment);
        }
        // the new place is still in the file for the cases that keep it: both places, one refusal
        let o = run_text(&w, &text);
        assert_refused(&w, &o, fragment, &needles);
    }
    // one more place per table: an unknown key of a nested table and a table of the top
    for (table, key, name) in [
        (Some("decider"), "colour", "decider.colour"),
        (None, "max-llm-calls", "settings.max-llm-calls"),
        (None, "Autonomy", "settings.Autonomy"),
    ] {
        let w = World::new(&tag("unk"));
        let o = run_text(&w, &member(&base_text(&w), table, key, "\"x\""));
        assert_refused(&w, &o, name, &[name]);
    }
    let w = World::new(&tag("unk"));
    let o = run_text(&w, &format!("{}\n[colour]\nx = 1\n", base_text(&w)));
    assert_refused(&w, &o, "unknown top-level table", &["settings.colour"]);
    let w = World::new(&tag("unk"));
    let o = run_text(&w, &format!("{}\n[decider.sub]\nx = 1\n", base_text(&w)));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
}

#[test]
fn a_duplicate_key_or_table_is_refused_by_the_reader_before_a_run_directory_exists() {
    for (what, extra) in [
        ("max_llm_calls twice", "max_llm_calls = 100"),
        ("cap twice", "cap_micro_usd = 1"),
        ("decider twice", "[decider]\nkind = \"rule\"\nverdict = \"approve\""),
        ("review twice", "[review]\nadapter = \"claude-cli\""),
    ] {
        let w = World::new(&tag("dup"));
        let text = if extra.starts_with('[') { format!("{}\n{extra}\n", base_text(&w)) } else { top_with(&base_text(&w), &[], extra) };
        let o = run_text(&w, &text);
        assert_refused(&w, &o, what, &["--settings", "duplicate"]);
    }
}

#[test]
fn a_toml_syntax_error_on_the_login_line_does_not_print_the_login_path() {
    for (what, line) in [
        ("an unquoted path", "login_file = LOGINPATH"),
        ("a second definition of the key", "login_file = \"LOGINPATH\""),
        ("junk after the string", "login_file = \"LOGINPATH\" junk"),
        ("an unterminated string", "login_file = \"LOGINPATH"),
    ] {
        let w = World::new(&tag("echo"));
        let login = w.login_path().display().to_string();
        let line = line.replace("LOGINPATH", &login);
        let text = top_with(&base_text(&w), &[], &line);
        let o = run_text(&w, &text);
        assert_eq!(code(&o), 2, "{what}: {}", stderr(&o));
        assert!(!stderr(&o).contains("loginhome"), "{what}: the refusal prints the login path: {}", stderr(&o));
    }
}

#[test]
fn a_toml_syntax_error_on_a_pasted_login_content_does_not_print_it() {
    for (what, line) in [
        ("a JSON object as the value", "login_file = {\"accessToken\": \"SECRET-TOKEN-8842\"}"),
        ("the content unquoted", "login_file = SECRET-TOKEN-8842 SECRET"),
        ("the content as a table", "login_file = {\"accessToken\" = \"SECRET-TOKEN-8842\"}"),
        ("the content as an array", "login_file = [\"SECRET-TOKEN-8842\"]"),
    ] {
        let w = World::new(&tag("secret"));
        let text = top_with(&base_text(&w), &["login_file"], line);
        let o = run_text(&w, &text);
        assert_eq!(code(&o), 2, "{what}: {}", stderr(&o));
        assert!(!stderr(&o).contains("SECRET-TOKEN-8842"), "{what}: the refusal prints the content: {}", stderr(&o));
    }
}

#[test]
fn an_old_json_file_after_a_bom_is_refused_without_printing_the_line_that_holds_its_login_path() {
    let w = World::new(&tag("bom"));
    let json = serde_json::to_string(&json!({"run": "r1", "loom": {"login_file": w.login_path().display().to_string()}})).unwrap();
    let o = run_bytes(&w, format!("\u{feff}{json}").as_bytes(), |_| {});
    assert_refused(&w, &o, "BOM then compact JSON", &["--settings"]);
    assert!(!stderr(&o).contains("loginhome"), "the refusal prints the login path: {}", stderr(&o));
}

#[test]
fn a_login_path_with_a_quote_a_backslash_and_a_letter_outside_ascii_reaches_no_record_and_no_output() {
    let w = World::new(&tag("odd"));
    install_decider(&w, APPROVE);
    tree_with_approval(&w, "0.30");
    let odd = w.root.join("we\"ird\\dir").join("\u{fc}n\u{ef}");
    std::fs::create_dir_all(&odd).unwrap();
    let file = odd.join("login.json");
    std::fs::write(&file, "{\"token\":\"ODD-LOGIN-CONTENT-77\"}").unwrap();
    let mut v = fixture_settings(&w, 5_000_000);
    v["login_file"] = json!(file.display().to_string());
    write_settings(&w, &v);
    let o = w.run(&w.args("r1", 5_000_000));
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let all = format!("{}{}", String::from_utf8_lossy(&o.stdout), stderr(&o));
    for needle in ["we\"ird", "ODD-LOGIN-CONTENT-77", "\u{fc}n\u{ef}"] {
        assert!(!all.contains(needle), "{needle} is in the output: {all}");
        let mut hits = Vec::new();
        grep_tree(&w.run_dir("r1"), needle, &mut hits);
        assert!(hits.is_empty(), "{needle} found in {hits:?}");
    }
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(w.run_dir("r1/settings.json")).unwrap()).unwrap();
    assert_eq!(recorded["login_file"], "named");
}
