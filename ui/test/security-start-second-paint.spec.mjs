// Security review of the unit daemon-page-starts-a-run: the second paint of the start form (what the
// person picked while the form read the floor and the list) and the floor when its read fails.
// The shared daemon of page-daemon.mjs (stub runtime, no token, floor linux); each test counts the runs it adds.
// Invented texts; no model call; no request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const SUBMIT = '**/daemon/commands/submit-run';
const CONFIGURATION = '**/daemon/views/configuration';
const RUNBOOKS = '**/daemon/views/runbooks';
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const floorRow = (floor) => ({json: {rows: [{daemon_id: 'd', confinement_floor: floor, principal: 'agent:x', started_at: '2026-01-01T00:00:00Z'}]}});
// a route held on a promise the test opens itself, where a fixed delay kept a window open
const held = () => { let open; const wait = new Promise((r) => { open = r; }); return {wait, open}; };
// the form has read the floor of the daemon (linux): only linux is offered from then on
const readEnd = (page) => expect.poll(() => page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value))).toEqual(['linux']).then(() => null);

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

// the form opened and read to its end (open() ends by putting the focus into the intake)
async function opened(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toBeFocused();
}

const level = (page) => page.locator('#st-confinement').evaluate((el) => [el.selectedIndex, el.value]);
const offered = (page) => page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value));

test('the floor rises between the two paints: the level picked below it is dropped, Start is disabled and nothing leaves the page', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  // a first opening on a page that is told the floor local
  await page.route(CONFIGURATION, (route) => route.fulfill(floorRow('local')));
  await opened(page);
  expect(await offered(page)).toEqual(['local', 'linux']);
  await page.click('#st-close');
  // the second opening: the floor is the daemon's own (linux) and its answer is held until the picks and the intake are in
  await page.unroute(CONFIGURATION);
  const gate = held();
  await page.route(CONFIGURATION, async (route) => { await gate.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('local');
  await page.locator('#st-intake').fill('an invented intake under a floor that rises');
  gate.open();
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect.poll(() => offered(page)).toEqual(['linux']);
  expect(await level(page)).toEqual([-1, '']);
  await expect(page.locator('#st-start')).toBeDisabled();
  await expect(page.locator('#st-note')).toContainText('floor of this daemon is linux');
  await page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
  await expect(page.locator('#st-msg')).toContainText('Pick a confinement');
  await page.waitForTimeout(300);
  expect(sent.length, 'a start left the page').toBe(0);
  expect(await added()).toBe(0);
});

test('a start pressed between the two paints, with a level the earlier floor offered: the daemon refuses it and nothing is started (measured: whether it leaves the page)', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await page.route(CONFIGURATION, (route) => route.fulfill(floorRow('local')));
  await opened(page);
  await page.click('#st-close');
  await page.unroute(CONFIGURATION);
  const gate = held();
  await page.route(CONFIGURATION, async (route) => { await gate.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake pressed before the floor was read again');
  await page.locator('#st-confinement').selectOption('local');
  const answer = page.waitForResponse((r) => isStart(r.request()), {timeout: 10000}).catch(() => null);
  await page.locator('#st-start').click({timeout: 2000}).catch(() => {});
  // the press is done: the read of the floor ends, and a start that left the page has been answered by then or is not coming
  gate.open();
  const response = await Promise.race([answer, readEnd(page)]);
  expect(Boolean(response), 'a start left the page and its answer was not seen').toBe(sent.length > 0);
  const line = response ? `the start left the page with ${sent[0].confinement} and the daemon answered ${response.status()} ${(await response.json()).outcome}` : 'no start left the page';
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-second-paint] a start pressed before the second read of the floor: ${line}`);
  if (response) expect(response.status(), 'the daemon took a start below its floor').toBe(409);
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect.poll(() => offered(page)).toEqual(['linux']);
  expect(await added(), 'a run below the floor of the daemon').toBe(0);
});

test('the list changes between the two paints: a runbook that is still listed stays picked, and one that is gone is not sent', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await opened(page);
  const all = await page.locator('#st-runbook option').evaluateAll((els) => els.map((e) => e.value));
  expect(all).toContain('sdc-bugfix');
  expect(all.indexOf('sdc-bugfix')).toBeGreaterThan(0);
  await page.click('#st-close');

  // still listed: the pick made during the read stays
  const stays = held();
  await page.route(RUNBOOKS, async (route) => { await stays.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await page.locator('#st-confinement').selectOption('linux');
  // the list is held until the runbook and the level are picked
  stays.open();
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.waitForTimeout(200);
  await expect(page.locator('#st-runbook')).toHaveValue('sdc-bugfix');
  expect(await level(page)).toEqual([0, 'linux']);
  await page.click('#st-close');

  // gone: the list that arrives has no sdc-bugfix
  await page.unroute(RUNBOOKS);
  const goes = held();
  await page.route(RUNBOOKS, async (route) => {
    const answer = await route.fetch();
    const body = await answer.json();
    await goes.wait;
    return route.fulfill({json: {...body, rows: body.rows.filter((r) => r.runbook_id !== 'sdc-bugfix')}});
  });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake for a runbook that went away');
  // the list without the runbook arrives once the picks and the intake are in
  goes.open();
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect.poll(() => page.locator('#st-runbook option').evaluateAll((els) => els.map((e) => e.value))).not.toContain('sdc-bugfix');
  const now = await page.locator('#st-runbook').evaluate((el) => [el.selectedIndex, el.value]);
  const said = await page.locator('#st-msg').textContent();
  const line = `the picked runbook left the list: the select now shows ${JSON.stringify(now)}, the form says "${said}", Start is ${await page.locator('#st-start').isDisabled() ? 'disabled' : 'enabled'}`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-second-paint] ${line}`);
  expect(now[1]).not.toBe('sdc-bugfix');
  expect(now[0], 'another runbook was selected in place of the one that left the list').toBe(-1);
  await expect(page.locator('#st-start')).toBeDisabled();
  await expect(page.locator('#st-msg')).toContainText('no longer listed');
  // picking another runbook ends the state: the message goes and Start is enabled again
  await page.locator('#st-runbook').selectOption({index: 0});
  await expect(page.locator('#st-msg')).toHaveText('');
  await expect(page.locator('#st-start')).toBeEnabled();
  expect(sent.length).toBe(0);
  expect(await added()).toBe(0);
});

