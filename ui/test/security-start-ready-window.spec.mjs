// Security review of the unit daemon-page-starts-a-run: the windows of the start form in which the page
// holds a floor that is not the daemon's any more. No answer is forged: a daemon with the floor `local` serves the
// page, is ended and comes back on the same address and state directory with the floor `linux`; routes only hold
// an answer of a daemon until the test opens a gate.
// Daemons of this file's own (stub runtime, no token), ended by their handle. Invented texts; no model call; no
// request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const CONFIGURATION = '**/daemon/views/configuration';
const RUNBOOKS = '**/daemon/views/runbooks';
const SUBMIT = '**/daemon/commands/submit-run';
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const later = (ms) => new Promise((r) => setTimeout(r, ms));
const held = () => { let open; const wait = new Promise((r) => { open = r; }); return {wait, open}; };
const dirs = [];
const procs = [];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.afterAll(() => {
  for (const p of procs) p.kill();
  for (const d of dirs) fs.rmSync(d, {recursive: true, force: true});
});

function serve(dir, listen, floor) {
  return new Promise((resolve, reject) => {
    fs.mkdirSync(path.join(dir, 'empty'), {recursive: true});
    const proc = spawnChild(bin, ['serve', '--ui', '--principal', 'agent:test-daemon', '--listen', listen, '--state-dir', path.join(dir, 'state'), '--min-confinement', floor], {cwd: path.join(dir, 'empty'), env: {...process.env, HOME: dir, RUNBOOK_TOKEN: ''}, stdio: ['ignore', 'pipe', 'pipe']});
    procs.push(proc);
    let out = '', err = '';
    const timer = setTimeout(() => { proc.kill(); reject(new Error(`no listening line in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { err += d; });
    proc.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], dir}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const rowsOf = (base, view) => fetch(`${base}/daemon/views/${view}`).then((r) => r.json()).then((j) => j.rows);

// a daemon with the floor local, and the page it serves with the form opened once (floor local read) and closed
async function begin(page) {
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  const dir = fs.mkdtempSync(path.join(root, '.pw-start-ready-window-'));
  dirs.push(dir);
  const d = await serve(dir, '127.0.0.1:0', 'local');
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await page.goto(`${d.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect(page.locator('#st-note')).toContainText('floor of this daemon is local');
  return {d, sent};
}

async function stop(d) {
  const gone = new Promise((r) => d.proc.once('exit', r));
  d.proc.kill();
  await gone;
  await expect.poll(() => fetch(`${d.base}/daemon/views/configuration`).then(() => 'answers', () => 'gone')).toBe('gone');
}

// the daemon ended and back on the same address with the floor linux
async function higher(d) {
  await stop(d);
  let next = null;
  for (let i = 0; i < 20 && !next; i++) next = await serve(d.dir, new URL(d.base).host, 'linux').catch(async () => { await later(250); return null; });
  expect(next, 'the second daemon did not start on the address of the first').not.toBeNull();
  expect(next.base).toBe(d.base);
  expect((await rowsOf(next.base, 'configuration'))[0].confinement_floor).toBe('linux');
  return next;
}

const offered = (page) => page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value));
const forced = (page) => page.locator('#st-start').evaluate((b) => { b.disabled = false; b.click(); });
const below = (sent) => sent.filter((b) => b.confinement === 'local');

test('while the form reads the floor again, no press sends: the button, the button enabled by script and Enter in the intake', async ({page}) => {
  const {d, sent} = await begin(page);
  await page.click('#st-close');
  const now = await higher(d);
  const n0 = (await rowsOf(now.base, 'runs')).length;
  const gate = held();
  await page.route(CONFIGURATION, async (route) => { await gate.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  expect(await offered(page)).toEqual(['local', 'linux']);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake pressed while the floor is read');
  await page.locator('#st-confinement').selectOption('local');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-start').click({timeout: 1000}).catch(() => {});
  await page.locator('#st-intake').press('Enter');
  await page.locator('#st-intake').press('Control+Enter');
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('still reading the floor');
  await expect(page.locator('#st-start'), 'the press by script left Start enabled while the floor is read').toBeDisabled();
  await page.waitForTimeout(300);
  expect(sent.length, 'a start left the page while the form read the floor').toBe(0);
  // the floor of this opening arrives: the level below it is dropped, the reading message goes
  gate.open();
  await expect.poll(() => offered(page)).toEqual(['linux']);
  await expect(page.locator('#st-intake')).toBeFocused();
  expect(await page.locator('#st-confinement').evaluate((el) => el.value)).toBe('');
  await expect(page.locator('#st-start')).toBeDisabled();
  await expect(page.locator('#st-msg')).not.toContainText('still reading');
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('Pick a confinement');
  // a level below the floor put into the select by hand
  await page.locator('#st-confinement').evaluate((el) => { const o = document.createElement('option'); o.value = 'local'; o.textContent = 'local'; el.append(o); el.value = 'local'; });
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('below the floor');
  await page.waitForTimeout(300);
  expect(below(sent)).toEqual([]);
  expect(sent.length).toBe(0);
  expect((await rowsOf(now.base, 'runs')).length - n0).toBe(0);
});

test('a read of an older opening that ends after a newer opening began enables nothing and changes no floor', async ({page}) => {
  const {d, sent} = await begin(page);
  await page.click('#st-close');

  // the older answer ends first, the newer read is still on its way: Start stays disabled
  const a = held();
  const b = held();
  let n = 0;
  const given = [];
  await page.route(CONFIGURATION, async (route) => { const mine = n++; await (mine === 0 ? a : b).wait; const answer = await route.fetch(); await route.fulfill({response: answer}); given.push(mine); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => n).toBe(1);
  await page.keyboard.press('Escape');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => n).toBe(2);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake of two openings in quick succession');
  await page.locator('#st-confinement').selectOption('linux');
  a.open();
  await expect.poll(() => given).toEqual([0]);
  await page.waitForTimeout(400);
  await expect(page.locator('#st-start'), 'the read of the older opening enabled Start of the newer one').toBeDisabled();
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('still reading the floor');
  await page.waitForTimeout(200);
  expect(sent.length, 'a start left the page before the newer opening had read the floor').toBe(0);
  b.open();
  await expect(page.locator('#st-start')).toBeEnabled();
  await page.click('#st-close');
  await page.unroute(CONFIGURATION);

  // the older answer (floor local, of the daemon before it came back) ends after the newer read (floor linux)
  const old = held();
  let taken = false;
  let delivered = false;
  n = 0;
  await page.route(CONFIGURATION, async (route) => {
    if (n++ > 0) return route.continue();
    const answer = await route.fetch();
    const body = await answer.text();
    taken = true;
    await old.wait;
    await route.fulfill({status: answer.status(), contentType: 'application/json', body});
    delivered = true;
  });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => taken).toBe(true);
  await page.keyboard.press('Escape');
  const now = await higher(d);
  const n0 = (await rowsOf(now.base, 'runs')).length;
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => offered(page)).toEqual(['linux']);
  await expect(page.locator('#st-intake')).toBeFocused();
  old.open();
  await expect.poll(() => delivered).toBe(true);
  await page.waitForTimeout(400);
  expect(await offered(page), 'the read of the older opening put the older floor back').toEqual(['linux']);
  await expect(page.locator('#st-note')).toContainText('floor of this daemon is linux');
  await page.locator('#st-confinement').evaluate((el) => { const o = document.createElement('option'); o.value = 'local'; o.textContent = 'local'; el.append(o); el.value = 'local'; });
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('below the floor');
  await page.waitForTimeout(300);
  expect(below(sent)).toEqual([]);
  expect((await rowsOf(now.base, 'runs')).length - n0).toBe(0);
});

test('a floor read that fails and a later one that succeeds: the level of the first opening is offered at no time after it', async ({page}) => {
  const {d, sent} = await begin(page);
  await page.click('#st-close');
  await stop(d);
  // the daemon is away: the form offers only the strictest level, presets none, and has no runbook to send
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect(page.locator('#st-note')).toContainText('could not be read');
  expect(await offered(page)).toEqual(['linux']);
  expect(await page.locator('#st-confinement').evaluate((el) => el.value)).toBe('');
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-intake').fill('an invented intake while the daemon is away');
  await forced(page);
  await page.waitForTimeout(200);
  expect(sent.length).toBe(0);
  await page.click('#st-close');
  // the daemon is back with the floor linux; the read of the next opening is held
  let now = null;
  for (let i = 0; i < 20 && !now; i++) now = await serve(d.dir, new URL(d.base).host, 'linux').catch(async () => { await later(250); return null; });
  expect(now).not.toBeNull();
  const n0 = (await rowsOf(now.base, 'runs')).length;
  const gate = held();
  await page.route(CONFIGURATION, async (route) => { await gate.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  expect(await offered(page), 'the level of an opening before the failed read is offered again').toEqual(['linux']);
  await expect(page.locator('#st-start')).toBeDisabled();
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start')).toBeDisabled();
  gate.open();
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect(page.locator('#st-note')).toContainText('floor of this daemon is linux');
  await page.locator('#st-runbook').selectOption('sdc');
  await expect(page.locator('#st-start')).toBeEnabled();
  const [answer] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(answer.status()).toBe(202);
  expect(sent.map((b) => b.confinement)).toEqual(['linux']);
  expect((await rowsOf(now.base, 'runs')).length - n0).toBe(1);
});

test('an opening while a start is on its way, and the daemon back with a higher floor: nothing below it is sent after it came back', async ({page}) => {
  const {d, sent} = await begin(page);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake taken at local before the daemon came back');
  await page.locator('#st-confinement').selectOption('local');
  const gate = held();
  let taken = false;
  let n = 0;
  await page.route(SUBMIT, async (route) => {
    if (n++ > 0) return route.continue();
    const answer = await route.fetch();
    const body = await answer.text();
    taken = true;
    await gate.wait;
    return route.fulfill({status: answer.status(), contentType: 'application/json', body});
  });
  await page.click('#st-start');
  await expect.poll(() => taken).toBe(true);
  await page.click('#st-close');
  const now = await higher(d);
  const n0 = (await rowsOf(now.base, 'runs')).length;
  const before = sent.length;
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => offered(page)).toEqual(['linux']);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-confinement').selectOption('linux');
  await expect(page.locator('#st-start'), 'a second command can leave while the first is on its way').toBeDisabled();
  gate.open();
  await expect(page.locator('#st-start')).toBeEnabled();
  expect(await offered(page)).toEqual(['linux']);
  await page.locator('#st-confinement').evaluate((el) => { const o = document.createElement('option'); o.value = 'local'; o.textContent = 'local'; el.append(o); el.value = 'local'; });
  await forced(page);
  await expect(page.locator('#st-msg')).toContainText('below the floor');
  await page.waitForTimeout(300);
  expect(below(sent.slice(before)), 'a start below the floor left the page after the daemon came back').toEqual([]);
  expect((await rowsOf(now.base, 'runs')).length - n0, 'a run started after the daemon came back').toBe(0);
});

test('measured: the daemon comes back with a higher floor while the form is open and has read its floor (after the floor paint, before the list is read)', async ({page}, info) => {
  const {d, sent} = await begin(page);
  await page.click('#st-close');
  const list = held();
  await page.route(RUNBOOKS, async (route) => { await list.wait; return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake pressed after the daemon came back');
  await page.locator('#st-confinement').selectOption('local');
  // the floor of this opening (local) is read and painted: Start is enabled, the list is still on its way
  await expect(page.locator('#st-start')).toBeEnabled();
  const now = await higher(d);
  const n0 = (await rowsOf(now.base, 'runs')).length;
  const answer = page.waitForResponse((r) => isStart(r.request()), {timeout: 8000}).catch(() => null);
  await page.locator('#st-start').click({timeout: 2000}).catch(() => {});
  const response = await answer;
  const line = response ? `a start left the page with ${sent[0]?.confinement} and the daemon answered ${response.status()} ${(await response.json()).outcome}` : 'no start left the page';
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-ready-window] the daemon came back higher after the floor of this opening was read: ${line}`);
  list.open();
  if (response) expect(response.status(), 'the daemon took a start below its floor').toBe(409);
  expect((await rowsOf(now.base, 'runs')).length - n0, 'a run below the floor of the daemon').toBe(0);
});

test('measured: Start of the closed form pressed by script after the daemon came back with a higher floor', async ({page}, info) => {
  const {d, sent} = await begin(page);
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-intake').fill('an invented intake of a form that is closed');
  await page.locator('#st-confinement').selectOption('local');
  await page.click('#st-close');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  const now = await higher(d);
  const n0 = (await rowsOf(now.base, 'runs')).length;
  const reach = {visible: await page.locator('#st-start').isVisible(), disabled: await page.locator('#st-start').isDisabled()};
  const answer = page.waitForResponse((r) => isStart(r.request()), {timeout: 3000}).catch(() => null);
  await page.locator('#st-start').evaluate((b) => b.click());
  const response = await answer;
  const line = `the button of the closed form is ${reach.visible ? 'visible' : 'not visible'} and ${reach.disabled ? 'disabled' : 'enabled'}; ${response ? `a start left the page with ${sent[0]?.confinement} and the daemon answered ${response.status()} ${(await response.json()).outcome}` : 'no start left the page'}`;
  info.annotations.push({type: 'measured', description: line});
  console.log(`[security-start-ready-window] ${line}`);
  expect(reach.visible, 'a person can reach the button of the closed form').toBe(false);
  if (response) expect(response.status(), 'the daemon took a start below its floor').toBe(409);
  expect((await rowsOf(now.base, 'runs')).length - n0, 'a run below the floor of the daemon').toBe(0);
});
