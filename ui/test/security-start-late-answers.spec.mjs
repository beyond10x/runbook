// Security review of the unit daemon-page-starts-a-run: an answer that arrives after the form was closed
// and opened again (it belongs to the opening that sent it), and which answers end the key of a filled form.
// In every case the route fetches the daemon's own answer first, so the daemon has taken or refused the command,
// and holds it until the test opens a gate.
// The shared daemons of page-daemon.mjs (stub runtime, floor linux; one without and one with a token); each test
// counts the runs it adds. Invented texts; no model call; no request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon, TOKEN} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const SUBMIT = '**/daemon/commands/submit-run';
const WRONG = 'wrong-token-0123456789-0123456789-0123456789';
let s, t, n0;

const runsOf = (d, token) => fetch(`${d.base}/daemon/views/runs`, token ? {headers: {authorization: `Bearer ${token}`}} : {}).then((r) => r.json()).then((j) => j.rows);
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const answered = (page) => page.waitForResponse((r) => isStart(r.request()));
const held = () => { let open; const wait = new Promise((r) => { open = r; }); return {wait, open}; };

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('open');
  t = await pageDaemon('token');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runsOf(s)).length;
});
const added = async () => (await runsOf(s)).length - n0;

function commands(page) {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push({...JSON.parse(r.postData()), request: r}); });
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

// the first start is fetched from the daemon and its answer (or the one `instead` makes of it) is held on the gate
async function hold(page, instead = null) {
  const gate = held();
  const state = {taken: null, gate};
  let n = 0;
  await page.unroute(SUBMIT).catch(() => {});
  await page.route(SUBMIT, async (route) => {
    if (n++ > 0) return route.continue();
    const answer = await route.fetch();
    const text = await answer.text();
    state.taken = {status: answer.status(), text};
    await gate.wait;
    return route.fulfill(instead ? instead(answer, text) : {status: answer.status(), contentType: 'application/json', body: text});
  });
  return state;
}

// press, close the form and open it again while the answer is held
async function pressCloseOpen(page, state) {
  await page.click('#st-start');
  await expect.poll(() => state.taken).not.toBeNull();
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.click('#st-close');
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start'), 'a second command can leave while the first is on its way').toBeDisabled();
}

test('a late success of an earlier opening, then the same content pressed in the form opened again: the first run back, one run', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  const content = 'an invented intake whose answer is late and whose form is pressed again';
  await page.locator('#st-intake').fill(content);
  const state = await hold(page);
  await pressCloseOpen(page, state);
  await expect(page.locator('#st-intake')).toHaveValue(content);
  state.gate.open();
  await expect(page.locator('#st-start')).toBeEnabled();
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toHaveValue(content);
  const run = JSON.parse(state.taken.text).published[0].payload.run_id;
  const line = `after a late success the form opened again says "${await page.locator('#st-msg').textContent()}" and "${await page.locator('#st-maybe').textContent()}"; the run list names the run: ${(await page.locator('#runpick').textContent()).includes(run.slice(-8))}`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-late-answers] ${line}`);
  const [again] = await Promise.all([answered(page), page.click('#st-start')]);
  expect((await again.json()).outcome).toBe('already-submitted');
  expect(sent[1].idempotency_key, 'the same content pressed again after a late success carried another key').toBe(sent[0].idempotency_key);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(await added(), 'one filled form made more than one run').toBe(1);
});

test('a late success of an earlier opening, then other content: a second run with a new key, as the story rules for changed content (measured: what the form says)', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-intake').fill('an invented first intake whose answer is late');
  const state = await hold(page);
  await pressCloseOpen(page, state);
  await page.locator('#st-intake').fill('an invented second intake typed into the form opened again');
  state.gate.open();
  await expect(page.locator('#st-start')).toBeEnabled();
  await expect(page.locator('#st-intake')).toHaveValue('an invented second intake typed into the form opened again');
  const line = `after a late success, with other content typed, the form says "${await page.locator('#st-msg').textContent()}" and "${await page.locator('#st-maybe').textContent()}"`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-late-answers] ${line}`);
  const [second] = await Promise.all([answered(page), page.click('#st-start')]);
  expect((await second.json()).outcome).toBe('submitted');
  expect(sent[1].idempotency_key).not.toBe(sent[0].idempotency_key);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(await added(), 'two filled forms, two runs').toBe(2);
});

