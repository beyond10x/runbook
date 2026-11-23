//! A value a user names for a limit: the reader every money option and variable of the programs
//! shares, and the cut of a refused value that reaches a console.

/// The most bytes of a refused value a message shows.
const SHOWN: usize = 40;

/// `v` cut at 40 bytes, at a character boundary.
pub fn cut(v: &str) -> &str {
    let mut end = v.len().min(SHOWN);
    while !v.is_char_boundary(end) {
        end -= 1;
    }
    &v[..end]
}

/// A named money value: a finite number of at least 0 (0 spends nothing), else a refusal that
/// names `name` and shows at most 40 bytes of the value. A named value never reads as no limit.
pub fn money(name: &str, raw: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Ok(v.abs()),
        _ => Err(format!("{name} must be a finite number of at least 0 (got {:?})", cut(raw))),
    }
}

/// A named whole-number cap (`--live-cap`): digits that fit 64 bits, 0 allowed. A value taken as text
/// is refused here in our words, naming `name` and showing at most 40 bytes of it.
pub fn whole(name: &str, raw: &str) -> Result<u64, String> {
    raw.parse::<u64>().map_err(|_| format!("{name} takes a whole number of at least 0, got {:?}", cut(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_cut_at_40_bytes_on_a_character_boundary() {
        assert_eq!(cut("abc"), "abc");
        assert_eq!(cut(&"q".repeat(100)).len(), 40);
        let s = format!("{}\u{20ac}", "a".repeat(39));
        assert_eq!(cut(&s).len(), 39);
    }

    #[test]
    fn money_is_a_finite_number_of_at_least_0() {
        assert_eq!(money("X", "0"), Ok(0.0));
        assert_eq!(money("X", "-0"), Ok(0.0));
        assert_eq!(money("X", "2.5"), Ok(2.5));
        for bad in ["", "-1", "inf", "NaN", "1e400", "abc", " 2"] {
            assert!(money("X", bad).is_err(), "{bad:?}");
        }
    }
}
