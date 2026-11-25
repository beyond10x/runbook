//! `sdc-decision-request <case dir> <question id>` on the trial-2 intake fixture
//! (`story:decision-request-from-question`). The fixture's questions carry `excerpt`,
//! `consequences` and `default_reason` as `story:interview-question-context` defines them: the
//! recorded trial-2 questions predate those keys, so the fixture adds them by hand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use sha2::{Digest, Sha256};

use serde_json::{json, Value};

const BIN: &str = env!("CARGO_BIN_EXE_sdc-decision-request");
const CASE: &str =
    "tests/fixtures/decision-request/trial-2-train/store/IDEA-1/children/IDEA-1.intake";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("decision-request")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The tool's temporary file goes here, not to `/tmp`; shared by the tests, never wiped.
fn tmpdir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("decision-request-tmp");
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(case: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg(case)
        .args(args)
        .env("TMPDIR", tmpdir())
        .output()
        .unwrap()
}

fn fixture_case() -> PathBuf {
    manifest().join(CASE)
}

fn request_for(id: &str) -> (Vec<u8>, Value) {
    let out = run(&fixture_case(), &[id]);
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).expect("stdout is one JSON value");
    (out.stdout, value)
}

/// The 1-based line of `  - id: <id>` in the fixture's dissection and the line before the next item.
fn dissection_item_lines(id: &str) -> (u64, u64) {
    let text = fs::read_to_string(fixture_case().join("work/dissection.yaml")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == format!("- id: {id}"))
        .unwrap();
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with("- id: "))
        .map_or(lines.len(), |p| start + 1 + p);
    (start as u64 + 1, end as u64)
}

fn fixture_question(id: &str) -> Value {
    let text = fs::read_to_string(fixture_case().join("work/questions.yaml")).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
    let list = doc["questions"].as_sequence().unwrap();
    let q = list.iter().find(|q| q["id"].as_str() == Some(id)).unwrap();
    serde_json::to_value(q).unwrap()
}

