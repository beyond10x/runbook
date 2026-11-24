// The reader of the change record behind /api/products, /api/live and /api/fschange, and the names it checks (story
// products-are-what-the-run-wrote): what the server keeps for one run is bounded by count and by bytes, every answer built from
// the kept list says when the list is not the whole record, an environment value that is no whole number of at least 1 is the
// default, a name is cut by characters, one answer resolves a bounded number of links per name, and the kept state follows the
// file it was read from. Own servers on ports the kernel gives; run directories written by hand with invented text. No browser
// except the one page case, no model, no network.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeWrittenRun, change} from './written-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
// each case makes its own run and server, so the cases run spread over the workers; each worker writes under a directory of
// its own, which its beforeAll empties
const root = path.join(FIXTURE_DIR, 'written-reader', `worker-${process.env.TEST_PARALLEL_INDEX ?? '0'}`);
const SECRET = 'OUTSIDE-THE-RUN-SECRET-91c7';
const servers = {};

test.describe.configure({mode: 'parallel'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => Object.values(servers).forEach((s) => s.proc.kill()));
test.beforeAll(() => { fs.rmSync(root, {recursive: true, force: true}); fs.mkdirSync(root, {recursive: true}); });

const make = async (name, opts, after = () => {}, env = process.env) => {
  const dir = path.join(root, name, 'run');
  writeWrittenRun(dir, opts);
  after(dir);
  servers[name] = await startServer(['--no-view', '--runs', path.join(root, name)], {env: typeof env === 'function' ? env(dir) : env});
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
const rows = (rs) => rs.map((r) => (typeof r === 'string' ? r : JSON.stringify(r))).join('\n') + '\n';
const append = (dir, rs) => fs.appendFileSync(path.join(dir, 'fs', 'changes.jsonl'), rows(rs));
const put = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, text); };
const late = (change_, action, note = '') => ({format: 'sdc.fs-attribution/1', change: change_, at: '2026-11-23T09:30:00Z', attribution: {case: null, action, record: null, tool: null, basis: 'late', step: 2, note}});
const readBytes = (name) => Number(fs.readFileSync(`/proc/${servers[name].proc.childPid}/io`, 'utf8').match(/rchar:\s+(\d+)/)[1]);

// ---- every answer built from the kept list says when the list is not the whole record ------------------------------------

test('the live feed and the route of one change say when the list of changes is cut at the record bound, as /api/products does', async () => {
  test.setTimeout(60000);
  const N = 3000;
  await make('livecut', {written: []}, (d) => {
    fs.mkdirSync(path.join(d, 'work/bulk'), {recursive: true});
    append(d, Array.from({length: N}, (_, i) => change(1000 + i, `bulk/f${i}.txt`, {text: `${i}\n`, step: 10 + i})));
  }, {...process.env, SDC_UI_MAX_RECORDS: '2000'});
  const p = await products('livecut');
  expect(p.json.recordsStopped).toMatchObject({cause: 'records', read: 2000});
  const live = await get('livecut', '/api/live?source=0');
  expect(live.status).toBe(200);
  expect(Math.max(...live.json.fs.changes.map((c) => c.seq)), 'the newest change the feed holds is not the newest of the run').toBeLessThan(1000 + N - 1);
  expect(live.json.fs.recordsStopped, JSON.stringify(Object.keys(live.json.fs))).toEqual(p.json.recordsStopped);
  const late = await get('livecut', `/api/fschange?source=0&seq=${1000 + N - 1}`);
  expect(late.status).toBe(404);
  expect(late.json.error).toMatch(/past the bound/);
  expect(late.json.recordsStopped).toEqual(p.json.recordsStopped);
  const none = await get('livecut', '/api/fschange?source=0&seq=500');
  expect(none.json).toEqual({error: 'no such change'});
  const held = await get('livecut', `/api/fschange?source=0&seq=${1000 + 5}`);
  expect(held.status).toBe(200);
});

test('a run whose record is whole has no note of a cut in the live feed', async () => {
  test.setTimeout(60000);
  await make('livewhole', {written: []}, (d) => append(d, [change(900, 'a.txt', {text: 'a\n', step: 3})]));
  const live = await get('livewhole', '/api/live?source=0');
  expect(live.json.fs.recordsStopped ?? null).toBe(null);
  const past = await get('livewhole', '/api/fschange?source=0&seq=999999');
  expect(past.status, 'a change past the end of a whole record').toBe(404);
  expect(past.json).toEqual({error: 'no such change'});
});

// ---- the bounds named by the environment -----------------------------------------------------------------------------------

const envCase = async (key, v, make1) => {
  const name = `env-${key}-${v.replace(/\W/g, '_') || 'empty'}`;
  await make1(name, {...process.env, [key]: v});
  const p = await products(name);
  servers[name].proc.kill();
  return {v, basis: p.json.basis, stopped: p.json.recordsStopped?.cause ?? null, read: p.json.recordsStopped?.read ?? null};
};
const small = (name, env) => make(name, {}, () => {}, env);

test('a value of SDC_UI_MAX_RECORDS between 0 and 1 is not a bound that keeps no record', async () => {
  test.setTimeout(60000);
  expect(await envCase('SDC_UI_MAX_RECORDS', '0.5', small)).toEqual({v: '0.5', basis: 'records', stopped: null, read: null});
  expect(await envCase('SDC_UI_MAX_RECORDS', '1.5', small)).toEqual({v: '1.5', basis: 'records', stopped: null, read: null});
});

