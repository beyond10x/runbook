// The cache of reads in runfiles.mjs (readText): what it holds is bounded in memory, and it is never a way around the
// run-path check. In-process, over invented files; no server, no network.
//   - the memory it holds stays under the one named bound (CACHE_BOUND): entries and their overhead count, a text with a
//     character above U+00FF counts two bytes per character, the least recently used entry goes first, a file over the
//     per-file cap is not kept;
//   - an entry whose file is gone, refused or changed leaves when it is seen;
//   - a path that was read and then becomes a link out, a second name, a directory, a pipe or is gone is refused, whatever
//     the cache holds; a limit smaller than the file never returns more than it; a file that grew is not served cut;
//   - the static route's root is checked with a separator (servedUnder).
import {test, expect} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import v8 from 'node:v8';
import vm from 'node:vm';
import {fixtureRoot, put} from './security-products-lib.mjs';
import {registerRun, readText, readJson, readCount, servedUnder, CACHE_BOUND, cacheStats, cacheClear} from '../runfiles.mjs';

const root = fixtureRoot('runfiles-cache');
const MiB = 1048576;
const SECRET = 'OUTSIDE-CACHE-7f3a';
const fresh = (name) => { const d = path.join(root, name); fs.rmSync(d, {recursive: true, force: true}); fs.mkdirSync(d, {recursive: true}); registerRun(d); return d; };
// the memory in use after a collection (strings live on the heap)
const gc = (() => { v8.setFlagsFromString('--expose_gc'); return vm.runInNewContext('gc'); })();
const heap = () => { gc(); gc(); const m = process.memoryUsage(); return m.heapUsed + m.external; };
const mb = (n) => (n / MiB).toFixed(1);

test.beforeEach(({}, info) => { test.skip(info.project.name !== 'wide', 'in-process, one viewport'); cacheClear(); });
test.afterAll(() => { fs.rmSync(root, {recursive: true, force: true}); });

// ---- the bound is memory --------------------------------------------------------------------------------------------
test('cache-count-bound: files that add no content (empty, long names) are held under the named bound, not without end', () => {
  test.setTimeout(60000);
  const dir = fresh('count');
  // a path of about 3 700 characters: the key and the real path of an entry are long strings
  let deep = dir;
  for (let i = 0; i < 14; i++) deep = path.join(deep, `${'d'.repeat(248)}${String(i).padStart(2, '0')}`);
  fs.mkdirSync(deep, {recursive: true});
  const N = 3000;
  for (let i = 0; i < N; i++) fs.writeFileSync(path.join(deep, `f${i}.json`), '');
  const h0 = heap();
  for (let i = 0; i < N; i++) readText(path.join(deep, `f${i}.json`));
  const held = heap() - h0;
  console.log(`cache count bound: ${N} empty files with paths of ${deep.length} characters hold ${mb(held)} MiB (bound ${mb(CACHE_BOUND)} MiB); ${cacheStats().entries} entries`);
  expect(held, `the cache holds ${mb(held)} MiB for a named bound of ${mb(CACHE_BOUND)} MiB`).toBeLessThan(CACHE_BOUND);
  expect(cacheStats().held).toBeLessThanOrEqual(CACHE_BOUND);
  expect(cacheStats().entries, 'entries held').toBeLessThan(N);
});

test('cache-accounting-is-not-below-memory: what the cache counts for an entry with a short path and no content is at least the memory it takes', () => {
  const dir = fresh('overhead');
  const N = 3000;
  for (let i = 0; i < N; i++) fs.writeFileSync(path.join(dir, `f${i}.json`), '');
  const h0 = heap();
  for (let i = 0; i < N; i++) readText(path.join(dir, `f${i}.json`));
  const held = heap() - h0;
  const {entries, held: counted} = cacheStats();
  console.log(`cache accounting: ${entries} empty files with short paths: counted ${(counted / entries).toFixed(0)} bytes each, memory ${(held / entries).toFixed(0)} bytes each`);
  expect(entries).toBe(N);
  expect(counted, 'the bound counts less than the memory an entry takes: the entry itself (its map slot and record) is left out').toBeGreaterThanOrEqual(held);
});

