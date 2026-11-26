//! The model routes a daemon declares (`docs/model-routes.md`, section 1; `docs/daemon-api.md`, "Model routes").
//!
//! The daemon's `runbook.toml` declares its gateways (`[gateways.<name>]`: an address and a token file), its routes
//! (`[routes.<name>]`: a gateway by name, the gateway's alias of a model, the kind, and optional prices and wait) and
//! its policy (`[route_policy]`: the route of a run that names none and the route of a delegation's classify calls).
//! A lone `[gateway]` table is the gateway `default`.
//!
//! [`RouteTable::declare`] checks the tables and reads nothing; [`RouteTable::read_tokens`] reads the token files
//! once, when the daemon starts. Every refusal names the key (`routes.qwen.gateway`) and never a value, a part of a
//! value or a path. [`Listing`] is the view `routes`: what the gateway says of each route, read at most once every
//! [`READ_EVERY_MILLIS`] of the daemon's clock with `GET` requests only, in the background and within
//! [`READ_BOUND`]; a read sends no model call and starts nothing. [`RouteTable::wake`] is the one request that may
//! start a model: the daemon sends it only for a managed route whose wake is on, booked against its daily cap.

use crate::client::http::{Client, Token};
use crate::client::url::Target;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The tables of `runbook.toml`.
pub const GATEWAYS: &str = "gateways";
pub const ROUTES: &str = "routes";
pub const POLICY: &str = "route_policy";
/// The one gateway of a file before routes: read as the gateway [`DEFAULT_GATEWAY`].
pub const LONE_GATEWAY: &str = "gateway";
pub const DEFAULT_GATEWAY: &str = "default";
/// How long the rows of a read of the gateways are answered before the gateways are read again.
pub const READ_EVERY_MILLIS: i64 = 15_000;
/// How long one `GET` of a gateway may take, its whole answer included.
const GET_BOUND: Duration = Duration::from_secs(3);
/// How long one read of every gateway may take: a `GET` still open then ends as no answer.
pub const READ_BOUND: Duration = Duration::from_secs(5);
/// What the first request waits beyond [`READ_BOUND`] for the read's rows to reach the cache.
const WAIT_SLACK: Duration = Duration::from_millis(250);
/// The most bytes of a gateway's token (`sdc_tools::gateway::TOKEN_LIMIT`).
const TOKEN_LIMIT: usize = sdc_tools::gateway::TOKEN_LIMIT;

const GATEWAY_MEMBERS: [&str; 2] = ["base_url", "token_file"];
const ROUTE_MEMBERS: [&str; 9] = ["gateway", "model", "kind", "start_wait_seconds", "price_in_micro_usd_per_mtok", "price_out_micro_usd_per_mtok", "wake_on_submit", "wake_daily_cap_micro_usd", "wake_cost_micro_usd"];
const WAKE_MEMBERS: [&str; 3] = ["wake_on_submit", "wake_daily_cap_micro_usd", "wake_cost_micro_usd"];
const POLICY_MEMBERS: [&str; 2] = ["default", "classifier"];
/// The members a route never has: what reaches a model is its gateway's.
const ROUTE_REFUSED: [&str; 4] = ["token", "token_file", "base_url", "api_key"];

/// What stands behind a route's alias at its gateway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    /// A model on a pod the gateway starts and stops.
    Managed,
    /// A fixed upstream: nothing is started.
    Hosted,
    /// A model server on the gateway's machine (Ollama).
    Local,
}

impl RouteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteKind::Managed => "managed",
            RouteKind::Hosted => "hosted",
            RouteKind::Local => "local",
        }
    }

    fn parse(text: &str) -> Option<RouteKind> {
        match text {
            "managed" => Some(RouteKind::Managed),
            "hosted" => Some(RouteKind::Hosted),
            "local" => Some(RouteKind::Local),
            _ => None,
        }
    }

    /// Whether the gateway has a status of the model that is not its own process: a pod or a local server.
    fn has_model_status(self) -> bool {
        self != RouteKind::Hosted
    }
}

/// What the ledger books per million tokens of a call on the route; none declared: the cost is not known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prices {
    pub in_micro_usd_per_mtok: Option<u64>,
    pub out_micro_usd_per_mtok: Option<u64>,
}

/// The wake keys of a managed route (`docs/model-routes.md`, section 3 point 4). Off unless the file sets
/// `wake_on_submit = true`, which is refused at start unless the daily cap and the cost of a wake are both above 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wake {
    pub on_submit: bool,
    pub daily_cap_micro_usd: Option<u64>,
    pub cost_micro_usd: Option<u64>,
}

