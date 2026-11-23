// Security review, pass 2, of the unit daemon-page-starts-a-run: the life of the idempotency key of one filled
// form, and the level that nobody picked. The shared daemon of page-daemon.mjs (stub runtime, no token, floor linux) on
// 127.0.0.1:0, shared by its tests: each test counts the runs it adds. Invented texts; no model call.
// Its state directory is page-daemon.mjs's, one per worker.
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

// the open form with a runbook; the level is the caller's to pick
async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-runbook').selectOption('sdc');
}

// the daemon takes the command and the page never gets the answer
const takenAndLost = async (route) => { const answer = await route.fetch(); expect(answer.status()).toBe(202); return route.abort('connectionreset'); };

test('an accepted start whose answer arrives without its whole body, pressed again as the form says, is one run', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake whose answer is cut on the way back');
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    if (n++ > 0) return route.continue();
    // the daemon takes the command; the status reaches the page, the body is cut after 20 bytes
    const answer = await route.fetch();
    expect(answer.status()).toBe(202);
    return route.fulfill({status: 202, contentType: 'application/json', body: (await answer.text()).slice(0, 20)});
  });
  await page.click('#st-start');
  // the form says what it holds: press again with the same content, at most one run
  await expect(page.locator('#st-msg')).toContainText('at most one run');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  expect(await added(), 'the daemon took the first command').toBe(1);
  await Promise.all([answered(page), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect.soft(sent[1].idempotency_key, 'the second press of the same form carries another key').toBe(sent[0].idempotency_key);
  const after = await added();
  expect(after, `the form said "at most one run is started" and ${after} runs were started`).toBe(1);
});

test('a lost answer, then a 401, then the token and Start again: one run, or the form says that an earlier press may have started one', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake with a 401 between two presses');
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    n += 1;
    if (n === 1) return takenAndLost(route);
    // what the HTTP layer of a daemon answers once its token is another one: the command is not looked at
    if (n === 2) return route.fulfill({status: 401, json: {refused: 'unauthorized', message: 'a bearer token is required'}});
    return route.continue();
  });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  expect(await added(), 'the daemon took the first command').toBe(1);
  await Promise.all([answered(page), page.click('#st-start')]);
  await expect(page.locator('#st-msg')).toContainText('401');
  await expect(page.locator('#st-token')).toBeVisible();
  await page.locator('#st-token').fill('a-token-typed-after-the-401-0123456789');
  const noteBefore = await page.locator('#st-maybe').textContent();
  const [response] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(response.status()).toBe(202);
  const note = noteBefore + await page.locator('#st-maybe').textContent();
  const after = await added();
  expect.soft(sent[2].idempotency_key, 'the 401 said nothing about the command, and the key of the form is gone').toBe(sent[0].idempotency_key);
  expect(after === 1 || note.includes('may have started a run'), `the same form made ${after} runs and the form said "${note}" about the first one`).toBe(true);
});

test('a lost answer, other content pressed and lost, the first content again: the form says that an earlier press may have started a run', async ({page}, info) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  const first = 'the first invented content of this form';
  await page.locator('#st-intake').fill(first);
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    n += 1;
    if (n === 1) return takenAndLost(route);
    if (n === 2) return route.abort('connectionreset');
    return route.continue();
  });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await expect(page.locator('#st-start')).toBeEnabled();
  await page.locator('#st-intake').fill('other invented content, typed in between');
  await page.click('#st-start');
  await expect.poll(() => sent.length).toBe(2);
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await expect(page.locator('#st-start')).toBeEnabled();
  await expect(page.locator('#st-maybe')).toContainText('may have started a run');
  await page.locator('#st-intake').fill(first);
  // the note is on the form when the first content is pressed again
  await expect(page.locator('#st-maybe')).toContainText('may have started a run');
  const [response] = await Promise.all([answered(page), page.click('#st-start')]);
  expect(response.status()).toBe(202);
  expect(new Set(sent.map((b) => b.idempotency_key)).size, 'three presses, three keys').toBe(3);
  expect((await response.json()).outcome).toBe('submitted');
  // measured, not asserted: the story lets the key change with the content, and the note was on the form. The
  // other content never reached the daemon, so every run added here is a run of the first content.
  info.annotations.push({type: 'measured', description: `runs of the first content: ${await added()}`});
  console.log(`[security-start-key] the first content pressed again after other content: runs of the first content ${await added()}`);
  expect(await added()).toBeGreaterThanOrEqual(1);
});

