use pager::page_count;

#[test]
fn ten_items_make_two_pages() {
    assert_eq!(page_count(10, 5), 2);
}