/// A gateway: an address and, once [`RouteTable::read_tokens`] ran, the token of its token file. The token is
/// read through [`Gateway::token`]; neither it nor the address is in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Gateway {
    name: String,
    /// The table the gateway was declared in, for a refusal: `gateways.<name>`, or `gateway` for a lone table.
    member: String,
    base_url: String,
    token_file: Option<String>,
    token: Option<String>,
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = match (&self.token, &self.token_file) {
            (Some(_), _) => "<read>",
            (None, Some(_)) => "<named>",
            (None, None) => "<none>",
        };
        f.debug_struct("Gateway").field("name", &self.name).field("token", &token).finish_non_exhaustive()
    }
}

impl Gateway {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The token of the gateway's token file; none for a gateway without one, and before the file was read.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

/// A declared route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub name: String,
    /// The name of its gateway.
    pub gateway: String,
    /// The gateway's alias of the model: what `GET /v1/models` lists and a call names.
    pub alias: String,
    pub kind: RouteKind,
    /// How long a run waits for the route's model (`docs/model-routes.md`, section 3); none declared: none set.
    pub start_wait_seconds: Option<u64>,
    pub prices: Prices,
    pub wake: Wake,
}

/// What a route name resolves to: the address of its gateway, the gateway's token, the alias, the kind and the
/// prices. Its `Debug` holds no token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Resolved<'a> {
    pub route: &'a str,
    pub base_url: &'a str,
    pub token: Option<&'a str>,
    pub alias: &'a str,
    pub kind: RouteKind,
    pub prices: Prices,
}

impl std::fmt::Debug for Resolved<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = if self.token.is_some() { "<read>" } else { "<none>" };
        f.debug_struct("Resolved").field("route", &self.route).field("base_url", &self.base_url).field("token", &token).field("alias", &self.alias).field("kind", &self.kind).field("prices", &self.prices).finish()
    }
}

/// The tables of a file as read (TOML read into JSON values), each absent when the file has none. A relative
/// `token_file` is made absolute against the file before this (`config.rs`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Tables<'a> {
    pub gateway: Option<&'a Value>,
    pub gateways: Option<&'a Value>,
    pub routes: Option<&'a Value>,
    pub route_policy: Option<&'a Value>,
}

/// The gateways, the routes and the policy a daemon declares. Empty: no route.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteTable {
    gateways: BTreeMap<String, Gateway>,
    routes: BTreeMap<String, Route>,
    default: Option<String>,
    classifier: Option<String>,
    /// Whether the route `default` is the interim route of a lone `[gateway]` ([`RouteTable::with_interim_default`]).
    interim: bool,
}

/// A name of a gateway or a route: 1 to 64 letters, digits, `.`, `_` and `-`.
pub fn is_name(text: &str) -> bool {
    !text.is_empty() && text.len() <= 64 && text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// An alias of a gateway: 1 to 256 bytes of visible ASCII (the gateway's own bound), and no address.
fn is_alias(text: &str) -> bool {
    !text.is_empty() && text.len() <= 256 && text.bytes().all(|b| b.is_ascii_graphic()) && !text.contains("://")
}

/// A table of `[gateways]` or `[routes]` with its name.
type Named<'a> = (&'a String, &'a Map<String, Value>);

/// The tables of `table` (`gateways`, `routes`), each a table; a name that is not one is refused without the name.
fn tables_of<'a>(value: &'a Value, key: &str, what: &str) -> Result<Vec<Named<'a>>, String> {
    let Some(map) = value.as_object() else {
        return Err(format!("{key}: a table of tables, one per {what} ([{key}.<name>])"));
    };
    map.iter()
        .map(|(name, v)| {
            if !is_name(name) {
                return Err(format!("{key}: the name of a {what} is 1 to 64 letters, digits, '.', '_' or '-'"));
            }
            v.as_object().map(|t| (name, t)).ok_or_else(|| format!("{key}.{name}: a table"))
        })
        .collect()
}

fn whole(table: &Map<String, Value>, at: &str, key: &str) -> Result<Option<u64>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("{at}.{key}: a whole number of 0 or more")),
    }
}

fn gateway_of(name: &str, member: &str, table: &Map<String, Value>) -> Result<Gateway, String> {
    for key in table.keys() {
        if key == "token" {
            return Err(format!("{member}.token: not a key of runbook.toml (the file holds no token: name {member}.token_file)"));
        }
        if !GATEWAY_MEMBERS.contains(&key.as_str()) {
            return Err(format!("{member}.{key}: not a member of a gateway (known: {})", GATEWAY_MEMBERS.join(", ")));
        }
    }
    let base_url = match table.get("base_url") {
        None => return Err(format!("{member}.base_url: required")),
        Some(Value::String(url)) => {
            sdc_tools::gateway::check_base_url(url).map_err(|why| format!("{member}.base_url: {why}"))?;
            url.clone()
        }
        Some(_) => return Err(format!("{member}.base_url: text")),
    };
    let token_file = match table.get("token_file") {
        None => None,
        Some(Value::String(path)) if Path::new(path).is_absolute() => Some(path.clone()),
        Some(Value::String(_)) => return Err(format!("{member}.token_file: not an absolute path")),
        Some(_) => return Err(format!("{member}.token_file: a path as text")),
    };
    Ok(Gateway { name: name.to_owned(), member: member.to_owned(), base_url, token_file, token: None })
}

