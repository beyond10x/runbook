//! `runbook run --model-route <name>` (`docs/model-routes.md`, sections 2 and 3): the route of `runbook.toml` the
//! run's model programs take. The option names it, else `[route_policy] default`; with neither the settings stay as
//! they are.
//!
//! [`take`] runs before the `[gateway]` setting is read (`gateway.rs`) and nothing is written before it: it refuses a
//! name the file does not declare (naming the declared ones), a route beside a gateway or a login file of the
//! settings, and a route that cannot answer now: its gateway is asked `GET /ready`, `GET /v1/models` and the status
//! of the route's model, and nothing else. `runbook run` does not wait for a route: the wait is the daemon's
//! (`waiting-for-route`). A route that answers becomes the run's `[gateway]` (its gateway's address and token) and
//! its alias the model of every role and of a model decider, so that `gateway.rs` takes it as it takes a named
//! gateway and nothing after it holds the token.
//!
//! Every refusal names the route, the setting that chose it and what is wrong, never the address or the token.

use super::{Args, Confinement};
use runbook::routes::{Readiness, READ_BOUND};
use serde_json::{json, Map, Value};
use std::time::Instant;

/// The option that names the route.
pub const OPTION: &str = "--model-route";
/// The member of `runbook.toml` that names the route when the option does not.
pub const DEFAULT: &str = "route_policy.default";

/// The settings of the run on its route: see the module's text. With `--dry` a declared name is checked and nothing
/// is read or changed.
pub fn take(args: &Args, mut settings: Value) -> Result<Value, String> {
    let (name, chosen_by) = match (&args.model_route, args.routes.default_route()) {
        (Some(name), _) => (name.as_str(), OPTION),
        (None, Some(name)) => (name, DEFAULT),
        (None, None) => return Ok(settings),
    };
    let names = args.routes.names();
    if !names.contains(&name) {
        let declared = if names.is_empty() { "runbook.toml declares no route".to_owned() } else { format!("the routes of runbook.toml are {}", names.join(", ")) };
        return Err(format!("{chosen_by}: no route of that name ({declared})"));
    }
    let beside = [super::gateway::KEY, super::settings::LOGIN_KEY].into_iter().find(|k| settings.get(*k).is_some());
    if let Some(key) = beside {
        if chosen_by == DEFAULT {
            // the settings name their own way to a model: the default of the file does not replace it
            return Ok(settings);
        }
        return Err(format!("{key} and {OPTION}: the settings name a {} and the option a route; give one", if key == super::gateway::KEY { "gateway" } else { "login file" }));
    }
    if args.ports.dry {
        return Ok(settings);
    }
    if args.confinement != Confinement::Local {
        return Err(format!("route {name} (chosen by {chosen_by}): not available at --confinement {}: a route works at --confinement local only (docs/loom-run-settings.md, \"[gateway]\")", args.confinement.as_arg()));
    }
    let route = args.routes.route(name).ok_or_else(|| format!("{chosen_by}: no route of that name"))?.clone();
    // a model of the settings that is not the alias is refused before the gateway is read
    set_models(&mut settings, &route.alias, name)?;
    let mut routes = args.routes.clone();
    routes.read_token_of(&route.gateway).map_err(|why| format!("route {name} (chosen by {chosen_by}): {why}"))?;
    let refuse = |what: String| format!("route {name} (chosen by {chosen_by}): {what}; runbook run does not wait for a route (the daemon does), name another with {OPTION}");
    match routes.readiness(name, Instant::now() + READ_BOUND) {
        Some(Readiness::Callable) => {}
        Some(Readiness::Unreachable) | None => return Err(refuse(format!("the gateway {} does not answer", route.gateway))),
        Some(Readiness::NotReady) => return Err(refuse(format!("gateway not ready (the gateway {} answered GET /ready, not 200)", route.gateway))),
        Some(Readiness::Unlisted) => return Err(refuse(format!("the gateway {} serves no model {}", route.gateway, route.alias))),
        Some(Readiness::NotCallable { phase }) => return Err(refuse(format!("the model {} is not callable (phase {})", route.alias, phase.as_deref().unwrap_or("not given")))),
    }
    let gateway = routes.gateway(&route.gateway).ok_or_else(|| refuse("its gateway is not declared".to_owned()))?;
    let mut table = Map::new();
    table.insert("base_url".into(), json!(gateway.base_url()));
    if let Some(token) = gateway.token() {
        table.insert("token".into(), json!(token));
    }
    settings[super::gateway::KEY] = Value::Object(table);
    Ok(settings)
}

/// The alias as the model of every role and of a model decider; a model the settings already name and that is not
/// the alias is refused by its member.
fn set_models(settings: &mut Value, alias: &str, name: &str) -> Result<(), String> {
    let differs = |at: String, v: Option<&Value>| match v {
        Some(Value::String(m)) if m == alias => Ok(()),
        None => Ok(()),
        Some(_) => Err(format!("{at} and {OPTION}: the settings name a model and the route {name} names its own; give one")),
    };
    if !settings["models"].is_object() {
        settings["models"] = json!({});
    }
    for role in super::bindings_copy::ROLES {
        let models = &mut settings["models"];
        if !models[role].is_object() {
            models[role] = json!({});
        }
        differs(format!("models.{role}.model"), models[role].get("model"))?;
        models[role]["model"] = json!(alias);
    }
    if settings["decider"]["kind"].as_str() == Some("model") {
        differs("decider.model".to_owned(), settings["decider"].get("model"))?;
        settings["decider"]["model"] = json!(alias);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_alias_becomes_the_model_of_every_role_and_of_a_model_decider_and_another_model_is_refused_by_member() {
        let mut s = json!({"decider": {"kind": "model"}, "models": {"writer": {"max_turns": 3}}});
        set_models(&mut s, "qwen-alias", "qwen").unwrap();
        for role in super::super::bindings_copy::ROLES {
            assert_eq!(s["models"][role]["model"], "qwen-alias", "{s}");
        }
        assert_eq!((s["models"]["writer"]["max_turns"].clone(), s["decider"]["model"].clone()), (json!(3), json!("qwen-alias")));
        let mut s = json!({"decider": {"kind": "rule"}, "models": {"review": {"model": "another"}}});
        let e = set_models(&mut s, "qwen-alias", "qwen").unwrap_err();
        assert!(e.starts_with("models.review.model and --model-route") && !e.contains("another"), "{e}");
        let mut s = json!({"decider": {"kind": "rule"}});
        set_models(&mut s, "qwen-alias", "qwen").unwrap();
        assert!(s["decider"].get("model").is_none(), "{s}");
    }
}
