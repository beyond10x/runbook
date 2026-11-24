// What the server answers and sends while it does little: one server over four invented runs, started with --count-reads.
//   - a value of `source` that is no registered run is a refusal with no host path and no byte of a run, on every route;
//   - a page hears of its own run only, and a `source` that is no run hears no change;
//   - /api/reads is no route without the option, counts only with it, and names runs, never a path of the host;
//   - the file route and the case reads refuse a file that was replaced after the server read it;
//   - the run picker's row is read when the picker opens;
//   - pages that connect and leave leave no timer, stream or descriptor, and the idle server uses no CPU;
//   - the clock branch is turned on by SDC_CLOCK_DIR, not by an argument, and the README says so;
//   - the static route serves nothing outside web/.
// Fixtures only, invented text, no network beyond loopback. The servers listen on ports the kernel gives.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeLoomRun, iso} from './loom-run-fixture.mjs';
import {fixtureRoot, put, raw} from './security-products-lib.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = fixtureRoot('idle-server');
const SECRET = 'OUTSIDE-IDLE-7f3a';
const HOME = process.env.HOME || '/nonexistent-home';
const outside = path.join(root, 'outside');
let base, srv, plain, pid;
const streams = [];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); plain?.kill(); for (const s of streams) s.destroy(); });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const until = async (what, fn, ms = 20000) => {
  const end = Date.now() + ms;
  for (;;) { const v = await fn(); if (v) return v; if (Date.now() > end) throw new Error(`gave up waiting for ${what}`); await sleep(25); }
};
const runDir = (i) => path.join(root, 'runs', `r${i}`);
const storeOf = (i) => path.join(runDir(i), 'store');
const eventsOf = (i) => path.join(storeOf(i), 'CASE-1', 'events.jsonl');
const get = async (p, init) => { const r = await fetch(base + p, {signal: AbortSignal.timeout(20000), ...init}); const text = await r.text(); return {status: r.status, text, type: r.headers.get('content-type'), json: (() => { try { return JSON.parse(text); } catch { return null; } })()}; };
const reads = async () => (await fetch(`${base}/api/reads`)).json();
function watch(source, b = base) {
  const seen = [];
  let buf = '';
  const req = http.get(`${b}/events?source=${encodeURIComponent(source)}`, (res) => {
    res.setEncoding('utf8');
    res.on('data', (d) => {
      buf += d;
      for (let i = buf.indexOf('\n\n'); i >= 0; i = buf.indexOf('\n\n')) {
        const block = buf.slice(0, i); buf = buf.slice(i + 2);
        seen.push({block, event: /^event: (.*)$/m.exec(block)?.[1], data: /^data: (.*)$/m.exec(block)?.[1]});
      }
    });
  });
  req.on('error', () => {});
  streams.push(req);
  return {seen, close: () => req.destroy()};
}
const cpuTicks = (p) => { const f = fs.readFileSync(`/proc/${p}/stat`, 'utf8'); const a = f.slice(f.lastIndexOf(')') + 2).split(' '); return Number(a[11]) + Number(a[12]); };
const fds = (p) => fs.readdirSync(`/proc/${p}/fd`).length;
const mark = (i) => `OWN-r${i}-MARK`;

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  put(path.join(outside, 'secret.txt'), `${SECRET}\n`);
  for (let i = 0; i < 4; i++) {
    writeLoomRun(runDir(i));
    put(path.join(runDir(i), 'work', 'own.txt'), `${mark(i)}\n`);
    put(path.join(runDir(i), 'work', 'swap.txt'), `${mark(i)} swap\n`);
  }
  ({url: base, proc: srv} = await startServer(['--count-reads', '--no-view', '--runs', path.join(root, 'runs')]));
  pid = await srv.started;
});

// ---- `source` -----------------------------------------------------------------------------------------------------------
const ROUTES = ['/api/run', '/api/tree', '/api/live', '/api/products', '/api/workfacts', '/api/inbox', '/api/view', '/api/view-info',
  '/api/file?path=work/own.txt', '/api/output?case=.&seq=1', '/api/fschange?seq=1', `/api/blob?case=.&hash=${'a'.repeat(64)}`];