fn declared(names: impl Iterator<Item = impl AsRef<str>>) -> String {
    let names: Vec<String> = names.map(|n| n.as_ref().to_owned()).collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

fn route_of(name: &str, table: &Map<String, Value>, gateways: &BTreeMap<String, Gateway>) -> Result<Route, String> {
    let at = format!("{ROUTES}.{name}");
    for key in table.keys() {
        if ROUTE_REFUSED.contains(&key.as_str()) {
            return Err(format!("{at}.{key}: a route holds no address, token or key: those are its gateway's ([{GATEWAYS}.<name>])"));
        }
        if !ROUTE_MEMBERS.contains(&key.as_str()) {
            return Err(format!("{at}.{key}: not a member of a route (known: {})", ROUTE_MEMBERS.join(", ")));
        }
    }
    let gateway = match table.get("gateway") {
        None => return Err(format!("{at}.gateway: required (the name of a [{GATEWAYS}.<name>] table)")),
        Some(Value::String(g)) if gateways.contains_key(g) => g.clone(),
        Some(_) => return Err(format!("{at}.gateway: names no declared gateway (declared: {})", declared(gateways.keys()))),
    };
    let alias = match table.get("model") {
        None => return Err(format!("{at}.model: required (the gateway's alias of the model, as its /v1/models lists it)")),
        Some(Value::String(a)) if is_alias(a) => a.clone(),
        Some(_) => return Err(format!("{at}.model: the gateway's alias of a model (1 to 256 visible ASCII characters), never an address")),
    };
    let kind = match table.get("kind") {
        None => return Err(format!("{at}.kind: required (managed, hosted or local)")),
        Some(Value::String(k)) => RouteKind::parse(k).ok_or_else(|| format!("{at}.kind: one of managed, hosted, local"))?,
        Some(_) => return Err(format!("{at}.kind: one of managed, hosted, local")),
    };
    if kind != RouteKind::Managed {
        if let Some(key) = WAKE_MEMBERS.iter().find(|k| table.contains_key(**k)) {
            return Err(format!("{at}.{key}: a key of a managed route only"));
        }
    }
    let on_submit = match table.get("wake_on_submit") {
        None | Some(Value::Bool(false)) => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return Err(format!("{at}.wake_on_submit: true or false")),
    };
    let wake = Wake { on_submit, daily_cap_micro_usd: whole(table, &at, "wake_daily_cap_micro_usd")?, cost_micro_usd: whole(table, &at, "wake_cost_micro_usd")? };
    if on_submit {
        // the wake ships off: it turns on only behind a daily cap, and every wake books against it
        if !wake.daily_cap_micro_usd.is_some_and(|c| c > 0) {
            return Err(format!("{at}.wake_on_submit: true needs {at}.wake_daily_cap_micro_usd above 0 (the most the route's wakes may book per day, docs/model-routes.md section 3)"));
        }
        if !wake.cost_micro_usd.is_some_and(|c| c > 0) {
            return Err(format!("{at}.wake_on_submit: true needs {at}.wake_cost_micro_usd above 0 (what one wake books against the day's cap, docs/model-routes.md section 3)"));
        }
    }
    Ok(Route {
        name: name.to_owned(),
        gateway,
        alias,
        kind,
        start_wait_seconds: whole(table, &at, "start_wait_seconds")?,
        prices: Prices { in_micro_usd_per_mtok: whole(table, &at, "price_in_micro_usd_per_mtok")?, out_micro_usd_per_mtok: whole(table, &at, "price_out_micro_usd_per_mtok")? },
        wake,
    })
}

/// The token of the file `path`: one line, an end of line after it allowed. The file is opened once, without
/// following a link and without blocking (a FIFO opens at once and is no regular file), the kind is asked of the
/// open descriptor, and it is read up to a limit. An error names `member` and what is wrong, never the path or the
/// content.
pub fn read_token_file(path: &str, member: &str) -> Result<String, String> {
    let unreadable = || format!("{member}: not a readable file");
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC).open(path).map_err(|_| unreadable())?;
    if !file.metadata().map_err(|_| unreadable())?.is_file() {
        return Err(unreadable());
    }
    let mut text = String::new();
    file.take(TOKEN_LIMIT as u64 + 16).read_to_string(&mut text).map_err(|_| format!("{member}: not readable as text"))?;
    if text.trim().is_empty() {
        return Err(format!("{member}: empty"));
    }
    let line = text.strip_suffix('\n').map(|t| t.strip_suffix('\r').unwrap_or(t)).unwrap_or(&text);
    if line.contains('\n') || line.contains('\r') {
        return Err(format!("{member}: not one line"));
    }
    sdc_tools::gateway::check_token(line).map_err(|why| format!("{member}: the token {why}"))?;
    Ok(line.to_owned())
}

