// The read of a file of a run refuses what it cannot verify (story inspector-shows-step-products, fix round 2).
// The kernel's name for an open descriptor is what readRegular compares with the path it checked; where the system cannot
// give that name (no /proc) the file is not read, rather than compared with a path computed again. The name is a seam of
// readRegular (`nameOf`) so that the branch can be run on this platform. The same module: a file over the limit of readText is
// not read, `accept` is asked about the descriptor's own path before a byte is read, and a work/ that is a link is no root.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {readRegular, readText} from '../runfiles.mjs';
import {scopeOf, refusalOf} from '../products.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('runfiles-fail-closed');
const run = path.join(root, 'run');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));
test.beforeAll(() => {
  fs.rmSync(root, {recursive: true, force: true});
  put(path.join(run, 'work', 'a.txt'), 'inside the run\n');
  put(path.join(run, 'store', 'b.txt'), 'store text\n');
});
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

test('a descriptor that the system cannot name is not read', () => {
  const ok = readRegular(path.join(run, 'work', 'a.txt'), {root: run});
  expect(ok.state).toBe('ok');
  const blind = readRegular(path.join(run, 'work', 'a.txt'), {root: run, nameOf: () => null});
  expect(blind).toMatchObject({state: 'refused', why: 'unverified'});
  expect(blind.bytes).toBeUndefined();
});

test('a descriptor whose name is another file is not read, and accept is asked about the descriptor, before a byte is read', () => {
  const other = readRegular(path.join(run, 'work', 'a.txt'), {root: run, nameOf: () => path.join(run, 'store', 'b.txt')});
  expect(other.state).toBe('missing');
  const asked = [];
  const refused = readRegular(path.join(run, 'work', 'a.txt'), {root: run, accept: (real) => { asked.push(real); return 'no'; }});
  expect(refused).toMatchObject({state: 'refused', why: 'not served', reason: 'no'});
  expect(asked).toEqual([fs.realpathSync(path.join(run, 'work', 'a.txt'))]);
  expect(refused.bytes).toBeUndefined();
});

test('a file over the limit of readText is not read', () => {
  expect(readText(path.join(run, 'work', 'a.txt'), run)).toBe('inside the run\n');
  expect(readText(path.join(run, 'work', 'a.txt'), run, 5)).toBeNull();
});

test('a file with more than one name is not read as text, whoever asks', () => {
  const two = path.join(root, 'two');
  put(path.join(two, 'one.txt'), 'two names\n');
  fs.linkSync(path.join(two, 'one.txt'), path.join(two, 'other.txt'));
  expect(readText(path.join(two, 'one.txt'), two)).toBeNull();
  expect(readText(path.join(two, 'other.txt'), two)).toBeNull();
  put(path.join(two, 'single.txt'), 'one name\n');
  expect(readText(path.join(two, 'single.txt'), two)).toBe('one name\n');
});

test('a work/ that is a link is no root: only a real work/ directory of the run is served', () => {
  const linked = path.join(root, 'linked');
  fs.mkdirSync(path.join(linked, 'store'), {recursive: true});
  put(path.join(linked, 'store', 'c.txt'), 'c\n');
  fs.symlinkSync('store', path.join(linked, 'work'));
  expect(scopeOf(linked, [path.join(linked, 'work')]).roots).toEqual([]);
  const scope = scopeOf(run, [path.join(run, 'work')]);
  expect(scope.roots.map((r) => path.basename(r.real))).toEqual(['work']);
  expect(refusalOf(scope, fs.realpathSync(path.join(run, 'work', 'a.txt')))).toBeNull();
  expect(refusalOf(scope, fs.realpathSync(path.join(run, 'store', 'b.txt')))).toMatch(/work\//);
});
