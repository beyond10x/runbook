import { test } from 'node:test';
import assert from 'node:assert/strict';
import { page, pageCount } from '../src/pager.js';

test('last partial page is shown', () => {
  const items = Array.from({ length: 11 }, (_, i) => i + 1);
  assert.equal(pageCount(11, 5), 3);
  assert.deepEqual(page(items, 3, 5), [11]);
});
