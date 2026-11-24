// A page that does not read its stream is dropped once its unsent output passes a bound (server.mjs MAX_PENDING), so the memory
// held for a page is bounded and the rounds stop when it was the last one. The page here is a socket that asks for the stream
// and never reads; every round announces a change of 314 cases whose paths are about 3 000 characters long (nested cases:
// each level is a directory with a `children/` directory under it), about 1 MB, so the output outgrows the kernel's buffers
// (about 3 MB for a client that never reads) within a few rounds. The case is bounded in time (30 rounds at most); the
// server's pending output is bounded by the drop, not by the number of rounds.
// The cases after it hold the line of that bound with a seam (idle-pending-seam.cjs: a response that reports 2 MiB, or a number
// of bytes the case names, of unsent output after a `change`, and a ping every 150 ms or none): a page at the bound or under it
// is kept and served, one byte over it is ended by the next change pushed to it, and by the next ping when no change comes.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = fixtureRoot('idle-slow-page');
const DEPTH = 14, LEAVES = 300, NAME = 200;
const MiB = 1048576;
let srv, base;
const others = [], streams = [];
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); for (const s of others) s.kill(); for (const s of streams) s.destroy(); fs.rmSync(root, {recursive: true, force: true}); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// a request on a connection that the loaded server closed a moment ago is made again
const rounds = async () => {
  for (let tries = 0; ; tries++) {
    try { return (await (await fetch(`${base}/api/reads`, {headers: {connection: 'close'}})).json()).rounds; } catch (e) { if (tries >= 3) throw e; await sleep(100); }
  }
};
const name = (tag, i) => `${tag}${String(i).padStart(3, '0')}${'n'.repeat(NAME - 4)}`;

test('a page that never reads is dropped: its stream is ended and the rounds stop, in bounded time', async () => {
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const store = path.join(root, 'run', 'store');
  const cases = [];
  let dir = store;
  for (let i = 0; i < DEPTH; i++) { dir = path.join(dir, ...(i ? ['children'] : []), name('s', i)); cases.push(dir); }
  for (let i = 0; i < LEAVES; i++) cases.push(path.join(dir, 'children', name('l', i)));
  for (const c of cases) put(path.join(c, 'case.json'), '{"n":0}');
  expect(path.join(cases.at(-1), 'case.json').length, 'under the path limit of 4 096').toBeLessThan(4000);
  ({url: base, proc: srv} = await startServer(['--count-reads', '--no-view', store]));
  const sock = net.connect(Number(new URL(base).port), '127.0.0.1');
  sock.on('error', () => {});
  const closed = new Promise((r) => sock.once('close', r));
  await new Promise((r) => sock.once('connect', r));
  sock.pause();
  sock.write(`GET /events?source=0 HTTP/1.1\r\nHost: ${new URL(base).host}\r\n\r\n`);
  let dropped = false, spent = 0;
  const t0 = Date.now();
  for (let k = 1; k <= 30 && !dropped; k++) {
    const r = await rounds();
    for (const c of cases) fs.writeFileSync(path.join(c, 'case.json'), `{"n":${k}}`);
    // a round after the write: the counter moves, or it stands still because the page was dropped and no page is left
    let moved = false;
    for (const end = Date.now() + 6000; !moved && Date.now() < end; ) { moved = (await rounds()) > r; if (!moved) await sleep(50); }
    dropped = !moved;
    spent = k;
  }
  console.log(`slow page: dropped=${dropped} after ${spent} rounds of ${cases.length} changed cases, ${Date.now() - t0} ms`);
  expect(dropped, `a page that never reads was still served after ${spent} rounds (${cases.length} cases changed each)`).toBe(true);
  // what the page held reaches it, and then its stream is over: the server ended it
  sock.resume();
  await Promise.race([closed, sleep(10000).then(() => { throw new Error('the dropped page\'s connection is still open 10 s after it was dropped'); })]);
  expect(sock.destroyed).toBe(true);
});

