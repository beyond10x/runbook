import {test, expect} from '@playwright/test';

const NAVBAR = [
  {label: 'How it works', route: 'docs/'},
  {label: 'Protocols', route: 'protocols'},
  {label: 'Evaluation', route: 'evaluation'},
  {label: 'Run it', route: 'docs/run-it'},
];

const HOW_IT_WORKS = [
  {label: 'What Runbook is', route: 'docs/'},
  {label: 'The LLM is not the runtime', route: 'llm-is-not-the-runtime'},
  {label: 'The flow', route: 'docs/flow'},
  {label: 'Context', route: 'docs/context'},
  {label: 'Built on', route: 'docs/built-on'},
];

const DOCS_PAGES = [...HOW_IT_WORKS.filter((p) => p.route.startsWith('docs/')), {label: 'Run it', route: 'docs/run-it'}];

const NAVBAR_LINKS = 'nav.navbar .navbar__items:not(.navbar__items--right) a.navbar__link:not(.navbar__brand)';
const SIDEBAR_LINKS = 'aside .theme-doc-sidebar-menu .menu__link';

test.describe('1440 navigation', () => {
  test.use({viewport: {width: 1440, height: 900}});

  test('the navbar has four content links in order', async ({page}) => {
    await page.goto('', {waitUntil: 'networkidle'});
    await expect(page.locator(NAVBAR_LINKS)).toHaveText(NAVBAR.map((n) => n.label));
    await expect(page.locator('nav.navbar .navbar__items--right a[aria-label="GitHub repository"]')).toHaveCount(1);
  });

  test('each navbar link resolves with status 200', async ({page, baseURL}) => {
    await page.goto('', {waitUntil: 'networkidle'});
    const hrefs = await page.locator(NAVBAR_LINKS).evaluateAll((els) => els.map((e) => (e as HTMLAnchorElement).getAttribute('href')!));
    expect(hrefs).toHaveLength(NAVBAR.length);
    for (const href of hrefs) {
      const res = await page.request.get(new URL(href, baseURL!).toString());
      expect(res.status(), `status of ${href}`).toBe(200);
    }
  });

  for (const p of HOW_IT_WORKS) {
    test(`"How it works" is active on ${p.label}`, async ({page}) => {
      const res = await page.goto(p.route, {waitUntil: 'networkidle'});
      expect(res?.status()).toBe(200);
      const active = page.locator(`${NAVBAR_LINKS}.navbar__link--active`);
      await expect(active).toHaveCount(1);
      await expect(active).toHaveText('How it works');
    });
  }

  for (const [label, route] of [
    ['Protocols', 'protocols'],
    ['Protocols', 'protocols/sdc-story'],
    ['Evaluation', 'evaluation'],
    ['Run it', 'docs/run-it'],
  ]) {
    test(`"${label}" is the only active entry on /${route}`, async ({page}) => {
      await page.goto(route, {waitUntil: 'networkidle'});
      const active = page.locator(`${NAVBAR_LINKS}.navbar__link--active`);
      await expect(active).toHaveCount(1);
      await expect(active).toHaveText(label);
    });
  }

  for (const p of DOCS_PAGES) {
    test(`the sidebar lists the group in order on ${p.label}`, async ({page, baseURL}) => {
      const base = new URL(baseURL!).pathname;
      await page.goto(p.route, {waitUntil: 'networkidle'});
      await expect(page.locator(SIDEBAR_LINKS)).toHaveText(['How it works', ...HOW_IT_WORKS.map((h) => h.label), 'Run it']);
      const hrefs = await page.locator(`${SIDEBAR_LINKS}:not(.menu__link--sublist)`).evaluateAll((els) => els.map((e) => (e as HTMLAnchorElement).getAttribute('href')!));
      expect(hrefs.map((h) => h.slice(base.length).replace(/\/$/, ''))).toEqual([
        ...HOW_IT_WORKS.map((h) => h.route.replace(/\/$/, '')),
        'docs/run-it',
      ]);
    });
  }

  test('the LLM route has a way back into the group', async ({page}) => {
    await page.goto('llm-is-not-the-runtime', {waitUntil: 'networkidle'});
    const row = page.locator('main nav[aria-label="How it works"]');
    await expect(row).toBeVisible();
    await expect(row.locator('a')).toHaveText(['What Runbook is', 'The flow', 'Context', 'Built on']);
  });
});

for (const theme of ['light', 'dark']) {
  test.describe(`390 navigation, ${theme}`, () => {
    test.use({viewport: {width: 390, height: 844}, colorScheme: theme as 'light' | 'dark'});

    test('the menu opens and shows the four entries and the sidebar group', async ({page}) => {
      await page.addInitScript((t) => window.localStorage.setItem('theme', t), theme);
      await page.goto('docs/', {waitUntil: 'networkidle'});
      await page.locator('button.navbar__toggle').click();
      const menu = page.locator('.navbar-sidebar');
      await expect(menu).toBeVisible();
      const main = menu.locator('.navbar-sidebar__item.menu').first();
      await expect(main.locator('a.menu__link:not([aria-label])')).toHaveText(NAVBAR.map((n) => n.label));
      await expect(main.locator('a[aria-label="GitHub repository"]')).toHaveCount(1);
      await menu.locator('.navbar-sidebar__back').click();
      await expect(menu.locator('.navbar-sidebar__item.menu').nth(1).locator('a.menu__link')).toHaveText([
        'How it works',
        ...HOW_IT_WORKS.map((h) => h.label),
        'Run it',
      ]);
    });
  });
}
