//! A delegation that names no protocol, on a daemon with a classifier route (`docs/model-routes.md`, section 9;
//! `crate::delegation`). Before its run starts, one call on the classifier route through the daemon's relay picks a
//! runbook of the daemon with a confidence; at or above the threshold that runbook runs, below it a second call writes a
//! protocol of the daemon's parts, which the loader of a runbook checks before it runs.
//!
//! The calls are the run's: the run is on the relay as the role `classifier` for them, each is taken against the run's
//! caps and booked in its ledger like any call (`gate::RelayBooking`), and each is an event `runbook.model-call` of the
//! run. A call the route does not serve ends the start `route-unavailable: ...`, an answer that is not the JSON asked for
//! (after two more calls that quote the validator's error) `classifier-answer: ...`, a generated protocol the loader
//! refuses `protocol-refused: <the loader's line>`. The path taken is the run event `runbook.delegation-resolved` and the
//! file `delegation.json` of the run's directory, read again when the run goes on from its store.
//!
//! A generated protocol runs only the daemon's agent node (`delegation::bind_built`): the builder writes the protocol,
//! the artifacts and the evidence, never a program or its arguments; its programs run at the run's confinement level, as
//! every runbook's.

use super::gate::{Gate, Meter};
use super::{calls, layout, Engine};
use crate::caps::Caps;
use crate::client::http::{Client, Token};
use crate::delegation::{self as dlg, Built, CatalogEntry};
use crate::relay::{RouteTable, Ticket, ROUTE_UNAVAILABLE};
use crate::runtime::{EventSink, StartRequest};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a classify or build call waits for the next bytes of its answer.
const CALL_WAIT: Duration = Duration::from_secs(900);
const MAX_TOKENS_CLASSIFY: u64 = 1024;
const MAX_TOKENS_BUILD: u64 = 8192;
/// The setting that chose the route of the calls, as the ledger line names it.
const SOURCE: &str = "route_policy.classifier";
/// Why no further call went out: the daemon cancelled the run (the run ends `Cancelled`, not on this text).
const CANCELLED: &str = "cancelled: the daemon cancelled the run before this call";

