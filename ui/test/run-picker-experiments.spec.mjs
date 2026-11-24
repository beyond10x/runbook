// The run picker groups recent runs by experiment (decision D-009). A run belongs to an experiment by its
// experiment.json (sdc.experiment-run/1: experiment, variant, ordinal, tree_commit, …) or by its name in an
// experiment record's variants[].runs (experiments/<id>.yaml, sdc.experiment/1). Eight runs played by
// test/fixture-run.mjs (person) into SDC_UI_FIXTURE_DIR (default ui/target/fixture-runs)/experiments/runs, two
// records beside them, served by a server of its own (a port the kernel gives):
//   alpha    cmp-effort low #1 and other-rec v #1 (two records), closed delivered
//   bravo    cmp-effort low #2, closed delivered with a forced decision (not a clean pass)
//   charlie  cmp-effort high #1, stopped
//   delta    cmp-effort high #2 by its experiment.json, closed not delivered
//   echo     cmp-effort low #3, written last
//   foxtrot  solo-x base #1 by its experiment.json, no record
//   golf     no experiment: Unassigned
//   hotel    a stub run
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {playFixtures} from './fixtures.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';
import {axeViolations} from '../ux/core/contract.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const top = path.join(FIXTURE_DIR, 'experiments');
const runs = path.join(top, 'runs'), records = path.join(top, 'records');
let base;
const T0 = Date.now() - 60 * 60000;
const iso = (ms) => new Date(ms).toISOString().replace(/\.\d+Z$/, 'Z');
const settings = JSON.stringify({format: 'sdc.run-settings/1', at: iso(T0), leg: 'started', autonomy: 'A5', adapter: 'metaharness',
  decider: {kind: 'haiku', model: 'claude-haiku-5-5'}, models: {writer: {model: 'claude-sonnet-5-5'}, implement: {model: 'claude-sonnet-5-5'}}}) + '\n';
const closed = (extra) => JSON.stringify({format: 'sdc.run-status/1', at: iso(T0 + 20 * 60000), state: 'closed', closed_by: 'rule:autopilot', ...extra});
const expRun = (experiment, variant, ordinal) => JSON.stringify({format: 'sdc.experiment-run/1', experiment, variant, ordinal, run: `${experiment}.${variant}.${ordinal}`,
  tree_commit: 'a'.repeat(40), input_sha256: 'b'.repeat(64), settings_sha256: 'c'.repeat(64), started_at: iso(T0), retrospective: false});
