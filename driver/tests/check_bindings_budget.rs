//! The `budget` check of `scripts/check-bindings.sh` (`--budget <root>` runs it alone over
//! `<root>/bindings/*.yaml`). Loom reads `runs_per`, `rework_limit` and `rework_on` in a `budget`
//! (`story:rework-allowance-after-a-rejection`) and refuses at load what the decoder and the
//! executor check refuse; this fails first, by the same words, as a named line and not as a raw `jq`
//! error.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-bindings.sh")
}

const RETRY: &str = "    retry_when:\n      code_review: [rejected]\n      conformance_run: [failed]\n";
const PROGRAM: &str = "{kind: program, argv: [tools/x.sh]}";

fn action(executor: &str, budget: &str, retry_when: &str) -> String {
    format!("  - action: a.write\n    executor: {executor}\n    budget: {budget}\n{retry_when}")
}

/// The check over a root whose one bindings file holds `body` after a good action.
fn check(name: &str, body: &str) -> Output {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("check-bindings-budget").join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bindings")).unwrap();
    let doc = format!("format: loom.bindings/1\nactions:\n  - action: good.action\n    executor: {PROGRAM}\n    budget: {{max_runs: 3}}\n{body}");
    fs::write(root.join("bindings/fixture.yaml"), doc).unwrap();
    Command::new("bash").arg(script()).arg("--budget").arg(&root).output().unwrap()
}

