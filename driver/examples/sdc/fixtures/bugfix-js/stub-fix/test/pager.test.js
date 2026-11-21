import { test } from 'node:test';
import assert from 'node:assert/strict';
import { page, pageCount } from '../src/pager.js';

const ten = Array.from({ length: 10 }, (_, i) => i + 1);

test('full pages', () => {
  assert.equal(pageCount(10, 5), 2);
});

test('no items', () => {
  assert.equal(pageCount(0, 5), 0);
});

test('zero per page', () => {
  assert.equal(pageCount(10, 0), 0);
});

test('first page', () => {
  assert.deepEqual(page(ten, 1, 5), [1, 2, 3, 4, 5]);
});

test('page zero is empty', () => {
  assert.deepEqual(page(ten, 0, 5), []);
});