test('a value of SDC_UI_MAX_RECORDS or SDC_UI_MAX_RECORD_BYTES that is 0, negative, text or empty is the default bound', async () => {
  test.setTimeout(60000);
  for (const key of ['SDC_UI_MAX_RECORDS', 'SDC_UI_MAX_RECORD_BYTES']) {
    for (const v of ['0', '-5', 'abc', '', '  ', 'NaN', '0.5']) expect(await envCase(key, v, small)).toEqual({v, basis: 'records', stopped: null, read: null});
  }
});

test('a value of SDC_UI_MAX_RECORDS that is a whole number a number cannot hold exactly (1e300) is the default bound, not a bound that is off', async () => {
  test.setTimeout(60000);
  const N = 100001;
  await make('raised', {written: [], extra: []}, (d) => {
    const lines = [];
    for (let i = 0; i < N; i++) lines.push(JSON.stringify(change(2000 + i, `bulk/dir${i % 50}/file${i}.txt`, {text: 'x\n', step: 5})));
    fs.appendFileSync(path.join(d, 'fs/changes.jsonl'), lines.join('\n') + '\n');
  }, {...process.env, SDC_UI_MAX_RECORDS: '1e300'});
  const p = await products('raised');
  expect(p.json.basis).toBe('records');
  expect(p.json.recordsStopped, `a bound of 1e300 records keeps ${p.json.total} products and says nothing`).toMatchObject({cause: 'records', read: 100000});
});

test('what the server keeps for one run is bounded in bytes as well as in count, and the answer says the bytes bound stopped the reader', async () => {
  test.setTimeout(60000);
  const note = 'N'.repeat(20000);
  const dir = await make('heldbytes', {written: [], extra: []}, (d) => {
    for (let i = 0; i < 30; i++) put(path.join(d, `work/f${i}.txt`), 'x\n');
    fs.writeFileSync(path.join(d, 'fs/changes.jsonl'), rows(Array.from({length: 30}, (_, i) => ({...change(900 + i, `f${i}.txt`, {text: 'x\n', step: 3}), note}))));
  }, {...process.env, SDC_UI_MAX_RECORD_BYTES: '100000'});
  const p = await products('heldbytes');
  expect(p.json.basis).toBe('records');
  expect(p.json.recordsStopped, JSON.stringify(p.json.recordsStopped)).toMatchObject({cause: 'bytes'});
  expect(p.json.recordsStopped.read).toBeGreaterThan(0);
  expect(p.json.recordsStopped.read).toBeLessThan(10);
  expect(p.json.recordsStopped.why).toMatch(/bytes/);
  expect(p.json.truncated).toBe(true);
  expect(p.json.files.length).toBe(p.json.recordsStopped.read);
  const live = await get('heldbytes', '/api/live?source=0');
  expect(live.json.fs.recordsStopped).toEqual(p.json.recordsStopped);
  expect(fs.statSync(path.join(dir, 'fs/changes.jsonl')).size, 'the file is longer than the bound').toBeGreaterThan(100000);
});

test('the diff of a change is not kept and does not count in the bytes bound: thirty records of 20000 diff characters under a bound of 100000 are all read', async () => {
  test.setTimeout(60000);
  const diff = '+' + 'D'.repeat(20000) + '\n';
  const dir = await make('diffbytes', {written: [], extra: []}, (d) => {
    for (let i = 0; i < 30; i++) put(path.join(d, `work/f${i}.txt`), 'x\n');
    fs.writeFileSync(path.join(d, 'fs/changes.jsonl'), rows(Array.from({length: 30}, (_, i) => ({...change(900 + i, `f${i}.txt`, {text: 'x\n', step: 3}), diff}))));
  }, {...process.env, SDC_UI_MAX_RECORD_BYTES: '100000'});
  const p = await products('diffbytes');
  expect(fs.statSync(path.join(dir, 'fs/changes.jsonl')).size, 'the file is longer than the bound').toBeGreaterThan(600000);
  expect(p.json.recordsStopped, JSON.stringify(p.json.recordsStopped)).toBeNull();
  expect(p.json.truncated).toBe(false);
  expect(p.json.files.length).toBe(30);
});

// ---- the strings of a record ----------------------------------------------------------------------------------------

test('an action name of 300 four-byte characters is cut at 256 characters, not at 256 UTF-16 units, and never inside a character', async () => {
  test.setTimeout(60000);
  const action = '\u{1F600}'.repeat(300);
  await make('astral', {written: [{step: 2, path: 'work/a.md', text: '# a\n', op: 'create', action}]});
  const p = await products('astral');
  const got = p.json.groups.find((g) => g.step === 2).actions[0];
  expect(p.json.cutStrings).toBe(1);
  expect([...got].length, `the cut name holds ${got.length} UTF-16 units`).toBe(256);
  expect(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/.test(got), 'a lone surrogate').toBe(false);
});

test('a 1 MB name, a NUL, a dot component, a path of 5000 components and a path of exactly 1024 characters are counted, dropped or listed; the answer is small and quick', async () => {
  test.setTimeout(60000);
  const deep = Array.from({length: 5000}, () => 'd').join('/');
  const exact = `${['e', 'f', 'g', 'h'].map((c) => c.repeat(250)).join('/')}/${'i'.repeat(17)}.md`; // 1024 characters, no component over 255 bytes
  expect(exact.length).toBe(1024);
  await make('strings', {extra: []}, (d) => {
    append(d, [
      change(900, 'x'.repeat(1 << 20), {text: 'x\n', step: 3}),
      change(901, 'nul\u0000name.md', {text: 'x\n', step: 3}),
      change(902, 'a/.hidden/b.md', {text: 'x\n', step: 3}),
      change(903, deep + '/f.md', {text: 'x\n', step: 3}),
      change(904, `${'q/'.repeat(500)}f.md`, {text: 'x\n', step: 3}),
      change(905, exact, {text: 'x\n', step: 3}),
    ]);
    put(path.join(d, 'work', exact), 'x\n');
  });
  const p = await products('strings');
  expect(p.status).toBe(200);
  expect(p.text.length).toBeLessThan(200000);
  expect(p.ms).toBeLessThan(5000);
  expect(p.json.refusedRecords).toBeGreaterThanOrEqual(3);
  expect(p.json.files.map((f) => f.path)).toContain(`work/${exact}`);
  expect(p.json.files.map((f) => f.path).join('')).not.toMatch(/hidden|nul/);
});