const RUNS = {
  alpha: {files: {'status.json': closed({outcome: 'operating', delivered: true})}, ago: 20},
  bravo: {files: {'status.json': closed({outcome: 'operating', delivered: true, forced: 1})}, ago: 15},
  charlie: {files: {'run-stopped.json': JSON.stringify({format: 'sdc.run-stopped/1', at: iso(T0 + 9 * 60000), by: 'rule:autopilot', reason: 'spent', cause: 'writer_spent'})}, ago: 4},
  delta: {files: {'status.json': closed({outcome: 'not_delivered', delivered: false}), 'experiment.json': expRun('cmp-effort', 'high', 2)}, ago: 2},
  echo: {files: {}, ago: 0},
  foxtrot: {files: {'status.json': closed({outcome: 'operating', delivered: true}), 'experiment.json': expRun('solo-x', 'base', 1)}, ago: 30},
  golf: {files: {}, ago: 1},
  hotel: {files: {}, ago: 3, stub: true},
};
// the records as sdc-experiment writes them: quoted scalars, settings maps, and a result whose variants
// list repeats the runs (not the record's own variants)
const RECORDS = {
  'cmp-effort.yaml': `format: sdc.experiment/1
id: cmp-effort
kind: prospective
title: 'Effort low against high, everything else fixed'
hypothesis: "a hypothesis: with a colon"
variable: efforts.implement.effort
fixed:
  autonomy: A5
variants:
- id: low
  settings:
    efforts.implement.effort: low
  runs:
  - alpha
  - bravo
  - echo
- id: high
  settings:
    efforts.implement.effort: high
  runs:
  - charlie
baseline: low
result:
  variants:
  - id: low
    runs:
    - not-a-run
`,
  'other-rec.yaml': `format: sdc.experiment/1
id: other-rec
title: Another record that holds alpha
variants:
- id: v
  runs: [alpha]
`,
};
const procs = [];
let sources;
const src = (name) => sources.find((s) => s.name === name).index;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server: one project is enough'));
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  needRun(fixtureSource('person'), 'run-picker-experiments.spec.mjs (fixture source)', test);
  fs.rmSync(top, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  fs.mkdirSync(records, {recursive: true});
  for (const [f, text] of Object.entries(RECORDS)) fs.writeFileSync(path.join(records, f), text);
  await playFixtures(Object.keys(RUNS).map((name) => ['person', path.join(runs, name)]));
  for (const [name, r] of Object.entries(RUNS)) {
    const dir = path.join(runs, name);
    for (const f of ['run-settings.jsonl', 'llm-calls.jsonl', 'decisions.jsonl', 'status.json', 'run-stopped.json']) fs.rmSync(path.join(dir, f), {force: true});
    fs.writeFileSync(path.join(dir, 'run-settings.jsonl'), r.stub ? settings.replace('metaharness', '/x/llm-adapter-fixture.sh') : settings);
    for (const [f, text] of Object.entries(r.files)) fs.writeFileSync(path.join(dir, f), text);
    const t = new Date(Date.now() - r.ago * 60000);
    for (const q of [dir, path.join(dir, 'store'), path.join(dir, 'run.log'), path.join(dir, 'run-settings.jsonl'), ...Object.keys(r.files).map((f) => path.join(dir, f))]) if (fs.existsSync(q)) fs.utimesSync(q, t, t);
  }
  { const s = await startServer(['--runs', runs, '--experiments', records]); base = s.url; procs.push(s.proc); }
  sources = await (await fetch(base + '/api/sources')).json();
});

const open = async (page) => { await page.locator('#runpick').click(); await expect(page.locator('#pk-list [role=option]').first()).toBeVisible(); };
const listed = (page) => page.locator('#pk-list').evaluate((el) => [...el.querySelectorAll('[role=group]')].map((g) => [g.getAttribute('aria-label'),
  [...g.querySelectorAll('.pk-row')].map((o) => `${o.querySelector('.pk-rn').textContent}${o.querySelector('.pk-var') ? ` ${o.querySelector('.pk-var').textContent}` : ''}`)]));

test('/api/sources: each run names its experiments, from experiment.json or from the records', () => {
  const ex = Object.fromEntries(sources.map((s) => [s.name, s.experiments.map((e) => `${e.id}/${e.variant}#${e.ordinal} ${e.from}`)]));
  expect(ex).toEqual({alpha: ['cmp-effort/low#1 record', 'other-rec/v#1 record'], bravo: ['cmp-effort/low#2 record'], charlie: ['cmp-effort/high#1 record'],
    delta: ['cmp-effort/high#2 experiment.json'], echo: ['cmp-effort/low#3 record'], foxtrot: ['solo-x/base#1 experiment.json'], golf: [], hotel: []});
  const d = sources.find((s) => s.name === 'delta').experiments[0];
  expect(d).toMatchObject({title: 'Effort low against high, everything else fixed', vi: 1, commit: 'a'.repeat(40), retrospective: false});
  expect(sources.find((s) => s.name === 'foxtrot').experiments[0]).toMatchObject({title: null, vi: null});
});

