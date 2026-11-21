//! End-to-end tests of `sdc-goals`: the real binary on fixture files (tests/fixtures) and on
//! documents written per test under the crate's target tmp dir.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc-goals"))
        .args(args)
        .output()
        .unwrap();
    Out {
        code: o.status.code().unwrap(),
        stdout: String::from_utf8(o.stdout).unwrap(),
        stderr: String::from_utf8(o.stderr).unwrap(),
    }
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Writes a document with the given text and returns its path.
fn write(text: &str) -> String {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("goals-tests");
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!(
        "{}-{}.yaml",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

fn doc_in(layer: &str, entries: &str) -> String {
    write(&format!(
        "format: sdc.goals/1\nlayer: {layer}\nentries:\n{entries}\n"
    ))
}

fn doc(entries: &str) -> String {
    doc_in("company", entries)
}

const OK: &str = r#"kind: principle, priority: 1, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21"#;

fn has(o: &Out, file: &str, id: &str, code: &str) {
    assert!(
        o.stderr.contains(&format!("{file}:{id}: {code}: ")),
        "wanted {file}:{id}: {code}, got:\n{}",
        o.stderr
    );
}

/// One company document with one defective entry: exit 1 and exactly one finding line.
fn one_finding(entry: &str, id: &str, code: &str) {
    let f = doc(&format!("  - {{{entry}}}"));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1, "stderr: {}", o.stderr);
    assert_eq!(o.stderr.lines().count(), 1, "stderr: {}", o.stderr);
    has(&o, &f, id, code);
}

#[test]
fn trial_2_example_layers_are_valid() {
    let o = run(&[
        "check",
        &fixture("company.good.yaml"),
        &fixture("product.good.yaml"),
    ]);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

#[test]
fn four_bad_fixtures_each_name_their_entry() {
    let c = fixture("company.good.yaml");
    let cases = [
        (
            vec![fixture("bad.duplicate-id.yaml")],
            "company/c1",
            "duplicate_id",
        ),
        (
            vec![fixture("bad.unknown-kind.yaml")],
            "company/c1",
            "unknown_kind",
        ),
        (
            vec![fixture("bad.missing-source.yaml")],
            "company/c1",
            "missing_source",
        ),
        (
            vec![c, fixture("bad.override-constraint.yaml")],
            "product/p1",
            "override_constraint",
        ),
    ];
    for (files, id, code) in cases {
        let mut args = vec!["check".to_string()];
        args.extend(files.iter().cloned());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = run(&args);
        assert_eq!(o.code, 1, "{code}: {}", o.stderr);
        has(&o, files.last().unwrap(), id, code);
    }
}

#[test]
fn lower_layer_may_override_a_principle() {
    let o = run(&[
        "check",
        &fixture("company.good.yaml"),
        &fixture("product.override-principle.yaml"),
    ]);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
}

#[test]
fn override_of_a_goal_is_refused() {
    let c = doc(
        r#"  - {id: company/c1, kind: goal, priority: 1, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21}"#,
    );
    let p = doc_in(
        "product",
        &format!("  - {{id: product/p1, {OK}, overrides: company/c1}}"),
    );
    let o = run(&["check", &c, &p]);
    assert_eq!(o.code, 1);
    has(&o, &p, "product/p1", "override_kind");
}

#[test]
fn override_must_name_a_higher_layer() {
    let c = doc(&format!("  - {{id: company/c1, {OK}}}"));
    let p = doc_in(
        "product",
        &format!("  - {{id: product/p1, {OK}, overrides: company/c1}}"),
    );
    let c2 = doc(&format!(
        "  - {{id: company/c2, {OK}, overrides: product/p1}}\n  - {{id: company/c3, {OK}, overrides: company/c1}}"
    ));
    let o = run(&["check", &c, &p, &c2]);
    assert_eq!(o.code, 1);
    has(&o, &c2, "company/c2", "override_layer");
    has(&o, &c2, "company/c3", "override_layer");
}

#[test]
fn override_target_must_exist_in_the_given_files() {
    let c = doc(&format!("  - {{id: company/c1, {OK}}}"));
    let p = doc_in(
        "product",
        &format!("  - {{id: product/p1, {OK}, overrides: company/c9}}"),
    );
    let o = run(&["check", &c, &p]);
    assert_eq!(o.code, 1);
    has(&o, &p, "product/p1", "override_target_missing");
    let o = run(&["check", &p]);
    assert_eq!(o.code, 1);
    has(&o, &p, "product/p1", "override_target_missing");
}

#[test]
fn duplicate_id_across_files() {
    let a = doc(&format!("  - {{id: company/c1, {OK}}}"));
    let b = doc(&format!("  - {{id: company/c1, {OK}}}"));
    let o = run(&["check", &a, &b]);
    assert_eq!(o.code, 1);
    has(&o, &b, "company/c1", "duplicate_id");
}

#[test]
fn layer_key_is_required_and_every_id_prefix_matches_it() {
    let f = write(&format!(
        "format: sdc.goals/1\nentries:\n  - {{id: company/c1, {OK}}}\n"
    ));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "layer");
    let f = write(&format!(
        "format: sdc.goals/1\nlayer: team\nentries:\n  - {{id: company/c1, {OK}}}\n"
    ));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "layer");
    let f = doc(&format!(
        "  - {{id: company/c1, {OK}}}\n  - {{id: product/p1, {OK}}}"
    ));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "product/p1", "layer_mismatch");
    assert_eq!(o.stderr.lines().count(), 1);
    let f = doc_in("product", &format!("  - {{id: company/c1, {OK}}}"));
    has(&run(&["check", &f]), &f, "company/c1", "layer_mismatch");
}

