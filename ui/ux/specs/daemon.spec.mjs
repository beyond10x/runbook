// L2 layout contract and L3 screen-only user for the daemon's run page at a live run (scenario daemon-live, story
// run-page-watches-a-live-run), at each viewport project. The synthetic daemon (test/daemon-live-fixture.mjs) holds the
// run's last two events back: the contract samples the page while the loop releases them (the step in flight starts
// on screen), then the screen-only user reads the scenario's truth with only what it can see. Metrics go to
// ux/out/metrics-<viewport>.jsonl for the ratchet. No page clock: the page knows the daemon served it by the
// Server-Timing name, which a faked clock hides; the fixture anchors its times to now instead.
import {test, expect} from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';
import {daemonScenario as s, daemonPage} from '../adapter.mjs';
import {serve, LIVE} from '../../test/daemon-live-fixture.mjs';
import {scrollOffenders, visibilityMisses, axeViolations, truncatedWithoutValue, startShifts, shiftsPerMinute, visibleText, wordsIn} from '../core/contract.mjs';
import {record, median, p95, max} from '../core/metrics.mjs';
import {OUT, SHOTS, STEP_MS, gating} from './env.mjs';

const SAMPLES = 5;
// animations and transitions off in the measured page, as in the inspector's scenarios (env.mjs): a constructed sheet,
// which the daemon's CSP (no inline style) lets through
const STILL = '*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }';
let d;
test.beforeAll(async () => { d = await serve({hold: 2}); });
test.afterAll(() => d?.close());
test.use({timezoneId: 'Europe/Berlin', locale: 'en-US'});

test(`${s.name}: layout contract and the screen-only user`, async ({page}, info) => {
  const vp = info.project.name;
  const m = (k, v) => record(OUT, vp, s.name, k, v);
  const pol = daemonPage(vp);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((css) => { document.addEventListener('DOMContentLoaded', () => { const st = new CSSStyleSheet(); st.replaceSync(css); document.adoptedStyleSheets = [...document.adoptedStyleSheets, st]; }); }, STILL);
  await fetch(`${d.base}/__fixture/anchor?ago=132000`);
  await page.goto(`${d.base}/?run=${LIVE}`);
  await expect(page.locator('#dsteps tbody tr')).toHaveCount(6, {timeout: 20000});
  await expect(page.locator('#feed .fd-h')).toContainText('12 engine lines folded');
  // the page settled before the shifts count: the header holds its update time and the daemon's controls, the fonts loaded
  await expect(page.locator('#live .lt')).toBeAttached();
  await expect(page.locator('#startbtn')).toBeAttached();
  await expect(page.locator('#routesbtn')).toBeAttached();
  await page.evaluate(() => document.fonts.ready.then(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))));
  await startShifts(page);

  // ---- L2: sample the states while the run moves: the two held events (the engine's evaluation, the start of the
  // step in flight) are released after the first two samples; the duration of the step in flight ticks after that
  const scroll = [], vis = [], trunc = [], words = [];
  let axe = [];
  const vw = page.viewportSize().width;
  for (let i = 0; i < SAMPLES; i++) {
    scroll.push(...(await scrollOffenders(page, pol.scrollAllowed)).filter((x) => !(pol.pageScrollY && x.sel === 'document' && x.w <= vw + 1)).map((x) => x.sel));
    vis.push(...(await visibilityMisses(page, pol.alwaysVisible)).map((x) => `${x.sel}: ${x.why}`));
    trunc.push((await truncatedWithoutValue(page)).length);
    words.push(await wordsIn(page, s.primary));
    if (i < 2) {
      await fetch(`${d.base}/__fixture/next`);
      // each release painted before the next sample: the evaluation is folded (the count says so), the start is a row
      if (i === 0) await expect(page.locator('#feed .fd-h')).toContainText('13 engine lines folded');
      else await expect(page.locator('#dsteps tbody tr[data-state=running]')).toHaveCount(1);
    } else {
      if (i === 2) { axe = await axeViolations(page); if (SHOTS) { fs.mkdirSync(SHOTS, {recursive: true}); await page.screenshot({path: path.join(SHOTS, `${vp}-${s.name}.png`)}); } }
      await page.waitForTimeout(1000);
    }
  }
  // per minute of the loop's steps (one sample a step, as the inspector's scenarios), not of this machine's wall time
  m('layout_shift.per_minute', await shiftsPerMinute(page, (SAMPLES * STEP_MS) / 60000));
  m('reading_load.max', max(words));
  m('reading_load.median', median(words));
  m('contract.scroll', new Set(scroll).size);
  m('contract.hidden', new Set(vis).size);
  m('contract.axe_serious', axe.length);
  m('contract.truncated_hidden', max(trunc));
  if (gating(vp)) {
    expect.soft([...new Set(scroll)], 'hard gate 1: nothing scrolls but what the baseline allows').toEqual([]);
    expect.soft([...new Set(vis)], 'hard gate 2: always visible').toEqual([]);
    expect.soft(axe, 'hard gate 5: axe serious or critical').toEqual([]);
  }

  // ---- L3: the screen-only user reads the truth, with no action: each task is a fix of 0 actions -----------------
  const u = {looks: [], fixes: []};
  if (pol.pageScrollY) await page.evaluate(() => window.scrollTo(0, 0));
  // a text in view in the primary area is a glance 1, elsewhere 2; scrolled to (a phone) or not readable, 3
  const read = async (scope, text) => {
    const glance = scope.startsWith(s.primary) || scope === '#dnow' ? 1 : 2;
    if (await visibleText(page, scope, text)) { u.looks.push(glance); return true; }
    if (pol.pageScrollY) {
      await page.locator(scope).getByText(text, {exact: false}).first().evaluate((e) => e.scrollIntoView({block: 'center'})).catch(() => null);
      if (await visibleText(page, scope, text)) { u.looks.push(3); return true; }
    }
    u.looks.push(3);
    return false;
  };
  // a task is done when every reading of it is: a fix of 0 actions, else 3 (not done)
  const task = (oks) => u.fixes.push(oks.every(Boolean) ? 0 : 3);
  const run = s.truth.running, failed = s.truth.failed;
  const running = '#dsteps tr[data-state=running]', exit = '#dsteps tr[data-state=exit]';
  const sees = async (scope, text, what) => { const ok = await read(scope, text); expect.soft(ok, what).toBe(true); return ok; };
  task([await sees(running, run.action, `sees ${run.action} in flight`), await sees(running, run.role, `sees who runs it: ${run.role}`),
    await sees('#dnow', 'for 2 min', 'sees for how long it runs')]);
  task([await sees(exit, failed.action, `sees the step that failed: ${failed.action}`), await sees(exit, failed.state, `sees how it ended: ${failed.state}`)]);
  task([await sees('#dsteps .ds-counts', s.truth.calls, `reads ${s.truth.calls}`), await sees('#dsteps .ds-counts', s.truth.cost, `reads the cost ${s.truth.cost}`)]);
  m('screen_only.actions', 0);
  m('screen_only.looks', u.looks.reduce((a, b) => a + b, 0));
  m('screen_only.steers', 0);
  m('glance_cost.median', median(u.looks));
  m('glance_cost.p95', p95(u.looks));
  m('actions_per_fix.max', max(u.fixes));
  m('actions_per_fix.median', median(u.fixes));
  if (gating(vp)) expect.soft(max(u.fixes), 'hard gate 4: every fix in at most 2 actions').toBeLessThanOrEqual(2);
  expect.soft(errors, 'no page errors').toEqual([]);
});
