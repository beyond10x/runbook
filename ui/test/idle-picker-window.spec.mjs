// The run picker's list against the stream of a page. A page holds the list /api/sources gave it (the header button and the
// picker show a run's state from it) and reads it again only when the stream sends `sources`. A run that changes state after
// the page's list and before the stream's first round must still reach the page, and with no page connected nothing runs.
// The cases from "a list older than a minute" on start a server of their own, with a seam (idle-list-seam.cjs: a clock that
// moves on by 61 s on a signal, and the length of the longest list of states the server stored for a source), and hold the
// guards of the list states: a list older than a minute is no news, the store is bounded, lists of a new state do not push an
// older one out, a new page does not move the baseline of the pages that were there, a second stop is a new state.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeLoomRun, iso} from './loom-run-fixture.mjs';
import {fixtureRoot} from './security-products-lib.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = fixtureRoot('idle-picker-window');
let srv, base;
const streams = [];
const others = [];
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); for (const s of others) s.kill(); for (const s of streams) s.destroy(); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const runDir = (i) => path.join(root, 'runs', `r${i}`);
const get = async (p) => (await fetch(base + p)).json();
const stopRun = (i, reason) => fs.writeFileSync(path.join(runDir(i), 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at: iso(30), by: 'tester', reason, verdict_at_stop: 'waiting', case: 'CASE-1'}));
function watch(source, b = base) {
  const seen = [];
  let buf = '';
  const req = http.get(`${b}/events?source=${source}`, (res) => {
    res.setEncoding('utf8');
    res.on('data', (d) => {
      buf += d;
      for (let i = buf.indexOf('\n\n'); i >= 0; i = buf.indexOf('\n\n')) { const b = buf.slice(0, i); buf = buf.slice(i + 2); seen.push({event: /^event: (.*)$/m.exec(b)?.[1], data: /^data: (.*)$/m.exec(b)?.[1]}); }
    });
  });
  req.on('error', () => {});
  streams.push(req);
  return {seen, close: () => req.destroy()};
}
const rounds = async () => (await get('/api/reads')).rounds;
const roundsFrom = async (r0, n) => { for (let t = Date.now(); Date.now() - t < 20000 && (await rounds()) < r0 + n; ) await sleep(50); };

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  for (let i = 0; i < 4; i++) {
    const d = writeLoomRun(runDir(i));
    fs.rmSync(path.join(d, 'status.json')); fs.rmSync(path.join(d, 'outcome.json'));
    const ev = path.join(d, 'store', 'CASE-1', 'events.jsonl');
    fs.writeFileSync(ev, fs.readFileSync(ev, 'utf8').trim().split('\n').filter((l) => !l.includes('"terminated"')).join('\n') + '\n');
  }
  ({url: base, proc: srv} = await startServer(['--count-reads', '--no-view', '--runs', path.join(root, 'runs')]));
});

test('a run that stops between the page\'s last list of runs and the first round of its stream is announced to the page', async () => {
  test.setTimeout(60000);
  // the page's list (what the header button and the picker show): run 2 is not ended
  const list = await get('/api/sources');
  expect(list[2].run.state).toBe('unknown');
  // the run is stopped on purpose; no page watched it yet
  stopRun(2, 'window');
  // the page picks run 2: a stream for it
  const page = watch(2);
  const r0 = await rounds();
  await roundsFrom(r0, 4);
  await sleep(300);
  const heard = page.seen.filter((e) => e.event === 'sources');
  const now = (await get('/api/sources'))[2].run.state;
  expect(now).toBe('stopped');
  expect(heard.length, `the page holds a list that says "unknown" for a run that is ${now}, and heard ${heard.length} 'sources' event(s) in 4 rounds (the picker and the header read the list again on that event only)`).toBeGreaterThan(0);
  page.close();
});

