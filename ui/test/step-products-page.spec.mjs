// What a step produced, in the page (story inspector-shows-step-products): the Products tab of the panel lists
// the evidence records, the actions and the files of a run, and opens each in the same panel: an evidence record
// with every field it holds, an action with its exit code, duration, start, end and output, a file as text, as
// indented JSON, as rendered Markdown, or as its size and type. The product of the run is one click from the first
// screen. A run of `runbook run` of a runbook that is not SDC (protocol example@1, test/products-fixture.mjs) and
// an SDC run (test/fixture-run.mjs, scenario person). A value the run did not record says "not recorded by this
// run", never an empty field. Screenshots: SDC_UI_SHOTS, else ui/screenshots (git-ignored).
import {test, expect} from '@playwright/test';
import {playFixture} from './fixtures.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';
import {writeProductsRun, viewWith, REPORT_MD, TOKEN_TEXT} from './products-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = process.env.SDC_UI_SHOTS || path.join(ui, 'screenshots');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'step-products-page');
const NR = 'not recorded by this run';
let srv, srv2, srv3, base, base2, base3;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); srv2?.kill(); srv3?.kill(); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  needRun(fixtureSource('person'), 'step-products-page.spec.mjs (fixture source)', test);
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  writeProductsRun(path.join(runs, 'viewed'), {product: 'view'});
  writeProductsRun(path.join(root, 'runs2', 'unwritten'), {product: 'missing'});
  writeProductsRun(path.join(root, 'runs3', 'newest'), {product: 'newest'});
  const view = path.join(root, 'example.view.json');
  fs.writeFileSync(view, JSON.stringify(viewWith('work/example/example.md')));
  const view2 = path.join(root, 'example-unwritten.view.json');
  fs.writeFileSync(view2, JSON.stringify(viewWith('work/example/not-written.md')));
  const [first, second, third] = await Promise.all([
    playFixture('person', path.join(runs, 'sdc')).then(() => startServer(['--view', view, '--view', path.resolve(ui, '../views/sdc.view.json'), '--runs', runs])),
    startServer(['--view', view2, '--runs', path.join(root, 'runs2')]),
    startServer(['--no-view', '--runs', path.join(root, 'runs3')]),
  ]);
  ({url: base, proc: srv} = first);
  ({url: base2, proc: srv2} = second);
  ({url: base3, proc: srv3} = third);
});

const open = async (page, name, at = () => base) => {
  await page.goto(`${at()}/?run=${name}`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible({timeout: 20000});
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
};
const tab = async (page) => {
  const phone = await page.evaluate(() => document.documentElement.classList.contains('phone'));
  if (phone && !(await page.locator('#tabs').isVisible())) await page.locator('#railbtn').click();
  await expect(page.locator('#tabs button[data-tab=products]')).toBeVisible();
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod')).toBeVisible();
};
const detail = (page, kind) => page.locator(`#panel .prod-detail[data-kind=${kind}]`);
const field = (page, kind, name) => detail(page, kind).locator(`[data-field="${name}"] .pf-v`).first();
const snap = (page, name) => { fs.mkdirSync(shots, {recursive: true}); return page.screenshot({path: path.join(shots, `${page.viewportSize().width}x${page.viewportSize().height}-products-${name}.png`)}); };

test('an evidence record opens with its result, subject, producer, time and every field it holds, nested values readable', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  await expect(page.locator('#panel .prod-ev')).toHaveCount(3);
  await page.locator('#panel .prod-ev[data-id="e0007-report.check"]').click();
  const d = detail(page, 'evidence');
  await expect(d).toBeVisible();
  await expect(field(page, 'evidence', 'result')).toHaveText('invalid');
  await expect(field(page, 'evidence', 'subject')).toContainText('report');
  await expect(field(page, 'evidence', 'producer')).toHaveText('example-check');
  await expect(field(page, 'evidence', 'observed_at')).toContainText('2026-11-21');
  await expect(field(page, 'evidence', 'kind')).toHaveText('report_shape');
  // nested values: a field of a field of a list, and an explicit null
  const nested = d.locator('.pf-nested');
  await expect(nested).toContainText('details');
  await expect(nested).toContainText('checks');
  await expect(nested.locator('[data-path="details.checks.0.reason"] .pf-v')).toHaveText('no heading named Summary');
  await expect(nested.locator('[data-path="details.checks.0.ok"] .pf-v')).toHaveText('false');
  await expect(nested.locator('[data-path="details.nothing"] .pf-v')).toHaveText('null');
  // nothing the page shows is an empty field
  for (const el of await d.locator('.pf-v').all()) expect((await el.innerText()).trim().length).toBeGreaterThan(0);
  await snap(page, 'evidence');
  // back to the lists
  await d.locator('.prod-back').click();
  await expect(page.locator('#panel .prod-ev')).toHaveCount(3);
});

