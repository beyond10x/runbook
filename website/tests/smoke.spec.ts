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
  await expect(page.locator('a[href*="docs/conformance.md#L18-L19"]')).toBeAttached();
  const runs010 = page.locator('section[aria-labelledby="runs-0.1.0"]');
  await expect(runs010).toContainText('spec scenarios proven passed: 2 of 16 (lower bound)');
  await expect(runs010).toContainText('of which 8: precondition not in the specification');
  await expect(runs010).toContainText('not observed');
  for (const id of ['metric-cost', 'metric-wall', 'metric-llm', 'metric-decider', 'metric-loop-change-implement', 'metric-fpy-spec-author', 'metric-conformance']) {
    await expect(page.locator(`#${id}`)).toBeAttached();
  }
  const labels = await page.locator('#metric-cost .sdc-metric__release').allTextContents();
  expect(labels[0]).toBe('0.1.0');
  await expect(page.locator('#runs')).toBeAttached();
}

async function checkLlm(page: Page): Promise<void> {
  await expect(page.locator('#canon')).toBeAttached();
  const timeline = page.locator('.sdc-tl');
  const pending = page.getByText('Timeline pending');
  expect((await timeline.count()) > 0 || (await pending.count()) > 0).toBe(true);
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