fn said(o: &Output) -> (String, String) {
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn a_budget_loom_reads_passes() {
    for (name, executor, budget, retry_when) in [
        ("max-runs", PROGRAM, "{max_runs: 3}", RETRY),
        ("case", PROGRAM, "{max_runs: 3, runs_per: case}", RETRY),
        ("rejection", PROGRAM, "{max_runs: 3, runs_per: rejection, rework_limit: 2}", RETRY),
        ("rework-on", PROGRAM, "{max_runs: 3, runs_per: rejection, rework_limit: 1, rework_on: {code_review: [rejected]}}", RETRY),
        ("person", "{kind: person, principal: person:owner}", "{runs_per: rejection}", "    retry_when:\n      merge_approval: [rejected]\n"),
        ("open", "{kind: open, name: c, bindings: c.yaml}", "{max_runs: 3}", ""),
    ] {
        let o = check(name, &action(executor, budget, retry_when));
        let (out, err) = said(&o);
        assert_eq!(o.status.code(), Some(0), "{name}: {out}\n{err}");
        assert!(out.contains("pass budget"), "{name}: {out}\n{err}");
    }
}

#[test]
fn each_budget_defect_loom_refuses_at_load_is_a_named_line() {
    for (name, executor, budget, retry_when, want) in [
        ("runs-per-window", PROGRAM, "{runs_per: window}", RETRY, "budget.runs_per is \"window\"; it is case or rejection"),
        ("runs-per-number", PROGRAM, "{runs_per: 3}", RETRY, "budget.runs_per is 3; it is case or rejection"),
        ("no-result", PROGRAM, "{runs_per: rejection}", "    retry_when: {code_review: []}\n", "budget.runs_per is rejection and retry_when lists no result, so no record could open an allowance"),
        ("no-retry-when", PROGRAM, "{runs_per: rejection}", "", "budget.runs_per is rejection and retry_when lists no result, so no record could open an allowance"),
        ("limit-zero", PROGRAM, "{runs_per: rejection, rework_limit: 0}", RETRY, "budget.rework_limit is 0; it is a whole number from 1"),
        ("limit-text", PROGRAM, "{runs_per: rejection, rework_limit: many}", RETRY, "budget.rework_limit is \"many\"; it is a whole number from 1"),
        ("limit-under-case", PROGRAM, "{runs_per: case, rework_limit: 2}", RETRY, "budget.rework_limit is read only under runs_per: rejection"),
        ("limit-alone", PROGRAM, "{rework_limit: 2}", RETRY, "budget.rework_limit is read only under runs_per: rejection"),
        ("on-under-case", PROGRAM, "{runs_per: case, rework_on: {code_review: [rejected]}}", RETRY, "budget.rework_on is read only under runs_per: rejection"),
        ("on-empty", PROGRAM, "{runs_per: rejection, rework_on: {}}", RETRY, "budget.rework_on is empty; it names the kinds and results that are rejections"),
        ("on-kind-empty", PROGRAM, "{runs_per: rejection, rework_on: {code_review: []}}", RETRY, "budget.rework_on.code_review is empty; it lists the results of the kind that are rejections"),
        ("on-not-an-object", PROGRAM, "{runs_per: rejection, rework_on: [code_review]}", RETRY, "budget.rework_on is a list; it is an object"),
        ("on-pair", PROGRAM, "{runs_per: rejection, rework_on: {code_review: [approved]}}", RETRY, "budget.rework_on lists code_review approved, which retry_when does not list; a rejection is a record that sends the action back"),
        ("on-kind", PROGRAM, "{runs_per: rejection, rework_on: {merge_approval: [rejected]}}", RETRY, "budget.rework_on lists merge_approval rejected, which retry_when does not list; a rejection is a record that sends the action back"),
        ("agent", "{kind: agent, adapter: claude-code, model: m}", "{runs_per: rejection}", RETRY, "budget.runs_per belongs to the Program and Person executors, not Agent"),
        ("open", "{kind: open, name: c, bindings: c.yaml}", "{runs_per: rejection}", RETRY, "budget.runs_per belongs to the Program and Person executors, not Open"),
        ("copy", "{kind: copy, evidence: e, from: p@1}", "{rework_limit: 2, runs_per: rejection}", RETRY, "budget.rework_limit belongs to the Program and Person executors, not Copy"),
    ] {
        let o = check(name, &action(executor, budget, retry_when));
        let (out, err) = said(&o);
        assert_eq!(o.status.code(), Some(1), "{name}: {out}\n{err}");
        assert!(out.contains("FAIL budget"), "{name}: {out}\n{err}");
        assert!(out.contains(&format!("bindings/fixture.yaml a.write: {want}")), "{name}: {out}\n{err}");
        assert!(!out.contains("good.action") && !err.contains("jq:"), "{name}: {out}\n{err}");
    }
}

/// What `rework_limit` is as Loom's decoder reads it (a YAML integer from 1 to `i64::MAX` and
/// nothing else: a float, an exponent form, a number past `i64::MAX`, a quoted number and `.inf`
/// are refused) and the value as Loom shows it. `runbook check` takes the same table
/// (`runbook/src/check.rs`).
const LIMITS: [(&str, Option<&str>); 22] = [
    ("2", None),
    ("+2", None),
    ("0x2", None),
    ("9223372036854775807", None),
    ("2.0", Some("2")),
    ("1e0", Some("1")),
    ("1e3", Some("1000")),
    ("9223372036854775808", Some("9223372036854775808")),
    ("99999999999999999999", Some("99999999999999999999")),
    (".inf", Some("inf")),
    ("-.inf", Some("-inf")),
    (".nan", Some("NaN")),
    ("2.5", Some("2.5")),
    ("0", Some("0")),
    ("-1", Some("-1")),
    ("007", Some("\"007\"")),
    ("\"2\"", Some("\"2\"")),
    ("null", Some("null")),
    ("true", Some("true")),
    ("[2]", Some("a list")),
    ("{a: 2}", Some("an object")),
    ("{\"$scalar\": {tag: \"!!int\", text: \"2\"}}", Some("an object")),
];

#[test]
fn a_rework_limit_is_a_whole_number_as_loom_reads_one_and_is_shown_as_loom_shows_it() {
    for (n, (value, shown)) in LIMITS.into_iter().enumerate() {
        let o = check(&format!("limit-{n}"), &action(PROGRAM, &format!("{{runs_per: rejection, rework_limit: {value}}}"), RETRY));
        let (out, err) = said(&o);
        match shown {
            None => {
                assert_eq!(o.status.code(), Some(0), "rework_limit {value}: {out}\n{err}");
                assert!(out.contains("pass budget"), "rework_limit {value}: {out}\n{err}");
            }
            Some(shown) => {
                assert_eq!(o.status.code(), Some(1), "rework_limit {value}: {out}\n{err}");
                assert!(out.contains("FAIL budget"), "rework_limit {value}: {out}\n{err}");
                assert!(out.contains(&format!("bindings/fixture.yaml a.write: budget.rework_limit is {shown}; it is a whole number from 1")), "rework_limit {value}: {out}\n{err}");
                assert!(!out.contains("good.action") && !err.contains("jq:") && !err.contains("yq:") && !err.contains("Error"), "rework_limit {value}: {out}\n{err}");
            }
        }
    }
}

fn refused(name: &str, value: &str) {
    let o = check(name, &action(PROGRAM, &format!("{{runs_per: rejection, rework_limit: {value}}}"), RETRY));
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "rework_limit {value}: {out}\n{err}");
    assert!(out.contains("FAIL budget") && out.contains("budget.rework_limit is"), "rework_limit {value}: {out}\n{err}");
    assert!(!err.contains("jq:") && !err.contains("yq:") && !err.contains("Error"), "rework_limit {value}: {out}\n{err}");
}