// ---- one answer resolves a bounded number of links per name ----------------------------------------------------------

const hopsRun = async (name, hops, names) => {
  await make(name, {written: [], extra: []}, (d) => {
    fs.symlinkSync('.', path.join(d, 'work/l'));
    append(d, names.map((n, i) => { put(path.join(d, `work/${n}.txt`), `${i}\n`); return change(900 + i, `${'l/'.repeat(hops(i))}${n}.txt`, {text: `${i}\n`, step: 3}); }));
  });
  return products(name);
};

test('a name of more link components than the bound is answered as a name the check did not resolve, counted, and the answer says the bound bit', async () => {
  test.setTimeout(60000);
  await make('hopsprobe', {});
  const max = (await products('hopsprobe')).json.maxLinks;
  expect(Number.isInteger(max) && max >= 1, `maxLinks in the answer: ${max}`).toBe(true);
  const p = await hopsRun('hops', (i) => (i === 0 ? max : max + 1), ['in', 'over']);
  expect(p.json.files.map((f) => f.path)).toEqual([`work/${'l/'.repeat(max)}in.txt`]);
  expect(p.json.leftOut).toBe(1);
  expect(p.json.linkCut).toBe(1);
  expect(p.json.cut, 'a name left out for its links is a cut answer').toBe('links');
  expect(p.json.truncated).toBe(true);
  expect(p.json.gone).toBe(0);
  expect(p.json.maxLinks).toBe(max);
  const none = await hopsRun('hopsnone', () => 1, ['one']);
  expect(none.json.linkCut).toBe(0);
  expect(none.json.cut).toBe(null);
  expect(none.json.truncated).toBe(false);
  expect(none.json.files.map((f) => f.path)).toEqual(['work/l/one.txt']);
});

test('500 names of 500 link components each resolve no more than the bound of link components in one answer, whatever the load', async () => {
  test.setTimeout(60000);
  const names = Array.from({length: 500}, (_, i) => `f${i}`);
  const first = await hopsRun('deeplinks', () => 500, names);
  const second = await products('deeplinks');
  const max = first.json.maxLinks;
  expect(first.json.leftOut).toBe(500);
  // the work of an answer is counted, not timed: each name resolves its first `max` link components and is left out at the next,
  // so 500 names are 500 * max components, not 500 * 500, in the first answer and again in the second
  expect(first.json.linksResolved, 'link components the first answer resolved').toBe(500 * max);
  expect(second.json.linksResolved, 'link components the second answer resolved').toBe(500 * max);
  expect((await get('deeplinks', '/api/sources')).status).toBe(200);
});

test('500 names of 8 link components that are chains of 37 links each are all listed, and an answer resolves 8 components per name, whatever the load', async () => {
  test.setTimeout(60000);
  const names = Array.from({length: 500}, (_, i) => `f${i}`);
  await make('chains', {written: [], extra: []}, (d) => {
    for (let i = 0; i < 37; i++) fs.symlinkSync(i === 36 ? '.' : `c${i + 1}`, path.join(d, `work/c${i}`));
    append(d, names.map((n, i) => { put(path.join(d, `work/${n}.txt`), `${i}\n`); return change(900 + i, `${'c0/'.repeat(8)}${n}.txt`, {text: `${i}\n`, step: 3}); }));
  });
  const first = await products('chains');
  const second = await products('chains');
  expect(first.json.files.length, JSON.stringify([first.json.leftOut, first.json.linkCut, first.json.gone])).toBe(500);
  // the work is the components the answer resolved (each of them one resolution by the kernel, which follows at most 40 links);
  // the links inside a component are the kernel's, not counted here
  expect(first.json.maxLinks).toBe(8);
  expect(first.json.linksResolved, 'link components the first answer resolved').toBe(500 * 8);
  expect(second.json.linksResolved, 'link components the second answer resolved').toBe(500 * 8);
  expect((await get('chains', '/api/sources')).status).toBe(200);
});

