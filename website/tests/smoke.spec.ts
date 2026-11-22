import {test, expect, type Page} from '@playwright/test';
import {everyMode, seriousViolations} from './variants';
import fs from 'node:fs';
import path from 'node:path';

const dataDir = path.join(__dirname, '..', 'data', 'protocols');
const index = JSON.parse(fs.readFileSync(path.join(dataDir, 'index.json'), 'utf8'));
const protocols: Array<{stem: string; id: string; actions: number}> = index.protocols;

const PAGES = ['', 'docs/', 'docs/flow', 'docs/built-on', 'docs/run-it', 'protocols', 'evaluation', 'llm-is-not-the-runtime'];
const ROUTES = [...PAGES, ...protocols.map((p) => `protocols/${p.stem}`)];

const FLOW_BOX_MIN_WIDTH = 200;
const FLOW_BOX_MAX_HEIGHT = 260;

test.use({viewport: {width: 1440, height: 900}, colorScheme: 'light'});

for (const route of ROUTES) {
  test(`/${route}`, async ({page}) => {
    await everyMode(page, route, async (mode, errors) => {
      await expect(page.locator('h1').first()).toBeVisible();
      const scroll = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(scroll, 'horizontal page scroll in px').toBeLessThanOrEqual(0);
      const stem = route.startsWith('protocols/') ? route.slice('protocols/'.length) : null;
      if (stem) {
        const p = protocols.find((x) => x.stem === stem)!;
        await expect(page.locator('.sdc-action')).toHaveCount(p.actions);
        await expect(page.locator('.sdc-action').first()).toBeVisible();
      }
      if (route === 'docs/flow') await checkFlow(page, mode.viewport.width);
      if (route === 'evaluation') await checkEvaluation(page);
      if (route === 'llm-is-not-the-runtime') await checkLlm(page);
      expect(await seriousViolations(page), 'serious or critical axe violations').toEqual([]);
      expect(errors, 'console errors').toEqual([]);
    });
  });
}

type LedgerRun = {clean_pass: boolean; cost_usd: number | null};
type LedgerEntry = {
  kind: string;
  version: string | null;
  label: string;
  settings: Array<{setting: string; runs: LedgerRun[]}>;
};

const ledger: {entries: LedgerEntry[]} = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'docs', 'progress.json'), 'utf8'));

function versionKey(v: string): number[] {
  return v.split('.').map((p) => Number.parseInt(p, 10) || 0);
}

/** The releases with committed runs, in version order, as the evaluation page charts them. */
function releasesWithRuns(): LedgerEntry[] {
  return ledger.entries
    .filter((e) => e.kind === 'release' && e.settings.length > 0)
    .sort((a, b) => {
      const x = versionKey(a.version ?? a.label);
      const y = versionKey(b.version ?? b.label);
      for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0);
      return 0;
    });
}

/** The labels of the releases that have a run with a cost: the columns of the cost chart. */
function releasesWithCost(): string[] {
  return releasesWithRuns()
    .filter((e) => e.settings.some((c) => c.runs.some((r) => typeof r.cost_usd === 'number')))
    .map((e) => e.label);
}

/** The settings of the latest release with runs, in the order of the ledger's cells, with their runs. */
function latestRelease(): {label: string; settings: Array<{setting: string; runs: LedgerRun[]}>} {
  const e = releasesWithRuns().at(-1)!;
  const ids = [...new Set(e.settings.map((c) => c.setting))];
  return {label: e.label, settings: ids.map((id) => ({setting: id, runs: e.settings.filter((c) => c.setting === id).flatMap((c) => c.runs)}))};
}

function medianOf(values: number[]): number {
  const v = [...values].sort((a, b) => a - b);
  const m = v.length >> 1;
  return v.length % 2 ? v[m] : (v[m - 1] + v[m]) / 2;
}

async function checkFlow(page: Page, width: number): Promise<void> {
  const boxes = await page.locator('.sdc-flow:not(.sdc-flow--actions) .sdc-flow__box').evaluateAll((els) => els.map((e) => e.getBoundingClientRect()).map((r) => ({w: r.width, h: r.height})));
  expect(boxes.length).toBeGreaterThan(5);
  if (width >= 1440) {
    for (const b of boxes) {
      expect(b.w, 'flow box width').toBeGreaterThanOrEqual(FLOW_BOX_MIN_WIDTH);
      expect(b.h, 'flow box height').toBeLessThanOrEqual(FLOW_BOX_MAX_HEIGHT);
    }
  }
  const broken = await page.locator('.sdc-flow code, .sdc-flow__name').evaluateAll((els) => els.filter((e) => e.getClientRects().length > 1).map((e) => e.textContent));
  expect(broken, 'identifiers broken across lines').toEqual([]);
}

