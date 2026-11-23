// Independent review, invariants 1 to 3 and 6 of story inspector-shows-step-products: every read of a run file goes
// through the one run-path check, only regular files are opened, no dot component is listed or served by any switch,
// the display cut reads at most the limit, a file that looks like a page is never served as one.
// Own server on a port the kernel gives; the run is written by hand (products-fixture.mjs); invented text only.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, viewWith, TOKEN_TEXT, SECRET_TEXT} from './products-fixture.mjs';
import {EXAMPLE_VIEW} from './loom-run-fixture.mjs';
import {checkView} from '../web/view.js';
import {fixtureRoot, put, ask, fileUrl, VmHWM} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-paths');
const E = 'work/example';
const GIB = 2 * 1024 * 1024 * 1024;
let srv, base, sock, srvPid;
const extra = {};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); sock?.close(); Object.values(extra).forEach((s) => s.proc.kill()); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const dir = path.join(root, 'runs', 'hostile');
  writeProductsRun(dir);
  const W = path.join(dir, 'work', 'example');
  const outside = path.join(root, 'outside', 'dir');
  put(path.join(outside, 'secret.txt'), `${SECRET_TEXT}\n`);
  fs.symlinkSync(outside, path.join(W, 'outdir')); // a plain-named link to a directory outside the run
  fs.symlinkSync('../.cache', path.join(W, 'cachelink')); // a plain-named link to a dot-directory inside the run
  fs.linkSync(path.join(outside, 'secret.txt'), path.join(W, 'hard.txt')); // a hard link to a file outside the run
  put(path.join(W, 'x.html'), '<!doctype html><script>window.__pwned = 1</script><h1>x</h1>');
  put(path.join(W, 'x.svg'), '<svg xmlns="http://www.w3.org/2000/svg" onload="window.__pwned = 2"><script>window.__pwned = 3</script></svg>');
  // a unix socket path holds at most 107 bytes: bind it by a relative name from inside the directory
  const cwd = process.cwd();
  process.chdir(W);
  await new Promise((ok) => { sock = net.createServer().listen('sock.s', ok); });
  process.chdir(cwd);
  const fd = fs.openSync(path.join(W, 'sparse.md'), 'w');
  fs.writeSync(fd, Buffer.alloc(1100000, 0x61));
  fs.ftruncateSync(fd, GIB);
  fs.closeSync(fd);
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
  srvPid = await srv.started;
});

const NO_LEAK = (r, label) => { expect(r.hung, `${label}: no answer`).toBe(false); expect(r.text, label).not.toContain(SECRET_TEXT); expect(r.text, label).not.toContain(TOKEN_TEXT); };

test('inv 1: a path that climbs, is encoded twice, is absolute, holds a look-alike or is very long never reads outside the run', async () => {
  const literal = [
    '/etc/passwd', 'file:///etc/passwd', '\\etc\\passwd', 'work\\example\\..\\..\\..\\outside', 'work/example/..\\..\\x',
    `${E}/outdir/secret.txt`, `${E}/outdir`, `${E}/leak.txt/`, `${E}/leak.txt/.`, `${E}/notes.txt/..`,
    'work/․․/x', `${E}/．．/．．/outside/dir/secret.txt`, `${E}/‥/‥/outside/dir/secret.txt`, `${E}/..%2foutside`,
    `work/%2e%2e/%2e%2e/outside/dir/secret.txt`, 'a'.repeat(5000), `${'a/'.repeat(3000)}x`, `work/${'./'.repeat(40000)}.cache/token.txt`,
    'work//.cache//token.txt', 'work/.cache/token.txt/', 'WORK/.CACHE/token.txt', ` ${E}/.hidden.txt`, `${E}/.hidden.txt/`, `${E}/./.hidden.txt`, `${E}/sub/../.hidden.txt`,
  ];
  for (const rel of literal) {
    const r = await ask(base, fileUrl(0, rel));
    NO_LEAK(r, JSON.stringify(rel).slice(0, 80));
    expect(r.status, JSON.stringify(rel).slice(0, 80)).not.toBe(200);
    expect([400, 403, 404, 414, 415, 431], JSON.stringify(rel).slice(0, 80)).toContain(r.status);
  }
  for (const encoded of ['%2e%2e%2foutside%2fdir%2fsecret.txt', 'work%2F.cache%2Ftoken.txt', 'work/%252e%252e/%252e%252e/outside/dir/secret.txt', 'work/example/%2e%2e%2f%2e%2e%2f%2e%2e%2foutside', '%00', 'work/example/notes.txt%00.md']) {
    const r = await ask(base, `/api/file?source=0&path=${encoded}`);
    NO_LEAK(r, encoded);
    expect(r.status, encoded).not.toBe(200);
  }
  expect((await ask(base, '/api/sources')).status).toBe(200);
});

