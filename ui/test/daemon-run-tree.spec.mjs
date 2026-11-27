// The daemon's page shows a run's case tree and products, with one address per run (story
// daemon-page-shows-a-run-tree-and-products). A daemon of the Loom runtime over a three-level runbook home
// (daemon-tree-fixture.mjs: root, section, draft); two runs wait on the person step of `draft`. The page reads the
// daemon's views `cases`, `products` and `product`, and nothing else of a run. The binary: SDC_RUNBOOK_BIN, else
// runbook/target/debug/runbook (`task test:ui` builds it first). With SDC_SHOTS_DIR set, two screenshots are written
// there (the tree with a nested case, the Products tab).
import {test, expect as plain} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {ui, bin, startDaemon, submit, review, settle, rows, workspace, post} from './daemon-tree-fixture.mjs';

const expect = plain.configure({timeout: 15000});
let d, dir;
const runs = {};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  expect(fs.existsSync(bin), `${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-tree-'));
  d = await startDaemon(dir);
  runs.a = await submit(d, 'a note on tea');
  runs.b = await submit(d, 'a note on coffee');
  runs.c = await submit(d, 'a note on water');
  for (const id of [runs.a, runs.b, runs.c]) await expect.poll(() => review(d, id).then((x) => x?.request_id ?? null), {timeout: 30000}).not.toBeNull();
  // what the cases wrote, and three files of the run written here: Markdown, a file over the read bound, and bytes that are not text
  const w = workspace(d, runs.a);
  fs.mkdirSync(path.join(w, 'notes'), {recursive: true});
  fs.writeFileSync(path.join(w, 'notes', 'report.md'), '# Report on tea\n\nThe draft is *short*.\n');
  fs.writeFileSync(path.join(w, 'big.txt'), 'x'.repeat(1048576 + 10));
  fs.writeFileSync(path.join(w, 'blob.bin'), Buffer.from([0, 1, 2, 255]));
});
test.afterAll(() => {
  if (d?.proc && d.proc.exitCode == null) d.proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

const tag = (id) => id.slice(-8);
const shot = (page, name) => (process.env.SDC_SHOTS_DIR ? page.screenshot({path: path.join(process.env.SDC_SHOTS_DIR, name), fullPage: false}) : null);
function watch(page) {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') problems.push(`console ${m.text()}`); });
  return problems;
}
// the tree as the page draws it: each case's label, its depth (the guides before it) and its state word
const treeOf = (page) => page.locator('#tree .case').evaluateAll((els) => els.map((e) => `${'  '.repeat(e.querySelectorAll('.guide').length)}${e.querySelector('.cid').textContent} ${e.querySelector('.cst').dataset.v}`));

test('the page shows a run of the daemon as its case tree, nested to the depth the run has, each case with its state', async ({page}) => {
  const problems = watch(page);
  await page.goto(`${d.base}/?run=${runs.a}`);
  await expect(page.locator('#runpick')).toContainText(tag(runs.a));
  await expect(page.locator('#tree .case')).toHaveCount(3);
  await expect.poll(() => treeOf(page)).toEqual([`${runs.a} open`, '  section open', '    draft awaiting a person']);
  await expect(page.locator('#tree .case .proto')).toHaveText(['engine-demo', 'engine-section', 'engine-draft']);
  // the case that waits is the one the page opens on; the feed stays the run's events
  await expect(page.locator('#gtitle .gt-id')).toHaveText('draft');
  await expect(page.locator('#feed .fd-row').first()).toBeVisible();
  await expect(page.locator('#feed')).toContainText('runbook.submitted');
  // a case is chosen from the tree; the centre shows the run's steps (no graph, and no text in its place about one)
  await page.locator('#tree .case', {hasText: 'section'}).click();
  await expect(page.locator('#gtitle .gt-id')).toHaveText('section');
  await expect(page.locator('#graph')).toBeHidden();
  await expect(page.locator('#dsteps tbody tr').first()).toBeVisible();
  await expect(page.locator('#center')).not.toContainText('serves no');
  await page.locator('#tree .case', {hasText: 'draft'}).click();
  await shot(page, 'tree.png');
  expect(problems).toEqual([]);
});

test('the centre lists the steps the run took, read from the runtime\'s own events, and says the run waits on a decision', async ({page}) => {
  const problems = watch(page);
  await page.goto(`${d.base}/?run=${runs.c}`);
  await expect(page.locator('#tree .case')).toHaveCount(3);
  // the run's steps in order: the two openings of a child (no program, so no start), the check before the draft
  // exists (exit 1), the writer, the check again (exit 0); each with its state and try
  const rowsOf = page.locator('#dsteps tbody tr');
  await expect(rowsOf.locator('.ds-act')).toHaveText(['draft.open', 'draft.open', 'draft.check', 'draft.write', 'draft.check']);
  await expect(rowsOf.locator('.ds-st')).toHaveText(['done', 'done', 'exit 1', 'done', 'done']);
  await expect(rowsOf.locator('.ds-try')).toHaveText(['—', '—', '1', '1', '2']);
  // the steps of a run of three cases name their case
  await expect(page.locator('#dsteps thead')).toContainText('Case');
  await expect(page.locator('#dsteps tbody tr', {has: page.locator('.ds-act', {hasText: 'draft.write'})}).first().locator('.ds-case')).toHaveText('draft');
  await expect(page.locator('#dnow')).toHaveText(/^Waiting on a decision/);
  await expect(page.locator('#dsteps tbody tr[data-state=running]')).toHaveCount(0);
  expect(problems).toEqual([]);
});

test('the Products tab lists the run\'s files and shows one: text, Markdown, a file over the bound and bytes that are not text', async ({page}) => {
  const problems = watch(page);
  await page.goto(`${d.base}/?run=${runs.a}`);
  await expect(page.locator('#tree .case')).toHaveCount(3);
  await expect(page.locator('#product')).toContainText('5 files under the run\'s work/ directory');
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-daemon')).toHaveText('the files under the run\'s work/ directory, which its cases share');
  await expect(page.locator('#panel')).not.toContainText('serves no');
  await expect(page.locator('#panel .prod-file .prod-fp')).toHaveText(['big.txt', 'blob.bin', 'draft.txt', 'idea.yaml', 'notes/report.md']);
  await shot(page, 'products.png');
  await page.locator('#panel .prod-file[data-path="draft.txt"]').click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toHaveText('draft\n');
  await expect(page.locator('#panel .prod-meta')).toContainText('6 bytes · text');
  await page.locator('#panel .prod-back').click();
  await page.locator('#panel .prod-file[data-path="notes/report.md"]').click();
  await expect(page.locator('#panel .md h1')).toHaveText('Report on tea');
  await page.locator('#panel .prod-back').click();
  await page.locator('#panel .prod-file[data-path="big.txt"]').click();
  await expect(page.locator('#panel .prod-detail')).toContainText('showing the first 1048576 of 1048586 bytes');
  await page.locator('#panel .prod-back').click();
  await page.locator('#panel .prod-file[data-path="blob.bin"]').click();
  await expect(page.locator('#panel .prod-detail')).toContainText('shown by its size and type only');
  expect(problems).toEqual([]);
});

test('one address per run: ?run=<run id> opens that run after a reload and in another tab, and choosing a run writes its id', async ({page, context}) => {
  await page.goto(`${d.base}/?run=${runs.b}`);
  await expect(page.locator('#runpick')).toContainText(tag(runs.b));
  await expect(page.locator('#tree .case')).toHaveCount(3);
  await page.reload();
  await expect(page.locator('#runpick')).toContainText(tag(runs.b));
  await expect(page.locator('#tree .case').first()).toContainText(runs.b);
  const other = await context.newPage();
  await other.goto(`${d.base}/?run=${runs.a}`);
  await expect(other.locator('#runpick')).toContainText(tag(runs.a));
  await expect(other.locator('#tree .case').first()).toContainText(runs.a);
  await other.close();
  await page.click('#runpick');
  await page.locator(`#pk-list [role=option][title*="${tag(runs.a)}"]`).click();
  await expect(page.locator('#runpick')).toContainText(tag(runs.a));
  await expect.poll(() => new URL(page.url()).searchParams.get('run')).toBe(runs.a);
});

