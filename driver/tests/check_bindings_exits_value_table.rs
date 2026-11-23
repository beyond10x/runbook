//! Every member that `scripts/check-bindings.sh` reads from a bindings file, with every JSON type in it.
//!
//! One table (`MEMBERS`): the member, the type the script needs of it, and what the script does with a value
//! of each type. Every cell of the table runs the script twice over a root of its own: `--exits <root>` and
//! the full check cut before the cargo steps (the same script, so the loop of the protocols, the exits step
//! and the registrations step read the file). Each cell ends in exit 0 or exit 1, never another code; an exit
//! 1 prints a `FAIL` line that names the file (and the action, where the member lies in one); neither stream
//! has a line of jq or yq that is no finding.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const FILE: &str = "bindings/example.yaml";

const GENERIC: &[&str] = &[
    "null",
    "false",
    "true",
    "7",
    r#""x""#,
    r#""""#,
    "[]",
    "[1]",
    r#"["a"]"#,
    "{}",
    r#"{"a":1}"#,
];
const TEXT: &[&str] = &[
    r#""x\n""#,
    r#""+1""#,
    r#""01""#,
    "99999999999999999999999999",
];
const KEYS: &[&str] = &[
    r#""0""#,
    r#""1""#,
    r#""255""#,
    r#""*""#,
    r#""256""#,
    r#""x""#,
    r#""""#,
    r#""x\n""#,
    r#""1\n""#,
    r#""+1""#,
    r#""01""#,
    r#""-1""#,
    r#""1.5""#,
    r#"" 1""#,
    r#""99999999999999999999999999""#,
    r#""٣""#,
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Exits,
    Full,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Expect {
    Refuse,
    Pass,
    Either,
}
use Expect::{Either, Pass, Refuse};

struct Member {
    name: &'static str,
    build: fn(&str) -> String,
    values: &'static [&'static str],
    text: bool,
    extra: &'static [&'static str],
    names_action: bool,
    expect: fn(&str, Mode) -> Expect,
}

