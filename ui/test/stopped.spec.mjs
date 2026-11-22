// Stopped runs, waiting on a person as a failure, and runs that appear later. One server of its own
// (a port the kernel gives) over a runs directory (--runs) of runs played by test/fixture-run.mjs
// into SDC_UI_FIXTURE_DIR (default test/fixtures/runs)/stopped-runs, plus one explicit store:
//   person   a person stop, no settings record, not stopped (explicit store argument, source 0)
//   held     the same stop, with run-settings.jsonl giving the decisions to a decider (haiku)
//   stopped  the same stop, the same settings, then stopped on purpose (run-stopped.json)
//   late     appears while the page is open
// Screenshots at both gating viewports: screenshots/{1600x1000,1366x768}-stopped*.png, -held.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'stopped-runs');
const runs = path.join(root, 'runs');
let base;
const GATING = [{name: '1600x1000', width: 1600, height: 1000}, {name: '1366x768', width: 1366, height: 768}];
const fixture = (scenario, name) => { const r = spawnSync('node', ['test/fixture-run.mjs', scenario, path.join(runs, name)], {cwd: ui, encoding: 'utf8'}); if (r.status !== 0) throw new Error(`fixture ${scenario}: ${r.stderr}`); };
let srv, sources;
const src = (name) => sources.find((s) => s.path.endsWith(`/${name}/store`)).index;

test.describe.configure({mode: 'serial'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, both gating viewports set per test'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  needRun(fixtureSource('stopped'), 'stopped.spec.mjs (fixture source)', test);
  fs.rmSync(root, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  fixture('person', 'person');
  fixture('stopped', 'stopped');
  fixture('person', 'held');
  fs.copyFileSync(path.join(runs, 'stopped', 'run-settings.jsonl'), path.join(runs, 'held', 'run-settings.jsonl'));
  ({url: base, proc: srv} = await startServer(['--write', '--principal', 'operator', path.join(runs, 'person', 'store'), '--runs', runs]));
  sources = await (await fetch(base + '/api/sources')).json();
});

test('a stopped run says who stopped it, when and why, what it waited on, and every clock stands at the stop', async ({page}) => {
  const stop = JSON.parse(fs.readFileSync(path.join(runs, 'stopped', 'run-stopped.json'), 'utf8'));
  const clock = await page.evaluate((t) => new Date(t).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}), Date.parse(stop.at));
  const s = src('stopped');
  expect(sources.find((x) => x.index === s).stopped).toMatchObject({by: 'operator', verdict_at_stop: 'awaiting_person', case: 'IDEA-1.direction'});
  for (const vp of GATING) {
    await page.setViewportSize(vp);
    await page.goto(`${base}/?source=${s}`);
    const head = page.locator('#now .now-s');
    await expect(head).toHaveText(`Stopped by operator at ${clock}: ${stop.reason}`);
    await expect(page.locator('#now .now-sub')).toContainText('It was awaiting authority for context.approve in IDEA-1.direction');
    await expect(page.locator('#now .nc.k-stopped')).toBeVisible();
    await expect(page.locator('#now .nc-was').first()).toContainText('until the stop');
    await expect(page.locator('#now').getByRole('button', {name: 'Decide'})).toHaveCount(0);
    // the run picker's button and the case tree mark the run
    await expect(page.locator('#runpick .pk-mk')).toHaveClass(/m-stop/);
    await expect(page.locator('#runpick')).toContainText('stopped');
    await expect(page.locator('#tree .tree-stop')).toHaveText(`Run stopped by operator at ${clock}`);
    await expect(page.locator('#tree .case[data-case="IDEA-1/children/IDEA-1.direction"] .cst')).toHaveText('stopped');
    await expect(page.locator('#tree .cst-spin')).toHaveCount(0);
    // the clocks stand: elapsed and waiting end at the stop; the open stop waited 30 min - 11 min
    const kv = page.locator('#kpis .k-time .k-v');
    const wait = page.locator('#kpis .k-time .k-s b').first();
    await expect(page.locator('#kpis .k-time .k-stop .ks-d').first()).toHaveText(/^(18:59|19:00|19:01)$/);
    const [a, w] = [await kv.textContent(), await wait.textContent()];
    const ticking = await page.locator('[data-since]').evaluateAll((els) => els.map((e) => e.textContent));
    await page.waitForTimeout(2300);
    expect(await kv.textContent()).toBe(a);
    expect(await wait.textContent()).toBe(w);
    expect(await page.locator('[data-since]').evaluateAll((els) => els.map((e) => e.textContent))).toEqual(ticking);
    await page.screenshot({path: path.join(shots, `${vp.name}-stopped.png`)});
  }
});

test('the open decision of a stopped run is shown greyed in the drawer and is not taken', async ({page, request}) => {
  const s = src('stopped');
  const ib = await (await fetch(`${base}/api/inbox?source=${s}`)).json();
  expect(ib.stopped).toMatchObject({by: 'operator'});
  expect(ib.items.length).toBeGreaterThan(0);
  for (const vp of GATING) {
    await page.setViewportSize(vp);
    await page.goto(`${base}/?source=${s}&drawer=1`);
    await expect(page.locator('#drawer .gap.stopgap')).toContainText('Run stopped by operator');
    const cards = page.locator('#drawer .dcard');
    await expect(cards.first()).toBeVisible();
    expect(await cards.count()).toBe(await page.locator('#drawer .dcard.stopped').count());
    await expect(cards.first().locator('.dstop')).toHaveText('run stopped');
    for (const b of await page.locator('#drawer .dcard button.btn').all()) await expect(b).toBeDisabled();
    await expect(page.locator('#decbtn')).not.toHaveClass(/\bhas\b/);
    await page.screenshot({path: path.join(shots, `${vp.name}-stopped-drawer.png`)});
  }
  const item = ib.items.find((i) => i.kind === 'approve');
  const r = await request.post(`${base}/api/act?source=${s}`, {headers: {'x-sdc-ui': '1'}, data: {kind: 'approve', rel: item.rel, action: item.action, revision: item.revision, sha: item.artifact?.sha}});
  expect(r.status()).toBe(409);
  expect((await r.json()).error).toMatch(/^refused: the run was stopped by operator/);
  expect(fs.existsSync(path.join(runs, 'stopped', 'ui-actions.jsonl'))).toBe(false);
});

