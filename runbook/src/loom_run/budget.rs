//! The budget of one call: before each step the bindings copy is limited so that no call may
//! spend more than the ledger granted. Only the `--max-budget-usd` argv value of a model tool is
//! touched: in a tree only `max_runs` acts, and Loom refuses `max_budget_micro_usd` (and
//! `max_turns`, `max_tokens`) at load on a non-agent binding, so this module never adds or keeps
//! such a member.

use crate::loom_run::record_write::Held;
use serde_yaml::Value;
use std::ffi::OsString;
use std::path::Path;

const ARG: &str = "--max-budget-usd";

/// The bindings directory `dir`, held ([`Held::open`]: a link at its name is an error), and the names of its `*.yaml` and
/// `*.yml` files, listed through the descriptor.
fn yaml_files(dir: &Path) -> Result<(Held, Vec<OsString>), String> {
    let held = Held::open(dir)?;
    let files = held.names()?.into_iter().filter(|n| Path::new(n).extension().is_some_and(|x| x == "yaml" || x == "yml") && held.is_file(n)).collect();
    Ok((held, files))
}

fn usd_text_to_micro(s: &str) -> Option<u64> {
    let x: f64 = s.trim().parse().ok()?;
    (x.is_finite() && x >= 0.0).then(|| (x * 1_000_000.0).round() as u64)
}

fn micro_to_usd_text(micro: u64) -> String {
    let s = format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn scalar_usd(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => usd_text_to_micro(s),
        Value::Number(n) => n.as_f64().and_then(|x| (x.is_finite() && x >= 0.0).then(|| (x * 1_000_000.0).round() as u64)),
        _ => None,
    }
}

fn walk_largest(v: &Value, best: &mut Option<u64>) {
    match v {
        Value::Sequence(items) => {
            for (i, item) in items.iter().enumerate() {
                if item.as_str() == Some(ARG) {
                    if let Some(m) = items.get(i + 1).and_then(scalar_usd) {
                        *best = Some(best.map_or(m, |b| b.max(m)));
                    }
                }
                walk_largest(item, best);
            }
        }
        Value::Mapping(map) => {
            for item in map.values() {
                walk_largest(item, best);
            }
        }
        _ => {}
    }
}

fn walk_limit(v: &mut Value, limit: u64) -> bool {
    let mut changed = false;
    match v {
        Value::Sequence(items) => {
            for i in 0..items.len() {
                if items[i].as_str() == Some(ARG) {
                    if let Some(m) = items.get(i + 1).and_then(scalar_usd) {
                        if m > limit {
                            items[i + 1] = Value::String(micro_to_usd_text(limit));
                            changed = true;
                        }
                    }
                }
                changed |= walk_limit(&mut items[i], limit);
            }
        }
        Value::Mapping(map) => {
            for item in map.values_mut() {
                changed |= walk_limit(item, limit);
            }
        }
        _ => {}
    }
    changed
}

/// The largest per-call budget the bindings in `dir` name (`--max-budget-usd` values), micro USD; `None` when they name none.
pub fn largest_call_budget(dir: &Path) -> Result<Option<u64>, String> {
    let mut best = None;
    let (held, files) = yaml_files(dir)?;
    for name in files {
        let text = held.read_text(&name)?;
        let v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", dir.join(&name).display()))?;
        walk_largest(&v, &mut best);
    }
    Ok(best)
}

/// Lowers every per-call budget in the bindings copy `dir` to at most `micro_usd`. The directory is held for the whole
/// call: every file is read and written through its descriptor, never through its path.
pub fn limit_call_budgets(dir: &Path, micro_usd: u64) -> Result<(), String> {
    let (held, files) = yaml_files(dir)?;
    for name in files {
        let file = dir.join(&name);
        let text = held.read_text(&name)?;
        let mut v: Value = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        if walk_limit(&mut v, micro_usd) {
            let out = serde_yaml::to_string(&v).map_err(|e| format!("{}: {e}", file.display()))?;
            held.write(&name, out.as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::current_exe().unwrap().parent().unwrap().join(format!("loomrun-budget-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const BINDINGS: &str = "format: loom.bindings/1\nactions:\n  - action: a\n    executor:\n      kind: program\n      argv: [sdc-node, a, --max-turns, \"15\", --max-budget-usd, \"2\", --stub, x]\n    budget: {max_runs: 3}\n  - action: b\n    executor:\n      kind: program\n      argv: [sdc-node, b, --max-budget-usd, \"0.5\"]\n";

    #[test]
    fn the_largest_budget_is_read_and_limiting_lowers_only_the_argv_values() {
        let d = dir("limit");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        assert_eq!(largest_call_budget(&d).unwrap(), Some(2_000_000));
        limit_call_budgets(&d, 300_000).unwrap();
        let text = std::fs::read_to_string(d.join("sdc.yaml")).unwrap();
        assert!(!text.contains("max_budget_micro_usd"), "no member Loom refuses on a program binding: {text}");
        assert!(text.contains("max_runs: 3"), "{text}");
        assert!(!text.contains("'2'") && !text.contains("\"2\"") && !text.contains("'0.5'") && !text.contains("\"0.5\""), "{text}");
        assert_eq!(largest_call_budget(&d).unwrap(), Some(300_000));
    }

    #[test]
    fn limiting_refuses_a_link_at_a_bindings_file_and_writes_nothing_outside() {
        let d = dir("link");
        let outside = d.join("outside.yaml");
        std::fs::write(&outside, BINDINGS).unwrap();
        std::fs::create_dir(d.join("bindings")).unwrap();
        std::os::unix::fs::symlink(&outside, d.join("bindings/sdc.yaml")).unwrap();
        let refused = limit_call_budgets(&d.join("bindings"), 300_000).unwrap_err();
        assert!(refused.contains("sdc.yaml"), "{refused}");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), BINDINGS, "the file the link points at was written");
        assert!(std::fs::symlink_metadata(d.join("bindings/sdc.yaml")).unwrap().file_type().is_symlink(), "the link was replaced by a file the run never read");
    }

    #[test]
    fn limiting_keeps_the_rework_members_of_a_budget_as_they_were() {
        let d = dir("rework");
        let text = BINDINGS.replace("budget: {max_runs: 3}", "budget: {max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}");
        std::fs::write(d.join("sdc.yaml"), &text).unwrap();
        limit_call_budgets(&d, 300_000).unwrap();
        let v: Value = serde_yaml::from_str(&std::fs::read_to_string(d.join("sdc.yaml")).unwrap()).unwrap();
        let want: Value = serde_yaml::from_str("{max_runs: 3, runs_per: rejection, rework_limit: 2, rework_on: {code_review: [rejected], merge_approval: [rejected]}}").unwrap();
        assert_eq!(v["actions"][0]["budget"], want, "{v:?}");
    }

    #[test]
    fn nothing_is_written_when_no_budget_is_above_the_limit() {
        let d = dir("nochange");
        std::fs::write(d.join("sdc.yaml"), BINDINGS).unwrap();
        limit_call_budgets(&d, 5_000_000).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("sdc.yaml")).unwrap(), BINDINGS);
    }
}