/// The path a delegation took: the runbook its run runs and the home it is read from.
pub struct Taken {
    pub runbook: String,
    pub home: PathBuf,
    /// The run's agent node reads the request whole (the free run's and a generated protocol's), not as an idea document.
    pub whole_text: bool,
    /// A generated protocol: a refusal when its tree is opened is the loader's too.
    pub generated: bool,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The path of the delegation `request`, taken now with the classify and build calls, or read from the run's directory when
/// the run goes on from its store. `meter` is the run's: the calls count against its caps.
pub fn resolve(engine: &Engine, request: &StartRequest, sink: &dyn EventSink, caps: Caps, workspace: &Path, meter: &Arc<Meter>, cancelled: &dyn Fn() -> bool) -> Result<Taken, String> {
    let file = request.state_dir.join(dlg::RESOLVED_FILE);
    if let Ok(text) = std::fs::read_to_string(&file) {
        let record: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", dlg::RESOLVED_FILE))?;
        return taken_of(engine, request, &record);
    }
    let route = engine.classifier.clone().ok_or("the daemon has no classifier route ([route_policy] classifier)")?;
    let relay = engine.relay.as_ref().ok_or("the engine has no relay of the daemon to take the classifier's calls")?;
    let classifier = route.name.clone();
    let ticket = relay.open(&request.run_id, RouteTable::default().with(dlg::CLASSIFIER_ROLE, route, SOURCE))?;
    let mut gate = Gate::open(caps, &engine.state_dir, &request.runbook, &request.run_id, &request.state_dir, workspace, false, meter.clone(), crate::budget::Reserves::default())?;
    let step = Arc::new(Mutex::new(Some("delegation.classify".to_owned())));
    let booking = gate.route(step.clone());
    // a call of unknown cost books what one such call may cost, never all the cap has left (docs/model-routes.md, section 5)
    booking.set_max_call(dlg::CALL_RESERVATION_MICRO_USD);
    ticket.take_calls(booking);
    let caller = Caller::new(&ticket, &classifier, cancelled)?;
    let mut record = json!({"classifier_route": classifier});
    let taken = take(engine, request, workspace, &caller, &step, &mut record);
    // the token is void before the calls are recorded: no call of the run is on the relay any more
    drop(ticket);
    gate.record_calls(sink);
    gate.persist();
    if record.get("path").is_some() {
        sink.record(dlg::RESOLVED_EVENT, record.clone());
    }
    let taken = taken?;
    std::fs::write(&file, record.to_string()).map_err(|e| format!("{}: {e}", dlg::RESOLVED_FILE))?;
    Ok(taken)
}

/// The path a record of [`dlg::RESOLVED_EVENT`] names.
fn taken_of(engine: &Engine, request: &StartRequest, record: &Value) -> Result<Taken, String> {
    let protocol = record["protocol"].as_str().unwrap_or("");
    match record["path"].as_str() {
        Some("classified") => Ok(Taken { runbook: protocol.to_owned(), home: layout::home_of(&engine.homes, protocol)?, whole_text: false, generated: false }),
        Some("generated") => Ok(Taken { runbook: dlg::BUILT_RUNBOOK.to_owned(), home: request.state_dir.join(dlg::BUILT_DIR), whole_text: true, generated: true }),
        Some("free") => Ok(Taken { runbook: dlg::FREE_RUNBOOK.to_owned(), home: engine.free_home.clone().ok_or("the engine runs no free run")?, whole_text: true, generated: false }),
        _ => Err(format!("{} names no path", dlg::RESOLVED_FILE)),
    }
}

/// The classify call, and below the threshold the build call and the loader; `record` takes what is known as it is known.
fn take(engine: &Engine, request: &StartRequest, workspace: &Path, caller: &Caller<'_>, step: &Mutex<Option<String>>, record: &mut Value) -> Result<Taken, String> {
    let catalog = catalog_of(engine);
    let products = workspace.join(dlg::PRODUCTS_DIR);
    let classified = ask(caller, &dlg::classify_prompt(&request.intake, &catalog), MAX_TOKENS_CLASSIFY, |t| dlg::read_classified(t, &catalog))?;
    put(&products, "classify.json", &(serde_json::to_string_pretty(&json!({"protocol": classified.protocol, "confidence": classified.confidence})).unwrap_or_default() + "\n"))?;
    record["confidence"] = json!(classified.confidence);
    if let Some(id) = classified.protocol.filter(|_| classified.confidence >= dlg::CLASSIFY_THRESHOLD) {
        record["path"] = json!("classified");
        record["protocol"] = json!(id);
        let home = layout::home_of(&engine.homes, &id)?;
        return Ok(Taken { runbook: id, home, whole_text: false, generated: false });
    }
    *lock(step) = Some("delegation.build".to_owned());
    let built = ask(caller, &dlg::build_prompt(&request.intake, &catalog), MAX_TOKENS_BUILD, dlg::read_built)?;
    let free_home = engine.free_home.clone().ok_or("the engine runs no free run, whose agent node a generated protocol runs")?;
    let (protocol, bindings) = match built {
        Built::Free => {
            record["path"] = json!("free");
            record["protocol"] = json!(dlg::FREE_RUNBOOK);
            return Ok(Taken { runbook: dlg::FREE_RUNBOOK.to_owned(), home: free_home, whole_text: true, generated: false });
        }
        Built::Protocol { protocol, bindings } => (protocol, bindings),
    };
    record["path"] = json!("generated");
    record["protocol_digest"] = json!(dlg::digest(protocol.as_bytes()));
    put(&products, "protocol.yaml", &protocol)?;
    put(&products, "bindings.yaml", &bindings)?;
    let refused = |why: String| format!("protocol-refused: {why}");
    let (id, revision, registration) = dlg::protocol_of(&protocol).map_err(refused)?;
    record["protocol"] = json!(id);
    let free_bindings = std::fs::read_to_string(free_home.join("bindings").join(format!("{}.yaml", dlg::FREE_RUNBOOK))).map_err(|e| format!("the free run's bindings: {e}"))?;
    let bound = dlg::bind_built(&registration, &bindings, &free_bindings).map_err(refused)?;
    let home = request.state_dir.join(dlg::BUILT_DIR);
    put(&home.join("protocol"), &format!("{}.protocol.yaml", dlg::BUILT_RUNBOOK), &protocol)?;
    put(&home.join("bindings"), &format!("{}.yaml", dlg::BUILT_RUNBOOK), &bound)?;
    // the loader of a shipped runbook: the layout, the budgets of the bindings, Loom's reader of the protocol directory
    let found = layout::resolve(&home, dlg::BUILT_RUNBOOK, None).map_err(refused)?;
    crate::budget::reserves(&found.bindings).map_err(refused)?;
    calls::check_protocols(&found.protocol_dir, &id, revision).map_err(refused)?;
    Ok(Taken { runbook: dlg::BUILT_RUNBOOK.to_owned(), home, whole_text: true, generated: true })
}

/// Writes `text` to `dir/name`, the directory made first.
fn put(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(name), text).map_err(|e| format!("{}: {e}", dir.join(name).display()))
}