test('waiting on a person while a decider holds the decisions is a failed KPI with its stops; a person-held run stays neutral', async ({page}) => {
  for (const vp of GATING) {
    await page.setViewportSize(vp);
    // held: the settings record gives the decisions to the decider; the person stop is open
    await page.goto(`${base}/?source=${src('held')}`);
    const tile = page.locator('#kpis .kt.k-time');
    await expect(tile).toHaveClass(/\bfail\b/);
    await expect(tile.locator('.k-f0')).toHaveText('should be 0');
    const stop = tile.locator('.k-stop').first();
    // no driver attached: the open wait on direction ends at the last event, so the longest stop is an earlier one
    const kase = (await stop.locator('.ks-c').textContent()).trim();
    await expect(stop.locator('.ks-a')).toHaveText(/\S/);
    await expect(stop.locator('.ks-d')).toHaveText(/^\d+:\d\d$/);
    // held by, from the settings record (not "not recorded")
    const card = page.locator('#now .nc.wait');
    await expect(card.locator('.nc-hold')).toContainText('Decider claude-haiku-5-5');
    await expect(card.locator('.nc-hold')).toContainText('run-settings.jsonl');
    await expect(card.locator('.nc-hold')).not.toContainText('not recorded');
    await expect(page.locator('#now .now-s')).toHaveText('Held by the decider: it decides when the run continues');
    // no stop record and no driver attached: said, and every clock stands at the last event
    await expect(page.locator('#now .now-sub')).toHaveText(/^no driver attached, last activity [\d: APM]+$/);
    const clocks = () => page.locator('#now .tk, #kpis .kt.k-time').allTextContents();
    const before = await clocks();
    await page.waitForTimeout(2100);
    expect(await clocks()).toEqual(before);
    await page.screenshot({path: path.join(shots, `${vp.name}-held.png`)});
    // the stop links to its event
    await stop.click();
    await expect(page.locator('#tree .case.sel')).toHaveAttribute('data-case', new RegExp(`IDEA-1\\.${kase.replace(/\./g, '\\.')}$`));
    // the stopped run keeps the failure, frozen at the stop
    await page.goto(`${base}/?source=${src('stopped')}`);
    await expect(page.locator('#kpis .kt.k-time')).toHaveClass(/\bfail\b/);
    // no settings record: neutral, and the holder says the setting is not recorded
    await page.goto(`${base}/?source=0`);
    await expect(page.locator('#kpis .kt.k-time')).not.toHaveClass(/\bfail\b/);
    await expect(page.locator('#now .nc.wait .nc-hold')).toContainText('not recorded');
  }
});

test('a person-held setting keeps the tile neutral', async ({page}) => {
  const d = path.join(runs, 'held', 'run-settings.jsonl');
  const line = JSON.parse(fs.readFileSync(d, 'utf8').trim().split('\n').at(-1));
  fs.appendFileSync(d, JSON.stringify({...line, leg: 'resumed', decider: {kind: 'person', model: null, escalate: 'stop', max_decider_calls: 20}}) + '\n');
  await page.goto(`${base}/?source=${src('held')}`);
  await expect(page.locator('#now .nc.wait .nc-hold')).toContainText('gives its decisions to a person');
  await expect(page.locator('#kpis .kt.k-time')).not.toHaveClass(/\bfail\b/);
  await expect(page.locator('#now .now-s')).toHaveText('Waiting on you: nothing runs until this is decided');
});

test('--runs: a run that appears later joins the run picker without a restart', async ({page}) => {
  await page.goto(`${base}/?source=0`);
  // the explicit store stays source 0 and is not listed twice
  expect(sources[0].path).toMatch(/person\/store$/);
  expect(sources.filter((x) => /person\/store$/.test(x.path))).toHaveLength(1);
  await page.locator('#runpick').click();
  const toggle = page.locator('#pk-stubs');
  if (/^Show/.test(await toggle.textContent())) await toggle.click();
  const rows = page.locator('#pk-list [role=option]');
  await expect(rows).toHaveCount(sources.length);
  // a run still being built aside (*.part) is not listed, before or after it is moved into place
  fs.mkdirSync(path.join(runs, 'aside.part', 'store'), {recursive: true});
  fixture('person', 'late');
  await expect(rows).toHaveCount(sources.length + 1, {timeout: 15000});
  await expect(page.locator('#pk-list [role=option]', {hasText: '.part'})).toHaveCount(0);
  // a fixture run: listed with the stub and fixture runs, labelled
  const opt = page.locator('#pk-list .g-stub [role=option]', {has: page.locator('.pk-rn', {hasText: /^late$/})});
  await expect(opt.locator('.pk-stub')).toHaveText('stub');
  await opt.click();
  await expect(page.locator('#tree .case').first()).toBeVisible({timeout: 15000});
  await expect(page.locator('#now .nc.wait')).toBeVisible();
});
