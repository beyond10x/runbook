// Pass 3 of the independent review of story inspector-shows-step-products: what fix round 2 added around the roots of
// the file route and the listing, and the blob route. Own server, runs written by hand (products-fixture.mjs), invented
// text.
//   (a) work/ is a link to the work/ of another run, or to a directory of the run that is called work: no root
//   (b) a case's work/ that is a link to the run's work/ is no second door to what lies outside work/
//   (c) the listing says that files were left out by a number and names nothing of a dot-directory
//   (d) the blob route: a name that starts with a dot, a second name, a link in blobs/ to a file elsewhere in the run,
//       a blobs/ that is a link, a blob larger than the bound
import {test, expect} from '@playwright/test';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT, OUTPUT, OUTPUT_DIGEST, SECRET_TEXT, TOKEN_TEXT} from './products-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex, fileUrl, VmHWM} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-3-roots');
const runs = path.join(root, 'runs');
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
const H = {dot: sha('3-dot'), inRun: sha('3-in-run'), hard: sha('3-hard'), big: sha('3-big'), linked: sha('3-linked')};
const MIB = 1048576;
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});

  const victim = path.join(runs, 'victim');
  writeProductsRun(victim);

  const other = path.join(runs, 'otherwork');
  writeProductsRun(other);
  fs.rmSync(path.join(other, 'work'), {recursive: true});
  fs.symlinkSync(path.join(victim, 'work'), path.join(other, 'work'));

  const named = path.join(runs, 'namedwork');
  writeProductsRun(named);
  fs.mkdirSync(path.join(named, 'store', 'work'), {recursive: true});
  put(path.join(named, 'store', 'work', 'inside.txt'), 'result: inside store/work\n');
  fs.rmSync(path.join(named, 'work'), {recursive: true});
  fs.symlinkSync(path.join('store', 'work'), path.join(named, 'work'));

  const casework = path.join(runs, 'casework');
  writeProductsRun(casework);
  fs.symlinkSync(path.join(casework, 'work'), path.join(casework, 'store', ROOT, 'work'));

  const b = path.join(runs, 'blobs');
  writeProductsRun(b);
  const outside = path.join(root, 'outside', 'secret.txt');
  put(outside, `${SECRET_TEXT}\n`);
  put(path.join(b, 'store', '.cache', 'blobs', H.dot), `${TOKEN_TEXT}\n`);
  put(path.join(b, 'store', 'elsewhere', H.inRun), 'a file elsewhere in the run\n');
  fs.symlinkSync(path.join(b, 'store', 'elsewhere', H.inRun), path.join(b, 'store', 'blobs', H.linked));
  fs.linkSync(outside, path.join(b, 'store', 'blobs', H.hard));
  const big = path.join(b, 'store', 'blobs', H.big);
  fs.writeFileSync(big, '');
  fs.truncateSync(big, 300 * MIB);
  fs.mkdirSync(path.join(b, 'store', ROOT, 'work'), {recursive: true});

  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
});

const listing = async (name) => (await ask(base, `/api/products?source=${await sourceIndex(base, name)}`)).json;

test('(a) a work/ that is a link to the work/ of another run serves and lists nothing of it', async () => {
  const src = await sourceIndex(base, 'otherwork');
  const r = await ask(base, fileUrl(src, 'work/example/notes.txt'));
  expect(r.status).not.toBe(200);
  expect((await listing('otherwork')).files).toEqual([]);
});

test('(a) a work/ that is a link to store/work (a directory of the run that is called work) is no root', async () => {
  const src = await sourceIndex(base, 'namedwork');
  const r = await ask(base, fileUrl(src, 'work/inside.txt'));
  expect(r.status).not.toBe(200);
  expect((await listing('namedwork')).files).toEqual([]);
});

test('(b) a case work/ that is a link to the run work/ gives no path to files outside work/, and the run work/ is still served', async () => {
  const src = await sourceIndex(base, 'casework');
  const ok = await ask(base, fileUrl(src, 'work/example/notes.txt'));
  expect(ok.status).toBe(200);
  for (const rel of [`store/${ROOT}/events.jsonl`, 'store/blobs', `store/${ROOT}/work/../events.jsonl`]) {
    const r = await ask(base, fileUrl(src, rel));
    expect(r.status === 200 && r.text.includes('action_finished') ? `${rel} served` : 'ok').toBe('ok');
  }
});

test('(c) the listing counts what it leaves out and names nothing of a dot-directory', async () => {
  const l = await listing('victim');
  const text = JSON.stringify(l);
  expect(text).not.toMatch(/\.cache|\.hidden|invented-token/);
  expect(typeof l.leftOut).toBe('number');
  const src = await sourceIndex(base, 'victim');
  const f = await ask(base, fileUrl(src, 'work/example/viatoken.txt'));
  expect(f.text).not.toMatch(/\.cache|\.hidden|invented-token/);
  expect(f.status).not.toBe(200);
});

const blob = async (rel, hash, timeout = 10000) => ask(base, `/api/blob?source=${await sourceIndex(base, 'blobs')}&case=${encodeURIComponent(rel)}&hash=${hash}`, {timeout});

test('(d) control: a blob of store/blobs/ is served', async () => {
  expect((await blob('.', OUTPUT_DIGEST['notes.check'])).text).toContain(OUTPUT['notes.check'].slice(0, 20));
});

test('(d) a blob under a dot-directory, in spellings of the case that reach it, is not served', async () => {
  for (const rel of ['.cache', './.cache', '.cache/', 'x/../.cache'.replace('x/../', ''), '%2ecache']) {
    const r = await blob(rel, H.dot);
    expect(`${rel}: ${r.text.includes(TOKEN_TEXT) ? 'served' : 'not served'}`).toBe(`${rel}: not served`);
  }
});

test('(d) a second name of a file outside the run is not served', async () => {
  expect((await blob('.', H.hard)).text).not.toContain(SECRET_TEXT);
});

test('(d) a link in blobs/ to a file elsewhere in the run is not served', async () => {
  const r = await blob('.', H.linked);
  expect(r.text).not.toContain('a file elsewhere in the run');
});

test('(d) a blob of 300 MB is read at most to the bound the route shows: peak memory grows by less than 50 MB, at most 200000 bytes answered', async () => {
  test.setTimeout(60000);
  const pid = await srv.started;
  const before = VmHWM(pid);
  const r = await blob('.', H.big, 40000);
  const grew = Math.round((VmHWM(pid) - before) / MIB);
  console.log(`blob of 300 MB: status ${r.status}, ${r.ms} ms, ${r.text.length} characters, peak memory grew by ${grew} MiB`);
  expect(r.text.length).toBeLessThanOrEqual(200000);
  expect(grew).toBeLessThan(50);
});
