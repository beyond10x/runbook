// Security review, pass 1, of the unit daemon-page-starts-a-run: one press of Start is one run.
// A daemon of its own (stub runtime, no token, floor linux) on 127.0.0.1:0; invented texts; no model call.
// The state directory lies under SEC_SCRATCH when it is set (the reviewer's scratch), else under node_modules.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
let s, dir;

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
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1]}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-sec-idem-'));
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  // a daemon for each test: its run list starts empty
  s = await start(`d-${info.testId}-${info.retry}`);
});
test.afterEach(() => { if (s?.proc.exitCode == null) s?.proc.kill(); });
test.afterAll(() => { if (dir) fs.rmSync(dir, {recursive: true, force: true}); });

async function form(page) {
  await page.goto(`${s.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await page.locator('#st-runbook').selectOption('sdc');
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
  expect((await runs()).length, 'the daemon took the first command').toBe(1);
  // the form kept everything and says the daemon was not reached: the person presses Start again
  await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  expect(sent.length).toBe(2);
  expect(sent[1].intake).toBe(sent[0].intake);
  const after = await runs();
  expect(after.map((r) => r.run_id).length, `one intake, pressed again after a lost answer, made ${after.length} runs (keys ${sent.map((b) => b.idempotency_key).join(', ')})`).toBe(1);
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
  expect((await runs()).length).toBe(1);
  // the next form can start again
  await page.click('#startbtn');
  await expect(page.locator('#st-start')).toBeEnabled();
});

test('a refused start makes no run, and the start after it carries another key', async ({page}) => {
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await form(page);
  await page.locator('#st-intake').evaluate((el) => { el.value = 'a'.repeat(1048577); });
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('intake-too-large');
  expect((await runs()).length).toBe(0);
  await page.locator('#st-intake').fill('another invented intake');
  await Promise.all([page.waitForResponse((r) => isStart(r.request()) && r.status() === 202), page.click('#st-start')]);
  expect(sent.length).toBe(2);
  expect(sent[0].idempotency_key).not.toBe(sent[1].idempotency_key);
  const after = await runs();
  expect(after.length).toBe(1);
  expect(after[0].intake ?? 'another invented intake').toBe('another invented intake');
});
