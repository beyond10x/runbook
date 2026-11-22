// The layout contract at phone sizes (decision D-006, ux/adapter.mjs `phone`): at every key moment of
// every hermetic scenario and of the real recorded runs, the measured facts of ux/core/phone.mjs. At
// the gating phone size (390x844) each is a hard gate; at the measured-only sizes it is reported.
// Metrics go to ux/out/metrics-<viewport>.jsonl (keys phone.*), the facts to
// ux/out/phone-facts-<viewport>.jsonl and, in a round, to ux/rounds/<R>/shots/phone-facts.jsonl (git-ignored) with the shots.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {UI, scenarios, phone, isPhone, viewports} from '../adapter.mjs';
import {scrollOffenders, axeViolations, truncatedWithoutValue} from '../core/contract.mjs';
import {horizontalOverflow, smallTargets, smallText, coveredControls, alwaysVisibleAcross, withinOneScroll, sheetProblems} from '../core/phone.mjs';
import {record} from '../core/metrics.mjs';
import {missingRun} from '../../test/need-run.mjs';
import {start, serverUp, OUT, SHOTS, ROUND, gating} from './env.mjs';

let env, rec;
test.describe.configure({mode: 'default'});
test.beforeAll(async ({}, info) => {
  if (!isPhone(info.project.name)) return;
  env = await start(info.project.name, 'phone');
  // the recorded runs: one read-only server of their own (a free port; SDC_UI_UX_PORT + 80 pins it)
  const runs = phone.runs.filter((r) => !missingRun(r.dir));
  if (runs.length) {
    const all = [...viewports.gating, ...viewports.measured];
    const port = process.env.SDC_UI_UX_PORT ? Number(process.env.SDC_UI_UX_PORT) + 80 + all.findIndex((v) => v.name === info.project.name) : 0;
    const srv = spawn('node', ['server.mjs', '--port', String(port), ...runs.map((r) => path.join(r.dir, 'store'))], {cwd: UI, stdio: ['ignore', 'pipe', 'pipe']});
    const base = await serverUp(srv, `${info.project.name} (recorded runs)`, {clock: false});
    rec = {base, srv, source: (name) => runs.findIndex((r) => r.name === name)};
  }
});
test.afterAll(() => { env?.stop(); if (rec?.srv?.exitCode == null) rec?.srv?.kill(); });

