//! The `dry` port of `runbook run` (`--dry`): a preset of the run's settings and the printed model
//! commands.
//!
//! The old launcher's `--dry` ran the stub cycle with the adapter `dry` (it builds the command the
//! real adapter would run, prints it and calls nothing), played the person's decisions by script and
//! labelled the run a stand-in. Here the same is three small parts:
//!
//! - [`preset`]: `adapter` of the settings becomes `dry` (every model node of the bindings copy runs
//!   the adapter `dry`); a model decider gets `decider.adapter: dry`, which answers by the approve
//!   rule and calls no model; with no decider in the settings the rule decider `approve` at
//!   autonomy `A5` plays the person (the decider's principal, `decider:approve`, is the one Loom
//!   is told the person steps of the copy are bound to). A settings file that names another
//!   adapter or another autonomy is refused, and so is `adapter: dry` without `--dry`.
//! - [`mark`]: `dry: true` and `stand_in` in the recorded settings, so `run-settings.jsonl` says
//!   the run is no evaluation's.
//! - [`CommandSink`]: prints every command a dry call would have run, as the dry adapter does
//!   (`would run: metaharness ...`, indented two spaces), and appends it to `dry-commands.txt` of
//!   the run directory. The dry adapter writes to its standard error, which Loom keeps; the sink
//!   reads the call's record (`argv.json`, `request.json`) instead.
//!
//! `--dry` lowers no option: the wall limit and the confinement stay required; the ledger, the cap and the call limit are optional, as in every run.
//! A dry call costs 0 (`cost.rs`), so a ledger stays at 0 and no login file is read.

use super::events::{CostSource, Event, EventSink};
use super::ports::{Ports, Refusal};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The adapter of every model call of a dry run.
pub const ADAPTER: &str = "dry";
/// The autonomy level at which the decider answers (`decider.rs`): the scripted person is one.
const AUTONOMY: &str = "A5";
/// The label a dry run's settings carry: a script stands in for the person.
pub const STAND_IN: &str = "runbook run --dry";
/// The file of the run directory that holds the printed commands, one per line.
pub const LOG: &str = "dry-commands.txt";

/// What of `--dry` stays refused: nothing. The function stays so `refuse_unbuilt` (`ports.rs`)
/// is the same for the six ports.
pub fn dry_not_built(_ports: &Ports) -> Result<(), Refusal> {
    Ok(())
}

/// The settings of a dry run: every model node on the adapter `dry`, a model decider on its dry
/// adapter, the rule decider `approve` where none is named. Without `--dry`, the adapter `dry` in
/// the settings is refused: it belongs to the flag.
pub fn preset(dry: bool, mut settings: Value) -> Result<Value, String> {
    let top = settings.as_object_mut().ok_or("settings: an object")?;
    match (dry, top.get("adapter")) {
        (false, Some(a)) if a.as_str() == Some(ADAPTER) => {
            return Err("settings.adapter: dry is what --dry sets; give --dry instead".to_owned());
        }
        (false, _) => return Ok(settings),
        (true, None) => {}
        (true, Some(a)) if a.as_str() == Some(ADAPTER) => {}
        (true, Some(a)) => {
            return Err(format!("settings.adapter is {a}, and --dry runs every model node on the adapter dry: take adapter out of the settings"));
        }
    }
    top.insert("adapter".to_owned(), json!(ADAPTER));
    match top.get_mut("decider").and_then(Value::as_object_mut) {
        Some(decider) => {
            if decider.get("kind").and_then(Value::as_str) == Some("model") {
                match decider.get("adapter") {
                    None => {}
                    Some(a) if a.as_str() == Some(ADAPTER) => {}
                    Some(a) => {
                        return Err(format!("settings.decider.adapter is {a}, and --dry decides on the adapter dry: take it out of the settings"));
                    }
                }
                decider.insert("adapter".to_owned(), json!(ADAPTER));
            }
        }
        None => {
            match top.get("autonomy") {
                None => {}
                Some(a) if a.as_str() == Some(AUTONOMY) => {}
                Some(a) => {
                    return Err(format!("settings.autonomy is {a}: --dry plays the person's decisions by the rule decider approve, which answers only at {AUTONOMY}; take autonomy out of the settings"));
                }
            }
            top.insert("autonomy".to_owned(), json!(AUTONOMY));
            top.insert("decider".to_owned(), json!({"kind": "rule", "verdict": "approve"}));
        }
    }
    Ok(settings)
}

/// The recorded settings of a dry run: `dry` and the stand-in label added.
pub fn mark(dry: bool, mut recorded: Value) -> Value {
    if let (true, Some(o)) = (dry, recorded.as_object_mut()) {
        o.insert("dry".to_owned(), json!(true));
        o.insert("stand_in".to_owned(), json!(STAND_IN));
    }
    recorded
}