test('inv 1: a plain-named link to a directory outside the run, or to a dot-directory inside it, serves nothing and lists nothing', async () => {
  for (const rel of [`${E}/outdir/secret.txt`, `${E}/cachelink/token.txt`]) {
    const r = await ask(base, fileUrl(0, rel));
    NO_LEAK(r, rel);
    expect([403, 404], rel).toContain(r.status);
  }
  // a request that names no dot is answered as any other path that is not served: the answer does not say what the target is
  const viaLink = await ask(base, fileUrl(0, `${E}/cachelink/token.txt`));
  const elsewhere = await ask(base, fileUrl(0, 'store/none.txt'));
  expect(viaLink.json.error).not.toMatch(/dot/i);
  expect(viaLink.json.error).toBe(elsewhere.json.error);
  const list = await ask(base, '/api/products?source=0');
  NO_LEAK(list, 'listing');
  expect(list.json.files.map((f) => f.path).filter((p) => /outdir|cachelink|token|secret|\.cache|\.hidden/.test(p))).toEqual([]);
  expect(list.json.hiddenDirectories).toBe(1);
  expect(list.json.hiddenFiles).toBe(1);
  expect(JSON.stringify(list.json)).not.toMatch(/\.cache|\.hidden|cachelink|outdir/);
});

test('inv 1 (note): a hard link to a file outside the run is a regular file of the run for every path check', async () => {
  const r = await ask(base, fileUrl(0, `${E}/hard.txt`));
  // Recorded, not asserted as a defect: no path check can tell it from a file of the run (st_nlink is the only mark).
  console.log(`hard link to a file outside the run: status ${r.status}, content served: ${r.text.includes(SECRET_TEXT)}`);
  expect(r.hung).toBe(false);
});

test('inv 3: no query parameter, header or view member turns the dot filter off', async () => {
  const switches = ['hidden=1', 'dot=1', 'dots=1', 'dotfiles=allow', 'all=1', 'showHidden=true', 'show_hidden=true', 'raw=1', 'download=1', 'debug=1', 'include=.cache', 'format=html'];
  const base0 = fileUrl(0, 'work/.cache/token.txt');
  for (const sw of switches) {
    const r = await ask(base, `${base0}&${sw}`);
    NO_LEAK(r, sw);
    expect(r.status, sw).toBe(404);
    const l = await ask(base, `/api/products?source=0&${sw}`);
    NO_LEAK(l, `products ${sw}`);
    expect(JSON.stringify(l.json), sw).not.toMatch(/\.cache|\.hidden/);
  }
  for (const headers of [{'x-show-hidden': '1'}, {'x-dotfiles': 'allow'}, {accept: 'text/html'}, {'x-forwarded-for': '127.0.0.1'}, {range: 'bytes=0-10'}]) {
    const r = await ask(base, base0, {headers});
    NO_LEAK(r, JSON.stringify(headers));
    expect(r.status, JSON.stringify(headers)).toBe(404);
  }
  for (const member of ['hidden', 'dotfiles', 'show_hidden', 'show_dot_files']) {
    expect(checkView({...EXAMPLE_VIEW, [member]: true}).join(' '), member).toContain(member);
  }
});

