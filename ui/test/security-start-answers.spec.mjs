// Security review of the unit daemon-page-starts-a-run: which answers end the key of a filled form.
// In every case here the daemon took the command (the route fetches it from the daemon) and the page gets
// another answer in its place; the same form pressed again must be one run in the daemon's list.
// The shared daemon of page-daemon.mjs (stub runtime, no token, floor linux); each test counts the runs it adds.
// Invented texts; no model call; no request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const SUBMIT = '**/daemon/commands/submit-run';
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const answered = (page) => page.waitForResponse((r) => isStart(r.request()));

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

// the open form, read to its end, with a runbook and the level linux
async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('linux');
}

// what the page gets in the place of the daemon's answer, after the daemon took the command
const KINDS = [
  ['a 500 of the HTTP layer', () => ({status: 500, json: {refused: 'internal error', message: 'an invented failure'}})],
  ['a 502 with a page of markup', () => ({status: 502, contentType: 'text/html', body: '<h1>Bad Gateway</h1>'})],
  ['a 503 without a body', () => ({status: 503, body: ''})],
  ['a 413', () => ({status: 413, json: {refused: 'request body too large', message: 'invented'}})],
  ['a 404', () => ({status: 404, json: {refused: 'not found', message: 'no such route'}})],
  ['a 429', () => ({status: 429, body: 'slow down'})],
  ['a 401', () => ({status: 401, json: {refused: 'unauthorized', message: 'a bearer token is required'}})],
  ['a 403', () => ({status: 403, json: {refused: 'forbidden', message: 'invented'}})],
  ['a 400', () => ({status: 400, json: {refused: 'bad request', message: 'invented'}})],
  ['a 415', () => ({status: 415, json: {refused: 'unsupported media type', message: 'invented'}})],
  ['a 302 to the page', () => ({status: 302, headers: {location: '/'}, body: ''})],
  ['a 202 with two JSON values', (text) => ({status: 202, contentType: 'application/json', body: `${text}{"outcome":"submitted"}`})],
  ['a 202 without a body', () => ({status: 202, contentType: 'application/json', body: ''})],
  ['a 202 whose body is null', () => ({status: 202, contentType: 'application/json', body: 'null'})],
  ['a 202 whose body is a list', (text) => ({status: 202, contentType: 'application/json', body: `[${text}]`})],
  ['a 202 whose body is a string', () => ({status: 202, contentType: 'application/json', body: '"submitted"'})],
  ['a 202 whose outcome is empty', () => ({status: 202, json: {outcome: '', published: []}})],
  ['a 202 whose outcome is a number', () => ({status: 202, json: {outcome: 1, published: []}})],
  ['a 200 whose outcome is an object', () => ({status: 200, json: {outcome: {name: 'submitted'}, published: []}})],
  ['a body cut after 20 bytes', (text) => ({status: 202, contentType: 'application/json', body: text.slice(0, 20)})],
];

for (const [part, kinds] of [['first', KINDS.slice(0, 5)], ['second', KINDS.slice(5, 10)], ['third', KINDS.slice(10, 15)], ['fourth', KINDS.slice(15)]]) {
  test(`the daemon took the command and the page got another answer (${part} quarter of the kinds): the same form pressed again is one run`, async ({page}) => {
    const sent = commands(page);
    await enter(page);
    let made = 0;
    for (const [kind, answer] of kinds) {
      await form(page);
      await page.locator('#st-intake').fill(`an invented intake answered with ${kind}`);
      let n = 0;
      await page.unroute(SUBMIT);
      await page.route(SUBMIT, async (route) => {
        if (n++ > 0) return route.continue();
        const taken = await route.fetch();
        expect(taken.status(), kind).toBe(202);
        return route.fulfill(answer(await taken.text()));
      });
      const at = sent.length;
      await page.click('#st-start');
      await expect.poll(() => sent.length, kind).toBe(at + 1);
      await expect(page.locator('#st-start'), kind).toBeEnabled();
      await expect(page.locator('#startdlg'), `${kind}: the form closed on an answer that names no run`).toHaveJSProperty('open', true);
      expect(await added(), `${kind}: the daemon took the first command`).toBe(made + 1);
      if (await page.locator('#st-token').isVisible()) await page.locator('#st-token').fill('a-token-typed-after-an-answer-0123456789');
      const [response] = await Promise.all([answered(page), page.click('#st-start')]);
      expect(response.status(), kind).toBe(202);
      await expect(page.locator('#startdlg'), kind).toHaveJSProperty('open', false);
      expect.soft(sent[at + 1].idempotency_key, `${kind} ended the key of the form`).toBe(sent[at].idempotency_key);
      made += 1;
      expect(await added(), `${kind}: one filled form made more than one run`).toBe(made);
    }
  });
}