// ---- the line of the bound, with the seam ---------------------------------------------------------------------------------
const until = async (what, fn, ms = 20000) => {
  const end = Date.now() + ms;
  for (;;) { const v = await fn(); if (v) return v; if (Date.now() > end) throw new Error(`gave up waiting for ${what}`); await sleep(25); }
};
// a server over one invented case, with the seam; a page on its stream that reads (the seam makes it look behind)
async function seamServer(tag, pending, ping) {
  const store = path.join(root, tag, 'run', 'store');
  const caseFile = path.join(store, 'CASE-1', 'case.json');
  put(caseFile, '{"n":0}');
  const env = {...process.env, NODE_OPTIONS: `--require ${path.join(here, 'idle-pending-seam.cjs')}`, IDLE_PENDING: String(pending), IDLE_PING: ping};
  const s = await startServer(['--count-reads', '--no-view', store], {env});
  others.push(s.proc);
  const rounds = async () => { for (let t = 0; ; t++) { try { return (await (await fetch(`${s.url}/api/reads`, {headers: {connection: 'close'}})).json()).rounds; } catch (e) { if (t >= 3) throw e; await sleep(100); } } };
  const page = {seen: [], closed: false};
  let buf = '';
  const req = http.get(`${s.url}/events?source=0`, (res) => {
    res.on('close', () => { page.closed = true; });
    res.setEncoding('utf8');
    res.on('data', (d) => {
      buf += d;
      for (let i = buf.indexOf('\n\n'); i >= 0; i = buf.indexOf('\n\n')) { page.seen.push(/^event: (.*)$/m.exec(buf.slice(0, i))?.[1]); buf = buf.slice(i + 2); }
    });
  });
  req.on('error', () => {});
  streams.push(req);
  await until('hello', () => page.seen.includes('hello'));
  const write = (n) => fs.writeFileSync(caseFile, `{"n":${n}}`);
  const changes = () => page.seen.filter((e) => e === 'change').length;
  return {rounds, page, write, changes};
}

test('a page that is behind by the bound or less is kept and served, one byte over the bound is ended by the next change', async () => {
  test.setTimeout(60000);
  for (const [tag, pending] of [['under', MiB - 1], ['exact', MiB]]) {
    const {page, write, changes} = await seamServer(`slow-${tag}`, pending, 'none');
    for (let k = 1; k <= 3; k++) {
      const n = changes();
      write(k);
      await until(`change ${k} reaches the page (${tag})`, () => changes() > n, 10000);
    }
    expect(page.closed, `a page ${tag === 'exact' ? 'at' : 'under'} the bound was dropped`).toBe(false);
  }
  const {page, write, changes} = await seamServer('slow-over', MiB + 1, 'none');
  write(1);
  await until('the first change', () => changes() > 0, 10000);
  write(2);
  await until('one byte over the bound: the next change ended the page', () => page.closed, 5000);
});

test('a page that is past the bound after the last change is ended by the next ping, with no further change', async () => {
  test.setTimeout(60000);
  const {page, write, changes} = await seamServer('ping', 2 * MiB, 'fast');
  write(1);
  await until('a change', () => changes() > 0, 10000);
  await until('the ping ended the page, no change was written after the last one', () => page.closed, 5000)
    .catch(() => { throw new Error('a page past the bound was not ended by a ping within 5 s'); });
});

test('with no ping at all, a page that is past the bound is ended by the next change pushed to it', async () => {
  test.setTimeout(60000);
  const {rounds, page, write, changes} = await seamServer('push', 2 * MiB, 'none');
  write(1);
  await until('a change', () => changes() > 0, 10000);
  const r = await rounds();
  await until('two rounds', async () => (await rounds()) >= r + 2, 10000);
  expect(page.closed, 'no ping and no change since: the page is still there').toBe(false);
  write(2);
  await until('the next change ended the page', () => page.closed, 5000)
    .catch(() => { throw new Error('a page past the bound was not ended by the next change pushed to it'); });
});
