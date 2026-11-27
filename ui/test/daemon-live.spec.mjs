// The run page of `runbook serve --ui` while a run runs (story run-page-watches-a-live-run). One synthetic daemon
// (daemon-live-fixture.mjs: the runbook `investigate`, six steps done, the seventh in flight) in this worker, and a
// second that holds the last two events back, so a test moves the run while the page shows it. Before each page load the
// fixture anchors its times to now (the step in flight started 132 s ago): a faked page clock would hide the Server-Timing
// name the page reads to know the daemon served it. The browser runs in the zone Europe/Berlin with the locale en-US: the
// 24 h time and the zone's name must not depend on the locale. One case per finding of the before screenshots
// (ui/ux/findings/UXF-027.md to UXF-036.md), then the loop's hard gates at the four gating viewports (ui/ux/adapter.mjs).
import {test, expect as plain} from '@playwright/test';
import {serve, LIVE, DONE} from './daemon-live-fixture.mjs';
import {viewports, daemonPage} from '../ux/adapter.mjs';
import {scrollOffenders, visibilityMisses, axeViolations} from '../ux/core/contract.mjs';

const expect = plain.configure({timeout: 10000});
const ZONE = {timezoneId: 'Europe/Berlin', locale: 'en-US'};
const CLOCK = /^\d{2}:\d{2}:\d{2} CES?T$/;
// 2 min 12 s and the few seconds a test takes after the anchor
const NEAR_132 = /^2 min 1[2-9] s$/;
let fx, held;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon; the gating sizes are set per test'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  [fx, held] = await Promise.all([serve(), serve({hold: 2})]);
});
test.afterAll(() => { fx?.close(); held?.close(); });
test.use(ZONE);

async function open(page, {run = LIVE, ago = 132000, d = fx} = {}) {
  await fetch(`${d.base}/__fixture/anchor?ago=${ago}`);
  await page.goto(`${d.base}/?run=${run}`);
  await expect(page.locator('#tree .case')).toHaveCount(1);
}
const problems = (page) => { const p = []; page.on('pageerror', (e) => p.push(String(e))); page.on('console', (m) => { if (m.type() === 'error') p.push(m.text()); }); return p; };

test('UXF-027: the centre shows the run\'s steps in order, the step in flight marked and live, and the counts', async ({page}) => {
  const errs = problems(page);
  await open(page);
  const steps = page.locator('#dsteps');
  await expect(steps).toBeVisible();
  await expect(steps.locator('tbody tr .ds-act')).toHaveText(['subject.read', 'evidence.deploy', 'evidence.tickets', 'evidence.repo', 'evidence.chat', 'report.write', 'findings.judge']);
  await expect(steps.locator('tbody tr[data-state=running]')).toHaveCount(1);
  const running = steps.locator('tbody tr[data-state=running]');
  await expect(running.locator('.ds-act')).toHaveText('findings.judge');
  await expect(running.locator('.ds-role')).toHaveText('program');
  await expect(running.locator('.ds-try')).toHaveText('1');
  await expect(running.locator('.ds-took')).toHaveText(NEAR_132);
  await expect(steps.locator('tbody tr').nth(3).locator('.ds-st')).toHaveText('exit 10');
  await expect(steps.locator('tbody tr').nth(0).locator('.ds-st')).toHaveText('done');
  await expect(steps.locator('tbody tr').nth(2).locator('.ds-took')).toHaveText('4.0 s');
  // the head says what runs now, in words, and the duration ticks
  await expect(page.locator('#dnow')).toHaveText(/^Running step 7: findings\.judge, program, try 1, since \d{2}:\d{2}:\d{2} CES?T, for 2 min 1[2-9] s$/);
  const first = await running.locator('.ds-took').innerText();
  await expect(running.locator('.ds-took')).not.toHaveText(first);
  // five steps ended with exit 0 and one with exit 10: two counts, no step in both
  await expect(page.locator('#dsteps .ds-counts span').first()).toHaveText('5 done, 1 exited non-zero');
  await expect(page.locator('#dsteps .ds-counts')).toContainText('1 model call');
  await expect(page.locator('#dsteps .ds-counts')).toContainText('2,000 tokens');
  await expect(page.locator('#dsteps .ds-counts')).toContainText('$0.0042');
  expect(errs).toEqual([]);
});

