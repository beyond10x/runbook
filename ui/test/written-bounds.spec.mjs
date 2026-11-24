// The bounds of /api/products against the records a run wrote (fs/changes.jsonl, seeded.json, seeded-files.jsonl), as integrity
// checks of the unit's own contract (ui/README.md "What a step produced"): what a record names is checked before anything of
// the host is looked at, every string of a record that enters the answer has a length bound and the answer says when it cut one,
// a reader that stops before the end of the change record says so, and what the server holds for one run is bounded.
// Own servers on ports the kernel gives; run directories written by hand with invented text. No browser, no model, no network.
// The cases after "guards that stand" hold what the tab must not do with a record that is not what the runner wrote.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeWrittenRun, change} from './written-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(FIXTURE_DIR, 'written-bounds');
const SECRET = 'OUTSIDE-THE-RUN-SECRET-b04d';
const servers = {};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => Object.values(servers).forEach((s) => s.proc.kill()));

const make = async (name, opts, after = () => {}, env = process.env) => {
  const dir = path.join(root, name, 'run');
  writeWrittenRun(dir, opts);
  after(dir);
  servers[name] = await startServer(['--no-view', '--runs', path.join(root, name)], {env});
  return dir;
};
const get = async (name, p) => {
  const t = Date.now();
  const r = await fetch(servers[name].url + p);
  const text = await r.text();
  let json = null;
  try { json = JSON.parse(text); } catch { /* not JSON */ }
  return {status: r.status, text, json, ms: Date.now() - t};
};
const products = (name) => get(name, '/api/products?source=0');
const append = (dir, rows) => fs.appendFileSync(path.join(dir, 'fs', 'changes.jsonl'), rows.map((r) => (typeof r === 'string' ? r : JSON.stringify(r))).join('\n') + '\n');
const put = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, text); };

test.beforeAll(() => { fs.rmSync(root, {recursive: true, force: true}); fs.mkdirSync(root, {recursive: true}); });

// ---- a record is checked before the host is looked at --------------------------------------------------------

test('a record that names a file behind a link out of the run is answered the same whether or not that file exists', async () => {
  test.setTimeout(60000);
  const outside = path.join(root, 'oracle-outside');
  fs.mkdirSync(outside, {recursive: true});
  fs.writeFileSync(path.join(outside, 'exists.txt'), `${SECRET}\n`);
  await make('oracle', {extra: []}, (d) => {
    fs.symlinkSync(outside, path.join(d, 'work/link-out'));
    append(d, [change(900, 'link-out/exists.txt', {text: 'x\n', step: 3}), change(901, 'link-out/absent.txt', {text: 'x\n', step: 3}), change(902, 'link-out/deeper/absent.txt', {text: 'x\n', step: 3})]);
  });
  const r = await products('oracle');
  expect(r.json.basis).toBe('records');
  // every name is behind a link that leaves the run: one answer for all, and none says whether a host file is there
  expect({gone: r.json.gone, leftOut: r.json.leftOut}).toEqual({gone: 0, leftOut: 3});
  expect(r.text).not.toContain(SECRET);
});

test('a name that lies in the run and is not there is gone, a name behind a link that stays in the run is described, a dangling link is refused', async () => {
  test.setTimeout(60000);
  await make('inside', {extra: []}, (d) => {
    put(path.join(d, 'work/real/f.txt'), 'f\n');
    fs.symlinkSync('real', path.join(d, 'work/ln-real'));
    fs.symlinkSync('nowhere', path.join(d, 'work/dangling'));
    append(d, [change(900, 'real/f.txt', {text: 'f\n', step: 3}), change(901, 'real/absent.txt', {text: 'x\n', step: 3}), change(902, 'ln-real/f.txt', {text: 'f\n', step: 4}),
      change(903, 'dangling/f.txt', {text: 'x\n', step: 4}), change(904, 'f.txt/below.txt', {text: 'x\n', step: 4})]);
    put(path.join(d, 'work/f.txt'), 'f\n');
  });
  const r = await products('inside');
  const names = r.json.files.map((f) => f.path);
  expect(names).toContain('work/real/f.txt');
  expect(names).toContain('work/ln-real/f.txt');
  expect(names).not.toContain('work/real/absent.txt');
  expect({gone: r.json.gone, leftOut: r.json.leftOut}).toEqual({gone: 2, leftOut: 1});
});

