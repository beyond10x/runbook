//! No refusal of the settings reader prints a line, a value or a part of a value of the settings file: not at
//! any position of a syntax error, not for any number a setting cannot take. The reader walks the parser's
//! syntax tree itself, so what it prints is what these cases read.

use super::core::{code, stderr, toml_text, World};
use super::decider::fixture_settings;
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

const S: &str = "LOGINSECRET-9f3a";

fn tag(prefix: &str) -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    format!("r2{prefix}{}", N.fetch_add(1, Ordering::Relaxed))
}

fn base_text(w: &World) -> String {
    toml_text(&fixture_settings(w, 5_000_000))
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

fn run_bytes(w: &World, bytes: &[u8]) -> Output {
    std::fs::write(w.root.join("settings.toml"), bytes).unwrap();
    w.run(&w.args("r1", 5_000_000))
}

fn all_output(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), stderr(o))
}

fn assert_before_anything(w: &World, o: &Output, what: &str) {
    assert_eq!(code(o), 2, "{what}: {}", stderr(o));
    assert!(!w.run_dir("r1").exists(), "{what}: a run directory was made");
    assert_eq!(w.fake.calls_so_far(), 0, "{what}: the binary was called");
    assert!(!w.ledger().exists(), "{what}: the ledger was made");
}

/// Every kind of syntax error the parser has, each with the secret inside the bad text.
fn syntax_errors() -> Vec<(&'static str, String)> {
    let s = S;
    let raw = |bytes: &[u8]| String::from_utf8(bytes.to_vec()).unwrap();
    vec![
        ("bare value", format!("confinement = {s}")),
        ("bare value then junk", format!("x = {s} {s}")),
        ("missing value", format!("{s} =")),
        ("key with a space", format!("{s} {s} = 1")),
        ("two quoted keys", format!("\"{s}\" \"{s}\" = 1")),
        ("dotted key unfinished", format!("{s}.{s}. = 1")),
        ("key with no equals", s.to_owned()),
        ("extra equals", format!("{s} = = {s}")),
        ("unclosed table header", format!("[models.{s}")),
        ("unclosed array header", format!("[[{s}]")),
        ("extra close", format!("[{s}]]")),
        ("space in header", format!("[{s} {s}]")),
        ("empty header part", format!("[{s}.]")),
        ("inline table, double comma", format!("x = {{ {s} = 1,, }}")),
        ("inline table, no value", format!("x = {{ {s} = }}")),
        ("inline table, unclosed", format!("x = {{ {s} = 1")),
        ("inline table, no comma", format!("x = {{ {s} = 1 {s} = 2 }}")),
        ("array, no comma", format!("x = [ \"{s}\" \"{s}\" ]")),
        ("array, extra comma", format!("x = [ \"{s}\", , ]")),
        ("array, unclosed", format!("x = [ \"{s}\"")),
        ("multi-line basic, unclosed", format!("x = \"\"\"{s}\n{s}")),
        ("multi-line literal, unclosed", format!("x = '''{s}\n{s}")),
        ("basic string, unclosed", format!("x = \"{s}")),
        ("literal string, unclosed", format!("x = '{s}")),
        ("bad escape", format!("x = \"{s}\\q{s}\"")),
        ("short unicode escape", format!("x = \"{s}\\u12{s}\"")),
        ("surrogate escape", format!("x = \"{s}\\uD800{s}\"")),
        ("control char in a string", format!("x = \"{s}{}{s}\"", raw(&[1]))),
        ("control char in a comment", format!("# {s}{}{s}", raw(&[1]))),
        ("delete char in a string", format!("x = \"{s}{}{s}\"", raw(&[0x7f]))),
        ("lone carriage return", format!("x = 1\r{s}")),
        ("date with letters", format!("x = 1979-{s}")),
        ("time with letters", format!("x = 1979-05-27T07:32:00.{s}Z")),
        ("date that is no date", format!("x = 2024-02-30 {s}")),
        ("hex with letters", format!("x = 0x{s}")),
        ("underscores", format!("x = 1__0 {s}")),
        ("float with letters", format!("x = 1.{s}")),
        ("duplicate key named by the secret", format!("{s} = 1\n{s} = 2")),
        ("duplicate table named by the secret", format!("[{s}]\n[{s}]")),
        ("key then table of the same name", format!("{s} = 1\n[{s}]")),
        ("array then dotted key", format!("{s} = [1]\n{s}.x = 1")),
        ("inline table extended", format!("{s} = {{ a = 1 }}\n{s}.b = 2")),
        ("dotted key into a string", format!("{s} = \"v\"\n{s}.x = 1")),
        ("a table through an array of tables mixed", format!("[[{s}]]\n[{s}]")),
        ("recursion limit", format!("x = {}{s}{}", "[".repeat(2000), "]".repeat(2000))),
        ("recursion limit, inline tables", format!("x = {}", "{ a = ".repeat(2000))),
        ("recursion limit, headers", format!("{}{s}{}", "[".repeat(2000), "]".repeat(2000))),
        ("a key that is a multi-line string", format!("\"\"\"{s}\"\"\" = 1")),
        ("a key that is a multi-line literal", format!("'''{s}''' = 1")),
    ]
}

