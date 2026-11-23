// Independent review, invariant 6 of story inspector-shows-step-products ("a huge directory listing is bounded"):
// the listing of the files of a run (/api/products, products.mjs listFiles) is read at every poll of the page, on the
// server's one thread. It stops adding files at 500, and a run directory is written by a confined model. The checks:
// the time of one listing for a directory of 30000 files, for links that point back at their own directory, and for
// a wide tree of empty directories, each at most 5 s. One server per case: a listing that takes long blocks it.
// Run written by hand (products-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';
import {fileURLToPath} from 'node:url';
import {listFiles} from '../products.mjs';
import {ask} from './security-products-lib.mjs';

// the large trees are built outside test/ (Playwright walks test/ at every start) and removed when the file is done
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'node_modules', '.pw-fixtures', 'security-products-listing');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.beforeAll(({}, info) => { if (info.project.name === 'wide') fs.rmSync(root, {recursive: true, force: true}); });
test.afterAll(() => { fs.rmSync(root, {recursive: true, force: true}); });

async function listing(name, build) {
  const runs = path.join(root, name);
  const dir = path.join(runs, 'r');
  writeProductsRun(dir);
  build(path.join(dir, 'work'));
  const s = await startServer(['--no-view', '--runs', runs]);
  try {
    await ask(s.url, '/api/tree?source=0'); // the page's first reads, so that the cost below is the listing's
    return await ask(s.url, '/api/products?source=0', {timeout: 25000});
  } finally { s.proc.kill(); }
}

test('inv 6: a directory of 30000 files is listed in at most 5 s, 500 of them', async () => {
  test.setTimeout(60000);
  const r = await listing('files', (work) => {
    fs.mkdirSync(path.join(work, 'many'), {recursive: true});
    // 30000 files made by a few processes, not by 30000 calls of the test
    const names = Array.from({length: 30000}, (_, i) => `f${String(i).padStart(5, '0')}.txt`).join('\n');
    spawnSync('xargs', ['-n', '5000', 'touch'], {cwd: path.join(work, 'many'), input: names});
  });
  expect(r.hung).toBe(false);
  expect(r.json.files.length).toBeLessThanOrEqual(500);
  expect(r.json.truncated).toBe(true);
  expect(r.ms, `one listing took ${r.ms} ms`).toBeLessThan(5000);
});

test('inv 6: eight links that point back at their own directory are listed in at most 5 s', async () => {
  test.setTimeout(60000);
  const r = await listing('loops', (work) => {
    for (const n of ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h']) { fs.mkdirSync(path.join(work, 'lp'), {recursive: true}); fs.symlinkSync('.', path.join(work, 'lp', n)); };
  });
  expect(r.hung, 'no answer in 25 s for the listing of a directory with eight links to itself').toBe(false);
  expect(r.ms, `one listing took ${r.ms} ms`).toBeLessThan(5000);
});

test('inv 6: a directory is read an entry at a time and the read stops at the entry bound, not at the end of the directory', () => {
  test.setTimeout(60000);
  const run = path.join(root, 'entries', 'r');
  writeProductsRun(run);
  const many = path.join(run, 'work', 'many');
  fs.mkdirSync(many, {recursive: true});
  const names = Array.from({length: 25000}, (_, i) => `f${String(i).padStart(5, '0')}.txt`).join('\n');
  spawnSync('xargs', ['-n', '5000', 'touch'], {cwd: many, input: names});
  let read = 0;
  const real = fs.opendirSync;
  fs.opendirSync = (p, ...rest) => {
    const d = real(p, ...rest);
    if (path.basename(String(p)) !== 'many') return d;
    return new Proxy(d, {get: (t, k) => (k === 'readSync' ? () => { read++; return t.readSync(); } : typeof t[k] === 'function' ? t[k].bind(t) : t[k])});
  };
  let out;
  try { out = listFiles(run, [path.join(run, 'work')]); } finally { fs.opendirSync = real; }
  expect(out.truncated).toBe(true);
  expect(read, `${read} entries read from a directory of 25000`).toBeGreaterThan(0);
  expect(read, `${read} entries read from a directory of 25000`).toBeLessThanOrEqual(20001);
});

test('inv 6: a tree of 20000 empty directories is listed in at most 5 s', async () => {
  test.setTimeout(60000);
  const r = await listing('dirs', (work) => {
    const dirs = [];
    for (let i = 0; i < 25; i++) for (let j = 0; j < 20; j++) for (let k = 0; k < 40; k++) dirs.push(`d${i}/e${j}/g${k}`);
    fs.mkdirSync(work, {recursive: true});
    spawnSync('xargs', ['-n', '4000', 'mkdir', '-p'], {cwd: work, input: dirs.join('\n')});
  });
  expect(r.hung).toBe(false);
  expect(r.ms, `one listing took ${r.ms} ms`).toBeLessThan(5000);
});
