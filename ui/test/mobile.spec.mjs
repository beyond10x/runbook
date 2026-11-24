// The phone findings of round R-03 (ux/findings/UXF-016 to UXF-026), one test each, at the gating phone
// size 390x844 (decision D-006). Self-contained: one server of its own (test/serve.mjs:
// a port the kernel gives) over runs played by test/fixture-run.mjs into SDC_UI_FIXTURE_DIR (default
// ui/target/fixture-runs)/mobile-runs: person (waiting on a person, writable), stopped (stopped on purpose,
// a failed waiting KPI) and decider (a decider deciding, with file changes). They play from the
// committed slim copy when runs/ is absent, so the suite runs on a fresh checkout.
// Screenshots: screenshots/390x844-mobile-*.png.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {playFixtures} from './fixtures.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';
import {settled} from './settled.mjs';
import {horizontalOverflow, smallTargets, smallText, coveredControls, alwaysVisibleAcross, withinOneScroll, sheetProblems} from '../ux/core/phone.mjs';
import {axeViolations, truncatedWithoutValue} from '../ux/core/contract.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const runs = path.join(FIXTURE_DIR, 'mobile-runs');
let base;
const PHONE = {width: 390, height: 844};
const TARGETS = 'button, a[href], select, input:not([type=hidden]), textarea, summary, [role=button], [role=tab], label.switch, [data-goto], [data-case], [data-feed], [data-seq], [data-tab], #graph [data-id]';
const ALWAYS = {state: ['#nowbar .nb-state'], step: ['#nowbar .nb-step'], decisions: ['#decbtn']};
const SRC = {person: 0, stopped: 1, decider: 2};
let srv;

test.describe.configure({mode: 'default'});
// a phone: its size, touch and a coarse pointer (the page's phone layout is for a phone, index.html SDC_PHONE)
test.use({viewport: PHONE, isMobile: true, hasTouch: true});
test.beforeEach(async ({page}, info) => {
  test.skip(info.project.name !== 'wide', 'own server: one project is enough');
  await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
});
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  for (const s of Object.keys(SRC)) needRun(fixtureSource(s), `mobile.spec.mjs (fixture source of ${s})`, test);
  fs.rmSync(runs, {recursive: true, force: true});
  fs.mkdirSync(runs, {recursive: true});
  await playFixtures(Object.keys(SRC).map((s) => [s, path.join(runs, s), '1000']));
  ({url: base, proc: srv} = await startServer([...Object.keys(SRC).map((s) => path.join(runs, s, 'store'))]));
});

async function open(page, s) {
  await page.goto(`${base}/?source=${SRC[s]}`);
  await expect(page.locator('#now .now-h')).toBeVisible({timeout: 20000});
  await expect(page.locator('#kpis .kt').first()).toBeVisible();
  await settled(page);
}
const snap = (page, name) => page.screenshot({path: path.join(shots, `390x844-mobile-${name}.png`)});

test('UXF-016: nothing runs past the right edge of the phone, on every run', async ({page}) => {
  for (const s of Object.keys(SRC)) {
    await open(page, s);
    const o = await horizontalOverflow(page);
    expect.soft(o.doc, `${s}: content past the right edge (px)`).toBe(0);
    expect.soft(o.wide, `${s}: elements running past the screen`).toEqual([]);
  }
});

test('UXF-017: the Decisions control is in view and 44 px tall, at the top and scrolled to the bottom', async ({page}) => {
  for (const s of ['person', 'stopped']) {
    await open(page, s);
    expect.soft(await alwaysVisibleAcross(page, {decisions: ALWAYS.decisions}), `${s}: Decisions in view at every scroll`).toEqual([]);
    const r = await page.locator('#decbtn').boundingBox();
    expect.soft(r && r.height >= 44 && r.width >= 44, `${s}: Decisions is a 44x44 target`).toBe(true);
  }
});