async function checkEvaluation(page: Page): Promise<void> {
  await expect(page.locator('#metric-clean-passes')).toBeVisible();
  expect(await page.locator('.sdc-metric').count()).toBeGreaterThanOrEqual(18);
  const conf = page.locator('#metric-conformance');
  await expect(conf.locator('svg')).toBeVisible();
  await expect(conf.locator('figcaption')).toContainText('lower bound');
  await expect(conf.locator('figcaption')).toContainText('Higher is better');
  await expect(conf.locator('.sdc-metric__missing').first()).toHaveText('not observed');
  const confLabel = (await conf.locator('svg').getAttribute('aria-label')) ?? '';
  expect(confLabel).toMatch(/0\.1\.0: All Haiku not observed; Sonnet work, Haiku decider median 32 %, min 13 %, max 79 %, n 3/);
  await expect(page.locator('#conformance')).toBeAttached();
  await expect(page.locator('#case-quality-0\\.1\\.0')).toBeAttached();
  await expect(page.locator('a[href$="/llm-is-not-the-runtime#quality"]')).toBeAttached();
  await expect(page.locator('a[href*="docs/conformance.md#L18-L19"]')).toBeAttached();
  const runs010 = page.locator('section[aria-labelledby="runs-0.1.0"]');
  await expect(runs010).toContainText('spec scenarios proven passed: 2 of 16 (lower bound)');
  await expect(runs010).toContainText('of which 8: precondition not in the specification');
  await expect(runs010).toContainText('not observed');
  for (const id of ['metric-cost', 'metric-wall', 'metric-llm', 'metric-decider', 'metric-loop-change-implement', 'metric-fpy-spec-author', 'metric-conformance']) {
    await expect(page.locator(`#${id}`)).toBeAttached();
  }
  const labels = await page.locator('#metric-cost .sdc-metric__release').allTextContents();
  expect(labels.slice(0, 2), 'the first two releases with runs do not change').toEqual(['0.1.0', '0.1.2']);
  expect(labels, 'the releases of docs/progress.json with a cost, in version order').toEqual(releasesWithCost());
  await expect(page.locator('.sdc-metric__nodata')).toHaveCount(0);
  await expect(page.locator('#no-runs')).toHaveText(/^No runs yet on: 0\.1\.1, 0\.1\.3, 0\.1\.4(, \d+\.\d+\.\d+)*\.$/);
  await expect(page.locator('#runs')).toBeAttached();
  const runHeads = await page.locator('section[aria-labelledby="runs"] > section > h3').allTextContents();
  expect(runHeads.map((h) => h.split(' ·')[0]).slice(-2), 'the first two releases with runs, newest last').toEqual(['Release 0.1.2', 'Release 0.1.0']);
  expect(runHeads.map((h) => h.split(' ·')[0]), 'newest release first').toEqual(releasesWithRuns().map((e) => `Release ${e.label}`).reverse());
  await expect(page.locator('#runs-none')).toHaveText(/^No runs yet on: 0\.1\.1, 0\.1\.3, 0\.1\.4(, \d+\.\d+\.\d+)*\.$/);

  const order = await page.evaluate(() =>
    ['#tiles', '#metric-clean-passes', '#metric-cost', '#more-metrics', '#stops', '#experiments', '#runs', '#method', '#limits'].map((s) => document.querySelector(s)!.getBoundingClientRect().top + window.scrollY),
  );
  expect(order, 'sections in the decided order').toEqual([...order].sort((a, b) => a - b));
  const tilesTop = order[0];
  expect(tilesTop, 'tiles in the first screen').toBeLessThan(page.viewportSize()!.height);
  const latest = latestRelease();
  await expect(page.locator('#tile-clean')).toContainText(`release ${latest.label}`);
  await expect(page.locator('#tile-clean li')).toHaveText(latest.settings.map((s) => new RegExp(`${s.setting}\\s*${s.runs.filter((r) => r.clean_pass).length} of ${s.runs.length}`)));
  for (const s of latest.settings) {
    const median = medianOf(s.runs.map((r) => r.cost_usd).filter((x): x is number => typeof x === 'number'));
    await expect(page.locator('#tile-cost li').filter({hasText: s.setting}), `median cost of ${s.setting}`).toContainText(`$${median.toFixed(2)}`);
  }
  await expect(page.locator('#tile-wall')).toContainText(`release ${latest.label}`);
  await expect(page.locator('#tile-target .sdc-tile__big')).toHaveText('$0.47');
  await expect(page.locator('#tile-target')).toContainText('to go');
  await expect(page.locator('#tile-target')).toContainText('free-agent-0.1.0');
  await expect(page.locator('#tile-quality')).toContainText('of 23');
  await expect(page.locator('#tile-quality')).toContainText('release 0.1.0');
  await expect(page.locator('#tile-conformance')).toContainText('lower bound');
  await expect(page.locator('#tile-conformance')).toContainText('release 0.1.0');
  const tileBoxes = await page.locator('.sdc-tile').evaluateAll((els) => els.map((e) => e.getBoundingClientRect().width));
  const vw = page.viewportSize()!.width;
  if (vw < 600) for (const w of tileBoxes) expect(w, 'two tiles per row at 390').toBeLessThan(vw / 2);

  await expect(page.locator('#metric-cost .sdc-metric__key--target')).toContainText('target $0.47 per run');
  await expect(page.locator('#metric-cost .sdc-metric__key--reference')).toContainText('free agent median $0.47 (free-agent-0.1.0, n = 3)');
  await expect(page.locator('#metric-cost .sdc-metric__ref--target .sdc-metric__reflabel')).toHaveText('target');
  await expect(page.locator('#metric-wall .sdc-metric__key--reference')).toContainText('free agent median 3 min');
  await expect(page.locator('#metric-wall .sdc-metric__ref--reference .sdc-metric__reflabel')).toHaveText('free agent');
  await expect(page.locator('#cost-split')).toContainText(`release ${latest.label}`);
  await expect(page.locator('#cost-split .sdc-split__row')).toHaveCount(3);
  await expect(page.locator('#cost-split tbody tr').filter({hasText: 'decider'})).toHaveCount(1);
  const split = (await page.locator('#cost-split svg[role="img"]').getAttribute('aria-label')) ?? '';
  const strongRuns = latest.settings.find((s) => s.setting === 'strong')!.runs;
  const strongMean = strongRuns.reduce((a, r) => a + r.cost_usd!, 0) / strongRuns.length;
  expect(split).toContain(`Sonnet work, Opus decider, n ${strongRuns.length}: $${strongMean.toFixed(2)}`);

  const exp = page.locator('section[aria-labelledby="experiments"] tbody tr');
  expect(await exp.count()).toBeGreaterThanOrEqual(5);
  await expect(exp.filter({hasText: 'free-agent-0.1.0'})).toContainText('contradicted');
  const pageStates = ['running', 'planned', 'ended'];
  const buildConstraints = exp.filter({hasText: 'build-constraints'});
  await expect(buildConstraints).toHaveCount(1);
  expect(pageStates, 'a state the page renders').toContain((await buildConstraints.locator('.sdc-state').innerText()).trim());
  await expect(page.locator('[id="exp-free-agent-0.1.0"]')).toHaveCount(1);
  for (const id of ['baseline-0.1.0', 'release-0.1.2', 'trial-3-attempts', 'undefined']) await expect(page.locator(`[id="exp-${id}"]`)).toHaveCount(0);
}

