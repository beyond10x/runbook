//! The one clock of the daemon: every instant the daemon records (`opened_at`, `submitted_at`,
//! `ended_at`, the `at` of an event, ...) is read from the `Clock` of its state.
//!
//! Production has one clock, `SystemClock`. `SetClock` is for tests that build the daemon in
//! process (`Daemon::start_with_clock`) and need an exact instant: it says what it is told. No
//! command-line option, environment variable or HTTP input sets a clock; `serve` builds the daemon
//! with `Daemon::start`, which takes `SystemClock`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    /// Milliseconds since 1970-01-01T00:00:00Z.
    fn now_millis(&self) -> i64;
}

/// The host's clock: the only clock of a running daemon.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> i64 {
        let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        d.as_millis() as i64
    }
}

/// A clock that stands at the instant it was last given, for tests.
#[derive(Debug)]
pub struct SetClock {
    millis: AtomicI64,
}

impl SetClock {
    pub fn at_millis(millis: i64) -> SetClock {
        SetClock { millis: AtomicI64::new(millis) }
    }

    /// A clock standing at an RFC 3339 instant in UTC (`2026-11-22T10:05:00Z`, a fraction allowed).
    pub fn at(instant: &str) -> Option<SetClock> {
        parse_instant(instant).map(SetClock::at_millis)
    }

    pub fn set_millis(&self, millis: i64) {
        self.millis.store(millis, Ordering::SeqCst);
    }

    /// Move the clock to an RFC 3339 instant in UTC; false, and no move, when the text is not one.
    pub fn set(&self, instant: &str) -> bool {
        match parse_instant(instant) {
            Some(m) => {
                self.set_millis(m);
                true
            }
            None => false,
        }
    }
}

impl Clock for SetClock {
    fn now_millis(&self) -> i64 {
        self.millis.load(Ordering::SeqCst)
    }
}

/// The RFC 3339 form the daemon records: UTC, with milliseconds (`2026-11-22T10:05:00.000Z`).
pub fn format_instant(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let ms = millis.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Milliseconds since the epoch of an RFC 3339 instant in UTC: `YYYY-MM-DDTHH:MM:SS[.f]Z`.
pub fn parse_instant(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let digits = |from: usize, to: usize| -> Option<i64> {
        let s = b.get(from..to)?;
        if s.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(s).ok()?.parse().ok()
        } else {
            None
        }
    };
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || *b.last()? != b'Z' {
        return None;
    }
    let (year, month, day) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    let mut ms = 0;
    if b.len() > 20 {
        if b[19] != b'.' || b.len() < 22 {
            return None;
        }
        let fraction = &text[20..b.len() - 1];
        if !fraction.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let first3: String = fraction.chars().chain("00".chars()).take(3).collect();
        ms = first3.parse().ok()?;
    } else if b[19] != b'Z' {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + ms)
}
