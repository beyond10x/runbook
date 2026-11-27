// A person answers an open decision from the page the daemon serves (`runbook serve --ui`, the binary cargo
// built; story ui-embedded-in-the-binary, slice 2). The daemon is the stub runtime's preset `stub-decision`:
// a run that raises two decision requests at once (the question is the run's intake), waits until both are
// answered, records `stub.answered` and ends `approved`. The page reads the open requests from the view
// `open-decisions` and answers with the daemon's own command `answer-decision`, the one `runbook decide`
// sends: a `POST` with `Authorization: Bearer <token>` (when the daemon has a token) and
// `Content-Type: application/json`. The cookie of the login is for reads and never authorises a command.
// The token stays in the memory of the page. Each daemon listens on 127.0.0.1:0, with a state directory of its own.
import {test, expect as plain} from '@playwright/test';
import {spawnChild, SERVE_CAPS} from './child.mjs';
import {installPageTime, spendPageTime} from './page-time.mjs';
import fs from 'node:fs';
import {randomBytes} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const TOKEN = `tok-${randomBytes(24).toString('hex')}`;
const PHONE = {width: 390, height: 844};
const servers = {};
let dir;

function start(name, env) {
  return new Promise((resolve, reject) => {
    fs.mkdirSync(path.join(dir, name, 'empty'), {recursive: true});
    const proc = spawnChild(bin, ['serve', ...SERVE_CAPS, '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, name, 'state')], {cwd: path.join(dir, name, 'empty'), env: {...process.env, HOME: path.join(dir, name), RUNBOOK_TOKEN: '', ...env}, stdio: ['ignore', 'pipe', 'pipe']});
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
const open = (s, id) => rows(s, 'open-decisions', `?run_id=${id}`);

// a run that waits on two decisions
async function waiting(s, intake) {
  const r = await post(s, 'submit-run', {runbook_id: 'stub-decision', intake, confinement: 'linux'});
  const id = r.published[0].payload.run_id;
  await expect.poll(async () => (await open(s, id)).length, {timeout: 15000}).toBe(2);
  return id;
}

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemons: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-decide-'));
  servers.open = await start('open', {});
  servers.guarded = await start('guarded', {RUNBOOK_TOKEN: TOKEN});
});
test.afterAll(() => {
  for (const s of Object.values(servers)) if (s.proc.exitCode == null) s.proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

// every error of the page fails the test, except the 401 that a daemon with a token is meant to answer
// (the probe of the page, a wrong token): the browser logs each as a console error
function watch(page, {allow401 = false} = {}) {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('dialog', (d) => { problems.push(`dialog ${d.message()}`); d.dismiss(); });
  page.on('console', (m) => { if (m.type() === 'error' && !(allow401 && /status of 401/.test(m.text()))) problems.push(`console ${m.text()}`); });
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

async function pick(page, id) {
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${id.slice(-8)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(id.slice(-8));
}

async function drawer(page, id, cards = 2) {
  await pick(page, id);
  await expect(page.locator('#deccount')).toHaveText(String(cards));
  await page.click('#decbtn');
  await expect(page.locator('#drawer .dcard')).toHaveCount(cards);
}

const card = (page, n) => page.locator('#drawer .dcard').nth(n);
const choose = (page, n, letter) => card(page, n).locator(`button[data-answer="${letter}"]`).click();

test('a run that waits shows its open decisions, and an answer from the page ends the wait', async ({page}) => {
  const s = servers.open;
  const problems = watch(page);
  const sent = commands(page);
  const id = await waiting(s, 'Approve the release plan');
  await enter(page, s);
  await drawer(page, id);

  // what is asked and the options the daemon's view names
  const first = card(page, 0);
  await expect(first).toContainText('Approve the release plan');
  await expect(first.locator('[data-field=case_id]')).toHaveText('case case-1');
  await expect(first.locator('[data-field=action]')).toHaveText('action stub.approve');
  await expect(first.locator('[data-field=capability]')).toHaveText('capability approve');
  await expect(first.locator('button[data-answer="A"]')).toContainText('yes');
  await expect(first.locator('button[data-answer="B"]')).toContainText('no');
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'none');
  await expect(page.locator('#dd-token')).toBeHidden();

  await page.locator('#dd-by').fill('page-reader');
  await choose(page, 0, 'A');
  await expect(page.locator('#drawer .dcard')).toHaveCount(1);
  await expect(page.locator('#deccount')).toHaveText('1');
  await expect.poll(async () => (await open(s, id)).length).toBe(1);
  expect((await rows(s, 'run', `?run_id=${id}`))[0].state).toBe('WaitingOnDecision');

  await choose(page, 0, 'B');
  await expect(page.locator('#drawer .dcard')).toHaveCount(0);
  await expect(page.locator('#deccount')).toHaveText('0');
  await expect.poll(async () => (await open(s, id)).length).toBe(0);
  await expect.poll(async () => (await rows(s, 'run', `?run_id=${id}`))[0]).toMatchObject({state: 'Ended', outcome: 'approved'});

  // the daemon recorded the answers, by the name the form gave
  const decided = await rows(s, 'decisions', `?run_id=${id}`);
  expect(decided.map((d) => [d.state, d.answer, d.answered_by]).sort()).toEqual([['Answered', 'A', 'page-reader'], ['Answered', 'B', 'page-reader']]);

  // the run goes on: its events continue in the feed, read from the daemon's stream
  await page.click('#drclose');
  await expect(page.locator('#feed')).toContainText('stub.answered');

  // two commands, each the daemon's own, as `runbook decide` sends it: JSON, no token, no cookie
  expect(sent.map((r) => `${r.method()} ${new URL(r.url()).pathname}`)).toEqual(Array(2).fill('POST /daemon/commands/answer-decision'));
  for (const r of sent) {
    const h = await r.allHeaders();
    expect(h['content-type']).toBe('application/json');
    expect(h.authorization).toBeUndefined();
    expect(h.cookie).toBeUndefined();
    expect(Object.keys(JSON.parse(r.postData())).sort()).toEqual(['answer', 'answered_by', 'request_id', 'run_id']);
  }
  console.log(`REQUESTS ${sent.map((r) => `${r.method()} ${new URL(r.url()).pathname}`).join(' | ')}; headers ${Object.keys(await sent[0].allHeaders()).sort().join(',')}`);
  expect(problems).toEqual([]);
});

test('with a token the page asks for it once, a wrong token shows the daemon\'s refusal and the answer is not sent again by itself', async ({page}) => {
  const s = servers.guarded;
  const problems = watch(page, {allow401: true});
  const sent = commands(page);
  const id = await waiting(s, 'Approve with a token');
  await installPageTime(page);
  await enter(page, s);
  await drawer(page, id);
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'needed');
  await expect(page.locator('#dd-token')).toBeVisible();
  await page.locator('#dd-by').fill('guarded-reader');

  // no token typed: the page asks and sends nothing
  await choose(page, 0, 'A');
  await expect(page.locator('#dd-msg')).toContainText('token');
  expect(sent).toHaveLength(0);

  // a wrong token: one request, the daemon's refusal on the page, the field is empty, the request stays open
  await page.locator('#dd-token').fill('not-the-token');
  await choose(page, 0, 'A');
  await expect(page.locator('#dd-msg')).toContainText('401');
  await expect(page.locator('#dd-msg')).toContainText('bearer token is required');
  await expect(page.locator('#dd-token')).toHaveValue('');
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'needed');
  // 2.5 s of the page's own time (page-time.mjs): a timer of the page that would send the answer again fires in it
  await spendPageTime(page, 2500);
  expect(sent).toHaveLength(1);
  expect(await open(s, id)).toHaveLength(2);
  expect((await sent[0].allHeaders()).authorization).toBe('Bearer not-the-token');

  // the right token: sent, and not asked for again
  await page.locator('#dd-token').fill(s.token);
  await choose(page, 0, 'A');
  await expect(page.locator('#drawer .dcard')).toHaveCount(1);
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'held');
  await expect(page.locator('#dd-token')).toBeHidden();
  await choose(page, 0, 'B');
  await expect(page.locator('#drawer .dcard')).toHaveCount(0);
  await expect.poll(async () => (await rows(s, 'run', `?run_id=${id}`))[0]).toMatchObject({state: 'Ended', outcome: 'approved'});
  expect(sent).toHaveLength(3);
  for (const r of sent.slice(1)) {
    const h = await r.allHeaders();
    expect(h.authorization).toBe(`Bearer ${s.token}`);
    expect(h['content-type']).toBe('application/json');
    expect(h.cookie).toBeUndefined();
  }
  console.log(`REQUESTS ${sent.slice(1).map((r) => `${r.method()} ${new URL(r.url()).pathname}`).join(' | ')}; headers ${Object.keys(await sent[2].allHeaders()).sort().join(',')}`);
  expect(problems).toEqual([]);
});

test('after a reload the page can read, and a command asks for the token again before it is sent', async ({page}) => {
  const s = servers.guarded;
  const problems = watch(page, {allow401: true});
  const sent = commands(page);
  const id = await waiting(s, 'Approve after a reload');
  await enter(page, s);
  await drawer(page, id);
  await page.locator('#dd-by').fill('before-reload');
  await page.locator('#dd-token').fill(s.token);
  await choose(page, 0, 'A');
  await expect(page.locator('#drawer .dcard')).toHaveCount(1);
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'held');
  expect(sent).toHaveLength(1);

  await page.reload();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await drawer(page, id, 1);
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'needed');
  await expect(page.locator('#dd-token')).toBeVisible();
  await page.locator('#dd-by').fill('after-reload');
  await choose(page, 0, 'B');
  await expect(page.locator('#dd-msg')).toContainText('token');
  expect(sent).toHaveLength(1);
  expect(await open(s, id)).toHaveLength(1);
  await page.locator('#dd-token').fill(s.token);
  await choose(page, 0, 'B');
  await expect(page.locator('#drawer .dcard')).toHaveCount(0);
  expect(sent).toHaveLength(2);
  expect(problems).toEqual([]);
});

test('the token is in no cookie, no storage, not in the URL, the history state or the DOM', async ({page, context}) => {
  const s = servers.guarded;
  const problems = watch(page, {allow401: true});
  const id = await waiting(s, 'Approve and look for the token');
  await enter(page, s);
  await drawer(page, id);
  await page.locator('#dd-by').fill('looking');
  await page.locator('#dd-token').fill(s.token);
  await choose(page, 0, 'A');
  await expect(page.locator('#drawer .dcard')).toHaveCount(1);
  await expect(page.locator('#dd-who')).toHaveAttribute('data-token', 'held');
  // a command was sent with the token, so the page holds it now
  await expect.poll(async () => (await open(s, id)).length).toBe(1);

  const cookies = await context.cookies();
  expect(cookies.map((c) => c.name)).toEqual(['runbook_ui']);
  for (const c of cookies) expect(`${c.name}=${c.value}`).not.toContain(s.token);
  const seen = await page.evaluate(async () => ({
    local: JSON.stringify({...localStorage}),
    session: JSON.stringify({...sessionStorage}),
    databases: (await indexedDB.databases()).map((d) => d.name),
    url: location.href,
    state: JSON.stringify(history.state),
    html: document.documentElement.outerHTML,
    text: document.documentElement.innerText,
    values: [...document.querySelectorAll('input:not([type=checkbox]):not([type=radio]), textarea')].map((e) => e.value),
    cookie: document.cookie,
  }));
  expect(seen.databases).toEqual([]);
  for (const k of ['local', 'session', 'url', 'state', 'html', 'text', 'cookie']) expect(seen[k], k).not.toContain(s.token);
  expect(seen.values.filter((v) => v.includes(s.token))).toEqual([]);
  expect(seen.values.every((v) => v === '' || v === 'looking')).toBe(true);
  expect(problems).toEqual([]);
});

test('the text of a decision is shown as text: markup in a question runs nothing', async ({page}) => {
  const s = servers.open;
  const problems = watch(page);
  const hostile = '<img src=x onerror="window.__pwned=1;alert(1)"><b>bold</b>';
  const id = await waiting(s, hostile);
  await enter(page, s);
  await drawer(page, id);
  await expect(card(page, 0)).toContainText(hostile);
  expect(await page.locator('#drawer img, #drawer b').count()).toBe(0);
  expect(await page.locator('#drawer').innerHTML()).toContain('&lt;img src=x');
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  await page.waitForTimeout(500);
  expect(problems).toEqual([]);
});

test.describe('on a phone', () => {
  test.use({viewport: PHONE, isMobile: true, hasTouch: true});

  test('the drawer lists the decisions and a tap answers one', async ({page}) => {
    const s = servers.open;
    const problems = watch(page);
    const id = await waiting(s, 'Approve on a phone');
    await enter(page, s);
    await drawer(page, id);
    await expect(card(page, 0)).toContainText('Approve on a phone');
    // the drawer slides in (a .3 s transition): wait for it to settle before measuring
    const button = card(page, 0).locator('button[data-answer="A"]');
    await expect.poll(async () => { const b = await button.boundingBox(); return b.x + b.width; }).toBeLessThanOrEqual(PHONE.width);
    const box = await button.boundingBox();
    expect(box.height).toBeGreaterThanOrEqual(24);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.locator('#dd-by').fill('phone');
    await card(page, 0).locator('button[data-answer="A"]').tap();
    await expect(page.locator('#drawer .dcard')).toHaveCount(1);
    await card(page, 0).locator('button[data-answer="B"]').tap();
    await expect(page.locator('#drawer .dcard')).toHaveCount(0);
    await expect.poll(async () => (await rows(s, 'run', `?run_id=${id}`))[0]).toMatchObject({state: 'Ended', outcome: 'approved'});
    expect(problems).toEqual([]);
  });
});