test('UXF-027: a step that finishes and the next that starts show without a reload; an ended run says how it ended', async ({page}) => {
  await open(page, {d: held});
  // the two held events: the engine's evaluation and the start of findings.judge
  await expect(page.locator('#dsteps tbody tr')).toHaveCount(6);
  await expect(page.locator('#dnow')).toHaveText(/^Between steps/);
  for (let i = 0; i < 2; i++) await fetch(`${held.base}/__fixture/next`);
  await expect(page.locator('#dsteps tbody tr')).toHaveCount(7);
  await expect(page.locator('#dsteps tbody tr[data-state=running] .ds-act')).toHaveText('findings.judge');
  await page.goto(`${fx.base}/?run=${DONE}`);
  await expect(page.locator('#dnow')).toHaveText(/^Ended: investigated/);
  await expect(page.locator('#dsteps tbody tr')).toHaveCount(7);
  await expect(page.locator('#dsteps tbody tr[data-state=running]')).toHaveCount(0);
  await expect(page.locator('#dsteps .ds-counts')).toContainText('2 model calls');
});

test('UXF-028: times are 24 h local time of the browser with the zone\'s name', async ({page}) => {
  await open(page);
  await expect(page.locator('#dsteps tbody tr').first().locator('.ds-at')).toHaveText(CLOCK);
  await expect(page.locator('#feed .fd-t').first()).toHaveText(CLOCK);
  for (const t of await page.locator('#feed .fd-t').allInnerTexts()) expect(t).toMatch(CLOCK);
  await expect(page.locator('#live .lt')).toHaveText(/^updated \d{2}:\d{2}:\d{2} CES?T$/);
  expect(await page.locator('body').innerText()).not.toMatch(/\b\d{1,2}:\d{2}(:\d{2})? ?[AP]M\b/);
});

test('UXF-029: no run event prints a raw record or a digest', async ({page}) => {
  await open(page);
  await expect(page.locator('#feed .fd-row').first()).toBeVisible();
  const text = await page.locator('#feed').innerText();
  expect(text).not.toContain('{"');
  expect(text).not.toMatch(/[0-9a-f]{32}/);
});

test('UXF-030: one form of event name; the engine\'s evaluation and step-end lines are folded, and shown in words on ask', async ({page}) => {
  await open(page);
  const heads = page.locator('#feed .fd-row .fd-s');
  // 30 events: 7 evaluations and 6 step-budget stops folded (each ends one step, not the run)
  await expect(heads).toHaveCount(17);
  const words = await heads.allInnerTexts();
  for (const w of words) expect(w, 'a sentence-case name of words').toMatch(/^[A-Z][a-z]+( [a-z]+)*$/);
  expect(words.slice(0, 3)).toEqual(['Step started', 'Model call', 'Step finished']);
  await expect(page.locator('#feed .fd-h')).toContainText('13 engine lines folded');
  await page.getByRole('button', {name: 'Show all 30'}).click();
  await expect(heads).toHaveCount(30);
  await expect(page.locator('#feed .fd-row', {hasText: 'Step ended'}).first()).toContainText('the run goes on');
  await expect(page.locator('#feed .fd-s', {hasText: /^Stopped$/})).toHaveCount(0);
  await expect(page.locator('#feed')).not.toContainText('BudgetSpent');
});

