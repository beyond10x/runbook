// Security review, pass 1, of the unit daemon-page-starts-a-run: one filled form is one run. The idempotency key
// belongs to the filled form: it stays the same for a press repeated after an answer that did not arrive and
// changes when the daemon answered (accepted or refused) or when the runbook, the intake or the confinement changed.
// One daemon for the file (stub runtime, no token, floor linux) on 127.0.0.1:0, shared by its tests: each test counts
// the runs it adds. Invented texts; no model call.
// The state directory lies under SEC_SCRATCH when it is set (the reviewer's scratch), else under node_modules.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
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
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1]}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const runs = () => fetch(`${s.base}/daemon/views/runs`).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-start-idempotency-'));
  s = await start('idempotency');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});
test.afterAll(() => { if (s?.proc.exitCode == null) s?.proc.kill(); if (dir) fs.rmSync(dir, {recursive: true, force: true}); });

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