test('a name behind a link out of the run is answered alike whether the far file exists, the far directory exists or neither', async () => {
  test.setTimeout(60000);
  const far = path.join(root, 'oracle-far');
  fs.mkdirSync(path.join(far, 'sub'), {recursive: true});
  fs.writeFileSync(path.join(far, 'sub/there.txt'), `${SECRET}\n`);
  const answers = {};
  for (const [tag, target, name] of [['file-exists', far, 'sub/there.txt'], ['file-absent', far, 'sub/not-there.txt'], ['dir-absent', path.join(root, 'oracle-nowhere'), 'sub/there.txt'], ['dangling-in-run', 'no-such-in-run', 'sub/there.txt']]) {
    await make(`oracle-${tag}`, {extra: []}, (d) => {
      fs.symlinkSync(target, path.join(d, 'work/lnk'));
      append(d, [change(900, `lnk/${name}`, {text: 'x\n', step: 3}), change(901, `lnk/${name}.more`, {text: 'x\n', step: 3})]);
    });
    const r = await products(`oracle-${tag}`);
    expect(r.text).not.toContain(SECRET);
    expect(r.text).not.toContain('oracle-far');
    answers[tag] = {status: r.status, gone: r.json.gone, leftOut: r.json.leftOut, files: r.json.files.length, bytes: r.text.length, ms: r.ms};
  }
  const same = (k) => new Set(Object.values(answers).map((a) => a[k])).size === 1;
  expect({status: same('status'), gone: same('gone'), leftOut: same('leftOut'), files: same('files')}, JSON.stringify(answers)).toEqual({status: true, gone: true, leftOut: true, files: true});
  const sizes = Object.values(answers).map((a) => a.bytes);
  expect(Math.max(...sizes) - Math.min(...sizes), JSON.stringify(answers)).toBeLessThan(64);
  expect(Math.max(...Object.values(answers).map((a) => a.ms))).toBeLessThan(2000);
});

test('the count of link components an answer resolved is the same whether anything lies behind a link out of the run, and it is in the Products answer only', async () => {
  test.setTimeout(60000);
  const far = path.join(root, 'count-far');
  fs.mkdirSync(path.join(far, 'sub'), {recursive: true});
  fs.writeFileSync(path.join(far, 'sub/there.txt'), `${SECRET}\n`);
  const nowhere = path.join(root, 'count-nowhere');
  const answers = {};
  const forms = [['file-exists', far, 'sub/there.txt', false], ['file-absent', far, 'sub/not-there.txt', false], ['dir-absent', nowhere, 'sub/there.txt', false],
    ['dangling-in-run', 'no-such-in-run', 'sub/there.txt', false], ['chain-exists', far, 'sub/there.txt', true], ['chain-absent', nowhere, 'sub/there.txt', true]];
  for (const [tag, target, name, chain] of forms) {
    await make(`count-${tag}`, {extra: []}, (d) => {
      if (chain) { fs.symlinkSync('mid', path.join(d, 'work/lnk')); fs.symlinkSync(target, path.join(d, 'work/mid')); } else fs.symlinkSync(target, path.join(d, 'work/lnk'));
      append(d, [change(900, `lnk/${name}`, {text: 'x\n', step: 3}), change(901, `lnk/${name}.more`, {text: 'x\n', step: 3})]);
    });
    const r = await products(`count-${tag}`);
    expect(r.text).not.toContain(SECRET);
    expect(r.text).not.toContain('count-far');
    answers[tag] = r.json;
    const live = await get(`count-${tag}`, '/api/live?source=0');
    const one = await get(`count-${tag}`, '/api/fschange?source=0&seq=900');
    const sources = await get(`count-${tag}`, '/api/sources');
    for (const other of [live, one, sources]) expect(other.text, 'a route other than /api/products').not.toContain('linksResolved');
  }
  const pick = (a) => ({total: a.total, shown: a.shown, leftOut: a.leftOut, gone: a.gone, linkCut: a.linkCut, files: a.files.length, cut: a.cut, truncated: a.truncated, keys: Object.keys(a).sort().join()});
  for (const tag of ['file-absent', 'dir-absent', 'dangling-in-run']) expect(pick(answers[tag]), tag).toEqual(pick(answers['file-exists']));
  expect(pick(answers['chain-absent'])).toEqual(pick(answers['chain-exists']));
  const counts = Object.fromEntries(Object.entries(answers).map(([k, a]) => [k, a.linksResolved]));
  expect(counts, 'the number of links the answer resolved, not what lies behind them').toEqual({'file-exists': 2, 'file-absent': 2, 'dir-absent': 2, 'dangling-in-run': 2, 'chain-exists': 2, 'chain-absent': 2});
  const one = answers['file-exists'];
  expect(Number.isSafeInteger(one.linksResolved) && one.linksResolved <= one.shown * one.maxLinks).toBe(true);
});

// ---- the kept state follows the record file ---------------------------------------------------------------------------

const replacedRun = async (name) => {
  const dir = await make(name, {written: [], extra: []}, (d) => {
    for (const n of ['a', 'b', 'c', 'd', 'e', 'f']) put(path.join(d, `work/${n}.txt`), `${n}\n`);
    append(d, ['a', 'b', 'c'].map((n, i) => change(900 + i, `${n}.txt`, {text: `${n}\n`, step: 3})));
  });
  const file = path.join(dir, 'fs/changes.jsonl');
  const names = async () => (await products(name)).json.files.map((f) => f.path).filter((p) => /work\/[a-f]\.txt/.test(p)).sort();
  expect(await names()).toEqual(['work/a.txt', 'work/b.txt', 'work/c.txt']);
  const seedLines = fs.readFileSync(file, 'utf8').split('\n').filter((l) => l && !/"path":"[a-f]\.txt"/.test(l));
  return {file, names, seedLines};
};

test('a record file replaced by a shorter one shows what the file holds now', async () => {
  test.setTimeout(60000);
  const {file, names, seedLines} = await replacedRun('shorter');
  fs.writeFileSync(file, rows([...seedLines, change(900, 'a.txt', {text: 'a\n', step: 3})]));
  expect(await names(), 'replaced by a shorter file').toEqual(['work/a.txt']);
});

test('a record file truncated to nothing lists no product of the old records', async () => {
  test.setTimeout(60000);
  const {file, names} = await replacedRun('truncated');
  fs.writeFileSync(file, '');
  expect(await names(), 'truncated').toEqual([]);
});