test('UXF-018: the run state and the current step stay readable in the status bar while the page scrolls', async ({page}) => {
  // the decider fixture, played to its end, waits on a person again after the decider's call
  const want = {person: [/^Waiting on you/, /^context\.approve · held by You/], stopped: [/^Stopped by operator at/, /^Was (awaiting authority|waiting on a person) for context\.approve in IDEA-1\.direction/], decider: [/^(Decider deciding|Waiting on you)/, /^[a-z]+\.[a-z]+ · held by /]};
  for (const [s, [state, step]] of Object.entries(want)) {
    await open(page, s);
    expect.soft(await alwaysVisibleAcross(page, {state: ALWAYS.state, step: ALWAYS.step}), `${s}: state and step in view at every scroll`).toEqual([]);
    await expect.soft(page.locator('#nowbar .nb-state'), `${s}: the state in words`).toHaveText(state);
    await expect.soft(page.locator('#nowbar .nb-step'), `${s}: the current step`).toHaveText(step);
    // the bar says what the Now band's head says
    expect.soft(await page.locator('#nowbar .nb-state').getAttribute('title'), `${s}: the bar and the band agree`).toBe((await page.locator('#now .now-s').innerText()).trim());
    // a line the bar shortens keeps its whole value (title), and the Now band below says it in full
    expect.soft(await truncatedWithoutValue(page, '#nowbar'), `${s}: nothing cut without its value`).toEqual([]);
    expect.soft(await truncatedWithoutValue(page, '#now'), `${s}: the Now band cuts nothing`).toEqual([]);
    expect.soft(await coveredControls(page, '#now [data-goto], #now button'), `${s}: nothing covers the Now band`).toEqual([]);
    if (s === 'stopped') await snap(page, 'stopped');
  }
});

test('UXF-019: the right panel is a full-screen sheet opened from Details, never a column', async ({page}) => {
  await open(page, 'person');
  // closed: no column beside the page
  expect.soft(await page.locator('#right').isVisible(), 'no rail column while closed').toBe(false);
  const details = page.getByRole('button', {name: 'Details'});
  await expect(details).toBeVisible();
  await details.click();
  await expect(page.locator('html.rp-open')).toHaveCount(1);
  await settled(page);
  expect.soft(await sheetProblems(page, '#right', '#rpclose'), 'the panel is a full-screen sheet, Close in view').toEqual([]);
  await expect.soft(page.locator('#panel')).toBeVisible();
  await snap(page, 'panel');
  await page.locator('#rpclose').click();
  await expect(page.locator('html.rp-open')).toHaveCount(0);
  expect.soft(await page.locator('#right').isVisible(), 'closed again').toBe(false);
});

test('UXF-020: the Decisions drawer and the change dialog are full-screen sheets, Close in view and the line that says where a decision is answered', async ({page}) => {
  await open(page, 'person');
  await page.locator('#now').getByRole('button', {name: 'Decide'}).click();
  await expect(page.locator('body.drawer-open')).toBeVisible();
  await settled(page);
  expect.soft(await sheetProblems(page, '#drawer', '#drclose'), 'the drawer is a full-screen sheet').toEqual([]);
  await expect.soft(page.locator('#drawer #decide-at')).toBeInViewport({ratio: 1});
  expect.soft(await horizontalOverflow(page).then((o) => o.wide), 'nothing in the drawer runs past the screen').toEqual([]);
  await snap(page, 'drawer');
  await page.locator('#drclose').click();
  await open(page, 'decider');
  const row = page.locator('#feed .fd-row button.fd-s').first();
  await row.scrollIntoViewIfNeeded();
  await row.click();
  await expect(page.locator('#diffdlg[open]')).toHaveCount(1);
  await settled(page);
  expect.soft(await sheetProblems(page, '#diffdlg[open]', '[data-fsclose]'), 'the change dialog is a full-screen sheet').toEqual([]);
  await snap(page, 'dialog');
});

