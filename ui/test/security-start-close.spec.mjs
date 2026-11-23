// Security review, pass 2, of the unit daemon-page-starts-a-run: the token field of the start form when the
// form closes by another way than its own buttons, and tokens that a header cannot carry.
// The shared daemon of page-daemon.mjs, with a token (stub runtime, floor linux) on 127.0.0.1:0; each test counts the runs
// it adds. Invented texts; no model call.
// Its state directory is page-daemon.mjs's, one per worker.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon, TOKEN} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const TYPED = 'typed-and-not-sent-0123456789-0123456789';
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`, {headers: {authorization: `Bearer ${TOKEN}`}}).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const inputs = (page) => page.evaluate(() => [...document.querySelectorAll('input, textarea')].map((e) => e.value));

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('token');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});

async function enter(page) {
  await page.goto(`${s.base}/`);
  await page.locator('input[name=token]').fill(TOKEN);
  await page.getByRole('button', {name: 'Open'}).click();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-runbook').selectOption('sdc');
  await expect(page.locator('#st-token')).toBeVisible();
}

test('a token typed and not sent leaves the field however the form closes: by script, by Escape twice, and before the form is shown again', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(r); });
  await enter(page);
  const held = async (how) => expect((await inputs(page)).filter((v) => v.includes(TYPED)), `after ${how} an input of the page holds the typed token`).toEqual([]);

  // closed by script, with and without a return value
  for (const [how, close] of [['close()', (d) => d.close()], ['close("x")', (d) => d.close('x')], ['requestClose()', (d) => (d.requestClose ? d.requestClose() : d.close())]]) {
    await form(page);
    await page.locator('#st-token').fill(TYPED);
    await page.locator('#startdlg').evaluate(close);
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    await expect.poll(() => inputs(page).then((all) => all.filter((v) => v.includes(TYPED)).length), {message: `after ${how} an input of the page holds the typed token`}).toBe(0);
  }

  // Escape, and Escape again on the next opening without a click in between
  await form(page);
  await page.locator('#st-token').fill(TYPED);
  await page.keyboard.press('Escape');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await held('Escape');
  await page.locator('#startdlg').evaluate((d) => { d.showModal(); document.getElementById('st-token').value = 'typed-and-not-sent-0123456789-0123456789'; });
  await page.keyboard.press('Escape');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await expect.poll(() => inputs(page).then((all) => all.filter((v) => v.includes(TYPED)).length), {message: 'after a second Escape an input of the page holds the typed token'}).toBe(0);

  // the attribute open taken away by script: no close event comes; the next opening shows an empty field
  await form(page);
  await page.locator('#st-token').fill(TYPED);
  await page.locator('#startdlg').evaluate((d) => d.removeAttribute('open'));
  const left = (await inputs(page)).filter((v) => v.includes(TYPED)).length;
  test.info().annotations.push({type: 'measured', description: `after removeAttribute('open') the field holds the typed token: ${left === 1}`});
  console.log(`[security-start-close] after removeAttribute('open') the field holds the typed token: ${left === 1}`);

  // a reload forgets everything
  await page.reload();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await held('a reload');
  await form(page);
  await held('a reload and a new opening');
  expect(sent.length).toBe(0);
  expect(await added()).toBe(0);
});

test('tokens that a header cannot carry are forgotten without a request and without their text; a Latin-1 token is sent and refused', async ({page}) => {
  const sent = [];
  const consoleLines = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(r); });
  page.on('console', (m) => consoleLines.push(m.text()));
  page.on('pageerror', (e) => consoleLines.push(e.message));
  await enter(page);
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake for tokens that cannot be sent');
  const odd = ['secret-Ā-0123456789-0123456789-0123456789', 'secret- -0123456789-0123456789-0123456789', 'secret-😀-0123456789-0123456789-01234567', 'secret-\ud800-0123456789-0123456789-0123456789', '€'.repeat(40)];
  for (const token of odd) {
    await expect(page.locator('#st-token')).toBeVisible();
    await page.locator('#st-token').evaluate((el, v) => { el.value = v; }, token);
    await page.click('#st-start');
    await expect(page.locator('#st-msg')).toContainText('cannot be sent');
    const said = await page.locator('#st-msg').textContent();
    expect(said.includes(token) || said.includes('secret-'), 'the message repeats the token').toBe(false);
    expect((await inputs(page)).filter((v) => v.includes(token))).toEqual([]);
    await expect(page.locator('#st-token')).toBeVisible();
    await expect(page.locator('#st-start')).toBeEnabled();
  }
  expect(sent.length, 'a request left the page').toBe(0);

  // a token of Latin-1 characters can be sent: the daemon refuses it and the page forgets it
  const latin = 'secret-éüÿ-0123456789-0123456789-0123456789';
  await page.locator('#st-token').fill(latin);
  const [response] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(response.status()).toBe(401);
  await expect(page.locator('#st-token')).toBeVisible();
  await expect(page.locator('#st-token')).toHaveValue('');
  const said = await page.locator('#st-msg').textContent();
  expect(said.includes('secret-')).toBe(false);
  expect(s.log().includes('secret-'), 'the output of the daemon carries a token').toBe(false);
  expect(consoleLines.filter((l) => l.includes('secret-'))).toEqual([]);

  // the right token still works in the same form, and it starts one run
  await page.locator('#st-token').fill(TOKEN);
  const [ok] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(ok.status()).toBe(202);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect((await inputs(page)).filter((v) => v.includes(TOKEN))).toEqual([]);
  expect(await added()).toBe(1);
});
