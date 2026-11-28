//! The day's figures of each route (story `any-route-kind-takes-a-daily-cap`; `docs/model-routes.md`, section 5;
//! `docs/daemon-api.md`, "The day's figures of a route"): what the relay booked on a route on a day and how many calls
//! it sent there. A route of any kind that declares `daily_cap_micro_usd` or `daily_max_llm_calls` refuses a call past
//! the day's figure before it is sent ([`RouteDays::admit`]).
//!
//! The day is [`day_of`] the daemon's clock: the UTC date, the first 10 characters of its instant, the day a wake books
//! on. A call counts on the day it was admitted, its calls at once and its cost when its answer has ended (before its
//! caller sees that end), whatever day that is. The rule of the spend cap: a call is admitted while the day's booked
//! spend is below the cap, so no call is sent once the booked figure has reached it; the last call admitted below the
//! cap, and the calls in flight beside it, may take the figure above the cap by their cost, which is not known before
//! their answers end. The rule of the calls: a call is admitted while the day's calls are below the cap, counted as it
//! is admitted, so the count never passes it.
//!
//! The figures are held in the daemon's memory under one lock, read by the relay and by the view `routes` alike, and
//! rebuilt when the daemon starts from the model-call lines of every spend ledger in `<state>/ledgers/`: the relay
//! writes into each line the day it counts on ([`DAY_KEY`]), a key the daemon's day figures of
//! `story:daemon-caps-are-keys-of-one-file` can read too. A call that was in flight when the daemon was killed has no
//! line, and a daemon started again does not count it. A ledger that cannot be read at start leaves the figures
//! unknown: every call on a route with a daily cap is refused then, naming the ledger (fail closed).

use crate::clock::{format_instant, Clock};
use crate::routes::{Daily, RouteTable};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The member of a ledger line (and of the run event `runbook.model-call`) that names the day the call counts on.
pub const DAY_KEY: &str = "day";
/// The key of a route's daily spend cap in `runbook.toml`.
pub const DAILY_CAP: &str = "daily_cap_micro_usd";
/// The key of a route's daily cap of calls in `runbook.toml`.
pub const DAILY_CALLS: &str = "daily_max_llm_calls";

/// The day of the instant `millis` (milliseconds since the epoch): its UTC date, `YYYY-MM-DD`.
pub fn day_of(millis: i64) -> String {
    format_instant(millis).chars().take(10).collect()
}

/// The key a refusal names: `route.<name>.<key>`.
pub fn cap_key(route: &str, key: &str) -> String {
    format!("route.{route}.{key}")
}

/// What a route sent and booked on one day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Figure {
    booked_micro_usd: u64,
    calls: u64,
}

/// A call the route's daily cap refused: the route, the key ([`cap_key`]), whether it is the cap of calls, and what the
/// figure stood at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub route: String,
    pub key: String,
    pub calls: bool,
    pub detail: String,
}

/// The day's figures of every route the relay sends to.
pub struct RouteDays {
    caps: BTreeMap<String, Daily>,
    clock: Arc<dyn Clock>,
    figures: Mutex<HashMap<(String, String), Figure>>,
    /// Why the figures could not be rebuilt at start; none when they were.
    broken: Option<String>,
}

impl std::fmt::Debug for RouteDays {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteDays").field("caps", &self.caps).field("broken", &self.broken).finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl RouteDays {
    /// The figures of the routes of `routes` on the days of `clock`, rebuilt from the model-call lines of the spend
    /// ledgers in `ledgers` (a directory that does not exist holds none) for the day of the clock and later.
    pub fn open(routes: &RouteTable, clock: Arc<dyn Clock>, ledgers: &Path) -> RouteDays {
        let caps = routes.routes().map(|r| (r.name.clone(), r.daily)).collect();
        let today = day_of(clock.now_millis());
        let mut figures: HashMap<(String, String), Figure> = HashMap::new();
        let broken = rebuild(ledgers, &today, &mut figures).err();
        RouteDays { caps, clock, figures: Mutex::new(figures), broken }
    }

    /// The day of the daemon's clock now.
    pub fn day(&self) -> String {
        day_of(self.clock.now_millis())
    }

    /// Takes one call on `route` against its daily caps: the day it counts on, its call counted; refused, with nothing
    /// counted, when the day's calls have reached `daily_max_llm_calls` or the day's booked spend `daily_cap_micro_usd`
    /// (the cap of calls is read first).
    pub fn admit(&self, route: &str) -> Result<String, Refused> {
        self.hold(route, true)
    }