impl RouteTable {
    /// The table of a file's tables: every key checked, nothing read. A refusal names the key, never a value.
    pub fn declare(tables: &Tables) -> Result<RouteTable, String> {
        let mut gateways = BTreeMap::new();
        if let Some(lone) = tables.gateway {
            if tables.gateways.and_then(|g| g.get(DEFAULT_GATEWAY)).is_some() {
                return Err(format!("{LONE_GATEWAY} and {GATEWAYS}.{DEFAULT_GATEWAY}: one or the other (a lone [{LONE_GATEWAY}] table is the gateway {DEFAULT_GATEWAY})"));
            }
            let table = lone.as_object().ok_or_else(|| format!("{LONE_GATEWAY}: a table with base_url and token_file"))?;
            gateways.insert(DEFAULT_GATEWAY.to_owned(), gateway_of(DEFAULT_GATEWAY, LONE_GATEWAY, table)?);
        }
        if let Some(value) = tables.gateways {
            for (name, table) in tables_of(value, GATEWAYS, "gateway")? {
                gateways.insert(name.clone(), gateway_of(name, &format!("{GATEWAYS}.{name}"), table)?);
            }
        }
        let mut routes = BTreeMap::new();
        if let Some(value) = tables.routes {
            for (name, table) in tables_of(value, ROUTES, "route")? {
                routes.insert(name.clone(), route_of(name, table, &gateways)?);
            }
        }
        let (mut default, mut classifier) = (None, None);
        if let Some(value) = tables.route_policy {
            let table = value.as_object().ok_or_else(|| format!("{POLICY}: a table with default and classifier"))?;
            for (key, v) in table {
                let slot = match key.as_str() {
                    "default" => &mut default,
                    "classifier" => &mut classifier,
                    _ => return Err(format!("{POLICY}.{key}: not a member of the policy (known: {})", POLICY_MEMBERS.join(", "))),
                };
                match v {
                    Value::String(route) if routes.contains_key(route) => *slot = Some(route.clone()),
                    _ => return Err(format!("{POLICY}.{key}: names no declared route (declared: {})", declared(routes.keys()))),
                }
            }
        }
        Ok(RouteTable { gateways, routes, default, classifier, interim: false })
    }

    /// Reads the token file of every gateway that names one, once: every `[gateways.<name>]`, and a lone `[gateway]`
    /// only when a route names it (without one it is the model decider's alone, which reads it itself, as before
    /// routes). A refusal names the member, never the path or the content.
    pub fn read_tokens(&mut self) -> Result<(), String> {
        let routes = &self.routes;
        for gateway in self.gateways.values_mut() {
            let lone_and_unused = gateway.member == LONE_GATEWAY && !routes.values().any(|r| r.gateway == gateway.name);
            if lone_and_unused {
                continue;
            }
            if let Some(path) = &gateway.token_file {
                gateway.token = Some(read_token_file(path, &format!("{}.token_file", gateway.member))?);
            }
        }
        Ok(())
    }

    /// The table of a daemon that declares no route and has a lone `[gateway]` (the gateway `default`) and a model
    /// decider: the interim route `default` on that gateway, kind hosted, with `alias` (the decider's model) and no
    /// price, and the policy's default on it, so that the route a model run takes is one the view `routes` lists.
    /// Unchanged when the table declares a route or has no gateway `default`.
    pub fn with_interim_default(mut self, alias: &str) -> Result<RouteTable, String> {
        if !self.routes.is_empty() || !self.gateways.contains_key(DEFAULT_GATEWAY) {
            return Ok(self);
        }
        let route = Route {
            name: DEFAULT_GATEWAY.to_owned(),
            gateway: DEFAULT_GATEWAY.to_owned(),
            alias: alias.to_owned(),
            kind: RouteKind::Hosted,
            start_wait_seconds: None,
            prices: Prices::default(),
            wake: Wake::default(),
        };
        self.routes.insert(DEFAULT_GATEWAY.to_owned(), route);
        self.default = Some(DEFAULT_GATEWAY.to_owned());
        self.interim = true;
        self.read_token_of(DEFAULT_GATEWAY)?;
        Ok(self)
    }

