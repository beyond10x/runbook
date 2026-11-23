// Independent review, invariant 6 of story inspector-shows-step-products ("a huge directory listing is bounded"):
// the listing of the files of a run (/api/products, products.mjs listFiles) is read at every poll of the page, on the
// server's one thread. It stops adding files at 500, and a run directory is written by a confined model. The checks:
// the time of one listing for a directory of 30000 files, for links that point back at their own directory, and for
// a wide tree of empty directories, each at most 5 s. One server per case: a listing that takes long blocks it.
// Run written by hand (products-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';
import {fixtureRoot, ask} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-listing');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.beforeAll(({}, info) => { if (info.project.name === 'wide') fs.rmSync(root, {recursive: true, force: true}); });

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
    for (let i = 0; i < 30000; i++) fs.writeFileSync(path.join(work, 'many', `f${String(i).padStart(5, '0')}.txt`), '');
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

test('inv 6: a tree of 20000 empty directories is listed in at most 5 s', async () => {
  test.setTimeout(60000);
  const r = await listing('dirs', (work) => {
    for (let i = 0; i < 25; i++) for (let j = 0; j < 20; j++) for (let k = 0; k < 40; k++) fs.mkdirSync(path.join(work, `d${i}`, `e${j}`, `g${k}`), {recursive: true});
  });
  expect(r.hung).toBe(false);
  expect(r.ms, `one listing took ${r.ms} ms`).toBeLessThan(5000);
});