test('cache-bytes-bound: files with one character above U+00FF hold no more memory than the named bound', () => {
  test.setTimeout(60000);
  const dir = fresh('bytes');
  // ASCII but for one euro sign: the text is two bytes per character in memory, twice the size of the file
  const euro = (size) => 'a'.repeat(size - 3) + '€';
  for (let i = 0; i < 5; i++) fs.writeFileSync(path.join(dir, `mid${i}.txt`), euro(3 * MiB));
  for (let i = 0; i < 3; i++) fs.writeFileSync(path.join(dir, `big${i}.txt`), euro(8 * MiB));
  expect(fs.statSync(path.join(dir, 'big0.txt')).size).toBe(8 * MiB);
  const h0 = heap();
  for (let i = 0; i < 5; i++) expect(readText(path.join(dir, `mid${i}.txt`))?.length).toBe(3 * MiB - 2);
  const mid = heap() - h0;
  // 8 MiB files cost 16 MiB each in memory: more than the bound allows for one
  for (let i = 0; i < 3; i++) expect(readText(path.join(dir, `big${i}.txt`))?.length).toBe(8 * MiB - 2);
  const held = heap() - h0;
  console.log(`cache bytes bound: 5 files of 3 MiB hold ${mb(mid)} MiB, then 3 files of 8 MiB: ${mb(held)} MiB (bound ${mb(CACHE_BOUND)} MiB)`);
  expect(mid, `5 files of 3 MiB: held ${mb(mid)} MiB for a named bound of ${mb(CACHE_BOUND)} MiB`).toBeLessThan(CACHE_BOUND);
  expect(held, `then 3 files of 8 MiB: held ${mb(held)} MiB for a named bound of ${mb(CACHE_BOUND)} MiB`).toBeLessThan(CACHE_BOUND);
  expect(cacheStats().held).toBeLessThanOrEqual(CACHE_BOUND);
});

test('cache-evicts-the-least-recently-used: past the bound the oldest use goes first, a file just read again stays', () => {
  test.setTimeout(60000);
  const dir = fresh('lru');
  const f = (i) => path.join(dir, `f${i}.txt`);
  // three files of 5 MiB fit under the bound, a fourth does not
  for (let i = 0; i < 4; i++) fs.writeFileSync(f(i), 'a'.repeat(5 * MiB));
  readCount.on = true;
  try {
    const reads = () => readCount.total;
    for (let i = 0; i < 3; i++) readText(f(i));
    const afterThree = reads();
    expect(cacheStats().entries).toBe(3);
    readText(f(0));
    expect(reads(), 'a hit reads nothing').toBe(afterThree);
    readText(f(3));
    expect(reads()).toBe(afterThree + 1);
    expect(cacheStats().held, 'past the bound: evicted').toBeLessThanOrEqual(CACHE_BOUND);
    expect(cacheStats().entries).toBe(3);
    const before = reads();
    readText(f(0));
    expect(reads(), 'the file read again a moment ago is still held').toBe(before);
    readText(f(1));
    expect(reads(), 'the oldest use was evicted: read again').toBe(before + 1);
  } finally { readCount.on = false; }
});

test('cache-file-cap: a file over 8 MiB is never kept, one of 8 MiB is', () => {
  test.setTimeout(60000);
  const dir = fresh('cap');
  fs.writeFileSync(path.join(dir, 'over.txt'), 'a'.repeat(8 * MiB + 1));
  fs.writeFileSync(path.join(dir, 'at.txt'), 'a'.repeat(8 * MiB));
  expect(readText(path.join(dir, 'over.txt'))?.length).toBe(8 * MiB + 1);
  expect(cacheStats().entries, 'a file over the cap is not kept').toBe(0);
  expect(readText(path.join(dir, 'at.txt'))?.length).toBe(8 * MiB);
  expect(cacheStats().entries).toBe(1);
  expect(cacheStats().held).toBeGreaterThanOrEqual(8 * MiB);
});

test('cache-entry-leaves: an entry whose file is gone, refused or changed leaves when a call sees it', () => {
  const dir = fresh('leaves');
  const out = fresh('leaves-out');
  const held = () => cacheStats().entries;
  fs.writeFileSync(path.join(dir, 'a.json'), '{"a":1}');
  fs.writeFileSync(path.join(dir, 'b.json'), '{"b":1}');
  fs.writeFileSync(path.join(dir, 'c.json'), '{"c":1}');
  for (const n of ['a', 'b', 'c']) readText(path.join(dir, `${n}.json`));
  expect(held()).toBe(3);
  fs.rmSync(path.join(dir, 'a.json'));
  expect(readText(path.join(dir, 'a.json'))).toBeNull();
  expect(held(), 'a file that is gone').toBe(2);
  fs.rmSync(path.join(dir, 'b.json'));
  fs.symlinkSync(path.join(out, 'x.json'), path.join(dir, 'b.json'));
  expect(readText(path.join(dir, 'b.json'))).toBeNull();
  expect(held(), 'a path that became a link out').toBe(1);
  fs.linkSync(path.join(dir, 'c.json'), path.join(out, 'c-alias.json'));
  expect(readText(path.join(dir, 'c.json'))).toBeNull();
  expect(held(), 'a file that got a second name').toBe(0);
});

