// The inspector does no work while nobody looks. A server over a directory of 50 runs, started with the test
// option --count-reads (the one place the counter of reads of run files is exposed: GET /api/reads, a 404
// without the option), is watched from outside:
//   - with no page connected no round runs and no run file is read;
//   - a request for a run nobody watches reads that run on request, once per change of a file;
//   - with one page on one run (an event stream naming the source) the rounds read that run only, and none in a
//     round where nothing changed; a change of the shown run reaches the page, a change of another does not;
//   - a run that appears under --runs is announced to the page;
//   - a cached read is served for the run and the path it was read for, and never past a change of the file
//     or past the run-path check.
// No model, no network. The windows in which "nothing happens" is looked at are the only waits; everything else
// waits on the counter of rounds the option exposes.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeLoomRun, iso} from './loom-run-fixture.mjs';
import {registerRun, readText, readJson} from '../runfiles.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(FIXTURE_DIR, 'idle-reads');
const RUNS = 50;
const name = (i) => `run-${String(i).padStart(3, '0')}`;
const runDir = (i) => path.join(root, 'runs', name(i));
const eventsOf = (i) => path.join(runDir(i), 'store', 'CASE-1', 'events.jsonl');
let base, srv, plain;
const streams = [];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); plain?.kill(); for (const s of streams) s.destroy(); });

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  for (let i = 0; i < RUNS; i++) writeLoomRun(runDir(i));
  ({url: base, proc: srv} = await startServer(['--count-reads', '--runs', path.join(root, 'runs')]));
});

