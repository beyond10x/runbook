//! Which decisions the decider takes at an autonomy level, and which stay with a person.
//!
//! Two files, both read from the root above the bindings copy (the run directory at run time, the
//! checkout when the copy is made): `driver/examples/sdc/decision-classes.json`
//! (`sdc.decision-classes/1`: the class of an action and the side it belongs to) and
//! `tools/autonomy-levels.json` (`sdc.autonomy-levels/1`: the holder of each class at each level,
//! the table of `docs/autonomy-levels.md` § 2.2). This module is the one loader of both; the
//! requester decider reads the classes through it too. The run reads them once, before its first
//! action (`Decider::pin_routing`): the run directory's copy is writable by a bound program.
//!
//! The rule is closed. Below A5 the decider takes a decision only where the table gives its class
//! to `decider`; every other holder (a person, an advised person, a rule, a rule with the decider)
//! stops the run for a person, a class the table does not know stops it, and a table that is
//! missing, unreadable or inconsistent stops it. Nothing here widens what a level gives.

use super::*;
use std::collections::BTreeMap;

const CLASSES_FILE: &str = "driver/examples/sdc/decision-classes.json";
/// The holder table, relative to the root above the bindings copy.
pub const LEVELS_FILE: &str = "tools/autonomy-levels.json";
const LEVELS_FORMAT: &str = "sdc.autonomy-levels/1";
/// The values of `sdc.decision.Holder` and `none` (a class that does not arise at the level).
const HOLDERS: [&str; 9] = ["person", "advised", "decider", "decider_final", "rule", "rule_final", "rule_and_decider", "rule_and_decider_final", "none"];
/// The class of the close of the root case, and the holder that closes it by rule.
const CLOSE_CLASS: &str = "close";
const RULE_HOLDER: &str = "rule";
/// The principal of a close taken by rule.
pub(super) const AUTOPILOT: &str = "rule:autopilot";

/// The classes of `decision-classes.json`.
pub(super) fn read_classes(root: &Path) -> Result<Map<String, Value>, String> {
    let text = std::fs::read_to_string(root.join(CLASSES_FILE)).map_err(|e| format!("{CLASSES_FILE} is needed above the bindings copy: {}", e.kind()))?;
    let doc: Value = serde_json::from_str(&text).map_err(|_| format!("{CLASSES_FILE} is not JSON"))?;
    doc["classes"].as_object().cloned().ok_or_else(|| format!("{CLASSES_FILE} has no classes"))
}

/// The class of a request of `kind` (`approval`, `question`, `finding` or `close`) on `action`: a
/// question is a `scope_changing_answer`, a finding a `finding`, a close a `close`, an approval the
/// first class that lists the action (`<protocol>/<action>`), else `approval`.
pub(super) fn class_of(classes: &Map<String, Value>, kind: &str, action: &str) -> String {
    match kind {
        "question" => "scope_changing_answer".to_owned(),
        "finding" => "finding".to_owned(),
        "close" => CLOSE_CLASS.to_owned(),
        _ => {
            let tail = format!("/{action}");
            classes
                .iter()
                .find(|(_, v)| v["actions"].as_array().is_some_and(|a| a.iter().filter_map(Value::as_str).any(|x| x.ends_with(&tail))))
                .map_or_else(|| "approval".to_owned(), |(k, _)| k.clone())
        }
    }
}

/// The classes and the holder of each class at each level.
pub struct Routing {
    classes: Map<String, Value>,
    cells: BTreeMap<String, BTreeMap<String, String>>,
}

impl Routing {
    /// Reads and checks both files under `root`: the format, every level A0 to A5, and for each
    /// level exactly the classes of the classes file with a known holder. Anything else is an error,
    /// and the caller stops for a person.
    pub fn load(root: &Path) -> Result<Routing, String> {
        let classes = read_classes(root)?;
        let text = std::fs::read_to_string(root.join(LEVELS_FILE)).map_err(|e| format!("{LEVELS_FILE} is needed above the bindings copy: {}", e.kind()))?;
        let doc: Value = serde_json::from_str(&text).map_err(|_| format!("{LEVELS_FILE} is not JSON"))?;
        if doc["format"] != LEVELS_FORMAT {
            return Err(format!("{LEVELS_FILE}: format is not {LEVELS_FORMAT}"));
        }
        let levels = doc["levels"].as_object().ok_or_else(|| format!("{LEVELS_FILE} has no levels"))?;
        let mut cells = BTreeMap::new();
        for level in AUTONOMY_LEVELS {
            let row = levels.get(level).and_then(Value::as_object).ok_or_else(|| format!("{LEVELS_FILE}: level {level} is missing"))?;
            let mut held = BTreeMap::new();
            for (class, holder) in row {
                if !classes.contains_key(class) {
                    return Err(format!("{LEVELS_FILE}: level {level} names the class {class}, which {CLASSES_FILE} does not have"));
                }
                match holder.as_str() {
                    Some(h) if HOLDERS.contains(&h) => held.insert(class.clone(), h.to_owned()),
                    _ => return Err(format!("{LEVELS_FILE}: level {level}, class {class}: not a holder ({})", HOLDERS.join(", "))),
                };
            }
            if let Some(class) = classes.keys().find(|c| !held.contains_key(*c)) {
                return Err(format!("{LEVELS_FILE}: level {level} gives no holder to the class {class}"));
            }
            cells.insert(level.to_owned(), held);
        }
        if let Some(extra) = levels.keys().find(|k| !AUTONOMY_LEVELS.contains(&k.as_str())) {
            return Err(format!("{LEVELS_FILE}: {extra} is not a level (A0 to A5)"));
        }
        Ok(Routing { classes, cells })
    }

    /// The class of a decision (see [`class_of`]).
    pub fn class_of(&self, kind: &str, action: &str) -> String {
        class_of(&self.classes, kind, action)
    }

    /// The side a class belongs to (`requester` or `delivering`); a class that is not in the file
    /// is on the delivering side.
    pub fn side_of(&self, class: &str) -> String {
        self.classes.get(class).and_then(|c| c["side"].as_str()).unwrap_or("delivering").to_owned()
    }

    /// The holder of `class` at `level`; `None` for a class or a level the table does not have.
    pub fn holder(&self, level: &str, class: &str) -> Option<&str> {
        self.cells.get(level)?.get(class).map(String::as_str)
    }

    /// Whether the decider takes a decision of `kind` on `action` at `level`: only where the table
    /// gives the class to `decider`. A class the table does not know is not given.
    pub fn decider_takes(&self, level: &str, kind: &str, action: &str) -> bool {
        self.holder(level, &self.class_of(kind, action)) == Some("decider")
    }

    /// Whether the close of the root case through `outcome` is taken by rule at `level`: the close
    /// class lists `sdc/close:<outcome>` and its holder at the level is `rule`.
    pub fn closes_by_rule(&self, level: &str, outcome: &str) -> bool {
        let want = format!("sdc/close:{outcome}");
        let listed = self.classes.get(CLOSE_CLASS).and_then(|c| c["actions"].as_array()).is_some_and(|a| a.iter().any(|x| x.as_str() == Some(want.as_str())));
        listed && self.holder(level, CLOSE_CLASS) == Some(RULE_HOLDER)
    }
}