test('UXF-031: the title is the runbook\'s name; the run id is short beside it, whole on hover', async ({page}) => {
  await open(page);
  await expect(page.locator('#gtitle .gt-id')).toHaveText('investigate');
  const run = page.locator('#gtitle .gt-run');
  await expect(run).toHaveText('run 0000a1b2');
  await expect(run).toHaveAttribute('title', `run ${LIVE}`);
  await expect(page.locator('#gtitle .gt-main')).not.toContainText(LIVE);
  await expect(page.locator('#gtitle .pill')).toHaveText('Running');
  const size = (sel) => page.locator(sel).evaluate((e) => parseFloat(getComputedStyle(e).fontSize));
  expect(await size('#gtitle .gt-id')).toBeGreaterThan(await size('#gtitle .gt-run'));
});

test('UXF-032: the case card cuts no text; the run line above it lines up with the tree', async ({page}) => {
  await open(page);
  const cut = await page.locator('#left').evaluate((root) => [...root.querySelectorAll('*')].filter((e) => e.offsetParent && e.children.length === 0 && e.textContent.trim() && e.scrollWidth > e.clientWidth + 1).map((e) => e.textContent.trim()));
  expect(cut).toEqual([]);
  await expect(page.locator('#tree .case .cid')).toHaveText('0000a1b2');
  await expect(page.locator('#tree .case .cid')).toHaveAttribute('title', LIVE);
  // where the run line's text starts, not its box: the padding is inside the box
  const pad = await page.evaluate(() => {
    const r = document.createRange();
    r.selectNodeContents(document.querySelector('[data-key=dhead]'));
    return r.getBoundingClientRect().left - document.querySelector('#left').getBoundingClientRect().left;
  });
  expect(pad).toBeGreaterThanOrEqual(12);
});