test('a late refusal and a late failure of an earlier opening are shown in the form opened again and change nothing typed there', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);

  // a refusal of the command (422 intake-too-large) that arrives late: the key ended, nothing was started
  await form(page);
  await page.locator('#st-intake').evaluate((el) => { el.value = 'a'.repeat(1048577); });
  let state = await hold(page);
  await pressCloseOpen(page, state);
  expect(state.taken.status).toBe(422);
  await page.locator('#st-intake').fill('an invented short intake typed after the refused one');
  state.gate.open();
  await expect(page.locator('#st-msg')).toContainText('intake-too-large (422)');
  await expect(page.locator('#st-intake')).toHaveValue('an invented short intake typed after the refused one');
  await expect(page.locator('#st-start')).toBeEnabled();
  expect(await added()).toBe(0);
  const [ok] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(ok.status()).toBe(202);
  expect(sent[1].idempotency_key).not.toBe(sent[0].idempotency_key);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(await added()).toBe(1);

  // a 500 in the place of an answer the daemon gave (the run was taken): the same content pressed again is that run
  await form(page);
  const content = 'an invented intake taken by the daemon and answered late with a 500';
  await page.locator('#st-intake').fill(content);
  state = await hold(page, () => ({status: 500, json: {refused: 'internal error', message: 'an invented failure'}}));
  await pressCloseOpen(page, state);
  expect(state.taken.status).toBe(202);
  state.gate.open();
  await expect(page.locator('#st-msg')).toContainText('internal error (500)');
  await expect(page.locator('#st-msg')).toContainText('Press Start again with the same content');
  await expect(page.locator('#st-intake')).toHaveValue(content);
  const [again] = await Promise.all([answered(page), page.click('#st-start')]);
  expect((await again.json()).outcome).toBe('already-submitted');
  expect(sent[3].idempotency_key).toBe(sent[2].idempotency_key);
  expect(await added(), 'one filled form made more than one run').toBe(2);

  // the same late 500, and other content typed into the form opened again: a second run, and the form says that an earlier press may have started one
  await form(page);
  await page.locator('#st-intake').fill('an invented intake taken and answered late with a 500, then replaced');
  state = await hold(page, () => ({status: 500, json: {refused: 'internal error', message: 'an invented failure'}}));
  await pressCloseOpen(page, state);
  await page.locator('#st-intake').fill('an invented intake that replaced it');
  state.gate.open();
  await expect(page.locator('#st-msg')).toContainText('internal error (500)');
  const [other] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(other.status()).toBe(202);
  expect(sent[5].idempotency_key).not.toBe(sent[4].idempotency_key);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  const maybe = await page.locator('#st-maybe').textContent();
  info.annotations.push({type: 'measured', description: `after a late 500 and a start with other content the next opening says "${maybe}"`});
  console.log(`[security-start-late-answers] after a late 500 and a start with other content the next opening says "${maybe}"`);
  expect(await added()).toBe(4);
});

test('a late 401 of an earlier opening: the token is forgotten, the field of the form opened again is empty, and the next start needs the token', async ({page, context}) => {
  const before = (await runsOf(t, TOKEN)).length;
  const sent = commands(page);
  await page.goto(`${t.base}/`);
  await page.locator('input[name=token]').fill(TOKEN);
  await page.getByRole('button', {name: 'Open'}).click();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await form(page);
  const content = 'an invented intake sent with a token the daemon does not know';
  await page.locator('#st-intake').fill(content);
  await page.locator('#st-token').fill(WRONG);
  const state = await hold(page);
  await page.click('#st-start');
  await expect.poll(() => state.taken).not.toBeNull();
  expect(state.taken.status).toBe(401);
  await page.click('#st-close');
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-confinement').selectOption('linux');
  // the page still holds the token it sent: the form opened again does not ask for one
  await expect(page.locator('#st-token')).toBeHidden();
  state.gate.open();
  await expect(page.locator('#st-token')).toBeVisible();
  await expect(page.locator('#st-token')).toHaveValue('');
  await expect(page.locator('#st-msg')).toContainText('(401)');
  await expect(page.locator('#st-intake')).toHaveValue(content);
  const found = await page.evaluate((n) => {
    const hit = [];
    if (document.documentElement.outerHTML.includes(n)) hit.push('markup');
    for (const el of document.querySelectorAll('*')) if (typeof el.value === 'string' && el.value.includes(n)) hit.push(`value of #${el.id}`);
    if (document.cookie.includes(n) || location.href.includes(n) || JSON.stringify(history.state ?? null).includes(n)) hit.push('cookie, URL or history state');
    for (const store of [localStorage, sessionStorage]) for (let i = 0; i < store.length; i++) if (String(store.getItem(store.key(i))).includes(n)) hit.push('storage');
    return hit;
  }, WRONG);
  expect(found).toEqual([]);
  expect(JSON.stringify(await context.storageState()).includes(WRONG)).toBe(false);
  // the forgotten token is not sent again
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('needs its token');
  await page.waitForTimeout(300);
  expect(sent.length, 'a start left the page with the forgotten token').toBe(1);
  // the right token: the same key (a 401 is no answer of the command), one run
  await page.locator('#st-token').fill(TOKEN);
  const [ok] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(ok.status()).toBe(202);
  expect((await sent[1].request.allHeaders()).authorization).toBe(`Bearer ${TOKEN}`);
  expect(sent[1].idempotency_key).toBe(sent[0].idempotency_key);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect((await runsOf(t, TOKEN)).length - before).toBe(1);
});