// ---- every string of a record that enters the answer has a bound -----------------------------------------------

test('an action name of a record is cut at 256 characters, the answer says it cut one, and it is not as long as the record', async () => {
  test.setTimeout(60000);
  const written = [];
  for (let i = 0; i < 8; i++) written.push({step: 10 + i, path: `work/gen/g${i}.txt`, text: `g${i}\n`, op: 'create', action: `${'A'.repeat(1 << 20)}-${i}`});
  written.push({step: 30, path: 'work/gen/short.txt', text: 's\n', op: 'create', action: 'a.short'});
  await make('longstring', {written});
  const r = await products('longstring');
  expect(r.json.basis).toBe('records');
  expect(r.text.length).toBeLessThan(1 << 18);
  expect(r.json.cutStrings).toBe(8);
  expect(r.json.maxAction).toBe(256);
  expect(r.json.truncated).toBe(true);
  expect(r.json.cut).toBe('strings');
  const actions = r.json.groups.flatMap((g) => g.actions);
  expect(actions.every((a) => a.length <= 256)).toBe(true);
  expect(actions).toContain('a.short');
});

test('a path of a record longer than the bound is counted, never listed, and the answer stays small', async () => {
  test.setTimeout(60000);
  const ok = `${'c'.repeat(245) + '/'}`.repeat(4) + 'x.txt';
  await make('longpath', {written: []}, (d) => {
    append(d, [change(900, `${'a/'.repeat(600)}x.txt`, {text: 'x\n', step: 3}), change(901, `${('b'.repeat(200) + '/').repeat(6)}x.txt`, {text: 'x\n', step: 3}), change(902, ok, {text: 'x\n', step: 3})]);
    put(path.join(d, 'work', ok), 'x\n');
  });
  const r = await products('longpath');
  expect(r.json.refusedRecords).toBe(2);
  expect(r.json.files.map((f) => f.path)).toEqual([`work/${ok}`]);
});

// ---- a reader that stops before the end of the change record says so --------------------------------------------

test('a line of the change record longer than what the server reads is said, with the records read before it, not dropped silently', async () => {
  test.setTimeout(60000);
  await make('longline', {written: []}, (d) => {
    put(path.join(d, 'work/before.txt'), 'b\n');
    put(path.join(d, 'work/after.txt'), 'a\n');
    const big = JSON.stringify({...change(901, 'long.txt', {text: 'l\n', step: 2}), note: 'N'.repeat(33 * 1048576)});
    append(d, [change(900, 'before.txt', {text: 'b\n', step: 2}), big, change(902, 'after.txt', {text: 'a\n', step: 3})]);
  });
  const r = await products('longline');
  expect(r.json.basis).toBe('records');
  const names = r.json.files.map((f) => f.path);
  expect(names).toContain('work/before.txt');
  expect(r.json.truncated, JSON.stringify({names})).toBe(true);
  expect(r.json.cut).toBe('records');
  expect(r.json.recordsStopped).toMatchObject({cause: 'line', read: 202}); // the 201 records of the seed and before.txt
  expect(r.json.recordsStopped.why).toContain('after it are not read');
  // asked again, with the file as it was: the same answer, and the reader does not read the 32 MiB again
  const again = await products('longline');
  expect(again.json.recordsStopped).toEqual(r.json.recordsStopped);
});