test('an address that names a run the daemon does not have says so with that id and shows no other run', async ({page}) => {
  const problems = watch(page);
  const missing = '01a2-no-such-run-<b>x</b>';
  await page.goto(`${d.base}/?run=${encodeURIComponent(missing)}`);
  await expect(page.locator('#tree [data-missing-run]')).toContainText(`No run ${missing} in this daemon`);
  await expect(page.locator('#runpick')).toContainText('no run');
  await expect(page.locator('#tree .case')).toHaveCount(0);
  await expect(page.locator('#tree b')).toHaveCount(0);
  expect(await page.locator('#feed .fd-row').count()).toBe(0);
  for (const id of [runs.a, runs.b]) expect(await page.locator('body').innerText()).not.toContain(id);
  expect(problems).toEqual([]);
});

test('a run that ends while the page shows it: its cases change to ended without a reload', async ({page}) => {
  await page.goto(`${d.base}/?run=${runs.b}`);
  await expect.poll(() => treeOf(page)).toEqual([`${runs.b} open`, '  section open', '    draft awaiting a person']);
  await settle(d, runs.b, (f) => expect.poll(f, {timeout: 30000}).toBe('Ended'));
  await expect.poll(() => treeOf(page)).toEqual([`${runs.b} ended delivered`, '  section ended passed', '    draft ended passed']);
  await expect(page.locator('#runpick')).toContainText('Ended');
  // the tree it showed is the daemon's
  const view = await rows(d, 'cases', `?run_id=${runs.b}`);
  expect(view.map((r) => r.case_state)).toEqual(['Ended', 'Ended', 'Ended']);
});

