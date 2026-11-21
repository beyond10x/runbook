// "What changed", the KPI strip and the collapsible right panel, against the `changes` fixture (every
// class of change, test/fixture-run.mjs) and the recorded trial-todo-1 run (no fs/changes.jsonl).
// Own server on port 7503 (wide) / 7504 (laptop), SDC_UI_CHANGES_PORT to move.
// Screenshots: screenshots/<project>-changes-*.png.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun} from './need-run.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const procs = [];
let base;

test.beforeAll(async ({}, info) => {
  needRun(path.resolve(process.env.SDC_UI_FIXTURE_SOURCE || path.join(ui, '../runs/trial-todo-1')), 'changes.spec.mjs (fixture source)');
  needRun(path.resolve(ui, '../runs/trial-todo-1/store'), 'changes.spec.mjs');
  const k = info.project.name === 'wide' ? 0 : 1;
  const port = Number(process.env.SDC_UI_CHANGES_PORT || 7503) + k;
  const dir = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), info.project.name);
  base = `http://127.0.0.1:${port}`;
  fs.mkdirSync(dir, {recursive: true});
  const run = (args) => { const p = spawn('node', args, {cwd: ui, stdio: 'ignore'}); procs.push(p); return p; };
  await new Promise((r) => run(['test/fixture-run.mjs', 'changes', path.join(dir, 'changes')]).on('exit', r));
  run(['server.mjs', '--port', String(port), path.join(dir, 'changes', 'store'), path.resolve(ui, '../runs/trial-todo-1/store')]);
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((r) => setTimeout(r, 100)); } }
});
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });
const shot = (page, info, name) => page.screenshot({path: path.join(shots, `${info.project.name}-changes-${name}.png`)});
const errorsOf = (page) => { const e = []; page.on('pageerror', (x) => e.push(String(x))); return e; };

test('what changed: every class with its meaning, filters, the record one click away', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=0');
  const feed = page.locator('#feed');
  await expect(feed.locator('.fd-row').first()).toBeVisible();
  for (const [cls, n] of [['planning', 3], ['spec', 2], ['direction', 4], ['code', 1]]) await expect(feed.locator(`.fchip.c-${cls} i`)).toHaveText(String(n));
  // the ADR, the decider's decision, and the approvals the store already holds
  expect(Number(await feed.locator('.fchip.c-decision i').textContent())).toBeGreaterThanOrEqual(2);
  await expect(feed.locator('.fd-row.c-spec .fd-s').first()).toHaveText('entity TodoItem: field `due` added');
  await expect(feed.locator('.fd-row.c-planning .fd-s', {hasText: 'draft → active'})).toHaveText('story storage: draft → active');
  await expect(feed.locator('.fd-row.c-direction .fd-s', {hasText: 'export'})).toHaveText('roadmap: epic export added');
  await expect(feed.locator('.fd-row.c-direction .fd-s', {hasText: 'q3'})).toHaveText('question q3 answered: “Kept on the server, in the daemon\'s data file” (by human)');
  await expect(feed.locator('.fd-row.c-decision .fd-s', {hasText: 'recorded'})).toHaveText('decision recorded: “Keep the items in the daemon\'s data file” (accepted)');
  await expect(feed.locator('.fd-row.unparsed .fd-s')).toContainText('broken.md created (not parsed');
  // the row says who caused it, in which case and action
  await expect(feed.locator('.fd-row.c-spec').first().locator('.fd-2')).toContainText('spec.author');
  await expect(feed.locator('.fd-row.c-spec').first().locator('.fd-w')).toHaveText('LLM agent · Edit');
  await shot(page, info, 'feed');
  // a filter chip keeps one class, and is remembered
  await feed.locator('.fchip.c-planning').click();
  await expect(feed.locator('.fd-row')).toHaveCount(3);
  await expect(feed.locator('.fd-row:not(.c-planning)')).toHaveCount(0);
  await page.reload();
  await expect(page.locator('#feed .fchip.c-planning')).toHaveClass(/on/);
  await page.locator('#feed .fchip.c-planning').click();
  await expect(page.locator('#feed .fd-row.c-code')).toHaveCount(1);
  // the meaning above the text diff
  await page.locator('#feed .fd-row.c-spec').first().click();
  const dlg = page.locator('#diffdlg');
  await expect(dlg.locator('.dmean li')).toHaveText(['entity TodoItem: field `due` added', 'entity TodoItem: state `Done` added', 'entity TodoItem: transition Stored → Done on CompleteItem added', 'entity TodoItem: terminal states Stored → Done', 'command AddItem: outcome `duplicate` added']);
  await expect(dlg.locator('.dl.add').first()).toBeVisible();
  await expect(dlg.locator('.dh-lang')).toHaveText('YAML');
  await shot(page, info, 'diff-spec');
  await dlg.locator('[data-fsclose]').click();
  // a decision opens its grounds
  await page.locator('#feed .fd-row.c-decision', {hasText: 'roadmap.approve: rejected'}).first().click();
  await expect(dlg.locator('.drow')).toContainText('roadmap.approve');
  await expect(dlg.locator('.dreasons li').first()).toContainText('roadmap');
  await shot(page, info, 'grounds');
  await dlg.locator('[data-fsclose]').click();
  // the Now band shows the in-flight step's changes by meaning too
  await expect(page.locator('#now .nc.run .fr-m .cb').first()).toHaveText('Spec');
  expect(errors).toEqual([]);
});