const SOURCES = ['-1', '99', 'abc', '1.5', '1e9', '99999999999999999999', '4294967296', '4294967295', '-0', '0x1', '1e0', ' 2', '%20', 'NaN', 'Infinity', '-Infinity',
  '__proto__', 'constructor', '../../..', '/etc/passwd', 'file:///etc/passwd', '0/../1', '0;1', '1,2', '[1]', '{}', 'null', 'undefined', '0%00', 'é'];

test('source-parse: a source that is no registered run is a refusal with no host path and no byte of a run, on every route', async () => {
  test.setTimeout(60000);
  const bad = [];
  const valid = (v) => { const n = Number(v); return Number.isInteger(n) && n >= 0 && n < 4; };
  for (const v of SOURCES) for (const r of ROUTES) {
    const sep = r.includes('?') ? '&' : '?';
    const a = await get(`${r}${sep}source=${encodeURIComponent(v)}`);
    const leak = [root, HOME + '/', '/home/', '/etc/passwd:', SECRET].find((s) => a.text.includes(s));
    const content = [0, 1, 2, 3].map(mark).find((m) => a.text.includes(m));
    if (leak) bad.push(`${r} source=${v}: path or secret ${leak}`);
    if (!valid(v) && content) bad.push(`${r} source=${v}: content ${content}`);
    if (!valid(v) && a.status >= 500) bad.push(`${r} source=${v}: status ${a.status}`);
  }
  // a repeated parameter and a parameter that is not first: the first one counts, and only for a registered run
  const rep = await get('/api/file?path=work/own.txt&source=1&source=2');
  expect(rep.text).toContain(mark(1));
  expect(rep.text).not.toContain(mark(2));
  const fl = await get(`/api/file?path=work/own.txt&source=${encodeURIComponent('1&source=2')}`);
  expect(fl.text).not.toContain(mark(2));
  expect(bad).toEqual([]);
});

test('events-source: a page of one source receives no event of another, and a source that is no run receives no change at all', async () => {
  test.setTimeout(60000);
  const values = ['abc', '1.5', '-1', '99', '1e9', '../../..', '/etc/passwd', 'NaN', 'Infinity', '1', '2', '0x1'];
  const pages = values.map((v) => [v, watch(v)]);
  await until('every hello', () => pages.every(([, p]) => p.seen.some((e) => e.event === 'hello')));
  const before = (await reads()).rounds;
  // every run changes: a line, a new case directory whose name is the content that must not cross
  for (let i = 0; i < 4; i++) {
    fs.appendFileSync(eventsOf(i), JSON.stringify({event: 'note', seq: 500 + i, at: iso(70), source: 'loom-runlog'}) + '\n');
    const c = path.join(storeOf(i), `LEAKCASE-r${i}`);
    put(path.join(c, 'events.jsonl'), JSON.stringify({event: 'opened', at: iso(0), protocol: {id: 'example', revision: 1}, seq: 0, source: 'loom-runlog'}) + '\n');
    put(path.join(c, 'case.json'), JSON.stringify({id: `LEAKCASE-r${i}`}));
  }
  await until('rounds', async () => (await reads()).rounds >= before + 4);
  await sleep(800);
  const bad = [];
  for (const [v, p] of pages) {
    const want = Number(v);
    const valid = Number.isInteger(want) && want >= 0 && want < 4;
    for (const e of p.seen) {
      const all = e.block;
      if ([root, HOME + '/', '/home/', SECRET].some((s) => all.includes(s))) bad.push(`${v}: path or secret in ${e.event}`);
      if (e.event === 'change') {
        if (!valid) bad.push(`${v}: a change event for a source that is no run`);
        else if (JSON.parse(e.data).source !== want) bad.push(`${v}: change of source ${JSON.parse(e.data).source}`);
      }
      for (let i = 0; i < 4; i++) if ((!valid || i !== want) && (all.includes(`LEAKCASE-r${i}`) || all.includes(mark(i)))) bad.push(`${v}: content of run ${i} in ${e.event}`);
    }
    if (valid) expect(p.seen.some((e) => e.event === 'change' && JSON.parse(e.data).source === want), `source ${v} hears of its own change`).toBe(true);
  }
  for (const [, p] of pages) p.close();
  expect(bad).toEqual([]);
});