test('inv 3: a view product that is a plain-named link into a dot-directory, or a pipe, is reported missing and is not read', async () => {
  for (const [name, product] of [['cachelink', `${E}/cachelink/token.txt`], ['fifo', `${E}/pipe.fifo`], ['dir', `${E}/sub`], ['leak', `${E}/leak.txt`]]) {
    const f = path.join(root, `${name}.view.json`);
    fs.writeFileSync(f, JSON.stringify(viewWith(product)));
    extra[name] = await startServer(['--view', f, '--runs', path.join(root, 'runs')]);
    const r = await ask(extra[name].url, '/api/products?source=0');
    NO_LEAK(r, name);
    expect(r.json.product, name).toMatchObject({via: 'view', path: product, missing: true});
    extra[name].proc.kill();
  }
});

test('inv 2: a pipe, a socket, a device link and a directory are answered at once and never opened', async () => {
  for (const [rel, status] of [[`${E}/pipe.fifo`, 415], [`${E}/sock.s`, 415], [`${E}/sub`, 415], [`${E}/null.dev`, 403]]) {
    const r = await ask(base, fileUrl(0, rel), {timeout: 4000});
    expect(r.hung, rel).toBe(false);
    expect(r.status, rel).toBe(status);
  }
  const list = await ask(base, '/api/products?source=0', {timeout: 6000});
  expect(list.hung).toBe(false);
  expect(list.json.files.map((f) => f.path).filter((p) => /pipe|sock|null\.dev/.test(p))).toEqual([]);
});

test('inv 6: a file named x.html or x.svg is JSON text with nosniff, never a document, in the browser too', async ({page}) => {
  for (const name of ['x.html', 'x.svg']) {
    const r = await ask(base, fileUrl(0, `${E}/${name}`));
    expect(r.status, name).toBe(200);
    expect(r.headers.get('content-type'), name).toBe('application/json; charset=utf-8');
    expect(r.headers.get('x-content-type-options'), name).toBe('nosniff');
    expect(r.headers.get('content-disposition'), name).toBeNull();
    const dialogs = [];
    page.on('dialog', (d) => { dialogs.push(d.message()); d.dismiss(); });
    await page.goto(base + fileUrl(0, `${E}/${name}`));
    expect(await page.evaluate(() => document.contentType), name).toBe('application/json');
    expect(await page.evaluate(() => window.__pwned), name).toBeUndefined();
    expect(dialogs).toEqual([]);
  }
  for (const route of ['/api/products?source=0', '/api/output?source=0&case=.&seq=1', fileUrl(0, `${E}/nothing.txt`), fileUrl(0, '')]) {
    const r = await ask(base, route);
    expect(r.headers.get('x-content-type-options'), route).toBe('nosniff');
    expect(r.headers.get('content-type'), route).toMatch(/^application\/json/);
  }
});

test('inv 6: a file of 2 GiB is cut at 1 MiB by reading 1 MiB, and the server does not grow by the file', async () => {
  const before = VmHWM(srvPid);
  const t0 = Date.now();
  const r = await ask(base, fileUrl(0, `${E}/sparse.md`), {timeout: 20000});
  expect(r.hung).toBe(false);
  expect(r.status).toBe(200);
  expect(r.json.kind).toBe('text');
  expect(r.json.truncated).toBe(true);
  expect(r.json.bytes).toBe(GIB);
  expect(Buffer.byteLength(r.json.text)).toBe(1048576);
  expect(Date.now() - t0).toBeLessThan(5000);
  const after = VmHWM(srvPid);
  if (before != null && after != null) expect(after - before, `peak resident memory grew by ${after - before} bytes`).toBeLessThan(64 * 1024 * 1024);
  const list = await ask(base, '/api/products?source=0');
  expect(list.json.files.find((f) => f.path === `${E}/sparse.md`).size).toBe(GIB);
});