test('an evidence record that lacks a field says it was not recorded, never an empty value', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  await page.locator('#panel .prod-ev[data-id="e0009-bare"]').click();
  for (const name of ['result', 'subject', 'producer', 'observed_at']) await expect(field(page, 'evidence', name)).toHaveText(NR);
  await expect(field(page, 'evidence', 'kind')).toHaveText('notes_shape');
});

test('from the explanation of an evidence node, a record opens in the Products panel', async ({page}) => {
  await open(page, 'viewed');
  await page.locator('#graph [data-id="evidence:notes_shape"]').click();
  await expect(page.locator('#panel a[data-open="evidence"]').first()).toBeVisible();
  await page.locator('#panel a[data-open="evidence"]').first().click();
  await expect(detail(page, 'evidence')).toBeVisible();
  await expect(field(page, 'evidence', 'result')).toHaveText('valid');
});

test('the files of the run are listed, a step file with its result beside its name, and a click shows the content as text', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  const list = page.locator('#panel .prod-files');
  await expect(list.locator('.prod-file')).not.toHaveCount(0);
  const notes = list.locator('.prod-file[data-path="work/example/notes.txt"]');
  await expect(notes.locator('.prod-result')).toHaveText('valid');
  await expect(list.locator('.prod-file[data-path="work/example/check.txt"] .prod-result')).toHaveCount(0);
  await expect(list).not.toContainText('token.txt');
  await expect(list).not.toContainText('.hidden');
  await expect(list).not.toContainText('leak.txt');
  await expect(list.locator('.prod-hidden')).toContainText('2 entries whose name starts with a dot are not listed');
  await notes.click();
  await expect(detail(page, 'file').locator('.prod-path')).toHaveText('work/example/notes.txt');
  await expect(detail(page, 'file').locator('pre.prod-text')).toHaveText('subject: notes\nlines: 3\nresult: valid\nnote: an invented note\n');
  await expect(detail(page, 'file').locator('.prod-meta')).toContainText('61 bytes');
  await expect(page.locator('body')).not.toContainText(TOKEN_TEXT);
});

test('JSON is shown indented and YAML as it is written; neither is rendered as markup', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  await page.locator('#panel .prod-file[data-path="work/example/data.json"]').click();
  const t = await detail(page, 'file').locator('pre.prod-text').innerText();
  expect(t).toContain('{\n  "run": "example",\n  "counts": {\n    "notes": 3,');
  await detail(page, 'file').locator('.prod-back').click();
  await page.locator('#panel .prod-file[data-path="work/idea.yaml"]').click();
  await expect(detail(page, 'file').locator('pre.prod-text')).toContainText('idea:\n  title: an invented idea\n  items:\n    - one');
});

test('a Markdown file is rendered: headings, paragraphs, lists, code, emphasis, tables, quotes, rules and links', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  await page.locator('#panel .prod-file[data-path="work/example/report.md"]').click();
  const md = detail(page, 'file').locator('.md');
  await expect(md.locator('h1')).toHaveText('Example report');
  await expect(md.locator('h2')).toHaveText('What was done');
  await expect(md.locator('p', {hasText: 'An invented report with'}).locator('strong')).toHaveText('strong');
  await expect(md.locator('p', {hasText: 'An invented report with'}).locator('em')).toHaveText('emphasis');
  await expect(md.locator('p', {hasText: 'An invented report with'}).locator('code')).toHaveText('inline code');
  await expect(md.locator('ul > li')).toHaveCount(4);
  await expect(md.locator('ul > li > ul > li')).toHaveText(['nested item one', 'nested item two']);
  await expect(md.locator('ol > li')).toHaveText(['first step', 'second step']);
  await expect(md.locator('table th')).toHaveText(['step', 'result']);
  await expect(md.locator('table tbody tr')).toHaveCount(2);
  await expect(md.locator('table tbody tr').nth(1).locator('td').nth(1)).toHaveText('invalid');
  await expect(md.locator('blockquote')).toHaveText('a quoted line');
  await expect(md.locator('hr')).toHaveCount(1);
  await expect(md.locator('pre code')).toHaveText('echo "a fenced block"');
  await expect(md.locator('a[href="https://example.com/docs"]')).toHaveAttribute('target', '_blank');
  await md.locator('a', {hasText: 'the data'}).click();
  await expect(detail(page, 'file').locator('.prod-path')).toHaveText('work/example/data.json');
  expect(REPORT_MD.length).toBeGreaterThan(100);
});