// The root case card names the run by the last 8 characters of its id, as the run list does (web/source.js), on one
// line; the whole id is its title and a copy control's, which writes it to the clipboard and is reached with Tab.
for (const vp of viewports.gating.filter((v) => ['1366x768', '390x844'].includes(v.name))) {
  test(`the case card at ${vp.name}: the run id short on one line, whole on hover and by a copy control reached with Tab`, async ({browser}) => {
    const ctx = await browser.newContext({...ZONE, viewport: {width: vp.width, height: vp.height}, ...(vp.phone ? {isMobile: true, hasTouch: true} : {}), reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write']});
    const page = await ctx.newPage();
    await open(page);
    const cid = page.locator('#tree .case .cid');
    await expect(cid).toHaveText('0000a1b2');
    await expect(cid).toHaveAttribute('title', LIVE);
    const lines = await cid.evaluate((e) => { const r = document.createRange(); r.selectNodeContents(e); return {lines: r.getClientRects().length, cut: e.scrollWidth > e.clientWidth + 1}; });
    expect(lines).toEqual({lines: 1, cut: false});
    const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
    await expect(copy).toHaveAttribute('title', `Copy the run id ${LIVE}`);
    let tabs = 0;
    while (tabs < 40 && !(await copy.evaluate((e) => e === document.activeElement))) { await page.keyboard.press('Tab'); tabs++; }
    expect(tabs, 'the copy control reached by Tab').toBeLessThan(40);
    // focused, it is whole in view (a phone scrolls the page down to the tree)
    await expect(copy).toBeInViewport({ratio: 1});
    await page.keyboard.press('Enter');
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(LIVE);
    await ctx.close();
  });
}

test('UXF-033: no empty band between the header and the run; the files line sits with the steps', async ({page}) => {
  await open(page);
  const gap = await page.evaluate(() => document.querySelector('main').getBoundingClientRect().top - document.querySelector('header').getBoundingClientRect().bottom);
  expect(gap).toBeLessThanOrEqual(1);
  await expect(page.locator('#dsteps #product')).toContainText('6 files under the run\'s work/ directory');
});

test('UXF-034: the panel has only the tabs the daemon serves data for, their names written across; the closed drawer is not laid out', async ({page}) => {
  await open(page);
  const tabs = page.locator('#tabs button[data-tab]:visible');
  await expect(tabs).toHaveText(['Products', 'Events']);
  // a tab with no data is no button at all (not in the page's reading order either)
  await expect(page.locator('#tabs button[data-tab]:not([hidden])')).toHaveCount(2);
  for (const t of await tabs.all()) expect(await t.evaluate((e) => getComputedStyle(e).writingMode)).toBe('horizontal-tb');
  const box = await page.locator('#drawer').evaluate((e) => { const r = e.getBoundingClientRect(); return {w: r.width, right: r.right}; });
  expect(box.w === 0 || box.right <= 1600).toBe(true);
  await tabs.first().click();
  await expect(page.locator('#panel .prod-daemon')).toBeVisible();
  await expect(page.locator('#panel .prod-file')).toHaveCount(6);
  // the Events tab: every event of the run, oldest first, in words; nothing in the panel says what is missing
  await page.locator('#tabs button[data-tab=events]').click();
  await expect(page.locator('#panel .ev-row')).toHaveCount(30);
  await expect(page.locator('#panel .ev-row').first()).toContainText('Run submitted');
  await expect(page.locator('#panel .ev-row').nth(6)).toContainText('Step ended');
  await expect(page.locator('#panel')).not.toContainText('serves no');
  await page.locator('#decbtn').click();
  await expect(page.locator('#drawer')).toBeInViewport();
});

test('UXF-035: no legend of a graph that is not drawn', async ({page}) => {
  await page.setViewportSize({width: 1920, height: 1080});
  await open(page);
  await expect(page.locator('.legend')).toBeHidden();
  await expect(page.locator('#zfit')).toBeHidden();
});

test('UXF-036: on a phone the header controls do not cover each other; the state pill holds its words; the bar says what runs', async ({browser}) => {
  const ctx = await browser.newContext({...ZONE, viewport: {width: 390, height: 844}, isMobile: true, hasTouch: true, reducedMotion: 'reduce'});
  const page = await ctx.newPage();
  await open(page);
  await expect(page.locator('#startbtn')).toBeVisible();
  const boxes = await page.evaluate(() => ['#mode', '.selectw', 'header .switch', '#decbtn', '#startbtn', '#routesbtn', '#railbtn', '#live'].map((s) => { const e = document.querySelector(s); const r = e?.getBoundingClientRect(); return {s, r: r && r.width ? {l: r.left, t: r.top, r: r.right, b: r.bottom} : null}; }));
  const over = [];
  for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i].r, b = boxes[j].r;
    if (a && b && a.l < b.r - 1 && b.l < a.r - 1 && a.t < b.b - 1 && b.t < a.b - 1) over.push(`${boxes[i].s} over ${boxes[j].s}`);
  }
  expect(over).toEqual([]);
  for (const s of ['#mode', '#startbtn', '#routesbtn', '#decbtn']) await expect(page.locator(s), s).toBeInViewport({ratio: 1});
  const pill = await page.locator('#gtitle .pill').evaluate((e) => ({fits: e.scrollWidth <= e.clientWidth + 1, h: e.getBoundingClientRect().height}));
  expect(pill.fits).toBe(true);
  expect(pill.h).toBeLessThan(26);
  await expect(page.locator('#nowbar .nb-state')).toHaveText('Running');
  await expect(page.locator('#nowbar .nb-step')).toContainText('findings.judge');
  await ctx.close();
});

test('UXF-037: no pane of the page explains what the daemon does not serve in place of showing something', async ({page}) => {
  await open(page);
  const missing = /serves no|does not serve|no count of/i;
  await expect(page.locator('#gtitle .gt-id')).toBeVisible();
  expect(await page.locator('#center').innerText()).not.toMatch(missing);
  for (const t of await page.locator('#tabs button[data-tab]:visible').all()) {
    await t.click();
    await expect(page.locator('#panel .tabpane, #panel .prod').first()).toBeVisible();
    expect(await page.locator('body').innerText()).not.toMatch(missing);
  }
  await page.locator('#routesbtn').click();
  await expect(page.locator('#routes-body tr')).toHaveCount(1);
  expect(await page.locator('#routesdlg').innerText()).not.toMatch(missing);
});

