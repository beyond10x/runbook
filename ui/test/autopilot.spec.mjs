// "It should never say person if there isn't a person" (the operator, 2026-11-21). A run in autopilot
// (run-settings.jsonl: autonomy A5, stand_in true; every principal decider:haiku) and a legacy run whose
// records say "human" without an id, played by test/fixture-run.mjs into SDC_UI_FIXTURE_DIR (default
// test/fixtures/runs)/principals, served by a server of its own (test/serve.mjs: a port the kernel gives)
// over the runs directory. Screenshots: screenshots/{1600x1000,1366x768}-autopilot.png, -human.png.
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const runs = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'principals');
let base;
const GATING = [{name: '1600x1000', width: 1600, height: 1000}, {name: '1366x768', width: 1366, height: 768}];
const procs = [];
let sources;
const src = (name) => sources.find((s) => s.path.endsWith(`/${name}/store`)).index;
// the regions that name who decided or who holds a decision
const REGIONS = ['#now', '#kpis', '#feed', '#tree', '.legend', '#runmark', '#drawer'];
// what the page says, without the artifacts' own text (a roadmap may speak of a person)
const said = (page) => page.evaluate((sel) => sel.map((s) => [...document.querySelectorAll(s)].map((e) => {
  const k = e.cloneNode(true);
  for (const x of k.querySelectorAll('.art .mdv, .art pre, .ctx .cv, .ctx .cr, .dreasons, .dr1, .dec, .fd-more')) x.remove();
  const w = document.createTreeWalker(k, NodeFilter.SHOW_TEXT);
  const out = [];
  for (let n = w.nextNode(); n; n = w.nextNode()) out.push(n.nodeValue);
  return out.join('\n');
}).join('\n')).join('\n'), REGIONS).then((t) => t.split('\n').filter((l) => /\bperson\b|\byou\b/i.test(l)));

test.describe.configure({mode: 'serial'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, both gating viewports set per test'));
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  needRun(fixtureSource('autopilot'), 'autopilot.spec.mjs (fixture source)', test);
  fs.rmSync(runs, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  const r = spawnSync('node', ['test/fixture-run.mjs', 'human', path.join(runs, 'human')], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`fixture human: ${r.stderr}`);
  { const s = await startServer(['--runs', runs]); base = s.url; procs.push(s.proc); }
});

test('autopilot: the decider is deciding or no decision is pending, never a person, never "waiting on you"', async ({page}) => {
  test.setTimeout(60000);
  // the decider's call is held after its last event, response still empty, until the page has been seen deciding
  const hold = path.join(runs, 'autopilot.hold');
  procs.push(spawnChild('node', ['test/fixture-run.mjs', 'autopilot', path.join(runs, 'autopilot'), '50', hold], {cwd: ui, stdio: 'ignore'}));
  for (let i = 0; i < 100 && !fs.existsSync(path.join(runs, 'autopilot', 'store')); i++) await new Promise((x) => setTimeout(x, 100));
  await expect.poll(async () => (sources = await (await fetch(base + '/api/sources')).json()).some((s) => s.path.endsWith('/autopilot/store')), {timeout: 15000}).toBe(true);
  await page.setViewportSize(GATING[0]);
  await page.goto(`${base}/?source=${src('autopilot')}`);
  const head = page.locator('#now .now-s');
  await expect(head).toHaveText('The decider is deciding', {timeout: 20000});
  await expect(page.locator('#now .nc.wait .nc-hold')).toContainText('Decider');
  await expect(page.locator('#runmark')).toContainText('end-to-end test with a stand-in');
  await expect(page.locator('#runmark')).toContainText('autopilot');
  await expect(page.locator('#kpis .k-standin')).toHaveText('not eval numbers');
  await expect(page.locator('#lg-person')).toHaveText('decider (on the operator’s behalf)');
  expect(await said(page)).toEqual([]);
  // the call ends: nothing pends on anyone
  fs.writeFileSync(hold, '');
  await expect(head).toHaveText('No decision is pending: the decider takes the next one when the run continues', {timeout: 60000});
  await expect(page.locator('#kpis .kt.k-decisions .k-l')).toContainText('Decisions');
  await expect(page.locator('#feed .fd-w.w-decider').first()).toContainText('on the operator’s behalf');
  for (const vp of GATING) {
    await page.setViewportSize(vp);
    await page.goto(`${base}/?source=${src('autopilot')}`);
    await expect(head).toHaveText(/^No decision is pending/);
    expect(await said(page)).toEqual([]);
    await page.screenshot({path: path.join(shots, `${vp.name}-autopilot.png`)});
    await page.goto(`${base}/?source=${src('autopilot')}&drawer=1`);
    await expect(page.locator('#drawer .dr-h')).toContainText(/no decision is pending|held by the decider/);
    expect(await said(page)).toEqual([]);
    await page.screenshot({path: path.join(shots, `${vp.name}-autopilot-drawer.png`)});
  }
  // zero person principals in the store, the decisions and the answers
  const all = (await (await fetch(`${base}/api/inbox?source=${src('autopilot')}`)).json()).history;
  expect(all.length).toBeGreaterThan(0);
  expect(all.filter((d) => d.who === 'person')).toEqual([]);
});

test('a legacy record that says "human" without an id is said as such, never as a person', async ({page}) => {
  sources = await (await fetch(base + '/api/sources')).json();
  const s = src('human');
  const hist = (await (await fetch(`${base}/api/inbox?source=${s}`)).json()).history;
  expect(hist.some((d) => d.principal === 'human')).toBe(true);
  expect(hist.filter((d) => d.principal === 'human').every((d) => d.who === 'legacy')).toBe(true);
  for (const vp of GATING) {
    await page.setViewportSize(vp);
    await page.goto(`${base}/?source=${s}&drawer=1`);
    await expect(page.locator('#drawer .dr-hist')).toContainText('recorded as human (principal not recorded)');
    await expect(page.locator('#feed .fd-w.w-legacy').first()).toHaveText('Recorded as human (principal not recorded)');
    const tile = page.locator('#kpis .kt.k-decisions');
    await expect(tile.locator('.k-v')).toHaveText('0');
    await expect(tile.locator('.k-s')).toContainText('recorded as human');
    expect(await page.locator('#feed').innerText()).not.toMatch(/Person human/);
    await page.screenshot({path: path.join(shots, `${vp.name}-human.png`)});
  }
});
