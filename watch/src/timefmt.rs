//! UTC timestamps as milliseconds since the epoch, written and read as RFC 3339 (`...Z`).

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `2026-11-21T10:02:03.412Z`
pub fn fmt_ms(ms: i64) -> String {
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let (y, m, d) = civil_from_days(days);
    let (h, mi, s, f) = (rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{f:03}Z")
}

/// Reads `YYYY-MM-DDTHH:MM:SS[.fff...](Z|+hh:mm|-hh:mm)`.
pub fn parse_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, sec) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    let mut i = 19;
    let mut frac = 0i64;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let digits = &s[start..i];
        let ms3: String = digits.chars().chain("000".chars()).take(3).collect();
        frac = ms3.parse().ok()?;
    }
    let offset_min = match b.get(i) {
        Some(b'Z') | Some(b'z') => 0,
        Some(c @ (b'+' | b'-')) => {
            let sign = if *c == b'+' { 1 } else { -1 };
            sign * (n(i + 1..i + 3)? * 60 + n(i + 4..i + 6)?)
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    Some(((days * 24 + h) * 60 + mi - offset_min) * 60_000 + sec * 1000 + frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let t = parse_ms("2026-11-20T23:57:55.783Z").unwrap();
        assert_eq!(fmt_ms(t), "2026-11-20T23:57:55.783Z");
        assert_eq!(parse_ms("2026-11-20T23:16:04Z").unwrap() % 1000, 0);
        assert_eq!(parse_ms("2026-11-21T00:57:55.783+01:00"), Some(t));
        assert_eq!(parse_ms("-"), None);
        let now = now_ms();
        assert_eq!(parse_ms(&fmt_ms(now)), Some(now));
    }
}
