//! The time a managed route's pod ran for the runs that used it, and each run's share of it (`docs/model-routes.md`,
//! section 5; story `a-managed-route-books-its-pods-measured-time`).
//!
//! A run uses the pod of its managed route from its start (the daemon starts a run only once the route was read ready)
//! to its end, less the stretches in which the daemon read the pod as not ready while the run lived (the run event
//! `runbook.route-pod-phase`). The split rule: the time is cut at every start and end of a run's use, and each piece
//! is split equally over the runs that used the pod in it; a millisecond a piece cannot split equally goes to the
//! runs first in submit order, one each. A run books its milliseconds at the route's price per hour, rounded up to the
//! micro-USD. All figures are milliseconds of the daemon's clock.

/// One run's use of a pod: stretches `[from, to)` in milliseconds, in order and apart.
pub type Uses = Vec<(i64, i64)>;

/// The use of a pod by a run that started at `start` and ended at `end` (or lives at `end`), given the daemon's reads
/// of the pod while it lived, in order, each its instant and whether the pod was ready: the pod is ready at the start.
pub fn uses_of(start: i64, end: i64, reads: &[(i64, bool)]) -> Uses {
    let mut out = Vec::new();
    let mut open = Some(start);
    for &(at, ready) in reads {
        let at = at.clamp(start, end.max(start));
        match (open, ready) {
            (Some(from), false) => {
                if at > from {
                    out.push((from, at));
                }
                open = None;
            }
            (None, true) => open = Some(at),
            _ => {}
        }
    }
    if let Some(from) = open.filter(|from| end > *from) {
        out.push((from, end));
    }
    out
}

/// How many milliseconds `uses` holds within `[from, to)`.
pub fn length(uses: &Uses, from: i64, to: i64) -> i64 {
    uses.iter().map(|&(a, b)| (b.min(to) - a.max(from)).max(0)).sum()
}

/// Each run's share of the pod's time within `[from, to)`, in milliseconds: `uses` holds the runs in submit order, and
/// the result is in the same order. The shares add up to the time within `[from, to)` in which any run used the pod.
pub fn shares(uses: &[Uses], from: i64, to: i64) -> Vec<i64> {
    let mut cuts: Vec<i64> = uses.iter().flatten().flat_map(|&(a, b)| [a, b]).chain([from, to]).filter(|t| (from..=to).contains(t)).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = vec![0i64; uses.len()];
    for piece in cuts.windows(2) {
        let (a, b) = (piece[0], piece[1]);
        let on: Vec<usize> = (0..uses.len()).filter(|&i| uses[i].iter().any(|&(x, y)| x <= a && b <= y)).collect();
        let Ok(n) = i64::try_from(on.len()) else { continue };
        if n == 0 {
            continue;
        }
        let (each, left) = ((b - a) / n, (b - a) % n);
        for (k, i) in (0i64..).zip(on) {
            out[i] += each + i64::from(k < left);
        }
    }
    out
}

/// What `millis` of a pod cost at `price_per_hour` micro-USD, in micro-USD, rounded up.
pub fn cost(millis: i64, price_per_hour: u64) -> u64 {
    let millis = u128::try_from(millis).unwrap_or(0);
    u64::try_from((millis * u128::from(price_per_hour)).div_ceil(3_600_000)).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn two_runs_that_share_30_minutes_at_3_usd_an_hour_book_0_75_usd_each() {
        let both = vec![vec![(0, 30 * MIN)], vec![(0, 30 * MIN)]];
        let s = shares(&both, 0, 30 * MIN);
        assert_eq!(s, vec![15 * MIN, 15 * MIN]);
        assert_eq!((cost(s[0], 3_000_000), cost(s[1], 3_000_000)), (750_000, 750_000));
    }

    #[test]
    fn a_millisecond_that_does_not_split_goes_to_the_first_submitted_and_the_shares_add_up() {
        let three = vec![vec![(0, 10)], vec![(0, 10)], vec![(0, 10)]];
        assert_eq!(shares(&three, 0, 10), vec![4, 3, 3]);
        let apart = vec![vec![(0, 7)], vec![(3, 12)]];
        let s = shares(&apart, 0, 12);
        assert_eq!(s, vec![3 + 2, 2 + 5]);
        assert_eq!(s.iter().sum::<i64>(), 12);
        // a window cuts the time it counts
        assert_eq!(shares(&apart, 5, 10), vec![1, 4]);
    }

    #[test]
    fn a_read_of_a_pod_not_ready_ends_a_use_and_a_read_ready_opens_one_again() {
        assert_eq!(uses_of(0, 100, &[]), vec![(0, 100)]);
        assert_eq!(uses_of(0, 100, &[(20, false), (40, false), (60, true)]), vec![(0, 20), (60, 100)]);
        assert_eq!(uses_of(10, 100, &[(5, false)]), Vec::<(i64, i64)>::new());
        assert_eq!(length(&vec![(0, 20), (60, 100)], 10, 70), 20);
        assert_eq!(cost(1, 3_600_000), 1);
        assert_eq!(cost(1, 1), 1, "a part of a micro-USD is rounded up");
        assert_eq!(cost(0, 3_000_000), 0);
    }
}
