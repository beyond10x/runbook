//! Pagination over a list of items.

/// The number of pages needed to show `total` items, `per_page` on each page; 0 when there is
/// nothing to show or `per_page` is 0.
pub fn page_count(total: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 0;
    }
    total.div_ceil(per_page)
}

/// The items on page `page` (numbered from 1); empty when that page does not exist.
pub fn page<T>(items: &[T], page: usize, per_page: usize) -> &[T] {
    if page == 0 || page > page_count(items.len(), per_page) {
        return &[];
    }
    let start = (page - 1) * per_page;
    let end = (start + per_page).min(items.len());
    &items[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_pages() {
        assert_eq!(page_count(10, 5), 2);
    }

    #[test]
    fn no_items() {
        assert_eq!(page_count(0, 5), 0);
    }

    #[test]
    fn zero_per_page() {
        assert_eq!(page_count(10, 0), 0);
    }

    #[test]
    fn first_page() {
        let items: Vec<u32> = (1..=10).collect();
        assert_eq!(page(&items, 1, 5), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn page_zero_is_empty() {
        let items: Vec<u32> = (1..=10).collect();
        assert!(page(&items, 0, 5).is_empty());
    }
}
