// The page served by the daemon (`runbook serve --ui`) shows the autonomy level and the decider kind of a run as
// text, next to its runbook and its state (story daemon-runs-take-autonomy-and-a-decider). The daemon here runs the
// Loom runtime over the fixture runbook of the Rust cases (`runbook/tests/fixtures/autonomy-runbook`) with a rule
// decider that approves; the run is submitted at A3, so the capability approval is answered by the rule and the
// reviewer's step waits for a person. A run submitted with no level shows neither.
// The binary: SDC_RUNBOOK_BIN, else runbook/target/debug/runbook (`task test:ui` builds it first).
import {test, expect as plain} from '@playwright/test';
import {spawnChild, SERVE_CAPS} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const home = path.resolve(ui, '../runbook/tests/fixtures/autonomy-runbook');
let proc, base, dir;
const runs = {};

const post = (name, body) => fetch(`${base}/daemon/commands/${name}`, {method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(body)}).then((r) => r.json());
const rows = (view, q = '') => fetch(`${base}/daemon/views/${view}${q}`).then((r) => r.json()).then((j) => j.rows);

function start() {
  return new Promise((resolve, reject) => {
    const args = ['serve', ...SERVE_CAPS, '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, 'state'), '--runtime', 'loom', '--runbook-home', home, '--min-confinement', 'local'];
    proc = spawnChild(bin, args, {cwd: path.join(dir, 'empty'), env: {...process.env, HOME: dir, RUNBOOK_TOKEN: '', RUNBOOK_CONFIG: path.join(dir, 'runbook.toml')}, stdio: ['ignore', 'pipe', 'pipe']});
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
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-daemon-level-'));
  fs.mkdirSync(path.join(dir, 'empty'));
  fs.writeFileSync(path.join(dir, 'runbook.toml'), '[deciders.rule]\nverdict = "approve"\n');
  base = await start();
  proc.removeAllListeners('exit');
  const submit = async (name, extra) => {
    const r = await post('submit-run', {runbook_id: 'autonomy-demo', intake: 'a note on tea', confinement: 'local', ...extra});
    runs[name] = r.published[0].payload.run_id;
  };
  await submit('with', {autonomy: 'A3', decider: 'rule'});
  await submit('without', {});
  await expect.poll(async () => (await rows('runs')).filter((r) => r.state === 'WaitingOnDecision').length, {timeout: 30000}).toBe(2);
});
test.afterAll(() => {
  if (proc && proc.exitCode == null) proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

async function pick(page, run) {
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${run.slice(-8)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(run.slice(-8));
}

test('the run view names the level and the kind of a run that has them, and nothing of the sort for one that has none', async ({page}) => {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });
  await page.goto(`${base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);

  await pick(page, runs.with);
  await expect(page.locator('[data-key=dhead]')).toContainText('autonomy A3');
  await expect(page.locator('[data-key=dhead]')).toContainText('decider rule');

  await pick(page, runs.without);
  await expect(page.locator('[data-key=dhead]')).toContainText('autonomy-demo');
  await expect(page.locator('[data-key=dhead]')).not.toContainText('autonomy A');
  await expect(page.locator('[data-key=dhead]')).not.toContainText('decider');
  expect(problems).toEqual([]);
});