/// Runs `part` for `0..parts` on a thread each and returns what they return. A row of these matrices costs
/// one process start of `runbook run` (about 30 ms, 2 s for a row of 8 MiB); the rows
/// share nothing, so each part has a `World` of its own and the parts run beside each other.
fn in_parallel<R: Send>(parts: usize, part: impl Fn(usize) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let part = &part;
        let threads: Vec<_> = (0..parts).map(|i| scope.spawn(move || part(i))).collect();
        threads.into_iter().map(|t| t.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    })
}

/// The placements of one bad line in a settings file; `syntax_matrix_at` takes each on a thread of its own.
const PLACEMENTS: usize = 7;

#[test]
fn a_syntax_error_prints_none_of_the_text_at_any_position_and_any_neighbour_line() {
    let seen: usize = in_parallel(PLACEMENTS, syntax_matrix_at).into_iter().sum();
    assert!(seen > 300, "{seen} cases");
    assert_eq!(seen, syntax_errors().len() * PLACEMENTS, "a placement of the matrix is not run");
}

/// Every syntax error of `syntax_errors` at the placement number `placement`; returns the rows it ran.
fn syntax_matrix_at(placement: usize) -> usize {
    let w = World::new(&tag("syn"));
    let base = base_text(&w);
    let mut seen = 0;
    for (what, bad) in syntax_errors() {
        for (place, text) in [
            ("alone", format!("{bad}\n")),
            ("before the settings", format!("{bad}\n{base}")),
            ("after a line that holds the secret", format!("prev = \"{S}\"\n{bad}\n")),
            ("before a line that holds the secret", format!("{bad}\nnext = \"{S}\"\n")),
            ("inside a table named by the secret", format!("[{S}]\nok = 1\n{bad}\n")),
            ("after a comment that holds the secret", format!("# {S}\n{bad}\n")),
            ("after a bom", format!("\u{feff}{bad}\n")),
        ]
        .into_iter()
        .skip(placement)
        .take(1)
        {
            let o = run_bytes(&w, text.as_bytes());
            let what = format!("{what}, {place}");
            if code(&o) == 0 || w.run_dir("r1").exists() {
                panic!("{what}: the file was taken: {}", all_output(&o));
            }
            assert_before_anything(&w, &o, &what);
            let all = all_output(&o);
            assert!(!all.contains(S), "{what}: the secret is on the output: {all}");
            assert!(!all.contains("LOGINSECRET"), "{what}: part of the secret is on the output: {all}");
            assert!(stderr(&o).len() < 700, "{what}: {} bytes: {all}", stderr(&o).len());
            seen += 1;
        }
    }
    seen
}

