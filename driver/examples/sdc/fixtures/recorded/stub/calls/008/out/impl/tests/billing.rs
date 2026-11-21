use billing::{Billing, Refusal, State};

fn draft() -> (Billing, u64) {
    let mut b = Billing::default();
    let id = b.create_invoice("acc-1", "a@example.com", 10).unwrap();
    (b, id)
}

#[test]
fn create_invoice() {
    let (mut b, id) = draft();
    assert_eq!(b.invoices[&id].state, State::Draft);
    assert_eq!(b.create_invoice("acc-1", "a@example.com", 0), Err(Refusal::InvalidAmount { submitted: 0 }));
}

#[test]
fn issue_invoice() {
    let (mut b, id) = draft();
    b.issue_invoice(id).unwrap();
    assert_eq!(b.invoices[&id].state, State::Issued);
    assert_eq!(b.issue_invoice(id), Err(Refusal::InvoiceStateConflict { current: State::Issued }));
}

#[test]
fn pay_invoice() {
    let (mut b, id) = draft();
    assert!(b.pay_invoice(id).is_err());
    b.issue_invoice(id).unwrap();
    b.pay_invoice(id).unwrap();
    assert_eq!(b.invoices[&id].state, State::Paid);
}

#[test]
fn cancel_invoice() {
    let (mut b, id) = draft();
    b.cancel_invoice(id).unwrap();
    assert_eq!(b.invoices[&id].state, State::Cancelled);
    assert!(b.cancel_invoice(id).is_err());
}

#[test]
fn send_email() {
    let mut b = Billing::default();
    assert_eq!(b.send_email("a@example.com", "invoice-issued"), 1);
    assert_eq!(b.emails[0].1, "invoice-issued");
}