const reads = async () => (await fetch(`${base}/api/reads`)).json();
const delta = (a, b) => Object.fromEntries(Object.entries(b.byRun).map(([k, n]) => [k, n - (a.byRun[k] || 0)]).filter(([, n]) => n > 0));
const until = async (what, fn, ms = 20000) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`gave up waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 25));
  }
};
// an event stream like the page's: the kinds and data of what arrives
function watch(source) {
  const seen = [];
  let buf = '';
  const req = http.get(`${base}/events?source=${source}`, (res) => {
    res.setEncoding('utf8');
    res.on('data', (d) => {
      buf += d;
      for (let i = buf.indexOf('\n\n'); i >= 0; i = buf.indexOf('\n\n')) {
        const block = buf.slice(0, i);
        buf = buf.slice(i + 2);
        const ev = /^event: (.*)$/m.exec(block)?.[1];
        const data = /^data: (.*)$/m.exec(block)?.[1];
        if (ev) seen.push({event: ev, data: data ? JSON.parse(data) : null, at: Date.now()});
      }
    });
  });
  req.on('error', () => {});
  streams.push(req);
  return {seen, close: () => req.destroy()};
}

test('with no page connected no round runs and no run file is read (50 runs, 3 rounds of a page that is not there)', async () => {
  const a = await reads();
  expect(Object.keys(a)).toEqual(['rounds', 'total', 'byRun']);
  await new Promise((r) => setTimeout(r, 2400));
  const b = await reads();
  expect(b.rounds).toBe(0);
  expect(b.total).toBe(a.total);
  expect(delta(a, b)).toEqual({});
});

test('the counter is behind the option: a server without it has no such route', async () => {
  test.setTimeout(30000);
  const s = await startServer(['--runs', path.join(root, 'runs')]);
  plain = s.proc;
  const r = await fetch(`${s.url}/api/reads`);
  expect(r.status).toBe(404);
  expect(await r.text()).not.toContain('byRun');
  const web = fs.readdirSync(path.join(here, '..', 'web')).filter((f) => f.endsWith('.js') || f.endsWith('.html'));
  for (const f of web) expect(fs.readFileSync(path.join(here, '..', 'web', f), 'utf8')).not.toContain('/api/reads');
});

test('a request for a run nobody watches reads that run on request, and a file that did not change is not read again', async () => {
  // run 7: a new line in its llm-calls.jsonl (a file the run summary and the run route both read)
  const file = path.join(runDir(7), 'llm-calls.jsonl');
  await fetch(`${base}/api/run?source=7`).then((r) => r.json());
  const a = await reads();
  fs.appendFileSync(file, JSON.stringify({action: 'notes.write', case: 'CASE-1', model: 'claude-haiku-5-5', adapter: 'metaharness', cost_usd: 0.001}) + '\n');
  const first = await (await fetch(`${base}/api/run?source=7`)).json();
  expect(first.llm.length).toBe(1);
  const b = await reads();
  expect(Object.keys(delta(a, b))).toEqual([name(7)]);
  const again = await (await fetch(`${base}/api/run?source=7`)).json();
  expect(again.llm.length).toBe(1);
  expect(await (await fetch(`${base}/api/run?source=7`)).json()).toEqual(first);
  const c = await reads();
  expect(c.total).toBe(b.total);
  expect(c.rounds).toBe(0);
});

test('with one page on one run the rounds read that run only, none in a round where nothing changed, and a change of the shown run reaches the page', async () => {
  const before = await reads();
  const page = watch(2);
  await until('the hello of the stream', () => page.seen.some((e) => e.event === 'hello'));
  const r0 = (await until('a first round', async () => { const x = await reads(); return x.rounds >= 1 ? x : null; })).rounds;
  // the shown run changes: one new event line; another run changes too, and the page must not hear of it
  const wrote = Date.now();
  fs.appendFileSync(eventsOf(2), JSON.stringify({event: 'note', seq: 99, at: iso(70), source: 'loom-runlog'}) + '\n');
  fs.appendFileSync(eventsOf(9), JSON.stringify({event: 'note', seq: 99, at: iso(70), source: 'loom-runlog'}) + '\n');
  const change = await until('the change of the shown run', () => page.seen.find((e) => e.event === 'change' && e.data.source === 2 && e.data.cases.includes('CASE-1')));
  expect(change.at - wrote, 'the change reaches the page within 5 s (a round is 700 ms)').toBeLessThan(5000);
  expect(change.data.cases).toEqual(['CASE-1']);
  const during = await reads();
  expect(Object.keys(delta(before, during))).toEqual([name(2)]);
  // three more rounds, nothing changing: no read
  await until('three more rounds', async () => (await reads()).rounds >= during.rounds + 3);
  const after = await reads();
  expect(after.total).toBe(during.total);
  expect(delta(during, after)).toEqual({});
  expect(page.seen.filter((e) => e.event === 'change').every((e) => e.data.source === 2)).toBe(true);
  expect(r0).toBeGreaterThanOrEqual(1);
  // the page leaves: the rounds stop
  page.close();
  await until('the round counter to stand still', async () => { const x = await reads(); await new Promise((r) => setTimeout(r, 1000)); return (await reads()).rounds === x.rounds; });
});

test('a run that appears under --runs is announced to the page without a read of it', async () => {
  const page = watch(0);
  await until('the hello of the stream', () => page.seen.some((e) => e.event === 'hello'));
  const before = await reads();
  writeLoomRun(runDir(RUNS), {});
  const announced = await until('the sources event', () => page.seen.find((e) => e.event === 'sources'));
  expect(announced.data.count).toBe(RUNS + 1);
  expect(delta(before, await reads())[name(RUNS)]).toBeUndefined();
  page.close();
});

// ---- the cache of runfiles.mjs --------------------------------------------------------------------------------
test('a cached read is served for its run and path only, never past a change of the file or the run-path check', () => {
  const dir = path.join(root, 'cache');
  fs.rmSync(dir, {recursive: true, force: true});
  const [r1, r2, out] = ['one', 'two', 'outside'].map((n) => { const d = path.join(dir, n); fs.mkdirSync(d, {recursive: true}); return d; });
  registerRun(r1); registerRun(r2);
  const stamp = new Date('2026-11-21T10:00:00Z');
  const put = (d, f, text) => { fs.writeFileSync(path.join(d, f), text); fs.utimesSync(path.join(d, f), stamp, stamp); };
  put(r1, 'a.json', '{"v":"AAAA"}');
  put(r2, 'a.json', '{"v":"BBBB"}');
  // same name, same size, same modification time, two runs
  expect(readJson(path.join(r1, 'a.json')).v).toBe('AAAA');
  expect(readJson(path.join(r2, 'a.json')).v).toBe('BBBB');
  expect(readJson(path.join(r1, 'a.json')).v).toBe('AAAA');
  // another path of the same run with the same size and time
  put(r1, 'b.json', '{"v":"CCCC"}');
  expect(readJson(path.join(r1, 'b.json')).v).toBe('CCCC');
  expect(readJson(path.join(r1, 'a.json')).v).toBe('AAAA');
  // the file is written again with the same size, a later time
  fs.writeFileSync(path.join(r1, 'a.json'), '{"v":"ZZZZ"}');
  expect(readJson(path.join(r1, 'a.json')).v).toBe('ZZZZ');
  // the file is replaced by another file of the same size and time (a new inode)
  put(r1, 'c.json', '{"v":"DDDD"}');
  expect(readJson(path.join(r1, 'c.json')).v).toBe('DDDD');
  fs.rmSync(path.join(r1, 'c.json'));
  put(r1, 'c.json', '{"v":"EEEE"}');
  expect(readJson(path.join(r1, 'c.json')).v).toBe('EEEE');
  // the file becomes a link that leaves the run: the cached text is not served
  put(out, 'secret.json', '{"v":"OUTSIDE"}');
  fs.rmSync(path.join(r1, 'b.json'));
  fs.symlinkSync(path.join(out, 'secret.json'), path.join(r1, 'b.json'));
  expect(readText(path.join(r1, 'b.json'))).toBeNull();
  // the file gets a second name: refused as without a cache
  put(r1, 'd.json', '{"v":"FFFF"}');
  expect(readJson(path.join(r1, 'd.json')).v).toBe('FFFF');
  fs.linkSync(path.join(r1, 'd.json'), path.join(out, 'd-alias.json'));
  expect(readText(path.join(r1, 'd.json'))).toBeNull();
  // a file over the limit asked for is not served from the cache of a larger limit
  expect(readText(path.join(r1, 'a.json'), undefined, 4)).toBeNull();
  // the file is gone
  fs.rmSync(path.join(r1, 'a.json'));
  expect(readText(path.join(r1, 'a.json'))).toBeNull();
});
