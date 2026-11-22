//! The daemon's one clock: the system clock in production, a settable clock for tests that build the
//! daemon in process.

use super::commands::config;
use super::state::scratch;
use runbook::clock::{format_instant, parse_instant, Clock, SetClock, SystemClock};
use runbook::commands::{CloseSession, Daemon, OpenSession};
use runbook::stub::StubRuntime;
use std::sync::Arc;

fn session_row(d: &Daemon, id: &str) -> serde_json::Value {
    d.sessions().into_iter().find(|r| r["session_id"] == id).expect("the session is listed")
}

#[test]
fn an_instant_parses_and_formats_back_across_the_calendar() {
    for text in ["1970-01-01T00:00:00.000Z", "2000-02-29T23:59:59.999Z", "2026-11-22T10:05:00.000Z", "2100-03-01T00:00:00.001Z", "1969-12-31T23:59:59.500Z"] {
        let millis = parse_instant(text).unwrap_or_else(|| panic!("{text} parses"));
        assert_eq!(format_instant(millis), text);
    }
    assert_eq!(parse_instant("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_instant("2026-11-22T10:05:00Z"), parse_instant("2026-11-22T10:05:00.000Z"));
    assert_eq!(parse_instant("2026-11-22T10:05:00.5Z"), parse_instant("2026-11-22T10:05:00.500Z"));
    for bad in ["", "2026-11-22", "2026-11-22T10:05:00", "2026-11-22 10:05:00Z", "2026-13-22T10:05:00Z", "2026-11-22T24:05:00Z", "2026-11-22T10:05:00+01:00", "2026-11-22T10:05:00.Z"] {
        assert_eq!(parse_instant(bad), None, "{bad}");
    }
}

#[test]
fn a_set_clock_stands_where_it_was_put_and_refuses_a_bad_instant() {
    let c = SetClock::at("2026-11-22T10:05:00Z").unwrap();
    assert_eq!(format_instant(c.now_millis()), "2026-11-22T10:05:00.000Z");
    assert!(c.set("2026-11-22T10:06:00.250Z"));
    assert_eq!(format_instant(c.now_millis()), "2026-11-22T10:06:00.250Z");
    assert!(!c.set("tomorrow"));
    assert_eq!(format_instant(c.now_millis()), "2026-11-22T10:06:00.250Z");
    assert!(SetClock::at("tomorrow").is_none());
}

#[test]
fn the_daemon_records_the_instant_of_its_clock_and_no_other() {
    let dir = scratch("clock-set");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let clock = Arc::new(SetClock::at("2020-01-01T00:00:00Z").unwrap());
    let d = Daemon::start_with_clock(config(&dir), Arc::new(StubRuntime::new()), clock.clone()).unwrap();
    assert_eq!(d.daemon_configuration()[0]["started_at"], "2020-01-01T00:00:00.000Z");
    clock.set("2026-11-22T10:00:30Z");
    d.open_session(OpenSession { session_id: "nightly".into(), client: "ci".into() }).unwrap();
    clock.set("2026-11-22T10:02:00Z");
    d.close_session(CloseSession { session_id: "nightly".into() }).unwrap();
    let row = session_row(&d, "nightly");
    assert_eq!(row["opened_at"], "2026-11-22T10:00:30.000Z");
    assert_eq!(row["closed_at"], "2026-11-22T10:02:00.000Z");
    d.stop();
}

#[test]
fn the_daemon_started_the_usual_way_reads_the_system_clock() {
    let dir = scratch("clock-system");
    std::fs::create_dir_all(dir.join("ctx")).unwrap();
    let before = SystemClock.now_millis();
    let d = Daemon::start(config(&dir), Arc::new(StubRuntime::new())).unwrap();
    d.open_session(OpenSession { session_id: "s".into(), client: "ci".into() }).unwrap();
    let after = SystemClock.now_millis();
    let opened = parse_instant(session_row(&d, "s")["opened_at"].as_str().unwrap()).unwrap();
    assert!(before <= opened && opened <= after, "{before} <= {opened} <= {after}");
    d.stop();
}
