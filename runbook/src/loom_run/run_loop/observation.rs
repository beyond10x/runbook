//! The observation after a close (`--conformance observe`) under a named spend cap: a model writes
//! the tables of it, so it is started only when something is left, what it may spend is reserved
//! before and booked after through the ledger calls a step uses, it never books more than what was
//! reserved, and with nothing left it is not started. What it did is in the records of the run
//! (`conformance_booked`, `conformance_not_observed`). A child module of `run_loop.rs`, so that it
//! reaches the loop's own state.

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

    /// The line to print for an observation that is not started, after the event that says so
    /// (a fixed reason: no path and no byte of a file).
    fn skipped(&mut self, reason: &'static str, line: String) -> String {
        if self.runner.emit(Event::ObservationSkipped { reason }).is_err() {
            eprintln!("runbook run: conformance: that the observation was not started could not be written to the records");
        }
        line
    }
}

impl Gate for ObservationGate<'_, '_> {
    /// With no cap named nothing is held and nothing is stopped. With a cap named the observation
    /// asks for all the cap has left and is not started when that is nothing and the run may spend;
    /// the reservation is the most its model sessions may spend together (the budget it passes to
    /// the tool).
    fn start(&mut self) -> Result<Option<u64>, String> {
        let Some(cap) = self.runner.args.cap_micro_usd else { return Ok(None) };
        let granted = match self.runner.grant(cap) {
            Ok(granted) => granted,
            Err(e) => return Err(self.skipped("the spend ledger could not be read", format!("not observed ({e})"))),
        };
        if granted.left == 0 && self.runner.may_spend() {
            let spent = match &self.runner.ledger {
                Some(ledger) => match ledger.spent() {
                    Ok(spent) => spent,
                    Err(e) => return Err(self.skipped("the spend ledger could not be read", format!("not observed ({e})"))),
                },
                None => self.runner.spent,
            };
            return Err(self.skipped("spend cap: nothing left", format!("not observed (spend cap: {spent} of {cap} micro USD spent, nothing left)")));
        }
        let budget = self.runner.held(&granted);
        self.held = Some(granted);
        Ok(Some(budget))
    }

    /// Books what the observation cost, never more than its reservation (what the cap had left);
    /// a cost that is not known books the reservation, the most it could have spent. The record
    /// of the run says what was booked, and what the observation's own record named.
    fn finish(&mut self, spent: Spent) {
        let Some(reservation) = self.held.take() else { return };
        let held = self.runner.held(&reservation);
        let (what, record, micro) = match spent {
            Spent::Nothing => ("conformance observation", Some(0), 0),
            Spent::Cost(named) if named <= held => ("conformance observation", Some(named), named),
            Spent::Cost(named) => {
                eprintln!("runbook run: conformance: the record names {named} micro USD, more than the {held} reserved: {held} booked");
                ("conformance observation, record names more than the reservation: reservation booked", Some(named), held)
            }
            Spent::Unknown => ("conformance observation, cost unknown: reservation booked", None, held),
        };
        if self.runner.commit(&reservation, what, micro).is_err() {
            eprintln!("runbook run: conformance: what the observation cost could not be booked in the spend ledger");
        }
        self.runner.spent = self.runner.spent.saturating_add(micro);
        let booked = Event::ObservationBooked { reserved: held, record, booked: micro, spent_micro_usd: self.runner.spent };
        if self.runner.emit(booked).is_err() {
            eprintln!("runbook run: conformance: what the observation cost could not be written to the records");
        }
    }
}
