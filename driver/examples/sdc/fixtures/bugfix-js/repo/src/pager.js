// Pagination over a list of items.

// The number of pages needed to show `total` items, `perPage` on each page; 0 when there is
// nothing to show or `perPage` is 0.
export function pageCount(total, perPage) {
  if (perPage === 0) {
    return 0;
  }
  return Math.floor(total / perPage);
}

// The items on page `n` (numbered from 1); empty when that page does not exist.
export function page(items, n, perPage) {
  if (n === 0 || n > pageCount(items.length, perPage)) {
    return [];
  }
  const start = (n - 1) * perPage;
  return items.slice(start, start + perPage);
}
