// What a run wrote, as the inspector's server answers it (story products-are-what-the-run-wrote): /api/products lists the
// files the run's records name as written by an action, grouped by step, with the seeded trees as one input entry; a run
// without the records (an older run, one without a watcher, an incomplete seed record) gets today's listing and a note. A
// path a record names that leaves work/ or the run is not listed, not opened and not printed. Own servers on ports the
// kernel gives (test/serve.mjs). No browser, no model, no network.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeWrittenRun, change} from './written-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'written-api');
const SECRET = 'OUTSIDE-THE-RUN-SECRET-5e21';
const servers = {};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => Object.values(servers).forEach((s) => s.proc.kill()));

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const one = (name, opts, after = () => {}) => { const dir = path.join(root, name, 'run'); writeWrittenRun(dir, opts); after(dir); return path.join(root, name); };
  const outside = path.join(root, 'outside-secret.txt');
  fs.mkdirSync(root, {recursive: true});
  fs.writeFileSync(outside, `${SECRET}\n`);
  const many = Array.from({length: 520}, (_, i) => change(0, `bulk/f${String(i).padStart(4, '0')}.txt`, {text: `${i}\n`, step: 3}));
  const dirs = {
    rec: one('rec', {}),
    hostile: one('hostile', {extra: [
      change(0, '../../outside-secret.txt', {text: 'x\n', step: 3}), change(0, '/etc/hostname', {text: 'x\n', step: 3}), change(0, 'leak.txt', {text: 'x\n', step: 3}),
      change(0, 'link-dir/inside.txt', {text: 'x\n', step: 3}), change(0, '.cache/token.txt', {text: 'x\n', step: 3}),
    ]}, (dir) => {
      fs.symlinkSync(outside, path.join(dir, 'work/leak.txt'));
      fs.writeFileSync(path.join(root, 'inside.txt'), `${SECRET}\n`);
      fs.symlinkSync(root, path.join(dir, 'work/link-dir'));
    }),
    bulk: one('bulk', {extra: many}, (dir) => { fs.mkdirSync(path.join(dir, 'work/bulk')); for (let i = 0; i < 520; i++) fs.writeFileSync(path.join(dir, `work/bulk/f${String(i).padStart(4, '0')}.txt`), `${i}\n`); }),
    old: one('old', {seed: false}, (dir) => {
      fs.writeFileSync(path.join(dir, 'work/impl/Cargo.toml'), '[package]\nname = "invented"\n');
      const at = (f, s) => { const t = new Date(Date.parse('2026-11-23T09:00:00Z') + s * 1000); fs.utimesSync(path.join(dir, f), t, t); };
      at('work/aep/story.md', 10); at('work/dissection.md', 20); at('work/impl/README.md', 30);
    }),
    nowatch: one('nowatch', {watch: false}),
    incomplete: one('incomplete', {complete: false}),
  };
  for (const [name, dir] of Object.entries(dirs)) servers[name] = await startServer(['--no-view', '--runs', dir]);
});

const get = async (name, p) => {
  const r = await fetch(servers[name].url + p);
  const text = await r.text();
  let json = null;
  try { json = JSON.parse(text); } catch { /* not JSON */ }
  return {status: r.status, text, json};
};
const products = (name) => get(name, '/api/products?source=0');

test('the answer lists five products in three groups, newest step first, and one Inputs entry with the counts', async () => {
  const r = await products('rec');
  expect(r.status).toBe(200);
  const j = r.json;
  expect(j).toMatchObject({served: true, basis: 'records', total: 5, shown: 5, notShown: 0, truncated: false, cut: null, leftOut: 0, gone: 0, refusedRecords: 0});
  expect(j.groups.map((g) => [g.step, g.files.map((f) => f.path)])).toEqual([
    [3, ['work/impl/src/m7.rs', 'work/repro-result.txt']],
    [2, ['work/dissection.md', 'work/impl/tests/repro.rs']],
    [1, ['work/aep/story.md']],
  ]);
  expect(j.groups[0].actions).toEqual(['change.fix']);
  expect(Object.fromEntries(j.files.map((f) => [f.path, f.mark]))).toMatchObject({'work/impl/src/m7.rs': 'changed', 'work/impl/tests/repro.rs': 'added', 'work/dissection.md': null});
  expect(j.files.every((f) => f.size > 0 && f.format)).toBe(true);
  expect(j.inputs.files).toBe(201);
  expect(j.inputs.trees).toEqual([{path: 'idea.yaml', kind: 'file', files: 1}, {path: 'impl', kind: 'dir', files: 100}, {path: 'mainline', kind: 'dir', files: 100}]);
  expect(r.text).not.toContain('work/mainline/');
  expect(j.hiddenFiles).toBeUndefined();
});

