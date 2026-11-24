// What the inspector may serve of a run, and what it may spend on it (story inspector-shows-step-products, fix
// round 1 of the security review). Own servers on ports the kernel gives; runs written by hand with invented text
// (test/products-fixture.mjs).
//   the file route and the listing serve what lies under a work/ directory of the run, nothing else of it
//   a regular file with more than one link is not served, and not listed
//   the listing does not descend a link to a directory, caps the directories and entries it visits as well as the
//   files, and says what cut it
//   the run log of a case is read through the type check, in a window from its end
//   an output blob is a file of store/blobs/ of the run, whatever a link in it says
//   the Explain panel shows a hostile evidence record as text; a record nested 10000 deep is cut at a depth
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT, OUTPUT_DIGEST, SECRET_TEXT} from './products-fixture.mjs';
import {ask, put, sourceIndex, fileUrl} from './security-products-lib.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'step-products-bounds');
const runs = path.join(root, 'runs');
const hex = (s) => Buffer.from(s).toString('hex');
const MIB = 1024 * 1024;
const E = 'work/example';
let srv, base, srv2, base2;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => { srv?.kill(); srv2?.kill(); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});

  const main = path.join(runs, 'main');
  writeProductsRun(main);
  const outside = path.join(root, 'outside-main', 'secret.txt');
  fs.linkSync(outside, path.join(main, 'work', 'example', 'hard.txt'));
  fs.symlinkSync(path.join(main, 'settings.json'), path.join(main, 'work', 'example', 'settings-link.json'));
  fs.symlinkSync(path.join(main, 'work', 'example', 'sub'), path.join(main, 'work', 'example', 'dirlink'));
  put(path.join(main, 'store', ROOT, 'work', 'case-note.txt'), 'result: kept\n');

  const dirs = path.join(runs, 'dirs');
  writeProductsRun(dirs);
  for (let i = 0; i < 2300; i++) fs.mkdirSync(path.join(dirs, 'work', 'many', `d${String(i).padStart(4, '0')}`), {recursive: true});

  const win = path.join(runs, 'win');
  writeProductsRun(win);
  const log = path.join(win, 'store', `runlog-case-${hex(ROOT)}.jsonl`);
  const pad = `${JSON.stringify({seq: 99, kind: 'Note', text: 'x'.repeat(1000)})}\n`;
  fs.appendFileSync(log, pad.repeat(Math.ceil((9 * MIB) / pad.length)));

  const blob = path.join(runs, 'blob');
  writeProductsRun(blob);
  const target = path.join(blob, 'store', 'blobs', OUTPUT_DIGEST['notes.check']);
  fs.rmSync(target);
  fs.symlinkSync(path.join(blob, 'work', 'example', 'notes.txt'), target);

  const deep = path.join(runs, 'deep');
  writeProductsRun(deep);
  const nest = 10000;
  put(path.join(deep, 'store', ROOT, 'evidence', 'e0011-deep.json'),
    `{"format":"canon-evidence/1","id":"e0011-deep","kind":"notes_shape","details":${'['.repeat(nest)}"bottom"${']'.repeat(nest)}}\n`);

  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
});

const outputOf = async (name, action) => {
  const src = await sourceIndex(base, name);
  const c = (await ask(base, `/api/tree?source=${src}`)).json.cases.find((x) => x.id === ROOT);
  const e = c.events.find((x) => x.event === 'action_finished' && x.action === action);
  return ask(base, `/api/output?source=${src}&case=${encodeURIComponent(c.rel)}&seq=${e.seq}`, {timeout: 20000});
};

