// The run picker's list against the stream of a page. A page holds the list /api/sources gave it (the header button and the
// picker show a run's state from it) and reads it again only when the stream sends `sources`. A run that changes state after
// the page's list and before the stream's first round must still reach the page, and with no page connected nothing runs.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeLoomRun, iso} from './loom-run-fixture.mjs';
import {fixtureRoot} from './security-products-lib.mjs';

const root = fixtureRoot('idle-picker-window');
let srv, base;
const streams = [];
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); for (const s of streams) s.destroy(); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const runDir = (i) => path.join(root, 'runs', `r${i}`);
const get = async (p) => (await fetch(base + p)).json();
const stopRun = (i, reason) => fs.writeFileSync(path.join(runDir(i), 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at: iso(30), by: 'tester', reason, verdict_at_stop: 'waiting', case: 'CASE-1'}));
function watch(source) {
  const seen = [];
  let buf = '';
  const req = http.get(`${base}/events?source=${source}`, (res) => {
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