test('the product opened first is the last file the last finished step wrote, and the file route serves it and a seeded file alike', async () => {
  const j = (await products('rec')).json;
  expect(j.product).toMatchObject({via: 'last-written', path: 'work/repro-result.txt', missing: false});
  expect(j.product.note).toContain('step 3');
  const f = await get('rec', `/api/file?source=0&path=${encodeURIComponent(j.product.path)}`);
  expect(f.status).toBe(200);
  expect(f.json.text).toContain('result: red');
  // an input is not a product, and it can still be read: what the page shows as an input never limits what the file route reads
  const seeded = await get('rec', `/api/file?source=0&path=${encodeURIComponent('work/mainline/src/m1.rs')}`);
  expect(seeded.status).toBe(200);
});

test('a record that names a path outside work/ or the run, a link that leaves the run or a dot-directory lists nothing and prints no path', async () => {
  const r = await products('hostile');
  const j = r.json;
  expect(j.basis).toBe('records');
  expect(j.total).toBe(5 + 2);
  expect(j.files.map((f) => f.path)).not.toContain('work/leak.txt');
  expect(j.files.map((f) => f.path).filter((p) => p.includes('link-dir') || p.includes('hostname') || p.includes('token'))).toEqual([]);
  expect(j.refusedRecords).toBe(2);
  expect(j.leftOut).toBe(2);
  for (const bad of [SECRET, 'outside-secret', 'hostname', 'token.txt', 'inside.txt']) expect(r.text, bad).not.toContain(bad);
  // and the routes refuse what the records named
  for (const p of ['work/leak.txt', 'work/link-dir/inside.txt', '../../outside-secret.txt', 'work/.cache/token.txt']) {
    const f = await get('hostile', `/api/file?source=0&path=${encodeURIComponent(p)}`);
    expect(f.status, p).toBeGreaterThanOrEqual(400);
    expect(f.text, p).not.toContain(SECRET);
  }
});

test('the products are not cut by depth; the bound on lines stays and the answer says how many are not shown', async () => {
  const j = (await products('bulk')).json;
  expect(j.total).toBe(525);
  expect(j.shown).toBe(500);
  expect(j.notShown).toBe(25);
  expect(j.truncated).toBe(true);
  expect(j.cut).toBe('lines');
  expect(j.files.length).toBe(500);
});

test('a run without the records of a seed gives today\'s listing and says so; the newest Markdown file is never inside a tree with a manifest', async () => {
  const j = (await products('old')).json;
  expect(j.basis).toBe('listing');
  expect(j.basisNote).toContain('seeded.json');
  expect(j.files.length).toBeGreaterThan(200);
  expect(j.groups).toBeUndefined();
  expect(j.product).toMatchObject({via: 'newest-markdown', path: 'work/dissection.md'});
});

test('a run without a watcher and a run with an incomplete seed record give the listing and say why', async () => {
  const a = (await products('nowatch')).json;
  expect(a.basis).toBe('listing');
  expect(a.basisNote).toContain('--watch');
  const b = (await products('incomplete')).json;
  expect(b.basis).toBe('listing');
  expect(b.basisNote).toContain('not complete');
});

test('no route acts on a run: the products route takes no write', async () => {
  const r = await fetch(servers.rec.url + '/api/products?source=0', {method: 'POST', body: '{}'});
  expect(r.status).toBe(405);
});
