//! The input schemas of the tools, derived from the generated OpenAPI document of the daemon
//! (`ess/generated/openapi/openapi/runbookd.yaml`, the HTTP contract), the same document the HTTP
//! routes are held to. A command tool's schema is the `Input` schema of its command with the members
//! the handler fills removed; a view tool's schema is built from the query parameters of its view.
//! Every `$ref` is inlined, so a schema stands alone, and the `x-ess-*` annotations are dropped.

use serde_json::{json, Map, Value};
use std::sync::OnceLock;

/// The document, compiled into the binary.
const DOCUMENT: &str = include_str!("../../../ess/generated/openapi/openapi/runbookd.yaml");

fn doc() -> &'static Value {
    static DOC: OnceLock<Value> = OnceLock::new();
    DOC.get_or_init(|| serde_yaml::from_str(DOCUMENT).expect("the OpenAPI document of the daemon is YAML"))
}

fn pascal(kebab: &str) -> String {
    kebab.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect()
}

const REF_PREFIX: &str = "#/components/schemas/";

/// A schema with every `$ref` replaced by what it names and the `x-ess-*` members removed.
fn inline(v: &Value, depth: usize) -> Value {
    assert!(depth < 16, "a schema of the OpenAPI document nests too deep or refers to itself");
    match v {
        Value::Object(m) => {
            if let Some(name) = m.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix(REF_PREFIX)) {
                let target = doc()["components"]["schemas"].get(name).unwrap_or_else(|| panic!("the OpenAPI document has no schema {name}"));
                return inline(target, depth + 1);
            }
            Value::Object(m.iter().filter(|(k, _)| !k.starts_with("x-ess-")).map(|(k, x)| (k.clone(), inline(x, depth + 1))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| inline(x, depth + 1)).collect()),
        other => other.clone(),
    }
}

/// The input schema of a command (wire name, e.g. `submit-run`): the document's `Input` schema
/// without the members the handler fills (`filled`) and with the members the daemon defaults
/// (`defaulted`) not required.
pub fn command_input(command: &str, filled: &[&str], defaulted: &[&str]) -> Value {
    let name = format!("sdc.daemon.{}.Input", pascal(command));
    let input = doc()["components"]["schemas"].get(&name).unwrap_or_else(|| panic!("the OpenAPI document has no schema {name}"));
    let properties: Map<String, Value> = input["properties"]
        .as_object()
        .expect("an Input schema has properties")
        .iter()
        .filter(|(k, _)| !filled.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), inline(v, 0)))
        .collect();
    let required: Vec<Value> = input["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).filter(|k| !filled.contains(k) && !defaulted.contains(k)).map(|k| json!(k)).collect())
        .unwrap_or_default();
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

/// The schema of a query parameter of a view, by view wire name and parameter name.
pub fn view_param(view: &str, param: &str) -> Value {
    let parameters = doc()["paths"][format!("/daemon/views/{view}")]["get"]["parameters"].as_array().unwrap_or_else(|| panic!("the OpenAPI document has no view {view}"));
    let p = parameters.iter().find(|p| p["name"] == param).unwrap_or_else(|| panic!("the view {view} has no parameter {param}"));
    inline(&p["schema"], 0)
}

/// An object schema with these properties, the ones named in `required` required.
pub fn object(properties: Vec<(&str, Value)>, required: &[&str]) -> Value {
    let props: Map<String, Value> = properties.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}