test('a page that opens its stream on a list that is current is sent no sources event, and one on an older list than another page\'s is', async () => {
  test.setTimeout(60000);
  // page A holds the list with run 3 not ended; the run stops; page B reads the list (it shows run 3 stopped)
  const a = await get('/api/sources');
  expect(a[3].run.state).toBe('unknown');
  stopRun(3, 'two pages');
  const b = await get('/api/sources');
  expect(b[3].run.state).toBe('stopped');
  // page A's stream opens after page B's list: A still holds the list of before and is told
  const pageA = watch(3);
  // a page whose list is the current one (run 1 did not change) is told nothing
  const pageC = watch(1);
  const r0 = await rounds();
  await roundsFrom(r0, 3);
  await sleep(300);
  expect(pageA.seen.filter((e) => e.event === 'sources').length, 'page A, whose list is older than the state').toBeGreaterThan(0);
  expect(pageC.seen.filter((e) => e.event === 'sources').length, 'page C, whose list is current').toBe(0);
  pageA.close(); pageC.close();
});

test('a run that stops right after a page opened its stream on a current list is announced to that page', async () => {
  test.setTimeout(60000);
  const list = await get('/api/sources');
  expect(list[0].run.state).toBe('unknown');
  const page = watch(0);
  for (let t = Date.now(); Date.now() - t < 10000 && !page.seen.length; ) await sleep(10);
  expect(page.seen.map((e) => e.event), 'the stream is open').toContain('hello');
  expect(page.seen.filter((e) => e.event === 'sources').length, 'a current list is sent no sources event').toBe(0);
  stopRun(0, 'after connect');
  for (let t = Date.now(); Date.now() - t < 8000 && !page.seen.some((e) => e.event === 'sources'); ) await sleep(25);
  expect(page.seen.filter((e) => e.event === 'sources').length, 'the page holds a list that says "unknown" for a run that is now stopped').toBeGreaterThan(0);
  page.close();
});

test('when one of two pages leaves, the other still hears of its run', async () => {
  test.setTimeout(60000);
  const a = watch(0), b = watch(1);
  for (let t = Date.now(); Date.now() - t < 10000 && !(a.seen.length && b.seen.length); ) await sleep(50);
  a.close();
  await sleep(1500);
  const had = b.seen.filter((e) => e.event === 'change').length;
  fs.appendFileSync(path.join(runDir(1), 'store', 'CASE-1', 'events.jsonl'), JSON.stringify({event: 'note', seq: 900, at: iso(70), source: 'loom-runlog'}) + '\n');
  for (let t = Date.now(); Date.now() - t < 8000 && b.seen.filter((e) => e.event === 'change').length <= had; ) await sleep(50);
  expect(b.seen.filter((e) => e.event === 'change').length > had, 'the page that stayed hears of a change of its run').toBe(true);
  b.close();
});

// ---- the list states, on a server of each case's own ----------------------------------------------------------------------
const until = async (what, fn, ms = 20000) => {
  const end = Date.now() + ms;
  for (;;) { const v = await fn(); if (v) return v; if (Date.now() > end) throw new Error(`gave up waiting for ${what}`); await sleep(25); }
};
const stopIn = (dir, reason, n = 0) => fs.writeFileSync(path.join(dir, 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at: iso(30 + n), by: 'tester', reason, verdict_at_stop: 'waiting', case: 'CASE-1'}));
const sources = (p) => p.seen.filter((e) => e.event === 'sources').length;
// `count` runs not ended, under --runs, a server with the seam, and what a case asks of it
async function ownServer(name, count) {
  const runs = path.join(root, name);
  const dirs = [];
  for (let i = 0; i < count; i++) {
    const d = writeLoomRun(path.join(runs, `r${i}`));
    fs.rmSync(path.join(d, 'status.json')); fs.rmSync(path.join(d, 'outcome.json'));
    const ev = path.join(d, 'store', 'CASE-1', 'events.jsonl');
    fs.writeFileSync(ev, fs.readFileSync(ev, 'utf8').trim().split('\n').filter((l) => !l.includes('"terminated"')).join('\n') + '\n');
    dirs.push(d);
  }
  const seamFile = path.join(root, `${name}.seam.json`);
  const env = {...process.env, NODE_OPTIONS: `--require ${path.join(here, 'idle-list-seam.cjs')}`, IDLE_SEAM_FILE: seamFile};
  const s = await startServer(['--count-reads', '--no-view', '--runs', runs], {env});
  others.push(s.proc);
  const pid = await s.proc.started;
  const json = async (p) => { for (let t = 0; ; t++) { try { return await (await fetch(s.url + p, {headers: {connection: 'close'}, signal: AbortSignal.timeout(20000)})).json(); } catch (e) { if (t >= 3) throw e; await sleep(100); } } };
  let answered = 0;
  const longest = async () => {
    answered++;
    process.kill(pid, 'SIGUSR2');
    return (await until('the seam answers', () => { try { const a = JSON.parse(fs.readFileSync(seamFile, 'utf8')); return a.answers === answered ? a : null; } catch { return null; } })).longest;
  };
  return {url: s.url, pid, dirs, list: () => json('/api/sources'), clock: () => json('/api/clock'), rounds: async () => (await json('/api/reads')).rounds, longest};
}

