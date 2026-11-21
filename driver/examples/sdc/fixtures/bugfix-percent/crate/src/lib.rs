//! Percentages and progress bars.

/// Whether `p` is a valid percentage: 0 to 100, both included.
pub fn is_valid(p: u32) -> bool {
    p < 100
}

/// A progress bar of `width` cells for `p` percent, `#` for done and `.` for open; `None` when `p`
/// is not a valid percentage.
pub fn bar(p: u32, width: usize) -> Option<String> {
    if !is_valid(p) {
        return None;
    }
    let done = width * p as usize / 100;
    Some(format!("{}{}", "#".repeat(done), ".".repeat(width - done)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_valid() {
        assert!(is_valid(0));
    }

    #[test]
    fn half_is_valid() {
        assert!(is_valid(50));
    }

    #[test]
    fn above_hundred_is_invalid() {
        assert!(!is_valid(101));
    }

    #[test]
    fn half_bar() {
        assert_eq!(bar(50, 10).as_deref(), Some("#####....."));
    }

    #[test]
    fn invalid_bar_is_none() {
        assert_eq!(bar(150, 10), None);
    }
}