// what the page gets in the place of the daemon's answer, after the daemon took the command
const NAMED = [
  ['a 404 that names an outcome the command does not have', 404, (j) => ({outcome: 'no-such-thing', error: 'invented', payload: {}}), null],
  ['a 409 that names an outcome the command does not have', 409, (j) => ({outcome: 'queued-somewhere', error: 'invented', payload: {}}), null],
  ['a 422 that names submitted', 422, (j) => ({...j}), null],
  ['a 400 that names an outcome', 400, (j) => ({outcome: 'bad-somehow'}), null],
  ['a 200 that names submitted and no run', 200, (j) => ({outcome: 'submitted', published: []}), null],
  ['a 401 that names an outcome', 401, (j) => ({outcome: 'unauthorized-somehow'}), null],
  ['a 201 that names an outcome the command does not have, and no run', 201, (j) => ({outcome: 'queued-somewhere'}), null],
  ['a 500 that names submitted with the run', 500, (j) => ({...j}), 1],
  ['a 502 that names an outcome the command does not have', 502, (j) => ({outcome: 'queued-somewhere', published: []}), 1],
  ['a 503 that names no-runbook', 503, (j) => ({outcome: 'no-runbook', error: 'invented', payload: {}}), 1],
  ['a 599 that names already-submitted', 599, (j) => ({...j, outcome: 'already-submitted'}), 1],
];

for (const [half, kinds] of [['first half', NAMED.slice(0, 6)], ['second half', NAMED.slice(6)]]) {
  test(`answers that name an outcome after the daemon took the command (${half}): a 5xx keeps the key and is one run; the others are measured`, async ({page}, info) => {
    const sent = commands(page);
    await enter(page);
    for (const [kind, status, body, runs] of kinds) {
      const before = await added();
      await form(page);
      await page.locator('#st-intake').fill(`an invented intake answered with ${kind}`);
      const state = await hold(page, (answer, text) => ({status, contentType: 'application/json', body: JSON.stringify(body(JSON.parse(text)))}));
      state.gate.open();
      const at = sent.length;
      await Promise.all([answered(page), page.click('#st-start')]);
      await expect(page.locator('#st-start')).toBeEnabled();
      const said = await page.locator('#st-msg').textContent();
      // a 401 makes the page ask for a token: this daemon has none and takes any
      if (await page.locator('#st-token').isVisible()) await page.locator('#st-token').fill('a-token-for-a-daemon-without-one');
      const second = page.waitForResponse((r) => isStart(r.request()), {timeout: 8000}).catch(() => null);
      await page.click('#st-start');
      const again = !(await second) ? 'not sent' : sent[at + 1].idempotency_key === sent[at].idempotency_key ? 'same key' : 'another key';
      await expect(page.locator('#st-start')).toBeEnabled().catch(() => {});
      if (await page.locator('#startdlg').evaluate((d) => d.open)) await page.click('#st-close');
      const made = (await added()) - before;
      const line = `${kind}: the form said "${said}"; pressed again: ${again}; runs of this form: ${made}`;
      info.annotations.push({type: 'measured', description: line});
      console.log(`[security-start-late-answers] ${line}`);
      if (runs !== null) {
        expect.soft(again, `${kind}: a 5xx ended the key`).toBe('same key');
        expect.soft(made, `${kind}: one filled form made more than one run`).toBe(runs);
      }
    }
  });
}
