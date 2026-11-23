// What a run wrote, in the page (story products-are-what-the-run-wrote): the Products tab groups the files the run's
// records name as written by an action by step, newest first, with the marks changed / added, and shows what the run was
// seeded with as one closed "Inputs" line. A run without the records shows today's list and a note. What a run holds
// (a file name, an action name) is text, never markup. One case per size the UI gate checks (the wide and the laptop
// viewport of playwright.config.mjs, and a phone of 390 px): no horizontal scroll, the phone's text and tap sizes.
// Own servers on ports the kernel gives; screenshots: SDC_UI_SHOTS, else ui/screenshots (git-ignored).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeWrittenRun, WRITTEN} from './written-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = process.env.SDC_UI_SHOTS || path.join(ui, 'screenshots');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'written-page');
let srv, srv2, base, base2;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, the sizes are set in the cases'));
test.afterAll(() => { srv?.kill(); srv2?.kill(); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  writeWrittenRun(path.join(runs, 'seeded'), {});
  writeWrittenRun(path.join(runs, 'older'), {seed: false});
  const hostile = [
    {step: 2, path: 'work/<b>bold<b>.txt', text: 'subject: x\nresult: <i>fine</i>\n', op: 'create', action: '<img src=x onerror="window.__pwned=1">'},
    {step: 3, path: 'work/<script>window.__pwned=2<script>.md', text: '# a\n', op: 'create', action: 'change.fix'},
  ];
  writeWrittenRun(path.join(runs, 'hostile'), {written: hostile});
  writeWrittenRun(path.join(runs, 'cutname'), {written: [{step: 2, path: 'work/a.md', text: '# a\n', op: 'create', action: 'x'.repeat(300)}]});
  writeWrittenRun(path.join(root, 'stopped', 'run'), {});
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
  // 201 seeded files and five written ones are 206 records: this server keeps 203 of them
  ({url: base2, proc: srv2} = await startServer(['--no-view', '--runs', path.join(root, 'stopped')], {env: {...process.env, SDC_UI_MAX_RECORDS: '203'}}));
});

const open = async (page, name) => {
  await page.goto(`${base}/?run=${name}`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible({timeout: 20000});
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
};
const tab = async (page) => {
  const phone = await page.evaluate(() => document.documentElement.classList.contains('phone'));
  if (phone && !(await page.locator('#tabs').isVisible())) await page.locator('#railbtn').click();
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-groups')).toBeVisible();
};
const overflow = (page) => page.evaluate(() => Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth);
const snap = (page, name) => { fs.mkdirSync(shots, {recursive: true}); return page.screenshot({path: path.join(shots, `${page.viewportSize().width}x${page.viewportSize().height}-written-${name}.png`)}); };

async function checkTheTab(page, size) {
  await open(page, 'seeded');
  // the product line names the last file of the last finished step, not a file of the seeded repository
  await expect(page.locator('#product .prod-link').first()).toHaveText('work/repro-result.txt');
  await tab(page);
  // with the right panel open the page at 1600 is wider than the window by the close button of the panel, as it is for every tab of it (not this
  // story's): the groups and the Inputs line must not add to it, and the tab itself must fit its panel
  const base0 = await overflow(page);
  const groups = page.locator('#panel .prod-group');
  await expect(groups).toHaveCount(3);
  expect(await groups.evaluateAll((els) => els.map((e) => e.dataset.step))).toEqual(['3', '2', '1']);
  await expect(page.locator('#panel .prod-group .prod-file')).toHaveCount(5);
  await expect(page.locator('#panel .prod-group[data-step="3"] h5')).toContainText('Step 3');
  await expect(page.locator('#panel .prod-group[data-step="2"] h5')).toContainText('bug.dissect, test.reproduce');
  await expect(page.locator('#panel .prod-file[data-path="work/impl/src/m7.rs"] .prod-mark')).toHaveText('changed');
  await expect(page.locator('#panel .prod-file[data-path="work/impl/tests/repro.rs"] .prod-mark')).toHaveText('added');
  await expect(page.locator('#panel .prod-file[data-path="work/dissection.md"] .prod-mark')).toHaveCount(0);
  await expect(page.locator('#panel .prod-file[data-path="work/repro-result.txt"] .prod-result')).toHaveText('red');
  await expect(page.locator('#panel .prod-files')).not.toContainText('mainline');
  // the Inputs line is one line, closed, with the counts; opened it names the trees
  const inputs = page.locator('#panel details.prod-inputs');
  await expect(inputs).toHaveCount(1);
  expect(await inputs.evaluate((e) => e.open)).toBe(false);
  await expect(inputs.locator('summary')).toContainText('201 files');
  await expect(inputs.locator('.prod-input').first()).toBeHidden();
  await inputs.locator('summary').click();
  await expect(inputs.locator('.prod-input')).toHaveCount(3);
  await expect(inputs.locator('.prod-input[data-path="mainline"]')).toContainText('100 files');
  expect(await overflow(page)).toBeLessThanOrEqual(base0);
  expect(await page.locator('#panel .prod').evaluate((e) => e.scrollWidth - e.clientWidth)).toBeLessThanOrEqual(0);
  await snap(page, `${size}-tab`);
  // a product opens in the same panel, as text
  await page.locator('#panel .prod-file[data-path="work/dissection.md"]').click();
  await expect(page.locator('#panel .prod-detail[data-kind=file]')).toContainText('The invented cause lies in module 7');
  expect(await overflow(page)).toBeLessThanOrEqual(base0);
  return page;
}

test('1600x1000: the products by step, the marks and the closed Inputs line', async ({page}) => {
  await page.setViewportSize({width: 1600, height: 1000});
  await checkTheTab(page, 'wide');
});

test('1280x800: the products by step, the marks and the closed Inputs line', async ({page}) => {
  await page.setViewportSize({width: 1280, height: 800});
  await checkTheTab(page, 'laptop');
});

test('a phone of 390 px: no horizontal scroll, text of 14 px and more, every tap target 44 px', async ({page}) => {
  await page.setViewportSize({width: 390, height: 844});
  await page.addInitScript(() => { try { localStorage.clear(); } catch { /* first load */ } });
  await open(page, 'seeded');
  await tab(page);
  const small = () => page.evaluate(() => [...document.querySelectorAll('#product, #product *, #panel .prod, #panel .prod *')]
    .filter((e) => [...e.childNodes].some((n) => n.nodeType === 3 && n.nodeValue.trim()))
    .map((e) => ({sel: e.tagName.toLowerCase() + '.' + e.className, px: parseFloat(getComputedStyle(e).fontSize), label: e.classList.contains('prod-k') || (/^H[1-6]$/.test(e.tagName) && !e.closest('.md'))}))
    .filter((x) => x.px < (x.label ? 12 : 14)));
  expect(await overflow(page)).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  expect(await page.locator('#panel .prod-lists button, #panel .prod-inputs summary').evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().height)).filter((h) => h < 44))).toEqual([]);
  await page.locator('#panel details.prod-inputs summary').click();
  expect(await overflow(page)).toBeLessThanOrEqual(0);
  expect(await small()).toEqual([]);
  await snap(page, 'phone-tab');
});