// The loop's hard gates (SKILL § 4) for the run page at each gating viewport: no scroll but what the baseline allows (a
// phone scrolls down, never sideways), the always-visible controls whole in view, axe-core 0 serious or critical, and the
// steps reached with the keyboard alone.
for (const vp of viewports.gating) {
  test(`hard gates at ${vp.name}: scroll, always visible, axe, keyboard`, async ({browser}) => {
    const ctx = await browser.newContext({...ZONE, viewport: {width: vp.width, height: vp.height}, ...(vp.phone ? {isMobile: true, hasTouch: true} : {}), reducedMotion: 'reduce'});
    const page = await ctx.newPage();
    await open(page);
    await expect(page.locator('#startbtn')).toBeVisible();
    const pol = daemonPage(vp.name);
    const scroll = (await scrollOffenders(page, pol.scrollAllowed)).filter((x) => !(pol.pageScrollY && x.sel === 'document' && x.w <= vp.width + 1));
    expect(scroll, 'hard gate 1').toEqual([]);
    expect(await visibilityMisses(page, pol.alwaysVisible), 'hard gate 2').toEqual([]);
    expect(await axeViolations(page), 'hard gate 5: axe').toEqual([]);
    let tabs = 0;
    while (tabs < 40 && !(await page.evaluate(() => document.activeElement?.id === 'dsteps'))) { await page.keyboard.press('Tab'); tabs++; }
    expect(tabs, 'hard gate 5: the steps reached by Tab').toBeLessThan(40);
    await ctx.close();
  });
}

// Adversary cases of unit U2r (run page round 2): the copy control of the root case card.
// What a person perceives of a copy: the control's own words change or a live region says it (not a title, which a phone
// and a keyboard never show, and not a colour alone).
const copyTold = (page) => page.evaluate(() => {
  const b = document.querySelector('#tree .cid-copy');
  const live = [...document.querySelectorAll('[role=status],[role=alert],[aria-live]')].map((e) => e.textContent).join(' ');
  return b?.textContent.trim() !== 'Copy' || /run id/i.test(live);
});

test('adversary U2r: a copy the browser cannot make (no clipboard: an insecure context) raises no error and is told on the page', async ({browser}) => {
  const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce'});
  await ctx.addInitScript(() => Object.defineProperty(Navigator.prototype, 'clipboard', {configurable: true, get: () => undefined}));
  const page = await ctx.newPage();
  const errs = problems(page);
  await open(page);
  const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
  await copy.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => copy.getAttribute('title'), 'the handler ran').toMatch(/did not allow/);
  expect(errs, 'no uncaught error').toEqual([]);
  await expect.poll(() => copyTold(page), {message: 'the failed copy is told by the control\'s words or a live region', timeout: 3000}).toBe(true);
  await ctx.close();
});

test('adversary U2r: a copy that worked is told on the page in words, not by a colour alone', async ({browser}) => {
  const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write']});
  const page = await ctx.newPage();
  await open(page);
  const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
  await copy.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(LIVE);
  await expect.poll(() => copyTold(page), {message: 'the copy is told by the control\'s words or a live region', timeout: 3000}).toBe(true);
  await ctx.close();
});

test('adversary U2r: a copy refused once and then made: the control no longer says the browser did not allow it', async ({browser}) => {
  const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce'});
  await ctx.addInitScript(() => {
    let calls = 0;
    window.__copied = [];
    const fake = {writeText: (t) => (++calls === 1 ? Promise.reject(new DOMException('denied', 'NotAllowedError')) : (window.__copied.push(t), Promise.resolve()))};
    Object.defineProperty(Navigator.prototype, 'clipboard', {configurable: true, get: () => fake});
  });
  const page = await ctx.newPage();
  await open(page);
  const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
  await copy.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => copy.getAttribute('title')).toMatch(/did not allow/);
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => window.__copied)).toEqual([LIVE]);
  await expect(copy).not.toHaveAttribute('title', /did not allow/);
  await ctx.close();
});

