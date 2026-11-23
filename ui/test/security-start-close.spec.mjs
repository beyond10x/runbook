// Security review, pass 2, of the unit daemon-page-starts-a-run: the token field of the start form when the
// form closes by another way than its own buttons, and tokens that a header cannot carry.
// One daemon for the file, with a token (stub runtime, floor linux) on 127.0.0.1:0; each test counts the runs
// it adds. Invented texts; no model call.
// The state directory lies under SEC_SCRATCH when it is set (the reviewer's scratch), else under node_modules.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import {randomBytes} from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const TOKEN = `tok-${randomBytes(24).toString('hex')}`;
const TYPED = 'typed-and-not-sent-0123456789-0123456789';
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
const inputs = (page) => page.evaluate(() => [...document.querySelectorAll('input, textarea')].map((e) => e.value));

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-start-close-'));
  s = await start('close', [], {RUNBOOK_TOKEN: TOKEN});
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});
test.afterAll(() => { if (s?.proc.exitCode == null) s?.proc.kill(); if (dir) fs.rmSync(dir, {recursive: true, force: true}); });

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