/// `text` as one shell word in single quotes, as `jq`'s `@sh` makes it.
fn shell_word(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The line the dry adapter prints for the call whose record directory is `record`: the metaharness
/// command with the prompt as `"$(cat <prompt file>)"`. `None` when the directory holds no `argv.json`.
pub fn command_line(record: &Path) -> Option<String> {
    let argv: Vec<String> = serde_json::from_slice(&std::fs::read(record.join("argv.json")).ok()?).ok()?;
    let prompt = std::fs::read(record.join("request.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|r| r.get("prompt_file").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "<prompt_file>".to_owned());
    let words: Vec<String> = argv.iter().skip(1).map(|a| if a == "<prompt_file>" { format!("\"$(cat {prompt})\"") } else { shell_word(a) }).collect();
    Some(format!("would run: metaharness {}", words.join(" ")))
}

/// Prints the commands of the dry calls of a run, in the order the calls were made.
pub struct CommandSink {
    run_dir: PathBuf,
    seen: BTreeSet<PathBuf>,
}

impl CommandSink {
    /// A sink for the run directory `run_dir`.
    pub fn new(run_dir: &Path) -> CommandSink {
        CommandSink { run_dir: run_dir.to_path_buf(), seen: BTreeSet::new() }
    }

    fn print(&mut self, record: &Path) -> Result<(), String> {
        if !self.seen.insert(record.to_path_buf()) {
            return Ok(());
        }
        let Some(line) = command_line(record) else { return Ok(()) };
        println!("  {line}");
        let log = self.run_dir.join(LOG);
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        writeln!(file, "{line}").map_err(|e| format!("{}: {e}", log.display()))
    }

    /// The calls the decider made: one directory per call below `decider/` of the run directory.
    fn print_decider_calls(&mut self) -> Result<(), String> {
        let Ok(entries) = std::fs::read_dir(self.run_dir.join("decider")) else { return Ok(()) };
        let mut dirs: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        dirs.iter().try_for_each(|d| self.print(d))
    }
}

impl EventSink for CommandSink {
    fn event(&mut self, _at_secs: u64, event: &Event) -> Result<(), String> {
        match event {
            Event::CostedCall { source: CostSource::Step, record, .. } if record.get("dry") == Some(&Value::Bool(true)) => {
                match record.get("record").and_then(Value::as_str) {
                    Some(dir) => self.print(Path::new(dir)),
                    None => Ok(()),
                }
            }
            Event::DecisionAnswered { .. } | Event::AnswerRecorded { .. } | Event::DeclaredTaken { .. } | Event::Ended { .. } => self.print_decider_calls(),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_node_goes_to_the_dry_adapter_and_a_model_decider_to_its_own() {
        let given = json!({"decider": {"kind": "model", "model": "m"}, "models": {"writer": {"model": "w"}}});
        let out = preset(true, given).unwrap();
        assert_eq!(out["adapter"], "dry");
        assert_eq!(out["decider"]["adapter"], "dry");
        assert_eq!(out["models"]["writer"]["model"], "w", "the rest is kept");
    }

    #[test]
    fn a_rule_decider_and_a_scripted_one_are_left_alone() {
        for decider in [json!({"kind": "rule", "verdict": "approve"}), json!({"kind": "scripted", "answers": []})] {
            let out = preset(true, json!({"decider": decider.clone()})).unwrap();
            assert_eq!(out["decider"], decider);
        }
    }

    #[test]
    fn another_adapter_is_refused_with_the_flag_and_dry_is_refused_without_it() {
        for settings in [json!({"adapter": "metaharness"}), json!({"adapter": "claude-cli"}), json!({"decider": {"kind": "model", "adapter": "claude-cli"}})] {
            let why = preset(true, settings).unwrap_err();
            assert!(why.contains("--dry") && why.contains("take"), "{why}");
        }
        assert!(preset(false, json!({"adapter": "dry"})).unwrap_err().contains("give --dry"));
        assert_eq!(preset(false, json!({"adapter": "claude-cli"})).unwrap(), json!({"adapter": "claude-cli"}));
        assert_eq!(preset(true, json!({"adapter": "dry"})).unwrap()["adapter"], "dry");
    }

    #[test]
    fn the_rule_decider_approve_plays_the_person_when_the_settings_name_no_decider() {
        let out = preset(true, json!({})).unwrap();
        assert_eq!((out["decider"].clone(), out["autonomy"].clone()), (json!({"kind": "rule", "verdict": "approve"}), json!("A5")));
        assert_eq!(preset(true, json!({"autonomy": "A5"})).unwrap()["autonomy"], "A5");
        assert!(preset(true, json!({"autonomy": "A0"})).unwrap_err().contains("autonomy"));
        assert_eq!(preset(false, json!({})).unwrap(), json!({}), "no flag, no preset");
        let named = json!({"kind": "rule", "verdict": "reject"});
        assert_eq!(preset(true, json!({"decider": named.clone(), "autonomy": "A3"})).unwrap()["decider"], named, "a decider that is named is left alone");
    }

    #[test]
    fn the_recorded_settings_say_stand_in() {
        let m = mark(true, json!({"a": 1}));
        assert_eq!((m["dry"].clone(), m["stand_in"].clone()), (json!(true), json!("runbook run --dry")));
        assert_eq!(mark(false, json!({"a": 1})), json!({"a": 1}));
    }

    #[test]
    fn a_command_is_the_dry_adapters_line() {
        let scratch = crate::assets::testdir::Dir::new("dry-line");
        let dir = scratch.path().to_path_buf();
        let argv = json!(["/x/metaharness-stand-in", "run", "claude", "--model", "m-1", "--note", "it's", "-p", "<prompt_file>"]);
        std::fs::write(dir.join("argv.json"), argv.to_string()).unwrap();
        std::fs::write(dir.join("request.json"), json!({"prompt_file": "/w/prompt.md"}).to_string()).unwrap();
        assert_eq!(
            command_line(&dir).unwrap(),
            "would run: metaharness 'run' 'claude' '--model' 'm-1' '--note' 'it'\\''s' '-p' \"$(cat /w/prompt.md)\""
        );
        assert_eq!(command_line(&dir.join("nothing")), None);
    }
}
