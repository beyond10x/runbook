// The page served by the daemon (`runbook serve --ui`) names a run's model route in the run's header and lists the
// daemon's routes in a Routes view (story daemon-page-shows-the-route; docs/model-routes.md, section 6). The daemon runs
// the Loom runtime over the fixture runbook of the Rust cases (`runbook/tests/fixtures/autonomy-runbook`) with a rule
// decider and declares two routes on one stand-in gateway that this file serves on loopback: `qwen` (managed, its model
// starting, so a run on it waits for its route) and `claude` (hosted, ready). The stand-in answers GET requests from a
// table and keeps every request line: no model call, no wake, nothing beyond loopback.
// The binary: SDC_RUNBOOK_BIN, else runbook/target/debug/runbook (`task test:ui` builds it first).
import {test, expect as plain} from '@playwright/test';
import {spawnChild, SERVE_CAPS} from './child.mjs';
import {installPageTime, spendPageTime} from './page-time.mjs';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {eventWords} from '../web/runsteps.js';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const home = path.resolve(ui, '../runbook/tests/fixtures/autonomy-runbook');
let proc, base, dir, gateway;
const lines = [];
const runs = {};

// What the stand-in answers: ready, both aliases listed, qwen's pod starting (not callable) and claude callable.
const ANSWERS = {
  'GET /ready': {ready: true},
  'GET /v1/models': {object: 'list', data: [{id: 'qwen', object: 'model', wires: ['chat']}, {id: 'claude', object: 'model', wires: ['messages']}]},
  'GET /v1/status': {format: 'llm-gateway.status/1', models: [
    {model: 'qwen', kind: 'pod', wires: ['chat'], phase: 'starting', callable: false},
    {model: 'claude', kind: 'hosted', wires: ['messages'], phase: 'ready', callable: true}]},
};

function standIn() {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const line = `${req.method} ${req.url}`;
      lines.push(line);
      const body = ANSWERS[line];
      res.writeHead(body ? 200 : 404, {'content-type': 'application/json', connection: 'close'});
      res.end(JSON.stringify(body ?? {error: {code: 'not-found', message: 'no such route'}}));
    });
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

const post = (name, body) => fetch(`${base}/daemon/commands/${name}`, {method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(body)}).then((r) => r.json());
const rows = (view, q = '') => fetch(`${base}/daemon/views/${view}${q}`).then((r) => r.json()).then((j) => j.rows);

