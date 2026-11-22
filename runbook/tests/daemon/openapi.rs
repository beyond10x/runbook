//! Reads the generated OpenAPI document of the daemon, the HTTP contract, at test time.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

pub fn doc() -> &'static Value {
    static DOC: OnceLock<Value> = OnceLock::new();
    DOC.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ess/generated/openapi/openapi/runbookd.yaml");
        serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    })
}

/// Every (METHOD, path) the document has.
pub fn routes() -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for (path, item) in doc()["paths"].as_object().unwrap() {
        for method in item.as_object().unwrap().keys() {
            out.insert((method.to_uppercase(), path.clone()));
        }
    }
    out
}

fn pascal(kebab: &str) -> String {
    kebab.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect()
}

fn refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x.as_str()) {
                    ("$ref", Some(r)) => out.push(r.to_string()),
                    _ => refs(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| refs(x, out)),
        _ => {}
    }
}

/// For a command, each outcome the document gives a response to, with the status it is under.
pub fn statuses(command: &str) -> BTreeMap<String, u16> {
    let prefix = format!("#/components/schemas/sdc.daemon.{}.", pascal(command));
    let responses = &doc()["paths"][format!("/daemon/commands/{command}")]["post"]["responses"];
    let mut out = BTreeMap::new();
    for (status, response) in responses.as_object().unwrap() {
        let mut found = vec![];
        refs(response, &mut found);
        for r in found {
            if let Some(outcome) = r.strip_prefix(&prefix).and_then(|s| s.strip_suffix(".Response")) {
                out.insert(outcome.to_string(), status.parse().unwrap());
            }
        }
    }
    out
}

fn schema(name: &str) -> &'static Value {
    let s = &doc()["components"]["schemas"][name];
    assert!(!s.is_null(), "no schema {name}");
    s
}

fn resolve(r: &Value) -> &'static Value {
    schema(r["$ref"].as_str().unwrap().rsplit('/').next().unwrap())
}

fn check_object(what: &str, schema: &Value, body: &Value) {
    let obj = body.as_object().unwrap_or_else(|| panic!("{what}: not an object: {body}"));
    let props = schema["properties"].as_object().unwrap();
    for r in schema["required"].as_array().into_iter().flatten() {
        assert!(obj.contains_key(r.as_str().unwrap()), "{what}: missing {r} in {body}");
    }
    if schema["additionalProperties"] == Value::Bool(false) {
        for k in obj.keys() {
            assert!(props.contains_key(k), "{what}: the document has no member {k}: {body}");
        }
    }
}

/// The body of an outcome is the document's response schema for it: its members, its constants, the
/// payload of its error and the events it publishes.
pub fn check_response(command: &str, outcome: &str, body: &Value) {
    let what = format!("{command}/{outcome}");
    let s = schema(&format!("sdc.daemon.{}.{outcome}.Response", pascal(command)));
    check_object(&what, s, body);
    let props = &s["properties"];
    assert_eq!(body["outcome"], props["outcome"]["const"], "{what}");
    if let Some(e) = props.get("error") {
        assert_eq!(body["error"], e["const"], "{what}");
        check_object(&format!("{what} payload"), resolve(&props["payload"]), &body["payload"]);
    }
    let published = body["published"].as_array().unwrap();
    if props["published"]["maxItems"] == 0 {
        assert!(published.is_empty(), "{what}");
    }
    for (i, p) in props["published"]["prefixItems"].as_array().into_iter().flatten().enumerate() {
        assert_eq!(published[i]["event"], p["properties"]["event"]["const"], "{what}");
        check_object(&format!("{what} event"), resolve(&p["properties"]["payload"]), &published[i]["payload"]);
    }
}

/// One row of a view is the document's row schema.
pub fn check_row(view_schema: &str, row: &Value) {
    let s = schema(&format!("sdc.daemon.{view_schema}.Row"));
    let obj = row.as_object().unwrap();
    for r in s["required"].as_array().into_iter().flatten() {
        assert!(obj.contains_key(r.as_str().unwrap()), "{view_schema}: missing {r} in {row}");
    }
    let props = s["properties"].as_object().unwrap();
    for k in obj.keys() {
        assert!(props.contains_key(k), "{view_schema}: the document has no member {k}: {row}");
    }
}
