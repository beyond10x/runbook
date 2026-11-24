//! The caps of a daemon run: the most wall seconds, the most model calls and the most spend (micro
//! USD) a run may use, as `runbook run` spells them (`--max-wall-seconds`, `--max-llm-calls`,
//! `--cap-micro-usd`). There is no default in the binary: the daemon is started with a value for each
//! (`runbook serve`), and a run is never without them.
//!
//! A submit may name its own caps in the `settings` of the request, under the same three names, up to
//! a ceiling the daemon is started with (`CapPolicy`). The daemon writes the caps a run really has
//! into the run's settings, so the view of the run shows them; what is used so far is added to that
//! view as `used_wall_seconds`, `used_llm_calls` and `spent_micro_usd`.

use std::collections::BTreeMap;

/// The settings key (and the `runbook run` option name, with dashes) of the wall cap.
pub const MAX_WALL_SECONDS: &str = "max_wall_seconds";
/// The settings key of the cap on model calls.
pub const MAX_LLM_CALLS: &str = "max_llm_calls";
/// The settings key of the spend cap, micro USD.
pub const CAP_MICRO_USD: &str = "cap_micro_usd";
/// The settings keys of the three caps.
pub const KEYS: [&str; 3] = [MAX_WALL_SECONDS, MAX_LLM_CALLS, CAP_MICRO_USD];

/// The view keys of what a run has used.
pub const USED_WALL_SECONDS: &str = "used_wall_seconds";
pub const USED_LLM_CALLS: &str = "used_llm_calls";
pub const SPENT_MICRO_USD: &str = "spent_micro_usd";

/// The three caps of one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub max_wall_seconds: u64,
    pub max_llm_calls: u64,
    pub cap_micro_usd: u64,
}

impl Caps {
    fn get(&self, key: &str) -> u64 {
        match key {
            MAX_WALL_SECONDS => self.max_wall_seconds,
            MAX_LLM_CALLS => self.max_llm_calls,
            _ => self.cap_micro_usd,
        }
    }

    /// Writes the caps into `settings`, replacing what was there.
    pub fn put(&self, settings: &mut BTreeMap<String, String>) {
        for key in KEYS {
            settings.insert(key.to_owned(), self.get(key).to_string());
        }
    }
}

/// What a run has used so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub wall_seconds: u64,
    pub llm_calls: u64,
    pub spent_micro_usd: u64,
}

impl Usage {
    /// Writes the usage into `settings` (the view of a run), replacing what was there.
    pub fn put(&self, settings: &mut BTreeMap<String, String>) {
        settings.insert(USED_WALL_SECONDS.to_owned(), self.wall_seconds.to_string());
        settings.insert(USED_LLM_CALLS.to_owned(), self.llm_calls.to_string());
        settings.insert(SPENT_MICRO_USD.to_owned(), self.spent_micro_usd.to_string());
    }
}

/// The daemon's defaults and ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapPolicy {
    /// The caps of a run whose submit names none.
    pub defaults: Caps,
    /// The most a submit may name, cap by cap.
    pub ceilings: Caps,
}

impl CapPolicy {
    /// A policy; a cap of 0, and a ceiling below its default, are refused naming the cap.
    pub fn new(defaults: Caps, ceilings: Caps) -> Result<CapPolicy, String> {
        for key in KEYS {
            if defaults.get(key) == 0 {
                return Err(format!("{key} is 0: a cap is a whole number of 1 or more"));
            }
            if ceilings.get(key) < defaults.get(key) {
                return Err(format!("the ceiling of {key}, {}, is below its default {}", ceilings.get(key), defaults.get(key)));
            }
        }
        Ok(CapPolicy { defaults, ceilings })
    }

    /// A policy whose ceilings are its defaults: a submit may lower a cap and not raise it.
    pub fn fixed(defaults: Caps) -> Result<CapPolicy, String> {
        CapPolicy::new(defaults, defaults)
    }

    /// The caps a run has: what its `settings` name, else the defaults. A value that is not a whole
    /// number of 1 or more, or is over its ceiling, is refused naming the cap (and the ceiling).
    pub fn resolve(&self, settings: &BTreeMap<String, String>) -> Result<Caps, String> {
        let mut caps = self.defaults;
        for key in KEYS {
            let Some(text) = settings.get(key) else { continue };
            let value: u64 = text.trim().parse().ok().filter(|v| *v >= 1).ok_or_else(|| format!("{key} {text:?}: not a whole number of 1 or more"))?;
            let ceiling = self.ceilings.get(key);
            if value > ceiling {
                return Err(format!("{key} {value} is over the daemon's ceiling {ceiling}"));
            }
            match key {
                MAX_WALL_SECONDS => caps.max_wall_seconds = value,
                MAX_LLM_CALLS => caps.max_llm_calls = value,
                _ => caps.cap_micro_usd = value,
            }
        }
        Ok(caps)
    }
}
