import {test, expect} from '@playwright/test';
import {everyMode, seriousViolations, visit} from './variants';

const ROUTE = 'docs/context';
const HEADINGS = [
  'What one node sees',
  'How context passes from node to node',
  'What the program holds',
  'How you can change the context',
  'What you can inspect afterwards',
  'Why it is built this way',
];

test.use({viewport: {width: 1440, height: 900}, colorScheme: 'light'});

test('page, graphics, headings, scroll, axe', async ({page}) => {
  await everyMode(page, ROUTE, async (_mode, errors) => {
    await expect(page.locator('h1')).toHaveText('Context');
    const h2 = (await page.locator('article h2').allInnerTexts()).map((t) => t.replace(/[​\s]+$/, '').trim());
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

    expect(await seriousViolations(page), 'serious or critical axe violations').toEqual([]);
    expect(errors, 'console errors').toEqual([]);
  });
});

test.describe('context links', () => {
  test('every internal link on the page resolves', async ({page, request, baseURL}) => {
    await visit(page, ROUTE);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
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