const home = (s) => s.replace(/\/home\/[^/\s"'<>\\]+/g, '~');
function facts(vp, scenario, moment, f) {
  const line = JSON.stringify({viewport: vp, scenario, moment, ...f}) + '\n';
  fs.mkdirSync(OUT, {recursive: true});
  fs.appendFileSync(path.join(OUT, `phone-facts-${vp}.jsonl`), home(line));
  if (SHOTS) { fs.mkdirSync(SHOTS, {recursive: true}); fs.appendFileSync(path.join(SHOTS, `${PHASE}phone-facts.jsonl`), home(line)); }
}
// UX_PHASE=before: the round's measurement before its change set (shots before-*, before-phone-facts.jsonl)
const PHASE = process.env.UX_PHASE ? `${process.env.UX_PHASE}-` : '';
const shot = async (page, vp, name) => {
  if (!SHOTS) return;
  fs.mkdirSync(SHOTS, {recursive: true});
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({path: path.join(SHOTS, `${PHASE}${vp}-${name}.png`)});
};
const cut = (xs, n = 25) => xs.slice(0, n);

// One moment: measure, record, shoot, and at the gating size assert.
async function moment(page, vp, scenario, name, {sheet = null} = {}) {
  await page.waitForTimeout(400);
  await page.evaluate(() => window.scrollTo(0, 0));
  const m = (k, v) => record(OUT, vp, scenario, k, v);
  const ho = await horizontalOverflow(page);
  const targets = await smallTargets(page, phone.targets, phone.target);
  const text = await smallText(page, phone.text);
  const covered = sheet ? [] : await coveredControls(page, phone.targets);
  const trunc = await truncatedWithoutValue(page);
  const scroll = (await scrollOffenders(page, phone.scrollAllowed)).filter((x) => !(x.sel === 'document' && x.w <= page.viewportSize().width + 1));
  const hidden = sheet ? [] : await alwaysVisibleAcross(page, phone.alwaysVisible);
  const reach = name === 'truth' ? await withinOneScroll(page, phone.reach) : null;
  const sheetP = sheet ? await sheetProblems(page, phone.sheets[sheet].dialog, phone.sheets[sheet].close) : [];
  const axe = name === 'truth' || sheet ? await axeViolations(page) : [];
  let graph = null;
  if (name === 'graph') {
    graph = await page.evaluate((sel) => {
      const g = document.querySelector(sel);
      if (!g) return {problem: 'no graph'};
      g.scrollIntoView({block: 'start'});
      const r = g.getBoundingClientRect();
      const cs = getComputedStyle(g);
      const wider = g.scrollWidth > g.clientWidth + 1;
      const scrolls = /(auto|scroll)/.test(cs.overflowX);
      const hint = [...document.querySelectorAll('[data-ux=hint]')].some((h) => h.getClientRects().length && h.getBoundingClientRect().top < innerHeight && h.getBoundingClientRect().bottom > 0);
      let cutBy = null;
      for (let a = g.parentElement; a && a !== document.body; a = a.parentElement) {
        const s = getComputedStyle(a);
        if (s.overflowX === 'visible' && s.overflowY === 'visible') continue;
        const q = a.getBoundingClientRect();
        if (r.right > q.right + 1 || r.left < q.left - 1 || r.bottom > q.bottom + 1) { cutBy = a.id || a.tagName.toLowerCase(); break; }
      }
      const problem = cutBy ? `the graph box is cut by #${cutBy}` : r.right > innerWidth + 1 ? 'the graph box runs past the screen' : wider && !scrolls ? 'the graph is clipped' : wider && !hint ? 'the graph scrolls sideways with no visible hint' : r.height < 120 ? `the graph box is ${Math.round(r.height)} px high` : null;
      return {problem, box: [Math.round(r.left), Math.round(r.top), Math.round(r.width), Math.round(r.height)], content: [g.scrollWidth, g.scrollHeight]};
    }, phone.graph);
    await page.waitForTimeout(200);
    if (SHOTS) await page.screenshot({path: path.join(SHOTS, `${PHASE}${vp}-${scenario}-graph.png`)});
  }
  m('phone.hscroll_px', ho.doc);
  m('phone.wide', ho.wide.length);
  m('phone.small_targets', targets.length);
  m('phone.small_text', text.filter((x) => x.kind === 'label').length);
  m('phone.small_body_text', text.filter((x) => x.kind === 'body').length);
  m('phone.covered', covered.length);
  m('phone.truncated_hidden', trunc.length);
  m('phone.scroll', new Set(scroll.map((x) => x.sel)).size);
  m('phone.hidden', new Set(hidden.map((x) => `${x.key}@${x.at}`)).size);
  if (reach) m('phone.kpi_reach', reach.reach);
  if (sheet) m(`phone.sheet_${sheet}`, sheetP.length);
  if (name === 'truth' || sheet) m('phone.axe_serious', axe.length);
  if (graph) m('phone.graph', graph.problem ? 1 : 0);
  facts(vp, scenario, name, {hscroll_px: ho.doc, wide: cut(ho.wide), small_targets: targets.length, targets: cut(targets), small_text: text.length, text: cut(text), covered: cut(covered), truncated: cut(trunc), scroll, hidden, reach, sheet: sheet ? {name: sheet, problems: sheetP} : null, axe, graph});
  if (name !== 'graph') await shot(page, vp, `${scenario}-${name}`);
  if (!gating(vp)) return;
  const at = `${scenario} ${name}`;
  expect.soft(ho.doc, `phone contract (${at}): no horizontal page scroll`).toBe(0);
  expect.soft(ho.wide, `phone contract (${at}): nothing runs past the screen`).toEqual([]);
  expect.soft(hidden, `phone contract (${at}): state, current step and decisions always visible`).toEqual([]);
  expect.soft(targets, `phone contract (${at}): tap targets at least ${phone.target}x${phone.target}`).toEqual([]);
  expect.soft(covered, `phone contract (${at}): no control covered`).toEqual([]);
  expect.soft(text, `phone contract (${at}): no label under ${phone.text.min} px, no body text under ${phone.text.body} px`).toEqual([]);
  expect.soft(trunc, `phone contract (${at}): no text cut off without its full value`).toEqual([]);
  expect.soft(scroll, `phone contract (${at}): only the allowed boxes scroll`).toEqual([]);
  if (reach) expect.soft(reach.reach, `phone contract (${at}): the KPI strip within one scroll screen`).toBe(0);
  if (sheet) expect.soft(sheetP, `phone contract (${at}): the ${sheet} is a full-screen sheet with its close control in view`).toEqual([]);
  if (name === 'truth' || sheet) expect.soft(axe, `phone contract (${at}): axe serious or critical`).toEqual([]);
  if (graph) expect.soft(graph.problem, `phone contract (${at}): the graph is a list or scrolls in its own box with a hint`).toBe(null);
}

async function openSheet(page, which) {
  if (which === 'drawer') {
    const b = page.locator('#decbtn');
    await b.scrollIntoViewIfNeeded();
    await b.click();
    await expect(page.locator('body.drawer-open')).toBeVisible();
  } else if (which === 'picker') {
    await phone.sheets.picker.open(page);
    await expect(page.locator('#pk-list [role=option]').first()).toBeVisible();
  } else if (which === 'rail') {
    await phone.sheets.rail.open(page);
    await expect(page.locator('html.rp-open')).toHaveCount(1);
  }
  await page.waitForTimeout(450);
}
async function closeSheet(page, which) {
  if (which === 'diff') await page.keyboard.press('Escape');
  else if (which === 'drawer') { await page.locator('#drclose').click(); }
  else if (which === 'rail') { const x = page.locator('#rpclose'); if (await x.isVisible()) await x.click(); }
  else if (which === 'picker') await page.locator('#pk-close').click();
  await page.waitForTimeout(300);
}

for (const s of scenarios) {
  test(`${s.name}: the phone contract at every key moment`, async ({page}, info) => {
    const vp = info.project.name;
    test.skip(!isPhone(vp), 'phone sizes only');
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    // on the shared clock (decision D-005): the run stands still while a moment is measured
    const played = await env.play(s);
    await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
    await env.prepare(page);
    await page.goto(`${env.base}/?source=${env.source(s)}`);
    await expect(page.locator('#now .now-h')).toBeVisible({timeout: 20000});
    await env.sync(page);
    const truthOn = async () => {
      if (s.truth.running.length) return (await page.locator('#now .nc.run').count()) >= s.truth.running.length;
      if (s.truth.waiting) return (await page.locator('#now .nc.wait').count()) > 0 && (!s.truth.waiting.who || (await page.locator('#now').innerText()).includes(s.truth.waiting.who));
      if (s.truth.completion === 'closed') return (await page.locator('#now.m-closed').count()) > 0;
      return true;
    };
    for (let i = 0; i < 40 && !(await truthOn()); i++) await env.step(page);
    await moment(page, vp, s.name, 'truth');
    if (s.decide) { await openSheet(page, 'drawer'); await moment(page, vp, s.name, 'drawer', {sheet: 'drawer'}); await closeSheet(page, 'drawer'); }
    // the run picker as a full-screen sheet (D-009), on the recorded run of the closed scenario
    if (s.name === 'closed') { await openSheet(page, 'picker'); await moment(page, vp, s.name, 'picker', {sheet: 'picker'}); await closeSheet(page, 'picker'); }
    if (s.tasks.some((t) => /open the (diff of the )?last change/.test(t))) {
      const first = page.locator('#feed .fd-row button.fd-s').first();
      await first.scrollIntoViewIfNeeded();
      await first.click();
      await expect(page.locator('#diffdlg[open]')).toHaveCount(1);
      await moment(page, vp, s.name, 'diff', {sheet: 'diff'});
      await closeSheet(page, 'diff');
    }
    if (s.anchor && !s.decide) {
      await openSheet(page, 'rail'); await moment(page, vp, s.name, 'rail', {sheet: 'rail'}); await closeSheet(page, 'rail');
      await moment(page, vp, s.name, 'graph');
    }
    expect.soft(errors, 'no page errors').toEqual([]);
    env.release();
    if (played) await played.done;
  });
}

for (const r of phone.runs) {
  test(`recorded ${r.name}: the phone contract at every key moment`, async ({page}, info) => {
    const vp = info.project.name;
    test.skip(!isPhone(vp), 'phone sizes only');
    const miss = missingRun(r.dir);
    test.skip(!!miss, `recorded ${r.name}: ${miss}`);
    await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
    await page.goto(`${rec.base}/?source=${rec.source(r.name)}`);
    await expect(page.locator('#now .now-h')).toBeVisible({timeout: 30000});
    await expect(page.locator('#kpis .kt').first()).toBeVisible({timeout: 30000});
    for (const mo of r.moments) {
      if (mo === 'truth' || mo === 'graph') { await moment(page, vp, r.name, mo); continue; }
      await openSheet(page, mo);
      await moment(page, vp, r.name, mo, {sheet: mo});
      await closeSheet(page, mo);
    }
  });
}
