// The test step's budget is held by the runner (playwright.config.mjs: 60 s a test, 120 s the run, the ten
// slowest printed), and this file holds the config and the specs to it: a spec that sets a longer
// test timeout would take its test out of the runner's reach, and a fixed port would make a test talk
// to whatever else listens there. Pure file checks, no browser. Each check is a function of its inputs
// (budget-checks.mjs), and its case calls it on the live lists and on a good and a bad input of its own, so
// that a check that is broken fails a case.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import gate, {BUDGET} from '../playwright.config.mjs';
import * as lanes from './lanes.mjs';
import {slowFiles, bothFiles, wideOnlyFiles, WIDE_ONLY, BOTH, SIZE_EXCEPTIONS, FIXED_WAITS, SPEC_FORM} from './lanes.mjs';
import {SIZE, SKIP_SPELLINGS, stripComments, code, reasonProblems, listProblems, bothSkipProblems, sizeProblems, sizePatternProblems, sleeps, sleepProblems, closure, strayProblems} from './budget-checks.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const specs = fs.readdirSync(here).filter((f) => f.endsWith('.spec.mjs'));
const gateSpecs = specs.filter((f) => !slowFiles.includes(f));
const text = (f) => fs.readFileSync(path.join(here, f), 'utf8');
const textOf = (names) => Object.fromEntries(names.map((f) => [f, text(f)]));
const maybeText = (f) => (fs.existsSync(path.join(here, f)) ? text(f) : undefined);
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

test('code() drops comment lines, test.skip lines and trailing comments, and stripComments() keeps the skip lines', () => {
  const src = ['await a()', '  // await expect(x).toBeVisible()', "test.skip(info.project.name !== 'wide', 'toBeHidden')", 'await b() // boundingBox here', 'await c()'].join('\n');
  expect(code(src)).toBe('await a()\nawait b()\nawait c()');
  expect(stripComments(src)).toBe(['await a()', "test.skip(info.project.name !== 'wide', 'toBeHidden')", 'await b()', 'await c()'].join('\n'));
  expect(code('await a()')).toBe('await a()');
});

test('every spec file is in exactly one list of test/lanes.mjs: both viewports, wide only, or slow', () => {
  const live = {both: bothFiles, wide: wideOnlyFiles, slow: slowFiles};
  expect(listProblems(specs, live)).toEqual([]);
  const one = ['a.spec.mjs'];
  expect(listProblems(one, {both: ['a.spec.mjs'], wide: [], slow: []})).toEqual([]);
  expect(listProblems(one, {both: ['a.spec.mjs'], wide: ['a.spec.mjs'], slow: []})).toEqual(['a.spec.mjs: in both and wide']);
  expect(listProblems(one, {both: ['a.spec.mjs'], wide: [], slow: ['a.spec.mjs']})).toEqual(['a.spec.mjs: in both and slow']);
  expect(listProblems(one, {both: [], wide: [], slow: []})).toEqual(['a.spec.mjs: in no list']);
  expect(listProblems(one, {both: [], wide: ['a.spec.mjs', 'b.mjs'], slow: []})).toEqual(['b.mjs: listed in wide, not a spec file under test/']);
  expect(listProblems(one, {both: ['a.spec.mjs'], wide: [], slow: ['gone.spec.mjs']})).toEqual(['gone.spec.mjs: listed in slow, not a spec file under test/']);
});

const REASON_MAPS = {WIDE_ONLY, BOTH, SIZE_EXCEPTIONS};
const isStringMap = (v) => v && Object.getPrototypeOf(v) === Object.prototype && Object.values(v).every((x) => typeof x === 'string');

