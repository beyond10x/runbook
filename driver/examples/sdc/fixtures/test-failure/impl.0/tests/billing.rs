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

#[test]
fn a_second_email_is_counted() {
    let mut b = Billing::default();
    b.send_email("a@example.com", "invoice-issued");
    assert_eq!(b.send_email("a@example.com", "invoice-paid"), 1, "fixture-failure: the second email of an account counts 2");
}

// Forty passing tests, so the failure sits far above the end of the full cargo output.
macro_rules! passing { ($($n:ident)*) => { $( #[test] fn $n() {} )* } }
passing!(p01 p02 p03 p04 p05 p06 p07 p08 p09 p10 p11 p12 p13 p14 p15 p16 p17 p18 p19 p20
  p21 p22 p23 p24 p25 p26 p27 p28 p29 p30 p31 p32 p33 p34 p35 p36 p37 p38 p39 p40);
