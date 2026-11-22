//! Adapters: which adapter each schema-reply node class runs on and whether it takes a response schema.
use std::path::Path;

use serde_json::{json, Map, Value};

use super::{read_json, sorted};

/// The adapter of a schema-reply class from the environment, and where it came from.
pub fn class_adapter(class: &str, env: &dyn Fn(&str) -> Option<String>) -> (String, String) {
    let var = format!("SDC_{}_ADAPTER", class.to_ascii_uppercase());
    if let Some(a) = env(&var).filter(|a| !a.is_empty()) {
        return (a, var);
    }
    match env("SDC_LLM_ADAPTER").filter(|a| !a.is_empty()) {
        Some(a) if a == "metaharness" => ("claude-cli".into(), "default: claude-cli, SDC_LLM_ADAPTER is metaharness (no response schema)".into()),
        Some(a) => (a, "SDC_LLM_ADAPTER".into()),
        None => ("stub".into(), "default: stub".into()),
    }
}

/// Whether an adapter takes a response schema: Some(true|false), or None when it calls no model.
pub fn takes_schema(adapter: &str) -> Option<bool> {
    match adapter {
        "stub" | "dry" => None,
        "claude-cli" => Some(true),
        "metaharness" => Some(false),
        path => Some(std::process::Command::new(path).arg("--capabilities").output().ok()
            .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
            .is_some_and(|v| v["json_schema"] == true)),
    }
}

/// `{class: {adapter, source, json_schema, nodes}}` for every schema-reply class, and the refusals.
pub fn adapters(tools: &Path, env: &dyn Fn(&str) -> Option<String>) -> (Value, Vec<String>) {
    let classes = read_json(&tools.join("node-classes.json"));
    let mut out = Map::new();
    let mut refused = Vec::new();
    for c in classes["schema_reply"].as_array().into_iter().flatten().filter_map(|c| c.as_str()) {
        let (a, src) = class_adapter(c, env);
        let schema = takes_schema(&a);
        let nodes = classes["classes"][c].clone();
        if schema == Some(false) {
            refused.push(format!("node class {c} ({}) replies under a response schema, but its adapter {a} ({src}) takes none: \
                set SDC_{}_ADAPTER=claude-cli", nodes.as_array().into_iter().flatten().filter_map(|n| n.as_str()).collect::<Vec<_>>().join(", "),
                c.to_ascii_uppercase()));
        }
        out.insert(c.to_string(), json!({"adapter": a, "source": src, "json_schema": schema, "nodes": nodes}));
    }
    // change.implement as one request (SDC_IMPLEMENT_MODE=request): its reply is the files under a
    // response schema, so it needs a schema adapter like the review; session mode adds nothing.
    match env("SDC_IMPLEMENT_MODE").filter(|m| !m.is_empty()).as_deref() {
        None | Some("session") => {}
        Some("request") => {
            let (a, src) = class_adapter("implement", env);
            let schema = takes_schema(&a);
            if schema == Some(false) {
                refused.push(format!("node class implement (change.implement) replies under a response schema in SDC_IMPLEMENT_MODE=request, \
                    but its adapter {a} ({src}) takes none: set SDC_IMPLEMENT_ADAPTER=claude-cli"));
            }
            out.insert("implement".into(), json!({"adapter": a, "source": src, "json_schema": schema, "nodes": ["change.implement"], "mode": "request"}));
        }
        Some(m) => refused.push(format!("SDC_IMPLEMENT_MODE={m}: neither session nor request")),
    }
    (sorted(Value::Object(out)), refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn the_review_runs_on_claude_cli_when_the_writers_run_on_metaharness() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness")]));
        assert_eq!((v["review"]["adapter"].as_str(), v["review"]["json_schema"].as_bool()), (Some("claude-cli"), Some(true)));
        assert!(refused.is_empty());
        // Unset: the stub (no model); an explicit setting wins.
        assert_eq!(adapters(&tools, &env_of(&[])).0["review"]["adapter"], "stub");
        assert_eq!(adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness"), ("SDC_REVIEW_ADAPTER", "claude-cli")])).0["review"]["source"], "SDC_REVIEW_ADAPTER");
    }

    #[test]
    fn a_schema_class_on_an_adapter_without_a_schema_is_refused() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_REVIEW_ADAPTER", "metaharness")]));
        assert_eq!(v["review"]["json_schema"], false);
        assert_eq!(refused.len(), 1);
        assert!(refused[0].starts_with("node class review (spec.review, change.review) replies under a response schema, but its adapter metaharness (SDC_REVIEW_ADAPTER) takes none"));
        // An adapter program that does not answer --capabilities takes none.
        assert_eq!(takes_schema("/bin/false"), Some(false));
    }

    #[test]
    fn request_mode_puts_change_implement_on_a_schema_adapter() {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/tools");
        // Session mode (unset or `session`): no implement entry, the adapters line as before.
        assert!(adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness")])).0.get("implement").is_none());
        assert!(adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "session")])).0.get("implement").is_none());
        let (v, refused) = adapters(&tools, &env_of(&[("SDC_LLM_ADAPTER", "metaharness"), ("SDC_IMPLEMENT_MODE", "request")]));
        assert_eq!(v["implement"], json!({"adapter": "claude-cli", "source": "default: claude-cli, SDC_LLM_ADAPTER is metaharness (no response schema)",
            "json_schema": true, "nodes": ["change.implement"], "mode": "request"}));
        assert!(refused.is_empty());
        let (_, refused) = adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "request"), ("SDC_IMPLEMENT_ADAPTER", "metaharness")]));
        assert_eq!(refused.len(), 1);
        assert!(refused[0].starts_with("node class implement (change.implement) replies under a response schema"), "{}", refused[0]);
        let (_, refused) = adapters(&tools, &env_of(&[("SDC_IMPLEMENT_MODE", "both")]));
        assert_eq!(refused, vec!["SDC_IMPLEMENT_MODE=both: neither session nor request"]);
    }
}