/// A mapping is an object to Loom whatever its keys are, and the script's own way of carrying a
/// scalar is no key a mapping of the file can borrow: refused by name, no raw `jq` error.
#[test]
fn a_mapping_is_an_object_whatever_keys_it_holds() {
    refused("marker-other", "{\"$scalar\": 1}");
    refused("marker-text", "{\"$scalar\": {tag: \"!!int\", text: \"two\"}}");
    refused("float-key", "{\"$float\": hi}");
    refused("map", "{a: 2}");
    refused("list", "[2]");
}

/// Loom's reader expands an alias, so `rework_limit: *n` after `max_runs: &n 3` is the integer 3 and
/// the budget is one Loom takes; an alias of a value Loom refuses is refused as that value.
#[test]
fn an_alias_of_a_whole_number_is_a_rework_limit_loom_takes() {
    let o = check("alias", &action(PROGRAM, "{runs_per: rejection, max_runs: &n 3, rework_limit: *n}", RETRY));
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert!(out.contains("pass budget"), "{out}\n{err}");
    let o = check("alias-float", &action(PROGRAM, "{runs_per: rejection, max_runs: &n 2.5, rework_limit: *n}", RETRY));
    let (out, err) = said(&o);
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    assert!(out.contains("budget.rework_limit is 2.5; it is a whole number from 1"), "{out}\n{err}");
}

/// Loom takes the core tag `!!int` and refuses any other tag; the script refuses the limit by name.
#[test]
fn a_rework_limit_with_a_tag_that_is_not_a_core_tag_is_refused() {
    refused("tag-foo", "!foo 2");
    let o = check("tag-int", &action(PROGRAM, "{runs_per: rejection, rework_limit: !!int 2}", RETRY));
    assert_eq!(o.status.code(), Some(0), "{}", said(&o).0);
}

/// Loom's reader takes a hex, octal and binary integer and bounds it at `i64::MAX`: one form each
/// side of each bound.
#[test]
fn a_hex_octal_or_binary_limit_is_taken_up_to_i64_max_and_refused_past_it() {
    let ones63 = format!("0b{}", "1".repeat(63));
    let one_then_zeros63 = format!("0b1{}", "0".repeat(63));
    let ones64 = format!("0b{}", "1".repeat(64));
    for (i, value) in ["0x7FFFFFFFFFFFFFFF", "0x000000000000000000002", "0xAB", "0o777777777777777777777", "0o2", "0b10", ones63.as_str()].into_iter().enumerate() {
        let o = check(&format!("take-{i}"), &action(PROGRAM, &format!("{{runs_per: rejection, rework_limit: {value}}}"), RETRY));
        let (out, err) = said(&o);
        assert_eq!(o.status.code(), Some(0), "rework_limit {value}: {out}\n{err}");
    }
    for (i, value) in ["0x8000000000000000", "0xFFFFFFFFFFFFFFFF", "0x", "0o1000000000000000000000", "0o1777777777777777777777", one_then_zeros63.as_str(), ones64.as_str()].into_iter().enumerate() {
        refused(&format!("refuse-{i}"), value);
    }
}
