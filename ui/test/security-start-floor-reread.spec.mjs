// Security review of the unit daemon-page-starts-a-run: the floor the start form checks against while it
// reads the floor again. No answer is forged here: a daemon with the floor `local` serves the page and the form is
// opened once; that daemon is ended and one with the floor `linux` is started on the same address and state
// directory (an operator who raises the floor); the form is opened again and Start is pressed while the read of
// the floor is on its way (the route only delays the daemon's own answer).
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
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';
const later = (ms) => new Promise((r) => setTimeout(r, ms));
let dir = null;
const procs = [];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.afterAll(() => {
  for (const p of procs) p.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

function serve(listen, floor) {
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
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1]}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

const rowsOf = (base, view) => fetch(`${base}/daemon/views/${view}`).then((r) => r.json()).then((j) => j.rows);

test('the daemon comes back with a higher floor under an open page: a start pressed while the form reads the floor again does not leave the page with a level below it', async ({page}, info) => {
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  fs.mkdirSync(root, {recursive: true});
  dir = fs.mkdtempSync(path.join(root, '.pw-start-floor-reread-'));
  const first = await serve('127.0.0.1:0', 'local');
  const sent = [];
  page.on('request', (r) => { if (isStart(r)) sent.push(JSON.parse(r.postData())); });
  await page.goto(`${first.base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await page.click('#startbtn');
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect(page.locator('#st-note')).toContainText('floor of this daemon is local');
  await page.click('#st-close');

  // the daemon is ended and comes back on the same address with the floor linux
  const gone = new Promise((r) => first.proc.once('exit', r));
  first.proc.kill();
  await gone;
  await expect.poll(() => fetch(`${first.base}/daemon/views/configuration`).then(() => 'answers', () => 'gone')).toBe('gone');
  let second = null;
  for (let i = 0; i < 20 && !second; i++) second = await serve(new URL(first.base).host, 'linux').catch(async () => { await later(250); return null; });
  expect(second, 'the second daemon did not start on the address of the first').not.toBeNull();
  expect(second.base).toBe(first.base);
  expect((await rowsOf(second.base, 'configuration'))[0].confinement_floor).toBe('linux');
  const n0 = (await rowsOf(second.base, 'runs')).length;

  // the form is opened again on the page that was not loaded again; the daemon's own answer about its floor takes 2.5 s
  await page.route(CONFIGURATION, async (route) => { await later(2500); return route.continue(); });
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  const offered = await page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value));
  info.annotations.push({type: 'measured', description: `levels offered while the floor is read again: ${JSON.stringify(offered)}`});
  console.log(`[security-start-floor-reread] levels offered while the floor is read again: ${JSON.stringify(offered)}`);
  if (offered.includes('local')) {
    await page.locator('#st-runbook').selectOption('sdc');
    await page.locator('#st-intake').fill('an invented intake pressed before the floor was read again');
    await page.locator('#st-confinement').selectOption('local');
    const answer = page.waitForResponse((r) => isStart(r.request()), {timeout: 8000}).catch(() => null);
    await page.locator('#st-start').click({timeout: 2000}).catch(() => {});
    const response = await answer;
    const line = response ? `a start left the page with ${sent[0]?.confinement} and the daemon answered ${response.status()} ${(await response.json()).outcome}` : 'no start left the page';
    info.annotations.push({type: 'measured', description: line});
    console.log(`[security-start-floor-reread] ${line}`);
  }
  // after the read: only linux is offered and nothing was started
  await expect(page.locator('#st-intake')).toBeFocused();
  await expect.poll(() => page.locator('#st-confinement option').evaluateAll((els) => els.map((e) => e.value))).toEqual(['linux']);
  expect((await rowsOf(second.base, 'runs')).length - n0, 'a run below the floor of the daemon').toBe(0);
  expect(sent.filter((b) => b.confinement === 'local'), 'a start below the floor of the daemon left the page').toEqual([]);
});