// ---- /api/reads ---------------------------------------------------------------------------------------------------------
test('reads-route: without the option the route is an unknown route (same answer), with it it counts only and names runs, never a path', async () => {
  test.setTimeout(60000);
  const s = await startServer(['--no-view', '--runs', path.join(root, 'runs')]);
  plain = s.proc;
  const ask = async (p, method = 'GET') => { const r = await fetch(s.url + p, {method}); return {status: r.status, type: r.headers.get('content-type'), body: await r.text()}; };
  for (const [a, b] of [['/api/reads', '/api/zzzzz'], ['/api/reads?source=0', '/api/zzzzz?source=0'], ['/api/reads?source=99', '/api/zzzzz?source=99'], ['/api/reads/', '/api/zzzzz/'], ['/api/READS', '/api/zzzzz']]) {
    expect(await ask(a), `${a} against ${b}`).toEqual(await ask(b));
  }
  expect(await ask('/api/reads', 'POST')).toEqual(await ask('/api/zzzzz', 'POST'));
  for (let i = 0; i < 4; i++) for (const r of ['/api/run', '/api/tree', '/api/live', '/api/inbox', '/api/file?path=work/own.txt']) await get(`${r}${r.includes('?') ? '&' : '?'}source=${i}`);
  const t = await get('/api/reads');
  expect(t.status).toBe(200);
  expect(Object.keys(t.json).sort()).toEqual(['byRun', 'rounds', 'total']);
  for (const [k, n] of Object.entries(t.json.byRun)) { expect(k.startsWith('/') || k.includes(root) || k.includes('/home/'), `byRun key ${k}`).toBe(false); expect(Number.isInteger(n)).toBe(true); }
  for (const bad of [root, HOME + '/', SECRET, 'OWN-r', 'events.jsonl']) expect(t.text).not.toContain(bad);
  expect((await get('/api/reads', {method: 'POST'})).status).toBe(405);
  expect((await get('/events', {method: 'POST'})).status).toBe(405);
});

test('a request that names no run reads no run: a source that is none is not run 0', async () => {
  test.setTimeout(60000);
  const before = (await reads()).byRun;
  fs.appendFileSync(eventsOf(0), JSON.stringify({event: 'note', seq: 9000, at: iso(70), source: 'test'}) + '\n');
  for (const s of ['99', '-1', 'abc']) for (const r of ['/api/tree', '/api/live', '/api/inbox', '/api/run']) await get(`${r}?source=${s}`);
  expect((await reads()).byRun, 'a run was read for a request that names none').toEqual(before);
  await get('/api/tree?source=0');
  expect((await reads()).byRun, 'the change of run 0 is read when its run is named').not.toEqual(before);
});

test('reads-names-no-host-path: runs given as a store argument, through a link, as a case directory or under --runs are all named by run, whatever is read', async () => {
  test.setTimeout(60000);
  const link = path.join(root, 'link-to-r1');
  fs.rmSync(link, {force: true});
  fs.symlinkSync(runDir(1), link);
  const s = await startServer(['--count-reads', '--no-view', path.join(link, 'store'), path.join(storeOf(2), 'CASE-1'), storeOf(3), '--runs', path.join(root, 'runs')]);
  try {
    const n = (await (await fetch(s.url + '/api/sources')).json()).length;
    expect(n).toBeGreaterThanOrEqual(4);
    for (let i = 0; i < n; i++) for (const r of ['/api/run', '/api/tree', '/api/live', '/api/inbox', '/api/products', '/api/file?path=work/own.txt', '/api/output?case=.&seq=1']) await fetch(`${s.url}${r}${r.includes('?') ? '&' : '?'}source=${i}`);
    const t = await (await fetch(s.url + '/api/reads')).json();
    expect(Object.keys(t.byRun).length).toBeGreaterThan(0);
    for (const k of Object.keys(t.byRun)) {
      expect(k.startsWith('/') || k.startsWith('~') || k.includes(root) || k.includes(HOME) || k.includes('/home/'), `byRun key ${k}`).toBe(false);
    }
    expect(JSON.stringify(t)).not.toContain(root);
  } finally { s.proc.kill(); }
});