test('UXF-021: the case graph scrolls in its own box, with a visible hint, at a readable size', async ({page}) => {
  await open(page, 'person');
  await page.locator('#graph').scrollIntoViewIfNeeded();
  const g = await page.evaluate(() => {
    const el = document.querySelector('#graph');
    el.scrollIntoView({block: 'start'});
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    const hint = document.querySelector('[data-ux=hint]');
    const h = hint?.getBoundingClientRect();
    const svg = el.querySelector('svg');
    const sub = el.querySelector('.nsub');
    return {wider: el.scrollWidth > el.clientWidth + 1, scrolls: /(auto|scroll)/.test(cs.overflowX), height: r.height, right: r.right,
      hint: !!(hint && h.height && h.top >= 0 && h.bottom <= innerHeight), subPx: sub ? parseFloat(getComputedStyle(sub).fontSize) * svg.getScreenCTM().a : null};
  });
  expect.soft(g.right, 'the graph box ends at the screen edge').toBeLessThanOrEqual(PHONE.width + 1);
  expect.soft(g.height, 'the graph box is high enough to read').toBeGreaterThanOrEqual(300);
  expect.soft(g.scrolls, 'it scrolls inside its box').toBe(true);
  if (g.wider) expect.soft(g.hint, 'a visible hint says it scrolls sideways').toBe(true);
  expect.soft(g.subPx, 'node sub-labels drawn at 12 px or more').toBeGreaterThanOrEqual(12);
  await snap(page, 'graph');
});

test('UXF-022: every tap target is at least 44x44 CSS px, and axe finds no serious target-size problem', async ({page}) => {
  await open(page, 'person');
  expect.soft(await smallTargets(page, TARGETS, 44), 'page').toEqual([]);
  expect.soft((await axeViolations(page)).filter((v) => v.id === 'target-size'), 'axe target-size').toEqual([]);
  await page.locator('#decbtn').click();
  await settled(page);
  expect.soft(await smallTargets(page, TARGETS, 44), 'drawer open').toEqual([]);
  await page.locator('#drclose').click();
  await page.getByRole('button', {name: 'Details'}).click();
  await settled(page);
  expect.soft(await smallTargets(page, TARGETS, 44), 'panel open').toEqual([]);
});

test('UXF-023: no label under 12 px and no body text under 14 px', async ({page}) => {
  for (const s of Object.keys(SRC)) {
    await open(page, s);
    expect.soft(await smallText(page, {min: 12, body: 14}), s).toEqual([]);
  }
});

test('UXF-024: the KPI tiles fit the phone: in the first screen, nothing cut or drawn over another', async ({page}) => {
  for (const s of ['person', 'stopped']) {
    await open(page, s);
    expect.soft((await withinOneScroll(page, '#kpis')).docBottom, `${s}: the KPI strip ends in the first screen`).toBeLessThanOrEqual(PHONE.height);
    expect.soft(await truncatedWithoutValue(page, '#kpis'), `${s}: no KPI cut`).toEqual([]);
    const overlaps = await page.evaluate(() => {
      const out = [];
      for (const t of document.querySelectorAll('#kpis .kt')) {
        const parts = [...t.querySelectorAll('.k-l, .k-v, .k-s, .k-f0, .k-stop')].filter((x) => x.getClientRects().length).map((x) => [x, x.getBoundingClientRect()]);
        for (let i = 0; i < parts.length; i++) for (let j = i + 1; j < parts.length; j++) {
          const [a, ra] = parts[i], [b, rb] = parts[j];
          if (a.contains(b) || b.contains(a)) continue;
          const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left), h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
          if (w > 1 && h > 1) out.push(`${a.className} over ${b.className}`);
        }
      }
      return out;
    });
    expect.soft(overlaps, `${s}: KPI parts drawn over each other`).toEqual([]);
    if (s === 'stopped') await snap(page, 'kpis');
  }
});

test('UXF-026: What changed names each row\'s actor and action in full', async ({page}) => {
  await open(page, 'decider');
  await page.locator('#feed').scrollIntoViewIfNeeded();
  expect.soft(await truncatedWithoutValue(page, '#feed'), 'rows cut without their value').toEqual([]);
});