/// The runbooks a person can start from the engine's homes, each with its title, description and inputs.
fn catalog_of(engine: &Engine) -> Vec<CatalogEntry> {
    use crate::runtime::Runtime;
    let homes = &engine.homes;
    let names: BTreeSet<String> = layout::list_in(homes).into_iter().map(|r| r.name).collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for home in homes {
        for entry in crate::runbooks::read_protocols(&home.join("protocol")).unwrap_or_default() {
            if !entry.startable || !names.contains(&entry.id) || !seen.insert(entry.id.clone()) {
                continue;
            }
            let file = home.join("protocol").join(format!("{}.protocol.yaml", entry.id));
            let doc: serde_yaml::Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_yaml::from_str(&t).ok()).unwrap_or_default();
            out.push(CatalogEntry::of(&entry.id, &doc, engine.runbook_facts(&entry.id).inputs));
        }
    }
    out
}

/// One call and up to [`dlg::RETRIES`] more, each with the answer before it and the validator's error, until `read` takes
/// an answer.
fn ask<T>(caller: &Caller<'_>, prompt: &str, max_tokens: u64, read: impl Fn(&str) -> Result<T, String>) -> Result<T, String> {
    let mut messages = vec![json!({"role": "user", "content": prompt})];
    let mut last = String::new();
    for _ in 0..=dlg::RETRIES {
        let text = caller.call(&messages, max_tokens)?;
        match read(&text) {
            Ok(v) => return Ok(v),
            Err(why) => {
                let said = if text.trim().is_empty() { "(no text)".to_owned() } else { text };
                messages.push(json!({"role": "assistant", "content": said}));
                messages.push(json!({"role": "user", "content": dlg::retry_prompt(&why)}));
                last = why;
            }
        }
    }
    Err(format!("classifier-answer: the classifier route answered no JSON of the shape asked for in {} calls: {last}", dlg::RETRIES + 1))
}

/// The run's side of its relay for the classify and build calls: a Messages call as the role `classifier`.
struct Caller<'a> {
    client: Client,
    path: String,
    route: String,
    /// Whether the daemon cancelled the run: asked before every call, so no call goes out after a cancel.
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Caller<'a> {
    fn new(ticket: &Ticket, route: &str, cancelled: &'a dyn Fn() -> bool) -> Result<Caller<'a>, String> {
        let rest = ticket.base_url().strip_prefix("http://").ok_or("the relay's address is not http://")?;
        let (authority, prefix) = rest.split_once('/').ok_or("the relay's address has no path")?;
        let target = crate::client::url::parse(&format!("http://{authority}")).map_err(|f| f.message)?;
        Ok(Caller { client: Client::new(target, Some(Token::new(ticket.token().to_owned()))), path: format!("/{prefix}/v1/messages"), route: route.to_owned(), cancelled })
    }

    /// The text of the answer to `messages`; `Err` names the cause, the route and what the relay said.
    fn call(&self, messages: &[Value], max_tokens: u64) -> Result<String, String> {
        if (self.cancelled)() {
            return Err(CANCELLED.to_owned());
        }
        let body = json!({"model": dlg::CLASSIFIER_ROLE, "max_tokens": max_tokens, "messages": messages}).to_string();
        let reply = self
            .client
            .request_waiting("POST", &self.path, Some(body.as_bytes()), CALL_WAIT)
            .map_err(|f| format!("{ROUTE_UNAVAILABLE}: route {} (chosen by {SOURCE}): the relay did not answer: {}", self.route, f.message))?;
        let v: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
        if !reply.ok() {
            let code = v["error"]["code"].as_str().or_else(|| v["error"]["type"].as_str()).unwrap_or("");
            let cause = if reply.status == 429 { crate::relay::CAP_REACHED } else { ROUTE_UNAVAILABLE };
            return Err(format!("{cause}: route {} (chosen by {SOURCE}) answered {} {code}: {}", self.route, reply.status, v["error"]["message"].as_str().unwrap_or("")));
        }
        Ok(v["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "text").filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join(""))
    }
}