test('a run without fs/changes.jsonl: revisions, transitions and decisions from the store', async ({page}) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=1');
  const feed = page.locator('#feed');
  await expect(feed.locator('.fd-note')).toContainText('No fs/changes.jsonl');
  await expect(feed.locator('.fd-row.c-planning .fd-s', {hasText: 'story storage'})).toHaveText('story storage: draft → active');
  await expect(feed.locator('.fd-row.c-direction .fd-s', {hasText: 'roadmap'}).first()).toHaveText(/^roadmap (written|revised)$/);
  await expect(feed.locator('.fd-row.c-decision').first()).toBeVisible();
  expect(errors).toEqual([]);
});

test('KPI strip: five KPIs, "not recorded" where the run has none, each links to its Metrics row', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=0');
  const strip = page.locator('#kpis .kt');
  await expect(strip).toHaveCount(5);
  await expect(strip.locator('.k-l')).toHaveText([/^Elapsed/, /^Cost/, /^LLM calls/, /^Person decisions/, /^Progress/]);
  // the fixture has decisions.jsonl and no llm-calls.jsonl
  await expect(page.locator('#kpis .k-cost .k-p', {hasText: 'work'}).locator('.k-nr')).toHaveText('not recorded');
  await expect(page.locator('#kpis .k-cost .k-p', {hasText: 'decider'})).toContainText('$0.004');
  // elapsed ticks in place while the run is open
  const t = page.locator('#kpis .k-time .k-v [data-since]');
  const el = await t.elementHandle();
  const v1 = await t.textContent();
  await page.waitForTimeout(1600);
  expect(await t.textContent()).not.toBe(v1);
  expect(await el.evaluate((n) => n.isConnected)).toBe(true);
  await page.locator('#kpis .k-cost').click();
  await expect(page.locator('#tabs button[data-tab=metrics]')).toHaveClass(/on/);
  await expect(page.locator('#kpi-cost')).toBeInViewport();
  await shot(page, info, 'kpis-metrics');
  await page.goto(base + '/?source=1');
  await expect(page.locator('#kpis .k-cost .k-v')).toHaveText(/^\$\d+\.\d\d$/);
  await expect(page.locator('#kpis .k-progress .k-v')).toContainText('8/8');
  expect(errors).toEqual([]);
});

test('right panel: a rail until a tab is opened, toggled, remembered, opened by a node', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=1');
  await page.evaluate(() => localStorage.removeItem('sdc-ui.panel'));
  await page.reload();
  await expect(page.locator('#panel')).toBeHidden();
  const right = await page.locator('#right').boundingBox();
  expect(right.width).toBeLessThan(50);
  const g0 = (await page.locator('#graph').boundingBox()).width;
  await shot(page, info, 'rail');
  await page.locator('#tabs button[data-tab=changes]').click();
  await expect(page.locator('#panel')).toBeVisible();
  expect((await page.locator('#graph').boundingBox()).width).toBeLessThan(g0 - 250);
  await page.locator('#tabs button[data-tab=changes]').click();
  await expect(page.locator('#panel')).toBeHidden();
  await page.locator('#tabs button[data-tab=autonomy]').click();
  await page.reload();
  // no jump: the panel is open from the first paint
  await expect(page.locator('html')).toHaveClass(/rp-open/);
  await expect(page.locator('#tabs button[data-tab=autonomy]')).toHaveClass(/on/);
  await page.locator('#rpclose').click();
  await expect(page.locator('#panel')).toBeHidden();
  await page.locator('#graph [data-id^="claim:"]').first().click();
  await expect(page.locator('#panel')).toBeVisible();
  await expect(page.locator('#tabs button[data-tab=explain]')).toHaveClass(/on/);
  await page.locator('#rpclose').click();
  expect(errors).toEqual([]);
});
