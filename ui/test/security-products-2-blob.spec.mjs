// Confirmation pass of the independent review: the route GET /api/blob (server.mjs, `readText`) next to the routes the
// fix round guarded. Its siblings (/api/file, /api/output) now refuse a name that starts with a dot, a file with more
// than one link, and read at most a bounded window. /api/blob reads through readText, which has none of the three.
// Own server, run written by hand (products-fixture.mjs), invented text.
import {test, expect} from '@playwright/test';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, OUTPUT, OUTPUT_DIGEST, SECRET_TEXT, TOKEN_TEXT} from './products-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex, VmHWM} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-2-blob');
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
const H = {dot: sha('dot'), dotWork: sha('dot-work'), hard: sha('hard'), big: sha('big')};
const MIB = 1048576;
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'runs', 'r');
  writeProductsRun(run);
  put(path.join(run, 'store', '.cache', 'blobs', H.dot), `${TOKEN_TEXT}\n`);
  put(path.join(run, 'store', 'CASE-1', '.cache', 'blobs', H.dotWork), `${TOKEN_TEXT}\n`);
  const outside = path.join(root, 'outside', 'secret.txt');
  put(outside, `${SECRET_TEXT}\n`);
  fs.linkSync(outside, path.join(run, 'store', 'blobs', H.hard));
  const big = path.join(run, 'store', 'blobs', H.big);
  fs.writeFileSync(big, '');
  fs.truncateSync(big, 300 * MIB);
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

const blob = async (rel, hash, timeout = 10000) => ask(base, `/api/blob?source=${await sourceIndex(base, 'r')}&case=${encodeURIComponent(rel)}&hash=${hash}`, {timeout});

test('control: a blob of store/blobs/ is served', async () => {
  const r = await blob('.', OUTPUT_DIGEST['notes.check']);
  expect(r.status).toBe(200);
  expect(r.text).toContain(OUTPUT['notes.check'].slice(0, 20));
});

test('rule: a path with a dot-directory component is neither listed nor served: /api/blob names one in `case`', async () => {
  for (const [rel, h] of [['.cache', H.dot], ['CASE-1/.cache', H.dotWork]]) {
    const r = await blob(rel, h);
    expect(`${rel}: ${r.status} ${r.text.includes(TOKEN_TEXT) ? 'served the text of a dot-directory' : ''}`).toBe(`${rel}: 404 `);
  }
});

test('the fix round refuses a file with more than one link; the blob route too: a blob that is a hard link (more than one link) is not served', async () => {
  const r = await blob('.', H.hard);
  expect(`${r.status} ${r.text.includes(SECRET_TEXT) ? 'served the text of the other name' : ''}`).not.toMatch(/^200/);
  expect(r.text).not.toContain(SECRET_TEXT);
});

test('finding 5 again: a blob of 300 MB is not read whole into the server (peak memory grows by less than 100 MB)', async () => {
  test.setTimeout(60000);
  const pid = await srv.started;
  const before = VmHWM(pid);
  const r = await blob('.', H.big, 40000);
  const after = VmHWM(pid);
  const grew = Math.round((after - before) / MIB);
  console.log(`blob of 300 MB: status ${r.status}, ${r.ms} ms, text ${r.text.length} chars, peak memory grew by ${grew} MiB`);
  expect(grew, `the server's peak memory grew by ${grew} MiB for one request`).toBeLessThan(100);
});