function start() {
  return new Promise((resolve, reject) => {
    const args = ['serve', ...SERVE_CAPS, '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, 'state'), '--runtime', 'loom', '--runbook-home', home, '--min-confinement', 'local'];
    proc = spawnChild(bin, args, {cwd: path.join(dir, 'empty'), env: {...process.env, HOME: dir, XDG_RUNTIME_DIR: dir, RUNBOOK_TOKEN: '', RUNBOOK_CONFIG: path.join(dir, 'runbook.toml')}, stdio: ['ignore', 'pipe', 'pipe']});
    let out = '', err = '';
    const timer = setTimeout(() => { proc.kill(); reject(new Error(`no listening line in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { err += d; });
    proc.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) { clearTimeout(timer); resolve(m[1]); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  expect(fs.existsSync(bin), `${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-daemon-routes-'));
  fs.mkdirSync(path.join(dir, 'empty'));
  gateway = await standIn();
  const url = `http://127.0.0.1:${gateway.address().port}`;
  fs.writeFileSync(path.join(dir, 'runbook.toml'), [
    '[deciders.rule]', 'verdict = "approve"',
    '[gateways.standin]', `base_url = "${url}"`,
    '[routes.qwen]', 'gateway = "standin"', 'model = "qwen"', 'kind = "managed"', 'start_wait_seconds = 600',
    '[routes.claude]', 'gateway = "standin"', 'model = "claude"', 'kind = "hosted"', ''].join('\n'));
  base = await start();
  proc.removeAllListeners('exit');
  const submit = async (route) => {
    const r = await post('submit-run', {runbook_id: 'autonomy-demo', intake: `a note on tea, on ${route}`, confinement: 'local', autonomy: 'A3', decider: 'rule', model_route: route});
    expect(r.outcome, JSON.stringify(r)).toBe('submitted');
    runs[route] = r.published[0].payload.run_id;
  };
  await submit('qwen');
  await submit('claude');
  // both runs stand still before a page opens: the one on qwen waits for its route, the one on claude for a person (the rule
  // decider answers the capability approval at A3 and the reviewer's step waits), so no event arrives during a case
  const states = async () => Object.fromEntries((await rows('runs')).map((r) => [r.model_route, r.state]));
  await expect.poll(states, {timeout: 30000}).toEqual({qwen: 'WaitingForRoute', claude: 'WaitingOnDecision'});
});
test.afterAll(() => {
  if (proc && proc.exitCode == null) proc.kill();
  if (gateway) gateway.close();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

async function enter(page) {
  await page.goto(`${base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function pick(page, run) {
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${run.slice(-8)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(run.slice(-8));
}

const head = (page) => page.locator('[data-key=dhead]');
const routeRow = (page, name) => page.locator(`#routesdlg [data-route="${name}"]`);

test('the header names the run\'s route, its kind and its status; the Routes view lists both routes with alias, kind, status and phase', async ({page}) => {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });
  await enter(page);

  await pick(page, runs.qwen);
  await expect(head(page)).toContainText('WaitingForRoute');
  await expect(head(page)).toContainText('route qwen · managed · starting');
  await expect(head(page)).toContainText('alias qwen · named by the run');

  await pick(page, runs.claude);
  await expect(head(page)).toContainText('route claude · hosted · ready');
  await expect(head(page)).not.toContainText('route qwen');

  await page.click('#routesbtn');
  await expect(page.locator('#routesdlg [data-route]')).toHaveCount(2);
  for (const [name, kind, status, phase] of [['qwen', 'managed', 'not-ready', 'starting'], ['claude', 'hosted', 'ready', 'ready']]) {
    const row = routeRow(page, name);
    await expect(row.locator('[data-field=alias]')).toHaveText(name);
    await expect(row.locator('[data-field=kind]')).toHaveText(kind);
    await expect(row.locator('[data-field=status]')).toHaveText(status);
    await expect(row.locator('[data-field=phase]')).toHaveText(phase);
  }
  await page.click('#routes-close');
  await expect(page.locator('#routesdlg[open]')).toHaveCount(0);

  expect(lines.filter((l) => !l.startsWith('GET ')), 'the stand-in got a call that is not a read: a model call or a wake').toEqual([]);
  expect(problems).toEqual([]);
});

// A phase or a wire the gateway sends reaches the row as visible ASCII up to 64 characters, so markup can stand in
// it: every field of a row the view answers is text in the header and in the Routes view (the rows are served by the
// test here, hostile in each field the page shows).
test('hostile fields of the view routes are text in the header and in the Routes view', async ({page}) => {
  const evil = '"><img src=x onerror="window.__pwned=1">';
  const row = {route: 'qwen', gateway: `gw${evil}`, alias: `qwen${evil}`, kind: `managed${evil}`, wires: [`chat${evil}`], status: `not-ready${evil}`, phase: `starting${evil}`, read_at: `2026-11-26T10:00:00Z${evil}`};
  await page.route('**/daemon/views/routes', (route) => route.fulfill({json: {rows: [row]}}));
  await enter(page);
  await pick(page, runs.qwen);
  await expect(head(page)).toContainText(`route qwen · managed${evil} · starting${evil}`);
  await page.click('#routesbtn');
  for (const field of ['alias', 'kind', 'gateway', 'status', 'phase', 'wires', 'read_at']) {
    await expect(routeRow(page, 'qwen').locator(`[data-field=${field}]`)).toHaveText(field === 'wires' ? row.wires[0] : row[field]);
  }
  expect(await page.locator('img, [onerror]').count()).toBe(0);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
});

// The bound of daemon-decide-review.spec.mjs (invariant 8): a daemon that refuses every request is asked at most 4
// times a second, here with the Routes view open, so its own reads add to the page's. The window starts once the page
// has every event of the daemon (the feed of the run that wrote the last one shows them all): the stream hands the
// page the events written before it opened in batches, and each batch reads the run list once, from a daemon that
// still answers.
test('a daemon that answers 403 to every request is polled at most 4 times a second with the Routes view open', async ({page}) => {
  await installPageTime(page);
  await enter(page);
  await pick(page, runs.claude);
  await expect(head(page)).toContainText('route claude · hosted');
  const all = await rows('daemon-events', '?after_position=0');
  const last = all.at(-1).run_id;
  await pick(page, last);
  // every event of that run the feed shows: the engine's own lines are folded (web/runsteps.js)
  await expect(page.locator('[data-key=fdl] .fd-row')).toHaveCount(all.filter((e) => e.run_id === last && !eventWords(e).engine).length);
  await page.click('#routesbtn');
  await expect(page.locator('#routesdlg [data-route]')).toHaveCount(2);
  await page.route('**/daemon/**', (route) => route.fulfill({status: 403, json: {refused: 'forbidden', message: 'refused'}}));
  const seen = [];
  page.on('request', (r) => seen.push(new URL(r.url()).pathname));
  // 5 s of the page's own time (page-time.mjs): every timer of the page that falls due fires, and every read it chains
  await spendPageTime(page, 5000);
  const perSecond = seen.length / 5;
  const byPath = {};
  for (const p of seen) byPath[p] = (byPath[p] || 0) + 1;
  console.log(`403 REQUESTS in 5 s with the Routes view open: ${seen.length} (${perSecond.toFixed(1)}/s): ${JSON.stringify(byPath)}`);
  expect(perSecond).toBeLessThanOrEqual(4);
  await expect(page.locator('#routesdlg[open]')).toHaveCount(1);
});