    /// Whether `name` is the interim route of a lone `[gateway]`, which a run takes as before routes: its gateway is not
    /// asked whether it can answer before the run starts.
    pub fn is_interim(&self, name: &str) -> bool {
        self.interim && name == DEFAULT_GATEWAY
    }

    /// Reads the token file of the gateway `name` alone, when it names one: `runbook run` reads the gateway of the one
    /// route it takes. A refusal names the member, never the path or the content.
    pub fn read_token_of(&mut self, name: &str) -> Result<(), String> {
        if let Some(gateway) = self.gateways.get_mut(name) {
            if let Some(path) = &gateway.token_file {
                gateway.token = Some(read_token_file(path, &format!("{}.token_file", gateway.member))?);
            }
        }
        Ok(())
    }

    /// What the gateway of the route `name` says of it now (`docs/model-routes.md`, section 3): `GET /ready`, `GET
    /// /v1/models` and the status of the route's model, every request done by `until`. None for a name that is not
    /// declared. A read sends no model call and starts nothing.
    pub fn readiness(&self, name: &str, until: Instant) -> Option<Readiness> {
        let route = self.routes.get(name)?;
        let gateway = self.gateways.get(&route.gateway)?;
        Some(match read_gateway(self, &route.gateway, gateway, until) {
            Reach::Unreachable => Readiness::Unreachable,
            Reach::NotReady => Readiness::NotReady,
            Reach::Ready { models, of_all, of_model } => {
                if !models.contains_key(&route.alias) {
                    Readiness::Unlisted
                } else {
                    let status = of_all.get(&route.alias).or_else(|| of_model.get(&route.alias).filter(|_| route.kind.has_model_status()));
                    match status {
                        Some(s) if !s.callable.unwrap_or(s.phase.as_deref() == Some("ready")) => Readiness::NotCallable { phase: s.phase.clone() },
                        _ => Readiness::Callable,
                    }
                }
            }
        })
    }

    /// Whether the file declares no gateway and no route.
    pub fn is_empty(&self) -> bool {
        self.gateways.is_empty() && self.routes.is_empty()
    }

    /// The declared route names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.routes.keys().map(String::as_str).collect()
    }

    pub fn route(&self, name: &str) -> Option<&Route> {
        self.routes.get(name)
    }

    pub fn routes(&self) -> impl Iterator<Item = &Route> {
        self.routes.values()
    }

    pub fn gateway(&self, name: &str) -> Option<&Gateway> {
        self.gateways.get(name)
    }

    /// `route_policy.default`: the route of a run that names none.
    pub fn default_route(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// `route_policy.classifier`: the route of a delegation's classify and build calls.
    pub fn classifier(&self) -> Option<&str> {
        self.classifier.as_deref()
    }

    /// The route `name` as a call reaches it: its gateway's address and token, the alias, the kind and the prices.
    /// None for a name that is not declared.
    pub fn resolve(&self, name: &str) -> Option<Resolved<'_>> {
        let route = self.routes.get(name)?;
        let gateway = self.gateways.get(&route.gateway)?;
        Some(Resolved { route: &route.name, base_url: &gateway.base_url, token: gateway.token(), alias: &route.alias, kind: route.kind, prices: route.prices })
    }

    /// Sends the wake of the route `name` (`POST /v1/models/{alias}/wake` with its gateway's token), done by `until`:
    /// what the gateway answered. None for a name that is not declared. The caller books the wake before it calls this.
    pub fn wake(&self, name: &str, until: Instant) -> Option<WakeAnswer> {
        let route = self.routes.get(name)?;
        let gateway = self.gateways.get(&route.gateway)?;
        let Some((client, prefix)) = client_of(gateway) else { return Some(WakeAnswer::default()) };
        let path = format!("{prefix}/v1/models/{}/wake", segment(&route.alias));
        Some(match client.with_deadline(until).request("POST", &path, None) {
            Err(_) => WakeAnswer::default(),
            Ok(reply) => {
                let body: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
                WakeAnswer { status: Some(reply.status), code: word(&body["error"]["code"]) }
            }
        })
    }
}

/// What a gateway answered a wake: its status (none: no answer in time) and its error code, when it gave one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WakeAnswer {
    pub status: Option<u16>,
    pub code: Option<String>,
}

/// The gateway's code of a wake that would pass one of its spend caps: the run that asked for it stops.
pub const SPEND_CEILING: &str = "spend-ceiling";