#[test]
fn a_file_that_is_not_utf8_prints_none_of_its_text() {
    let w = World::new(&tag("utf"));
    for (what, bytes) in [
        ("a bad byte in a string", [format!("a = \"{S}").as_bytes(), &[0xff], format!("{S}\"\n").as_bytes()].concat()),
        ("a bad byte in a key", [format!("{S}").as_bytes(), &[0xc3, 0x28], b" = 1\n".as_slice(), format!("# {S}\n").as_bytes()].concat()),
        ("a bad byte after a table named by the secret", [format!("[{S}]\n").as_bytes(), &[0xfe, 0xfe], b"\n".as_slice()].concat()),
        ("a truncated sequence at the end", [format!("a = \"{S}\"\n").as_bytes(), &[0xe2, 0x82]].concat()),
        ("a UTF-16 mark and text", {
            let mut v = vec![0xff, 0xfe];
            for c in format!("a = \"{S}\"").encode_utf16() {
                v.extend(c.to_le_bytes());
            }
            v
        }),
        ("a surrogate encoded as UTF-8", [format!("a = \"{S}").as_bytes(), &[0xed, 0xa0, 0x80], b"\"\n".as_slice()].concat()),
    ] {
        let o = run_bytes(&w, &bytes);
        assert_before_anything(&w, &o, what);
        let all = all_output(&o);
        assert!(!all.contains("LOGINSECRET"), "{what}: the secret is on the output: {all}");
        assert!(stderr(&o).len() < 700, "{what}: {} bytes", stderr(&o).len());
    }
}

/// The spellings of a number a setting cannot take, each with the text it must not echo.
const BAD_NUMBERS: [&str; 29] = [
    "18446744073714551616",
    "18446744073709551716",
    "18446744073709555216",
    "1979-05-27",
    "1979-05-27T07:32:00Z",
    "07:32:00",
    "1979-05-27 07:32:00-08:00",
    "18446744073709551616",
    "9223372036854775808",
    "-9223372036854775809",
    "0xFFFFFFFFFFFFFFFFF",
    "0o7777777777777777777777",
    "0b11111111111111111111111111111111111111111111111111111111111111111",
    "340282366920938463463374607431768211456",
    "99999999999999999999999999999999999999999",
    "1e400",
    "-1e400",
    "inf",
    "+inf",
    "-inf",
    "nan",
    "+nan",
    "-nan",
    "1e999",
    "1.7976931348623157e309",
    "123456789012345678901234567890.5e300",
    "1_0e400",
    "9_223_372_036_854_775_808",
    "0x8000_0000_0000_0000",
];

/// The whole-number members the matrix puts each bad number into; `number_matrix_at` takes each on a thread of its own.
const NUMBER_MEMBERS: [(Option<&str>, &str); 11] = [
    (None, "cap_micro_usd"),
    (None, "max_llm_calls"),
    (None, "max_wall_seconds"),
    (Some("decider"), "max_decider_calls"),
    (Some("decider"), "max_turns"),
    (Some("decider"), "timeout_seconds"),
    (Some("decider"), "retries"),
    (Some("decider"), "max_budget_usd"),
    (Some("models.writer"), "max_turns"),
    (Some("models.writer"), "max_budget_usd"),
    (Some("review"), "max_budget_usd"),
];

#[test]
fn a_number_a_setting_cannot_take_is_refused_with_the_member_named_and_the_value_not_printed() {
    let seen: usize = in_parallel(NUMBER_MEMBERS.len(), number_matrix_at).into_iter().sum();
    assert_eq!(seen, NUMBER_MEMBERS.len() * BAD_NUMBERS.len(), "a member of the matrix is not run");
}

/// Every bad number in the member number `member_at` of `NUMBER_MEMBERS`; returns the rows it ran.
fn number_matrix_at(member_at: usize) -> usize {
    let w = World::new(&tag("num"));
    let base = base_text(&w);
    let mut seen = 0;
    for (table, key) in NUMBER_MEMBERS.into_iter().skip(member_at).take(1) {
        for raw in BAD_NUMBERS {
            let at = format!("settings.{}{key}", table.map(|t| format!("{t}.")).unwrap_or_default());
            let o = run_bytes(&w, member(&base, table, key, raw).as_bytes());
            let what = format!("{key} = {raw}");
            assert_before_anything(&w, &o, &what);
            let err = stderr(&o);
            assert!(err.contains(&at), "{what}: the member {at} is not named: {err}");
            let digits: String = raw.trim_start_matches(['-', '+']).chars().take_while(|c| c.is_ascii_digit()).take(12).collect();
            assert!(!err.contains(raw) && (digits.len() < 8 || !err.contains(&digits)), "{what}: the value is on the output: {err}");
            assert!(err.len() < 700, "{what}: {} bytes", err.len());
            seen += 1;
        }
    }
    seen
}

