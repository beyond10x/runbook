// A page that does not read its stream is dropped once its unsent output passes a bound (server.mjs MAX_PENDING), so the memory
// held for a page is bounded and the rounds stop when it was the last one. The page here is a socket that asks for the stream
// and never reads; every round announces a change of 314 cases whose paths are about 3 000 characters long (nested cases:
// each level is a directory with a `children/` directory under it), about 1 MB, so the output outgrows the kernel's buffers
// (about 3 MB for a client that never reads) within a few rounds. The case is bounded in time (30 rounds at most); the
// server's pending output is bounded by the drop, not by the number of rounds.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('idle-slow-page');
const DEPTH = 14, LEAVES = 300, NAME = 200;
let srv, base;
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
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
