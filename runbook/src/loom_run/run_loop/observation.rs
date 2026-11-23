//! The observation after a close (`--conformance observe`) under a named spend cap: a model writes
//! the tables of it, so it is started only when something is left, what it may spend is reserved
//! before and booked after through the ledger calls a step uses, and with nothing left it is not
//! started. A child module of `run_loop.rs`, so that it reaches the loop's own state.

use super::*;
use crate::loom_run::conformance::{Gate, Spent};

/// The gate of one observation: holds the reservation between its start and its end.
pub(super) struct ObservationGate<'r, 'a> {
    runner: &'r mut Runner<'a>,
    held: Option<Reservation>,
}

impl<'r, 'a> ObservationGate<'r, 'a> {
    pub(super) fn new(runner: &'r mut Runner<'a>) -> Self {
        ObservationGate { runner, held: None }
    }
}

impl Gate for ObservationGate<'_, '_> {
    /// With no cap named nothing is held and nothing is stopped. With a cap named the observation
    /// asks for all the cap has left (it names no budget of its own, as a model tool that names
    /// none) and is not started when that is nothing and the run may spend.
    fn start(&mut self) -> Result<(), String> {
        let Some(cap) = self.runner.args.cap_micro_usd else { return Ok(()) };
        let granted = self.runner.grant(cap).map_err(|e| format!("not observed ({e})"))?;
        if granted.left == 0 && self.runner.may_spend() {
            let spent = match &self.runner.ledger {
                Some(ledger) => ledger.spent().map_err(|e| format!("not observed ({e})"))?,
                None => self.runner.spent,
            };
            return Err(format!("not observed (spend cap: {spent} of {cap} micro USD spent, nothing left)"));
        }
        self.held = Some(granted);
        Ok(())
    }

    /// Books what the observation cost; a cost that is not known books the reservation, the most
    /// it could have spent.
    fn finish(&mut self, spent: Spent) {
        let Some(reservation) = self.held.take() else { return };
        let (what, micro) = match spent {
            Spent::Nothing => ("conformance observation", 0),
            Spent::Cost(micro) => ("conformance observation", micro),
            Spent::Unknown => ("conformance observation, cost unknown: reservation booked", self.runner.held(&reservation)),
        };
        if self.runner.commit(&reservation, what, micro).is_err() {
            eprintln!("runbook run: conformance: what the observation cost could not be booked in the spend ledger");
        }
        self.runner.spent = self.runner.spent.saturating_add(micro);
    }
}
