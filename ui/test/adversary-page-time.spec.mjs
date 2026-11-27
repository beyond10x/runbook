// Adversary cases on the page's own time (story the-ui-test-step-stays-under-its-budget, test/page-time.mjs): what the
// cases that spend the page's time still catch. A mutant of web/app.js is served in place of the real one (page.route).
// The copy word that goes back after 9 s, not 2 s: the check of daemon-live.spec.mjs (2 s of the page's time, then the
// word within 500 ms) fails on it. A read chained on an answer: spent page time counts it as the host's clock would.
import {test, expect as plain} from '@playwright/test';
import {serve, LIVE} from './daemon-live-fixture.mjs';
import {installPageTime, spendPageTime} from './page-time.mjs';

const expect = plain.configure({timeout: 10000});
const ZONE = {timezoneId: 'Europe/Berlin', locale: 'en-US'};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own pages: one project is enough'));

test('the copy word that goes back after 9 s of the page\'s time fails the check of the copy-word case', async ({browser}) => {
  const own = await serve({hold: 1});
  const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write']});
  try {
    const page = await ctx.newPage();
    await page.route('**/app.js', async (route) => {
      const r = await route.fetch();
      const body = (await r.text()).replace('const COPY_MS = 2000;', 'const COPY_MS = 9000;');
      if (!body.includes('const COPY_MS = 9000;')) throw new Error('mutant not applied');
      await route.fulfill({response: r, body});
    });
    await installPageTime(page);
    await fetch(`${own.base}/__fixture/anchor?ago=132000`);
    await page.goto(`${own.base}/?run=${LIVE}`);
    const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
    await copy.click();
    await expect(copy).toHaveText('Copied');
    // the check of daemon-live.spec.mjs, which must fail here
    await spendPageTime(page, 2000);
    const caught = await plain(copy).toHaveText('Copy', {timeout: 500}).then(() => false, () => true);
    expect(caught, 'the word was back after 2 s of the page\'s time with COPY_MS 9000').toBe(true);
  } finally {
    await ctx.close();
    own.close();
  }
});

test('a read chained on an answer by a 200 ms timer is asked 25 times or more in 5 s of the page\'s time', async ({page}) => {
  await page.route('http://adv.test/', (r) => r.fulfill({contentType: 'text/html', body: '<!doctype html><title>t</title><script>window.go = async () => { for (;;) { try { await fetch("/x"); } catch {} await new Promise((ok) => setTimeout(ok, 200)); } };</script>'}));
  await page.route('http://adv.test/x', (r) => r.fulfill({status: 403, json: {refused: 'forbidden'}}));
  await installPageTime(page);
  await page.goto('http://adv.test/');
  const seen = [];
  page.on('request', (r) => { if (r.url().endsWith('/x')) seen.push(r); });
  await page.evaluate(() => { window.go(); });
  await expect.poll(() => seen.length).toBeGreaterThan(0);
  const before = seen.length;
  await spendPageTime(page, 5000);
  // 5000 / 200 on the host's clock with an answer that takes no time; before the page counted its own fetches the
  // stretch took 21 or 22 (an answer reached the page a step late)
  expect(seen.length - before, 'reads in 5 s of the page\'s time').toBeGreaterThanOrEqual(25);
});