test('a change record of 1 GB with no line the server can read answers the listing and why, not "no action has written a file yet"', async () => {
  test.setTimeout(60000);
  await make('unreadable', {}, (d) => {
    const f = path.join(d, 'fs/changes.jsonl');
    fs.rmSync(f, {force: true});
    const fd = fs.openSync(f, 'w');
    fs.ftruncateSync(fd, 1 << 30);
    fs.closeSync(fd);
  });
  const r = await products('unreadable');
  expect(r.json.basis).toBe('listing');
  expect(r.json.basisNote).toContain('cannot be read');
  expect(r.json.files.length).toBeGreaterThan(0);
});

// ---- what the server holds for one run is bounded, and the state it keeps is not a way around the checks -------

test('past the records the server keeps for one run the answer says it did not read the rest; a second answer is the same', async () => {
  test.setTimeout(60000);
  const N = 3000;
  await make('records', {written: []}, (d) => {
    fs.mkdirSync(path.join(d, 'work/bulk'), {recursive: true});
    const lines = [];
    for (let i = 0; i < N; i++) {
      lines.push(JSON.stringify(change(1000 + i, `bulk/f${i}.txt`, {text: `${i}\n`, step: 10 + i, action: 'a.b'})));
      if (i >= 1200) fs.writeFileSync(path.join(d, `work/bulk/f${i}.txt`), `${i}\n`);
    }
    append(d, lines);
  }, {...process.env, SDC_UI_MAX_RECORDS: '2000'});
  const first = await products('records');
  const second = await products('records');
  for (const r of [first, second]) {
    expect(r.json.basis).toBe('records');
    expect(r.json.groups.length).toBeLessThanOrEqual(500);
    expect(r.json.truncated).toBe(true);
    expect(r.json.recordsStopped).toMatchObject({cause: 'records', read: 2000});
  }
  // the 201 records of the seed and 1799 of the 3000 are read: the newest product listed is the last one read
  expect(first.json.files[0].path).toBe('work/bulk/f1798.txt');
  expect(first.json.files.length).toBe(500);
  expect(second.json.files).toEqual(first.json.files);
});

test('an answer kept for a run lists a file only while it is there: a file removed or swapped for a link after it was kept is checked again', async () => {
  test.setTimeout(60000);
  const outside = path.join(root, 'kept-outside.txt');
  fs.writeFileSync(outside, `${SECRET}\n`);
  const dir = await make('kept', {extra: []}, (d) => {
    put(path.join(d, 'work/a.txt'), 'a\n');
    put(path.join(d, 'work/b.txt'), 'b\n');
    put(path.join(d, 'work/c.txt'), 'c\n');
    append(d, ['a.txt', 'b.txt', 'c.txt'].map((p, i) => change(900 + i, p, {text: `${p[0]}\n`, step: 3})));
  });
  const first = await products('kept');
  expect(first.json.files.map((f) => f.path)).toEqual(expect.arrayContaining(['work/a.txt', 'work/b.txt', 'work/c.txt']));
  fs.rmSync(path.join(dir, 'work/a.txt'));
  fs.rmSync(path.join(dir, 'work/b.txt'));
  fs.symlinkSync(outside, path.join(dir, 'work/b.txt'));
  const second = await products('kept');
  const names = second.json.files.map((f) => f.path);
  expect(names).toContain('work/c.txt');
  expect(names).not.toContain('work/a.txt');
  expect(names).not.toContain('work/b.txt');
  expect({gone: second.json.gone, leftOut: second.json.leftOut}).toEqual({gone: 1, leftOut: 1});
  expect(second.text).not.toContain(SECRET);
  // a record that arrives later is listed by the answer that follows it
  put(path.join(dir, 'work/d.txt'), 'd\n');
  append(dir, [change(950, 'd.txt', {text: 'd\n', step: 4})]);
  expect((await products('kept')).json.files.map((f) => f.path)).toContain('work/d.txt');
});

// ---- guards that stand ------------------------------------------------------------------------------------

const fifo = (p) => execFileSync('mkfifo', [p]);
const sparse = (p, size) => { const fd = fs.openSync(p, 'w'); fs.ftruncateSync(fd, size); fs.closeSync(fd); };

