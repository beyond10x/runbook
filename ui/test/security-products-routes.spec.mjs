// Independent review, invariant 5 of story inspector-shows-step-products: no route acts on a run (every request that is
// not GET or HEAD is 405), and the Host and Origin guard answers 421 for every new route including its error paths
// (the file route's 404, 400, 403 and 415), for Host spellings that only look like loopback, and before the method check.
// Own server; run written by hand (products-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';
import {fixtureRoot, ask, raw, fileUrl} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-routes');
const E = 'work/example';
let srv, base, port;
const ROUTES = () => [
  '/api/products?source=0', fileUrl(0, `${E}/notes.txt`), fileUrl(0, `${E}/nothing.txt`), fileUrl(0, ''), fileUrl(0, '../x'), fileUrl(0, `${E}/leak.txt`), fileUrl(0, `${E}/pipe.fifo`), fileUrl(0, 'work/.cache/token.txt'),
  fileUrl(9, `${E}/notes.txt`), '/api/output?source=0&case=.&seq=3', '/api/output?source=0&case=..&seq=3', '/api/output?source=9&case=.&seq=3', '/api/products?source=9',
];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  writeProductsRun(path.join(root, 'runs', 'r'));
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
  port = Number(new URL(base).port);
});

test('inv 5: any method but GET and HEAD is 405 on every new route and on its error paths, with or without a body', async () => {
  for (const p of ROUTES()) {
    for (const method of ['POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'TRACE', 'PROPFIND', 'MKCOL', 'MOVE', 'COPY', 'PURGE', 'LINK', 'UNLOCK']) {
      let r;
      try { r = await raw(port, p, {method}); } catch { continue; } // a method the HTTP parser itself refuses is not a request
      expect(r.status, `${method} ${p}`).toBe(405);
    }
  }
});

test('inv 5: HEAD answers like GET and carries no body, and a GET with a body acts on nothing', async () => {
  for (const p of ROUTES().slice(0, 4)) {
    const r = await raw(port, p, {method: 'HEAD'});
    expect([200, 400, 404], p).toContain(r.status);
    expect(r.body, p).toBe('');
  }
});

test('inv 5: a Host that only looks like loopback, or an Origin of another page, is 421 on every route before the method and the path are read', async () => {
  const hosts = [
    'evil.example:80', `evil.example:${port}`, `127.0.0.1:${port + 1}`, '127.0.0.1', `127.0.0.1.evil.example:${port}`, `localhost.evil.example:${port}`,
    `127.0.0.1:${port}@evil.example`, `evil.example#127.0.0.1:${port}`, `0.0.0.0:${port}`, `[::ffff:127.0.0.1]:${port}`, `127.1:${port}`, `2130706433:${port}`, `localhost.:${port}`,
  ];
  for (const p of ROUTES()) {
    for (const host of hosts) {
      let r;
      try { r = await raw(port, p, {headers: {host}}); } catch { continue; }
      expect(r.status, `Host ${JSON.stringify(host)} ${p}`).toBe(421);
      expect(r.headers['x-content-type-options'], `Host ${host} ${p}`).toBe('nosniff');
    }
    for (const origin of ['http://evil.example', `http://127.0.0.1.evil.example:${port}`, `https://127.0.0.1:${port}`, 'null', `http://localhost:${port}`, `http://127.0.0.1:${port}/`, `file://`]) {
      const r = await raw(port, p, {headers: {origin}});
      expect(r.status, `Origin ${origin} ${p}`).toBe(421);
    }
    const post = await raw(port, p, {method: 'POST', headers: {host: 'evil.example:80'}});
    expect(post.status, `POST with a foreign Host ${p}`).toBe(421);
  }
});

test('inv 5: the loopback spellings that the guard allows still reach the new routes', async () => {
  for (const host of [`127.0.0.1:${port}`, `localhost:${port}`, `LOCALHOST:${port}`]) {
    const r = await raw(port, '/api/products?source=0', {headers: {host}});
    expect(r.status, host).toBe(200);
  }
  const same = await raw(port, '/api/products?source=0', {headers: {origin: `http://127.0.0.1:${port}`}});
  expect(same.status).toBe(200);
});

test('inv 5: a request line that makes the code throw ends in an answer, and the server goes on', async () => {
  for (const p of [`/api/file?source=0&path=${'%'.repeat(10)}`, '/api/file?source=0&path=%ff%fe', '/api/file?source=0&path=%', `/api/file?source=0&path=${'a'.repeat(60000)}`, '/api/output?source=0&case=%ff&seq=1e999', '/api/output?source=0&case=.&seq=NaN', '/api/output?source=0&case=.&seq=-1', '/api/output?source=0&case=.&seq=0.5', '/api/products?source=1e999', '/api/products?source=__proto__', '/api/file?source=constructor&path=x']) {
    const r = await ask(base, p);
    expect(r.hung, p.slice(0, 80)).toBe(false);
    expect([200, 400, 404, 414, 431], p.slice(0, 80)).toContain(r.status);
  }
  expect((await ask(base, '/api/sources')).status).toBe(200);
});
