// What a step produced, as the inspector's server answers it (story inspector-shows-step-products): the files of
// a run (/api/products, /api/file) and the output of an action (/api/output). A run of `runbook run` of a runbook
// that is not SDC (protocol example@1), written by hand with invented text (test/products-fixture.mjs). Every
// read goes through the run-path check; a regular file only, whatever the request names; a dot-directory is
// neither listed nor served; the content comes as JSON with nosniff; no route acts. Own servers on ports the
// kernel gives (test/serve.mjs). No browser.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeProductsRun, viewWith, ROOT, OUTPUT, TOKEN_TEXT, SECRET_TEXT} from './products-fixture.mjs';
import {readRegular} from '../runfiles.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(FIXTURE_DIR, 'step-products-api');
const servers = {};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => Object.values(servers).forEach((s) => s.proc.kill()));

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const view = (name, v) => { const f = path.join(root, `${name}.view.json`); fs.mkdirSync(root, {recursive: true}); fs.writeFileSync(f, JSON.stringify(v)); return f; };
  writeProductsRun(path.join(root, 'a', 'viewed'), {product: 'view'});
  writeProductsRun(path.join(root, 'b', 'missing'), {product: 'missing'});
  writeProductsRun(path.join(root, 'c', 'convention'), {product: 'convention'});
  writeProductsRun(path.join(root, 'c', 'newest'), {product: 'newest'});
  const [a, b, c] = await Promise.all([
    startServer(['--view', view('a', viewWith('work/example/example.md')), '--runs', path.join(root, 'a')]),
    startServer(['--view', view('b', viewWith('work/example/not-written.md')), '--runs', path.join(root, 'b')]),
    startServer(['--no-view', '--runs', path.join(root, 'c')]),
  ]);
  Object.assign(servers, {a, b, c});
});

const get = async (name, p) => {
  const r = await fetch(servers[name].url + p);
  const text = await r.text();
  let json = null;
  try { json = JSON.parse(text); } catch { /* not JSON */ }
  return {status: r.status, headers: r.headers, text, json};
};
const file = (name, rel, src = 0) => get(name, `/api/file?source=${src}&path=${encodeURIComponent(rel)}`);
const E = 'work/example';

test('the listing names the files of work/ with the result of a step file beside its name, and leaves out what it must', async () => {
  const r = await get('a', '/api/products?source=0');
  expect(r.status).toBe(200);
  const by = Object.fromEntries(r.json.files.map((f) => [f.path, f]));
  for (const p of [`${E}/notes.txt`, `${E}/check.txt`, `${E}/report.md`, `${E}/example.md`, `${E}/data.json`, `${E}/sub/inner.txt`, `${E}/alias.txt`, `${E}/big.txt`, `${E}/blob.bin`, 'work/idea.yaml', 'work/notes.txt']) expect(by[p], p).toBeTruthy();
  expect(by[`${E}/notes.txt`].result).toBe('valid');
  expect(by[`${E}/check.txt`].result).toBeNull();
  expect(by[`${E}/report.md`].format).toBe('markdown');
  expect(by[`${E}/data.json`].format).toBe('json');
  expect(by['work/idea.yaml'].format).toBe('yaml');
  expect(by[`${E}/notes.txt`].size).toBeGreaterThan(10);
  // not listed: a dot-directory and a dot-file, a link that leaves the run, a link into a dot-directory, a pipe, a device
  const paths = Object.keys(by).join('\n');
  for (const bad of ['.cache', '.hidden', 'leak.txt', 'viatoken.txt', 'pipe.fifo', 'null.dev']) expect(paths, bad).not.toContain(bad);
  expect(r.json.hiddenDirectories).toBe(1);
  expect(r.json.hiddenFiles).toBe(1);
  expect(r.text).not.toContain(TOKEN_TEXT);
  expect(r.text).not.toContain(SECRET_TEXT);
});

test('a text file is answered as text, Markdown, JSON and YAML with their format, in a JSON text member', async () => {
  const t = await file('a', `${E}/notes.txt`);
  expect(t.status).toBe(200);
  expect(t.json).toMatchObject({path: `${E}/notes.txt`, kind: 'text', format: 'text', truncated: false});
  expect(t.json.text).toBe('subject: notes\nlines: 3\nresult: valid\nnote: an invented note\n');
  expect(t.json.bytes).toBe(Buffer.byteLength(t.json.text));
  expect((await file('a', `${E}/report.md`)).json.format).toBe('markdown');
  const j = await file('a', `${E}/data.json`);
  expect(j.json.format).toBe('json');
  expect(JSON.parse(j.json.text).counts.notes).toBe(3);
  expect((await file('a', 'work/idea.yaml')).json.format).toBe('yaml');
  // a link that stays in the run is read
  expect((await file('a', `${E}/alias.txt`)).json.text).toContain('# Example report');
});