test('a run without the records of a seed shows today\'s list of files and says why', async ({page}) => {
  await page.setViewportSize({width: 1600, height: 1000});
  await open(page, 'older');
  await tab_listing(page);
  await expect(page.locator('#panel .prod-basis')).toContainText('seeded.json');
  await expect(page.locator('#panel .prod-group')).toHaveCount(0);
  await expect(page.locator('#panel details.prod-inputs')).toHaveCount(0);
  await expect(page.locator('#panel .prod-file[data-path="work/mainline/src/m1.rs"]')).toBeVisible();
});
async function tab_listing(page) {
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-files')).toBeVisible();
}

test('what a run holds is text: a file name and an action name with markup are shown as written, nothing runs', async ({page}) => {
  await page.setViewportSize({width: 1600, height: 1000});
  await open(page, 'hostile');
  await tab(page);
  const panel = page.locator('#panel .prod-groups');
  await expect(panel).toContainText('<b>bold<b>.txt');
  await expect(panel).toContainText('<img src=x onerror="window.__pwned=1">');
  await expect(panel).toContainText('<script>window.__pwned=2<script>.md');
  expect(await panel.locator('b, i, img, script').count()).toBe(0);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  expect(WRITTEN.length).toBe(5);
});

test('an action name that was cut and a list of changes that was not read to its end are said on the tab', async ({page}) => {
  await page.setViewportSize({width: 1600, height: 1000});
  await open(page, 'cutname');
  await tab(page);
  await expect(page.locator('#panel .prod-note').filter({hasText: '1 action name is cut to 256 characters'})).toHaveCount(1);
  const head = await page.locator('#panel .prod-group[data-step="2"] h5').innerText();
  expect(head).toContain('x'.repeat(256));
  expect(head).not.toContain('x'.repeat(257));
  await page.goto(`${base2}/`);
  await expect(page.locator('#product')).toBeVisible({timeout: 20000});
  await tab(page);
  const note = page.locator('#panel .prod-cut').filter({hasText: 'read to record 203 only'});
  await expect(note).toHaveCount(1);
  await expect(note).toContainText('the records after them are not read');
  await expect(page.locator('#panel .prod-group .prod-file')).toHaveCount(2);
});
