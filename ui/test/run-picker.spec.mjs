// The run picker (decision D-009): a button in the header in place of the store select, a panel with a
// search field and the runs, most recent first, each row saying what the run is. Six runs played by
// test/fixture-run.mjs (person) into SDC_UI_FIXTURE_DIR (default test/fixtures/runs)/picker, then
// given what a real run leaves behind:
//   live-a    running (driver.json with a live pid), decider Opus, work Sonnet
//   done-ok   closed, delivered (status.json, with reopens and forced), cost and calls recorded
//   done-bad  closed, not delivered (status.json)
//   halted    stopped by a rule (run-stopped.json, cause writer_spent)
//   paused    nothing says it ended or runs: not running
//   canned    a stub run (adapter llm-adapter-fixture.sh)
// Served by a server of its own over the runs directory on port 7536 (SDC_UI_PICKER_PORT). Desktop,
// the operator tile (805x1270) and the phone (390x844 as a phone).
// Screenshots: screenshots/{1600x1000,805x1270,390x844}-run-picker.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun, fixtureSource} from './need-run.mjs';
import {phone} from '../ux/adapter.mjs';
import {horizontalOverflow, smallTargets, smallText, sheetProblems} from '../ux/core/phone.mjs';
import {axeViolations} from '../ux/core/contract.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const runs = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'picker');
const port = Number(process.env.SDC_UI_PICKER_PORT || 7536);
const base = `http://127.0.0.1:${port}`;
const T0 = Date.now() - 50 * 60000;
const iso = (ms) => new Date(ms).toISOString().replace(/\.\d+Z$/, 'Z');
const settings = (adapter, decider, work) => JSON.stringify({format: 'sdc.run-settings/1', at: iso(T0), leg: 'started', autonomy: 'A5', adapter,
  decider: {kind: 'haiku', model: decider, escalate: 'final'}, models: {writer: {model: work}, implement: {model: work}, review: {model: work}, other: {model: work}, decider: {model: decider}}}) + '\n';
const status = (extra) => JSON.stringify({format: 'sdc.run-status/1', at: iso(T0 + 30 * 60000), state: 'closed', note: null, closed_by: 'rule:autopilot', ...extra});
// each run: its files, and how many minutes ago it last wrote
const RUNS = {
  'live-a': {files: {'run-settings.jsonl': settings('metaharness', 'claude-opus-5-5', 'claude-sonnet-5-5')}, ago: 0, driver: true},
  'done-ok': {files: {'run-settings.jsonl': settings('metaharness', 'claude-haiku-5-5', 'claude-sonnet-5-5'), 'status.json': status({outcome: 'operating', delivered: true, reopens: 2, forced: 1}),
    'llm-calls.jsonl': [0.5, 0.25].map((c) => JSON.stringify({action: 'spec.author', adapter: 'metaharness', model: 'claude-sonnet-5-5', cost_usd: c, exit: 0})).join('\n') + '\n',
    'decisions.jsonl': JSON.stringify({time: iso(T0 + 60000), case: 'IDEA-1', principal: 'decider:haiku', decision: 'approve', model: 'claude-haiku-5-5', cost_usd: 0.25}) + '\n'}, ago: 10},
  'done-bad': {files: {'run-settings.jsonl': settings('metaharness', 'claude-haiku-5-5', 'claude-haiku-5-5'), 'status.json': status({outcome: 'not_delivered', delivered: false})}, ago: 20},
  'halted': {files: {'run-settings.jsonl': settings('metaharness', 'claude-haiku-5-5', 'claude-haiku-5-5'),
    'run-stopped.json': JSON.stringify({format: 'sdc.run-stopped/1', at: iso(T0 + 12 * 60000), by: 'rule:autopilot', reason: 'the writer spent its calls', cause: 'writer_spent'})}, ago: 30},
  'paused': {files: {'run-settings.jsonl': settings('metaharness', 'claude-haiku-5-5', 'claude-haiku-5-5')}, ago: 40},
  'canned': {files: {'run-settings.jsonl': settings('/x/driver/examples/sdc/tools/llm-adapter-fixture.sh', 'claude-haiku-5-5', 'claude-haiku-5-5')}, ago: 5},
};
const procs = [];
let sources;
const src = (name) => sources.find((s) => s.name === name).index;