test('a record that grows by half a line is not read until the line is whole', async () => {
  test.setTimeout(60000);
  const dir = await make('halfline', {written: [], extra: []}, (d) => { put(path.join(d, 'work/p.txt'), 'p\n'); put(path.join(d, 'work/q.txt'), 'q\n'); });
  const file = path.join(dir, 'fs/changes.jsonl');
  const names = async () => (await products('halfline')).json.files.map((f) => f.path).filter((p) => /work\/[pq]\.txt/.test(p)).sort();
  expect(await names()).toEqual([]);
  const whole = JSON.stringify(change(900, 'p.txt', {text: 'p\n', step: 3}));
  fs.appendFileSync(file, whole.slice(0, 40));
  expect(await names(), 'half a line').toEqual([]);
  fs.appendFileSync(file, whole.slice(40) + '\n');
  expect(await names(), 'the line made whole').toEqual(['work/p.txt']);
  fs.appendFileSync(file, JSON.stringify(change(901, 'q.txt', {text: 'q\n', step: 3})));
  expect(await names(), 'a whole line without its newline').toEqual(['work/p.txt']);
  fs.appendFileSync(file, '\n');
  expect(await names()).toEqual(['work/p.txt', 'work/q.txt']);
});

const bulk = (name, env = {SDC_UI_MAX_RECORDS: '250'}, pad = '') => make(name, {written: []}, (d) => {
  fs.mkdirSync(path.join(d, 'work/bulk'), {recursive: true});
  append(d, Array.from({length: 300}, (_, i) => ({...change(1000 + i, `bulk/f${i}.txt`, {text: `${i}\n`, step: 10}), note: pad})));
}, {...process.env, ...env});

test('a record file that became a link out of the run, or is gone, after the reader stopped is not answered from the kept list', async () => {
  test.setTimeout(60000);
  const outside = path.join(root, 'stopped-outside.jsonl');
  fs.writeFileSync(outside, rows([change(7000, 'from-outside-link.txt', {text: 'o\n', step: 9}), `{"marker":"${SECRET}"}`]));
  const dir = await bulk('stoppedlink');
  const first = await products('stoppedlink');
  expect(first.json.recordsStopped).toMatchObject({cause: 'records', read: 250});
  const file = path.join(dir, 'fs/changes.jsonl');
  fs.rmSync(file);
  fs.symlinkSync(outside, file);
  const second = await products('stoppedlink');
  expect(second.text).not.toContain(SECRET);
  expect(second.text).not.toContain('from-outside-link');
  expect(second.json.recordsStopped ?? null, `a link at the record file: ${second.json.basis}`).toBe(null);
  fs.rmSync(file);
  expect((await products('stoppedlink')).json.basis, 'the record file is gone').toBe('listing');
});

test('a record file that is replaced by a shorter one after the reader stopped at the record bound is read again', async () => {
  test.setTimeout(60000);
  const dir = await bulk('stoppedshort');
  expect((await products('stoppedshort')).json.recordsStopped).toMatchObject({cause: 'records', read: 250});
  const file = path.join(dir, 'fs/changes.jsonl');
  fs.writeFileSync(file, rows(fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).slice(0, 210)));
  const second = await products('stoppedshort');
  expect(second.json.basis).toBe('records');
  expect(second.json.recordsStopped ?? null, 'the file now holds 210 records, under the bound of 250').toBe(null);
});

test('a reader that stopped at the record bound does not read the file again while it is as it was or only longer', async () => {
  test.setTimeout(60000);
  const dir = await bulk('stoppedreads', {SDC_UI_MAX_RECORDS: '250'}, 'N'.repeat(8000));
  expect((await products('stoppedreads')).json.recordsStopped).toMatchObject({cause: 'records', read: 250});
  const before = readBytes('stoppedreads');
  for (let i = 0; i < 5; i++) await products('stoppedreads');
  expect(readBytes('stoppedreads') - before, 'bytes the server read for five answers about a stopped file (about 400 000 each if it read the rest again)').toBeLessThan(200000);
  append(dir, [change(5000, 'bulk/more.txt', {text: 'm\n', step: 11})]);
  const grown = await products('stoppedreads');
  expect(grown.json.recordsStopped).toMatchObject({cause: 'records', read: 250});
});

test('a reader that stopped at the bytes bound does not read the file again while it is only longer', async () => {
  test.setTimeout(60000);
  const dir = await bulk('stoppedbytes', {SDC_UI_MAX_RECORD_BYTES: '100000'}, 'N'.repeat(8000));
  const first = await products('stoppedbytes');
  expect(first.json.recordsStopped, JSON.stringify(first.json.recordsStopped)).toMatchObject({cause: 'bytes'});
  const before = readBytes('stoppedbytes');
  for (let i = 0; i < 5; i++) await products('stoppedbytes');
  expect(readBytes('stoppedbytes') - before, 'bytes the server read for five answers about a file stopped at the bytes bound').toBeLessThan(200000);
  append(dir, [change(5000, 'bulk/more.txt', {text: 'm\n', step: 11})]);
  const beforeGrown = readBytes('stoppedbytes');
  const grown = await products('stoppedbytes');
  expect(grown.json.recordsStopped).toMatchObject({cause: 'bytes', read: first.json.recordsStopped.read});
  expect(readBytes('stoppedbytes') - beforeGrown, 'bytes the server read for the answer after the file grew (about 2 000 000 if it read the rest again)').toBeLessThan(200000);
});

