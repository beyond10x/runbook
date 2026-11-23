// Security review of the unit daemon-page-starts-a-run: a runbook the person picked that left the list
// of the daemon, and the opening after it when the runbook is listed again.
// The shared daemon of page-daemon.mjs (stub runtime, no token, floor linux); each test counts the runs it adds.
// Invented texts; no model call; no request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const RUNBOOKS = '**/daemon/views/runbooks';
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const held = () => { let open; const wait = new Promise((r) => { open = r; }); return {wait, open}; };

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('open');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});

function commands(page) {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  return sent;
}

async function enter(page) {
  await page.goto(`${s.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function opened(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toBeFocused();
}

const runbook = (page) => page.locator('#st-runbook').evaluate((el) => [el.selectedIndex, el.value]);
const forced = (page) => page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
// the list of the daemon without one runbook, or with one more row
const without = (id) => async (route) => { const answer = await route.fetch(); const body = await answer.json(); return route.fulfill({json: {...body, rows: body.rows.filter((r) => r.runbook_id !== id)}}); };

test('a runbook picked in an earlier opening that left the list: none is selected, no press sends, and it is offered again when the list has it again', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await opened(page);
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake for a runbook that leaves the list and returns');
  await page.click('#st-close');

  // the next opening reads a list without it
  await page.route(RUNBOOKS, without('sdc-bugfix'));
  await opened(page);
  await expect(page.locator('#st-msg')).toContainText('no longer listed');
  await expect(page.locator('#st-msg')).toContainText('sdc-bugfix');
  expect((await runbook(page))[0]).toBe(-1);
  await expect(page.locator('#st-start')).toBeDisabled();
  // picking the level paints the form again: still none selected, Start still disabled
  await page.locator('#st-confinement').selectOption('linux');
  expect((await runbook(page))[0], 'a paint after the message selected another runbook').toBe(-1);
  await expect(page.locator('#st-start')).toBeDisabled();
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('Pick a runbook first');
  await page.waitForTimeout(300);
  expect(sent.length, 'a start left the page without a runbook the person picked').toBe(0);
  await page.click('#st-close');

  // the list has the runbook again
  await page.unroute(RUNBOOKS);
  await opened(page);
  const now = await runbook(page);
  const line = `the opening after the runbook returned: the select shows ${JSON.stringify(now)}, the form says "${await page.locator('#st-msg').textContent()}", Start is ${await page.locator('#st-start').isDisabled() ? 'disabled' : 'enabled'} before a level is picked`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-gone-runbook] ${line}`);
  await expect(page.locator('#st-msg')).not.toContainText('no longer listed');
  expect(await page.locator('#st-runbook option').evaluateAll((els) => els.map((e) => e.value))).toContain('sdc-bugfix');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start')).toBeEnabled();
  const [answer] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(answer.status()).toBe(202);
  expect(sent.map((b) => b.runbook_id)).toEqual(['sdc-bugfix']);
  expect(await added()).toBe(1);
});

test('the name of the runbook that left the list is shown as text; measured: the runbook the form showed selected without a pick leaves the list', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  const evil = '<img src=x onerror="window.__hit=1"><b>bold</b>';
  // a list with a runbook whose name looks like markup
  await page.route(RUNBOOKS, async (route) => { const answer = await route.fetch(); const body = await answer.json(); return route.fulfill({json: {...body, rows: [...body.rows, {...body.rows[0], runbook_id: evil}]}}); });
  await opened(page);
  await page.locator('#st-runbook').selectOption(evil);
  await page.click('#st-close');
  await page.unroute(RUNBOOKS);
  await opened(page);
  await expect(page.locator('#st-msg')).toContainText('no longer listed');
  await expect(page.locator('#st-msg')).toContainText(evil);
  expect(await page.locator('#st-msg *').count(), 'the name of the runbook became an element').toBe(0);
  expect(await page.evaluate(() => window.__hit ?? null)).toBeNull();
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.click('#st-close');

  // the first runbook of the list, which nobody picked, leaves the list: nothing is reported, the form shows the list as it is
  const first = await page.locator('#st-runbook option').first().evaluate((el) => el.value);
  await page.reload();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await opened(page);
  expect((await runbook(page))[1]).toBe(first);
  await page.click('#st-close');
  await page.route(RUNBOOKS, without(first));
  await opened(page);
  const now = await runbook(page);
  await page.locator('#st-confinement').selectOption('linux');
  const line = `the runbook shown selected without a pick (${first}) left the list: the select now shows ${JSON.stringify(now)}, the form says "${await page.locator('#st-msg').textContent()}", Start is ${await page.locator('#st-start').isDisabled() ? 'disabled' : 'enabled'} once a level is picked`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-gone-runbook] ${line}`);
  expect(now[1]).not.toBe(first);
  expect(sent.length).toBe(0);
  expect(await added()).toBe(0);
});