// ---- the cache is never a way around the checks ----------------------------------------------------------------------
test('cache-guard-after-first-read: a cached path that becomes a link out, a second name, a directory, a pipe or is gone is refused', () => {
  const dir = path.join(root, 'inproc');
  fs.rmSync(dir, {recursive: true, force: true});
  const r = path.join(dir, 'run'), out = path.join(dir, 'out');
  fs.mkdirSync(path.join(r, 'sub'), {recursive: true}); fs.mkdirSync(out, {recursive: true});
  registerRun(r);
  const stamp = new Date('2026-11-21T10:00:00Z');
  const make = (f, text) => { fs.mkdirSync(path.dirname(f), {recursive: true}); fs.writeFileSync(f, text); fs.utimesSync(f, stamp, stamp); };
  // a directory on the way becomes a link to a directory outside holding a file of the same name, size and time
  make(path.join(r, 'sub', 'a.json'), '{"v":"INSIDE-AAAA-0000"}');
  expect(readJson(path.join(r, 'sub', 'a.json')).v).toBe('INSIDE-AAAA-0000');
  make(path.join(out, 'a.json'), `{"v":"${SECRET}"}`);
  fs.renameSync(path.join(r, 'sub'), path.join(r, 'sub.old'));
  fs.symlinkSync(out, path.join(r, 'sub'));
  expect(readText(path.join(r, 'sub', 'a.json'))).toBeNull();
  // a file replaced by a hard link of an outside file, size and time kept
  make(path.join(r, 'h.json'), '{"v":"INSIDE-HHHH-0000"}');
  expect(readJson(path.join(r, 'h.json')).v).toBe('INSIDE-HHHH-0000');
  fs.rmSync(path.join(r, 'h.json'));
  fs.linkSync(path.join(out, 'a.json'), path.join(r, 'h.json'));
  expect(readText(path.join(r, 'h.json'))).toBeNull();
  // the second name of an inside file appears and then goes: refused while it is there, read again after
  make(path.join(r, 'n.json'), '{"v":"NNNN"}');
  expect(readJson(path.join(r, 'n.json')).v).toBe('NNNN');
  fs.linkSync(path.join(r, 'n.json'), path.join(out, 'n-alias.json'));
  expect(readText(path.join(r, 'n.json'))).toBeNull();
  fs.rmSync(path.join(out, 'n-alias.json'));
  expect(readJson(path.join(r, 'n.json')).v).toBe('NNNN');
  // a file replaced by a directory, and by a named pipe
  make(path.join(r, 'd.json'), '{"v":"DDDD"}');
  expect(readJson(path.join(r, 'd.json')).v).toBe('DDDD');
  fs.rmSync(path.join(r, 'd.json')); fs.mkdirSync(path.join(r, 'd.json'));
  expect(readText(path.join(r, 'd.json'))).toBeNull();
  make(path.join(r, 'p.json'), '{"v":"PPPP"}');
  expect(readJson(path.join(r, 'p.json')).v).toBe('PPPP');
  fs.rmSync(path.join(r, 'p.json'));
  execFileSync('mkfifo', [path.join(r, 'p.json')]);
  expect(readText(path.join(r, 'p.json'))).toBeNull();
  // created after the first (failed) read: a link out, then a regular file
  expect(readText(path.join(r, 'c.json'))).toBeNull();
  fs.symlinkSync(path.join(out, 'a.json'), path.join(r, 'c.json'));
  expect(readText(path.join(r, 'c.json'))).toBeNull();
  fs.rmSync(path.join(r, 'c.json')); make(path.join(r, 'c.json'), '{"v":"CCCC"}');
  expect(readJson(path.join(r, 'c.json')).v).toBe('CCCC');
  // a link inside the run to a file inside it is followed; retargeted out it is not
  make(path.join(r, 'target.json'), '{"v":"TTTT"}');
  fs.symlinkSync(path.join(r, 'target.json'), path.join(r, 'l.json'));
  expect(readJson(path.join(r, 'l.json')).v).toBe('TTTT');
  fs.rmSync(path.join(r, 'l.json')); fs.symlinkSync(path.join(out, 'a.json'), path.join(r, 'l.json'));
  expect(readText(path.join(r, 'l.json'))).toBeNull();
  // deleted, then recreated as a link out
  make(path.join(r, 'g.json'), '{"v":"GGGG"}');
  expect(readJson(path.join(r, 'g.json')).v).toBe('GGGG');
  fs.rmSync(path.join(r, 'g.json'));
  expect(readText(path.join(r, 'g.json'))).toBeNull();
  fs.symlinkSync(path.join(out, 'a.json'), path.join(r, 'g.json'));
  expect(readText(path.join(r, 'g.json'))).toBeNull();
});