// ---- the adversary's cases ----

test('a run cancelled while the page shows it: the waiting case is open, not stuck, and the page says the run was cancelled', async ({page}) => {
  const problems = watch(page);
  await page.goto(`${d.base}/?run=${runs.c}`);
  await expect.poll(() => treeOf(page)).toEqual([`${runs.c} open`, '  section open', '    draft awaiting a person']);
  await post(d, 'cancel-run', {run_id: runs.c});
  await expect.poll(() => treeOf(page)).toEqual([`${runs.c} open`, '  section open', '    draft open']);
  await expect(page.locator('#runpick')).toContainText('Cancelled');
  expect(problems).toEqual([]);
});

test('an address in the form the daemon\'s page wrote before (the run\'s name) or a part of a run id opens no run', async ({page}) => {
  for (const asked of [`engine-demo ${tag(runs.a)}`, tag(runs.a), runs.a.slice(0, -1), ` ${runs.a}`]) {
    await page.goto(`${d.base}/?run=${encodeURIComponent(asked)}`);
    await expect(page.locator('#tree [data-missing-run]')).toHaveAttribute('data-missing-run', asked);
    await expect(page.locator('#tree .case')).toHaveCount(0);
    await expect(page.locator('#runpick')).toContainText('no run');
  }
});

test('a tree the daemon cannot read for a moment (503) leaves the tree shown in place, and the next read draws it again', async ({page}) => {
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  await page.goto(`${d.base}/?run=${runs.a}`);
  await expect(page.locator('#tree .case')).toHaveCount(3);
  let refused = 0;
  const cases = (u) => u.pathname === '/daemon/views/cases';
  await page.route(cases, (r) => { refused++; return r.fulfill({status: 503, contentType: 'application/json', body: '{"refused":"tree not readable","message":"ask again"}'}); });
  await expect.poll(() => refused, {timeout: 10000}).toBeGreaterThan(1);
  await expect(page.locator('#tree .case')).toHaveCount(3);
  await page.unroute(cases);
  await page.reload();
  await expect(page.locator('#tree .case')).toHaveCount(3);
  expect(problems).toEqual([]);
});

test('the tree of the page gives every case its depth to any depth, and a parent loop or a missing parent neither hangs nor hides a case', async ({page}) => {
  await page.goto(`${d.base}/?run=${runs.a}`);
  const r = await page.evaluate(async () => {
    const {daemonCases} = await import('/source.js');
    const chain = Array.from({length: 40}, (_, i) => ({case_id: `c${i}`, parent: i ? `c${i - 1}` : undefined, name: i ? `n${i}` : undefined, protocol: 'p', case_state: 'Open'}));
    const deep = daemonCases(chain).map((c) => c.depth);
    const loop = daemonCases([{case_id: 'a', parent: 'b', protocol: 'p', case_state: 'Open'}, {case_id: 'b', parent: 'a', protocol: 'p', case_state: 'Open'}]).map((c) => c.depth);
    const orphan = daemonCases([{case_id: 'x', parent: 'gone', protocol: 'p', case_state: 'Stuck'}]).map((c) => [c.depth, c.events.at(-1).event]);
    const {caseState} = await import('/model.js');
    const capped = daemonCases(['SpendCapReached', 'CallCapReached', 'WallCapReached'].map((s, i) => ({case_id: `k${i}`, protocol: 'p', case_state: s}))).map((c) => caseState(c).label);
    return {deep, loop, orphan, capped};
  });
  expect(r.deep).toEqual(Array.from({length: 40}, (_, i) => i));
  expect(r.loop.every((x) => Number.isInteger(x) && x <= 64)).toBe(true);
  expect(r.orphan).toEqual([[0, 'stuck']]);
  // a case of a run a cap ended is labelled with that cap, never stuck
  expect(r.capped).toEqual(['LLM budget spent', 'LLM calls spent', 'wall time spent']);
  // the page draws every row it was given: a loop of two cases and a case whose parent is not in the tree included
  const fake = [{case_id: 'r', protocol: 'p', case_state: 'Open'}, {case_id: 'a', parent: 'b', name: 'loop-a', protocol: 'p', case_state: 'Open'},
    {case_id: 'b', parent: 'a', name: 'loop-b', protocol: 'p', case_state: 'Open'}, {case_id: 'x', parent: 'gone', name: 'orphan', protocol: 'p', case_state: 'Stuck'}];
  await page.route((u) => u.pathname === '/daemon/views/cases', (q) => q.fulfill({status: 200, contentType: 'application/json', body: JSON.stringify({rows: fake})}));
  await page.reload();
  await expect.poll(() => page.locator('#tree .case .cid').allTextContents().then((t) => t.sort())).toEqual(['loop-a', 'loop-b', 'orphan', 'r']);
});