test('a list older than a minute is no reason to announce a state to a page that opens its stream', async () => {
  test.setTimeout(60000);
  const s = await ownServer('window', 2);
  expect((await s.list()).map((x) => x.run.state)).toEqual(['unknown', 'unknown']);
  // a page on a list a moment old is told of the stop
  stopIn(s.dirs[0], 'young');
  const young = watch(0, s.url);
  await until('sources for a page on a list a moment old', () => sources(young) > 0, 10000);
  young.close();
  // the clock of the server moves on by 61 s: the list of run 1 is older than the window
  stopIn(s.dirs[1], 'old');
  const t0 = Date.now();
  process.kill(s.pid, 'SIGHUP');
  await until('the clock of the server is past the minute', async () => (await s.clock()).now >= t0 + 60000, 10000);
  const old = watch(1, s.url);
  await until('hello', () => old.seen.some((e) => e.event === 'hello'));
  const r0 = await s.rounds();
  await until('two rounds', async () => (await s.rounds()) >= r0 + 2, 20000);
  expect(sources(old), 'a list older than a minute is announced as stale').toBe(0);
  old.close();
});

test('the lists a source was served with are stored at most eight at a time, however many lists come', async () => {
  test.setTimeout(60000);
  const s = await ownServer('store', 1);
  for (let i = 0; i < 12; i++) {
    stopIn(s.dirs[0], `state ${i}${' '.repeat(i)}`, i);
    await s.list();
  }
  expect(await s.longest(), 'the longest list of states the server stored for the source, after 12 lists with 12 states').toBe(8);
});

test('a page on a list that was current before the run stopped is told, after any number of lists of the new state', async () => {
  test.setTimeout(60000);
  const s = await ownServer('many', 1);
  expect((await s.list()).map((x) => x.run.state)).toEqual(['unknown']);
  stopIn(s.dirs[0], 'stopped after the first list');
  for (let i = 0; i < 12; i++) await s.list();
  const page = watch(0, s.url);
  await until('sources for a page that may hold the first list', () => sources(page) > 0, 10000)
    .catch(() => { throw new Error('the page that may hold the list from before the stop was not told: twelve lists of the new state pushed the first one out'); });
  page.close();
});

test('a run that stopped just before another page connected is still announced to the page that was there', async () => {
  test.setTimeout(60000);
  const N = 8;
  const s = await ownServer('base', N);
  const old = Array.from({length: N}, (_, i) => watch(i, s.url));
  for (const p of old) await until('hello', () => p.seen.some((e) => e.event === 'hello'));
  const r0 = await s.rounds();
  await until('a round with the pages there', async () => (await s.rounds()) >= r0 + 2, 20000);
  const fresh = [];
  for (let i = 0; i < N; i++) { stopIn(s.dirs[i], 'just before', i); fresh.push(watch(i, s.url)); }
  await until('every page that was there is told', () => old.every((p) => sources(p) > 0), 15000)
    .catch(() => { throw new Error(`pages that were never told that their run stopped: ${old.map((p, i) => (sources(p) ? null : i)).filter((i) => i != null).join(',')}`); });
  for (const p of [...old, ...fresh]) p.close();
});

test('a stop note rewritten with other content moves the picker row of a stopped run, and the page is told again', async () => {
  test.setTimeout(60000);
  const s = await ownServer('restop', 1);
  const page = watch(0, s.url);
  await until('hello', () => page.seen.some((e) => e.event === 'hello'));
  stopIn(s.dirs[0], 'first');
  await until('sources for the stop', () => sources(page) >= 1, 10000);
  const before = sources(page);
  stopIn(s.dirs[0], 'a second and longer reason for the same stop', 5);
  await until('sources for the second note', () => sources(page) > before, 10000)
    .catch(() => { throw new Error('a stop note rewritten with other content was not announced to the page'); });
  page.close();
});