/// What a route's gateway says of it when a run is about to take it ([`RouteTable::readiness`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// The gateway is ready and lists the alias, and the status of the model, when one was read, says it can answer
    /// (`callable` of `GET /v1/status`, else the phase `ready` of `GET /v1/models/{alias}/status` for a managed or a
    /// local route). A hosted route on a gateway without `/v1/status` is read no further: no model call probes it.
    Callable,
    /// No answer from `GET /ready` or `GET /v1/models` in time, or an address the daemon does not speak (https).
    Unreachable,
    /// `GET /ready` answered, not 200, or `GET /v1/models` gave no list.
    NotReady,
    /// The gateway is ready and does not list the route's alias.
    Unlisted,
    /// The status of the model says it cannot answer now, with the phase the gateway gave.
    NotCallable { phase: Option<String> },
}

// ---- the view `routes`: what the gateways say ----

/// How a gateway answered `GET /ready` and `GET /v1/models`.
enum Reach {
    /// No answer in time, or an address the daemon does not speak (https).
    Unreachable,
    /// An answer that is not 200, or no list of models.
    NotReady,
    /// Ready, with its models as `GET /v1/models` lists them; the status of each model `GET /v1/status` gave (`of_all`)
    /// and of each model whose own status route answered (`of_model`), by alias.
    Ready { models: BTreeMap<String, Listed>, of_all: BTreeMap<String, ModelStatus>, of_model: BTreeMap<String, ModelStatus> },
}

/// One model of `GET /v1/models`: its wires, and its context length and tool calling when the gateway gives them.
#[derive(Clone, Default)]
struct Listed {
    wires: Vec<String>,
    max_model_len: Option<u64>,
    tool_calling: Option<bool>,
}

impl Listed {
    fn of(m: &Value) -> Listed {
        Listed { wires: wires(&m["wires"]), max_model_len: m["max_model_len"].as_u64(), tool_calling: m["tool_calling"].as_bool() }
    }
}

/// The status of one model as the gateway gives it.
#[derive(Clone, Default)]
struct ModelStatus {
    phase: Option<String>,
    callable: Option<bool>,
}

/// A client of a gateway and the path the gateway's routes start with.
type Reader = (Client, String);

/// A client of `base_url` and the path its routes start with; none for an address the daemon does not speak.
fn client_of(gateway: &Gateway) -> Option<Reader> {
    let rest = gateway.base_url.strip_prefix("http://")?;
    let (authority, prefix) = rest.split_once('/').map(|(a, p)| (a, format!("/{}", p.trim_end_matches('/')))).unwrap_or((rest, String::new()));
    let target: Target = crate::client::url::parse(&format!("http://{authority}")).ok()?;
    let prefix = if prefix == "/" { String::new() } else { prefix };
    Some((Client::new(target, gateway.token.clone().map(Token::new)), prefix))
}

/// A path segment with every byte that is not unreserved percent-encoded.
fn segment(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// `GET <prefix><path>`, whole within [`GET_BOUND`] and by `until`: the status and the body as JSON (null when it
/// is none); `None` when no whole answer came in that time.
fn get(client: &Client, prefix: &str, path: &str, until: Instant) -> Option<(u16, Value)> {
    let until = until.min(Instant::now() + GET_BOUND);
    let reply = client.clone().with_deadline(until).request("GET", &format!("{prefix}{path}"), None).ok()?;
    Some((reply.status, serde_json::from_str(&reply.body).unwrap_or(Value::Null)))
}

/// The most wires a row keeps of a model.
const MAX_WIRES: usize = 16;

/// A word a gateway gave (a wire, a phase), when it is 1 to 64 visible ASCII characters: nothing else of a gateway's
/// answer reaches a row.
fn word(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_graphic())).map(str::to_owned)
}

fn wires(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(word).take(MAX_WIRES).collect()).unwrap_or_default()
}

/// The status of one model in `m`; none when it has neither a phase nor a `callable` that is a boolean.
fn model_status(m: &Value) -> Option<ModelStatus> {
    let status = ModelStatus { phase: word(&m["phase"]), callable: m["callable"].as_bool() };
    (status.phase.is_some() || status.callable.is_some()).then_some(status)
}

