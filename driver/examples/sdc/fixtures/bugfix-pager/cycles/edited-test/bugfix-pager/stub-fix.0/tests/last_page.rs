use pager::{page, page_count};

#[test]
fn last_partial_page_is_shown() {
    let items: Vec<u32> = (1..=11).collect();
    assert_eq!(page_count(11, 5), 3);
    assert!(page(&items, 3, 5).len() <= 1);
}