test.describe.configure({mode: 'default'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server: one project is enough'));
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(120000);
  needRun(fixtureSource('person'), 'run-picker.spec.mjs (fixture source)', test);
  fs.rmSync(runs, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  for (const [name, r] of Object.entries(RUNS)) {
    const dir = path.join(runs, name);
    const p = spawnSync('node', ['test/fixture-run.mjs', 'person', dir], {cwd: ui, encoding: 'utf8'});
    if (p.status !== 0) throw new Error(`fixture ${name}: ${p.stderr}`);
    for (const f of ['run-settings.jsonl', 'llm-calls.jsonl', 'decisions.jsonl', 'status.json', 'run-stopped.json']) fs.rmSync(path.join(dir, f), {force: true});
    for (const [f, text] of Object.entries(r.files)) fs.writeFileSync(path.join(dir, f), text);
    // a driver holds the tree: this test's own process stands in for it (alive while the test runs)
    const root = fs.readdirSync(path.join(dir, 'store')).map((n) => path.join(dir, 'store', n)).find((p) => fs.existsSync(path.join(p, 'case.json')));
    if (r.driver) fs.writeFileSync(path.join(root, 'driver.json'), JSON.stringify({format: 'sdc.driver/1', pid: process.pid, started_at: iso(T0)}));
    const t = new Date(Date.now() - r.ago * 60000);
    for (const p of [dir, path.join(dir, 'store'), path.join(dir, 'run.log'), ...Object.keys(r.files).map((f) => path.join(dir, f))]) if (fs.existsSync(p)) fs.utimesSync(p, t, t);
  }
  if (await fetch(base + '/api/sources').then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_PICKER_PORT`);
  procs.push(spawn(process.execPath, ['server.mjs', '--port', String(port), '--runs', runs], {cwd: ui, stdio: 'ignore'}));
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((x) => setTimeout(x, 100)); } }
  sources = await (await fetch(base + '/api/sources')).json();
});

const open = async (page) => { await page.locator('#runpick').click(); await expect(page.locator('#pk-list [role=option]').first()).toBeVisible(); };
const rowOf = (page, name) => page.locator(`#pk-o-${src(name)}`);

test('one request carries every row: state, times, cost and calls, models, counts', () => {
  const run = Object.fromEntries(sources.map((s) => [s.name, s.run]));
  expect(run['live-a']).toMatchObject({state: 'running', setting: 'decider Opus · work Sonnet', ended: null});
  expect(run['done-ok']).toMatchObject({state: 'closed', outcome: 'operating', delivered: true, cost: 1, calls: 3, work: 2, decider: 1, reopens: 2, forced: 1, unstuck: null});
  expect(run['done-ok'].ended - run['done-ok'].started).toBe(30 * 60000);
  expect(run['done-bad']).toMatchObject({state: 'closed', outcome: 'not_delivered', delivered: false});
  expect(run['halted']).toMatchObject({state: 'stopped', cause: 'writer_spent', stopBy: 'rule:autopilot'});
  expect(run['paused']).toMatchObject({state: 'idle', cost: null, calls: null});
  expect(sources.find((s) => s.name === 'canned').program.kind).toBe('stub');
});

test('each row says its state in words and a shape; running on top, then recent, stub runs collapsed', async ({page}) => {
  await page.goto(`${base}/?run=done-ok`);
  await expect(page.locator('#runpick .pk-nm')).toHaveText('done-ok');
  await open(page);
  const groups = await page.locator('#pk-list').evaluate((el) => [...el.querySelectorAll('[role=group]')].map((g) => [g.getAttribute('aria-label'), [...g.querySelectorAll('.pk-rn')].map((n) => n.textContent)]));
  expect(groups).toEqual([['Running now', ['live-a']], ['Recent', ['done-ok', 'done-bad', 'halted', 'paused']]]);
  await expect(page.locator('#pk-stubs')).toHaveText('Show stub and fixture runs (1)');
  const states = {'live-a': ['m-run', 'running'], 'done-ok': ['m-ok', 'closed · operating'], 'done-bad': ['m-bad', 'closed · not delivered'], 'halted': ['m-stop', 'stopped · writer spent'], 'paused': ['m-idle', 'not running']};
  for (const [name, [cls, text]] of Object.entries(states)) {
    await expect(rowOf(page, name).locator('.pk-mk')).toHaveClass(new RegExp(cls));
    await expect(rowOf(page, name).locator('.pk-mt')).toHaveText(text);
  }
  // the run on screen is the selected option
  await expect(rowOf(page, 'done-ok')).toHaveAttribute('aria-selected', 'true');
  // a row at a glance: when, how long, cost, calls, the models, the counts
  const ok = rowOf(page, 'done-ok').locator('.pk-r2');
  for (const t of ['started 50 min ago', 'took 30:00', '$1.00', '3 calls', 'decider Haiku · work Sonnet', '2 reopens', '1 forced']) await expect(ok).toContainText(t);
  await expect(ok.locator('[title^="started "]')).toHaveCount(1);
  // a running run's duration is live
  const dur = rowOf(page, 'live-a').locator('[data-run-since]');
  const d0 = await dur.textContent();
  await expect(dur).not.toHaveText(d0, {timeout: 3000});
  // the stub group opens on demand, its run labelled
  await page.locator('#pk-stubs').click();
  await expect(page.locator('#pk-list .g-stub .pk-rn')).toHaveText(['canned']);
  await expect(rowOf(page, 'canned').locator('.pk-stub')).toHaveText('stub');
  await page.screenshot({path: path.join(shots, '1600x1000-run-picker.png')});
});

test('search filters as you type: name, model, outcome, stop cause; nothing found says so', async ({page}) => {
  await page.goto(`${base}/?run=done-ok`);
  await open(page);
  const names = () => page.locator('#pk-list .pk-rn').allTextContents();
  const q = page.locator('#pk-q');
  for (const [text, want] of [['opus', ['live-a']], ['writer spent', ['halted']], ['not delivered', ['done-bad']], ['done', ['done-ok', 'done-bad']], ['canned', ['canned']], ['stub', ['canned']]]) {
    await q.fill(text);
    await expect.poll(names, {message: `search "${text}"`}).toEqual(want);
  }
  await q.fill('zzz');
  await expect(page.locator('#pk-none')).toHaveText('No run matches “zzz”');
});

test('the keyboard alone: Enter opens with the focus in the search, arrows and Enter open a run, the URL names it, Escape closes', async ({page}) => {
  await page.goto(`${base}/?run=done-ok`);
  await page.locator('#runpick').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#pk-panel')).toBeVisible();
  await expect(page.locator('#pk-q')).toBeFocused();
  await expect(page.locator('#runpick')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.locator('#pk-q')).toHaveAttribute('aria-activedescendant', `pk-o-${src('done-ok')}`);
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('#pk-q')).toHaveAttribute('aria-activedescendant', `pk-o-${src('done-bad')}`);
  await expect(rowOf(page, 'done-bad')).toHaveClass(/\bact\b/);
  await page.keyboard.press('Enter');
  await expect(page.locator('#pk-panel')).toBeHidden();
  await expect(page.locator('#runpick .pk-nm')).toHaveText('done-bad');
  await expect(page.locator('#runpick')).toBeFocused();
  const url = new URL(page.url());
  expect([url.searchParams.get('run'), url.searchParams.get('source')]).toEqual(['done-bad', String(src('done-bad'))]);
  // the link opens that run
  await page.goto(url.href);
  await expect(page.locator('#runpick .pk-nm')).toHaveText('done-bad');
  // Escape closes and gives the focus back; a focus that leaves the panel closes it
  await page.keyboard.press('Tab');
  await page.locator('#runpick').focus();
  await page.keyboard.press('Enter');
  await page.keyboard.press('Escape');
  await expect(page.locator('#pk-panel')).toBeHidden();
  await expect(page.locator('#runpick')).toBeFocused();
  await page.keyboard.press('Enter');
  await page.locator('#follow').focus();
  await expect(page.locator('#pk-panel')).toBeHidden();
  // roles: a combobox that controls a listbox of options in groups
  await page.locator('#runpick').click();
  await expect(page.getByRole('combobox', {name: /Search runs/})).toBeVisible();
  await expect(page.getByRole('listbox', {name: 'Runs'}).getByRole('option')).toHaveCount(5);
  expect(await axeViolations(page, {include: ['#picker']})).toEqual([]);
});

test.describe('operator tile', () => {
  test.use({viewport: {width: 805, height: 1270}});
  test('in an 805x1270 desktop window the button fits the header beside Decisions and the Live pill; the panel is in view (D-008)', async ({page}) => {
    await page.goto(`${base}/?run=live-a`);
    await expect(page.locator('#runpick .pk-nm')).toHaveText('live-a');
    expect(await page.evaluate(() => document.documentElement.classList.contains('phone'))).toBe(false);
    const box = (sel) => page.locator(sel).evaluate((e) => e.getBoundingClientRect().toJSON());
    for (const sel of ['#runpick', '#decbtn', '#live']) { const r = await box(sel); expect(r.left, sel).toBeGreaterThanOrEqual(0); expect(r.right, sel).toBeLessThanOrEqual(805); }
    const n = await page.locator('#runpick .pk-nm').evaluate((e) => e.scrollWidth <= e.clientWidth + 1);
    expect(n, 'the run name whole in the button').toBe(true);
    expect(await horizontalOverflow(page)).toEqual({doc: 0, wide: []});
    await open(page);
    const p = await box('#pk-panel');
    expect(p.left).toBeGreaterThanOrEqual(0);
    expect(p.right).toBeLessThanOrEqual(805);
    expect(p.bottom).toBeLessThanOrEqual(1270);
    expect(await axeViolations(page, {include: ['#picker']})).toEqual([]);
    await page.screenshot({path: path.join(shots, '805x1270-run-picker.png')});
  });
});

test.describe('phone', () => {
  test.use({viewport: {width: 390, height: 844}, isMobile: true, hasTouch: true});
  test('on a phone the panel is a full-screen sheet with Close in view, 44 px targets and phone text sizes', async ({page}) => {
    await page.goto(`${base}/?run=done-ok`);
    await open(page);
    expect(await sheetProblems(page, phone.sheets.picker.dialog, phone.sheets.picker.close)).toEqual([]);
    expect(await horizontalOverflow(page)).toEqual({doc: 0, wide: []});
    expect(await smallTargets(page, '#pk-panel button, #pk-panel input, #pk-panel [role=option]', phone.target)).toEqual([]);
    expect(await smallText(page, phone.text)).toEqual([]);
    expect(await axeViolations(page, {include: ['#picker']})).toEqual([]);
    await page.screenshot({path: path.join(shots, '390x844-run-picker.png')});
    await rowOf(page, 'halted').click();
    await expect(page.locator('#pk-panel')).toBeHidden();
    await expect(page.locator('#runpick .pk-nm')).toHaveText('halted');
  });
});