test('a lost answer, a 500, a 401, a cut body and then the answer, all for one filled form: one run and one key', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-intake').fill('an invented intake pressed five times');
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    n += 1;
    if (n === 5) return route.continue();
    const taken = await route.fetch();
    expect(taken.status()).toBe(202);
    if (n === 1) return route.abort('connectionreset');
    if (n === 2) return route.fulfill({status: 500, json: {refused: 'internal error', message: 'invented'}});
    if (n === 3) return route.fulfill({status: 401, json: {refused: 'unauthorized', message: 'a bearer token is required'}});
    return route.fulfill({status: 202, contentType: 'application/json', body: (await taken.text()).slice(0, 31)});
  });
  for (let press = 1; press <= 4; press++) {
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(press);
    await expect(page.locator('#st-start')).toBeEnabled();
    await expect(page.locator('#st-maybe'), 'the content did not change: no note').toHaveText('');
    if (await page.locator('#st-token').isVisible()) await page.locator('#st-token').fill('a-token-typed-after-the-401-0123456789');
  }
  const [response] = await Promise.all([answered(page), page.click('#st-start')]);
  expect((await response.json()).outcome).toBe('already-submitted');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(new Set(sent.map((b) => b.idempotency_key)).size, 'five presses of one filled form').toBe(1);
  expect(await added()).toBe(1);
});

test('an answer whose body ends after the form was closed and opened again: one run, and no second command while it is on its way', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  const first = 'an invented intake whose answer is slow';
  await page.locator('#st-intake').fill(first);
  let cut = false;
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    if (n++ > 0) return route.continue();
    const taken = await route.fetch();
    expect(taken.status()).toBe(202);
    await new Promise((r) => setTimeout(r, 3500));
    const text = await taken.text();
    return route.fulfill({status: 202, contentType: 'application/json', body: cut ? text.slice(0, 25) : text});
  });

  // the whole answer arrives late
  await page.click('#st-start');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.click('#st-close');
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start'), 'a second command can leave while the first is on its way').toBeDisabled();
  await page.locator('#st-start').evaluate((b) => b.click());
  await page.locator('#st-intake').fill('typed into the form that was opened again');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent.length).toBe(1);
  expect(await added()).toBe(1);
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  info.annotations.push({type: 'measured', description: `what was typed into the reopened form while the answer was on its way is kept: ${(await page.locator('#st-intake').inputValue()) !== ''}`});
  await page.click('#st-close');

  // the late answer is cut: the form that was opened again keeps the key for the same content
  n = 0;
  cut = true;
  await form(page);
  const second = 'a second invented intake whose answer is slow and cut';
  await page.locator('#st-intake').fill(second);
  await page.click('#st-start');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.click('#st-close');
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect(page.locator('#st-msg')).toContainText('could not be read');
  await expect(page.locator('#st-intake')).toHaveValue(second);
  await page.locator('#st-confinement').selectOption('linux');
  const [response] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(response.status()).toBe(202);
  expect(sent[2].idempotency_key).toBe(sent[1].idempotency_key);
  expect(await added()).toBe(2);
});

test('answers that name an outcome end the key: a name the page does not know, with and without a run, and a 5xx that names one (measured)', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  const told = [];
  for (const [kind, answer] of [
    ['a 200 with an unknown outcome and the run', (j) => ({status: 200, json: {...j, outcome: 'queued-somewhere'}})],
    ['a 200 with an unknown outcome and no run', () => ({status: 200, json: {outcome: 'queued-somewhere', published: []}})],
    ['a 500 that names an outcome', () => ({status: 500, json: {outcome: 'no-daemon', error: 'invented', payload: {}}})],
  ]) {
    const before = await added();
    await form(page);
    await page.locator('#st-intake').fill(`an invented intake answered with ${kind}`);
    let n = 0;
    await page.unroute(SUBMIT);
    await page.route(SUBMIT, async (route) => {
      if (n++ > 0) return route.continue();
      const taken = await route.fetch();
      return route.fulfill(answer(await taken.json()));
    });
    const at = sent.length;
    await page.click('#st-start');
    await expect.poll(() => sent.length).toBe(at + 1);
    await expect(page.locator('#st-start')).not.toBeDisabled({timeout: 3000}).catch(() => {});
    const open = await page.locator('#startdlg').evaluate((d) => d.open);
    const said = open ? await page.locator('#st-msg').textContent() : '(the form closed and the run is shown)';
    let again = 'not pressed';
    if (open) {
      await Promise.all([answered(page), page.click('#st-start')]);
      again = sent[at + 1].idempotency_key === sent[at].idempotency_key ? 'same key' : 'another key';
      if (await page.locator('#startdlg').evaluate((d) => d.open)) await page.click('#st-close');
    }
    told.push(`${kind}: the form said "${said}"; pressed again: ${again}; runs of this form: ${(await added()) - before}`);
  }
  for (const line of told) { info.annotations.push({type: 'measured', description: line}); console.log(`[security-start-answers] ${line}`); }
  // held: an answer that names a run of the daemon closes the form with that run, and nothing here is markup
  expect(told[0]).toContain('the form closed');
});