test('a file that is not text shows its size and type and no content; a file over the limit is cut and says how many bytes it has; a refused file says why', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  await page.locator('#panel .prod-file[data-path="work/example/blob.bin"]').click();
  const d = detail(page, 'file');
  await expect(d.locator('.prod-note')).toContainText('7 bytes');
  await expect(d.locator('.prod-note')).toContainText('not valid UTF-8 text');
  await expect(d.locator('pre')).toHaveCount(0);
  await d.locator('.prod-back').click();
  await page.locator('#panel .prod-file[data-path="work/example/big.txt"]').click();
  await expect(d.locator('.prod-note')).toContainText('showing the first 1048576 of 1048586 bytes');
  expect((await d.locator('pre.prod-text').innerText()).length).toBeGreaterThanOrEqual(1048576);
});

test('an action shows its exit code, and where the run did not record its duration, start and end it says so, with the step span the run did record', async ({page}) => {
  await open(page, 'viewed');
  await tab(page);
  const acts = page.locator('#panel .prod-act');
  await expect(acts).toHaveCount(6);
  await acts.filter({hasText: 'report.check'}).click();
  const d = detail(page, 'action');
  await expect(field(page, 'action', 'exit')).toHaveText('1');
  await expect(d.locator('[data-field="exit"]')).toHaveAttribute('data-bad', 'true');
  for (const name of ['duration', 'started', 'ended']) await expect(field(page, 'action', name)).toContainText(NR);
  await expect(d.locator('.prod-span')).toContainText('the step this action ran in');
  await expect(d.locator('.prod-span')).toContainText('4 s');
  await expect(field(page, 'action', 'output')).toContainText('error: no heading named Summary');
  await expect(d.locator('[data-field="output"] .prod-note')).toContainText('standard output followed by standard error');
  for (const name of ['stdout', 'stderr']) await expect(field(page, 'action', name)).toContainText(NR);
  await snap(page, 'action');
  await d.locator('.prod-back').click();
  await acts.filter({hasText: 'notes.check'}).click();
  await expect(field(page, 'action', 'exit')).toHaveText('0');
  await expect(detail(page, 'action').locator('[data-field="exit"]')).not.toHaveAttribute('data-bad', 'true');
  await detail(page, 'action').locator('.prod-back').click();
  // a long output is cut to its tail and says how much there is
  await acts.filter({hasText: 'notes.write'}).click();
  await expect(field(page, 'action', 'output')).toContainText('writer finished');
  await expect(detail(page, 'action').locator('[data-field="output"] .prod-note')).toContainText('last');
  await detail(page, 'action').locator('.prod-back').click();
  // the model step whose blob is not in the run, and the person's step that names none
  await acts.filter({hasText: 'report.write'}).click();
  await expect(field(page, 'action', 'output')).toContainText(NR);
  await detail(page, 'action').locator('.prod-back').click();
  await acts.filter({hasText: 'report.approve'}).click();
  await expect(field(page, 'action', 'output')).toContainText(NR);
  await expect(field(page, 'action', 'principal')).toContainText('decider');
  for (const el of await detail(page, 'action').locator('.pf-v').all()) expect((await el.innerText()).trim().length).toBeGreaterThan(0);
});

test('the product of the run is one click from the first screen', async ({page}) => {
  await open(page, 'viewed');
  const bar = page.locator('#product');
  await expect(bar).toContainText('work/example/example.md');
  await expect(bar).toContainText('named by the view');
  await bar.locator('.prod-link').click();
  await expect(page.locator('html')).toHaveClass(/rp-open/);
  await expect(detail(page, 'file').locator('.prod-path')).toHaveText('work/example/example.md');
  await expect(detail(page, 'file').locator('.md h1')).toHaveText('Example');
});

test('a view that names a file the run has not written says so in the words of the rule; with no key the newest Markdown files are linked and the bar says which rule it used', async ({page}) => {
  await open(page, 'unwritten', () => base2);
  await expect(page.locator('#product')).toContainText('the view names work/example/not-written.md; this run has not written it');
  await expect(page.locator('#product .prod-link')).toHaveCount(0);
  await open(page, 'newest', () => base3);
  const bar = page.locator('#product');
  await expect(bar).toContainText('newest Markdown file');
  await expect(bar.locator('.prod-link').first()).toHaveText('work/example/report.md');
  await bar.locator('.prod-link').first().click();
  await expect(detail(page, 'file').locator('.md h1')).toHaveText('Example report');
});

