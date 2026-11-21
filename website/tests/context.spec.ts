import {test, expect, type Page} from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const ROUTE = 'docs/context';
const HEADINGS = [
  'What one node sees',
  'How context passes from node to node',
  'What the program holds',
  'How you can change the context',
  'What you can inspect afterwards',
  'Why it is built this way',
];
const MODES = [
  {name: '1440 light', viewport: {width: 1440, height: 900}, theme: 'light'},
  {name: '1440 dark', viewport: {width: 1440, height: 900}, theme: 'dark'},
  {name: '390 light', viewport: {width: 390, height: 844}, theme: 'light'},
  {name: '390 dark', viewport: {width: 390, height: 844}, theme: 'dark'},
] as const;

async function open(page: Page, theme: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
  const res = await page.goto(ROUTE, {waitUntil: 'networkidle'});
  expect(res?.status(), `status of ${ROUTE}`).toBe(200);
  await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
  return errors;
}

for (const mode of MODES) {
  test.describe(`context ${mode.name}`, () => {
    test.use({viewport: mode.viewport, colorScheme: mode.theme});

    test('page, graphics, headings, scroll, axe', async ({page}) => {
      const errors = await open(page, mode.theme);
      await expect(page.locator('h1')).toHaveText('Context');
      const h2 = (await page.locator('article h2').allInnerTexts()).map((t) => t.replace(/[\u200b\s]+$/, '').trim());
      for (const h of HEADINGS) expect(h2, 'h2 headings').toContain(h);

      const flow = page.getByTestId('context-flow');
      await expect(flow).toBeVisible();
      await expect(flow.locator('.sdc-ctx-card')).toHaveCount(2);
      await expect(flow.locator('.sdc-ctx-store')).toBeVisible();
      await expect(flow.locator('.sdc-ctx-not s').first()).toBeVisible();
      await expect(flow.locator('svg').first()).toBeVisible();
      const levers = page.getByTestId('context-levers');
      await expect(levers).toBeVisible();
      expect(await levers.locator('.sdc-ctx-layer').count()).toBeGreaterThanOrEqual(5);
      for (const owner of ['protocol', 'binding', 'earlier node', 'you']) await expect(levers.locator('.sdc-ctx-owner', {hasText: owner}).first()).toBeVisible();
      await expect(page.locator('.sdc-ctx-node')).toHaveCount(3);
      await expect(page.locator('.sdc-ctx-node .sdc-pred li').first()).toBeVisible();

      const scroll = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(scroll, 'horizontal page scroll in px').toBeLessThanOrEqual(0);
      const wide = await page.locator('.sdc-ctx-flow, .sdc-ctx-levers, .sdc-ctx-node').evaluateAll((els) =>
        els.filter((e) => e.getBoundingClientRect().right > document.documentElement.clientWidth + 1).length,
      );
      expect(wide, 'graphics wider than the viewport').toBe(0);

      const axe = await new AxeBuilder({page: page as never}).analyze();
      const bad = axe.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical');
      expect(bad.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).slice(0, 3).join(', ')}`), 'serious or critical axe violations').toEqual([]);
      expect(errors, 'console errors').toEqual([]);
    });
  });
}

test.describe('context links', () => {
  test.use({viewport: {width: 1440, height: 900}});

  test('every internal link on the page resolves', async ({page, request, baseURL}) => {
    await open(page, 'light');
    const origin = new URL(baseURL!).origin;
    const hrefs = await page.locator('main a[href], article a[href]').evaluateAll((els) => els.map((e) => (e as HTMLAnchorElement).href));
    const internal = [...new Set(hrefs.filter((h) => h.startsWith(origin)).map((h) => h.split('#')[0]))];
    expect(internal.length).toBeGreaterThan(3);
    for (const href of internal) {
      const res = await request.get(href);
      expect(res.status(), href).toBe(200);
    }
    const anchors = await page.locator('main a[href*="#action-"]').evaluateAll((els) => els.map((e) => (e as HTMLAnchorElement).href));
    for (const href of [...new Set(anchors)]) {
      await page.goto(href, {waitUntil: 'networkidle'});
      const id = decodeURIComponent(new URL(href).hash.slice(1));
      await expect(page.locator(`[id="${id}"]`), href).toHaveCount(1);
    }
  });
});
