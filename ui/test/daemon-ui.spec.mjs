// The page served by the daemon (`runbook serve --ui`, the binary cargo built; story
// ui-embedded-in-the-binary, slice 1): the UI reads the list of runs from GET /daemon/views/runs and the
// events of each run from the daemon's server-sent stream. Two stub runs in two sessions: the page lists
// both and the feed of each shows its own events. The daemon listens on 127.0.0.1:0 (the kernel gives the
// port) with a state directory and a working directory of its own, empty: no ui/ beside it.
// The binary: SDC_RUNBOOK_BIN, else runbook/target/debug/runbook (`task test:ui` builds it first).
import {test, expect as plain} from '@playwright/test';
import {spawnChild, SERVE_CAPS} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

// a page that loads thirty files may take more than the default 5 s on a loaded machine
const expect = plain.configure({timeout: 15000});
const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
let proc, base, dir;
const runs = [];

const post = (name, body) => fetch(`${base}/daemon/commands/${name}`, {method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(body)}).then((r) => r.json());
const rows = (view, q = '') => fetch(`${base}/daemon/views/${view}${q}`).then((r) => r.json()).then((j) => j.rows);

function start() {
  return new Promise((resolve, reject) => {
    proc = spawnChild(bin, ['serve', ...SERVE_CAPS, '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, 'state')], {cwd: path.join(dir, 'empty'), env: {...process.env, HOME: dir, RUNBOOK_TOKEN: ''}, stdio: ['ignore', 'pipe', 'pipe']});
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
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-daemon-'));
  fs.mkdirSync(path.join(dir, 'empty'));
  base = await start();
  proc.removeAllListeners('exit');
  for (const [i, session] of ['alpha', 'beta'].entries()) {
    await post('open-session', {session_id: session, client: `spec-${session}`});
    const r = await post('submit-run', {runbook_id: 'sdc', intake: `intake of the ${session} run`, session_id: session, confinement: 'linux'});
    runs.push({session, id: r.published[0].payload.run_id, i});
  }
  await expect.poll(async () => (await rows('runs')).filter((r) => r.state === 'Ended').length, {timeout: 15000}).toBe(2);
});
test.afterAll(() => {
  if (proc && proc.exitCode == null) proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

const tag = (r) => r.id.slice(-8);
const feedRows = (page) => page.locator('#feed .fd-row');

async function pick(page, run) {
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${tag(run)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(tag(run));
}

test('the daemon serves the page and the page lists both runs, each feed showing its own events', async ({page}) => {
  const problems = [];
  await page.addInitScript(() => { window.__csp = []; document.addEventListener('securitypolicyviolation', (e) => window.__csp.push(`${e.violatedDirective} ${e.blockedURI}`)); });
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });

  const index = await page.goto(`${base}/`);
  expect(index.status()).toBe(200);
  expect(index.headers()['content-security-policy']).toContain("default-src 'none'");
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);

  // both runs are listed, with the daemon's own state words
  await page.click('#runpick');
  const options = page.locator('#pk-list [role=option]');
  await expect(options).toHaveCount(2);
  for (const r of runs) await expect(page.locator(`#pk-list [role=option][title*="${tag(r)}"]`)).toContainText('Ended');
  await page.keyboard.press('Escape');

  // the feed of each run is its own: the events of that run, none of the other
  for (const [mine, other] of [[runs[0], runs[1]], [runs[1], runs[0]]]) {
    await pick(page, mine);
    const list = feedRows(page);
    await expect(list.first()).toBeVisible();
    await expect.poll(() => list.evaluateAll((els) => els.map((e) => e.dataset.sequence).join(','))).toBe('2,1');
    const owners = await list.evaluateAll((els) => [...new Set(els.map((e) => e.dataset.run))]);
    expect(owners).toEqual([mine.id]);
    const text = await page.locator('#feed').innerText();
    for (const kind of ['runbook.submitted', 'stub.note']) expect(text).toContain(kind);
    // each row belongs to its run by its data-run (above); its words name no record id (UXF-029), of this run or the other
    expect(text).not.toContain(mine.id);
    expect(text).not.toContain(other.id);
  }
  expect(await page.evaluate(() => window.__csp)).toEqual([]);
  expect(problems).toEqual([]);
});

test('a run that is submitted while the page is open appears in the list and its events arrive', async ({page}) => {
  await page.goto(`${base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  const before = (await rows('runs')).length;
  const r = await post('submit-run', {runbook_id: 'sdc', intake: 'a run that comes later', session_id: 'alpha', confinement: 'linux'});
  const id = r.published[0].payload.run_id;
  await page.click('#runpick');
  await expect(page.locator('#pk-list [role=option]')).toHaveCount(before + 1);
  await page.locator(`#pk-list [role=option][title*="${id.slice(-8)}"]`).click();
  await expect(page.locator('#feed .fd-row').first()).toBeVisible();
  await expect.poll(() => page.locator('#feed .fd-row').evaluateAll((els, want) => els.every((e) => e.dataset.run === want), id)).toBe(true);
  await expect(page.locator('#feed')).toContainText('runbook.submitted');
});

// story inspector-shows-step-products: the page of the daemon has the same panels. Since story
// daemon-page-shows-a-run-tree-and-products the daemon serves the files of a run's work/ directory (views `products` and
// `product`) and its case tree (view `cases`); a run of the stub runtime has neither files nor a tree, and the panels say
// so. The evidence and the actions of a case are not served, and the tab says that.
test('the daemon page has the Products panel and the product line; a stub run has no file and no case tree, and the page says so', async ({page}) => {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });
  await page.goto(`${base}/`);
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await pick(page, runs[0]);
  await expect(page.locator('#product')).toContainText('the run has written no file under work/ yet');
  await expect(page.locator('#product .prod-link')).toHaveCount(0);
  await expect(page.locator('#tree')).toContainText('This run has no case tree');
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod .prod-daemon')).toHaveText('the files under the run\'s work/ directory, which its cases share');
  await expect(page.locator('#panel')).not.toContainText('serves no');
  await expect(page.locator('#panel .prod-files')).toContainText('the run has written no file under work/ yet');
  await expect(page.locator('#panel .prod-file')).toHaveCount(0);
  expect(problems).toEqual([]);
  const served = await fetch(`${base}/markdown.js`);
  expect(served.status).toBe(200);
  expect(served.headers.get('content-type')).toContain('text/javascript');
  expect((await fetch(`${base}/products.js`)).status).toBe(200);
});
