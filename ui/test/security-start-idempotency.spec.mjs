// Security review, pass 1, of the unit daemon-page-starts-a-run: one filled form is one run. The idempotency key
// belongs to the filled form: it stays the same for a press repeated after an answer that did not arrive and
// changes when the daemon answered (accepted or refused) or when the runbook, the intake or the confinement changed.
// The shared daemon of page-daemon.mjs (stub runtime, no token, floor linux) on 127.0.0.1:0, shared by its tests: each test counts
// the runs it adds. Invented texts; no model call.
// Its state directory is page-daemon.mjs's, one per worker.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('open');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});

async function form(page) {
  await page.goto(`${s.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('linux');
}

test('a start whose answer was lost on the way back and is pressed again is one run, not two', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  const intake = 'one invented intake, which the person means to start once';
  await page.locator('#st-intake').fill(intake);
  // the daemon takes the first command; its answer does not reach the page (the connection is reset)
  let lost = 0;
  await page.route('**/daemon/commands/submit-run', async (route) => {
    if (lost++ === 0) { const answer = await route.fetch(); expect(answer.status()).toBe(202); return route.abort('connectionreset'); }
    return route.continue();
  });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toHaveValue(intake);
  expect(await added(), 'the daemon took the first command').toBe(1);
  // the form kept everything and says the daemon was not reached: the person presses Start again
  await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent.length).toBe(2);
  expect(sent[1].intake).toBe(sent[0].intake);
  expect(sent[1].idempotency_key, 'the key of one filled form stays the same for a press after a lost answer').toBe(sent[0].idempotency_key);
  const after = await added();
  expect(after, `one intake, pressed again after a lost answer, made ${after} runs (keys ${sent.map((b) => b.idempotency_key).join(', ')})`).toBe(1);
});

test('a double click and every press while the first command is on its way send one command and make one run', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  await page.locator('#st-intake').fill('an invented intake, pressed many times');
  await page.route('**/daemon/commands/submit-run', async (route) => { await new Promise((r) => setTimeout(r, 1500)); await route.continue(); });
  await page.locator('#st-start').dblclick();
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-start').click({force: true});
  await page.locator('#st-start').evaluate((b) => { b.click(); b.focus(); });
  await page.keyboard.press('Enter');
  await page.keyboard.press('Space');
  await page.locator('#st-intake').focus();
  await page.keyboard.press('Control+Enter');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent.length).toBe(1);
  expect(await added()).toBe(1);
  // the next form starts without a level (none is preset), and can start again once one is picked
  await page.click('#startbtn');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start')).toBeEnabled();
});

test('a refused start makes no run, and the start after it carries another key', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  await page.locator('#st-intake').evaluate((el) => { el.value = 'a'.repeat(1048577); });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('intake-too-large');
  expect(await added()).toBe(0);
  await page.locator('#st-intake').fill('another invented intake');
  await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 202), page.click('#st-start')]);
  expect(sent.length).toBe(2);
  expect(sent[0].idempotency_key).not.toBe(sent[1].idempotency_key);
  expect(await added()).toBe(1);
  const after = await runs();
  expect(after[0].intake ?? 'another invented intake').toBe('another invented intake');
});

test('a start the daemon refused and a start it accepted each end the key: the same content pressed again carries another key', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  await page.locator('#st-intake').fill('an invented intake the daemon refuses twice');
  let refused = 0;
  await page.route('**/daemon/commands/submit-run', (route) => (refused++ < 2 ? route.fulfill({status: 422, json: {outcome: 'no-runbook', error: 'no such runbook', payload: {}, published: []}}) : route.continue()));
  for (let i = 0; i < 2; i++) {
    await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
    await expect(page.locator('#st-msg')).toContainText('no-runbook');
    await expect(page.locator('#st-start')).toBeEnabled();
  }
  expect(sent.length).toBe(2);
  expect(sent[1].idempotency_key, 'an answered key is used up').not.toBe(sent[0].idempotency_key);
  await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 202), page.click('#st-start')]);
  expect(sent[2].idempotency_key).not.toBe(sent[1].idempotency_key);
  expect(await added()).toBe(1);
});

test('presses after answers that did not arrive keep the key while the content is the same, and change it with a note when the content changed', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  // the page is told a floor of local, so that a level can change; every command is lost before the daemon takes it
  await page.route('**/daemon/views/configuration', (route) => route.fulfill({json: {rows: [{daemon_id: 'd', confinement_floor: 'local', principal: 'agent:x', started_at: '2026-01-01T00:00:00Z'}]}}));
  await page.click('#st-close');
  await page.click('#startbtn');
  await expect.poll(() => page.locator('#st-confinement option').count()).toBe(2);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('linux');
  await page.route('**/daemon/commands/submit-run', (route) => route.abort('connectionreset'));
  const press = async () => {
    const n = sent.length;
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(n + 1);
    await expect(page.locator('#st-msg')).toContainText('could not be reached');
    await expect(page.locator('#st-start')).toBeEnabled();
  };
  const maybe = page.locator('#st-maybe');

  await page.locator('#st-intake').fill('an invented intake of the first filling');
  await press();
  await expect(maybe, 'no earlier press with other content: no note').toHaveText('');
  await press();
  expect(sent[1].idempotency_key, 'the same content, the same key').toBe(sent[0].idempotency_key);
  await expect(maybe).toHaveText('');

  // the intake changed after a lost answer
  await page.locator('#st-intake').fill('an invented intake of the second filling');
  await press();
  expect(sent[2].idempotency_key, 'another intake, another key').not.toBe(sent[1].idempotency_key);
  await expect(maybe).toContainText('may have started a run');
  await press();
  expect(sent[3].idempotency_key).toBe(sent[2].idempotency_key);

  // the runbook changed
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await press();
  expect(sent[4].idempotency_key, 'another runbook, another key').not.toBe(sent[3].idempotency_key);
  expect(sent[4].runbook_id).toBe('sdc-bugfix');

  // the confinement changed
  await page.locator('#st-confinement').selectOption('local');
  await press();
  expect(sent[5].idempotency_key, 'another level, another key').not.toBe(sent[4].idempotency_key);
  expect(sent[5].confinement).toBe('local');
  await page.locator('#st-confinement').selectOption('linux');
  await press();
  expect(sent[6].idempotency_key).not.toBe(sent[5].idempotency_key);
  await expect(maybe).toContainText('may have started a run');
  expect(new Set(sent.map((b) => b.idempotency_key)).size).toBe(5);
  expect(await added(), 'every command was lost before the daemon took it').toBe(0);
});

// What is not the whole answer of the command keeps the key: the answers of the HTTP layer (the command was not looked at,
// so the earlier press is not marked as possibly taken), a failure of the daemon and a body that is not JSON (the
// command may have been taken, so a change of content afterwards shows the note).
const KEEPS = [
  ['401', {status: 401, json: {refused: 'unauthorized', message: 'a bearer token is required'}}, false],
  ['403', {status: 403, json: {refused: 'forbidden', message: 'this request is not for this daemon'}}, false],
  ['500', {status: 500, json: {error: 'internal error'}}, true],
  ['a body that is not JSON', {status: 202, contentType: 'text/plain', body: 'not json at all'}, true],
];
for (const [kind, answer, taken] of KEEPS) {
  test(`an answer that is ${kind} keeps the key of the form: the same content pressed again carries the same key, and a change of content ${taken ? 'shows' : 'does not show'} the note`, async ({page}) => {
    const sent = [];
    page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
    await form(page);
    await page.locator('#st-intake').fill(`an invented intake answered with ${kind}`);
    // press 1 and press 3 get the answer, press 2 is the daemon's, press 4 never reaches it
    let n = 0;
    await page.route('**/daemon/commands/submit-run', (route) => {
      n += 1;
      if (n === 1 || n === 3) return route.fulfill(answer);
      if (n === 4) return route.abort('connectionreset');
      return route.continue();
    });
    const token = async () => { if (await page.locator('#st-token').isVisible()) await page.locator('#st-token').fill('a-token-typed-after-an-answer-0123456789'); };
    const enabled = () => expect(page.locator('#st-start')).toBeEnabled();
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(1);
    await enabled();
    await token();
    await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 202), page.click('#st-start')]);
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    expect(sent[1].idempotency_key, `the answer ${kind} ended the key`).toBe(sent[0].idempotency_key);
    expect(await added()).toBe(1);

    // a new form: the answer, then other content, whose press is lost
    await page.click('#startbtn');
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
    await expect(page.locator('#st-intake')).toBeFocused();
    await page.locator('#st-confinement').selectOption('linux');
    await page.locator('#st-intake').fill(`a second invented intake answered with ${kind}`);
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(3);
    await enabled();
    await token();
    await page.locator('#st-intake').fill(`a third invented intake after ${kind}`);
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(4);
    await expect(page.locator('#st-msg')).toContainText('could not be reached');
    expect(sent[3].idempotency_key).not.toBe(sent[2].idempotency_key);
    if (taken) await expect(page.locator('#st-maybe')).toContainText('may have started a run');
    else await expect(page.locator('#st-maybe')).toHaveText('');
    expect(await added()).toBe(1);
  });
}