test('every reason of WIDE_ONLY, BOTH and SIZE_EXCEPTIONS is one to five words, none empty', () => {
  expect(reasonProblems(REASON_MAPS)).toEqual([]);
  const stringMaps = Object.entries(lanes).filter(([name, v]) => name !== 'SLOW' && isStringMap(v)).map(([name]) => name).sort();
  expect(Object.keys(REASON_MAPS).sort(), 'every map of reasons in lanes.mjs but SLOW is checked').toEqual(stringMaps);
  for (const name of Object.keys(REASON_MAPS)) {
    for (const why of ['x', 'one two', 'one two three four five', '  one   two  ']) expect(reasonProblems({[name]: {'a.spec.mjs': why}}), `${name} ${why}`).toEqual([]);
    for (const why of ['one two three four five six', '', '   ', '\t', 7, null]) {
      const bad = reasonProblems({[name]: {'a.spec.mjs': why}});
      expect(bad, `${name} ${JSON.stringify(why)}`).toHaveLength(1);
      expect(bad[0]).toContain(`${name} a.spec.mjs`);
    }
    expect(reasonProblems({[name]: {'a.spec.mjs': 'fine', 'b.spec.mjs': ''}})).toHaveLength(1);
  }
});

test('the reason of a variable sleep in FIXED_WAITS is one to five words', () => {
  const variable = (entries) => Object.fromEntries(Object.entries(entries).filter(([, e]) => 'variable' in e).map(([f, e]) => [f, e.variable]));
  expect(Object.keys(variable(FIXED_WAITS)).length).toBeGreaterThan(0);
  expect(reasonProblems({FIXED_WAITS: variable(FIXED_WAITS)})).toEqual([]);
  for (const e of Object.values(FIXED_WAITS)) {
    expect(Number.isInteger(e.calls) && e.calls >= 1).toBe(true);
    expect(Number.isInteger(e.ms) && e.ms >= 0).toBe(true);
  }
  expect(reasonProblems({FIXED_WAITS: variable({'a.mjs': {calls: 1, ms: 0, variable: 'retry pause of a loop'}})})).toEqual([]);
  for (const why of ['one two three four five six', '', '  ']) expect(reasonProblems({FIXED_WAITS: variable({'a.mjs': {calls: 1, ms: 0, variable: why}})})).toHaveLength(1);
});

test('a file listed for both viewports has no skip on the project name, which would leave one of the sizes unrun', () => {
  const live = textOf(bothFiles);
  expect(Object.keys(live)).toEqual(bothFiles);
  expect(bothSkipProblems(live)).toEqual([]);
  expect(bothSkipProblems({'a.spec.mjs': "test('x', async ({page}) => { await page.goto('/'); });"})).toEqual([]);
  expect(bothSkipProblems({'a.spec.mjs': "// test.skip(info.project.name !== 'wide')\ntest('x', () => {});"})).toEqual([]);
  expect(SKIP_SPELLINGS).toHaveLength(4);
  for (const spelling of SKIP_SPELLINGS) {
    const bad = bothSkipProblems({'a.spec.mjs': `test.beforeEach(({}, info) => test.skip(info.${spelling}, 'x'));`, 'b.spec.mjs': 'test("x", () => {});'});
    expect(bad, spelling).toHaveLength(1);
    expect(bad[0]).toContain('a.spec.mjs: listed for both viewports and skips on the project name');
  }
  expect(bothSkipProblems({'a.spec.mjs': "if (info.project.name !== 'wide') return;"})).toHaveLength(1);
  expect(bothSkipProblems({'a.spec.mjs': "if (info.project.name === 'laptop') return;"})).toHaveLength(1);
});