test('the file route serves what lies under a work/ directory of the run and nothing else of it', async () => {
  const src = await sourceIndex(base, 'main');
  const log = `store/runlog-case-${hex(ROOT)}.jsonl`;
  for (const rel of ['settings.json', 'outcome.json', 'status.json', 'llm-calls.jsonl', 'run-settings.jsonl', 'decisions.jsonl', log, `store/${ROOT}/events.jsonl`, `store/blobs/${OUTPUT_DIGEST['notes.check']}`, `${E}/settings-link.json`]) {
    const r = await ask(base, fileUrl(src, rel));
    expect(r.status, rel).toBe(404);
    expect(r.json.error, rel).toMatch(/work\//);
  }
  for (const rel of [`${E}/notes.txt`, 'work/idea.yaml', `store/${ROOT}/work/case-note.txt`, `${E}/dirlink/inner.txt`]) expect((await ask(base, fileUrl(src, rel))).status, rel).toBe(200);
  const list = (await ask(base, `/api/products?source=${src}`)).json;
  const paths = list.files.map((f) => f.path);
  expect(paths).toContain(`store/${ROOT}/work/case-note.txt`);
  for (const p of paths) expect(p, p).toMatch(/(^|\/)work\//);
  expect(paths.join('\n')).not.toContain('settings-link');
});

test('a regular file with more than one link is neither served nor listed', async () => {
  const src = await sourceIndex(base, 'main');
  const r = await ask(base, fileUrl(src, `${E}/hard.txt`));
  expect(r.status).toBe(403);
  expect(r.json.error).toMatch(/more than one link/);
  expect(r.text).not.toContain(SECRET_TEXT);
  const list = (await ask(base, `/api/products?source=${src}`)).json;
  expect(list.files.map((f) => f.path).join('\n')).not.toContain('hard.txt');
});

test('files the listing leaves out because of their links are counted, in the answer and in the page', async ({page}) => {
  const src = await sourceIndex(base, 'main');
  const list = (await ask(base, `/api/products?source=${src}`)).json;
  expect(list.leftOut).toBeGreaterThanOrEqual(2); // hard.txt and settings-link.json
  await page.goto(`${base}/?run=main`);
  await page.locator('#tabs button[data-tab=products]').click({timeout: 20000});
  await expect(page.locator('#panel .prod-note', {hasText: 'more than one link'})).toBeVisible({timeout: 20000});
});

test('the listing does not descend a link to a directory', async () => {
  const src = await sourceIndex(base, 'main');
  const list = (await ask(base, `/api/products?source=${src}`)).json;
  const paths = list.files.map((f) => f.path);
  expect(paths).toContain(`${E}/sub/inner.txt`);
  expect(paths.join('\n')).not.toContain('dirlink');
});

test('a listing visits at most so many directories, and says that it was cut and by what', async () => {
  const src = await sourceIndex(base, 'dirs');
  const r = await ask(base, `/api/products?source=${src}`, {timeout: 20000});
  expect(r.hung).toBe(false);
  expect(r.json.truncated).toBe(true);
  expect(r.json.cut).toBe('directories');
  expect(r.ms, `one listing took ${r.ms} ms`).toBeLessThan(5000);
  const main = (await ask(base, `/api/products?source=${await sourceIndex(base, 'main')}`)).json;
  expect(main.truncated).toBe(false);
  expect(main.cut).toBeNull();
});

test('the run log is read in a window from its end: an action before the window is said to be outside it', async () => {
  const o = await outputOf('win', 'notes.check');
  expect(o.status).toBe(200);
  expect(o.json.recorded).toBe(false);
  expect(o.json.why).toMatch(/not found within the last 8 MiB of the run log/);
  const own = await outputOf('main', 'notes.check');
  expect(own.json.recorded).toBe(true);
  expect(own.json.text).toContain('notes');
});

test('an output blob that is a link to another file of the run is not read: it must be a file of store/blobs/', async () => {
  const o = await outputOf('blob', 'notes.check');
  expect(o.status).toBe(200);
  expect(o.json.recorded).toBe(false);
  expect(o.json.why).toMatch(/not a regular file inside the run's store\/blobs\//);
  expect(o.text).not.toContain('invented note');
  const own = await outputOf('main', 'notes.check');
  expect(own.json.recorded).toBe(true);
});

test('a record nested 10000 deep is cut at a depth, in the answer of the server and in the page, which stays usable', async ({page}) => {
  test.setTimeout(60000);
  const src = await sourceIndex(base, 'deep');
  const tree = await ask(base, `/api/tree?source=${src}`, {timeout: 20000});
  expect(tree.status).toBe(200);
  expect(tree.text.length).toBeLessThan(2 * MIB);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(`${base}/?run=deep`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible({timeout: 20000});
  await page.locator('#tabs button[data-tab=products]').click();
  await page.locator('#panel .prod-ev[data-id="e0011-deep"]').click();
  const d = page.locator('#panel .prod-detail[data-kind=evidence]');
  await expect(d).toContainText(/cut at depth \d+/);
  expect(await d.locator('.pf-grp').count()).toBeLessThan(200);
  await d.locator('.prod-back').click();
  await expect(page.locator('#panel .prod-ev[data-id="e0003-notes.check"]')).toBeVisible();
  expect(errors).toEqual([]);
});

test('the Explain panel shows an evidence record whose id and fields are markup as text', async ({page}) => {
  test.setTimeout(60000);
  needRun(fixtureSource('person'), 'step-products-bounds.spec.mjs (fixture source)', test);
  const run = path.join(root, 'runs2', 'sdc');
  const r = spawnSync('node', ['test/fixture-run.mjs', 'person', run], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`fixture person: ${r.stderr}`);
  const bad = '"><img src=x onerror="window.__pwned=1"><b id="pwned-b">x</b>';
  const f = path.join(run, 'store', 'IDEA-1', 'evidence', 'e0004-intake.open.json');
  const rec = JSON.parse(fs.readFileSync(f, 'utf8'));
  Object.assign(rec, {id: `e0004${bad}`, producer: bad, subject: bad, subject_revision: bad, result: bad, observed_at: bad});
  fs.writeFileSync(f, JSON.stringify(rec));
  ({url: base2, proc: srv2} = await startServer(['--view', path.resolve(ui, '../views/sdc.view.json'), '--runs', path.join(root, 'runs2')]));
  const dialogs = [];
  page.on('dialog', (dlg) => { dialogs.push(dlg.message()); dlg.dismiss(); });
  await page.goto(`${base2}/?run=sdc&case=IDEA-1&node=evidence:intake_opened`);
  const panel = page.locator('#panel');
  await expect(panel).toContainText('Records', {timeout: 20000});
  await expect(panel).toContainText('onerror="window.__pwned=1"');
  const found = await page.evaluate(() => ({
    img: document.querySelectorAll('#panel img, #panel [onerror]').length,
    b: document.querySelectorAll('#pwned-b').length,
    flag: window.__pwned ?? null,
    handlers: Array.from(document.querySelectorAll('#panel *')).flatMap((e) => Array.from(e.attributes).filter((a) => /^on/i.test(a.name)).map((a) => a.name)),
  }));
  expect(found).toEqual({img: 0, b: 0, flag: null, handlers: []});
  expect(dialogs).toEqual([]);
});