test('a stop that the growth of the file resolves is no longer reported', async () => {
  test.setTimeout(60000);
  const dir = await make('stopgrows', {written: []}, (d) => {
    put(path.join(d, 'work/p.txt'), 'p\n');
    fs.writeFileSync(path.join(d, 'fs/changes.jsonl'), Buffer.alloc(33 << 20, 'x'));
  });
  const first = await products('stopgrows');
  expect(first.json.basis).toBe('listing');
  expect(first.json.basisNote).toMatch(/longer than the inspector reads/);
  const file = path.join(dir, 'fs/changes.jsonl');
  const filler = `${' '.repeat(1000)}\n`.repeat(34000);
  fs.writeFileSync(file, rows([change(900, 'p.txt', {text: 'p\n', step: 3})]) + filler);
  const second = await products('stopgrows');
  expect(second.json.basis).toBe('records');
  expect(second.json.recordsStopped ?? null, JSON.stringify(second.json.recordsStopped)).toBe(null);
  expect(second.json.files.map((f) => f.path)).toEqual(['work/p.txt']);
});

test('a record file that stopped on a line over the chunk and is replaced by a longer file of records is read again', async () => {
  test.setTimeout(60000);
  const dir = await make('stoplonger', {written: []}, (d) => {
    put(path.join(d, 'work/p.txt'), 'p\n');
    fs.writeFileSync(path.join(d, 'fs/changes.jsonl'), Buffer.alloc(33 << 20, 'x'));
  });
  const first = await products('stoplonger');
  expect(first.json.basis).toBe('listing');
  const file = path.join(dir, 'fs/changes.jsonl');
  const stopped = fs.statSync(file).size;
  fs.writeFileSync(file, rows([change(900, 'p.txt', {text: 'p\n', step: 3})]) + `${' '.repeat(1000)}\n`.repeat(35000));
  expect(fs.statSync(file).size, 'the new file is longer than the one the reader stopped in').toBeGreaterThan(stopped);
  const second = await products('stoplonger');
  expect(second.json.basis).toBe('records');
  expect(second.json.files.map((f) => f.path)).toEqual(['work/p.txt']);
});

test('attribution lines that name more changes than the server keeps for one run stop the reader and say so', async () => {
  test.setTimeout(60000);
  await make('latebound', {written: []}, (d) => {
    append(d, Array.from({length: 400}, (_, i) => ({format: 'sdc.fs-attribution/1', change: i, at: '2026-11-23T09:30:00Z', attribution: {case: null, action: `late.${i}`, record: null, tool: null, basis: 'late', step: 2}})));
  }, {...process.env, SDC_UI_MAX_RECORDS: '300'});
  const p = await products('latebound');
  expect(p.json.basis).toBe('records');
  expect(p.json.recordsStopped, 'the 301st distinct late attribution is past the bound of 300').toMatchObject({cause: 'records'});
  expect(p.json.truncated).toBe(true);
});

test('attribution lines count in the bytes the server keeps for one run: more of them than the bound stop the reader with the cause bytes', async () => {
  test.setTimeout(60000);
  await make('latebytes', {written: []}, (d) => {
    append(d, Array.from({length: 60}, (_, i) => ({format: 'sdc.fs-attribution/1', change: i, at: '2026-11-23T09:30:00Z', attribution: {case: null, action: `late.${i}`, record: null, tool: null, basis: 'late', step: 2, note: 'N'.repeat(4000)}})));
  }, {...process.env, SDC_UI_MAX_RECORD_BYTES: '100000'});
  const p = await products('latebytes');
  expect(p.json.basis).toBe('records');
  expect(p.json.recordsStopped, JSON.stringify(p.json.recordsStopped)).toMatchObject({cause: 'bytes'});
  expect(p.json.truncated).toBe(true);
});

test('a late attribution line for a change that has one already replaces it and is not another change the server keeps', async () => {
  test.setTimeout(60000);
  // 201 records and 203 changes named by a late line, under a bound of 203: a replacement adds no change to what is kept
  await make('latereplace-count', {written: []}, (d) => {
    append(d, [...Array.from({length: 203}, (_, i) => late(i, `first.${i}`)), late(0, 'second.0')]);
  }, {...process.env, SDC_UI_MAX_RECORDS: '203'});
  const live = await get('latereplace-count', '/api/live?source=0');
  expect(live.json.fs.recordsStopped, JSON.stringify(live.json.fs.recordsStopped)).toBe(null);
  expect(live.json.fs.changes.find((c) => c.seq === 0).attribution.action, 'the replacement line is the one kept').toBe('second.0');
});

test('a last line that makes the bytes kept reach the bound exactly is held, one that passes it by a byte stops the reader', async () => {
  test.setTimeout(60000);
  const bytesOf = (d) => fs.readFileSync(path.join(d, 'fs/changes.jsonl'), 'utf8').split('\n').filter(Boolean).reduce((n, l) => n + Buffer.byteLength(l), 0);
  for (const [tag, last] of [['record', change(5000, 'z.txt', {text: 'z\n', step: 9})], ['late', late(5, 'last.late')]]) {
    for (const [delta, stops] of [[0, false], [-1, true]]) {
      const name = `exact-${tag}-${stops ? 'less' : 'equal'}`;
      await make(name, {written: []}, (d) => append(d, [last]), (d) => ({...process.env, SDC_UI_MAX_RECORD_BYTES: String(bytesOf(d) + delta)}));
      const live = await get(name, '/api/live?source=0');
      expect(live.json.fs.recordsStopped?.cause ?? null, `${tag} line last, bound ${delta} from the bytes of the lines`).toBe(stops ? 'bytes' : null);
      expect(live.json.fs.changes.length, 'the records held').toBe(tag === 'record' && !stops ? 202 : 201);
    }
  }
});