test('a run that names no product says so; an SDC run has its panels and its own files', async ({page}) => {
  await open(page, 'sdc');
  await expect(page.locator('#product')).toContainText('not named by this run');
  await tab(page);
  await expect(page.locator('#panel .prod-ev').first()).toBeVisible();
  await page.locator('#panel .prod-ev').first().click();
  await expect(field(page, 'evidence', 'producer')).toHaveText(/\S/);
  await detail(page, 'evidence').locator('.prod-back').click();
  await expect(page.locator('#panel .prod-file[data-path="store/IDEA-1/work/idea.yaml"]')).toBeVisible();
  await page.locator('#panel .prod-file[data-path="store/IDEA-1/work/idea.yaml"]').click();
  await expect(detail(page, 'file').locator('pre.prod-text')).not.toBeEmpty();
  await detail(page, 'file').locator('.prod-back').click();
  // the driver keeps an action's output as a blob of the case; the run records its exit
  await page.locator('#panel .prod-act').first().click();
  await expect(field(page, 'action', 'exit')).toHaveText(/^-?\d+$/);
  for (const name of ['duration', 'started', 'ended']) await expect(field(page, 'action', name)).toBeVisible();
  for (const name of ['duration', 'started', 'ended', 'stdout', 'stderr', 'output']) expect((await field(page, 'action', name).innerText()).trim().length).toBeGreaterThan(0);
});

test('the panels fit a phone: 390 px wide, no horizontal scroll of the page, in the lists, the rendered report and an evidence record', async ({page}) => {
  await page.setViewportSize({width: 390, height: 844});
  await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
  await open(page, 'viewed');
  // the phone contract of mobile.spec (UXF-023): a label is 12 px or more, any other text 14 px or more
  const small = () => page.evaluate(() => [...document.querySelectorAll('#product, #product *, #panel .prod, #panel .prod *')]
    .filter((e) => [...e.childNodes].some((n) => n.nodeType === 3 && n.nodeValue.trim()))
    .map((e) => ({sel: e.tagName.toLowerCase() + '.' + e.className, px: parseFloat(getComputedStyle(e).fontSize), label: e.classList.contains('prod-k') || (/^H[1-6]$/.test(e.tagName) && !e.closest('.md'))}))
    .filter((x) => x.px < (x.label ? 12 : 14)));
  const overflow = () => page.evaluate(() => Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth);
  expect(await overflow()).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  // every tap target is 44 px tall (UXF-022): the product link and each row of the lists
  expect(await page.locator('#product .prod-link').evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().height)).filter((h) => h < 44))).toEqual([]);
  await snap(page, 'first-screen');
  await tab(page);
  expect(await overflow()).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  expect(await page.locator('#panel .prod-lists button').evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().height)).filter((h) => h < 44))).toEqual([]);
  await snap(page, 'lists');
  await page.locator('#panel .prod-file[data-path="work/example/report.md"]').click();
  await expect(detail(page, 'file').locator('.md table')).toBeVisible();
  expect(await overflow()).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  await snap(page, 'report');
  await detail(page, 'file').locator('.prod-back').click();
  await page.locator('#panel .prod-ev[data-id="e0007-report.check"]').click();
  expect(await overflow()).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  await snap(page, 'evidence');
  await detail(page, 'evidence').locator('.prod-back').click();
  await page.locator('#panel .prod-act').filter({hasText: 'report.check'}).click();
  expect(await overflow()).toBeLessThanOrEqual(0);
  await snap(page, 'action');
});

test('screenshots of the panels and the rendered report at 1366x768', async ({page}) => {
  await page.setViewportSize({width: 1366, height: 768});
  await open(page, 'viewed');
  await snap(page, 'first-screen');
  await tab(page);
  await snap(page, 'lists');
  await page.locator('#panel .prod-file[data-path="work/example/report.md"]').click();
  await expect(detail(page, 'file').locator('.md table')).toBeVisible();
  await snap(page, 'report');
  await detail(page, 'file').locator('.prod-back').click();
  await page.locator('#panel .prod-ev[data-id="e0003-notes.check"]').click();
  await snap(page, 'evidence');
});
