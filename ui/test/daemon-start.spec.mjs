// A person starts a run from the page the daemon serves (`runbook serve --ui`, the binary cargo built;
// story daemon-page-starts-a-run). The page lists the runbooks from the view `runbooks`, takes the intake
// as text and the confinement (never below the daemon's floor), and sends the daemon's own command
// `submit-run`: a POST with `Authorization: Bearer <token>` (when the daemon has a token), JSON, a fresh
// idempotency key per press, and no cookie. The new run appears in the run list and is selected; a refusal
// of the daemon is shown with its name. The header says "operate". Under the stub runtime the form says
// that the run is scripted and nothing is executed. The token stays in the memory of the page and the text
// of an intake never becomes markup.
// Four daemons, each on 127.0.0.1:0 with a state directory of its own, and no model call: the stub
// runtime's presets and one runbook directory with an invented runbook `example`; one daemon runs the
// Loom runtime over the fixture runbook home of the engine tests and is only listed from, never run.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import {randomBytes} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const home = path.resolve(ui, '../runbook/tests/fixtures/engine-runbook');
const TOKEN = `tok-${randomBytes(24).toString('hex')}`;
const servers = {};
let dir;

function start(name, args = [], env = {}) {
  return new Promise((resolve, reject) => {
    fs.mkdirSync(path.join(dir, name, 'empty'), {recursive: true});
    const proc = spawnChild(bin, ['serve', '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, name, 'state'), ...args], {cwd: path.join(dir, name, 'empty'), env: {...process.env, HOME: path.join(dir, name), RUNBOOK_TOKEN: '', ...env}, stdio: ['ignore', 'pipe', 'pipe']});
    let out = '', err = '';
    const timer = setTimeout(() => { proc.kill(); reject(new Error(`no listening line in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { err += d; });
    proc.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], token: env.RUNBOOK_TOKEN || null}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

// a runbook directory with the invented runbook `example`
function books(name) {
  const d = path.join(dir, name, 'books');
  fs.mkdirSync(path.join(d, 'example'), {recursive: true});
  fs.writeFileSync(path.join(d, 'example', 'protocol.yaml'), 'invented: runbook\n');
  return d;
}

const headers = (s) => (s.token ? {authorization: `Bearer ${s.token}`} : {});
const rows = (s, view, q = '') => fetch(`${s.base}/daemon/views/${view}${q}`, {headers: headers(s)}).then((r) => r.json()).then((j) => j.rows);

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemons: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-start-'));
  servers.open = await start('open', ['--runbook-dir', books('open')]);
  servers.guarded = await start('guarded', ['--runbook-dir', books('guarded'), '--min-confinement', 'local'], {RUNBOOK_TOKEN: TOKEN});
  servers.refusing = await start('refusing', ['--runbook-dir', books('refusing')]);
  servers.loom = await start('loom', ['--runtime', 'loom', '--runbook-home', home]);
  servers.refusing.books = path.join(dir, 'refusing', 'books');
});
test.afterAll(() => {
  for (const s of Object.values(servers)) if (s.proc.exitCode == null) s.proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

// every error of the page fails the test, except the 401 that a daemon with a token is meant to answer and the 422 of a refusal
function watch(page, {allow401 = false, allow422 = false} = {}) {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('dialog', (d) => { problems.push(`dialog ${d.message()}`); d.dismiss(); });
  page.on('console', (m) => { if (m.type() === 'error' && !(allow401 && /status of 401/.test(m.text())) && !(allow422 && /status of 422/.test(m.text()))) problems.push(`console ${m.text()}`); });
  return problems;
}

// the commands the page sends
function commands(page) {
  const sent = [];
  page.on('request', (r) => { if (r.method() === 'POST' && new URL(r.url()).pathname.startsWith('/daemon/commands/')) sent.push(r); });
  return sent;
}

async function enter(page, s) {
  await page.goto(`${s.base}/`);
  if (s.token) {
    await page.locator('input[name=token]').fill(s.token);
    await page.getByRole('button', {name: 'Open'}).click();
  }
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
}

const texts = (page, sel) => page.locator(sel).evaluateAll((els) => els.map((e) => e.textContent));
const values = (page, sel) => page.locator(sel).evaluateAll((els) => els.map((e) => e.value));
const bodyOf = (req) => JSON.parse(req.postData());

test('the header says operate, the form lists the runbooks and says the run is scripted, and no level below the floor is offered', async ({page}) => {
  const s = servers.open;
  const problems = watch(page);
  const sent = commands(page);
  await enter(page, s);
  await expect(page.locator('#mode')).toHaveText('operate');
  await form(page);
  expect(await texts(page, '#st-runbook option')).toEqual(['example', 'sdc', 'sdc-bugfix', 'stub-decision']);
  await expect(page.locator('#st-note')).toContainText('scripted');
  await expect(page.locator('#st-note')).toContainText('nothing is executed');
  expect(await values(page, '#st-confinement option')).toEqual(['linux']);
  await expect(page.locator('#st-confinement')).toHaveValue('linux');
  await expect(page.locator('#st-token')).toBeHidden();

  // a level below the floor, put into the select by hand, is not sent
  await page.locator('#st-confinement').evaluate((el) => { el.insertAdjacentElement('afterbegin', Object.assign(document.createElement('option'), {value: 'local', textContent: 'local'})); el.value = 'local'; });
  await page.locator('#st-intake').fill('an intake of the invented kind');
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('floor');
  await expect(page.locator('#st-msg')).toContainText('linux');
  expect(sent.length).toBe(0);
  expect(await rows(s, 'runs')).toEqual([]);
  expect(problems).toEqual([]);
});

test('Start sends submit-run with a fresh key each press, the new run appears in the list and is selected', async ({page}) => {
  const s = servers.open;
  const problems = watch(page);
  const sent = commands(page);
  await enter(page, s);
  const keys = [];
  for (const [i, intake] of ['first intake of the invented runbook', 'second intake'].entries()) {
    await form(page);
    await page.locator('#st-runbook').selectOption('sdc');
    await page.locator('#st-intake').fill(intake);
    const [response] = await Promise.all([page.waitForResponse((r) => r.request().method() === 'POST'), page.click('#st-start')]);
    expect(response.status()).toBe(202);
    const run = (await response.json()).published[0].payload.run_id;
    const req = sent[i];
    expect(new URL(req.url()).pathname).toBe('/daemon/commands/submit-run');
    const h = await req.allHeaders();
    expect(h['content-type']).toContain('application/json');
    expect(h.authorization).toBeUndefined();
    expect(h.cookie).toBeUndefined();
    const body = bodyOf(req);
    expect(body).toMatchObject({runbook_id: 'sdc', intake, confinement: 'linux'});
    expect(typeof body.idempotency_key).toBe('string');
    expect(body.idempotency_key.length).toBeGreaterThanOrEqual(16);
    keys.push(body.idempotency_key);
    // the run is the one on screen, and the form has gone
    await expect(page.locator('#runpick')).toContainText(run.slice(-8));
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    await expect(page.locator('#feed')).toContainText('runbook.submitted');
    await expect(page.locator('#feed .fd-list')).toHaveAttribute('data-run', run);
  }
  expect(new Set(keys).size).toBe(2);
  expect((await rows(s, 'runs')).length).toBe(2);
  // the intake is cleared once it was sent
  await form(page);
  await expect(page.locator('#st-intake')).toHaveValue('');
  expect(problems).toEqual([]);
});

test('the text of an intake never becomes markup', async ({page}) => {
  const s = servers.open;
  const problems = watch(page);
  await enter(page, s);
  const intake = '<img src="x" onerror="window.__ran = 1"><b>bold</b> & <script>window.__ran = 2</script>';
  await form(page);
  await page.locator('#st-intake').fill(intake);
  const [response] = await Promise.all([page.waitForResponse((r) => r.request().method() === 'POST'), page.click('#st-start')]);
  const run = (await response.json()).published[0].payload.run_id;
  await expect(page.locator('#runpick')).toContainText(run.slice(-8));
  expect((await rows(s, 'run', `?run_id=${run}`))[0].intake).toBe(intake);
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => window.__ran)).toBeUndefined();
  expect(await page.evaluate(() => document.querySelectorAll('img[src="x"], #startdlg script').length + [...document.querySelectorAll('b')].filter((b) => b.textContent === 'bold').length)).toBe(0);
  expect(problems).toEqual([]);
});

test('a refusal of the daemon is shown with its name, and the form keeps what was typed', async ({page}) => {
  const s = servers.refusing;
  const problems = watch(page, {allow422: true});
  const sent = commands(page);
  await enter(page, s);
  await form(page);
  expect(await texts(page, '#st-runbook option')).toContain('example');
  // the runbook goes away after the page listed it: the daemon refuses the submit
  fs.rmSync(path.join(s.books, 'example'), {recursive: true});
  await page.locator('#st-runbook').selectOption('example');
  await page.locator('#st-intake').fill('a text the person does not want to lose');
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('no-runbook');
  await expect(page.locator('#st-msg')).toContainText('422');
  await expect(page.locator('#st-msg')).toHaveClass(/\bbad\b/);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toHaveValue('a text the person does not want to lose');
  expect(await rows(s, 'runs')).toEqual([]);

  // an intake over 1 MiB
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').evaluate((el) => { el.value = 'a'.repeat(1048577); });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('intake-too-large');
  expect(sent.length).toBe(2);
  expect(await rows(s, 'runs')).toEqual([]);

  // an empty intake is not sent
  await page.locator('#st-intake').fill('');
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('intake');
  expect(sent.length).toBe(2);
  expect(problems).toEqual([]);
});

test('a daemon with a token: the page asks once, sends the bearer and no cookie, and keeps the token in memory only', async ({page}) => {
  const s = servers.guarded;
  const problems = watch(page, {allow401: true});
  const sent = commands(page);
  await enter(page, s);
  await form(page);
  // the floor is local here: both levels are offered and the floor is the preset
  expect(await values(page, '#st-confinement option')).toEqual(['local', 'linux']);
  await expect(page.locator('#st-confinement')).toHaveValue('local');
  await expect(page.locator('#st-token')).toBeVisible();
  await page.locator('#st-intake').fill('an intake for the guarded daemon');
  await page.locator('#st-runbook').selectOption('sdc');

  // no token typed: nothing is sent
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('token');
  expect(sent.length).toBe(0);

  // a wrong token: the daemon says 401, the page forgets it and asks again
  await page.locator('#st-token').fill('not-the-token-of-this-daemon-0123456789');
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('401');
  await expect(page.locator('#st-token')).toBeVisible();
  await expect(page.locator('#st-token')).toHaveValue('');

  // the right one: the bearer goes, the cookie of the login does not, and the field is emptied
  await page.locator('#st-token').fill(s.token);
  const [response] = await Promise.all([page.waitForResponse((r) => r.request().method() === 'POST' && r.status() === 202), page.click('#st-start')]);
  const run = (await response.json()).published[0].payload.run_id;
  const h = await sent.at(-1).allHeaders();
  expect(h.authorization).toBe(`Bearer ${s.token}`);
  expect(h.cookie).toBeUndefined();
  expect(bodyOf(sent.at(-1))).toMatchObject({runbook_id: 'sdc', confinement: 'local'});
  await expect(page.locator('#runpick')).toContainText(run.slice(-8));
  await expect(page.locator('#st-token')).toHaveValue('');
  const leaks = await page.evaluate((t) => JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage), document.cookie, location.href, history.state, document.documentElement.outerHTML]).includes(t), s.token);
  expect(leaks).toBe(false);

  // the next start in the same page needs no typing; a reload forgets the token
  await form(page);
  await expect(page.locator('#st-token')).toBeHidden();
  await page.locator('#st-intake').fill('a second one');
  await Promise.all([page.waitForResponse((r) => r.request().method() === 'POST' && r.status() === 202), page.click('#st-start')]);
  expect((await sent.at(-1).allHeaders()).authorization).toBe(`Bearer ${s.token}`);
  await page.reload();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await form(page);
  await expect(page.locator('#st-token')).toBeVisible();
  expect(problems).toEqual([]);
});

test('a daemon on the Loom runtime lists the runbooks of its home and does not say the run is scripted', async ({page}) => {
  const s = servers.loom;
  const problems = watch(page);
  await enter(page, s);
  await expect(page.locator('#mode')).toHaveText('operate');
  await form(page);
  expect(await texts(page, '#st-runbook option')).toEqual(['engine-demo · version 1', 'engine-draft · version 1']);
  await expect(page.locator('#st-note')).not.toContainText('scripted');
  expect(problems).toEqual([]);
});
