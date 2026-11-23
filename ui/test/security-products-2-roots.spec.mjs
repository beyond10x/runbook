// Confirmation pass of the independent review, finding 7 of story inspector-shows-step-products (/api/file is confined
// to a work/ directory of the run), with an input the first case did not use, and the silent drops the confinement
// brought with it. Own server, runs written by hand (products-fixture.mjs), invented text.
//
// (a) work/ itself is a link to a directory of the run (to the run directory, to store/): the fix compares the real
//     path of a file with the real path of the root, and the root's real path is then that directory.
// (b) a regular file with more than one link (a work/ copied with `cp -al`, which gives every file two names) is
//     neither listed nor served; the page must at least say that files were left out.
// (c) a directory deeper than the walk's depth is not entered; the listing must say so.
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT} from './products-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex, fileUrl} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-2-roots');
const runs = path.join(root, 'runs');
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});

  const self = path.join(runs, 'selflink');
  writeProductsRun(self);
  fs.rmSync(path.join(self, 'work'), {recursive: true});
  fs.symlinkSync('.', path.join(self, 'work'));

  const st = path.join(runs, 'storelink');
  writeProductsRun(st);
  fs.rmSync(path.join(st, 'work'), {recursive: true});
  fs.symlinkSync('store', path.join(st, 'work'));

  const cp = path.join(runs, 'cpal');
  writeProductsRun(cp);
  const r = spawnSync('cp', ['-al', path.join(cp, 'work', 'example'), path.join(cp, 'work', 'clone')]);
  if (r.status !== 0) throw new Error(`cp -al: ${r.stderr}`);

  const deep = path.join(runs, 'deep');
  writeProductsRun(deep);
  put(path.join(deep, 'work', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'deep.txt'), 'result: deep\n');

  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
});

const listing = async (name) => (await ask(base, `/api/products?source=${await sourceIndex(base, name)}`)).json;
const KNOWN = ['served', 'product', 'files', 'hiddenDirectories', 'hiddenFiles', 'truncated', 'cut'];
const says = (b) => b.truncated === true || b.cut != null || Object.entries(b).some(([k, v]) => !KNOWN.includes(k) && v && v !== 0);

test('finding 7 again: work/ that is a link to the run directory serves no file outside work/', async () => {
  const src = await sourceIndex(base, 'selflink');
  const r = await ask(base, fileUrl(src, 'work/settings.json'));
  expect(r.status, 'work/settings.json is settings.json of the run').not.toBe(200);
  const l = await listing('selflink');
  const paths = (l.files || []).map((f) => f.path);
  expect(paths.filter((p) => /settings\.json|^work\/store\//.test(p)), 'files of the run outside work/ listed as work/').toEqual([]);
});

test('finding 7 again: work/ that is a link to store/ serves no file of store/', async () => {
  const src = await sourceIndex(base, 'storelink');
  const r = await ask(base, fileUrl(src, `work/${ROOT}/events.jsonl`));
  expect(r.status, `work/${ROOT}/events.jsonl is store/${ROOT}/events.jsonl`).not.toBe(200);
  const l = await listing('storelink');
  expect((l.files || []).map((f) => f.path).filter((p) => p.includes('events.jsonl'))).toEqual([]);
});

test('regression: a work/ copied with cp -al (every file has two names) is not silently empty: files are listed or the listing says some were left out', async () => {
  const l = await listing('cpal');
  const paths = (l.files || []).map((f) => f.path);
  const kept = paths.filter((p) => /^work\/(example|clone)\//.test(p));
  console.log(`cp -al: ${kept.length} files of work/example and work/clone listed; keys ${Object.keys(l)}; truncated ${l.truncated}, cut ${l.cut}`);
  expect(kept.length > 0 || says(l), 'no file of work/example or work/clone is listed and the listing says nothing about it').toBe(true);
  const src = await sourceIndex(base, 'cpal');
  if (l.product?.path && !l.product.missing) {
    const f = await ask(base, fileUrl(src, l.product.path));
    expect(f.status, `the listing names ${l.product.path} as the product and the file route answers ${f.status}`).toBe(200);
  }
});

test('regression: a file in a directory deeper than the walk is not silently left out: it is listed or the listing says it was cut', async () => {
  const l = await listing('deep');
  const listed = (l.files || []).some((f) => f.path.endsWith('deep.txt'));
  console.log(`deep: listed ${listed}; truncated ${l.truncated}, cut ${l.cut}`);
  expect(listed || says(l), 'work/a/b/c/d/e/f/g/h/deep.txt is not listed and the listing says nothing').toBe(true);
});