for (const vp of viewports.gating.filter((v) => ['1600x1000', '805x1270'].includes(v.name))) {
  test(`adversary U2r: the case card at ${vp.name}: the run id short on one line, the copy control whole`, async ({browser}) => {
    const ctx = await browser.newContext({...ZONE, viewport: {width: vp.width, height: vp.height}, reducedMotion: 'reduce'});
    const page = await ctx.newPage();
    await open(page);
    const cid = page.locator('#tree .case .cid');
    await expect(cid).toHaveText('0000a1b2');
    const m = await page.locator('#tree .case').first().evaluate((card) => {
      const e = card.querySelector('.cid'), b = card.querySelector('.cid-copy');
      const r = document.createRange(); r.selectNodeContents(e);
      const cr = card.getBoundingClientRect(), br = b.getBoundingClientRect();
      return {lines: r.getClientRects().length, cut: e.scrollWidth > e.clientWidth + 1, copyInside: br.left >= cr.left - 1 && br.right <= cr.right + 1, copyCut: b.scrollWidth > b.clientWidth + 1};
    });
    expect(m).toEqual({lines: 1, cut: false, copyInside: true, copyCut: false});
    await ctx.close();
  });
}

test('adversary U2r: a root id that looks like markup, and one shorter than 8 characters: text in the card, whole in the title; the clipboard gets the run id', async ({browser}) => {
  for (const id of ['r"><img src=x onerror=window.__pwned=70>&amp;\'x', 'r1']) {
    const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write']});
    const page = await ctx.newPage();
    const errs = problems(page);
    await page.route((u) => u.pathname === '/daemon/views/cases', (q) => q.fulfill({status: 200, contentType: 'application/json', body: JSON.stringify({rows: [{case_id: id, protocol: 'investigate', case_state: 'Running'}]})}));
    await open(page);
    const cid = page.locator('#tree .case .cid');
    await expect(cid).toHaveText(id.slice(-8));
    await expect(cid).toHaveAttribute('title', id);
    await page.locator('#tree .case').getByRole('button', {name: 'Copy run id'}).click();
    // the control copies the run's own id, not the root case's: the label "Copy run id" is true by construction
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(LIVE);
    expect(await page.evaluate(() => window.__pwned ?? null)).toBeNull();
    expect(errs).toEqual([]);
    await ctx.close();
  }
});

// What the copy control says of the last copy is the page's state: a redraw while it shows (an event of the run
// arrives) keeps it, and it goes back to "Copy" by itself.
test('the copy control says "Copied" in its words and its status line through a redraw, then "Copy" again', async ({browser}) => {
  const own = await serve({hold: 1});
  const ctx = await browser.newContext({...ZONE, viewport: {width: 1366, height: 768}, reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write']});
  const page = await ctx.newPage();
  await open(page, {d: own});
  const copy = page.locator('#tree .case').getByRole('button', {name: 'Copy run id'});
  const status = page.locator('#tree .case [role=status]');
  await expect(copy).toHaveText('Copy');
  await expect(status).toHaveText('');
  await copy.click();
  await expect(copy).toHaveText('Copied');
  await expect(status).toHaveText('Run id copied');
  // the held event: the step in flight starts and the page draws again
  await fetch(`${own.base}/__fixture/next`);
  await expect(page.locator('#dsteps tbody tr[data-state=running]')).toHaveCount(1);
  await expect(copy).toHaveText('Copied');
  await expect(copy).toHaveText('Copy', {timeout: 5000});
  await expect(status).toHaveText('');
  await expect(copy).toHaveAttribute('title', `Copy the run id ${LIVE}`);
  await ctx.close();
  own.close();
});