#[test]
fn q1_request_carries_what_a_cold_reader_needs() {
    let (_, out) = request_for("q1");
    let q = fixture_question("q1");
    let req = &out["request"];

    assert_eq!(req["question"], q["question"]);
    assert_eq!(
        out["context"],
        "trial-2-train, A to-do list web app for me and my partner that also works on the train without a connection, intake"
    );

    let options = req["options"].as_array().unwrap();
    assert_eq!(options.len(), 2);
    for (i, opt) in options.iter().enumerate() {
        let c = &q["consequences"][i];
        assert_eq!(opt["letter"], ["A", "B"][i]);
        assert_eq!(opt["does"], c["does"], "option {i} does");
        assert_eq!(opt["cost"], c["cost"], "option {i} cost");
        assert_eq!(
            opt["consequence"]["answer"], q["options"][i],
            "option {i} answer"
        );
        assert_eq!(opt["consequence"]["produces"], "artifact_revision");
        assert!(opt["consequence"]["admits"].is_array());
        assert!(opt["consequence"]["blocks"].is_array());
    }

    // The recommendation names the default (option A of q1) and cites the dissection item d3.
    assert_eq!(req["recommended_option"], "A");
    assert_eq!(req["recommended_by"], "rule");
    assert_eq!(req["recommendation_strength"], "weak");
    assert_eq!(req["default_on_silence"], false);
    let grounds = req["grounds"].as_array().unwrap();
    assert_eq!(grounds.len(), 1);
    let (line, line_end) = dissection_item_lines("d3");
    let ground = &grounds[0];
    assert_eq!(ground["source"], "idea");
    assert_eq!(ground["supports"], "A");
    assert_eq!(ground["how"], q["default_reason"]);
    assert_eq!(ground["citation"]["form"], "path_line");
    assert_eq!(ground["citation"]["value"]["path"], "work/dissection.yaml");
    assert_eq!(ground["citation"]["value"]["line"], line);
    assert_eq!(ground["citation"]["value"]["line_end"], line_end);
    assert!(ground["citation"]["value"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(req["grounding_report"]
        .as_str()
        .unwrap()
        .contains("all resolve"));

    // What the person is shown first and what ends the request.
    let keys: Vec<&String> = out.as_object().unwrap().keys().collect();
    assert_eq!(keys.last().unwrap().as_str(), "what_happens_next");
    let next = out["what_happens_next"].as_str().unwrap();
    assert!(next.contains("scope.answer"), "{next}");
    assert!(next.contains("q1"), "{next}");
    assert_eq!(req["unblocks"]["action"], "scope.answer");
    assert_eq!(req["unblocks"]["question_id"], "q1");
    assert!(out["about"]
        .as_str()
        .unwrap()
        .contains("for me and my partner"));
}

#[test]
fn a_question_of_three_options_gets_three_letters() {
    let (_, out) = request_for("q3");
    let letters: Vec<&str> = out["request"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["letter"].as_str().unwrap())
        .collect();
    assert_eq!(letters, ["A", "B", "C"]);
    // q3's default is its second option.
    assert_eq!(out["request"]["recommended_option"], "B");
    assert_eq!(out["request"]["grounds"][0]["supports"], "B");
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    let (a, _) = request_for("q1");
    let (b, _) = request_for("q1");
    assert_eq!(a, b);
}

#[test]
fn an_item_outside_the_dissection_leaves_no_recommendation_and_says_why() {
    let (_, out) = request_for("q7");
    let req = &out["request"];
    assert!(req.get("recommended_option").is_none());
    assert_eq!(req["recommendation_strength"], "none");
    assert_eq!(req["grounds"], json!([]));
    assert!(req["no_recommendation_reason"]
        .as_str()
        .unwrap()
        .contains("f1"));
    assert_eq!(req["options"].as_array().unwrap().len(), 3);
}

#[test]
fn a_question_that_does_not_change_scope_is_refused() {
    let out = run(&fixture_case(), &["q2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not scope-changing"));
}

#[test]
fn an_unknown_question_is_refused() {
    let out = run(&fixture_case(), &["q99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("q99"));
}

#[test]
fn a_question_missing_its_consequences_is_refused_naming_them() {
    let dir = scratch("missing");
    let case = dir.join("trial-x/store/IDEA-1/children/IDEA-1.intake");
    copy_dir(&fixture_case().join("work"), &case.join("work"));
    fs::copy(fixture_case().join("case.json"), case.join("case.json")).unwrap();
    let path = case.join("work/questions.yaml");
    let text = fs::read_to_string(&path).unwrap();
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
    doc["questions"][0]
        .as_mapping_mut()
        .unwrap()
        .remove("consequences");
    fs::write(&path, serde_yaml::to_string(&doc).unwrap()).unwrap();
    let out = run(&case, &["q1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("consequences"));
}

/// `sdc.decision.Request` refuses two options with one letter (`duplicate-letters`). The tool
/// letters its options A, B, C in order, so letters never repeat; it runs out of letters after Z,
/// where a 27th option would get a character that is no letter and a 257th a repeated one.
#[test]
fn a_question_with_more_options_than_letters_is_refused() {
    let dir = scratch("many");
    let case = dir.join("trial-x/store/IDEA-1/children/IDEA-1.intake");
    copy_dir(&fixture_case().join("work"), &case.join("work"));
    fs::copy(fixture_case().join("case.json"), case.join("case.json")).unwrap();
    let path = case.join("work/questions.yaml");
    let text = fs::read_to_string(&path).unwrap();
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
    let answers: Vec<String> = (0..27).map(|i| format!("answer {i}")).collect();
    let consequences: Vec<serde_yaml::Value> = answers
        .iter()
        .map(|a| serde_yaml::to_value(json!({"does": format!("take {a}"), "cost": "none"})).unwrap())
        .collect();
    let q = doc["questions"][0].as_mapping_mut().unwrap();
    q.insert("options".into(), serde_yaml::to_value(&answers).unwrap());
    q.insert("default".into(), "answer 0".into());
    q.insert("consequences".into(), consequences.into());
    fs::write(&path, serde_yaml::to_string(&doc).unwrap()).unwrap();
    let out = run(&case, &["q1"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout).chars().take(400).collect::<String>()
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("26 options"));
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

/// The output's `request` against the `sdc.decision` entity schema `ess` generates from
/// `ess/domains/decision.yaml`. `ess generate schema validate` picks a schema by an instance's
/// `schema` property, so the closed schema is given that one property.
const SCHEMA_ID: &str = "https://sdc.invalid/entities/sdc.decision.DecisionRequest.schema.json";
const SCHEMA_FILE: &str = "sdc.decision.DecisionRequest.schema.json";

/// The schema directory the check validates against: the committed generated schema of
/// `DecisionRequest` (`ess/generated/schema`, built with the same arguments `ess generate` is given
/// here before and held current by `task check:ess-generated`) with the `schema` property added,
/// in a cache on disk under `target/` keyed by the digest of the committed file and the patch. The
/// schema holds all its references inside the file, so it serves alone. No generator and no
/// `ess --version` runs: the copy is made once per test run (each test is its own process under
/// cargo-nextest, so it is written to a private directory and renamed into place; the process
/// that loses the rename uses the winner's directory).
fn schemas_with(cache: &Path) -> PathBuf {
    let committed = manifest().join("../ess/generated/schema/schema/entities").join(SCHEMA_FILE);
    let bytes = fs::read(&committed).unwrap_or_else(|e| panic!("{}: {e}", committed.display()));
    let mut h = Sha256::new();
    h.update(&bytes);
    h.update(SCHEMA_ID);
    let key: String = h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
    let ready = cache.join(&key);
    if ready.join(SCHEMA_FILE).is_file() {
        return ready;
    }
    fs::create_dir_all(cache).unwrap();
    let build = cache.join(format!("{key}.build-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build);
    fs::create_dir_all(&build).unwrap();
    let mut schema: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(schema["$id"], SCHEMA_ID);
    assert_eq!(schema["additionalProperties"], false);
    schema["properties"]["schema"] = json!({"const": SCHEMA_ID});
    fs::write(build.join(SCHEMA_FILE), serde_json::to_vec_pretty(&schema).unwrap()).unwrap();
    if fs::rename(&build, &ready).is_err() {
        let _ = fs::remove_dir_all(&build);
    }
    assert!(ready.join(SCHEMA_FILE).is_file(), "{} was not built", ready.display());
    ready
}

fn validate(instance: &Value, name: &str) -> (i32, String) {
    validate_with(&pinned_ess(), &default_cache(), instance, name)
}

/// The pinned ess of `driver/ess-pin.json` where `sdc-conform` installs it, never the `ess` on
/// `PATH`: the forge has none there and installs the pinned one (`sdc-conform ess-install`), and
/// `task test:driver` puts a trap `ess` first on the `PATH` of its tests.
fn pinned_ess() -> PathBuf {
    let pin: Value = serde_json::from_str(&fs::read_to_string(manifest().join("ess-pin.json")).unwrap()).unwrap();
    let ess = crate::ess_pin_guard::installed_ess(&pin);
    assert!(ess.is_file(), "the pinned ess is not installed (run `sdc-conform ess-install` or set ESS_BIN): {}", ess.display());
    ess
}

fn default_cache() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("decision-request-schemas")
}

fn validate_with(ess: &Path, cache: &Path, instance: &Value, name: &str) -> (i32, String) {
    let mut inst = instance.clone();
    inst["schema"] = json!(SCHEMA_ID);
    let inst_file = scratch(name).join("instance.json");
    fs::write(&inst_file, serde_json::to_vec(&inst).unwrap()).unwrap();
    let out = Command::new(ess)
        .args(["generate", "schema", "validate", "--schemas"])
        .arg(schemas_with(cache))
        .arg(&inst_file)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn the_request_validates_against_the_sdc_decision_shape() {
    for id in ["q1", "q3", "q7"] {
        let (_, out) = request_for(id);
        let (code, report) = validate(&out["request"], &format!("shape-{id}"));
        assert_eq!(code, 0, "{id}: {report}");
        assert!(report.contains("1 instance(s): valid"), "{id}: {report}");
    }
}

#[test]
fn the_shape_check_refuses_a_request_that_is_not_in_the_shape() {
    let (_, out) = request_for("q1");
    let mut broken = out["request"].clone();
    broken["state"] = json!("Bogus");
    let (code, report) = validate(&broken, "shape-negative");
    assert_eq!(code, 1, "{report}");
    broken = out["request"].clone();
    broken.as_object_mut().unwrap().remove("options");
    let (code, report) = validate(&broken, "shape-negative");
    assert_eq!(code, 1, "{report}");
}

/// An `ess` that logs each call's arguments and runs the pinned one: `(program, log)`.
fn counting_ess(dir: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let (program, log) = (dir.join("ess"), dir.join("calls.log"));
    fs::write(&program, format!("#!/bin/sh\necho \"$*\" >> '{}'\nexec '{}' \"$@\"\n", log.display(), pinned_ess().display())).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    (program, log)
}

/// The schema the check reads is the committed one (`ess/generated/schema`, held current by
/// `task check:ess-generated`) with the one property the lookup needs; no `ess generate` and no
/// `ess --version` runs for it, in this or in any later test process.
#[test]
fn the_shape_check_reads_the_committed_schemas_and_runs_no_generator() {
    let dir = scratch("shape-no-generator");
    let (ess, log) = counting_ess(&dir);
    let cache = dir.join("cache");
    let (_, out) = request_for("q1");
    for round in ["first", "second"] {
        let (code, report) = validate_with(&ess, &cache, &out["request"], &format!("shape-no-generator-{round}"));
        assert_eq!(code, 0, "{round}: {report}");
        assert!(report.contains("1 instance(s): valid"), "{round}: {report}");
    }
    let calls = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 2, "two validations, two ess starts and no more: {calls}");
    assert!(lines.iter().all(|l| l.starts_with("generate schema validate --schemas ")), "{calls}");

    let committed = manifest().join("../ess/generated/schema/schema/entities/sdc.decision.DecisionRequest.schema.json");
    let mut want: Value = serde_json::from_slice(&fs::read(committed).unwrap()).unwrap();
    want["properties"]["schema"] = json!({"const": SCHEMA_ID});
    let served: Vec<PathBuf> = fs::read_dir(&cache).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(served.len(), 1, "one cache entry: {served:?}");
    let file = served[0].join("sdc.decision.DecisionRequest.schema.json");
    let got: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(got, want, "the cache holds the committed schema with the lookup property");
}