// ---- a file replaced after the server read it ----------------------------------------------------------------------------
test('guard-server-after-first-read: the file route and the case reads refuse a file replaced after the first read', async () => {
  test.setTimeout(60000);
  const w = path.join(runDir(1), 'work', 'swap.txt');
  const first = await get('/api/file?path=work/swap.txt&source=1');
  expect(first.status).toBe(200);
  const target = path.join(outside, 'secret.txt');
  const tryIt = async (how) => {
    fs.rmSync(w, {recursive: true, force: true});
    if (how === 'link') fs.symlinkSync(target, w);
    else if (how === 'hard') fs.linkSync(target, w);
    else if (how === 'dir') fs.mkdirSync(w);
    const a = await get('/api/file?path=work/swap.txt&source=1');
    expect(a.text, `${how}`).not.toContain(SECRET);
    expect(a.status, `${how}`).not.toBe(200);
  };
  for (const how of ['link', 'hard', 'dir', 'gone']) await tryIt(how);
  // work/ itself becomes a link to the outside directory
  fs.rmSync(path.join(runDir(1), 'work'), {recursive: true, force: true});
  fs.symlinkSync(outside, path.join(runDir(1), 'work'));
  const a = await get('/api/file?path=work/secret.txt&source=1');
  expect(a.text).not.toContain(SECRET);
  expect(a.status).not.toBe(200);
  const lst = await get('/api/products?source=1');
  expect(lst.text).not.toContain(SECRET);
  // a dot directory and a place outside work/ are not served
  put(path.join(runDir(2), 'work', '.hidden', 'x.txt'), `${SECRET}\n`);
  for (const p of ['work/.hidden/x.txt', 'store/CASE-1/events.jsonl', 'run-settings.jsonl', 'status.json']) {
    const b = await get(`/api/file?path=${encodeURIComponent(p)}&source=2`);
    expect(b.status, p).not.toBe(200);
  }
  // events.jsonl of a case replaced by a link out after the case was read: no byte of the target
  const ev = eventsOf(3);
  const t1 = await get('/api/tree?source=3&cases=CASE-1');
  expect(t1.json.cases[0].events.length).toBeGreaterThan(3);
  put(path.join(outside, 'events.jsonl'), JSON.stringify({event: 'note', seq: 0, at: iso(0), source: SECRET}) + '\n');
  fs.rmSync(ev); fs.symlinkSync(path.join(outside, 'events.jsonl'), ev);
  const t2 = await get('/api/tree?source=3&cases=CASE-1');
  expect(t2.text).not.toContain(SECRET);
  expect(t2.json.cases.length === 0 || t2.json.cases[0].events.length === 0, 'the refused file is shown as refused, not as the old content').toBe(true);
  fs.rmSync(ev); fs.linkSync(path.join(outside, 'events.jsonl'), ev);
  const t3 = await get('/api/tree?source=3&cases=CASE-1');
  expect(t3.text).not.toContain(SECRET);
  expect(t3.json.cases.length === 0 || t3.json.cases[0].events.length === 0, 'the file with a second name is shown as refused').toBe(true);
});

test('picker-row-is-read-when-the-picker-opens: a run that ended, stopped or resumed shows so at once', async () => {
  test.setTimeout(60000);
  const d = runDir(0);
  const rows = async () => (await get('/api/sources')).json;
  const state = async () => (await rows())[0].run.state;
  fs.rmSync(path.join(d, 'status.json')); fs.rmSync(path.join(d, 'outcome.json'));
  const evs = fs.readFileSync(eventsOf(0), 'utf8').trim().split('\n').filter((l) => !l.includes('"terminated"'));
  fs.writeFileSync(eventsOf(0), evs.join('\n') + '\n');
  expect(await state()).toBe('unknown');
  fs.writeFileSync(path.join(d, 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at: iso(30), by: 'tester', reason: 'a test', verdict_at_stop: 'waiting', case: 'CASE-1'}));
  expect(await state()).toBe('stopped');
  fs.rmSync(path.join(d, 'run-stopped.json'));
  expect(await state()).toBe('unknown');
  fs.writeFileSync(path.join(d, 'status.json'), JSON.stringify({format: 'sdc.run-status/1', at: iso(60), state: 'closed', outcome: 'ready', delivered: false}));
  expect(await state()).toBe('closed');
  fs.writeFileSync(path.join(d, 'status.json'), JSON.stringify({format: 'sdc.run-status/1', at: iso(61), state: 'open', outcome: null, delivered: false}));
  expect(await state()).toBe('unknown');
});

