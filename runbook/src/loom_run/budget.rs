//! The budget of one call: before each step the bindings copy is limited so that no call may
//! spend more than the ledger granted.

use std::path::Path;

/// The largest per-call budget the bindings in `dir` name (`--max-budget-usd` values and
/// `max_budget_micro_usd` fields), micro USD; `None` when they name none.
pub fn largest_call_budget(_dir: &Path) -> Result<Option<u64>, String> {
    Err(crate::loom_run::not_built("budget scan"))
}

/// Lowers every per-call budget in the bindings copy `dir` to at most `micro_usd`.
pub fn limit_call_budgets(_dir: &Path, _micro_usd: u64) -> Result<(), String> {
    Err(crate::loom_run::not_built("budget limit"))
}