test('a lost answer keeps the key through an edit that is taken back and through Close and a new opening of the form', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  let lose = false;
  await page.route(SUBMIT, (route) => { if (!lose) return route.continue(); lose = false; return takenAndLost(route); });

  // an edit taken back before the next press
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  const one = 'an invented intake that is edited and put back';
  await page.locator('#st-intake').fill(one);
  lose = true;
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await page.locator('#st-intake').fill('something else for a moment');
  await page.locator('#st-runbook').selectOption('sdc-bugfix');
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill(one);
  await Promise.all([answered(page), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent[1].idempotency_key).toBe(sent[0].idempotency_key);
  expect(await added()).toBe(1);

  // Close and a new opening: the level is to be picked again, the key is the form's
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  const two = 'an invented intake of a form that is closed and opened again';
  await page.locator('#st-intake').fill(two);
  lose = true;
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await page.click('#st-close');
  await form(page);
  await expect(page.locator('#st-intake')).toHaveValue(two);
  await expect(page.locator('#st-confinement')).toHaveValue('');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-confinement').selectOption('linux');
  await Promise.all([answered(page), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent[3].idempotency_key).toBe(sent[2].idempotency_key);
  expect(sent[2].idempotency_key).not.toBe(sent[0].idempotency_key);
  expect(await added()).toBe(2);
});

test('content changed while a command is on its way: the answer that is lost leaves the note on the next press, and the key is in no URL, storage or output of the daemon', async ({page}) => {
  const sent = commands(page);
  const urls = [];
  page.on('request', (r) => urls.push(r.url()));
  await enter(page);
  await form(page);
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake that is changed while its command is on its way');
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    if (n++ > 0) { await new Promise((r) => setTimeout(r, 1000)); return route.continue(); }
    const answer = await route.fetch();
    expect(answer.status()).toBe(202);
    await new Promise((r) => setTimeout(r, 1200));
    return route.abort('connectionreset');
  });
  await page.click('#st-start');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-intake').fill('the content the person typed while waiting');
  await expect(page.locator('#st-msg')).toContainText('could not be reached');
  await expect(page.locator('#st-maybe')).toHaveText('');
  const second = answered(page);
  await page.click('#st-start');
  // while the second command is on its way the form says that the first may have started a run
  await expect(page.locator('#st-maybe')).toContainText('may have started a run', {timeout: 900});
  const response = await second;
  expect(response.status()).toBe(202);
  expect(sent[1].idempotency_key).not.toBe(sent[0].idempotency_key);
  expect(sent[1].intake).toBe('the content the person typed while waiting');
  expect(await added(), 'two contents, two runs: the first was taken by the daemon').toBe(2);

  // the key: 128 random bits, in the body only
  for (const b of sent) expect(b.idempotency_key).toMatch(/^page-[0-9a-f]{32}$/);
  const keys = sent.map((b) => b.idempotency_key);
  const places = await page.evaluate(() => JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage), document.cookie, location.href, history.state, document.documentElement.outerHTML]));
  for (const k of keys) {
    expect(places.includes(k), 'the key is in a storage, the URL, the history state or the DOM').toBe(false);
    expect(urls.some((u) => u.includes(k)), 'the key is in a URL').toBe(false);
    expect(s.log().includes(k), 'the key is in the output of the daemon').toBe(false);
  }
});

test('no press sends a start without a picked level: an enabled button, an injected level and the keyboard send nothing', async ({page}) => {
  const sent = commands(page);
  await enter(page);
  await form(page);
  await page.locator('#st-intake').fill('an invented intake without a level');
  const level = page.locator('#st-confinement');
  expect(await level.evaluate((el) => [el.selectedIndex, el.value])).toEqual([-1, '']);
  await expect(page.locator('#st-start')).toBeDisabled();
  // the keyboard
  await page.locator('#st-intake').focus();
  await page.keyboard.press('Control+Enter');
  await page.locator('#st-runbook').focus();
  await page.keyboard.press('Enter');
  await page.locator('#st-start').click({force: true});
  // the button enabled by hand, no level
  await page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
  await expect(page.locator('#st-msg')).toContainText('Pick a confinement');
  // an empty level and a level below the floor put into the select by hand, with the event the page listens to
  for (const v of ['', 'local', 'none', 'LINUX', ' linux']) {
    await level.evaluate((el, value) => { el.append(Object.assign(document.createElement('option'), {value, textContent: value || 'empty'})); el.value = value; el.dispatchEvent(new Event('change', {bubbles: true})); }, v);
    await expect(page.locator('#st-start'), `level ${JSON.stringify(v)}`).toBeDisabled();
    expect(await level.evaluate((el) => el.value), `level ${JSON.stringify(v)} stays picked`).toBe('');
  }
  // the same without the event, and the button enabled by hand
  for (const v of ['local', 'none', ' linux']) {
    await level.evaluate((el, value) => { el.append(Object.assign(document.createElement('option'), {value, textContent: value})); el.value = value; }, v);
    await page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
    await expect(page.locator('#st-msg'), `level ${JSON.stringify(v)}`).toContainText('below the floor');
  }
  await page.waitForTimeout(300);
  expect(sent.length, 'a start left the page without a picked level at or above the floor').toBe(0);
  expect(await added()).toBe(0);
  // a started run does not leave its level behind
  await page.click('#st-close');
  await form(page);
  await level.selectOption('linux');
  await Promise.all([answered(page), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await form(page);
  expect(await level.evaluate((el) => [el.selectedIndex, el.value])).toEqual([-1, '']);
  await expect(page.locator('#st-start')).toBeDisabled();
  expect(await added()).toBe(1);
});

test('a level picked while the form still reads the floor and the list of the daemon stays picked', async ({page}) => {
  await enter(page);
  // a first opening, so that the page knows the list and the floor
  await form(page);
  await page.click('#st-close');
  // the second opening: the list of the daemon takes 800 ms
  await page.route('**/daemon/views/runbooks', async (route) => { await new Promise((r) => setTimeout(r, 800)); return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start')).toBeEnabled();
  // open() ends by putting the focus into the intake
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.waitForTimeout(300);
  expect(await page.locator('#st-confinement').evaluate((el) => el.value), 'the level the person picked was thrown away when the list arrived').toBe('linux');
  await expect(page.locator('#st-start')).toBeEnabled();
});