test('every answer is JSON with nosniff, the content is never served as a type a browser renders, and no run file has its own type', async () => {
  for (const rel of [`${E}/hostile.md`, `${E}/report.md`, `${E}/data.json`, `${E}/notes.txt`, `${E}/blob.bin`, `${E}/leak.txt`, `${E}/pipe.fifo`, `${E}/nothing.txt`]) {
    const r = await file('a', rel);
    expect(r.headers.get('content-type'), rel).toBe('application/json; charset=utf-8');
    expect(r.headers.get('x-content-type-options'), rel).toBe('nosniff');
    expect(r.headers.get('content-disposition'), rel).toBeNull();
    expect(() => JSON.parse(r.text), rel).not.toThrow();
  }
  const hostile = await file('a', `${E}/hostile.md`);
  expect(hostile.json.text).toContain('<script>window.__pwned = 1</script>');
  const list = await get('a', '/api/products?source=0');
  expect(list.headers.get('x-content-type-options')).toBe('nosniff');
});

test('a file that is not text is answered with its size and type and no content', async () => {
  for (const rel of [`${E}/blob.bin`, `${E}/latin1.txt`]) {
    const r = await file('a', rel);
    expect(r.status, rel).toBe(200);
    expect(r.json.kind, rel).toBe('binary');
    expect(r.json.size, rel).toBeGreaterThan(0);
    expect(r.json.text, rel).toBeUndefined();
    expect(r.json.type, rel).toBe('not valid UTF-8 text');
  }
  expect((await file('a', `${E}/blob.bin`)).json.size).toBe(7);
});

test('a file over the display limit is cut at 1 MiB and the answer carries its full byte count', async () => {
  const r = await file('a', `${E}/big.txt`);
  expect(r.status).toBe(200);
  expect(r.json.kind).toBe('text');
  expect(r.json.truncated).toBe(true);
  expect(r.json.limit).toBe(1048576);
  expect(r.json.bytes).toBe(1048576 + 10);
  expect(Buffer.byteLength(r.json.text)).toBe(1048576);
});

test('a link that leaves the run, a link into a dot-directory, a pipe, a device and a directory are refused, and the server goes on', async () => {
  const leak = await file('a', `${E}/leak.txt`);
  expect(leak.status).toBe(403);
  expect(leak.json.error).toMatch(/leaves the run/);
  expect(leak.text).not.toContain(SECRET_TEXT);
  const dev = await file('a', `${E}/null.dev`);
  expect(dev.status).toBe(403);
  const started = Date.now();
  const pipe = await file('a', `${E}/pipe.fifo`);
  expect(Date.now() - started).toBeLessThan(5000);
  expect(pipe.status).toBe(415);
  expect(pipe.json.type).toBe('named pipe');
  expect(pipe.json.error).toMatch(/not a regular file/);
  const dir = await file('a', `${E}/sub`);
  expect(dir.status).toBe(415);
  expect(dir.json.type).toBe('directory');
  expect((await get('a', '/api/sources')).status).toBe(200);
  // the device itself, as the stat of a file the check lets through: refused by its type, never opened
  const direct = readRegular('/dev/null', {root: '/dev', limit: 16});
  expect(direct).toMatchObject({state: 'refused', type: 'character device'});
});

test('a dot-directory and a dot-file are never served: 404 for the path, for an encoded dot, and for a link into one', async () => {
  for (const rel of ['work/.cache/token.txt', `${E}/.hidden.txt`, 'work/%2ecache/token.txt', `${E}/viatoken.txt`, 'work/.cache', 'work/./.cache/token.txt', 'work/example/../.cache/token.txt']) {
    const r = await get('a', `/api/file?source=0&path=${encodeURIComponent(rel).replace(/%252e/gi, '%2e')}`);
    expect([400, 404], rel).toContain(r.status);
    expect(r.text, rel).not.toContain(TOKEN_TEXT);
  }
  const plain = await file('a', 'work/.cache/token.txt');
  expect(plain.status).toBe(404);
  expect(plain.json.error).toMatch(/dot/);
  const viaLink = await file('a', `${E}/viatoken.txt`);
  expect(viaLink.status).toBe(404);
  const list = await get('a', '/api/products?source=0');
  expect(list.json.hiddenDirectories + list.json.hiddenFiles).toBe(2);
});

test('a path that is absolute, leaves the run, is missing or is empty is refused with a clear answer', async () => {
  const cases = [['/etc/hostname', 400], ['../outside/secret.txt', 400], ['work/../../outside/secret.txt', 400], ['outside/secret.txt', 404], [`${E}/nothing.txt`, 404], ['', 400], ['work/example\u0000/notes.txt', 400]];
  for (const [rel, status] of cases) {
    const r = await file('a', rel);
    expect(r.status, JSON.stringify(rel)).toBe(status);
    expect(r.json?.error, JSON.stringify(rel)).toBeTruthy();
    expect(r.text).not.toContain(SECRET_TEXT);
  }
  expect((await get('a', '/api/file?source=0')).status).toBe(400);
  expect((await get('a', '/api/file?source=9&path=work/idea.yaml')).status).toBe(404);
});

