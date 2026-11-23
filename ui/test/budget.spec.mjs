// The test step's budget is held by the runner (playwright.config.mjs: 60 s a test, 120 s the run, the
// ten slowest printed), and this file holds the config and the specs to it: a spec that sets a longer
// test timeout would take its test out of the runner's reach, and a fixed port would make a test talk
// to whatever else listens there. Pure file checks, no browser.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import gate, {BUDGET} from '../playwright.config.mjs';
import {slowFiles, bothFiles, wideOnlyFiles, WIDE_ONLY, BOTH, SIZE_EXCEPTIONS, FIXED_WAITS} from './lanes.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const specs = fs.readdirSync(here).filter((f) => f.endsWith('.spec.mjs'));
const gateSpecs = specs.filter((f) => !slowFiles.includes(f));
const text = (f) => fs.readFileSync(path.join(here, f), 'utf8');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'file checks run once'));

test('the runner enforces 60 s a test and 120 s the run, and prints the ten slowest tests', () => {
  expect(BUDGET).toEqual({testMs: 60000, runMs: 120000});
  expect(gate.timeout).toBeLessThanOrEqual(60000);
  expect(gate.globalTimeout).toBeGreaterThan(0);
  expect(gate.globalTimeout).toBeLessThanOrEqual(120000);
  expect(JSON.stringify(gate.reporter)).toContain('slowest-reporter.mjs');
});