test('cache-guard-no-run: a path that lies under no registered run is not read, with a cache of reads in the module', () => {
  const dir = fresh('no-run');
  expect(readText('/etc/hostname')).toBeNull();
  const f = path.join(root, 'not-a-run.txt');
  expect(readText(f)).toBeNull();
  fs.writeFileSync(f, 'x');
  expect(readText(f)).toBeNull();
  fs.writeFileSync(path.join(dir, 'in.txt'), 'in');
  expect(readText(path.join(dir, 'in.txt'))).toBe('in');
});

test('cache-guard-limit-and-growth: a limit smaller than the file never returns more than it, a grown file is not served cut', () => {
  const dir = fresh('limit');
  const f = path.join(dir, 'x.txt');
  fs.writeFileSync(f, 'a'.repeat(100));
  expect(readText(f)?.length).toBe(100);
  expect(readText(f, undefined, 50)).toBeNull();
  expect(readText(f, undefined, 99)).toBeNull();
  expect(readText(f, undefined, 100)?.length).toBe(100);
  fs.appendFileSync(f, 'b'.repeat(20));
  expect(readText(f)?.length).toBe(120);
  expect(readText(f, undefined, 110)).toBeNull();
  fs.truncateSync(f, 10);
  expect(readText(f)?.length).toBe(10);
  fs.writeFileSync(f, 'é'.repeat(20000));
  expect(Buffer.byteLength(readText(f))).toBe(40000);
  expect(readText(f, undefined, 39999)).toBeNull();
});

test('cache-guard-same-size-rewrite: an in-place write of the same size read at once is never served stale (300 rounds, then with the old mtime put back)', () => {
  const dir = fresh('rewrite');
  const f = path.join(dir, 'w.txt');
  fs.writeFileSync(f, '0000');
  const stale = [];
  for (let i = 1; i <= 300; i++) {
    const text = String(1000 + i);
    const fd = fs.openSync(f, 'r+'); fs.writeSync(fd, text, 0); fs.closeSync(fd);
    const got = readText(f);
    if (got !== text) stale.push([i, got, text]);
    const fd2 = fs.openSync(f, 'r+'); fs.writeSync(fd2, String(2000 + i), 0); fs.closeSync(fd2);
    const st = fs.statSync(f);
    const again = readText(f);
    if (again !== String(2000 + i)) stale.push([i, 'second', again]);
    fs.utimesSync(f, st.atime, st.mtime);
  }
  expect(stale.slice(0, 3), `stale reads: ${stale.length} of 600`).toEqual([]);
  fs.writeFileSync(f, 'AAAA'); const t0 = fs.statSync(f).mtime;
  expect(readText(f)).toBe('AAAA');
  fs.writeFileSync(f, 'BBBB'); fs.utimesSync(f, t0, t0);
  expect(readText(f)).toBe('BBBB');
});

// ---- the static route's root --------------------------------------------------------------------------------------------
test('static-root-has-a-separator: a sibling directory whose name starts with the root\'s name is outside it', () => {
  const dir = fresh('static');
  put(path.join(dir, 'web', 'index.html'), '<p>in</p>');
  put(path.join(dir, 'web-extra', 'secret.txt'), SECRET);
  const web = path.join(dir, 'web');
  expect(servedUnder(web, 'index.html')).toBe(path.join(web, 'index.html'));
  expect(servedUnder(web, 'sub/../index.html')).toBe(path.join(web, 'index.html'));
  for (const rel of ['../web-extra/secret.txt', 'sub/../../web-extra/secret.txt', '../web2', '..', '../']) {
    expect(servedUnder(web, rel), rel).toBeNull();
  }
  expect(fs.existsSync(path.join(dir, 'web-extra', 'secret.txt'))).toBe(true);
});
