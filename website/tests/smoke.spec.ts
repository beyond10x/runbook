import {test, expect, type Page} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import fs from 'node:fs';
import path from 'node:path';

const dataDir = path.join(__dirname, '..', 'data', 'protocols');
const index = JSON.parse(fs.readFileSync(path.join(dataDir, 'index.json'), 'utf8'));
const protocols: Array<{stem: string; id: string; actions: number}> = index.protocols;

const PAGES = ['', 'docs/', 'docs/flow', 'docs/built-on', 'docs/run-it', 'protocols', 'evaluation', 'llm-is-not-the-runtime'];
const ROUTES = [...PAGES, ...protocols.map((p) => `protocols/${p.stem}`)];
const MODES = [
  {name: '1440 light', viewport: {width: 1440, height: 900}, theme: 'light'},
  {name: '1440 dark', viewport: {width: 1440, height: 900}, theme: 'dark'},
  {name: '390 light', viewport: {width: 390, height: 844}, theme: 'light'},
  {name: '390 dark', viewport: {width: 390, height: 844}, theme: 'dark'},
] as const;

const FLOW_BOX_MIN_WIDTH = 200;
const FLOW_BOX_MAX_HEIGHT = 260;

async function open(page: Page, route: string, theme: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
  const res = await page.goto(route, {waitUntil: 'networkidle'});
  expect(res?.status(), `status of ${route}`).toBe(200);
  await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
  return errors;
}

for (const mode of MODES) {
  test.describe(mode.name, () => {
    test.use({viewport: mode.viewport, colorScheme: mode.theme});
    for (const route of ROUTES) {
      test(`/${route}`, async ({page}) => {
        const errors = await open(page, route, mode.theme);
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
        const axe = await new AxeBuilder({page: page as never}).analyze();
        const bad = axe.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical');
        expect(bad.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).slice(0, 3).join(', ')}`), 'serious or critical axe violations').toEqual([]);
        expect(errors, 'console errors').toEqual([]);
      });
    }
  });
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
  expect(labels).toEqual(['0.1.0', '0.1.2']);
  await expect(page.locator('.sdc-metric__nodata')).toHaveCount(0);
  await expect(page.locator('#no-runs')).toHaveText(/^No runs yet on: 0\.1\.1, 0\.1\.3, 0\.1\.4(, \d+\.\d+\.\d+)*\.$/);
  await expect(page.locator('#runs')).toBeAttached();
  const runHeads = await page.locator('section[aria-labelledby="runs"] > section > h3').allTextContents();
  expect(runHeads.map((h) => h.split(' ·')[0])).toEqual(['Release 0.1.2', 'Release 0.1.0']);
  await expect(page.locator('#runs-none')).toHaveText(/^No runs yet on: 0\.1\.1, 0\.1\.3, 0\.1\.4(, \d+\.\d+\.\d+)*\.$/);

  const order = await page.evaluate(() =>
    ['#tiles', '#metric-clean-passes', '#metric-cost', '#more-metrics', '#stops', '#experiments', '#runs', '#method', '#limits'].map((s) => document.querySelector(s)!.getBoundingClientRect().top + window.scrollY),
  );
  expect(order, 'sections in the decided order').toEqual([...order].sort((a, b) => a - b));
  const tilesTop = order[0];
  expect(tilesTop, 'tiles in the first screen').toBeLessThan(page.viewportSize()!.height);
  await expect(page.locator('#tile-clean')).toContainText('release 0.1.2');
  await expect(page.locator('#tile-clean li')).toHaveText([/haiku\s*2 of 3/, /mixed\s*2 of 3/, /strong\s*3 of 3/]);
  await expect(page.locator('#tile-cost')).toContainText('$4.70');
  await expect(page.locator('#tile-wall')).toContainText('release 0.1.2');
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
  await expect(page.locator('#cost-split')).toContainText('release 0.1.2');
  await expect(page.locator('#cost-split .sdc-split__row')).toHaveCount(3);
  await expect(page.locator('#cost-split tbody tr').filter({hasText: 'decider'})).toHaveCount(1);
  const split = (await page.locator('#cost-split svg[role="img"]').getAttribute('aria-label')) ?? '';
  expect(split).toContain('Sonnet work, Opus decider, n 3: $4.66');

  const exp = page.locator('section[aria-labelledby="experiments"] tbody tr');
  expect(await exp.count()).toBeGreaterThanOrEqual(5);
  await expect(exp.filter({hasText: 'free-agent-0.1.0'})).toContainText('contradicted');
  await expect(exp.filter({hasText: 'build-constraints'})).toContainText('planned');
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

test('every internal link resolves', async ({page, request, baseURL}) => {
  const base = new URL(baseURL!);
  const seen = new Set<string>();
  const queue = [...ROUTES];
  const failures: string[] = [];
  for (const route of queue) {
    await page.goto(route, {waitUntil: 'networkidle'});
    const hrefs = await page.locator('a[href]').evaluateAll((as) => as.map((a) => (a as HTMLAnchorElement).href));
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