test('records that are not plain files or not as the runner wrote them are never opened blind, and the answer is the listing with a reason: a record that is a named pipe, a directory, a second hard link or a 1 GB file is never opened blind, and the answer is the listing with a reason', async () => {
  test.setTimeout(60000);
  const outside = path.join(root, 'guard-outside.json');
  fs.writeFileSync(outside, JSON.stringify({format: 'runbook.seeded/1', complete: true, files: 0, entries: [], marker: SECRET}));
  const swap = (d, rel, how) => {
    const f = path.join(d, rel);
    fs.rmSync(f, {recursive: true, force: true});
    how(f);
  };
  const edit = (d, rel, how) => {
    const f = path.join(d, rel);
    const v = JSON.parse(fs.readFileSync(f, 'utf8'));
    how(v);
    fs.writeFileSync(f, JSON.stringify(v));
  };
  const variants = {
    'seeded-fifo': (d) => swap(d, 'seeded.json', fifo),
    'seeded-dir': (d) => swap(d, 'seeded.json', (f) => fs.mkdirSync(f)),
    'seeded-hardlink': (d) => swap(d, 'seeded.json', (f) => fs.linkSync(outside, f)),
    'seeded-big': (d) => swap(d, 'seeded.json', (f) => sparse(f, 1 << 30)),
    'list-fifo': (d) => swap(d, 'seeded-files.jsonl', fifo),
    'list-dir': (d) => swap(d, 'seeded-files.jsonl', (f) => fs.mkdirSync(f)),
    'list-big': (d) => swap(d, 'seeded-files.jsonl', (f) => sparse(f, 1 << 30)),
    'seeded-padded': (d) => fs.appendFileSync(path.join(d, 'seeded.json'), ' '.repeat(2 << 20)),
    'seeded-format9': (d) => edit(d, 'seeded.json', (v) => { v.format = 'runbook.seeded/9'; }),
    'seeded-entry-slash': (d) => edit(d, 'seeded.json', (v) => { v.entries.push({path: 'a/b', kind: 'dir', files: 0}); }),
    'seeded-entry-kind': (d) => edit(d, 'seeded.json', (v) => { v.entries.push({path: 'odd', kind: 'weird', files: 0}); }),
    'list-padded': (d) => fs.appendFileSync(path.join(d, 'seeded-files.jsonl'), '\n'.repeat(9 << 20)),
    'list-badsha': (d) => {
      const f = path.join(d, 'seeded-files.jsonl');
      const lines = fs.readFileSync(f, 'utf8').split('\n');
      lines[0] = JSON.stringify({...JSON.parse(lines[0]), sha256: 'not-a-sha'});
      fs.writeFileSync(f, lines.join('\n'));
    },
    'changes-fifo': (d) => swap(d, 'fs/changes.jsonl', fifo),
    'changes-dir': (d) => swap(d, 'fs/changes.jsonl', (f) => fs.mkdirSync(f)),
    'changes-hardlink': (d) => swap(d, 'fs/changes.jsonl', (f) => fs.linkSync(outside, f)),
  };
  await Promise.all(Object.entries(variants).map(([name, how]) => make(`g-${name}`, {}, how)));
  for (const name of Object.keys(variants)) {
    const r = await products(`g-${name}`);
    expect(r.status, name).toBe(200);
    expect(r.json.basis, name).toBe('listing');
    expect(r.json.basisNote, name).toBeTruthy();
    expect(r.text, name).not.toContain(SECRET);
    expect(r.text, name).not.toContain(root);
    expect(r.ms, name).toBeLessThan(15000);
  }
});

