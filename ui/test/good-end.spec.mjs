// What the run picker says of a closed run of any runbook (docs: ui/README.md "The view file", good_outcomes).
// A good end is declared, never guessed from the SDC outcomes: by the view file (`good_outcomes`), or by the
// runner's own `delivered: true` in status.json; a closed run with neither is neutral ("ended: <outcome>"), not
// red. Three states: good (a check), bad (a cross), neutral (a diamond). The SDC runbook's pages do not change.
// Own servers on ports the kernel gives (test/serve.mjs); the runs are written by hand with invented text
// (test/loom-run-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {checkView} from '../web/view.js';
import {writeLoomRun, EXAMPLE_VIEW, withoutGoodEnd, iso} from './loom-run-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'good-end');
const procs = [];
let withKey, withoutKey, noView, sdc;

// a closed run of the SDC runbook: only the root's events and status.json, which is all the picker reads
function sdcRun(dir, outcome, {status = true} = {}) {
  const store = path.join(dir, 'store', 'IDEA-1');
  fs.mkdirSync(store, {recursive: true});
  fs.writeFileSync(path.join(dir, 'settings.json'), '{}\n');
  fs.writeFileSync(path.join(store, 'events.jsonl'), [{event: 'opened', at: iso(0), protocol: {id: 'sdc', revision: 8}, seq: 0}, {event: 'terminated', at: iso(30), outcome, seq: 1}].map((e) => JSON.stringify(e)).join('\n') + '\n');
  if (status) fs.writeFileSync(path.join(dir, 'status.json'), JSON.stringify({format: 'sdc.run-status/1', at: iso(30), state: 'closed', outcome, delivered: outcome === 'operating', note: null, closed_by: null}) + '\n');
}

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => { for (const p of procs) p.kill(); });

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  writeLoomRun(path.join(runs, 'ready'), {outcome: 'ready'});
  writeLoomRun(path.join(runs, 'escalated'), {outcome: 'escalated'});
  const viewFile = path.join(root, 'example.view.json');
  fs.mkdirSync(root, {recursive: true});
  fs.writeFileSync(viewFile, JSON.stringify(EXAMPLE_VIEW, null, 2));
  const plain = path.join(root, 'plain.view.json');
  fs.writeFileSync(plain, JSON.stringify(withoutGoodEnd(EXAMPLE_VIEW), null, 2));
  const sdcRuns = path.join(root, 'sdc-runs');
  sdcRun(path.join(sdcRuns, 'sdc-operating'), 'operating');
  sdcRun(path.join(sdcRuns, 'sdc-not-delivered'), 'not_delivered');
  sdcRun(path.join(sdcRuns, 'sdc-term-only'), 'operating', {status: false});
  for (const [args, set] of [[['--view', viewFile, '--runs', runs], (s) => { withKey = s; }], [['--view', plain, '--runs', runs], (s) => { withoutKey = s; }],
    [['--no-view', '--runs', runs], (s) => { noView = s; }], [['--runs', sdcRuns], (s) => { sdc = s; }]]) {
    const s = await startServer(args);
    procs.push(s.proc);
    set({url: s.url, sources: await (await fetch(s.url + '/api/sources')).json()});
  }
});

const runOf = (srv, name) => srv.sources.find((s) => s.name === name).run;
const rowMark = async (page, srv, name) => {
  const s = srv.sources.find((x) => x.name === name);
  await page.goto(`${srv.url}/?run=${name}`);
  await page.locator('#runpick').click();
  const row = page.locator(`#pk-list [data-pick="${s.index}"]`);
  await expect(row).toBeVisible();
  return {cls: await row.locator('.pk-mk').getAttribute('class'), text: await row.locator('.pk-mt').textContent(), button: await page.locator('#runpick .pk-mt').textContent(), buttonCls: await page.locator('#runpick .pk-mk').getAttribute('class')};
};

test('a view that lists the outcome as a good end: the run is good, the check and the outcome in words', async ({page}) => {
  expect(runOf(withKey, 'ready')).toMatchObject({state: 'closed', outcome: 'ready', delivered: false, end: 'good'});
  const m = await rowMark(page, withKey, 'ready');
  expect(m.cls).toContain('m-ok');
  expect(m.text).toBe('closed · ready');
  expect(m.button).toBe('closed');
  expect(m.text).not.toMatch(/not delivered/);
});

test('a view that lists good ends and does not list this outcome: the run is bad', async ({page}) => {
  expect(runOf(withKey, 'escalated')).toMatchObject({state: 'closed', outcome: 'escalated', end: 'bad'});
  const m = await rowMark(page, withKey, 'escalated');
  expect(m.cls).toContain('m-bad');
  expect(m.text).toContain('escalated');
});

test('a view without the key, or no view: the closed run is neutral, "ended: <outcome>", no cross, no red', async ({page}) => {
  for (const srv of [withoutKey, noView]) {
    expect(runOf(srv, 'ready')).toMatchObject({state: 'closed', outcome: 'ready', end: 'neutral'});
    const m = await rowMark(page, srv, 'ready');
    expect(m.cls).toContain('m-end');
    expect(m.cls).not.toMatch(/m-bad|m-ok/);
    expect(m.text).toBe('ended: ready');
    expect(m.button).toBe('ended');
    expect(m.buttonCls).toContain('m-end');
  }
});

test('the SDC runs are unchanged: operating is good, not delivered is bad, with and without status.json', async ({page}) => {
  expect(runOf(sdc, 'sdc-operating')).toMatchObject({end: 'good', delivered: true});
  expect(runOf(sdc, 'sdc-term-only')).toMatchObject({end: 'good', outcome: 'operating'});
  expect(runOf(sdc, 'sdc-not-delivered')).toMatchObject({end: 'bad', delivered: false});
  expect((await rowMark(page, sdc, 'sdc-operating')).text).toBe('closed · operating');
  expect((await rowMark(page, sdc, 'sdc-term-only')).cls).toContain('m-ok');
  const bad = await rowMark(page, sdc, 'sdc-not-delivered');
  expect(bad.cls).toContain('m-bad');
  expect(bad.text).toBe('closed · not delivered');
});

test('the view check knows good_outcomes: a list of text passes, another type is refused by name; the SDC view passes', () => {
  expect(checkView(EXAMPLE_VIEW)).toEqual([]);
  expect(checkView({...EXAMPLE_VIEW, good_outcomes: 'ready'}).join(' ')).toContain('good_outcomes: expected an array, found string');
  expect(checkView({...EXAMPLE_VIEW, good_outcomes: [1]}).join(' ')).toContain('good_outcomes[0]: expected string, found number');
  expect(checkView(JSON.parse(fs.readFileSync(path.join(here, '../../views/sdc.view.json'), 'utf8')))).toEqual([]);
});
