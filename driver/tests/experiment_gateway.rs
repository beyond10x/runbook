//! The gateway of a launched run (`docs/loom-run-settings.md`, "What `sdc-experiment run` writes"): a `loom`
//! record names a gateway by `gateway.base_url` and, for a gateway that takes a token, `gateway.token_file`, never
//! the token itself; `sdc-experiment run` writes the `[gateway]` table into the settings file of the run, and
//! `--print` shows the token file by its name only. A `free-agent` variant and `sdc-free-agent` take no gateway
//! and refuse it by name. Nothing here starts a unit, calls a model or reads a token file.

use super::experiment_loom::{experiment, fake_build, fixed, free_text, printed_settings, record_with, scratch, write_record, COMMIT};
use std::path::Path;
use std::process::Command;

const BASE: &str = "http://127.0.0.1:8080";
const MARKER: &str = "/opt/marker-gw-5b2e/token";

/// The example record's settings with a gateway: the adapters of the model nodes and the decider are claude-cli
/// (metaharness cannot take a gateway), no login file, and `extra` after them.
fn gateway_record(extra: &[(&str, &str)], commit: &str) -> String {
    let mut f: Vec<(String, String)> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").map(|(k, v)| if k == "adapter" { (k, "claude-cli".to_owned()) } else { (k, v) }).collect();
    f.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    record_with(&f, commit, "planned", ("", ""))
}

fn print(d: &Path, builds: &Path, more: &[&str]) -> (i32, String) {
    let mut args = vec!["run", "lx", "--print", "--builds-dir", builds.to_str().unwrap()];
    args.extend_from_slice(more);
    experiment(&args, d)
}

#[test]
fn a_loom_record_that_names_a_gateway_by_token_file_writes_the_table_and_prints_the_file_by_name_only() {
    let d = scratch("gateway-written");
    let builds = fake_build(&d, 8, true);
    write_record(&d, &gateway_record(&[("gateway.base_url", BASE), ("gateway.token_file", MARKER)], COMMIT));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_eq!(code, 0, "{out}");
    let (code, out) = print(&d, &builds, &[]);
    assert_eq!(code, 0, "{out}");
    let settings = printed_settings(&out);
    assert!(settings.contains(&format!("[gateway]\nbase_url = \"{BASE}\"\ntoken_file = \"<gateway.token_file>\"\n")), "{settings}");
    assert!(!settings.contains("login_file") && !out.contains("marker-gw-5b2e"), "{out}");
    // a gateway without ingress auth: the address alone
    write_record(&d, &gateway_record(&[("gateway.base_url", BASE)], COMMIT));
    let (code, out) = print(&d, &builds, &[]);
    assert_eq!(code, 0, "{out}");
    let settings = printed_settings(&out);
    assert!(settings.contains(&format!("[gateway]\nbase_url = \"{BASE}\"\n")) && !settings.contains("token"), "{settings}");
    // a record that names none writes no table
    write_record(&d, &gateway_record(&[], COMMIT));
    let (_, out) = print(&d, &builds, &[]);
    assert!(!printed_settings(&out).contains("gateway"), "{out}");
}

#[test]
fn a_loom_record_that_names_a_token_a_relative_file_a_login_beside_a_gateway_or_an_adapter_that_cannot_take_it_is_refused_by_name() {
    let d = scratch("gateway-refused");
    let builds = fake_build(&d, 8, true);
    // the stand-in value is made at run time from the process and the case's directory: the source holds no token-shaped text
    let standin = format!("standin-{}-{}", std::process::id(), d.file_name().unwrap().to_string_lossy());
    let ftp = format!("ftp://{standin}");
    let both = [("gateway.base_url", BASE), ("gateway.token_file", MARKER)];
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (vec![("gateway.base_url", BASE), ("gateway.token", standin.as_str())], "gateway.token"),
        (vec![("gateway.base_url", BASE), ("gateway.token_file", "relative/token")], "gateway.token_file"),
        (vec![("gateway.token_file", MARKER)], "gateway.base_url"),
        (vec![("gateway.base_url", ftp.as_str()), ("gateway.token_file", MARKER)], "gateway.base_url"),
        (vec![("gateway.base_url", BASE), ("gateway.model", standin.as_str())], "gateway.model"),
        ([both.as_slice(), &[("loom.login_file", "/opt/marker-login-9f3a/credentials.json")]].concat(), "loom.login_file"),
    ];
    for (extra, member) in &cases {
        write_record(&d, &gateway_record(extra, "null"));
        let (code, out) = experiment(&["check", "lx"], &d);
        assert_ne!(code, 0, "{member}: accepted: {out}");
        assert!(out.contains(member), "{member}: the refusal does not name it: {out}");
        assert!(!out.contains(&standin) && !out.contains("marker-gw-5b2e") && !out.contains("marker-login-9f3a"), "{member}: a value is in the refusal: {out}");
    }
    // metaharness, the model nodes' adapter or the model decider's default, cannot take a gateway
    let mut f: Vec<(String, String)> = fixed().into_iter().filter(|(k, _)| k != "max_llm_calls").collect();
    f.extend(both.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    write_record(&d, &record_with(&f, "null", "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert!(code != 0 && out.contains("adapter") && out.contains("metaharness") && out.contains("gateway"), "{out}");
    let f: Vec<(String, String)> = f.into_iter().filter(|(k, _)| k != "decider.adapter").map(|(k, v)| if k == "adapter" { (k, "claude-cli".to_owned()) } else { (k, v) }).collect();
    write_record(&d, &record_with(&f, "null", "planned", ("", "")));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert!(code != 0 && out.contains("decider.adapter") && out.contains("gateway"), "{out}");
    // a login file named at the start does not go beside a gateway
    write_record(&d, &gateway_record(&both, COMMIT));
    let (code, out) = print(&d, &builds, &["--login-file", "/opt/marker-login-9f3a/credentials.json"]);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("gateway") && out.contains("--login-file") && !out.contains("marker-login-9f3a") && !out.contains("marker-gw-5b2e"), "{out}");
}

#[test]
fn a_free_agent_variant_and_sdc_free_agent_refuse_a_gateway_by_name() {
    let d = scratch("gateway-free-agent");
    write_record(&d, &free_text(&format!("  gateway.base_url: {BASE}\n  gateway.token_file: {MARKER}\n"), "null"));
    let (code, out) = experiment(&["check", "lx"], &d);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("gateway.base_url") && out.contains("gateway.token_file") && out.contains("free-agent takes no gateway"), "{out}");
    assert!(!out.contains("marker-gw-5b2e"), "{out}");
    for option in ["--gateway-file", "--gateway"] {
        let o = Command::new(env!("CARGO_BIN_EXE_sdc-free-agent")).args(["run", option, MARKER]).env_clear().env("PATH", "/usr/bin:/bin").output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(!o.status.success(), "{option}: {err}");
        assert!(err.contains(option) && err.contains("takes no gateway") && !err.contains("marker-gw-5b2e"), "{option}: {err}");
    }
}