const BASE: &[(&str, &str)] = &[
    ("action", r#""a.one""#),
    ("executor", r#"{"kind":"program","argv":["tool","y"]}"#),
    ("exits", r#"{"error":[75,126,127]}"#),
    (
        "evidence",
        r#"[{"kind":"k","from":"exit","subject":"s","results":{"1":"a"}}]"#,
    ),
];

fn act(over: &[(&str, &str)]) -> String {
    let mut parts = Vec::new();
    for (k, v) in BASE {
        let v = over
            .iter()
            .find(|(o, _)| o == k)
            .map(|(_, v)| *v)
            .unwrap_or(v);
        if !v.is_empty() {
            parts.push(format!("\"{k}\":{v}"));
        }
    }
    for (k, v) in over {
        if !BASE.iter().any(|(b, _)| b == k) && !v.is_empty() {
            parts.push(format!("\"{k}\":{v}"));
        }
    }
    format!("{{{}}}", parts.join(","))
}

fn doc_with(protocol: &str, root: &str, actions: &str) -> String {
    format!(
        r#"{{"format":"loom.bindings/1","protocol":{protocol},"program_root":{root},"actions":{actions}}}"#
    )
}

fn doc(actions: &str) -> String {
    doc_with(r#""example@1""#, r#""bindings""#, actions)
}

fn one(a: &str) -> String {
    doc(&format!("[{a}]"))
}

fn is_null(v: &str) -> bool {
    v == "null"
}
fn is_map(v: &str) -> bool {
    v.starts_with('{')
}
fn is_str(v: &str) -> bool {
    v.starts_with('"')
}
fn strings_only(v: &str) -> bool {
    v == "[]" || v == r#"["a"]"#
}

const MEMBERS: &[Member] = &[
    Member {
        name: "the document",
        build: |v| v.to_string(),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: false,
        expect: |_, _| Refuse,
    },
    Member {
        name: "actions",
        build: |v| doc(v),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: false,
        expect: |v, _| if v == "[]" { Either } else { Refuse },
    },
    Member {
        name: "an entry of actions",
        build: |v| doc(&format!("[{v}]")),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: false,
        expect: |_, _| Refuse,
    },
    Member {
        name: "action",
        build: |v| one(&act(&[("action", v)])),
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: false,
        expect: |v, _| if is_str(v) { Either } else { Refuse },
    },
    Member {
        name: "executor",
        build: |v| one(&act(&[("executor", v)])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, _| if is_map(v) { Either } else { Refuse },
    },
    Member {
        name: "executor.kind",
        build: |v| {
            one(&act(&[(
                "executor",
                &format!(r#"{{"kind":{v},"argv":["tool","y"]}}"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "executor.argv",
        build: |v| {
            one(&act(&[(
                "executor",
                &format!(r#"{{"kind":"program","argv":{v}}}"#),
            )]))
        },
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if is_null(v) || strings_only(v) || (v == "[1]" && m == Mode::Exits) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "an element of executor.argv",
        build: |v| {
            one(&act(&[(
                "executor",
                &format!(r#"{{"kind":"program","argv":[{v},"y"]}}"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if m == Mode::Exits || is_str(v) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "the value after --instructions in executor.argv",
        build: |v| {
            one(&act(&[(
                "executor",
                &format!(r#"{{"kind":"program","argv":["tool","--instructions",{v}]}}"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, m| if m == Mode::Exits { Either } else { Refuse },
    },
    Member {
        name: "arguments_from",
        build: |v| one(&act(&[("arguments_from", v)])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if m == Mode::Exits || is_null(v) || strings_only(v) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "an element of arguments_from",
        build: |v| one(&act(&[("arguments_from", &format!("[{v}]"))])),
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if m == Mode::Exits || is_str(v) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "exits",
        build: |v| one(&act(&[("exits", v)])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |_, _| Refuse,
    },
    Member {
        name: "exits.error",
        build: |v| one(&act(&[("exits", &format!(r#"{{"error":{v}}}"#))])),
        values: GENERIC,
        text: false,
        extra: &["[75,126,127]"],
        names_action: true,
        expect: |v, _| if v == "[75,126,127]" { Pass } else { Refuse },
    },
    Member {
        name: "a code in exits.error",
        build: |v| one(&act(&[("exits", &format!(r#"{{"error":[{v},126,127]}}"#))])),
        values: GENERIC,
        text: true,
        extra: &["75"],
        names_action: true,
        expect: |v, _| if v == "75" { Pass } else { Refuse },
    },
    Member {
        name: "exits.not_called",
        build: |v| {
            one(&act(&[(
                "exits",
                &format!(r#"{{"error":[75,126,127],"not_called":{v}}}"#),
            )]))
        },
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, _| {
            if is_null(v) || v == "[]" {
                Pass
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "a code in exits.not_called of a model node",
        build: |v| {
            one(&act(&[
                ("executor", r#"{"kind":"program","argv":["sdc-node"]}"#),
                (
                    "exits",
                    &format!(r#"{{"error":[75,126,127],"not_called":[{v}]}}"#),
                ),
            ]))
        },
        values: GENERIC,
        text: true,
        extra: &["79"],
        names_action: true,
        expect: |v, _| if v == "79" { Pass } else { Refuse },
    },
    Member {
        name: "evidence",
        build: |v| one(&act(&[("evidence", v)])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if is_null(v) || v == "[]" {
                if m == Mode::Exits {
                    Pass
                } else {
                    Either
                }
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "an entry of evidence",
        build: |v| one(&act(&[("evidence", &format!("[{v}]"))])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, _| if is_map(v) { Either } else { Refuse },
    },
    Member {
        name: "evidence.kind",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":{v},"from":"exit","subject":"s","results":{{"1":"a"}}}}]"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "evidence.from",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":{v},"subject":"s","results":{{"1":"a"}}}}]"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "evidence.subject",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":"exit","subject":{v},"results":{{"1":"a"}}}}]"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "evidence.results",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":"exit","subject":"s","results":{v}}}]"#),
            )]))
        },
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, _| {
            if is_null(v) || is_map(v) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "a result key of an entry read from the exit",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":"exit","subject":"s","results":{{{v}:"a"}}}}]"#),
            )]))
        },
        values: KEYS,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, _| {
            if matches!(v, r#""0""# | r#""1""# | r#""255""# | r#""*""#) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "a result key of an entry not read from the exit",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":"effect","subject":"s","results":{{{v}:"a"}}}}]"#),
            )]))
        },
        values: KEYS,
        text: false,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "a result value",
        build: |v| {
            one(&act(&[(
                "evidence",
                &format!(r#"[{{"kind":"k","from":"exit","subject":"s","results":{{"1":{v}}}}}]"#),
            )]))
        },
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "retry_when",
        build: |v| one(&act(&[("retry_when", v)])),
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "handoff",
        build: |v| one(&act(&[("handoff", v)])),
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "budget",
        build: |v| one(&act(&[("budget", v)])),
        values: GENERIC,
        text: false,
        extra: &[],
        names_action: true,
        expect: |v, m| {
            if m == Mode::Exits || is_null(v) || is_map(v) {
                Either
            } else {
                Refuse
            }
        },
    },
    Member {
        name: "budget.max_runs",
        build: |v| one(&act(&[("budget", &format!(r#"{{"max_runs":{v}}}"#))])),
        values: GENERIC,
        text: true,
        extra: &[],
        names_action: true,
        expect: |_, _| Either,
    },
    Member {
        name: "protocol",
        build: |v| doc_with(v, r#""bindings""#, &format!("[{}]", act(&[]))),
        values: GENERIC,
        text: true,
        extra: &[r#""example@1""#],
        names_action: false,
        expect: |v, m| {
            if m == Mode::Full && v != r#""example@1""# {
                Refuse
            } else {
                Either
            }
        },
    },
    Member {
        name: "program_root",
        build: |v| doc_with(r#""example@1""#, v, &format!("[{}]", act(&[]))),
        values: GENERIC,
        text: true,
        extra: &[r#""bindings""#],
        names_action: false,
        expect: |v, m| {
            if m == Mode::Full && v != r#""bindings""# {
                Refuse
            } else {
                Either
            }
        },
    },
    Member {
        name: "executor.from of a copy executor",
        build: |v| {
            one(&act(&[
                ("executor", &format!(r#"{{"kind":"copy","from":{v}}}"#)),
                ("exits", ""),
            ]))
        },
        values: GENERIC,
        text: true,
        extra: &[r#""example@1""#],
        names_action: false,
        expect: |v, m| match m {
            Mode::Exits => Pass,
            Mode::Full if v == r#""example@1""# => Either,
            Mode::Full => Refuse,
        },
    },
];

const IR: &str = r#"{"actions":{"a.one":{"may_produce":[{"evidence":"k"}]}},"artifacts":{}}"#;
const JSON_BINDINGS: &str = r#"{"a.one":{"produces":"k","subject":"s","results":{"1":"a"}}}"#;
const PROTOCOL: &str = "protocol:\n  id: example\n  revision: 1\n";

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

fn work() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-value-table")
}

fn cut_script(work: &Path) -> PathBuf {
    let text = fs::read_to_string(script()).unwrap();
    let cut = text.split("# The reject and cap routes").next().unwrap();
    let path = work.join("check-bindings-cut.sh");
    fs::write(&path, format!("{cut}exit $fail\n")).unwrap();
    path
}

fn cell_root(work: &Path, id: &str, yaml: &str, cut: &Path) -> PathBuf {
    let root = work.join("cells").join(id);
    let _ = fs::remove_dir_all(&root);
    for d in [
        "bindings",
        "protocol/compiled",
        "driver/examples/sdc",
        "scripts",
    ] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    fs::write(root.join("bindings/example.yaml"), yaml).unwrap();
    fs::write(root.join("protocol/example.protocol.yaml"), PROTOCOL).unwrap();
    fs::write(root.join("protocol/compiled/example.ir.json"), IR).unwrap();
    fs::write(
        root.join("driver/examples/sdc/example.bindings.json"),
        JSON_BINDINGS,
    )
    .unwrap();
    std::os::unix::fs::symlink(cut, root.join("scripts/check-bindings.sh")).unwrap();
    root
}

fn run(mode: Mode, root: &Path) -> (Option<i32>, String, String) {
    let mut c = Command::new("bash");
    match mode {
        Mode::Exits => c.arg(script()).arg("--exits").arg(root),
        Mode::Full => c.arg(root.join("scripts/check-bindings.sh")),
    };
    let o = c.output().unwrap();
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn judge(m: &Member, value: &str, mode: Mode, got: &(Option<i32>, String, String)) -> Vec<String> {
    let (code, out, err) = got;
    let mut bad = Vec::new();
    if !matches!(code, Some(0) | Some(1)) {
        bad.push(format!("exit {code:?}, not 0 or 1"));
    }
    for l in out.lines().chain(err.lines()) {
        if l.starts_with("jq:") || l.starts_with("Error:") || l.contains("jq: error") {
            bad.push(format!("raw line: {l}"));
            break;
        }
    }
    if *code == Some(1) {
        if !out.lines().any(|l| l.starts_with("FAIL")) {
            bad.push("exit 1 without a FAIL line".into());
        }
        if !out.lines().any(|l| l.contains(FILE)) {
            bad.push("no line names the file".into());
        }
        if m.names_action && !out.lines().any(|l| l.contains(&format!("{FILE} a.one"))) {
            bad.push("no line names the file and the action".into());
        }
    }
    match (m.expect)(value, mode) {
        Refuse if *code != Some(1) => {
            bad.push(format!("accepted (exit {code:?}), want it refused"))
        }
        Pass if *code != Some(0) => bad.push(format!("exit {code:?}, want 0")),
        _ => {}
    }
    bad
}

fn values_of(m: &Member) -> Vec<&'static str> {
    let mut values: Vec<&'static str> = m.values.to_vec();
    if m.text {
        values.extend_from_slice(TEXT);
    }
    values.extend_from_slice(m.extra);
    values
}

fn runs_in_table() -> usize {
    MEMBERS.iter().map(|m| values_of(m).len() * 2).sum()
}

/// Every cell of one member of the table: its values, each in `--exits` and in the full check, each in a root of its own, on a few
/// threads. One test per member keeps each test short at any load.
fn check_member(name: &str) {
    let index = MEMBERS
        .iter()
        .position(|m| m.name == name)
        .expect("a member of the table");
    let m = &MEMBERS[index];
    let work = work().join(format!("member-{index}"));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).unwrap();
    let cut = cut_script(&work);
    let started = std::time::Instant::now();
    let values = values_of(m);
    let next = AtomicUsize::new(0);
    let red = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(value) = values.get(i) else { break };
                let root = cell_root(&work, &i.to_string(), &(m.build)(value), &cut);
                for mode in [Mode::Exits, Mode::Full] {
                    let got = run(mode, &root);
                    let bad = judge(m, value, mode, &got);
                    if !bad.is_empty() {
                        let first: Vec<&str> = got.1.lines().chain(got.2.lines()).take(3).collect();
                        red.lock().unwrap().push(format!(
                            "{} = {} [{mode:?}]: {} | {}",
                            m.name,
                            value,
                            bad.join("; "),
                            first.join(" / ")
                        ));
                    }
                }
            });
        }
    });
    println!(
        "{}: {} runs in {:.1?}",
        m.name,
        values.len() * 2,
        started.elapsed()
    );
    let mut red = red.into_inner().unwrap();
    red.sort();
    assert!(
        red.is_empty(),
        "{} of {} runs of `{}` are red:\n{}",
        red.len(),
        values.len() * 2,
        m.name,
        red.join("\n")
    );
}

macro_rules! member_tests {
    ($($test:ident: $name:literal,)*) => {
        const TESTED: &[&str] = &[$($name),*];
        $(
            #[test]
            fn $test() {
                check_member($name);
            }
        )*
    };
}

member_tests! {
    the_document_takes_every_json_type: "the document",
    actions_takes_every_json_type: "actions",
    an_entry_of_actions_takes_every_json_type: "an entry of actions",
    action_takes_every_json_type: "action",
    executor_takes_every_json_type: "executor",
    executor_kind_takes_every_json_type: "executor.kind",
    executor_argv_takes_every_json_type: "executor.argv",
    an_element_of_executor_argv_takes_every_json_type: "an element of executor.argv",
    the_value_after_instructions_takes_every_json_type: "the value after --instructions in executor.argv",
    arguments_from_takes_every_json_type: "arguments_from",
    an_element_of_arguments_from_takes_every_json_type: "an element of arguments_from",
    exits_takes_every_json_type: "exits",
    exits_error_takes_every_json_type: "exits.error",
    a_code_in_exits_error_takes_every_json_type: "a code in exits.error",
    exits_not_called_takes_every_json_type: "exits.not_called",
    a_code_in_exits_not_called_takes_every_json_type: "a code in exits.not_called of a model node",
    evidence_takes_every_json_type: "evidence",
    an_entry_of_evidence_takes_every_json_type: "an entry of evidence",
    evidence_kind_takes_every_json_type: "evidence.kind",
    evidence_from_takes_every_json_type: "evidence.from",
    evidence_subject_takes_every_json_type: "evidence.subject",
    evidence_results_takes_every_json_type: "evidence.results",
    a_result_key_read_from_the_exit_takes_every_spelling: "a result key of an entry read from the exit",
    a_result_key_not_read_from_the_exit_takes_every_spelling: "a result key of an entry not read from the exit",
    a_result_value_takes_every_json_type: "a result value",
    retry_when_takes_every_json_type: "retry_when",
    handoff_takes_every_json_type: "handoff",
    budget_takes_every_json_type: "budget",
    budget_max_runs_takes_every_json_type: "budget.max_runs",
    protocol_takes_every_json_type: "protocol",
    program_root_takes_every_json_type: "program_root",
    the_from_of_a_copy_executor_takes_every_json_type: "executor.from of a copy executor",
}

/// The split loses no cell: the tests above are the members of the table, in its order, and the table holds 436 cells, 872 runs.
#[test]
fn the_tests_cover_every_member_and_every_cell_of_the_table() {
    let names: Vec<&str> = MEMBERS.iter().map(|m| m.name).collect();
    assert_eq!(
        names, TESTED,
        "a member of the table has no test, or a test names none"
    );
    assert_eq!(names.len(), 32);
    let cells: usize = MEMBERS.iter().map(|m| values_of(m).len()).sum();
    println!(
        "{} members, {cells} cells, {} runs",
        names.len(),
        runs_in_table()
    );
    assert_eq!((cells, runs_in_table()), (436, 872));
}

#[test]
fn the_base_cell_is_clean_in_both_modes() {
    let work = work().join("base");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).unwrap();
    let cut = cut_script(&work);
    let base = cell_root(&work, "base", &one(&act(&[])), &cut);
    for mode in [Mode::Exits, Mode::Full] {
        let (code, out, err) = run(mode, &base);
        assert!(
            code == Some(0) && out.contains("pass exits"),
            "the base cell is not clean for {mode:?}: {out}\n{err}"
        );
    }
}