#[test]
fn a_number_a_setting_cannot_take_is_refused_inside_an_array_an_inline_table_and_an_array_of_tables() {
    let w = World::new(&tag("deep"));
    let base = base_text(&w);
    for raw in BAD_NUMBERS {
        for (what, fragment, member_name) in [
            ("array", format!("deep = [ 1, {raw} ]"), "settings.models.writer.deep[1]"),
            ("inline table", format!("deep = {{ a = {{ b = {raw} }} }}"), "settings.models.writer.deep.a.b"),
            ("array of inline tables", format!("deep = [ {{ x = {raw} }} ]"), "settings.models.writer.deep[0].x"),
            ("dotted key", format!("deep.a.b = {raw}"), "settings.models.writer.deep.a.b"),
        ] {
            let text = format!("{base}\n[models.writer]\n{fragment}\n");
            // a second [models.writer] is a duplicate table when the base has one; give it a name of its own
            let text = if base.contains("[models.writer]") { text.replace("\n[models.writer]\ndeep", "\n[models.deepwriter]\ndeep") } else { text };
            let name = member_name.replace("models.writer", if base.contains("[models.writer]") { "models.deepwriter" } else { "models.writer" });
            let o = run_bytes(&w, text.as_bytes());
            let label = format!("{what}: {raw}");
            assert_before_anything(&w, &o, &label);
            let err = stderr(&o);
            assert!(err.contains(&name), "{label}: {name} not named: {err}");
            assert!(!err.contains(raw), "{label}: the value is on the output: {err}");
        }
    }
}

#[test]
fn a_member_that_is_the_secret_is_named_and_a_value_that_is_the_secret_is_not() {
    // a name is how a member is named, as on main; the value is never printed by the reader
    let w = World::new(&tag("val"));
    let base = base_text(&w);
    for (what, table, key) in [
        ("an integer member", None, "max_llm_calls"),
        ("a decider member", Some("decider"), "max_decider_calls"),
        ("a model member", Some("models.writer"), "max_turns"),
        ("a cap", None, "cap_micro_usd"),
    ] {
        for raw in [format!("\"{S}\""), format!("[\"{S}\"]"), format!("{{ k = \"{S}\" }}"), "true".to_owned(), "\"\"".to_owned()] {
            let o = run_bytes(&w, member(&base, table, key, &raw).as_bytes());
            assert_ne!(code(&o), 0, "{what}: {raw}: {}", all_output(&o));
            let all = all_output(&o);
            assert!(!all.contains(S), "{what}: {raw}: the value is on the output: {all}");
        }
    }
}

#[test]
fn a_file_that_is_valid_toml_and_starts_like_json_is_read_as_toml() {
    let w = World::new(&tag("like"));
    let base = base_text(&w);
    // a quoted table header is a JSON array only when it is the whole file; with members it is TOML
    let text = base.replacen("[decider]", "[\"decider\"]", 1);
    assert!(text.contains("[\"decider\"]"));
    let o = run_bytes(&w, text.as_bytes());
    assert!(!stderr(&o).contains("refused:") && !stderr(&o).contains("--settings"), "a quoted header with members is refused: {}", all_output(&o));
    let _ = std::fs::remove_dir_all(w.run_dir("r1"));
    // a file that is nothing but a quoted header is valid JSON as well as valid TOML; it sets nothing
    for text in ["[\"decider\"]\n", "[1]\n", "[true]", "[null]\n", "[\"a\", \"b\"]"] {
        let w = World::new(&tag("like"));
        let o = run_bytes(&w, text.as_bytes());
        assert_before_anything(&w, &o, text);
        assert!(!all_output(&o).contains("panicked"), "{text}: {}", all_output(&o));
    }
}