test('a link to a file of the run outside work/, to a store directory, to a dot-directory, a second hard link and a named pipe are neither listed nor opened', async () => {
  test.setTimeout(60000);
  const outside = path.join(root, 'guard-hard-outside.txt');
  fs.writeFileSync(outside, `${SECRET}\n`);
  await make('named', {extra: []}, (d) => {
    put(path.join(d, 'work/.hidden/f.txt'), 'h\n');
    fs.symlinkSync('../settings.json', path.join(d, 'work/ln-settings.txt'));
    fs.symlinkSync('../store', path.join(d, 'work/ln-store'));
    fs.symlinkSync('.hidden/f.txt', path.join(d, 'work/ln-dot.txt'));
    fs.linkSync(outside, path.join(d, 'work/hl.txt'));
    fifo(path.join(d, 'work/pipe.txt'));
    append(d, ['ln-settings.txt', 'ln-store/tree.json', 'ln-dot.txt', 'hl.txt', 'pipe.txt'].map((p, i) => change(900 + i, p, {text: 'x\n', step: 3})));
  });
  const r = await products('named');
  expect(r.json.basis).toBe('records');
  const names = r.json.files.map((f) => f.path);
  for (const n of ['ln-settings', 'ln-store', 'ln-dot', 'hl.txt', 'pipe']) expect(names.filter((p) => p.includes(n)), n).toEqual([]);
  expect(r.json.leftOut + r.json.gone).toBe(5);
  expect(r.text).not.toContain(SECRET);
  expect(r.ms).toBeLessThan(15000);
});

test('a path of 1 MB, a NUL, a backslash, a dot-part and an absolute path are counted or dropped, never listed, and the answer stays small', async () => {
  test.setTimeout(60000);
  await make('strings', {extra: []}, (d) => {
    append(d, [change(900, 'a'.repeat(1 << 20), {text: 'x\n', step: 3}), change(901, 'nul\u0000.txt', {text: 'x\n', step: 3}), change(902, 'back\\slash.txt', {text: 'x\n', step: 3}),
      change(903, '/etc/hostname', {text: 'x\n', step: 3}), change(904, 'a/../../b.txt', {text: 'x\n', step: 3}), change(905, 'x.txt', {text: 'x\n', step: 3, root: 'a'.repeat(1 << 20)})]);
  });
  const r = await products('strings');
  expect(r.json.basis).toBe('records');
  expect(r.json.total).toBe(5);
  expect(r.json.refusedRecords).toBe(5);
  expect(r.text.length).toBeLessThan(20000);
  for (const bad of ['hostname', 'slash.txt', 'nul\\u0000', 'nul.txt', '../']) expect(r.text, bad).not.toContain(bad);
});

test('the one parameter, source, takes a registered run or is refused with no path and no byte of a file', async () => {
  test.setTimeout(60000);
  const dir = await make('params', {extra: []});
  for (const q of ['source=abc', 'source=-1', 'source=9', 'source=1e1', 'source=', 'source=%00', 'source=../../etc', 'source=0&source=9', 'source=99999999999999999999', `source=${"9".repeat(5000)}`,
    'path=/etc/passwd&dir=..&source=0', 'case=../..&seq=1&source=0', `source=${encodeURIComponent(dir)}`]) {
    const r = await get('params', `/api/products?${q}`);
    expect([200, 404], q.slice(0, 40)).toContain(r.status);
    expect(r.text, q.slice(0, 40)).not.toContain(root);
    expect(r.text, q.slice(0, 40)).not.toContain('root:x');
  }
  // a run no server was given: none of the numbers reach another run
  expect((await get('params', '/api/products?source=1')).status).toBe(404);
});

test('the code that builds the Products tab from a record sets text only: no markup sink, no address, no image', () => {
  const text = fs.readFileSync(path.join(here, '..', 'web', 'products.js'), 'utf8');
  const start = text.indexOf('function recordLists(');
  const end = text.indexOf('// ---- the panel');
  expect(start).toBeGreaterThan(0);
  expect(end).toBeGreaterThan(start);
  const block = text.slice(start, end);
  for (const sink of ['innerHTML', 'outerHTML', 'insertAdjacentHTML', 'document.write', 'href', 'src', 'javascript:', 'data:', 'eval(', 'Function(', 'createContextualFragment', 'srcdoc', 'setAttribute']) expect(block, sink).not.toContain(sink);
});
