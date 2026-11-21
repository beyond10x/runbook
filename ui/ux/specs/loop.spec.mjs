// L2 layout contract and L3 screen-only user, per scenario, at each viewport project.
// One test per scenario: the driver bot plays the run while the contract samples its states (hard
// gates 1, 2, 3, 5 and the measured values), then the screen-only user does the scenario's tasks
// with only what it can see. Metrics go to ux/out/metrics-<viewport>.jsonl for the ratchet.
import {test, expect} from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';
import {scenarios, primary, cue, policy} from '../adapter.mjs';
import {scrollOffenders, visibilityMisses, axeViolations, truncatedWithoutValue, startShifts, shiftsPerMinute, visibleControl, visibleText, wordsIn} from '../core/contract.mjs';
import {record, median, p95, max} from '../core/metrics.mjs';
import {start, OUT, SHOTS, gating} from './env.mjs';

let env;
test.describe.configure({mode: 'default'});
test.beforeAll(async ({}, info) => { env = await start(info.project.name); });
test.afterAll(() => env?.stop());

const SAMPLES = 5;
const shot = async (page, info, name) => { if (SHOTS) { fs.mkdirSync(SHOTS, {recursive: true}); await page.screenshot({path: path.join(SHOTS, `${info.project.name}-${name}.png`)}); } };

// Hard gate 5, keyboard: the person's decision taken with the keyboard alone.
test('keyboard alone: from the page to the decision', async ({page}, info) => {
  const s = scenarios.find((x) => x.write);
  await env.prepare(page);
  await page.goto(`${env.base}/?source=${env.source(s)}`);
  await expect(page.locator(cue.card)).toBeVisible({timeout: 20000});
  const focusText = () => page.evaluate(() => (document.activeElement?.innerText || document.activeElement?.getAttribute('aria-label') || '').trim());
  let tabs = 0;
  while (tabs < 120 && !/^Decide$/.test(await focusText())) { await page.keyboard.press('Tab'); tabs++; }
  record(OUT, info.project.name, s.name, 'keyboard.tabs_to_cue', tabs);
  expect.soft(await focusText(), 'Tab reaches the cue control').toBe('Decide');
  await page.keyboard.press('Enter');
  await expect.soft(page.locator('body.drawer-open')).toBeVisible();
  // on to the approval with the keyboard alone: Tab to the Approve control, Enter takes the decision
  let more = 0;
  while (more < 60 && !/^Approve t/.test(await focusText())) { await page.keyboard.press('Tab'); more++; }
  record(OUT, info.project.name, s.name, 'keyboard.tabs_to_decide', more);
  expect.soft(await focusText(), 'Tab reaches the Approve control').toMatch(/^Approve t/);
});