async function checkLlm(page: Page): Promise<void> {
  await expect(page.locator('#canon')).toBeAttached();
  const timeline = page.locator('.sdc-tl');
  const pending = page.getByText('Timeline pending');
  expect((await timeline.count()) > 0 || (await pending.count()) > 0).toBe(true);
  await expect(page.getByText('Runs pending')).toHaveCount(0);
  const lede = page.locator('#result');
  await expect(lede).toContainText('about 12 and 5 times cheaper');
  await expect(lede).toContainText('about 8 and 9 times faster');
  await expect(lede).toContainText('smaller product');
  await expect(lede).toContainText('contradicted');
  await expect(page.locator('.sdc-compare .sdc-tl')).toHaveCount(3);
  const medians = page.locator('#free-medians');
  for (const s of ['0.470 [0.414, 0.478] (n=3)', '5.503 [', '2.549 [', '413,432 [', '3,260,861 [', '3,046,058 [', '164.4 [', '1,247 [', '1,489 [', '13 [13, 13] (n=3)']) {
    await expect(medians).toContainText(s);
  }
  await expect(page.locator('#free-verdict tbody tr')).toHaveCount(4);
  await expect(page.locator('#free-verdict tbody td strong')).toHaveText(['contradicted', 'contradicted', 'contradicted', 'contradicted']);
  await expect(page.locator('#free-sizes')).toContainText('1,263.5');
  await expect(page.locator('#free-sizes tbody tr')).toHaveCount(11);
  await expect(page.locator('#free-conformance')).toContainText('spec scenarios proven passed: 6 of 8 (lower bound)');
  await expect(page.locator('#free-conformance')).toContainText('spec scenarios proven passed: 16 of 17 (lower bound)');
  await expect(page.locator('#free-records')).toContainText('5 in 3 of 3 runs');
  await expect(page.locator('.sdc-confounds li')).toHaveCount(9);
  await expect(page.getByText('Quality evaluation: pending')).toHaveCount(0);
  await expect(lede).toContainText('For about 5 and 12 times the cost, the SDC products scored 2 to 3 points more of 23 in a blinded browser evaluation, the difference being edit, remove and conflicting offline edits');
  const sides = page.locator('#quality-sides tbody tr');
  await expect(sides).toHaveCount(3);
  await expect(sides.nth(0)).toContainText('SDC strong');
  await expect(sides.nth(0)).toContainText('23 [20\u201323]');
  await expect(sides.nth(0)).toContainText('5.503 [5.432\u20136.194]');
  await expect(sides.nth(1)).toContainText('22 [20\u201323]');
  await expect(sides.nth(1)).toContainText('2.549 [2.497\u20133.883]');
  await expect(sides.nth(2)).toContainText('free agent');
  await expect(sides.nth(2)).toContainText('20 [19\u201320]');
  await expect(sides.nth(2)).toContainText('163 [143\u2013173]');
  await expect(page.locator('#quality-cost .sdc-quality__mark')).toHaveCount(9);
  expect(await page.locator('#quality-cost svg[role="img"]').getAttribute('aria-label')).toContain('free-agent-0.1.0-sonnet-3, free agent: 19 of 23');
  await expect(page.locator('#quality-items li')).toHaveCount(4);
  await expect(page.locator('#quality-items')).toContainText('F3 edit: passed in 2 of 3 (SDC strong), 2 of 3 (SDC mixed), 0 of 3 (free agent)');
  await expect(page.getByText('The other 19 items passed in all 9 products.')).toBeVisible();
  await expect(page.locator('#quality-defects li')).toHaveCount(4);
  await expect(page.getByText('4 defects, all in SDC products')).toBeVisible();
  await expect(page.getByText('Blinding did not hide the side')).toBeVisible();
}

test('every internal link resolves', async ({request, baseURL}) => {
  const base = new URL(baseURL!);
  const seen = new Set<string>();
  const queue = [...ROUTES];
  const failures: string[] = [];
  for (const route of queue) {
    const res = await request.get(route);
    expect(res.status(), `status of /${route}`).toBe(200);
    const html = await res.text();
    const hrefs = [...html.matchAll(/<a\s(?:[^>"']|"[^"]*"|'[^']*')*?\bhref=(?:"([^"]*)"|'([^']*)'|([^\s>"']+))/g)].map((m) => new URL((m[1] ?? m[2] ?? m[3]).replace(/&amp;/g, '&'), res.url()).href);
    for (const h of hrefs) {
      const u = new URL(h);
      if (u.origin !== base.origin || !u.pathname.startsWith(base.pathname)) continue;
      const key = u.pathname;
      if (seen.has(key)) continue;
      seen.add(key);
      const r = await request.get(u.origin + key);
      if (r.status() !== 200) failures.push(`${key} (${r.status()}) from /${route}`);
    }
  }
  expect(seen.size, 'distinct internal paths linked').toBeGreaterThanOrEqual(ROUTES.length);
  expect(failures).toEqual([]);
});