test('the sleeps of the gate: what sleeps() counts, in which spellings, and what it leaves out', () => {
  const ms = (src) => sleeps(src).map((s) => s.ms);
  expect(ms('await page.waitForTimeout(500);')).toEqual([500]);
  expect(ms('await page.waitForTimeout (500);')).toEqual([500]);
  expect(ms("await page['waitForTimeout'](500);")).toEqual([500]);
  expect(ms("await page['waitForTimeout'] (500);")).toEqual([500]);
  expect(ms('await page.waitForTimeout( 1_500 );')).toEqual([1500]);
  expect(ms('await page.waitForTimeout(ms);')).toEqual([null]);
  expect(ms('await page.waitForTimeout(2 * 500);')).toEqual([null]);
  expect(ms('const w = page.waitForTimeout.bind(page); await w(500);')).toEqual([null]);
  expect(ms('await new Promise((r) => setTimeout(r, 5000));')).toEqual([5000]);
  expect(ms('await new Promise(r => setTimeout(r, 3000));')).toEqual([3000]);
  expect(ms('await page.evaluate(() => new Promise((r) => setTimeout(r, 3000)));')).toEqual([3000]);
  expect(ms("await new Promise((resolve) => setTimeout(() => resolve('x'), 20000));")).toEqual([20000]);
  expect(ms('const later = (ms) => new Promise((r) => setTimeout(r, ms));')).toEqual([null]);
  expect(ms('await new Promise((ok) => { setTimeout(ok, 100); });')).toEqual([100]);
  expect(ms('test.skip(false); await page.waitForTimeout(500);')).toEqual([500]);
  expect(ms('for (let i = 0; i < 20; i++) await page.waitForTimeout(2300);')).toEqual([2300]);
  expect(ms('await page.waitForTimeout(300); await new Promise((r) => setTimeout(r, 200));')).toEqual([300, 200]);
  const notSleeps = [
    'await page.waitForFunction(() => false, null, {timeout: 3000});',
    'await expect.poll(async () => 1, {intervals: [4000], timeout: 9000}).toBe(2);',
    'const timer = setTimeout(() => proc.kill(), 15000);',
    'await new Promise((resolve, reject) => { const timer = setTimeout(() => { proc.kill(); reject(new Error("no")); }, 15000); proc.on("x", resolve); });',
    'q.setTimeout(8000, () => q.destroy());',
    '// await page.waitForTimeout(500);',
    'await a(); // page.waitForTimeout(500)',
    'await new Promise((r) => requestAnimationFrame(r));',
  ];
  for (const src of notSleeps) expect(ms(src), src).toEqual([]);
});