test('the new routes read and nothing else: another method is 405, a request for another host is 421', async () => {
  for (const p of ['/api/products?source=0', `/api/file?source=0&path=${E}/notes.txt`, '/api/output?source=0&case=.&seq=3']) {
    for (const method of ['POST', 'PUT', 'DELETE', 'PATCH']) {
      const r = await fetch(servers.a.url + p, {method, body: method === 'DELETE' ? undefined : '{}'});
      expect(r.status, `${method} ${p}`).toBe(405);
    }
    const host = await new Promise((ok, no) => {
      const q = http.request({host: '127.0.0.1', port: servers.a.port, path: p, headers: {host: 'evil.example:80'}}, (res) => { res.resume(); res.on('end', () => ok(res.statusCode)); });
      q.on('error', no);
      q.end();
    });
    expect(host, p).toBe(421);
    const origin = await new Promise((ok, no) => {
      const q = http.request({host: '127.0.0.1', port: servers.a.port, path: p, headers: {origin: 'http://evil.example'}}, (res) => { res.resume(); res.on('end', () => ok(res.statusCode)); });
      q.on('error', no);
      q.end();
    });
    expect(origin, p).toBe(421);
  }
  const src = fs.readFileSync(path.join(here, '..', 'server.mjs'), 'utf8') + fs.readFileSync(path.join(here, '..', 'products.mjs'), 'utf8');
  expect(src).not.toMatch(/child_process|execSync|execFile|spawn|writeFile|appendFile|unlink|rmSync|mkdirSync|rename\(|copyFile|truncateSync/);
});

test('the product of a run: the view names it; else work/<runbook>/<runbook>.md; else the newest Markdown files, said as which', async () => {
  const a = (await get('a', '/api/products?source=0')).json.product;
  expect(a).toMatchObject({via: 'view', path: `${E}/example.md`, missing: false});
  const b = (await get('b', '/api/products?source=0')).json.product;
  expect(b).toMatchObject({via: 'view', path: `${E}/not-written.md`, missing: true});
  expect(b.note).toBe(`the view names ${E}/not-written.md; this run has not written it`);
  const conv = (await get('c', '/api/products?source=0')).json.product;
  expect(conv).toMatchObject({via: 'runbook-file', path: `${E}/example.md`, missing: false});
  const newest = (await get('c', '/api/products?source=1')).json.product;
  expect(newest.via).toBe('newest-markdown');
  expect(newest.path).toBe(`${E}/report.md`);
  expect(newest.others[0].path).toBe(`${E}/hostile.md`);
  expect(newest.others.map((o) => o.path)).not.toContain(`${E}/report.md`);
  expect(newest.others.every((o) => /\.md$/.test(o.path))).toBe(true);
});

test('the output of an action is the tail of the blob the run log names, said as one stream; where nothing is kept it says so', async () => {
  const tree = (await get('a', '/api/tree?source=0')).json;
  const c = tree.cases.find((x) => x.id === ROOT);
  const seqOf = (action) => c.events.find((e) => e.event === 'action_finished' && e.action === action).seq;
  const out = (action) => get('a', `/api/output?source=0&case=${encodeURIComponent(c.rel)}&seq=${seqOf(action)}`);
  const check = (await out('notes.check')).json;
  expect(check).toMatchObject({recorded: true, truncated: false, text: OUTPUT['notes.check'], stream: 'standard output followed by standard error'});
  const write = (await out('notes.write')).json;
  expect(write.recorded).toBe(true);
  expect(write.truncated).toBe(true);
  expect(write.bytes).toBe(Buffer.byteLength(OUTPUT['notes.write']));
  expect(write.text.endsWith('writer finished\n')).toBe(true);
  expect(Buffer.byteLength(write.text)).toBeLessThanOrEqual(4096);
  expect(OUTPUT['notes.write'].endsWith(write.text)).toBe(true);
  const failed = (await out('report.check')).json;
  expect(failed.text).toContain('error: no heading named Summary');
  const lost = (await out('report.write')).json;
  expect(lost.recorded).toBe(false);
  expect(lost.why).toMatch(/not in the run/);
  const person = (await out('report.approve')).json;
  expect(person.recorded).toBe(false);
  expect(person.why).toMatch(/names no output/);
  // a request that names no action finish is refused
  const bad = await get('a', `/api/output?source=0&case=${encodeURIComponent(c.rel)}&seq=0`);
  expect(bad.status).toBe(404);
  expect((await get('a', '/api/output?source=0&case=..&seq=1')).status).toBe(404);
});
