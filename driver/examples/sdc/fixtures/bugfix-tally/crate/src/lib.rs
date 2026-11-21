//! A tally of named counts with a running total.

use std::collections::BTreeMap;

/// Named counts and their total.
#[derive(Debug, Default)]
pub struct Tally {
    counts: BTreeMap<String, u64>,
    total: u64,
}

impl Tally {
    /// An empty tally.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `n` to the count of `name`.
    pub fn add(&mut self, name: &str, n: u64) {
        *self.counts.entry(name.to_string()).or_insert(0) += n;
        self.total += n;
    }

    /// Remove `name` and its whole count; returns the count removed, 0 when `name` was not there.
    pub fn remove(&mut self, name: &str) -> u64 {
        self.counts.remove(name).unwrap_or(0)
    }

    /// The count of `name`, 0 when it is not there.
    pub fn count(&self, name: &str) -> u64 {
        self.counts.get(name).copied().unwrap_or(0)
    }

    /// The sum of every count.
    pub fn total(&self) -> u64 {
        self.total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_total_is_zero() {
        assert_eq!(Tally::new().total(), 0);
    }

    #[test]
    fn add_counts_and_totals() {
        let mut t = Tally::new();
        t.add("a", 2);
        t.add("a", 3);
        assert_eq!(t.count("a"), 5);
        assert_eq!(t.total(), 5);
    }

    #[test]
    fn remove_returns_the_count() {
        let mut t = Tally::new();
        t.add("a", 2);
        assert_eq!(t.remove("a"), 2);
        assert_eq!(t.count("a"), 0);
    }

    #[test]
    fn remove_missing_is_zero() {
        assert_eq!(Tally::new().remove("x"), 0);
    }
}
