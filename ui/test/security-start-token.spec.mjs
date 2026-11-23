// Security review, pass 1, of the unit daemon-page-starts-a-run: the token field of the start form.
// One daemon for the file, with a token (stub runtime, floor linux) on 127.0.0.1:0, shared by its tests: each test
// counts the runs it adds. Invented texts; no model call. The log of the daemon holds the output of every test, so
// the test that reads it holds the whole file to it.
// The state directory lies under SEC_SCRATCH when it is set (the reviewer's scratch), else under node_modules.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import {randomBytes} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const TOKEN = `tok-${randomBytes(24).toString('hex')}`;
const WRONG = 'not-the-token-of-this-daemon-0123456789';
let s, dir, n0;

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
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], log: () => out + err}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const runs = () => fetch(`${s.base}/daemon/views/runs`, {headers: {authorization: `Bearer ${TOKEN}`}}).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-start-token-'));
  s = await start('token', [], {RUNBOOK_TOKEN: TOKEN});
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});
test.afterAll(() => { if (s?.proc.exitCode == null) s?.proc.kill(); if (dir) fs.rmSync(dir, {recursive: true, force: true}); });

// the login of the page (the read cookie), then the start form with a runbook and an intake
async function form(page) {
  await page.goto(`${s.base}/`);
  await page.locator('input[name=token]').fill(TOKEN);
  await page.getByRole('button', {name: 'Open'}).click();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('linux');
  await page.locator('#st-intake').fill('an invented intake for the daemon with a token');
  await expect(page.locator('#st-token')).toBeVisible();
}

const inputs = (page) => page.evaluate(() => [...document.querySelectorAll('input, textarea')].map((e) => e.value));

test('a token the browser cannot put into a header is forgotten, and the form asks for the token again', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(r); });
  await form(page);
  // a token pasted with one character that no header can carry (a euro sign): the browser refuses to build the request
  const typed = `${WRONG}€`;
  await page.locator('#st-token').fill(typed);
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toHaveClass(/\bbad\b/);
  const said = await page.locator('#st-msg').textContent();
  test.info().annotations.push({type: 'the form said', description: said});
  expect(sent.length, 'nothing left the browser').toBe(0);
  expect.soft(said, 'the message echoes the typed token').not.toContain(WRONG);
  expect.soft(said, 'the form says the daemon could not be reached, and no request was made').not.toContain('could not be reached');
  expect(said, 'the message says that the token cannot be sent').toContain('cannot be sent');
  expect((await inputs(page)).filter((v) => v.includes(WRONG)), 'an input of the page still holds the typed token').toEqual([]);
  // the person has to be able to give the right token: the field is back, as after a 401
  await expect.soft(page.locator('#st-token'), 'the token field is gone and the typed value is kept in memory: the right token cannot be given without a reload').toBeVisible({timeout: 3000});
  // pressing Start again must not run into the same kept value
  await page.click('#st-start');
  await expect.soft(page.locator('#st-msg'), 'the second press uses the kept value again').toContainText('needs its token', {timeout: 3000});
  expect(await added()).toBe(0);
});

for (const [how, close] of [['Close', (page) => page.click('#st-close')], ['Escape', (page) => page.keyboard.press('Escape')]]) {
  test(`a token typed into the form and not sent is in no input of the page once the form is closed with ${how}`, async ({page}) => {
    await form(page);
    await page.locator('#st-token').fill(TOKEN);
    await close(page);
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    expect((await inputs(page)).filter((v) => v.includes(TOKEN)), 'an input of the closed form still holds the token').toEqual([]);
  });
}