test('the form closed between the two paints stays closed, and the late paint presets no level for the next opening', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  await opened(page);
  await page.click('#st-close');
  const late = held();
  await page.route(RUNBOOKS, async (route) => { await late.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake of a form that is closed while it reads');
  await page.keyboard.press('Escape');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  // the list arrives after the form was closed, and the late paint has run when the answer is read
  const listed = page.waitForResponse((r) => new URL(r.url()).pathname === '/daemon/views/runbooks');
  late.open();
  await (await listed).finished();
  await page.waitForTimeout(300);
  await expect(page.locator('#startdlg'), 'the late paint opened the form again').toHaveJSProperty('open', false);
  expect(await page.evaluate(() => document.activeElement?.id || ''), 'the late paint moved the focus into the closed form').not.toBe('st-intake');
  await page.unroute(RUNBOOKS);
  await opened(page);
  expect(await level(page)).toEqual([-1, '']);
  await expect(page.locator('#st-start')).toBeDisabled();
  expect(sent.length).toBe(0);
  expect(await added()).toBe(0);
});

test('a floor that cannot be read or is no level of the page: only linux is offered, none is preset, and nothing is sent without a pick', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  const answers = [
    ['no JSON', {status: 200, contentType: 'application/json', body: '<html>not json</html>'}],
    ['a 500', {status: 500, json: {refused: 'internal error', message: 'invented'}}],
    ['no rows', {json: {rows: []}}],
    ['no member rows', {json: {}}],
    ['rows that are no list', {json: {rows: 'local'}}],
    ['a row without the floor', {json: {rows: [{daemon_id: 'd'}]}}],
    ['LOCAL', floorRow('LOCAL')],
    ['local with a blank', floorRow('local ')],
    ['a list', floorRow(['local'])],
    ['a number', floorRow(0)],
    ['true', floorRow(true)],
    ['an object', floorRow({toString: 'local'})],
    ['__proto__', floorRow('__proto__')],
  ];
  for (const [kind, answer] of answers) {
    await page.unroute(CONFIGURATION);
    await page.route(CONFIGURATION, (route) => route.fulfill(answer));
    await opened(page);
    expect(await offered(page), kind).toEqual(['linux']);
    expect(await level(page), kind).toEqual([-1, '']);
    await expect(page.locator('#st-start'), kind).toBeDisabled();
    await page.locator('#st-intake').fill('an invented intake without a floor');
    await page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
    await expect(page.locator('#st-msg'), kind).toContainText('Pick a confinement');
    await page.click('#st-close');
  }
  expect(sent.length, 'a start left the page without a pick').toBe(0);
  expect(await added()).toBe(0);
});