/// What the gateway `name` says of the routes of `table` on it, every request done by `until`.
fn read_gateway(table: &RouteTable, name: &str, gateway: &Gateway, until: Instant) -> Reach {
    let Some((client, prefix)) = client_of(gateway) else { return Reach::Unreachable };
    match get(&client, &prefix, "/ready", until) {
        None => return Reach::Unreachable,
        Some((200, _)) => {}
        Some(_) => return Reach::NotReady,
    }
    let models: BTreeMap<String, Listed> = match get(&client, &prefix, "/v1/models", until) {
        None => return Reach::Unreachable,
        Some((200, body)) if body["data"].is_array() => body["data"].as_array().into_iter().flatten().filter_map(|m| Some((m["id"].as_str()?.to_owned(), Listed::of(m)))).collect(),
        Some(_) => return Reach::NotReady,
    };
    let listed: Vec<&Route> = table.routes.values().filter(|r| r.gateway == name && models.contains_key(&r.alias)).collect();
    let of_all = if listed.is_empty() { BTreeMap::new() } else { read_all_status(&client, &prefix, until).unwrap_or_default() };
    let mut of_model = BTreeMap::new();
    for route in listed {
        if route.kind.has_model_status() && !of_all.contains_key(&route.alias) && !of_model.contains_key(&route.alias) {
            if let Some(s) = read_model_status(&client, &prefix, &route.alias, until) {
                of_model.insert(route.alias.clone(), s);
            }
        }
    }
    Reach::Ready { models, of_all, of_model }
}