test('a late attribution line that replaces one already kept counts its growth in the bytes bound: the answer holds no more than the bound or says it stopped', async () => {
  test.setTimeout(60000);
  const BOUND = 150000;
  const names = Array.from({length: 10}, (_, i) => `f${i}`);
  await make('latereplace', {written: [], extra: []}, (d) => {
    for (const n of names) put(path.join(d, `work/${n}.txt`), 'x\n');
    append(d, names.map((n, i) => change(900 + i, `${n}.txt`, {text: 'x\n', step: 3})));
    append(d, names.map((n, i) => late(900 + i, `first.${i}`)));
    for (let round = 0; round < 3; round++) append(d, names.map((n, i) => late(900 + i, `second.${i}`, 'N'.repeat(15000))));
  }, {...process.env, SDC_UI_MAX_RECORD_BYTES: String(BOUND)});
  const live = await get('latereplace', '/api/live?source=0');
  expect(live.status).toBe(200);
  const held = JSON.stringify(live.json.fs.changes).length;
  expect(live.json.fs.changes.length, 'the records the answer holds').toBeGreaterThanOrEqual(211);
  expect(held <= BOUND || live.json.fs.recordsStopped != null, `the answer holds ${held} bytes of records under a bytes bound of ${BOUND} and says nothing (recordsStopped ${JSON.stringify(live.json.fs.recordsStopped)})`).toBe(true);
});