test('the token field is a password field outside any form, Enter sends nothing, and the token goes to the daemon as the bearer of the start only', async ({page, context}) => {
  const all = [];
  const consoleText = [];
  page.on('request', (r) => all.push(r));
  page.on('console', (m) => consoleText.push(m.text()));
  const pageErrors = [];
  page.on('pageerror', (e) => pageErrors.push(e.message));
  await form(page);
  const field = await page.locator('#st-token').evaluate((el) => ({type: el.type, autocomplete: el.autocomplete, name: el.name, form: !!el.closest('form'), value: el.getAttribute('value')}));
  expect(field).toEqual({type: 'password', autocomplete: 'off', name: '', form: false, value: null});
  expect(await page.evaluate(() => document.querySelectorAll('#startdlg form, #startdlg [name]').length)).toBe(0);

  // Enter in the token field, and in the intake: no command, no navigation
  const url = page.url();
  await page.locator('#st-token').fill(TOKEN);
  await page.locator('#st-token').press('Enter');
  await page.waitForTimeout(300);
  expect(all.filter(isStart).length).toBe(0);
  expect(page.url()).toBe(url);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);

  // a wrong token: 401, forgotten; the next press without typing sends nothing more
  await page.locator('#st-token').fill(WRONG);
  await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 401), page.click('#st-start')]);
  await expect(page.locator('#st-msg')).toContainText('401');
  expect(await page.locator('#st-msg').textContent()).not.toContain(WRONG);
  await expect(page.locator('#st-token')).toBeVisible();
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('needs its token');
  expect(all.filter(isStart).length, 'a refused token was sent again').toBe(1);

  // the right token
  await page.locator('#st-token').fill(TOKEN);
  await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 202), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await page.waitForTimeout(500);

  const origin = new URL(s.base).origin;
  for (const r of all) {
    const u = new URL(r.url());
    expect(u.origin, `a request left the daemon's origin: ${r.url()}`).toBe(origin);
    expect(r.url(), 'a URL carries the token').not.toContain(TOKEN);
    const h = await r.allHeaders();
    if (isStart(r)) {
      expect(h.cookie, 'the start carries the cookie').toBeUndefined();
    } else {
      expect(h.authorization, `${r.method()} ${u.pathname} carries the bearer`).toBeUndefined();
      expect(r.method() === 'POST' && u.pathname !== '/ui/login' ? r.postData() || '' : '', 'another body carries the token').not.toContain(TOKEN);
    }
    for (const [k, v] of Object.entries(h)) if (k !== 'authorization') expect(v, `the header ${k} carries the token`).not.toContain(TOKEN);
  }
  const starts = all.filter(isStart);
  expect(await starts.at(-1).allHeaders()).toMatchObject({authorization: `Bearer ${TOKEN}`});
  expect(starts.at(-1).postData()).not.toContain(TOKEN);

  const held = await page.evaluate(async (t) => {
    const attrs = [...document.querySelectorAll('*')].flatMap((e) => [...e.attributes].map((a) => `${a.name}=${a.value}`));
    const dbs = indexedDB.databases ? (await indexedDB.databases()).map((d) => d.name) : [];
    const places = {
      html: document.documentElement.outerHTML, text: document.documentElement.textContent, attrs: attrs.join('\n'),
      inputs: [...document.querySelectorAll('input, textarea, select')].map((e) => e.value).join('\n'),
      local: JSON.stringify(Object.entries(localStorage)), session: JSON.stringify(Object.entries(sessionStorage)), cookie: document.cookie,
      url: location.href, state: JSON.stringify(history.state), name: window.name, title: document.title,
      entries: performance.getEntries().map((e) => e.name).join('\n'), dbs: dbs.join('\n'),
      globals: Object.keys(window).filter((k) => { try { return typeof window[k] === 'string' && window[k].includes(t); } catch { return false; } }).join(','),
    };
    return Object.entries(places).filter(([, v]) => String(v).includes(t)).map(([k]) => k);
  }, TOKEN);
  expect(held, 'the token is held outside the memory of the module').toEqual([]);
  expect((await context.cookies()).filter((c) => c.value.includes(TOKEN))).toEqual([]);
  expect(consoleText.filter((t) => t.includes(TOKEN) || t.includes(WRONG)), 'a console line carries a token').toEqual([]);
  expect(s.log(), 'a log line of the daemon carries a token').not.toContain(TOKEN);
  expect(s.log()).not.toContain(WRONG);
  expect(pageErrors).toEqual([]);
  expect(await added()).toBe(1);
});

test('the cookie of the login starts no run: a start from the page without the bearer is 401, whatever the cookie', async ({page}) => {
  await form(page);
  const answers = await page.evaluate(async () => {
    const out = [];
    for (const credentials of ['include', 'same-origin', 'omit']) {
      const r = await fetch('/daemon/commands/submit-run', {method: 'POST', credentials, headers: {'content-type': 'application/json'}, body: JSON.stringify({runbook_id: 'sdc', intake: 'an invented intake', confinement: 'linux'})});
      out.push([credentials, r.status, await r.text()]);
    }
    // the read the cookie is for
    out.push(['read', (await fetch('/daemon/views/runbooks')).status, '']);
    return out;
  });
  expect(answers.slice(0, 3).map((a) => a[1])).toEqual([401, 401, 401]);
  expect(new Set(answers.slice(0, 3).map((a) => a[2])).size, 'the refusals differ by the cookie').toBe(1);
  expect(answers[3][1]).toBe(200);
  expect(await added()).toBe(0);
});
