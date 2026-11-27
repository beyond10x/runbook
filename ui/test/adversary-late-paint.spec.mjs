// Adversary case on the late-paint case of security-start-token-places.spec.mjs (story
// the-ui-test-step-stays-under-its-budget). A mutant of daemon/start.js is served in place of the real one (page.route):
// when the list of runbooks answers after the form was closed, the form opens again 150 ms later. The check of the
// late-paint case (1 s of the page's time after the answer, then the form closed) must see it open.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon, TOKEN} from './page-daemon.mjs';
import {installPageTime, spendPageTime} from './page-time.mjs';

const expect = plain.configure({timeout: 15000});
const RUNBOOKS = '**/daemon/views/runbooks';
let s;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('token');
});

const END = "$('st-intake').focus();\n}\n\nfunction build";

test('a form that a late answer opens again 150 ms after it fails the check of the late-paint case', async ({page}) => {
  await page.route('**/start.js', async (route) => {
    const r = await route.fetch();
    const text = await r.text();
    if (!text.includes(END)) throw new Error('mutant not applied');
    const body = text.replace(END, () => "if (!dlg.open) setTimeout(() => dlg.showModal(), 150);\n  $('st-intake').focus();\n}\n\nfunction build");
    await route.fulfill({response: r, body});
  });
  await installPageTime(page);
  await page.goto(`${s.base}/`);
  await page.locator('input[name=token]').fill(TOKEN);
  await page.getByRole('button', {name: 'Open'}).click();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  let release;
  const held = new Promise((r) => { release = r; });
  let asked;
  const reading = new Promise((r) => { asked = r; });
  await page.route(RUNBOOKS, async (route) => { asked(); await held; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await reading;
  await page.click('#st-close');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  const answered = page.waitForEvent('requestfinished', (r) => new URL(r.url()).pathname === '/daemon/views/runbooks');
  release();
  await answered;
  // the check of the late-paint case, which must see the form open here
  await spendPageTime(page, 1000);
  expect(await page.locator('#startdlg').evaluate((d) => d.open), 'the mutant opened the form again').toBe(true);
});