test('a fixed wait (waitForTimeout) is made only where FIXED_WAITS names the file and the count', () => {
  const roots = gateSpecs.filter((x) => x !== 'budget.spec.mjs');
  const files = closure(roots, maybeText);
  for (const f of roots) expect(files, `${f} is read`).toContain(f);
  expect(sleepProblems(textOf(files), FIXED_WAITS)).toEqual([]);

  const two = 'await page.waitForTimeout(300);\nawait new Promise((r) => setTimeout(r, 500));';
  const entries = {'a.mjs': {calls: 2, ms: 800}};
  expect(sleepProblems({'a.mjs': two}, entries)).toEqual([]);
  expect(sleepProblems({'a.mjs': two + '\nawait page.waitForTimeout(1);'}, entries)).toEqual([expect.stringContaining('a.mjs: 3 sleeps, 2 allowed')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(300);'}, entries)).toEqual([expect.stringContaining('a.mjs: 2 sleeps allowed, 1 made')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(300);\nawait page.waitForTimeout(50000);'}, entries)).toEqual([expect.stringContaining('add up to 50300 ms, the entry says 800')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(300);\nawait page.waitForTimeout(100);'}, entries)).toEqual([expect.stringContaining('add up to 400 ms')]);
  expect(sleepProblems({}, entries)).toEqual([expect.stringContaining('a.mjs: in FIXED_WAITS, not a file of the gate lane')]);
  expect(sleepProblems({'a.mjs': 'await a();'}, {})).toEqual([]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(300);'}, {})).toEqual([expect.stringContaining('a.mjs: 1 sleeps, none allowed')]);
  expect(sleepProblems({'a.mjs': 'await a(); // page.waitForTimeout(300)\n// await page.waitForTimeout(300);'}, {})).toEqual([]);
  expect(sleepProblems({'a.mjs': 'test.skip(false); await page.waitForTimeout(300);'}, {})).toHaveLength(1);

  const loose = 'await page.waitForTimeout(ms);';
  expect(sleepProblems({'a.mjs': loose}, {'a.mjs': {calls: 1, ms: 0}})).toEqual([expect.stringContaining('whose time is no number')]);
  expect(sleepProblems({'a.mjs': loose}, {'a.mjs': {calls: 1, ms: 0, variable: ''}})).toEqual([expect.stringContaining('whose time is no number')]);
  expect(sleepProblems({'a.mjs': loose}, {'a.mjs': {calls: 1, ms: 0, variable: 'a reason'}})).toEqual([]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(5);'}, {'a.mjs': {calls: 1, ms: 5, variable: 'a reason'}})).toEqual([expect.stringContaining('every sleep has a number')]);
  expect(sleepProblems({'a.mjs': loose, 'b.mjs': 'await page.waitForTimeout(5);'}, {'a.mjs': {calls: 1, ms: 0, variable: 'a reason'}, 'b.mjs': {calls: 1, ms: 5}})).toEqual([]);
});

test('the files the sleep check reads are the gate specs and the helper modules they import, by relative imports', () => {
  const files = {
    'a.spec.mjs': "import {x} from './b.mjs';\nimport y from './c.mjs'; // import z from './d.mjs'",
    'b.mjs': "export {k} from './e.mjs';\nconst l = await import('./a.spec.mjs');",
    'c.mjs': "import fs from 'node:fs';\nimport {q} from './missing.mjs';\nimport {w} from '../outside.mjs';",
    'd.mjs': 'export const d = 1;',
    'e.mjs': 'export const k = 1;',
    'f.mjs': 'export const f = 1;',
    'g.spec.mjs': "const m = await import('./h.mjs');",
    'h.mjs': 'export const h = 1;',
  };
  expect(closure(['a.spec.mjs'], (f) => files[f])).toEqual(['a.spec.mjs', 'b.mjs', 'c.mjs', 'e.mjs']);
  expect(closure(['f.mjs'], (f) => files[f])).toEqual(['f.mjs']);
  expect(closure(['g.spec.mjs'], (f) => files[f])).toEqual(['g.spec.mjs', 'h.mjs']);
  expect(closure([], (f) => files[f])).toEqual([]);
  expect(closure(['gone.spec.mjs'], (f) => files[f])).toEqual([]);
});

test('SIZE matches every kind of line the README and lanes.mjs say a size check is', () => {
  const NAMED = {
    'a viewport': ['viewport: {width: 390}', 'page.viewportSize()', 'page.setViewportSize(x)'],
    'a position': ['el.boundingBox()', 'el.getBoundingClientRect()', 'document.documentElement.scrollWidth', 'window.innerWidth'],
    'a screenshot': ['expect(page).toHaveScreenshot()', 'page.screenshot({})'],
    'a visibility matcher': ['el.isVisible()', 'expect(el).toBeVisible()', 'expect(el).toBeHidden()', 'el.isHidden()', 'expect(el).toBeInViewport()', 'el.scrollIntoViewIfNeeded()', "window.matchMedia('(max-width: 800px)')"],
  };
  const NONE = ['await page.locator("#x").click()', "await page.goto('/')", 'expect(await r.json()).toEqual({})'];
  expect(sizePatternProblems(SIZE, NAMED, NONE)).toEqual([]);
  expect(sizePatternProblems(/viewport\s*:/, NAMED, NONE).length).toBeGreaterThan(5);
  expect(sizePatternProblems(/toBeVisible/, {a: ['expect(el).toBeHidden()']}, [])).toEqual(['a: expect(el).toBeHidden() is not matched']);
  expect(sizePatternProblems(/click/, {}, NONE)).toEqual([`${NONE[0]} is matched and names no size`]);
  expect(sizePatternProblems(/./, NAMED, NONE)).toHaveLength(NONE.length);
});

test('a spec that names a size or a position is listed wide only only with a reason in SIZE_EXCEPTIONS', () => {
  const checked = wideOnlyFiles.filter((x) => x !== 'budget.spec.mjs');
  const live = textOf(checked);
  expect(Object.keys(live)).toEqual(checked);
  expect(sizeProblems(live, wideOnlyFiles, SIZE_EXCEPTIONS)).toEqual([]);

  const matches = 'await expect(page.locator("#x")).toBeVisible();';
  const quiet = 'await page.locator("#x").click();';
  expect(sizeProblems({'a.mjs': matches, 'b.mjs': quiet}, ['a.mjs', 'b.mjs'], {'a.mjs': 'presence'})).toEqual([]);
  expect(sizeProblems({'b.mjs': quiet}, ['b.mjs'], {})).toEqual([]);
  expect(sizeProblems({'a.mjs': matches}, ['a.mjs'], {})).toEqual([expect.stringContaining('a.mjs: names a size, a position or a visibility matcher and runs in wide only')]);
  expect(sizeProblems({'a.mjs': matches}, ['a.mjs'], {'a.mjs': 'presence', 'b.mjs': 'presence'})).toEqual([expect.stringContaining('b.mjs: an exception for a file that is not wide only')]);
  expect(sizeProblems({'a.mjs': matches}, ['a.mjs', 'b.mjs'], {'a.mjs': 'presence', 'b.mjs': 'presence'})).toEqual([expect.stringContaining('b.mjs: an exception for a file the check does not read')]);
  expect(sizeProblems({'b.mjs': quiet}, ['b.mjs'], {'b.mjs': 'presence'})).toEqual([expect.stringContaining('b.mjs: an exception for a file that names no size, position or visibility matcher')]);
  expect(sizeProblems({'b.mjs': `// ${matches}\ntest.skip(!(await x.isVisible()), 'y');`}, ['b.mjs'], {'b.mjs': 'presence'})).toEqual([expect.stringContaining('b.mjs: an exception for a file that names no size')]);
  expect(sizeProblems({'b.mjs': `// ${matches}\n${quiet} // ${matches}`}, ['b.mjs'], {})).toEqual([]);
});

test('the laptop project leaves out the wide-only and slow files, the wide project runs every gate file', () => {
  const ignored = (name) => [gate.testIgnore ?? [], gate.projects.find((p) => p.name === name)?.testIgnore ?? []].flat().map((g) => g.replace(/^\*\*\//, ''));
  const names = (name) => [...new Set(ignored(name))].sort();
  expect(names('laptop')).toEqual([...new Set([...wideOnlyFiles, ...slowFiles])].sort());
  expect(names('wide')).toEqual([...slowFiles].sort());
});

test('the config matches test files by the one form the lists know, and no file under test/ is left out by it', () => {
  expect(gate.testMatch).toBe(SPEC_FORM);
  const formMatches = (rel) => SPEC_FORM.test(path.join('/ui/test', rel));
  expect(formMatches('x.spec.mjs')).toBe(true);
  for (const rel of ['x.test.mjs', 'sub/x.spec.mjs', 'x.spec.js', 'x.spec.ts', 'x.spec.mts', 'x.mjs', 'x.spec.mjs.bak']) expect(formMatches(rel), rel).toBe(false);
  expect(strayProblems(['x.spec.mjs', 'x.mjs', 'data.json', 'fixtures/x.txt'], formMatches)).toEqual([]);
  const stray = ['x.test.mjs', 'sub/x.spec.mjs', 'x.spec.js', 'x.spec.ts', 'x.test.cjs', 'x.spec.mts', 'x.spec.tsx', 'sub/x.test.js'];
  expect(strayProblems([...stray, 'x.spec.mjs'], formMatches)).toHaveLength(stray.length);
  expect(strayProblems(['x.spec.mjs.bak', 'x.mjs'], formMatches)).toEqual([]);
  expect(strayProblems(fs.readdirSync(here, {recursive: true}), formMatches)).toEqual([]);
});

test('no file under test/ names a listening port: servers take one from the kernel (test/serve.mjs)', () => {
  const bad = [];
  for (const f of fs.readdirSync(here).filter((x) => x.endsWith('.mjs') && x !== 'budget.spec.mjs')) {
    const t = text(f);
    for (const m of t.matchAll(/--port['"],\s*(?:String\()?['"]?(\d+)|SDC_UI_[A-Z]+_PORT|\bport\s*(?:=|\|\|)\s*\d{4}|127\.0\.0\.1:\d{4}/g)) if (m[1] !== '0') bad.push(`${f}: ${m[0]}`);
  }
  expect(bad).toEqual([]);
});
