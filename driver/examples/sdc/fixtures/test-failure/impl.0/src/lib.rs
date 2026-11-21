//! Minimal in-memory billing: the stub implementer's fixed draft of the billing specification.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Draft,
    Issued,
    Paid,
    Cancelled,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    InvalidAmount { submitted: i64 },
    InvoiceStateConflict { current: State },
    UnknownInvoice,
}

#[derive(Debug)]
pub struct Invoice {
    pub account_id: String,
    pub customer_email: String,
    pub amount: i64,
    pub state: State,
}

#[derive(Default)]
pub struct Billing {
    pub invoices: BTreeMap<u64, Invoice>,
    pub emails: Vec<(String, String)>,
}

impl Billing {
    /// billing.invoice.CreateInvoice
    pub fn create_invoice(&mut self, account_id: &str, customer_email: &str, amount: i64) -> Result<u64, Refusal> {
        if amount <= 0 {
            return Err(Refusal::InvalidAmount { submitted: amount });
        }
        let id = self.invoices.len() as u64 + 1;
        self.invoices.insert(id, Invoice {
            account_id: account_id.into(),
            customer_email: customer_email.into(),
            amount,
            state: State::Draft,
        });
        Ok(id)
    }

    fn transition(&mut self, id: u64, from: &[State], to: State) -> Result<(), Refusal> {
        let inv = self.invoices.get_mut(&id).ok_or(Refusal::UnknownInvoice)?;
        if !from.contains(&inv.state) {
            return Err(Refusal::InvoiceStateConflict { current: inv.state });
        }
        inv.state = to;
        Ok(())
    }

    /// billing.invoice.IssueInvoice
    pub fn issue_invoice(&mut self, id: u64) -> Result<(), Refusal> {
        self.transition(id, &[State::Draft], State::Issued)
    }

    /// billing.invoice.PayInvoice
    pub fn pay_invoice(&mut self, id: u64) -> Result<(), Refusal> {
        self.transition(id, &[State::Issued], State::Paid)
    }

    /// billing.invoice.CancelInvoice
    pub fn cancel_invoice(&mut self, id: u64) -> Result<(), Refusal> {
        self.transition(id, &[State::Draft, State::Issued], State::Cancelled)
    }

    /// billing.email.SendEmail
    pub fn send_email(&mut self, recipient: &str, template: &str) -> usize {
        self.emails.push((recipient.into(), template.into()));
        self.emails.len()
    }
}