// ---- liveness -----------------------------------------------------------------------------------------------------------
test('idle-handles: 200 pages that connect and leave leave no timer, no stream and no descriptor; the server is idle after', async () => {
  test.setTimeout(60000);
  const fd0 = fds(pid);
  const one = (waitHello, i) => new Promise((ok) => {
    const req = http.get(`${base}/events?source=${i % 4}`, (res) => {
      if (waitHello) res.once('data', () => { req.destroy(); ok(); }); else { req.destroy(); ok(); }
    });
    req.on('error', () => ok());
    if (!waitHello) setTimeout(() => { req.destroy(); ok(); }, 1);
  });
  for (let b = 0; b < 10; b++) await Promise.all(Array.from({length: 20}, (_, k) => one(k % 2 === 0, b * 20 + k)));
  await sleep(1000);
  const r1 = (await reads()).rounds;
  await sleep(1700);
  const r2 = (await reads()).rounds;
  const fd1 = fds(pid);
  const c0 = cpuTicks(pid);
  await sleep(3000);
  const c1 = cpuTicks(pid);
  console.log(`idle handles: rounds ${r1} -> ${r2}, descriptors ${fd0} -> ${fd1}, cpu ticks in 3 s idle: ${c1 - c0}`);
  expect(r2, 'rounds after every page left').toBe(r1);
  expect(fd1 - fd0, 'descriptors left behind').toBeLessThanOrEqual(2);
  expect(c1 - c0, 'cpu ticks (1/100 s) in 3 s of idle').toBeLessThanOrEqual(5);
});

// ---- the clock ----------------------------------------------------------------------------------------------------------
test('clock-branch: only SDC_CLOCK_DIR turns it on (not an argument), a step of the clock reads every run with no page, and the texts say so', async () => {
  test.setTimeout(60000);
  const gate = path.join(root, 'gate');
  fs.rmSync(gate, {recursive: true, force: true});
  fs.mkdirSync(gate, {recursive: true});
  fs.writeFileSync(path.join(gate, 'clock'), String(Date.parse('2026-11-21T12:00:00Z')));
  fs.writeFileSync(path.join(gate, 'at'), '');
  await expect(startServer(['--clock', gate, '--count-reads', '--no-view', '--runs', path.join(root, 'runs')]), 'a --clock argument is not an option').rejects.toThrow(/unknown option/);
  const s = await startServer(['--count-reads', '--no-view', '--runs', path.join(root, 'runs')], {env: {...process.env, SDC_CLOCK_DIR: gate}});
  try {
    expect((await (await fetch(s.url + '/api/clock')).json()).clock).toBe(true);
    const r = async () => (await fetch(s.url + '/api/reads')).json();
    const a = await r();
    fs.writeFileSync(path.join(runDir(2), 'llm-calls.jsonl'), JSON.stringify({action: 'x', model: 'm', adapter: 'a', cost_usd: 0.1}) + '\n');
    fs.writeFileSync(path.join(gate, 'at'), '1');
    const b = await until('a round on the step of the clock', async () => { const x = await r(); return x.rounds > a.rounds ? x : null; });
    expect(b.total).toBeGreaterThan(a.total);
    expect(Object.keys(b.byRun).length).toBeGreaterThan(1);
  } finally { s.proc.kill(); }
  // the texts that say what turns the branch on name the variable, and neither says that an option does
  const readme = fs.readFileSync(path.join(here, '..', 'README.md'), 'utf8');
  const para = readme.slice(readme.indexOf('- When the server reads'), readme.indexOf('\n- ', readme.indexOf('- When the server reads') + 3));
  expect(para).toContain('SDC_CLOCK_DIR');
  expect(para).not.toMatch(/with `--clock`/i);
  const server = fs.readFileSync(path.join(here, '..', 'server.mjs'), 'utf8');
  expect(server).not.toMatch(/With --clock/);
  expect(server).toContain('With SDC_CLOCK_DIR set');
});

// ---- the static route -------------------------------------------------------------------------------------------------
test('static-route: no request reaches a file outside web/', async () => {
  test.setTimeout(30000);
  for (const p of ['/../server.mjs', '/%2e%2e/server.mjs', '/..%2fserver.mjs', '/..%5cserver.mjs', '/web/../server.mjs', '/../web-extra/x', '//etc/passwd', '/%00', '/..\\server.mjs']) {
    // sent as written (fetch would resolve the dots itself)
    const a = await raw(Number(new URL(base).port), p, {headers: {host: new URL(base).host}});
    expect(a.body, p).not.toContain('servedUnder');
    expect(a.body, p).not.toContain('root:x:');
    expect(a.status, p).not.toBe(200);
  }
  expect((await get('/index.html')).status).toBe(200);
});
