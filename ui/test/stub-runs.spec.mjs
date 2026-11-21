// Stub, fixture and dry runs are never listed beside real runs without a label (server.mjs programOf).
// Four runs played by test/fixture-run.mjs (person) into SDC_UI_FIXTURE_DIR (default test/fixtures/runs)/
// programs, then told apart by what they say about their program: two real (run-settings.jsonl adapter
// metaharness; an older one with llm-calls.jsonl naming a model and no settings), two stub (adapter
// llm-adapter-fixture.sh; dry llm-calls.jsonl lines only). Served by a server of its own over the
// runs directory on port 7534 (SDC_UI_STUB_PORT); the last two tests run in the operator tile (805x1270,
// a desktop window) and at the gating phone size 390x844 as a phone (touch, coarse pointer).
// Screenshots: screenshots/<project>-stub-run.png, screenshots/805x1270-stub-run.png, screenshots/390x844-stub-run.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun, fixtureSource} from './need-run.mjs';
import {phone} from '../ux/adapter.mjs';
import {horizontalOverflow, smallTargets, smallText} from '../ux/core/phone.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const runs = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'programs');
const port = Number(process.env.SDC_UI_STUB_PORT || 7534);
const base = `http://127.0.0.1:${port}`;
const NOTE = "stub run: canned content, not a model's work";
const HEAD = 'Stub and fixture runs (no model calls)';
const procs = [];
let sources;
const src = (name) => sources.find((s) => s.name === name).index;
const settings = (adapter) => JSON.stringify({format: 'sdc.run-settings/1', at: '2026-11-21T10:00:00Z', leg: 'started', autonomy: 'A3', adapter}) + '\n';
const call = (extra) => JSON.stringify({action: 'idea.dissect', case: 'IDEA-1.intake', model: 'claude-haiku-5-5', exit: 0, ...extra}) + '\n';
// each run: what it says about its program, and how many minutes ago it last wrote
const RUNS = {
  'real-new': {files: {'run-settings.jsonl': settings('metaharness')}, ago: 5},
  'real-old': {files: {'llm-calls.jsonl': call({adapter: 'metaharness', cost_usd: 0.0025})}, ago: 50},
  'stub-new': {files: {'run-settings.jsonl': settings('/x/driver/examples/sdc/tools/llm-adapter-fixture.sh')}, ago: 1},
  'dry-old': {files: {'llm-calls.jsonl': call({adapter: 'dry', dry: true})}, ago: 90},
};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server: one project is enough'));
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(120000);
  needRun(fixtureSource('person'), 'stub-runs.spec.mjs (fixture source)', test);
  fs.rmSync(runs, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  for (const [name, r] of Object.entries(RUNS)) {
    const dir = path.join(runs, name);
    const p = spawnSync('node', ['test/fixture-run.mjs', 'person', dir], {cwd: ui, encoding: 'utf8'});
    if (p.status !== 0) throw new Error(`fixture ${name}: ${p.stderr}`);
    for (const f of ['run-settings.jsonl', 'llm-calls.jsonl']) fs.rmSync(path.join(dir, f), {force: true});
    for (const [f, text] of Object.entries(r.files)) fs.writeFileSync(path.join(dir, f), text);
    const t = new Date(Date.now() - r.ago * 60000);
    for (const p of [dir, path.join(dir, 'store'), ...Object.keys(r.files).map((f) => path.join(dir, f))]) fs.utimesSync(p, t, t);
  }
  if (await fetch(base + '/api/sources').then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_STUB_PORT`);
  procs.push(spawn(process.execPath, ['server.mjs', '--port', String(port), '--runs', runs], {cwd: ui, stdio: 'ignore'}));
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((x) => setTimeout(x, 100)); } }
  sources = await (await fetch(base + '/api/sources')).json();
});

test('each run is real or stub by what it says about its program', () => {
  const kind = Object.fromEntries(sources.map((s) => [s.name, s.program.kind]));
  expect(kind).toEqual({'real-new': 'real', 'real-old': 'real', 'stub-new': 'stub', 'dry-old': 'stub'});
  expect(sources.find((s) => s.name === 'stub-new').program.why).toContain('llm-adapter-fixture.sh');
});

test('the run list: real runs first, newest first, stub runs in a group below, each labelled; the newest real run is the default', async ({page}, info) => {
  await page.goto(base + '/');
  const sel = page.locator('#source');
  await expect(sel).toHaveValue(String(src('real-new')));
  const list = await sel.evaluate((el) => [...el.children].map((c) => (c.tagName === 'OPTGROUP' ? {group: c.label, options: [...c.children].map((o) => o.textContent)} : c.textContent)));
  expect(list).toEqual(['real-new', 'real-old', {group: HEAD, options: ['stub-new · stub', 'dry-old · stub']}]);
  // a real run has no stub label anywhere
  await expect(page.locator('#runmark .rm-stub')).toHaveCount(0);
  await expect(page.locator('#now .now-stub')).toHaveCount(0);
  // the newest run of all is a stub: still not the default
  expect(sources.find((s) => s.name === 'stub-new').lastWrite).toBeGreaterThan(sources.find((s) => s.name === 'real-new').lastWrite);
  // a stub run says so beside its name in the header and in the Now band
  await sel.selectOption(String(src('stub-new')));
  await expect(page.locator('#runmark .rm-stub')).toHaveText(NOTE);
  await expect(page.locator('#now .now-stub')).toHaveText(NOTE);
  await page.goto(`${base}/?source=${src('dry-old')}`);
  await expect(page.locator('#runmark .rm-stub')).toHaveText(NOTE);
  await expect(page.locator('#now .now-stub')).toHaveText(NOTE);
  await page.screenshot({path: path.join(ui, 'screenshots', `${info.project.name}-stub-run.png`)});
});

test.describe('operator tile', () => {
  test.use({viewport: {width: 805, height: 1270}});
  test('in an 805x1270 desktop window the desktop layout, the stub labels in view, nothing past the edge (D-008)', async ({page}) => {
    await page.goto(`${base}/?source=${src('stub-new')}`);
    await expect(page.locator('#now .now-stub')).toHaveText(NOTE);
    expect(await page.evaluate(() => document.documentElement.classList.contains('phone')), 'desktop layout').toBe(false);
    await expect(page.locator('#nowbar')).toBeHidden();
    expect(await horizontalOverflow(page)).toEqual({doc: 0, wide: []});
    const top = await page.evaluate(() => ['header', '#kpis', '#now', '#phases'].map((s) => { const r = document.querySelector(s).getBoundingClientRect(); return [s, Math.round(r.top), Math.round(r.bottom)]; }));
    for (let i = 1; i < top.length; i++) expect(top[i][1], `${top[i][0]} below ${top[i - 1][0]}`).toBeGreaterThanOrEqual(top[i - 1][2] - 1);
    for (const el of [page.locator('#runmark .rm-stub'), page.locator('#now .now-stub'), page.locator('#decbtn')]) {
      const r = await el.evaluate((e) => e.getBoundingClientRect().toJSON());
      expect(r.left).toBeGreaterThanOrEqual(0);
      expect(r.right).toBeLessThanOrEqual(805);
    }
    await page.screenshot({path: path.join(ui, 'screenshots', '805x1270-stub-run.png')});
  });
});

test.describe('phone', () => {
  test.use({viewport: {width: 390, height: 844}, isMobile: true, hasTouch: true});
  test('on a phone (390x844) the stub labels are in view and not cut, and the phone contract holds', async ({page}) => {
    await page.goto(`${base}/?source=${src('stub-new')}`);
    const mark = page.locator('#runmark .rm-stub'), row = page.locator('#now .now-stub'), sel = page.locator('#source');
    await expect(mark).toHaveText(NOTE);
    await expect(row).toHaveText(NOTE);
    // in the first screen, inside the screen's width, nothing cut by its own box or an ellipsis
    for (const [what, el] of [['header label', mark], ['Now label', row]]) {
      const f = await el.evaluate((e) => { const r = e.getBoundingClientRect(); return {left: r.left, right: r.right, top: r.top, bottom: r.bottom, sw: e.scrollWidth, cw: e.clientWidth, sh: e.scrollHeight, ch: e.clientHeight, px: parseFloat(getComputedStyle(e).fontSize)}; });
      expect(f.left, what).toBeGreaterThanOrEqual(0);
      expect(f.right, what).toBeLessThanOrEqual(390);
      expect(f.top, what).toBeGreaterThanOrEqual(0);
      expect(f.bottom, what).toBeLessThanOrEqual(844);
      expect(f.sw, `${what}: cut sideways`).toBeLessThanOrEqual(f.cw + 1);
      expect(f.sh, `${what}: cut below`).toBeLessThanOrEqual(f.ch + 1);
      expect(f.px, `${what}: text size`).toBeGreaterThanOrEqual(phone.text.min);
    }
    // the store select shows the whole "name · stub", the stub group is in its list, and it is a 44 px target
    const s = await sel.evaluate((e) => {
      const cs = getComputedStyle(e), ctx = document.createElement('canvas').getContext('2d');
      ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
      const r = e.getBoundingClientRect();
      return {text: e.selectedOptions[0].textContent, need: ctx.measureText(e.selectedOptions[0].textContent).width, room: e.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight), w: r.width, h: r.height, right: r.right,
        group: [...e.querySelectorAll('optgroup')].map((g) => g.label)};
    });
    expect(s.text).toBe('stub-new · stub');
    expect(s.need, `"${s.text}" needs ${Math.ceil(s.need)} px, the select shows ${Math.floor(s.room)}`).toBeLessThanOrEqual(s.room);
    expect(s.group).toEqual([HEAD]);
    expect(Math.min(s.w, s.h)).toBeGreaterThanOrEqual(phone.target);
    expect(s.right).toBeLessThanOrEqual(390);
    // the phone layout contract on this page
    expect(await horizontalOverflow(page)).toEqual({doc: 0, wide: []});
    expect(await smallTargets(page, phone.targets, phone.target)).toEqual([]);
    expect(await smallText(page, phone.text)).toEqual([]);
    await page.screenshot({path: path.join(ui, 'screenshots', '390x844-stub-run.png')});
  });
});
