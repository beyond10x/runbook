// Security review of slice 2 (answering a decision from the page the daemon serves): the invariants
// `daemon-decide.spec.mjs` does not measure. Each daemon listens on 127.0.0.1:0 with a state directory of its own.
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
const servers = {};
let dir;

function start(name, env) {
  return new Promise((resolve, reject) => {
    fs.mkdirSync(path.join(dir, name, 'empty'), {recursive: true});
    const proc = spawnChild(bin, ['serve', '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, name, 'state')], {cwd: path.join(dir, name, 'empty'), env: {...process.env, HOME: path.join(dir, name), RUNBOOK_TOKEN: '', ...env}, stdio: ['ignore', 'pipe', 'pipe']});
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

const headers = (s, extra = {}) => ({...(s.token ? {authorization: `Bearer ${s.token}`} : {}), ...extra});
const post = (s, name, body) => fetch(`${s.base}/daemon/commands/${name}`, {method: 'POST', headers: headers(s, {'content-type': 'application/json'}), body: JSON.stringify(body)}).then((r) => r.json());
const rows = (s, view, q = '') => fetch(`${s.base}/daemon/views/${view}${q}`, {headers: headers(s)}).then((r) => r.json()).then((j) => j.rows);

async function waiting(s, intake) {
  const r = await post(s, 'submit-run', {runbook_id: 'stub-decision', intake});
  const id = r.published[0].payload.run_id;
  await expect.poll(async () => (await rows(s, 'open-decisions', `?run_id=${id}`)).length, {timeout: 15000}).toBe(2);
  return id;
}

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemons: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-decide-review-'));
  servers.open = await start('open', {});
  servers.guarded = await start('guarded', {RUNBOOK_TOKEN: TOKEN});
});
test.afterAll(() => {
  for (const s of Object.values(servers)) if (s.proc.exitCode == null) s.proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

async function enter(page, s) {
  await page.goto(`${s.base}/`);
  if (s.token) {
    await page.locator('input[name=token]').fill(s.token);
    await page.getByRole('button', {name: 'Open'}).click();
  }
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function drawer(page, id, cards = 2) {
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${id.slice(-8)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(id.slice(-8));
  await expect(page.locator('#deccount')).toHaveText(String(cards));
  await page.click('#decbtn');
  await expect(page.locator('#drawer .dcard')).toHaveCount(cards);
}

const card = (page, n) => page.locator('#drawer .dcard').nth(n);
const reqs = (page) => { const all = []; page.on('request', (r) => all.push(r)); return all; };
const isCommand = (r) => r.method() === 'POST' && new URL(r.url()).pathname.startsWith('/daemon/commands/');

// Invariant 2: the token the person typed is in no input of the DOM once the page has it. The page
// empties the field "as soon as it is read" (decide.js `answer`), but it reads the field after two
// early returns (no option or text, no name): a token typed first, a name forgotten, stays in the field.
for (const [what, act] of [
  ['no name is given', async (page) => { await card(page, 0).locator('button[data-answer="A"]').click(); }],
  ['an empty free text is sent', async (page) => { await page.locator('#dd-by').fill('someone'); await card(page, 0).locator('button[data-send]').click(); }],
]) {
  test(`a token typed into the page is not left filled in the DOM when ${what}`, async ({page}) => {
    const s = servers.guarded;
    const id = await waiting(s, `Approve and leave the token (${what})`);
    await enter(page, s);
    await drawer(page, id);
    await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'needed');
    await page.locator('#dd-token').fill(s.token);
    await act(page);
    await expect(page.locator('#dd-msg')).not.toHaveText('');
    const values = await page.evaluate(() => [...document.querySelectorAll('input, textarea')].map((e) => e.value));
    expect(values.filter((v) => v.includes(TOKEN)), 'an input of the DOM still holds the token').toEqual([]);
  });
}

// Invariant 3: every field of a row the daemon sends is text. The stub preset only makes the question and the
// `does` of an option from the intake; a runtime may send the others, so the rows are served hostile here
// (the browser's request for the view is answered by the test) and the page is looked at.
test('hostile fields of an open decision, in every place the page shows or stores one, are text', async ({page}) => {
  const s = servers.open;
  const id = await waiting(s, 'a real run, to select');
  const evil = '"><img src=x onerror="window.__pwned=1"><svg onload="window.__pwned=2">';
  const letter = `A"] , button[data-send], [x="${evil}`;
  const hostile = {
    request_id: `q"1${evil}`, run_id: id, opened_at: `2026-01-01T00:00:00${evil}`,
    case_id: `case${evil}`, action: `action${evil}`, capability: `capability${evil}`,
    question: `<script>window.__pwned=3</script>${evil}`,
    options: [{letter, does: evil, cost: evil, consequence: {admits: [evil], blocks: [evil]}}, {letter: 'B', does: 'plain', cost: 'none', consequence: {admits: [], blocks: []}}],
  };
  await page.route('**/daemon/views/open-decisions**', (route) => route.fulfill({json: {rows: [hostile]}}));
  const sentBodies = [];
  await page.route('**/daemon/commands/answer-decision', (route) => { sentBodies.push(JSON.parse(route.request().postData())); route.fulfill({status: 202, json: {outcome: 'answered', published: []}}); });
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message));
  await enter(page, s);
  await drawer(page, id, 1);
  await expect(card(page, 0)).toContainText('<script>');
  for (const field of ['case_id', 'action', 'capability']) {
    await expect(card(page, 0).locator(`[data-field=${field}]`)).toContainText(`${field === 'case_id' ? 'case' : field}${evil}`);
  }
  expect(await page.locator('#drawer img, #drawer svg, #drawer script, #drawer [onerror], #drawer [onload]').count()).toBe(0);
  expect(await page.locator('#drawer [data-run]').count()).toBe(1);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  await page.locator('#dd-by').fill('reviewer');
  await card(page, 0).locator('button[data-answer]').first().click();
  await expect.poll(() => sentBodies.length).toBe(1);
  expect(sentBodies[0]).toEqual({run_id: id, request_id: hostile.request_id, answer: letter, answered_by: 'reviewer'});
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  expect(problems).toEqual([]);
});

// Invariant 8: a daemon that refuses every read (a token the cookie no longer proves, a Host the daemon
// refuses) is asked at a bounded rate, and a refused command is not sent again by the page.
async function rate(page, ms) {
  const all = reqs(page);
  const before = all.length;
  await page.waitForTimeout(ms);
  return all.slice(before);
}

test('a daemon that answers 401 to every read is polled at a bounded rate and no command is sent again', async ({page, context}) => {
  const s = servers.guarded;
  const id = await waiting(s, 'Approve while reads are refused');
  await enter(page, s);
  await drawer(page, id);
  await context.clearCookies();
  await page.locator('#dd-by').fill('refused');
  await page.locator('#dd-token').fill('not-the-token');
  const all = reqs(page);
  await card(page, 0).locator('button[data-answer="A"]').click();
  await expect(page.locator('#dd-msg')).toContainText('401');
  const seen = await rate(page, 5000);
  const perSecond = seen.length / 5;
  console.log(`401 REQUESTS in 5 s: ${seen.length} (${perSecond.toFixed(1)}/s): ${[...new Set(seen.map((r) => new URL(r.url()).pathname))].join(' ')}`);
  expect(perSecond).toBeLessThanOrEqual(4);
  expect(all.filter(isCommand)).toHaveLength(1);
});

test('a daemon that answers 403 to every request is polled at a bounded rate and no command is sent again', async ({page}) => {
  const s = servers.open;
  const id = await waiting(s, 'Approve while everything is refused');
  await enter(page, s);
  await drawer(page, id);
  await page.locator('#dd-by').fill('refused');
  await page.route('**/daemon/**', (route) => route.fulfill({status: 403, json: {refused: 'forbidden', message: 'refused'}}));
  const all = reqs(page);
  await card(page, 0).locator('button[data-answer="A"]').click();
  await expect(page.locator('#dd-msg')).toContainText('403');
  const seen = await rate(page, 5000);
  const perSecond = seen.length / 5;
  console.log(`403 REQUESTS in 5 s: ${seen.length} (${perSecond.toFixed(1)}/s)`);
  expect(perSecond).toBeLessThanOrEqual(4);
  expect(all.filter(isCommand)).toHaveLength(1);
});