#[test]
fn a_toml_file_that_has_a_byte_order_mark_is_read_or_refused_and_never_panics() {
    let w = World::new(&tag("bom"));
    let o = run_bytes(&w, format!("\u{feff}{}", base_text(&w)).as_bytes());
    assert!(!all_output(&o).contains("panicked"), "{}", all_output(&o));
    let all = all_output(&o);
    eprintln!("PROBE bom: exit {} {}", code(&o), all.chars().take(200).collect::<String>());
}

#[test]
fn a_whole_file_of_one_giant_line_is_refused_in_bounded_text_and_time() {
    let started = std::time::Instant::now();
    let rows = [
        ("a key of 8 MiB", format!("{} = 1\n", "k".repeat(8 << 20))),
        ("a quoted key of 8 MiB with a bad value", format!("\"{}\" = oops\n", "k".repeat(8 << 20))),
        ("a table name of 8 MiB, unclosed", format!("[{}\n", "t".repeat(8 << 20))),
        ("a value of 8 MiB, bare", format!("a = {}\n", "v".repeat(8 << 20))),
        ("an array of a million items, unclosed", format!("a = [{}\n", "1,".repeat(1 << 20))),
    ];
    in_parallel(rows.len(), |i| giant_row(rows[i].0, &rows[i].1));
    assert!(started.elapsed().as_secs() < 30, "{:?}", started.elapsed());
}

/// One row of the giant-line case, in a `World` of its own: a row costs about 2 s (8 MiB written, read and parsed).
fn giant_row(what: &str, text: &str) {
    let w = World::new(&tag("giant"));
    let o = run_bytes(&w, text.as_bytes());
    assert_before_anything(&w, &o, what);
    assert!(stderr(&o).len() < 1024 || what.contains("key of 8 MiB"), "{what}: {} bytes", stderr(&o).len());
    let all = all_output(&o);
    assert!(!all.contains("kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk") || what == "a key of 8 MiB", "{what}: the key is on the output");
    assert!(!all.contains("vvvvvvvvvvvvvvvvvvvvvvvv") && !all.contains("tttttttttttttttttttttttt"), "{what}: text of the file is on the output");
}

/// The four places of `normalise` that print the file's value, and the member names. A probe, not a
/// check: it prints what a pasted login reaches (`cargo test ... -- --ignored --nocapture`).
#[test]
#[ignore = "a probe that asserts nothing: it prints what a pasted login reaches on stderr, run with --ignored --nocapture"]
fn what_normalise_prints_of_a_pasted_login_in_each_of_its_echoes() {
    let w = World::new(&tag("probe"));
    let base = base_text(&w);
    let big = format!("{S}{}", "x".repeat(1 << 20));
    for (what, table, key, raw) in [
        ("run", None, "run", format!("\"{big}\"")),
        ("format", None, "format", format!("\"{big}\"")),
        ("confinement", None, "confinement", format!("\"{big}\"")),
        ("decider.principal", Some("decider"), "principal", format!("\"{big}\"")),
        ("run, a table", None, "run", format!("{{ k = \"{S}\" }}")),
        ("run, an array", None, "run", format!("[ \"{S}\" ]")),
        ("unknown key as the secret", None, S, "1".to_owned()),
        ("unknown key in decider", Some("decider"), S, "1".to_owned()),
        ("unknown key 1 MiB", None, &"k".repeat(1 << 20), "1".to_owned()),
    ] {
        let o = run_bytes(&w, member(&base, table, key, &raw).as_bytes());
        let err = stderr(&o);
        eprintln!("PROBE {what}: exit {} stderr {} bytes, secret on it: {}, first 120: {}", code(&o), err.len(), err.contains(S), err.chars().take(120).collect::<String>());
        let _ = std::fs::remove_dir_all(w.run_dir("r1"));
    }
}