test('Recent groups by experiment, rows by variant then ordinal; the most recent experiment open, the others closed; Unassigned last', async ({page}) => {
  await page.goto(`${base}/?run=golf`);
  await open(page);
  expect(await listed(page)).toEqual([
    ['Experiment cmp-effort', ['alpha low #1', 'bravo low #2', 'echo low #3', 'charlie high #1', 'delta high #2']],
    ['Experiment other-rec', []],
    ['Experiment solo-x', []],
    ['Unassigned', ['golf']],
  ]);
  await expect(page.locator('#pk-list .pk-sh')).toHaveText('Recent');
  // each head: the id, the purpose, the variants with their runs, the clean passes of the ended runs
  const head = (n) => page.locator(`#pk-e-${n}`);
  await expect(head(0).locator('.pk-en')).toHaveText('cmp-effort');
  await expect(head(0).locator('.pk-r2')).toHaveText('Effort low against high, everything else fixed · low ×3 · high ×2');
  await expect(head(0).locator('.pk-ec')).toHaveText('1 clean pass of 4 ended');
  await expect(head(0)).toHaveClass(/\bopen\b/);
  await expect(head(1).locator('.pk-r2')).toHaveText('Another record that holds alpha · v ×1');
  await expect(head(2).locator('.pk-r2')).toHaveText('base ×1');
  await expect(head(2)).not.toHaveClass(/\bopen\b/);
  // a click on a head opens or closes its group
  await head(2).click();
  await expect.poll(() => listed(page)).toContainEqual(['Experiment solo-x', ['foxtrot base #1']]);
  await head(0).click();
  await expect.poll(() => listed(page)).toContainEqual(['Experiment cmp-effort', []]);
  // the stub run stays in its own group
  await page.locator('#pk-stubs').click();
  await expect(page.locator('#pk-list .g-stub .pk-rn')).toHaveText(['hotel']);
});

test('the group of the run on screen is open; a run in two records is a row in each', async ({page}) => {
  await page.goto(`${base}/?run=alpha`);
  await open(page);
  await expect(page.locator('#pk-e-0')).toHaveClass(/\bopen\b/);
  await expect(page.locator('#pk-e-1')).toHaveClass(/\bopen\b/);
  const both = page.locator(`[data-pick="${src('alpha')}"]`);
  await expect(both).toHaveCount(2);
  expect(await both.evaluateAll((els) => els.map((e) => e.id))).toEqual([`pk-o-${src('alpha')}`, `pk-o-${src('alpha')}-2`]);
  await expect(both.first()).toHaveAttribute('aria-selected', 'true');
});

test('search matches the experiment id and the variant, and opens the groups it matches in', async ({page}) => {
  await page.goto(`${base}/?run=golf`);
  await open(page);
  const q = page.locator('#pk-q');
  await q.fill('high');
  await expect.poll(() => listed(page)).toEqual([['Experiment cmp-effort', ['charlie high #1', 'delta high #2']]]);
  await q.fill('solo-x');
  await expect.poll(() => listed(page)).toEqual([['Experiment solo-x', ['foxtrot base #1']]]);
  await q.fill('other-rec');
  await expect.poll(() => listed(page)).toEqual([['Experiment cmp-effort', ['alpha low #1']], ['Experiment other-rec', ['alpha v #1']]]);
});

test('the keyboard: arrows reach an experiment head, Enter opens its group, Enter on a row opens the run', async ({page}) => {
  await page.goto(`${base}/?run=golf`);
  await page.locator('#runpick').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#pk-q')).toBeFocused();
  const ad = () => page.locator('#pk-q').getAttribute('aria-activedescendant');
  await page.keyboard.press('Control+Home');
  await expect.poll(ad).toBe('pk-e-0');
  for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowDown');
  await expect.poll(ad).toBe('pk-e-1');
  await page.keyboard.press('Enter');
  await expect(page.locator('#pk-e-1')).toHaveClass(/\bopen\b/);
  await expect(page.locator('#pk-panel')).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await expect.poll(ad).toBe(`pk-o-${src('alpha')}-2`);
  await page.keyboard.press('Enter');
  await expect(page.locator('#pk-panel')).toBeHidden();
  await expect(page.locator('#runpick .pk-nm')).toHaveText('alpha');
  expect(new URL(page.url()).searchParams.get('run')).toBe('alpha');
  await page.locator('#runpick').click();
  expect(await axeViolations(page, {include: ['#picker']})).toEqual([]);
});