    /// Whether [`Self::admit`] would take a call on `route` now, counting nothing: the relay asks it before the run's own
    /// caps, so a call those refuse never holds a count of the route's day another run could need meanwhile.
    pub fn check(&self, route: &str) -> Result<(), Refused> {
        self.hold(route, false).map(drop)
    }

    fn hold(&self, route: &str, count: bool) -> Result<String, Refused> {
        let day = self.day();
        let daily = self.caps.get(route).copied().unwrap_or_default();
        let capped = daily.micro_usd.is_some() || daily.llm_calls.is_some();
        let refused = |key: &str, detail: String| Refused { route: route.to_owned(), key: cap_key(route, key), calls: key == DAILY_CALLS, detail };
        if let (true, Some(why)) = (capped, &self.broken) {
            let key = if daily.llm_calls.is_some() { DAILY_CALLS } else { DAILY_CAP };
            return Err(refused(key, format!("route {route}: the day's figures could not be rebuilt when the daemon started: {why}")));
        }
        let mut figures = lock(&self.figures);
        // a day before this one counts no more
        figures.retain(|(_, d), _| d.as_str() >= day.as_str());
        let figure = figures.entry((route.to_owned(), day.clone())).or_default();
        if let Some(max) = daily.llm_calls.filter(|max| figure.calls >= *max) {
            return Err(refused(DAILY_CALLS, format!("route {route}: {} of {max} calls sent on {day}", figure.calls)));
        }
        if let Some(cap) = daily.micro_usd.filter(|cap| figure.booked_micro_usd >= *cap) {
            return Err(refused(DAILY_CAP, format!("route {route}: {} of {cap} micro USD booked on {day}", figure.booked_micro_usd)));
        }
        if count {
            figure.calls = figure.calls.saturating_add(1);
        }
        Ok(day)
    }

    /// Books `micro_usd`, what the ledger books for a call on `route` admitted on `day`.
    pub fn book(&self, route: &str, day: &str, micro_usd: u64) {
        let mut figures = lock(&self.figures);
        let figure = figures.entry((route.to_owned(), day.to_owned())).or_default();
        figure.booked_micro_usd = figure.booked_micro_usd.saturating_add(micro_usd);
    }

    /// The members of the row of `route` in the view `routes` for the day of the daemon's clock:
    /// `daily_booked_micro_usd` and `daily_llm_calls` for every route, `daily_cap_micro_usd` and `daily_max_llm_calls`
    /// where the route declares them.
    pub fn figures(&self, route: &str) -> Map<String, Value> {
        let day = self.day();
        let figure = lock(&self.figures).get(&(route.to_owned(), day)).copied().unwrap_or_default();
        let daily = self.caps.get(route).copied().unwrap_or_default();
        let mut m = Map::new();
        m.insert("daily_booked_micro_usd".into(), json!(figure.booked_micro_usd));
        if let Some(cap) = daily.micro_usd {
            m.insert(DAILY_CAP.into(), json!(cap));
        }
        m.insert("daily_llm_calls".into(), json!(figure.calls));
        if let Some(max) = daily.llm_calls {
            m.insert(DAILY_CALLS.into(), json!(max));
        }
        m
    }
}

/// Adds the model-call lines of every ledger in `dir` that count on `today` or later into `figures`; `Err` names the first
/// ledger that cannot be read.
fn rebuild(dir: &Path, today: &str, figures: &mut HashMap<(String, String), Figure>) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{}: {}", dir.display(), e.kind())),
    };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    paths.sort();
    for path in paths {
        let lines = crate::ledger::Ledger::open(&path).and_then(|l| l.entries())?;
        for line in lines.iter().filter(|l| l.get("what").and_then(Value::as_str) == Some("model call")) {
            let (Some(route), Some(day)) = (line.get("route").and_then(Value::as_str), line.get(DAY_KEY).and_then(Value::as_str)) else { continue };
            if day < today {
                continue;
            }
            let figure = figures.entry((route.to_owned(), day.to_owned())).or_default();
            figure.calls = figure.calls.saturating_add(1);
            figure.booked_micro_usd = figure.booked_micro_usd.saturating_add(line.get("micro_usd").and_then(Value::as_u64).unwrap_or(0));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_day_is_the_utc_date_of_the_instant() {
        assert_eq!(day_of(crate::clock::parse_instant("2026-11-28T23:59:59.999Z").unwrap()), "2026-11-28");
        assert_eq!(day_of(crate::clock::parse_instant("2026-11-29T00:00:00Z").unwrap()), "2026-11-29");
        assert_eq!(cap_key("h", DAILY_CAP), "route.h.daily_cap_micro_usd");
    }
}