/// `GET /v1/status`: the status of every model, by alias; none when the gateway does not serve it.
fn read_all_status(client: &Client, prefix: &str, until: Instant) -> Option<BTreeMap<String, ModelStatus>> {
    match get(client, prefix, "/v1/status", until)? {
        (200, body) if body["models"].is_array() => Some(
            body["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| Some((m["model"].as_str()?.to_owned(), model_status(m)?)))
                .collect(),
        ),
        _ => None,
    }
}

/// `GET /v1/models/{alias}/status`: the phase of one model; none when it did not answer one. A gateway that does not
/// serve the route answers 404: the model has no status there, and its gateway is no less reachable.
fn read_model_status(client: &Client, prefix: &str, alias: &str, until: Instant) -> Option<ModelStatus> {
    match get(client, prefix, &format!("/v1/models/{}/status", segment(alias)), until)? {
        (200, body) => word(&body["phase"]).map(|p| ModelStatus { phase: Some(p), callable: None }),
        _ => None,
    }
}

/// One read of every gateway of `table` at `now_millis`, each gateway on a thread of its own and every request done
/// by `until`: the rows of the view.
fn read(table: &RouteTable, now_millis: i64, until: Instant) -> Vec<Value> {
    let names: BTreeSet<&str> = table.routes.values().map(|r| r.gateway.as_str()).collect();
    let reads: BTreeMap<&str, Reach> = std::thread::scope(|scope| {
        let readers: Vec<_> = names.into_iter().filter_map(|name| Some((name, table.gateways.get(name)?))).map(|(name, gateway)| (name, scope.spawn(move || read_gateway(table, name, gateway, until)))).collect();
        readers.into_iter().map(|(name, reader)| (name, reader.join().unwrap_or(Reach::Unreachable))).collect()
    });
    rows_of(table, &reads, now_millis)
}

/// The rows of `table` from what its gateways said; a gateway that is not in `reads` gave no answer.
fn rows_of(table: &RouteTable, reads: &BTreeMap<&str, Reach>, now_millis: i64) -> Vec<Value> {
    let read_at = crate::clock::format_instant(now_millis);
    table
        .routes
        .values()
        .map(|route| {
            let (status, listed, phase) = match reads.get(route.gateway.as_str()) {
                None | Some(Reach::Unreachable) => ("gateway-unreachable", Listed::default(), None),
                Some(Reach::NotReady) => ("gateway-not-ready", Listed::default(), None),
                Some(Reach::Ready { models, of_all, of_model }) => match models.get(&route.alias) {
                    None => ("model-unknown", Listed::default(), None),
                    Some(listed) => {
                        let of_model = || of_model.get(&route.alias).filter(|_| route.kind.has_model_status());
                        match of_all.get(&route.alias).or_else(of_model).cloned() {
                            Some(s) => {
                                let ready = s.callable.unwrap_or(s.phase.as_deref() == Some("ready"));
                                (if ready { "ready" } else { "not-ready" }, listed.clone(), s.phase)
                            }
                            None => ("listed", listed.clone(), None),
                        }
                    }
                },
            };
            let mut row = Map::new();
            row.insert("route".into(), json!(route.name));
            row.insert("gateway".into(), json!(route.gateway));
            row.insert("alias".into(), json!(route.alias));
            row.insert("kind".into(), json!(route.kind.as_str()));
            row.insert("wires".into(), json!(listed.wires));
            row.insert("status".into(), json!(status));
            if let Some(phase) = phase {
                row.insert("phase".into(), json!(phase));
            }
            if let Some(len) = listed.max_model_len {
                row.insert("max_model_len".into(), json!(len));
            }
            if let Some(tools) = listed.tool_calling {
                row.insert("tool_calling".into(), json!(tools));
            }
            row.insert("read_at".into(), json!(read_at));
            Value::Object(row)
        })
        .collect()
}

/// The rows of the last read and whether a read runs.
#[derive(Debug, Default)]
struct Cache {
    last: Option<(i64, Vec<Value>)>,
    reading: bool,
}

#[derive(Debug, Default)]
struct Shared {
    table: Arc<RouteTable>,
    cache: Mutex<Cache>,
    read_done: Condvar,
}

impl Shared {
    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Ends a read: its rows, when it made any, become the cache, and the waiters wake. It ends the read on a panic too.
struct Reading {
    shared: Arc<Shared>,
    at: i64,
    rows: Option<Vec<Value>>,
}

impl Drop for Reading {
    fn drop(&mut self) {
        let mut cache = self.shared.cache();
        if let Some(rows) = self.rows.take() {
            cache.last = Some((self.at, rows));
        }
        cache.reading = false;
        self.shared.read_done.notify_all();
    }
}

/// The view `routes`: the routes of the table with what their gateways say, read at most once every
/// [`READ_EVERY_MILLIS`] of the clock it is given.
///
/// A request answers the rows of the last read at once. When they are older than [`READ_EVERY_MILLIS`] it starts
/// one read in the background, unless one runs, and still answers the older rows. Only the first request, before
/// any read ended, waits for a read, and at most [`READ_BOUND`]; no lock is held while a gateway is read.
#[derive(Debug, Default)]
pub struct Listing {
    shared: Arc<Shared>,
}

impl Listing {
    pub fn new(table: Arc<RouteTable>) -> Listing {
        Listing { shared: Arc::new(Shared { table, ..Shared::default() }) }
    }

    pub fn table(&self) -> &Arc<RouteTable> {
        &self.shared.table
    }

    /// The rows at `now_millis`.
    pub fn rows(&self, now_millis: i64) -> Vec<Value> {
        if self.shared.table.routes.is_empty() {
            return Vec::new();
        }
        let asked = Instant::now();
        let mut cache = self.shared.cache();
        let start = !cache.reading && cache.last.as_ref().is_none_or(|(at, _)| !(0..READ_EVERY_MILLIS).contains(&(now_millis - at)));
        if start {
            cache.reading = self.start(now_millis);
        }
        if let Some((_, rows)) = cache.last.as_ref() {
            return rows.clone();
        }
        drop(cache);
        let wait = || {
            let until = asked + READ_BOUND + WAIT_SLACK;
            let mut cache = self.shared.cache();
            while cache.last.is_none() {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                cache = self.shared.read_done.wait_timeout(cache, left).unwrap_or_else(|e| e.into_inner()).0;
            }
            cache.last.as_ref().map(|(_, rows)| rows.clone())
        };
        let rows = match tokio::runtime::Handle::try_current() {
            Ok(h) if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => tokio::task::block_in_place(wait),
            _ => wait(),
        };
        rows.unwrap_or_else(|| rows_of(&self.shared.table, &BTreeMap::new(), now_millis))
    }

    /// Starts one read at `now_millis` on a thread of its own: whether it started. The caller holds the lock of the
    /// cache, so the read ends after the caller marked it as running.
    fn start(&self, now_millis: i64) -> bool {
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new().name("runbook-routes".into()).spawn(move || {
            let mut reading = Reading { shared, at: now_millis, rows: None };
            reading.rows = Some(read(&reading.shared.table, now_millis, Instant::now() + READ_BOUND));
        });
        spawned.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_segment_is_encoded_and_a_base_with_a_path_keeps_it_as_a_prefix() {
        assert_eq!(segment("qwen"), "qwen");
        assert_eq!(segment("org/model:v1 x"), "org%2Fmodel%3Av1%20x");
        let g = |url: &str| Gateway { name: "g".into(), member: "gateways.g".into(), base_url: url.into(), token_file: None, token: None };
        assert_eq!(client_of(&g("http://127.0.0.1:9/prefix/")).map(|(_, p)| p), Some("/prefix".to_owned()));
        assert_eq!(client_of(&g("http://127.0.0.1:9/")).map(|(_, p)| p), Some(String::new()));
        assert_eq!(client_of(&g("http://127.0.0.1:9")).map(|(_, p)| p), Some(String::new()));
        assert!(client_of(&g("https://127.0.0.1:9")).is_none(), "the daemon speaks no https");
    }

    #[test]
    fn names_and_aliases() {
        assert!(is_name("local-qwen") && is_name("a.b_c") && !is_name("") && !is_name("a b") && !is_name(&"x".repeat(65)));
        assert!(is_alias("qwen") && is_alias("org/model:v1") && !is_alias("http://h/v1") && !is_alias("a b") && !is_alias(""));
    }
}