#[test]
fn id_needs_a_known_layer_prefix() {
    one_finding(&format!(r#"id: team/c1, {OK}"#), "team/c1", "unknown_layer");
    one_finding(&format!(r#"id: c1, {OK}"#), "c1", "bad_id");
    one_finding(&format!(r#"id: "company/", {OK}"#), "company/", "bad_id");
}

#[test]
fn entry_without_id() {
    let f = doc(&format!("  - {{{OK}}}"));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "missing_id");
}

#[test]
fn statement_is_not_empty() {
    one_finding(
        r#"id: company/c1, kind: goal, priority: 1, statement: "  ", sources: ["person:operator"], valid_from: 2026-11-21"#,
        "company/c1",
        "missing_statement",
    );
    one_finding(
        r#"id: company/c1, kind: goal, priority: 1, sources: ["person:operator"], valid_from: 2026-11-21"#,
        "company/c1",
        "missing_statement",
    );
}

#[test]
fn sources_is_a_non_empty_list() {
    let with = |s: &str| {
        format!(
            r#"id: company/c1, kind: goal, priority: 1, statement: "S.", {s} valid_from: 2026-11-21"#
        )
    };
    one_finding(&with(""), "company/c1", "missing_source");
    one_finding(&with("sources: [],"), "company/c1", "missing_source");
    one_finding(
        &with(r#"sources: "person:operator","#),
        "company/c1",
        "missing_source",
    );
    let f = doc(&format!(
        "  - {{{}}}",
        with(r#"source: "person:operator","#)
    ));
    let o = run(&["check", &f]);
    assert_eq!((o.code, o.stderr.lines().count()), (1, 1), "{}", o.stderr);
    has(&o, &f, "company/c1", "source_form");
    assert!(o.stderr.contains("use sources: [...]"), "{}", o.stderr);
}

#[test]
fn each_source_is_exactly_one_of_the_four_forms() {
    let base = |s: &str| {
        format!(
            r#"id: company/c1, kind: goal, priority: 1, statement: "S.", sources: [{s}], valid_from: 2026-11-21"#
        )
    };
    for good in [
        r#""person:operator""#,
        r#""aep:vision:O1@4""#,
        r#""canon/AGENTS.md:28""#,
        r#""decision:IDEA-1.q3""#,
        r#""person:operator", "canon/AGENTS.md:28""#,
    ] {
        let f = doc(&format!("  - {{{}}}", base(good)));
        let o = run(&["check", &f]);
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{good}");
    }
    for bad in [
        r#""canon/AGENTS.md:28 + surf/AGENTS.md:68""#,
        r#""canon/AGENTS.md:28 (the operator, 2026-11-20)""#,
        r#""canon/AGENTS.md:28; from company/c1""#,
        r#""aep:vision:O1@4 canon/x.md:3""#,
        r#""operator""#,
        r#""person:""#,
        r#""aep:vision:O1""#,
        r#""a.md:x""#,
        r#""a.md:0""#,
        r#""""#,
        "7",
    ] {
        one_finding(&base(bad), "company/c1", "source_form");
    }
}

#[test]
fn stated_is_by_and_a_date() {
    let with = |s: &str| format!("id: company/c1, {OK}, stated: {s}");
    let f = doc(&format!(
        "  - {{{}}}",
        with("{by: operator, at: 2026-11-20}")
    ));
    assert_eq!(run(&["check", &f]).code, 0);
    one_finding(
        &with("{by: operator, at: yesterday}"),
        "company/c1",
        "bad_stated",
    );
    one_finding(&with("{by: operator}"), "company/c1", "bad_stated");
    one_finding(&with("{at: 2026-11-20}"), "company/c1", "bad_stated");
    one_finding(
        &with(r#"{by: "", at: 2026-11-20}"#),
        "company/c1",
        "bad_stated",
    );
    one_finding(
        &with("{by: operator, at: 2026-11-20, why: x}"),
        "company/c1",
        "bad_stated",
    );
    one_finding(&with("operator"), "company/c1", "bad_stated");
}

#[test]
fn derived_from_names_entries_in_the_given_files() {
    let c = fixture("company.good.yaml");
    let p = doc_in(
        "product",
        &format!("  - {{id: product/p1, {OK}, derived_from: [company/c1, company/c2]}}"),
    );
    assert_eq!(run(&["check", &c, &p]).code, 0);
    let p = doc_in(
        "product",
        &format!("  - {{id: product/p1, {OK}, derived_from: [company/c1, company/c9]}}"),
    );
    let o = run(&["check", &c, &p]);
    assert_eq!(o.code, 1);
    has(&o, &p, "product/p1", "derived_unknown");
    assert_eq!(o.stderr.lines().count(), 1);
    let o = run(&["check", &p]);
    has(&o, &p, "product/p1", "derived_unknown");
    one_finding(
        &format!("id: company/c1, {OK}, derived_from: company/c2"),
        "company/c1",
        "bad_derived_from",
    );
    one_finding(
        &format!("id: company/c1, {OK}, derived_from: [7]"),
        "company/c1",
        "bad_derived_from",
    );
    one_finding(
        &format!("id: company/c1, {OK}, derived_from: []"),
        "company/c1",
        "bad_derived_from",
    );
}

#[test]
fn kind_is_required_and_known() {
    one_finding(
        r#"id: company/c1, priority: 1, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21"#,
        "company/c1",
        "unknown_kind",
    );
}

#[test]
fn priority_is_1_to_5_and_not_on_a_constraint() {
    let with = |kind: &str, prio: &str| {
        format!(
            r#"id: company/c1, kind: {kind}, {prio} statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21"#
        )
    };
    for p in [
        "priority: 0,",
        "priority: 6,",
        "priority: -1,",
        "priority: 1.5,",
        r#"priority: "high","#,
    ] {
        one_finding(&with("principle", p), "company/c1", "bad_priority");
    }
    one_finding(&with("principle", ""), "company/c1", "missing_priority");
    one_finding(&with("goal", ""), "company/c1", "missing_priority");
    one_finding(&with("preference", ""), "company/c1", "missing_priority");
    one_finding(
        &with("constraint", "priority: 1,"),
        "company/c1",
        "priority_on_constraint",
    );
    for p in 1..=5 {
        let f = doc(&format!(
            "  - {{{}}}",
            with("preference", &format!("priority: {p},"))
        ));
        assert_eq!(run(&["check", &f]).code, 0, "priority {p}");
    }
    let f = doc(&format!("  - {{{}}}", with("constraint", "")));
    assert_eq!(run(&["check", &f]).code, 0);
}

#[test]
fn validity_dates_are_well_formed_and_ordered() {
    let with = |dates: &str| {
        format!(
            r#"id: company/c1, kind: goal, priority: 1, statement: "S.", sources: ["person:operator"], {dates}"#
        )
    };
    one_finding(&with(""), "company/c1", "missing_valid_from");
    one_finding(&with("valid_from: 2026-13-01"), "company/c1", "bad_date");
    one_finding(&with("valid_from: 2026-02-30"), "company/c1", "bad_date");
    one_finding(&with("valid_from: 21.11.2026"), "company/c1", "bad_date");
    one_finding(
        &with("valid_from: 2026-11-21, valid_until: soon"),
        "company/c1",
        "bad_date",
    );
    one_finding(
        &with("valid_from: 2026-11-21T25:00:00Z"),
        "company/c1",
        "bad_date",
    );
    one_finding(
        &with("valid_from: 2026-11-21, valid_until: 2026-11-20"),
        "company/c1",
        "window_order",
    );
    one_finding(
        &with("valid_from: 2026-11-21, valid_until: 2026-11-21"),
        "company/c1",
        "window_order",
    );
    one_finding(
        &with("valid_from: 2026-11-21T10:00:00Z, valid_until: 2026-11-21"),
        "company/c1",
        "window_order",
    );
    let ok = doc(&format!(
        "  - {{{}}}",
        with("valid_from: 2026-11-21, valid_until: 2026-11-21T00:00:01Z")
    ));
    assert_eq!(run(&["check", &ok]).code, 0);
}

#[test]
fn rule_belongs_to_a_constraint() {
    one_finding(
        &format!(r#"id: company/c1, {OK}, rule: {{a: b}}"#),
        "company/c1",
        "rule_not_constraint",
    );
    let ok = doc(
        r#"  - {id: company/c1, kind: constraint, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21, rule: {a: b}}"#,
    );
    assert_eq!(run(&["check", &ok]).code, 0);
}

#[test]
fn unknown_fields_and_format_marker() {
    one_finding(
        &format!(r#"id: company/c1, {OK}, colour: red"#),
        "company/c1",
        "unknown_field",
    );
    let f = write(&format!(
        "format: sdc.goals/2\nlayer: company\nentries:\n  - {{id: company/c1, {OK}}}\n"
    ));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "format");
    let f = write(&format!(
        "format: sdc.goals/1\nlayer: company\nowner: x\nentries:\n  - {{id: company/c1, {OK}}}\n"
    ));
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "unknown_field");
    let f = write("layer: company\nentries: []\n");
    has(&run(&["check", &f]), &f, "-", "format");
}

#[test]
fn unparsable_document_is_a_finding() {
    let f = write("format: [unclosed\n");
    let o = run(&["check", &f]);
    assert_eq!(o.code, 1);
    has(&o, &f, "-", "parse");
}

#[test]
fn unreadable_file_and_usage_exit_2() {
    let o = run(&["check", "/nonexistent/goals.yaml"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("/nonexistent/goals.yaml"));
    assert_eq!(run(&["check"]).code, 2);
    assert_eq!(
        run(&["check", "--format", "xml", &fixture("company.good.yaml")]).code,
        2
    );
    assert_eq!(run(&[]).code, 2);
    assert_eq!(
        run(&["digest", "/nonexistent/goals.yaml", "company/c1"]).code,
        2
    );
    assert_eq!(
        run(&["resolve", "--at", "2026-11-21", "/nonexistent/goals.yaml"]).code,
        2
    );
}

fn sha256(s: &str) -> String {
    let h = Sha256::digest(s.as_bytes());
    format!(
        "sha256:{}",
        h.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

const C2_CANONICAL: &str = r#"{"id":"company/c2","kind":"constraint","sources":["person:operator"],"statement":"Data stays on devices and servers we run; no third-party cloud, account or mail service.","valid_from":"2026-11-21"}"#;

#[test]
fn digest_is_sha256_of_the_canonical_entry() {
    let o = run(&["digest", &fixture("company.good.yaml"), "company/c2"]);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    assert_eq!(o.stdout, format!("{}\n", sha256(C2_CANONICAL)));
}

#[test]
fn digest_ignores_key_order_and_layout_and_follows_content() {
    let a = doc(
        r#"  - {id: company/c1, kind: goal, priority: 1, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21, stated: {by: operator, at: 2026-11-20}}"#,
    );
    let b = write("format: sdc.goals/1\nlayer: company\nentries:\n  - valid_from: 2026-11-21\n    stated:\n      at: 2026-11-20\n      by: operator\n    sources:\n      - person:operator\n    statement: S.\n    priority: 1\n    kind: goal\n    id: company/c1\n");
    let c = doc(
        r#"  - {id: company/c1, kind: goal, priority: 1, statement: "S!", sources: ["person:operator"], valid_from: 2026-11-21, stated: {by: operator, at: 2026-11-20}}"#,
    );
    let d = |f: &str| run(&["digest", f, "company/c1"]).stdout;
    assert_eq!(d(&a), d(&b));
    assert_ne!(d(&a), d(&c));
    let canonical = r#"{"id":"company/c1","kind":"goal","priority":1,"sources":["person:operator"],"stated":{"at":"2026-11-20","by":"operator"},"statement":"S.","valid_from":"2026-11-21"}"#;
    assert_eq!(d(&a), format!("{}\n", sha256(canonical)));
}

#[test]
fn digest_of_an_unknown_id_exits_1() {
    let o = run(&["digest", &fixture("company.good.yaml"), "company/c9"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("company/c9"));
}

#[test]
fn json_report_lists_entries_digests_and_findings() {
    let o = run(&[
        "check",
        "--format",
        "json",
        &fixture("company.good.yaml"),
        &fixture("product.good.yaml"),
    ]);
    assert_eq!(o.code, 0);
    assert_eq!(o.stderr, "");
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["valid"], true);
    assert_eq!(v["findings"].as_array().unwrap().len(), 0);
    let e = v["entries"].as_array().unwrap();
    assert_eq!(e.len(), 5);
    assert_eq!(e[0]["id"], "company/c1");
    assert_eq!(e[0]["layer"], "company");
    assert_eq!(e[0]["kind"], "principle");
    assert_eq!(e[0]["priority"], 1);
    assert_eq!(e[0]["sources"][0], "person:operator");
    assert_eq!(e[1]["priority"], Value::Null);
    assert_eq!(e[1]["digest"], sha256(C2_CANONICAL));
    assert_eq!(e[4]["layer"], "product");
    let cli = run(&["digest", &fixture("product.good.yaml"), "product/p2"]);
    assert_eq!(
        format!("{}\n", e[4]["digest"].as_str().unwrap()),
        cli.stdout
    );

    let bad = fixture("bad.missing-source.yaml");
    let o = run(&["check", "--format", "json", &bad]);
    assert_eq!(o.code, 1);
    has(&o, &bad, "company/c1", "missing_source");
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["valid"], false);
    let f = &v["findings"][0];
    assert_eq!(
        (
            f["file"].as_str().unwrap(),
            f["id"].as_str().unwrap(),
            f["code"].as_str().unwrap()
        ),
        (bad.as_str(), "company/c1", "missing_source")
    );
    assert!(!f["message"].as_str().unwrap().is_empty());
}

#[test]
fn output_is_the_same_on_two_runs() {
    let args = [
        "check",
        "--format",
        "json",
        "tests/fixtures/bad.duplicate-id.yaml",
        "tests/fixtures/bad.unknown-kind.yaml",
    ];
    let run_in_crate = || {
        let o = Command::new(env!("CARGO_BIN_EXE_sdc-goals"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(args)
            .output()
            .unwrap();
        (o.stdout, o.stderr)
    };
    assert_eq!(run_in_crate(), run_in_crate());
}

fn resolved(args: &[&str]) -> (Out, Vec<Value>) {
    let o = run(args);
    assert_eq!(o.code, 0, "stderr: {}", o.stderr);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    let e = v["entries"].as_array().unwrap().clone();
    (o, e)
}

fn ids(e: &[Value]) -> Vec<&str> {
    e.iter().map(|x| x["id"].as_str().unwrap()).collect()
}

#[test]
fn resolve_prints_the_same_bytes_with_layer_sources_and_digest() {
    let args = [
        "resolve",
        "--at",
        "2026-11-21T00:00:00Z",
        &fixture("company.good.yaml"),
        &fixture("product.good.yaml"),
    ];
    let (a, e) = resolved(&args);
    let (b, _) = resolved(&args);
    assert_eq!(a.stdout, b.stdout);
    assert_eq!(
        ids(&e),
        [
            "company/c1",
            "company/c2",
            "company/c3",
            "product/p1",
            "product/p2"
        ]
    );
    assert_eq!(e[1]["layer"].as_str(), Some("company"));
    assert_eq!(e[1]["sources"][0].as_str(), Some("person:operator"));
    assert_eq!(e[1]["digest"].as_str(), Some(sha256(C2_CANONICAL).as_str()));
    assert_eq!(e[3]["layer"].as_str(), Some("product"));
    let v: Value = serde_json::from_str(&a.stdout).unwrap();
    assert_eq!(v["format"].as_str(), Some("sdc.goals/1"));
    assert_eq!(v["at"].as_str(), Some("2026-11-21T00:00:00Z"));
    // the layer files may come in any order; the output does not
    let (c, _) = resolved(&[
        "resolve",
        "--at",
        "2026-11-21T00:00:00Z",
        &fixture("product.good.yaml"),
        &fixture("company.good.yaml"),
    ]);
    assert_eq!(a.stdout, c.stdout);
}

#[test]
fn resolve_leaves_out_entries_outside_their_window() {
    let f = doc(
        r#"  - {id: company/c1, kind: goal, priority: 1, statement: "Kept.", sources: ["person:operator"], valid_from: 2026-11-21}
  - {id: company/c2, kind: goal, priority: 1, statement: "Expired.", sources: ["person:operator"], valid_from: 2026-01-01, valid_until: 2026-11-20}
  - {id: company/c3, kind: goal, priority: 1, statement: "Ends at the instant.", sources: ["person:operator"], valid_from: 2026-01-01, valid_until: 2026-11-21T00:00:00Z}
  - {id: company/c4, kind: goal, priority: 1, statement: "Not yet.", sources: ["person:operator"], valid_from: 2026-11-22}
  - {id: company/c5, kind: goal, priority: 1, statement: "Still open.", sources: ["person:operator"], valid_from: 2026-01-01, valid_until: 2026-11-21T00:00:01Z}"#,
    );
    let (_, e) = resolved(&["resolve", "--at", "2026-11-21T00:00:00Z", &f]);
    assert_eq!(ids(&e), ["company/c1", "company/c5"]);
    let (_, e) = resolved(&["resolve", "--at", "2026-11-21", &f]);
    assert_eq!(ids(&e), ["company/c1", "company/c5"]);
}

#[test]
fn resolve_replaces_an_overridden_entry_while_the_override_applies() {
    let c = fixture("company.good.yaml");
    let p = fixture("product.override-principle.yaml");
    let (_, e) = resolved(&["resolve", "--at", "2026-11-21T00:00:00Z", &c, &p]);
    assert_eq!(ids(&e), ["company/c1", "company/c2", "product/p3"]);
    let (_, e) = resolved(&["resolve", "--at", "2027-01-01T00:00:00Z", &c, &p]);
    assert_eq!(ids(&e), ["company/c1", "company/c2", "company/c3"]);
    let (_, e) = resolved(&["resolve", "--at", "2026-11-20T00:00:00Z", &c, &p]);
    assert!(e.is_empty());
}

#[test]
fn resolve_refuses_findings_and_a_bad_instant() {
    let o = run(&[
        "resolve",
        "--at",
        "2026-11-21",
        &fixture("bad.unknown-kind.yaml"),
    ]);
    assert_eq!((o.code, o.stdout.as_str()), (1, ""));
    has(
        &o,
        &fixture("bad.unknown-kind.yaml"),
        "company/c1",
        "unknown_kind",
    );
    assert_eq!(
        run(&[
            "resolve",
            "--at",
            "yesterday",
            &fixture("company.good.yaml")
        ])
        .code,
        2
    );
    assert_eq!(run(&["resolve", &fixture("company.good.yaml")]).code, 2);
}

/// `ground` on the given layer files, citing `goals@r1#<id>` with the current revision `current`.
fn ground(id: &str, digest: Option<&str>, current: &str, at: &str, files: &[String]) -> Out {
    let cite = format!("goals@r1#{id}");
    let mut args = vec!["ground", "--at", at, "--cite", &cite, "--current", current];
    if let Some(d) = digest {
        args.extend(["--digest", d]);
    }
    args.extend(files.iter().map(String::as_str));
    run(&args)
}

fn p1_digest() -> String {
    run(&["digest", &fixture("product.good.yaml"), "product/p1"])
        .stdout
        .trim()
        .to_string()
}

fn layers(product: &str) -> Vec<String> {
    vec![fixture("company.good.yaml"), fixture(product)]
}

/// The refusal codes of a `ground` report, in order.
fn refusals(o: &Out) -> Vec<String> {
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    v["refusals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["code"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn ground_holds_on_the_current_revision_and_digest() {
    let d = p1_digest();
    let o = ground(
        "product/p1",
        Some(&d),
        "r1",
        "2026-11-21",
        &layers("product.good.yaml"),
    );
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["holds"], true);
    assert_eq!(v["cite"], "goals@r1#product/p1");
    assert_eq!(v["entry"], "product/p1");
    assert_eq!(v["layer"], "product");
    assert_eq!(v["digest"].as_str(), Some(d.as_str()));
    assert_eq!(v["at"], "2026-11-21T00:00:00Z");
    assert!(v["refusals"].as_array().unwrap().is_empty());
}

#[test]
fn ground_without_a_digest_holds_and_prints_the_digest() {
    let o = ground(
        "product/p1",
        None,
        "r1",
        "2026-11-21",
        &layers("product.good.yaml"),
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["digest"].as_str(), Some(p1_digest().as_str()));
}

#[test]
fn ground_is_refused_when_the_statement_changed() {
    let o = ground(
        "product/p1",
        Some(&p1_digest()),
        "r1",
        "2026-11-21",
        &layers("product.p1-changed.yaml"),
    );
    assert_eq!(o.code, 1);
    assert_eq!(refusals(&o), ["digest_changed"]);
    assert!(
        o.stderr.contains("goals@r1#product/p1: digest_changed: "),
        "{}",
        o.stderr
    );
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["holds"], false);
}

#[test]
fn ground_is_refused_when_the_entry_was_removed() {
    let o = ground(
        "product/p1",
        Some(&p1_digest()),
        "r1",
        "2026-11-21",
        &layers("product.p1-removed.yaml"),
    );
    assert_eq!(o.code, 1);
    assert_eq!(refusals(&o), ["no_entry"]);
    assert!(o.stderr.contains("goals@r1#product/p1: no_entry: "));
}

#[test]
fn ground_is_refused_at_a_revision_that_is_not_current() {
    let o = ground(
        "product/p1",
        Some(&p1_digest()),
        "r2",
        "2026-11-21",
        &layers("product.good.yaml"),
    );
    assert_eq!(o.code, 1);
    assert_eq!(refusals(&o), ["not_current"]);
    assert!(o.stderr.contains("goals@r1#product/p1: not_current: "));
    assert!(o.stderr.contains("r2"), "{}", o.stderr);
}

#[test]
fn ground_is_refused_outside_the_entry_window() {
    let f = doc(
        r#"  - {id: company/c1, kind: goal, priority: 1, statement: "S.", sources: ["person:operator"], valid_from: 2026-11-21, valid_until: 2026-12-01}"#,
    );
    let at = |t: &str| ground("company/c1", None, "r1", t, std::slice::from_ref(&f));
    assert_eq!(at("2026-11-21").code, 0);
    let o = at("2026-11-20T23:59:59Z");
    assert_eq!(
        (o.code, refusals(&o)),
        (1, vec!["not_yet_valid".to_string()])
    );
    let o = at("2026-12-01");
    assert_eq!((o.code, refusals(&o)), (1, vec!["expired".to_string()]));
}

#[test]
fn ground_is_refused_while_a_lower_layer_overrides_the_entry() {
    let files = layers("product.override-principle.yaml");
    let o = ground("company/c3", None, "r1", "2026-11-21", &files);
    assert_eq!((o.code, refusals(&o)), (1, vec!["overridden".to_string()]));
    assert!(o.stderr.contains("product/p3"), "{}", o.stderr);
    assert_eq!(
        ground("company/c3", None, "r1", "2027-01-01", &files).code,
        0
    );
}

#[test]
fn ground_names_every_cause_at_once() {
    let o = ground(
        "product/p1",
        Some(&p1_digest()),
        "r2",
        "2026-11-20",
        &layers("product.p1-changed.yaml"),
    );
    assert_eq!(o.code, 1);
    assert_eq!(
        refusals(&o),
        ["not_current", "digest_changed", "not_yet_valid"]
    );
}

#[test]
fn ground_is_refused_on_invalid_goals() {
    let o = ground(
        "company/c1",
        None,
        "r1",
        "2026-11-21",
        &[fixture("bad.unknown-kind.yaml")],
    );
    assert_eq!(o.code, 1);
    assert_eq!(refusals(&o), ["invalid_goals"]);
    assert!(o.stderr.contains("unknown_kind"), "{}", o.stderr);
}

#[test]
fn ground_usage_errors_exit_2() {
    let c = fixture("company.good.yaml");
    for cite in [
        "goals@r1",
        "goals#company/c1",
        "vision@r1#company/c1",
        "goals@#company/c1",
        "goals@r1#",
    ] {
        let o = run(&[
            "ground",
            "--at",
            "2026-11-21",
            "--cite",
            cite,
            "--current",
            "r1",
            &c,
        ]);
        assert_eq!(o.code, 2, "{cite}: {}", o.stderr);
    }
    let o = run(&[
        "ground",
        "--at",
        "2026-11-21",
        "--cite",
        "goals@r1#company/c1",
        &c,
    ]);
    assert_eq!(o.code, 2, "--current is required");
    let o = run(&[
        "ground",
        "--at",
        "2026-11-21",
        "--cite",
        "goals@r1#company/c1",
        "--current",
        "r1",
        "/nonexistent/goals.yaml",
    ]);
    assert_eq!(o.code, 2);
}
