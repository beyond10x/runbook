// Security review, pass 1, of the unit daemon-page-starts-a-run: what the start form shows is text, and the
// floor is the daemon's. A daemon of its own (stub runtime, no token, floor linux) on 127.0.0.1:0. The rows of
// a view and the refusals are answered by the test where the stub cannot make them hostile; invented texts.
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
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], log: () => out + err}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const rows = (view, q = '') => fetch(`${s.base}/daemon/views/${view}${q}`).then((r) => r.json()).then((j) => j.rows);
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const HOSTILE = '<img src=x onerror="window.__ran=1"><script>window.__ran=2</script><b id="sec-b">b</b><svg onload="window.__ran=3">"\'>&amp;';
// what markup would leave behind, anywhere in the page
const made = (page) => page.evaluate(() => ({ran: window.__ran ?? null, nodes: document.querySelectorAll('img[src="x"], #sec-b, svg[onload], script:not([src]):not([type]), a[href^="javascript:"], [onerror], [onload], [onclick]').length - document.querySelectorAll('head script:not([src]):not([type])').length}));

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-sec-content-'));
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await start(`d-${info.testId}-${info.retry}`);
});
test.afterEach(() => { if (s?.proc.exitCode == null) s?.proc.kill(); });
test.afterAll(() => { if (dir) fs.rmSync(dir, {recursive: true, force: true}); });

function watch(page) {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('dialog', (d) => { problems.push(`dialog ${d.message()}`); d.dismiss(); });
  return problems;
}