for (const s of scenarios) {
  test(`${s.name}: layout contract and the screen-only user`, async ({page}, info) => {
    const vp = info.project.name;
    const m = (k, v) => record(OUT, vp, s.name, k, v);
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    const played = await env.play(s);
    await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
    await env.prepare(page);
    await page.goto(`${env.base}/?source=${env.source(s)}`);
    await expect(page.locator('#now .now-h')).toBeVisible({timeout: 20000});
    await env.sync(page);
    // step the run until the scenario's first truth is on screen: the steps it takes are time_to_act
    let steps = 0;
    const truthOn = async () => {
      if (s.truth.running.length) return (await page.locator('#now .nc.run').count()) >= s.truth.running.length;
      // a wait is the truth once it is the scenario's wait: held by whom the scenario says
      if (s.truth.waiting) return (await page.locator('#now .nc.wait').count()) > 0 && (!s.truth.waiting.who || await visibleText(page, primary, s.truth.waiting.who));
      if (s.truth.completion === 'closed') return (await page.locator('#now.m-closed').count()) > 0;
      return true;
    };
    while (!(await truthOn()) && steps < 40) { await env.step(page); steps++; }
    expect(await truthOn(), 'the scenario reaches its truth').toBe(true);
    m('screen_only.time_to_act_steps.max', steps);
    await startShifts(page);

    // ---- L3, first part: the screen-only user reads the truth while it is the truth ---------------
    const u = {actions: 0, looks: [], steers: 0, fixes: []};
    // at a phone size the page scrolls vertically (D-006): what is below the fold is scrolled to, at
    // the cost of a glance 3 (not in view), and is then read or used where it is
    const pol = policy(vp);
    const scrollTo = async (loc) => { if (!pol.pageScrollY || !(await loc.count())) return false; await loc.filter({visible: true}).first().evaluate((e) => e.scrollIntoView({block: 'center'}), null, {timeout: 5000}).catch(() => null); return true; };
    const read = async (scope, text) => {
      let ok = await visibleText(page, scope, text);
      if (!ok && await scrollTo(page.locator(scope).getByText(text, {exact: false}))) { ok = await visibleText(page, scope, text); if (ok) { u.looks.push(3); return true; } }
      u.looks.push(ok ? (scope === primary ? 1 : 2) : 3);
      return ok;
    };
    const use = async (role, name) => {
      let c = await visibleControl(page, role, name, primary);
      let scrolled = false;
      if (!c && await scrollTo(page.getByRole(role, typeof name === 'string' ? {name, exact: true} : {name}))) { c = await visibleControl(page, role, name, primary); scrolled = true; }
      if (!c) { u.looks.push(3); return false; }
      u.looks.push(scrolled ? 3 : c.glance);
      await c.locator.click();
      u.actions++;
      return true;
    };
    for (const r of s.truth.running) {
      expect.soft(await read(primary, r.action), `sees ${r.action} in flight`).toBe(true);
      expect.soft(await read(primary, r.executor), `sees ${r.action}'s executor ${r.executor}`).toBe(true);
      if (r.detail) expect.soft(await read(primary, r.detail), `sees ${r.detail}`).toBe(true);
    }
    if (s.truth.waiting) {
      expect.soft(await read(primary, s.truth.waiting.action), 'sees what waits').toBe(true);
      expect.soft(await read(primary, s.truth.waiting.who), 'sees who decides').toBe(true);
      if (s.truth.waiting.detail) expect.soft(await read(primary, s.truth.waiting.detail), `sees ${s.truth.waiting.detail}`).toBe(true);
      if (s.truth.waiting.holder) expect.soft(await read(primary, s.truth.waiting.holder), `sees who holds the decision: ${s.truth.waiting.holder}`).toBe(true);
      if (s.truth.waiting.say) expect.soft(await read(primary, s.truth.waiting.say), `reads in words: ${s.truth.waiting.say}`).toBe(true);
    }

    // ---- L2: sample the states while the run moves, one step of the shared clock apart -------------
    const scroll = [], vis = [], trunc = [], words = [], keys = [];
    let axe = [];
    if (pol.pageScrollY) await page.evaluate(() => window.scrollTo(0, 0));
    const vw = page.viewportSize().width;
    for (let i = 0; i < SAMPLES; i++) {
      scroll.push(...(await scrollOffenders(page, pol.scrollAllowed)).filter((x) => !(pol.pageScrollY && x.sel === 'document' && x.w <= vw + 1)).map((x) => x.sel));
      vis.push(...(await visibilityMisses(page, pol.alwaysVisible)).map((x) => `${x.sel}: ${x.why}`));
      trunc.push((await truncatedWithoutValue(page)).length);
      words.push(await wordsIn(page, primary));
      keys.push(await page.locator('#now .now-cards > .nc').first().getAttribute('data-key').catch(() => null));
      if (i === 0) { axe = await axeViolations(page); await shot(page, info, `${s.name}-0`); }
      await env.step(page);
    }
    // the cue (the Now band's first card): how often it changes, and whether it comes back (A→B→A)
    const k2 = keys.map((k) => k || '');
    let changes = 0, flips = 0;
    for (let i = 1; i < k2.length; i++) if (k2[i] !== k2[i - 1]) { changes++; if (i > 1 && k2[i] === k2[i - 2]) flips++; }
    m('layout_shift.per_minute', await shiftsPerMinute(page, env.minutes(SAMPLES)));
    m('reading_load.max', max(words));
    m('reading_load.median', median(words));
    m('cue_churn.changes', changes);
    m('cue_churn.flip_flops', flips);
    m('contract.scroll', new Set(scroll).size);
    m('contract.hidden', new Set(vis).size);
    m('contract.axe_serious', axe.length);
    m('contract.truncated_hidden', max(trunc));
    // completion (hard gate 3): the end state is on screen when it is the truth, and only then
    const closedShown = (await page.locator('#now.m-closed').count()) > 0;
    const waitShown = (await page.locator(cue.card).count()) > 0;
    const runShown = (await page.locator('#now .nc.run').count()) > 0;
    const shown = {closed: closedShown, waiting: waitShown, running: runShown};
    const completionMiss = s.truth.completion && !shown[s.truth.completion] ? 1 : 0;
    const falseComplete = s.truth.completion !== 'closed' && closedShown ? 1 : 0;
    m('completion.miss', completionMiss);
    m('completion.false', falseComplete);
    if (gating(vp)) {
      expect.soft([...new Set(scroll)], 'hard gate 1: nothing scrolls but what the baseline allows').toEqual([]);
      expect.soft([...new Set(vis)], 'hard gate 2: always visible').toEqual([]);
      expect.soft(completionMiss + falseComplete, 'hard gate 3: completion').toBe(0);
      expect.soft(axe, 'hard gate 5: axe serious or critical').toEqual([]);
    }

    // ---- L3, second part: the scenario's tasks ---------------------------------------------------------
    if (s.truth.completion === 'closed') {
      expect.soft(await read(primary, s.truth.outcome), 'sees the outcome').toBe(true);
      const cost = await page.locator('#kpis .k-cost .k-v').innerText();
      expect.soft(cost, 'reads the total cost').toMatch(s.truth.cost);
      u.looks.push(2);
    }
    if (s.tasks.some((t) => /open the (diff of the )?last change/.test(t))) {
      // the newest change: the first row of What changed, else the in-flight step's newest file
      const first = page.locator('#feed .fd-row').first();
      const name = (await first.locator('.fd-s').innerText().catch(() => '')).trim();
      if (s.truth.lastChange?.text) expect.soft(name, 'the newest row of What changed is the last change').toContain(s.truth.lastChange.text);
      const a0 = u.actions;
      if (s.truth.lastChange?.cls) expect.soft(await first.getAttribute('class'), `the newest change is ${s.truth.lastChange.cls}`).toContain(`c-${s.truth.lastChange.cls}`);
      const ok = !!name && await use('button', name);
      expect.soft(ok, 'finds the last change as a control it can use').toBe(true);
      if (!ok) u.fixes.push(3);
      if (ok) {
        await expect.soft(page.getByRole('dialog')).toBeVisible();
        u.looks.push(1);
        u.fixes.push(u.actions - a0);
        await shot(page, info, `${s.name}-diff`);
        await page.keyboard.press('Escape');
      }
    }
    if (s.tasks.some((t) => /only the specification/.test(t))) {
      const a0 = u.actions;
      expect.soft(await use('button', /^Specification/), 'finds the Specification filter').toBe(true);
      expect.soft(await page.locator('#feed .fd-row:not(.c-spec)').count(), 'only specification rows remain').toBe(0);
      u.fixes.push(u.actions - a0);
      await use('button', /^Specification/);
    }
    if (s.write) {
      // the cue says what waits; its control opens the decision; one more action takes it
      expect.soft(await read(primary, s.truth.waiting.action), 'the cue names what waits').toBe(true);
      const a0 = u.actions;
      expect.soft(await use(cue.control.role, cue.control.name), 'the cue has a control').toBe(true);
      await page.waitForTimeout(300);
      const ok = await use('button', /^Approve t/);
      expect.soft(ok, 'the approve control is in view').toBe(true);
      // the act is taken and its result is on screen; a refusal by the driver is shown with its reason
      // (the person fixture keeps its final work/ revisions, so Canon may refuse: review-report T16)
      if (ok) {
        await expect.soft(page.locator('#toast.show')).toBeVisible({timeout: 60000});
        m('screen_only.decision_refused', (await page.locator('#toast.show.bad').count()) ? 1 : 0);
      }
      u.fixes.push(ok ? u.actions - a0 : 3);
      if (ok) await shot(page, info, `${s.name}-decided`);
    }
    m('screen_only.actions', u.actions);
    m('screen_only.looks', u.looks.reduce((a, b) => a + b, 0));
    m('screen_only.steers', u.steers);
    m('glance_cost.median', median(u.looks));
    m('glance_cost.p95', p95(u.looks));
    m('actions_per_fix.max', max(u.fixes));
    m('actions_per_fix.median', median(u.fixes));
    if (gating(vp)) expect.soft(max(u.fixes), 'hard gate 4: every fix in at most 2 actions').toBeLessThanOrEqual(2);
    await shot(page, info, `${s.name}-end`);
    expect.soft(errors, 'no page errors').toEqual([]);
    env.release();
    if (played) await played.done;
  });
}