test('no spec of the gate lane sets a test timeout over 60 s or marks a test slow', () => {
  const bad = [];
  for (const f of gateSpecs) {
    for (const m of text(f).matchAll(/test\.setTimeout\((\d+)\)/g)) if (Number(m[1]) > BUDGET.testMs) bad.push(`${f}: setTimeout(${m[1]})`);
    if (/test\.slow\(|test\.setTimeout\(\s*0\s*\)/.test(text(f))) bad.push(`${f}: test.slow or no timeout`);
  }
  expect(bad).toEqual([]);
});

test('every slow-lane spec exists, and no spec of the gate lane needs runs/, a built binary or a server of somebody else', () => {
  for (const f of slowFiles) expect(specs, `${f} is named in test/lanes.mjs`).toContain(f);
  const bad = [];
  for (const f of gateSpecs.filter((x) => x !== 'budget.spec.mjs')) {
    const t = text(f);
    if (/needDriver\(|needTool\(/.test(t)) bad.push(`${f}: needs a built binary`);
    if (/['"/]runs\/(trial-todo-1|full-cycle-1)/.test(t)) bad.push(`${f}: reads runs/`);
    if (/SDC_UI_(URL|WRITE_URL|LIVE_DIR)/.test(t)) bad.push(`${f}: needs a server somebody else started`);
  }
  expect(bad).toEqual([]);
});

const code = (f) => text(f).split('\n').filter((l) => !/^\s*\/\//.test(l) && !/test\.skip\(/.test(l)).join('\n').replace(/\s\/\/ .*$/gm, '');
const SIZE = /setViewportSize|viewport\s*:|viewportSize|boundingBox|getBoundingClientRect|scrollWidth|innerWidth|toHaveScreenshot|\.screenshot\(|\.isVisible\(|toBeVisible|toBeHidden|isHidden|toBeInViewport|scrollIntoViewIfNeeded|matchMedia/;

test('every spec file is in exactly one list of test/lanes.mjs: both viewports, wide only, or slow', () => {
  const lists = {both: bothFiles, wide: wideOnlyFiles, slow: slowFiles};
  const bad = [];
  for (const f of specs) {
    const in_ = Object.entries(lists).filter(([, l]) => l.includes(f)).map(([k]) => k);
    if (in_.length !== 1) bad.push(`${f}: in ${in_.length ? in_.join(' and ') : 'no list'}`);
  }
  for (const [k, l] of Object.entries(lists)) for (const f of l) if (!specs.includes(f)) bad.push(`${f}: listed in ${k}, not a spec file under test/`);
  expect(bad).toEqual([]);
});

test('every reason of WIDE_ONLY, BOTH and SIZE_EXCEPTIONS is one to five words, none empty', () => {
  const words = (why) => why.trim().split(/\s+/).filter(Boolean).length;
  const bad = [];
  for (const [list, map] of Object.entries({WIDE_ONLY, BOTH, SIZE_EXCEPTIONS})) {
    for (const [name, why] of Object.entries(map)) if (words(why) < 1 || words(why) > 5) bad.push(`${list} ${name}: "${why}" (${words(why)} words)`);
  }
  expect(bad).toEqual([]);
});

test('a file listed for both viewports has no skip on the project name, which would leave one of the sizes unrun', () => {
  const bad = [];
  for (const f of bothFiles.filter((x) => specs.includes(x))) {
    if (text(f).split('\n').some((l) => !/^\s*\/\//.test(l) && /project\.name\s*!==?\s*['"]wide['"]|project\.name\s*===?\s*['"]laptop['"]/.test(l))) bad.push(`${f}: listed for both viewports and skips on the project name`);
  }
  expect(bad).toEqual([]);
});

test('a fixed wait (waitForTimeout) is made only where FIXED_WAITS names the file and the count', () => {
  const bad = [];
  for (const f of gateSpecs.filter((x) => x !== 'budget.spec.mjs')) {
    const n = (code(f).match(/waitForTimeout\(/g) ?? []).length;
    const allowed = FIXED_WAITS[f] ?? 0;
    if (n > allowed) bad.push(`${f}: ${n} fixed waits, ${allowed} allowed: wait for an event (settled, expect, expect.poll)`);
    if (n < allowed) bad.push(`${f}: ${allowed} fixed waits allowed, ${n} made: lower the entry in FIXED_WAITS`);
  }
  for (const f of Object.keys(FIXED_WAITS)) if (!gateSpecs.includes(f)) bad.push(`${f}: in FIXED_WAITS, not a spec of the gate lane`);
  expect(bad).toEqual([]);
});

test('SIZE matches every kind of line the README and lanes.mjs say a size check is', () => {
  const NAMED = {
    'a viewport': ['viewport: {width: 390}', 'page.viewportSize()', 'page.setViewportSize(x)'],
    'a position': ['el.boundingBox()', 'el.getBoundingClientRect()', 'document.documentElement.scrollWidth', 'window.innerWidth'],
    'a screenshot': ['expect(page).toHaveScreenshot()', 'page.screenshot({})'],
    'a visibility check': ['el.isVisible()', 'expect(el).toBeVisible()', 'expect(el).toBeHidden()', 'el.isHidden()', 'expect(el).toBeInViewport()', 'el.scrollIntoViewIfNeeded()', "window.matchMedia('(max-width: 800px)')"],
  };
  const missed = [];
  for (const [what, lines] of Object.entries(NAMED)) for (const l of lines) if (!SIZE.test(l)) missed.push(`${what}: ${l}`);
  expect(missed).toEqual([]);
  expect(SIZE.test('await page.locator("#x").click()')).toBe(false);
});

test('a spec that names a size or a position is listed wide only only with a reason in SIZE_EXCEPTIONS', () => {
  const bad = [];
  for (const f of wideOnlyFiles.filter((x) => specs.includes(x) && x !== 'budget.spec.mjs')) if (SIZE.test(code(f)) && !(f in SIZE_EXCEPTIONS)) bad.push(`${f}: names a size or a position and runs in wide only`);
  for (const f of Object.keys(SIZE_EXCEPTIONS)) {
    if (!wideOnlyFiles.includes(f)) bad.push(`${f}: an exception for a file that is not wide only`);
    else if (!SIZE.test(code(f))) bad.push(`${f}: an exception for a file that names no size or position`);
  }
  expect(bad).toEqual([]);
});

test('the laptop project leaves out the wide-only and slow files, the wide project runs every gate file', () => {
  const ignored = (name) => [gate.testIgnore ?? [], gate.projects.find((p) => p.name === name)?.testIgnore ?? []].flat().map((g) => g.replace(/^\*\*\//, ''));
  const names = (name) => [...new Set(ignored(name))].sort();
  expect(names('laptop')).toEqual([...new Set([...wideOnlyFiles, ...slowFiles])].sort());
  expect(names('wide')).toEqual([...slowFiles].sort());
});

test('no file under test/ names a listening port: servers take one from the kernel (test/serve.mjs)', () => {
  const bad = [];
  for (const f of fs.readdirSync(here).filter((x) => x.endsWith('.mjs') && x !== 'budget.spec.mjs')) {
    const t = text(f);
    for (const m of t.matchAll(/--port['"],\s*(?:String\()?['"]?(\d+)|SDC_UI_[A-Z]+_PORT|\bport\s*(?:=|\|\|)\s*\d{4}|127\.0\.0\.1:\d{4}/g)) if (m[1] !== '0') bad.push(`${f}: ${m[0]}`);
  }
  expect(bad).toEqual([]);
});