const countedOf = (file) => {
  let bytes = 0;
  const longest = new Map();
  for (const l of fs.readFileSync(file, 'utf8').split('\n')) {
    if (!l.trim()) continue;
    const r = JSON.parse(l);
    const len = Buffer.byteLength(l);
    if (r.format === 'sdc.fs-attribution/1') longest.set(r.change, Math.max(len, longest.get(r.change) ?? 0));
    else bytes += len - (typeof r.diff === 'string' ? r.diff.length : 0);
  }
  for (const v of longest.values()) bytes += v;
  return bytes;
};
const prng = (seed) => () => { seed = (seed + 0x6D2B79F5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const padOf = (n) => 'é€'.repeat(Math.ceil(n / 2)).slice(0, n);
const LATE_ORDERS = {
  'new then shrink then grow then shrink': [[900, 100], [900, 5], [900, 400], [900, 1]],
  'equal lines again and again': [[900, 300], [900, 300], [900, 300]],
  'two changes interleaved, one never has a record': [[900, 10], [7777, 10], [900, 500], [7777, 500], [900, 1], [7777, 1]],
  'shrink first, grow last': [[901, 2000], [901, 20], [901, 3000]],
};
for (const seed of [11, 29, 47]) {
  const next = prng(seed);
  LATE_ORDERS[`seeded ${seed}`] = Array.from({length: 14}, () => [900 + Math.floor(next() * 4) * (next() < 0.3 ? 1000 : 1), Math.floor(next() * next() * 4000)]);
}

test('the bytes counted are the lines of the records and the longest late line of each change, in every order of new, shrinking and growing late lines, and the bound stops the reader at one byte under it', async () => {
  test.setTimeout(60000);
  for (const [tag, steps] of Object.entries(LATE_ORDERS)) {
    const lines = steps.map(([ch, n], i) => late(ch, `act.${i}`, padOf(n)));
    let total = 0;
    for (const delta of [0, -1]) {
      const name = `lateorder-${Object.keys(LATE_ORDERS).indexOf(tag)}-${delta === 0 ? 'eq' : 'under'}`;
      await make(name, {written: []}, (d) => append(d, lines), (d) => ({...process.env, SDC_UI_MAX_RECORD_BYTES: String((total = countedOf(path.join(d, 'fs/changes.jsonl'))) + delta)}));
      const live = await get(name, '/api/live?source=0');
      expect(live.status).toBe(200);
      const stop = live.json.fs.recordsStopped;
      if (delta === 0) {
        expect(stop, `${tag}: the bound is exactly the bytes held, ${total}`).toBe(null);
        for (const ch of new Set(steps.map(([c]) => c))) {
          const rec = live.json.fs.changes.find((c) => c.seq === ch);
          if (rec) expect(rec.attribution.action, `${tag}: the last line for change ${ch} is the one kept`).toBe(`act.${steps.map(([c]) => c).lastIndexOf(ch)}`);
        }
      } else {
        expect(stop?.cause, `${tag}: the bound is one byte under the bytes held, ${total - 1}`).toBe('bytes');
      }
    }
  }
});

test('a late attribution line for a change that was read is shown at its step on the next answer', async () => {
  test.setTimeout(60000);
  const dir = await make('lateshown', {written: []}, (d) => {
    put(path.join(d, 'work/a.txt'), 'a\n');
    append(d, [change(900, 'a.txt', {text: 'a\n', step: 3, action: 'first.act'})]);
  });
  const steps = async () => (await products('lateshown')).json.groups.map((g) => [g.step, g.actions]);
  expect(await steps()).toEqual([[3, ['first.act']]]);
  append(dir, [{format: 'sdc.fs-attribution/1', change: 900, at: '2026-11-23T09:30:00Z', attribution: {case: null, action: 'late.act', record: null, tool: null, basis: 'late', step: 5}}]);
  expect(await steps()).toEqual([[5, ['late.act']]]);
});

// ---- the bound on describe calls and the cost on 100 000 products ----------------------------------------------------------

test('100 000 products of a run cost an answer 500 looks at the host and a second answer almost nothing', async () => {
  test.setTimeout(60000);
  const N = 100000;
  await make('hundredk', {written: [], extra: []}, (d) => {
    const lines = [];
    for (let i = 0; i < N; i++) lines.push(JSON.stringify(change(2000 + i, `bulk/dir${i % 50}/file${i}.txt`, {text: 'x\n', step: 5})));
    fs.appendFileSync(path.join(d, 'fs/changes.jsonl'), lines.join('\n') + '\n');
  });
  const first = await products('hundredk');
  const second = await products('hundredk');
  const third = await products('hundredk');
  expect(first.json.basis).toBe('records');
  expect(first.json.gone + first.json.leftOut + first.json.files.length).toBe(500);
  expect(first.json.total).toBeGreaterThan(99000);
  expect(second.ms).toBeLessThan(3000);
  expect(third.ms).toBeLessThan(3000);
});

// ---- the page ------------------------------------------------------------------------------------------------------------

test('the Products tab says in text that a name passed more links than the bound and was not listed', async ({page}) => {
  test.setTimeout(60000);
  await make('pagelinks', {written: [], extra: []}, (d) => {
    fs.symlinkSync('.', path.join(d, 'work/l'));
    put(path.join(d, 'work/deep.txt'), 'd\n');
    append(d, [change(900, `${'l/'.repeat(30)}deep.txt`, {text: 'd\n', step: 3})]);
  });
  await page.setViewportSize({width: 1600, height: 1000});
  await page.goto(`${servers.pagelinks.url}/?run=run`);
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-cut').filter({hasText: /more than \d+ links/})).toHaveCount(1);
});

test('the File changes tab of a run whose reader stopped before the first record says the list was not read, as the feed does', async ({page}) => {
  test.setTimeout(60000);
  await make('emptystop', {written: []}, (d) => { fs.writeFileSync(path.join(d, 'fs/changes.jsonl'), Buffer.alloc(33 << 20, 'x')); });
  const live = await get('emptystop', '/api/live?source=0');
  expect(live.json.fs.present).toBe(true);
  expect(live.json.fs.changes).toEqual([]);
  expect(live.json.fs.recordsStopped, JSON.stringify(live.json.fs)).toMatchObject({cause: 'line', read: 0});
  await page.setViewportSize({width: 1600, height: 1000});
  await page.goto(`${servers.emptystop.url}/?run=run`);
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
  await expect(page.locator('#feed .fd-cut')).toHaveCount(1);
  await page.locator('#tabs button[data-tab=changes]').click();
  await expect(page.locator('#panel .sect').first()).toContainText('File changes');
  const panel = await page.locator('#panel').innerText();
  expect(panel, 'the tab of a run whose list of changes was not read at all').toMatch(/not read|read to record 0/);
});

test('the Products tab redraws when only the count of names past the link bound changes', async ({page}) => {
  test.setTimeout(60000);
  const outside = path.join(root, 'redraw-outside');
  fs.mkdirSync(outside, {recursive: true});
  const dir = await make('pageredraw', {written: [], extra: []}, (d) => {
    fs.symlinkSync(outside, path.join(d, 'work/l'));
    put(path.join(d, 'work/ok.txt'), 'o\n');
    // an action name cut at 256 characters makes the answer `cut: "strings"` before and after, so `cut` does not move with linkCut
    append(d, [change(900, `${'l/'.repeat(30)}deep.txt`, {text: 'd\n', step: 3}), change(901, 'ok.txt', {text: 'o\n', step: 3, action: 'x'.repeat(300)})]);
  });
  await page.setViewportSize({width: 1600, height: 1000});
  await page.goto(`${servers.pageredraw.url}/?run=run`);
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-groups')).toBeVisible();
  const before = await products('pageredraw');
  expect([before.json.leftOut, before.json.linkCut, before.json.cut], 'the name is left out behind a link that leaves the run, not for its links').toEqual([1, 0, 'strings']);
  await expect(page.locator('#panel .prod-cut').filter({hasText: /more than \d+ links/})).toHaveCount(0);
  // the same name, now past the link bound: leftOut stays 1 and no file changes, only linkCut moves
  fs.unlinkSync(path.join(dir, 'work/l'));
  fs.symlinkSync('.', path.join(dir, 'work/l'));
  const after = await products('pageredraw');
  expect([after.json.leftOut, after.json.linkCut, after.json.cut, after.json.files.length]).toEqual([1, 1, 'strings', before.json.files.length]);
  // a change of the record file is what makes the page ask again (at most every 1.5 s); a late line changes no file of the list
  const notes = () => page.locator('#panel .prod-cut').filter({hasText: /more than \d+ links/}).count();
  await expect.poll(async () => { append(dir, [late(900, 'again')]); return notes(); }, {timeout: 20000, intervals: [1600]}).toBe(1);
});

test('an action name that is markup and is cut inside it is shown as text, with the note', async ({page}) => {
  test.setTimeout(60000);
  const action = '<img src=x onerror="window.__pwned=1"><b>'.repeat(20);
  await make('pagecut', {written: [{step: 2, path: 'work/a.md', text: '# a\n', op: 'create', action}]});
  await page.setViewportSize({width: 1600, height: 1000});
  await page.goto(`${servers.pagecut.url}/?run=run`);
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-groups')).toBeVisible();
  await expect(page.locator('#panel .prod-note').filter({hasText: 'cut to 256 characters'})).toHaveCount(1);
  expect(await page.locator('#panel .prod-groups').locator('b, img, script').count()).toBe(0);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  const head = await page.locator('#panel .prod-group[data-step="2"] h5').innerText();
  expect(head).toContain('<img src=x onerror=');
});
