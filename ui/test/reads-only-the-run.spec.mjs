// The inspector reads only the run it was started on, and answers only requests meant for it. A run
// directory is written by a confined model, so a link inside it must not make the server read, list or
// serve a file outside it; a request must not be able to end the server; a request that names another
// host or comes from a page of another origin is refused (421) before any route.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'reads-only-the-run');
const HASH = 'a'.repeat(64);
const HASH2 = 'b'.repeat(64);
const HASH3 = 'c'.repeat(64);
const MARK = 'OUTSIDE-THE-RUN-MARKER';
const QUESTION = (word) => `questions:\n- id: q1\n  item: i1\n  question: ${word} may the scope grow?\n  status: open\n  scope_changing: true\n`;
let base, port, srv;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

const caseDir = (store, id) => {
  const c = path.join(store, id);
  fs.mkdirSync(path.join(c, 'work', 'aep'), {recursive: true});
  fs.mkdirSync(path.join(c, 'blobs'), {recursive: true});
  fs.writeFileSync(path.join(c, 'case.json'), JSON.stringify({id, protocol: 'sdc.intake'}) + '\n');
  fs.writeFileSync(path.join(c, 'events.jsonl'), '');
  fs.writeFileSync(path.join(c, 'bindings.json'), JSON.stringify({$artifacts: {questions: 'questions.yaml'}}) + '\n');
  return c;
};

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const store = path.join(root, 'run', 'store');
  const outside = path.join(root, 'outside');
  fs.mkdirSync(path.join(outside, 'dir'), {recursive: true});
  fs.writeFileSync(path.join(outside, 'secret.md'), `${MARK}\n`);
  fs.writeFileSync(path.join(outside, 'dir', 'inner.md'), `${MARK}\n`);
  fs.writeFileSync(path.join(outside, 'questions.yaml'), QUESTION(MARK));
  const c = caseDir(store, 'C1');
  fs.writeFileSync(path.join(c, 'work', 'aep', 'inside.md'), 'INSIDE-THE-RUN-MARKER\n');
  fs.symlinkSync(path.join(outside, 'secret.md'), path.join(c, 'work', 'aep', 'leak.md'));
  fs.symlinkSync(path.join(outside, 'dir'), path.join(c, 'work', 'aep', 'leakdir'));
  fs.symlinkSync(path.join(c, 'work', 'aep', 'inside.md'), path.join(c, 'work', 'aep', 'alias.md'));
  fs.symlinkSync(path.join(outside, 'secret.md'), path.join(c, 'blobs', HASH));
  fs.writeFileSync(path.join(c, 'blobs', HASH3), 'INSIDE-THE-RUN-MARKER\n');
  fs.symlinkSync(path.join(c, 'blobs', HASH3), path.join(c, 'blobs', HASH2));
  fs.symlinkSync(path.join(outside, 'questions.yaml'), path.join(c, 'work', 'questions.yaml'));
  const d = caseDir(store, 'C2');
  fs.writeFileSync(path.join(d, 'work', 'questions.yaml'), QUESTION('INSIDE'));
  ({url: base, proc: srv} = await startServer([store]));
  port = new URL(base).port;
});

const raw = (headers, p = '/api/sources') => new Promise((ok, no) => {
  const r = http.request({host: '127.0.0.1', port, path: p, method: 'GET', headers}, (res) => {
    let body = '';
    res.on('data', (d) => { body += d; });
    res.on('end', () => ok({status: res.statusCode, body}));
  });
  r.on('error', no);
  r.end();
});

test('a link inside the run to a file outside it is not read into /api/workfacts, and one inside the run is', async ({request}) => {
  const body = await (await request.get(`${base}/api/workfacts?source=0`)).text();
  expect(body).not.toContain(MARK);
  expect(body).not.toContain('leak');
  expect(body).toContain('inside.md');
  expect(body).toContain('alias.md');
});

test('a link inside the run to a file outside it is not served by /api/blob, and one inside the run is', async ({request}) => {
  const out = await request.get(`${base}/api/blob?source=0&case=C1&hash=${HASH}`);
  expect(out.status()).toBe(404);
  expect(await out.text()).not.toContain(MARK);
  const within = await request.get(`${base}/api/blob?source=0&case=C1&hash=${HASH2}`);
  expect(within.status()).toBe(200);
  expect(await within.text()).toContain('INSIDE-THE-RUN-MARKER');
});

test('an artifact that is a link out of the run is not read into /api/inbox, a plain one is', async ({request}) => {
  const body = await (await request.get(`${base}/api/inbox?source=0`)).text();
  expect(body).toContain('INSIDE may the scope grow?');
  expect(body).not.toContain(MARK);
});

test('a request that names another host is refused with 421, and the loopback names of the server are not', async () => {
  for (const host of ['rebind.example', 'rebind.example:' + port, '127.0.0.1', '127.0.0.1:1', 'localhost.example:' + port]) {
    expect((await raw({host})).status, host).toBe(421);
  }
  expect((await raw({host: 'rebind.example'}, '/')).status).toBe(421);
  expect((await raw({host: 'rebind.example'}, '/events')).status).toBe(421);
  for (const host of [`127.0.0.1:${port}`, `localhost:${port}`, `LOCALHOST:${port}`]) {
    expect((await raw({host})).status, host).toBe(200);
  }
});

test('a request with an Origin of another origin is refused with 421, one of the own origin is not', async () => {
  expect((await raw({host: `127.0.0.1:${port}`, origin: 'http://evil.example'})).status).toBe(421);
  expect((await raw({host: `127.0.0.1:${port}`, origin: 'null'})).status).toBe(421);
  expect((await raw({host: `127.0.0.1:${port}`, origin: `http://localhost:${port}`})).status).toBe(421);
  expect((await raw({host: `127.0.0.1:${port}`, origin: `http://127.0.0.1:${port}`})).status).toBe(200);
  expect((await raw({host: `localhost:${port}`, origin: `http://localhost:${port}`})).status).toBe(200);
});

test('a request line the URL parser refuses is answered 400 and does not end the server', async ({request}) => {
  for (const p of ['//a:99999/', '//[/', '//a:b:c/']) {
    const r = await raw({host: `127.0.0.1:${port}`}, p).catch((e) => ({status: String(e.code)}));
    expect(r.status, p).toBe(400);
  }
  expect((await request.get(`${base}/api/sources`)).status()).toBe(200);
});

test('a request value that would throw inside a route is answered, not left to end the server', async ({request}) => {
  for (const p of ['/%', '/%zz', '/api/blob?case=%E0%A4%A&hash=' + HASH, '/api/fschange?seq=%', '/api/tree?cases=%00', '/%00']) {
    const r = await raw({host: `127.0.0.1:${port}`}, p).catch((e) => ({status: String(e.code)}));
    expect([200, 400, 404], p).toContain(r.status);
  }
  expect((await request.get(`${base}/api/sources`)).status()).toBe(200);
});