async function enter(page) {
  await page.goto(`${s.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  // open() ends by putting the focus into the intake: the floor and the list are the ones it read
  await expect(page.locator('#st-intake')).toBeFocused();
}

test('a runbook name, a version, a runtime and a refusal that look like markup are shown as text', async ({page}) => {
  const problems = watch(page);
  await enter(page);
  const baseline = await made(page);
  await page.route('**/daemon/views/runbooks', (route) => route.fulfill({json: {rows: [
    {runbook_id: HOSTILE, runbook_version: HOSTILE, runtime: 'stub'},
    {runbook_id: 'javascript:window.__ran=4', runbook_version: '"><svg onload=window.__ran=5>', runtime: HOSTILE},
    {runbook_id: '--help', runtime: 'stub'},
    {runbook_id: '../../etc/hostname', runbook_version: null, runtime: null},
    {runbook_id: 7, runbook_version: 8, runtime: 9},
  ]}}));
  await form(page);
  const options = await page.locator('#st-runbook option').evaluateAll((els) => els.map((e) => [e.value, e.textContent, e.children.length]));
  expect(options).toEqual([
    [HOSTILE, `${HOSTILE} · version ${HOSTILE}`, 0],
    ['javascript:window.__ran=4', 'javascript:window.__ran=4 · version "><svg onload=window.__ran=5>', 0],
    ['--help', '--help', 0],
    ['../../etc/hostname', '../../etc/hostname', 0],
    ['7', '7 · version 8', 0],
  ]);
  expect(await page.locator('#st-note').evaluate((e) => e.children.length)).toBe(0);

  // refusals of every shape the form reads, each with markup in every member
  const answers = [
    {status: 422, json: {outcome: HOSTILE, error: HOSTILE, payload: {run_id: HOSTILE}, published: []}},
    {status: 400, json: {refused: HOSTILE, message: HOSTILE}},
    {status: 500, contentType: 'text/html', body: HOSTILE},
    {status: 202, json: {outcome: 'submitted', published: [{event: HOSTILE, payload: {run_id: HOSTILE}}]}},
    {status: 202, json: {outcome: HOSTILE, published: HOSTILE}},
  ];
  let n = 0;
  await page.route('**/daemon/commands/submit-run', (route) => route.fulfill(answers[n++]));
  await page.locator('#st-runbook').selectOption({index: 0});
  for (const a of answers) {
    if (!(await page.locator('#startdlg').evaluate((d) => d.open))) await form(page);
    await page.locator('#st-intake').fill(HOSTILE);
    await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
    await expect(page.locator('#st-start')).toBeEnabled();
    const msg = await page.locator('#st-msg').evaluate((e) => [e.children.length, e.textContent]);
    expect(msg[0], `a refusal of status ${a.status} became elements: ${msg[1]}`).toBe(0);
    if (a.status === 422 || a.status === 400) expect(msg[1]).toContain(HOSTILE);
  }
  await page.waitForTimeout(300);
  expect(await made(page)).toEqual(baseline);
  expect(problems).toEqual([]);
  expect(await rows('runs')).toEqual([]);
});

test('the floor is the daemon\'s: a page told a lower floor is refused by the daemon, and an unread floor offers only the strictest level', async ({page}) => {
  const problems = watch(page);
  await enter(page);
  expect((await rows('configuration'))[0].confinement_floor).toBe('linux');
  const eventsBefore = await rows('daemon-events', '?after_position=0');
  const values = () => page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value));

  // the configuration cannot be read
  await page.route('**/daemon/views/configuration', (route) => route.abort());
  await form(page);
  await expect.poll(values).toEqual(['linux']);
  await page.click('#st-close');

  // floors the form does not know
  for (const floor of ['oci', '', null, HOSTILE]) {
    await page.unroute('**/daemon/views/configuration');
    await page.route('**/daemon/views/configuration', (route) => route.fulfill({json: {rows: [{daemon_id: 'd', confinement_floor: floor, principal: 'agent:x', started_at: '2026-01-01T00:00:00Z'}]}}));
    await form(page);
    await expect.poll(values, `floor ${JSON.stringify(floor)}`).toEqual(['linux']);
    await page.click('#st-close');
  }

  // the page is told the floor is local, on a daemon whose floor is linux
  await page.unroute('**/daemon/views/configuration');
  await page.route('**/daemon/views/configuration', (route) => route.fulfill({json: {rows: [{daemon_id: 'd', confinement_floor: 'local', principal: 'agent:x', started_at: '2026-01-01T00:00:00Z'}]}}));
  await form(page);
  await expect.poll(values).toEqual(['local', 'linux']);
  await page.locator('#st-confinement').selectOption('local');
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake below the floor');
  const [response] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(JSON.parse(response.request().postData()).confinement).toBe('local');
  expect(response.status()).toBe(409);
  await expect(page.locator('#st-msg')).toContainText('below-floor (409)');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  expect(await rows('runs')).toEqual([]);
  expect(await rows('daemon-events', '?after_position=0')).toEqual(eventsBefore);
  expect(problems).toEqual([]);
});

test('an intake with control characters, markup and 200 000 characters reaches the daemon unchanged and the page that shows the run makes no element of it', async ({page}) => {
  const problems = watch(page);
  await enter(page);
  const baseline = await made(page);
  const intake = `line one\nline two\ttab \u0000 nul \u0007 bell \u001b[31m esc ‮ rtl 😀 </textarea></dialog>${HOSTILE} --confinement local ; $(id) \`id\` ${'x'.repeat(200000)}`;
  await form(page);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').evaluate((el, v) => { el.value = v; }, intake);
  const [response] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(response.status()).toBe(202);
  const sent = JSON.parse(response.request().postData());
  expect(sent.intake).toBe(intake);
  expect(sent.confinement).toBe('linux');
  expect(Object.keys(sent).sort()).toEqual(['confinement', 'idempotency_key', 'intake', 'runbook_id']);
  const run = (await response.json()).published[0].payload.run_id;
  await expect(page.locator('#runpick')).toContainText(run.slice(-8));
  const row = (await rows('run', `?run_id=${run}`))[0];
  expect(row.intake).toBe(intake);
  expect(row.confinement).toBe('linux');
  await page.click('#runpick');
  await page.waitForTimeout(500);
  expect(await made(page)).toEqual(baseline);
  expect(problems).toEqual([]);
});
