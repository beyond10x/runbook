// The test step's budget is held by the runner (playwright.config.mjs: 60 s a test, 120 s the run, the ten
// slowest printed), and this file holds the config and the specs to it: a spec that sets a longer
// test timeout would take its test out of the runner's reach, and a fixed port would make a test talk
// to whatever else listens there. Pure file checks, no browser. Each check is a function of its inputs
// (budget-checks.mjs), and its case calls it on the live lists and on a good and a bad input of its own, so
// that a check that is broken fails a case.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import gate, {BUDGET} from '../playwright.config.mjs';
import * as lanes from './lanes.mjs';
import {slowFiles, bothFiles, wideOnlyFiles, WIDE_ONLY, BOTH, SIZE_EXCEPTIONS, FIXED_WAITS, SPEC_FORM} from './lanes.mjs';
import {SIZE, SKIP_SPELLINGS, stripComments, code, reasonProblems, listProblems, bothSkipProblems, sizeProblems, sizePatternProblems, sleeps, sleepProblems, closure, strayProblems} from './budget-checks.mjs';
import {spawnChild} from './child.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const specs = fs.readdirSync(here).filter((f) => f.endsWith('.spec.mjs'));
const gateSpecs = specs.filter((f) => !slowFiles.includes(f));
const text = (f) => fs.readFileSync(path.join(here, f), 'utf8');
const textOf = (names) => Object.fromEntries(names.map((f) => [f, text(f)]));
const maybeText = (f) => (fs.statSync(path.join(here, f), {throwIfNoEntry: false})?.isFile() ? text(f) : undefined);
// The files of test/ and its subdirectories, but not the two directories of data: fixtures/ (git-ignored scratch of this file; the
// other specs write their run directories under target/fixture-runs, fixture-dir.mjs) and recorded/ (committed data).
const DATA_DIRS = ['fixtures', 'recorded'];
const testFiles = (dir = here, rel = '') => fs.readdirSync(dir, {withFileTypes: true}).flatMap((e) => (e.isDirectory() ? (rel === '' && DATA_DIRS.includes(e.name) ? [] : testFiles(path.join(dir, e.name), `${rel}${e.name}/`)) : [rel + e.name]));
// The files a spec of the gate imports, in test/ and in ux/, by their own walk of the imports.
const imported = () => {
  const ui = path.resolve(here, '..');
  const seen = new Map();
  const todo = gateSpecs.filter((f) => f !== 'budget.spec.mjs').map((f) => path.join(here, f));
  while (todo.length) {
    const f = todo.pop();
    if (seen.has(f) || !fs.existsSync(f)) continue;
    const src = fs.readFileSync(f, 'utf8');
    seen.set(f, src);
    for (const m of stripComments(src).matchAll(/(?:from\s+|import\s*\(\s*)['"](\.{1,2}\/[^'"]+\.mjs)['"]/g)) {
      const next = path.resolve(path.dirname(f), m[1]);
      if (next.startsWith(here + path.sep) || next.startsWith(path.join(ui, 'ux') + path.sep)) todo.push(next);
    }
  }
  return seen;
};
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

test('the reason of a window and a why in FIXED_WAITS is one to five words, and a reason that is no text or has tabs is told, not thrown', () => {
  const reasons = (entries, key) => Object.fromEntries(Object.entries(entries).filter(([, e]) => key in e).map(([f, e]) => [f, e[key]]));
  for (const key of ['window', 'why']) {
    expect(Object.keys(reasons(FIXED_WAITS, key)).length, key).toBeGreaterThan(0);
    expect(reasonProblems({FIXED_WAITS: reasons(FIXED_WAITS, key)}), key).toEqual([]);
    expect(reasonProblems({FIXED_WAITS: reasons({'a.mjs': {calls: 1, ms: 0, [key]: 'retry pause of a loop'}}, key)})).toEqual([]);
    for (const why of ['one two three four five six', '', '  ']) expect(reasonProblems({FIXED_WAITS: reasons({'a.mjs': {calls: 1, ms: 0, [key]: why}}, key)})).toHaveLength(1);
  }
  for (const why of [7, null, undefined, {}, ['a']]) expect(() => reasonProblems({L: {'a.mjs': why}}), String(why)).not.toThrow();
  expect(reasonProblems({L: {'a.mjs': 7}})).toEqual(['L a.mjs: a reason that is no text']);
  expect(reasonProblems({L: {'a.spec.mjs': 'a\tb\tc\td\te\tf'}})).toHaveLength(1);
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
  const among = ["import {test} from '@playwright/test';", "test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'x'));", "test('x', () => {});"].join('\n');
  expect(bothSkipProblems({'a.spec.mjs': among})).toHaveLength(1);
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
  expect(ms('await page.waitForTimeout(0);')).toEqual([0]);
  expect(ms('await new Promise(async (r) => { setTimeout(r, 5000); });')).toEqual([5000]);
  expect(ms('await new Promise((r) => setTimeout(() => { go({a: 1, b: 2}); r(); }, 500));')).toEqual([500]);
  expect(sleeps('await new Promise((r) => { new Promise((r2) => setTimeout(() => { r(); r2(); }, 5)); });')).toHaveLength(1);
  expect(sleeps('const end = Date.now() + 8_000;\nwhile (Date.now() < end) {}')).toEqual([{ms: 8000, window: true}]);
  expect(sleeps('for (const end = Date.now()+1500; Date.now() < end;) { await page.waitForTimeout(25); }')).toEqual([{ms: 25}, {ms: 1500, window: true}]);
  const notSleeps = [
    "await new Promise((resolve, reject) => { setTimeout(() => reject(new Error('could not resolve it')), 5); });",
    'await new Promise(function (r) { setTimeout(r, 5000); });',
    "import {setTimeout as nap} from 'node:timers/promises';\nawait nap(5000);",
    'Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5000);',
    'const st = setTimeout; await new Promise((r) => st(r, 5000));',
    'await new Promise((r) => setInterval(() => r(), 5000));',
    'const end = Date.now() + ms;',
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
  expect(files.filter((f) => f.endsWith('.spec.mjs')), 'the specs the closure reads').toEqual(gateSpecs.filter((x) => x !== 'budget.spec.mjs').sort());
  for (const f of ['../ux/core/phone.mjs', '../ux/core/contract.mjs']) expect(files, `${f} is read`).toContain(f);
  expect(sleepProblems(textOf(files), FIXED_WAITS)).toEqual([]);
  // the files by an own walk of the imports in test/ and ux/ say the same: each that sleeps or has a window is in FIXED_WAITS
  const unlisted = [];
  for (const [f, src] of imported()) {
    const rel = path.relative(here, f);
    if ((sleeps(src).length || /Date\.now\(\)\s*\+\s*\d/.test(stripComments(src))) && !(rel in FIXED_WAITS)) unlisted.push(rel);
  }
  expect(unlisted).toEqual([]);

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
  expect(sleepProblems({'a.mjs': loose}, {'a.mjs': {calls: 1, ms: 0, variable: 'a reason'}})).toEqual([expect.stringContaining('needs "helper"')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(5);'}, {'a.mjs': {calls: 1, ms: 5, variable: 'a reason'}})).toEqual([expect.stringContaining('every sleep has a number')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(0);'}, {})).toHaveLength(1);
  const withLoose = {'a.mjs': {calls: 1, ms: 0, variable: 'a reason'}};
  expect(sleepProblems({'a.mjs': loose, 'b.mjs': 'await page.waitForTimeout(5);'}, {...withLoose, 'b.mjs': {calls: 1, ms: 5}})).toEqual([expect.stringContaining('needs "helper"')]);

  const later = 'const later = (ms) => new Promise((r) => setTimeout(r, ms));\n';
  const helper = {'a.mjs': {calls: 1, ms: 0, variable: 'retry pause', helper: 'later', largest: 250}};
  const calls = (args) => later + args.map((a) => `await later(${a});`).join('\n');
  expect(sleepProblems({'a.mjs': calls(['250', '100', '1_0'])}, helper)).toEqual([]);
  expect(sleepProblems({'a.mjs': calls(['250']) + '\nawait obj.later(60000); await mylater(60000); // await later(60000)'}, helper)).toEqual([]);
  expect(sleepProblems({'a.mjs': calls(['60000'])}, helper)).toEqual([expect.stringContaining('later(60000) is over the largest the entry names, 250')]);
  expect(sleepProblems({'a.mjs': calls(['250', '251'])}, helper)).toEqual([expect.stringContaining('later(251) is over')]);
  expect(sleepProblems({'a.mjs': calls(['250', 'ms * 2'])}, helper)).toEqual([expect.stringContaining('later(ms * 2) passes no number')]);
  expect(sleepProblems({'a.mjs': calls(['250', 'f(1)'])}, helper)).toEqual([expect.stringContaining('later(f(1)) passes no number')]);
  expect(sleepProblems({'a.mjs': calls(['100'])}, helper)).toEqual([expect.stringContaining('pass at most 100, the entry says 250')]);
  expect(sleepProblems({'a.mjs': calls([])}, helper)).toEqual([expect.stringContaining('the helper later has no call site')]);
  expect(sleepProblems({'a.mjs': calls(['250'])}, {'a.mjs': {...helper['a.mjs'], helper: undefined}})).toEqual([expect.stringContaining('needs "helper"')]);
  expect(sleepProblems({'a.mjs': calls(['250'])}, {'a.mjs': {...helper['a.mjs'], largest: '250'}})).toEqual([expect.stringContaining('needs "helper"')]);
  expect(sleepProblems({'a.mjs': calls(['250']).replace('const later = ', 'function later')}, helper)).toEqual([]);

  const windowed = 'const end = Date.now() + 8000;\nwhile (Date.now() < end) {}';
  expect(sleepProblems({'a.mjs': windowed}, {'a.mjs': {calls: 1, ms: 8000, window: 'a reason'}})).toEqual([]);
  expect(sleepProblems({'a.mjs': windowed}, {})).toEqual([expect.stringContaining('a.mjs: 1 sleeps, none allowed')]);
  expect(sleepProblems({'a.mjs': windowed}, {'a.mjs': {calls: 1, ms: 8000}})).toEqual([expect.stringContaining('needs "window"')]);
  expect(sleepProblems({'a.mjs': windowed.replace('8000', '80000')}, {'a.mjs': {calls: 1, ms: 8000, window: 'a reason'}})).toEqual([expect.stringContaining('add up to 80000 ms, the entry says 8000')]);
  expect(sleepProblems({'a.mjs': 'await a();'}, {'a.mjs': {calls: 1, ms: 0, window: 'a reason'}})).toEqual([expect.stringContaining('1 sleeps allowed, 0 made'), expect.stringContaining('drop it')]);
  expect(sleepProblems({'a.mjs': 'await page.waitForTimeout(5);'}, {'a.mjs': {calls: 1, ms: 5, window: 'a reason'}})).toEqual([expect.stringContaining('the entry says "window", no Date.now() + number window is made')]);
});

test('a call site of a helper that the live FIXED_WAITS names as variable, raised, added or given no number, is told', () => {
  const variable = Object.entries(FIXED_WAITS).filter(([, e]) => e.variable);
  expect(variable.length).toBeGreaterThan(0);
  for (const [f, e] of variable) {
    const t = text(f);
    expect(t, f).toContain(`${e.helper}(${e.largest})`);
    expect(sleepProblems({[f]: t}, {[f]: e}), f).toEqual([]);
    expect(sleepProblems({[f]: t.split(`${e.helper}(${e.largest})`).join(`${e.helper}(60000)`)}, {[f]: e}), `${f} with ${e.helper}(60000)`).not.toEqual([]);
    expect(sleepProblems({[f]: `${t}\nawait ${e.helper}(60000);\n`}, {[f]: e}), `${f} with one more ${e.helper}(60000)`).not.toEqual([]);
    expect(sleepProblems({[f]: `${t}\nawait ${e.helper}(ms);\n`}, {[f]: e}), `${f} with ${e.helper}(ms)`).not.toEqual([]);
  }
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

  const tree = {
    'a.spec.mjs': "import {p} from '../ux/core/p.mjs';\nimport {s} from './sub/s.mjs';",
    '../ux/core/p.mjs': "import {q} from './q.mjs';\nimport {w} from '../w.mjs';",
    '../ux/core/q.mjs': 'export const q = 1;',
    '../ux/w.mjs': 'export const w = 1;',
    'sub/s.mjs': "import {t} from '../t.mjs';",
    't.mjs': 'export const t = 1;',
    'b.mjs': 'export const b = 1;',
    '../b.mjs': 'export const outer = 1;',
  };
  expect(closure(['a.spec.mjs'], (f) => tree[f])).toEqual(['../ux/core/p.mjs', '../ux/core/q.mjs', '../ux/w.mjs', 'a.spec.mjs', 'sub/s.mjs', 't.mjs']);
  expect(closure(['x.spec.mjs'], (f) => ({'x.spec.mjs': "import {b} from '../b.mjs';"})[f] ?? tree[f])).toEqual(['../b.mjs', 'x.spec.mjs']);

  expect(closure(['a.spec.mjs'], (f) => ({'a.spec.mjs': "import './b.mjs';", 'b.mjs': ''})[f]), 'an import with no bindings is not followed').toEqual(['a.spec.mjs']);

  const cycle = {'a.spec.mjs': "import {b} from './b.mjs';", 'b.mjs': "import {c} from './c.mjs';", 'c.mjs': "import {a} from './a.spec.mjs'; import {b} from './b.mjs';"};
  let reads = 0;
  const counted = (f) => { if (++reads > 20) throw new Error(`closure read ${f} the ${reads}th time: a cycle is not cut`); return cycle[f]; };
  expect(closure(['a.spec.mjs'], counted)).toEqual(['a.spec.mjs', 'b.mjs', 'c.mjs']);
  expect(reads).toBeLessThanOrEqual(6);

  const chain = (last = Infinity) => {
    let n = 0;
    return (f) => {
      if (++n > 2000) throw new Error('the closure does not stop');
      const i = Number(/\d+/.exec(f)[0]);
      return i < last ? `import {x} from './n${i + 1}.mjs';` : 'export const x = 1;';
    };
  };
  expect(() => closure(['n0.mjs'], chain())).toThrow('the closure reads more than 500 files');
  expect(() => closure(['n0.mjs'], chain(), 5)).toThrow('the closure reads more than 5 files');
  expect(closure(['n0.mjs'], chain(4), 5)).toEqual(['n0.mjs', 'n1.mjs', 'n2.mjs', 'n3.mjs', 'n4.mjs']);
});

test('SIZE matches every kind of line the README and lanes.mjs say a size check is', () => {
  const NAMED = {
    'a viewport': ['viewport: {width: 390}', 'page.viewportSize()', 'page.setViewportSize(x)'],
    'a position': ['el.boundingBox()', 'el.getBoundingClientRect()', 'document.documentElement.scrollWidth', 'window.innerWidth'],
    'a screenshot': ['expect(page).toHaveScreenshot()', 'page.screenshot({})'],
    'a visibility matcher': ['el.isVisible()', 'expect(el).toBeVisible()', 'expect(el).toBeHidden()', 'el.isHidden()', 'expect(el).toBeInViewport()', 'el.scrollIntoViewIfNeeded()', "window.matchMedia('(max-width: 800px)')"],
  };
  const NONE = ['await page.locator("#x").click()', "await page.goto('/')", 'expect(await r.json()).toEqual({})', 'const viewports = {};', 'const isVisibleName = 1;', 'await myscreenshot(x);'];
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
  const formMatches = (rel) => SPEC_FORM.test(path.join(here, rel));
  expect(formMatches('x.spec.mjs')).toBe(true);
  for (const rel of ['x.test.mjs', 'sub/x.spec.mjs', 'x.spec.js', 'x.spec.ts', 'x.spec.mts', 'x.mjs', 'x.spec.mjs.bak']) expect(formMatches(rel), rel).toBe(false);
  expect(strayProblems(['x.spec.mjs', 'x.mjs', 'data.json', 'fixtures/x.txt'], formMatches)).toEqual([]);
  const stray = ['x.test.mjs', 'sub/x.spec.mjs', 'x.spec.js', 'x.spec.ts', 'x.test.cjs', 'x.spec.mts', 'x.spec.tsx', 'sub/x.test.js'];
  expect(strayProblems([...stray, 'x.spec.mjs'], formMatches)).toHaveLength(stray.length);
  expect(strayProblems(['x.spec.mjs.bak', 'x.mjs'], formMatches)).toEqual([]);
  expect(strayProblems(testFiles(), formMatches)).toEqual([]);
});

// A spec below a directory that is itself named `test` (inside test/, test/fixtures/ or test/recorded/) is no test of the step: the
// config's pattern starts at test/. The stray check tells such a file below test/ (its walk reads neither fixtures/ nor recorded/).
for (const rel of ['sub/test/x.spec.mjs', 'fixtures/a/test/x.spec.mjs', 'recorded/a/test/x.spec.mjs']) {
  test(`the config does not take a spec below a nested directory named test: ${rel}`, () => {
    expect(SPEC_FORM.test(path.join(here, rel)), `Playwright's testMatch takes ${rel}`).toBe(false);
  });
}
test('the stray check tells a spec below a nested directory named test: sub/test/x.spec.mjs', () => {
  const rel = 'sub/test/x.spec.mjs';
  const formMatches = (r) => SPEC_FORM.test(path.join(here, r));
  expect(formMatches('x.spec.mjs'), 'the form takes a spec directly under test/').toBe(true);
  expect(strayProblems([rel], formMatches), rel).toHaveLength(1);
});

// The live calls of the two checks above stay whole: a case that replaces the assertion line itself is no case of a clean tree,
// so the text of this file is read. The patterns are regular expressions, so that this case does not contain what it looks for.
// Each call is a statement that starts its own line: a call in a comment or in a string does not hold it.
const LIVE_SLEEP_CALL = /^\s*expect\(sleepProblems\(textOf\(files\), FIXED_WAITS\)\)\.toEqual\(\[\]\);$/m;
const LIVE_STRAY_CALL = /^\s*expect\(strayProblems\(testFiles\(\), formMatches\)\)\.toEqual\(\[\]\);$/m;
test('the live sleep check and the live stray check of this file are called whole', () => {
  const src = text('budget.spec.mjs');
  expect(src).toMatch(LIVE_SLEEP_CALL);
  expect(src).toMatch(LIVE_STRAY_CALL);
});
test('the two live calls of this file are statements that start a line, not a comment or a string', () => {
  const calls = [
    ['expect(sleepProblems(textOf(files), FIXED_WAITS)).toEqual([]);', LIVE_SLEEP_CALL],
    ['expect(strayProblems(testFiles(), formMatches)).toEqual([]);', LIVE_STRAY_CALL],
  ];
  for (const [live, pattern] of calls) {
    expect(`    ${live}\n`, 'a statement of its own line').toMatch(pattern);
    expect(`    // ${live}\n`, 'commented out').not.toMatch(pattern);
    expect(`    const x = \`${live}\`;\n`, 'in a template string').not.toMatch(pattern);
  }
});

// The form of a spec file is `^<escaped test directory>/[^/]+\.spec\.mjs$`. The cases below hold its three parts: the start, the
// escape of the directory and the name.
test('the spec form takes a spec directly under test/ and not one below a chain that repeats the test directory', () => {
  expect(SPEC_FORM.test(path.join(here, 'x.spec.mjs')), 'directly under test/').toBe(true);
  expect(SPEC_FORM.test(path.join(here, here, 'x.spec.mjs')), 'the directory repeated below itself').toBe(false);
});

// The pattern is built from the directory, so a directory with a character a regular expression reads must still match itself and
// nothing else. A copy of lanes.mjs sits in a directory per character (in the git-ignored fixtures/), is imported there, and its
// SPEC_FORM is asked about a file directly in that directory, one below it and a sibling the character would read as a pattern.
for (const ch of ['.', '*', '+', '?', '^', '$', '(', ')', '[', ']', '{', '}', '|', ' ']) {
  test(`the spec form of a test directory named with ${JSON.stringify(ch)} takes its own spec and no other`, async () => {
    const base = path.join(here, 'fixtures');
    fs.mkdirSync(base, {recursive: true});
    const dir = fs.mkdtempSync(path.join(base, `form-a${ch}b-`));
    try {
      fs.copyFileSync(path.join(here, 'lanes.mjs'), path.join(dir, 'lanes.mjs'));
      const copy = await import(`${pathToFileURL(path.join(dir, 'lanes.mjs')).href}?c=${encodeURIComponent(ch)}`);
      expect(copy.SPEC_FORM.test(path.join(dir, 'x.spec.mjs')), 'its own spec').toBe(true);
      expect(copy.SPEC_FORM.test(path.join(dir, 'sub', 'x.spec.mjs')), 'one below').toBe(false);
      const sibling = path.join(base, path.basename(dir).replace(`a${ch}b`, 'aXb'), 'x.spec.mjs');
      expect(copy.SPEC_FORM.test(sibling), `a sibling the character would read as a pattern: ${sibling}`).toBe(false);
    } finally {
      fs.rmSync(dir, {recursive: true, force: true});
    }
  });
}

test('the spec form wants a name and a literal .spec.mjs: no name, or another character for the dot, is no test', () => {
  expect(SPEC_FORM.test(path.join(here, 'x.spec.mjs')), 'a name and a literal dot').toBe(true);
  for (const rel of ['.spec.mjs', 'x-spec.mjs', 'xspec.mjs', 'x_spec.mjs']) expect(SPEC_FORM.test(path.join(here, rel)), rel).toBe(false);
});

const FIXTURE_PLAYER = 'fixture-run.mjs';
test('the README counts the sleep call sites of the fixture player as the file has them', () => {
  const readme = fs.readFileSync(path.join(here, '..', 'README.md'), 'utf8');
  const said = Number((readme.match(/`test\/fixture-run\.mjs`[^.]*?holds (\d+) `sleep\(/) || [])[1]);
  const calls = (text(FIXTURE_PLAYER).match(/\bsleep\(/g) || []).length;
  expect(said, 'the README states a number').toBeGreaterThan(0);
  expect(calls, `the README says ${said} sleep( call sites`).toBe(said);
});

const LATER = 'const later = (ms) => new Promise((r) => setTimeout(r, ms));\nawait later(250);';
const laterEntry = (helper) => ({'a.mjs': {calls: 1, ms: 0, variable: 'retry pause', helper, largest: 250}});
test('the helper an entry names for a variable sleep is one word, not any text', () => {
  expect(sleepProblems({'a.mjs': LATER}, laterEntry('later'))).toEqual([]);
  for (const helper of ['two words', 'later|x', '', 'a.b']) {
    expect(sleepProblems({'a.mjs': LATER}, laterEntry(helper)), `helper "${helper}"`).toEqual([expect.stringContaining('needs "helper"')]);
  }
});
test('a call site of the helper that passes no number is told with its argument as written, spaces left out', () => {
  expect(sleepProblems({'a.mjs': `${LATER}\nawait later( ms );`}, laterEntry('later'))).toEqual([expect.stringContaining('later(ms) passes no number')]);
});

const CHURN = `
const fs = require('node:fs'); const path = require('node:path');
const [root, stop] = process.argv.slice(1);
while (!fs.existsSync(stop)) {
  fs.rmSync(root, {recursive: true, force: true});
  for (let j = 0; j < 8; j++) {
    const c = path.join(root, 'runs', 'run-' + j, 'store', 'C1');
    fs.mkdirSync(c, {recursive: true});
    fs.writeFileSync(path.join(c, 'case.json'), '{}');
  }
}`;

test('the list of test files leaves out fixtures/ and recorded/, and is read without a throw while a child churns run directories under test/fixtures/runs', async () => {
  const top = fs.readdirSync(here, {withFileTypes: true}).filter((e) => e.isFile()).map((e) => e.name).sort();
  const listed = testFiles();
  expect(listed.filter((f) => !f.includes('/')).sort(), 'the files of test/ itself').toEqual(top);
  expect(listed.filter((f) => f.startsWith('fixtures/') || f.startsWith('recorded/'))).toEqual([]);
  expect(fs.readdirSync(path.join(here, 'recorded')).length, 'recorded/ holds data the list leaves out').toBeGreaterThan(0);
  fs.mkdirSync(path.join(here, 'fixtures'), {recursive: true});
  const root = fs.mkdtempSync(path.join(here, 'fixtures', 'budget-list-'));
  try {
    fs.mkdirSync(path.join(root, 'sub'));
    fs.writeFileSync(path.join(root, 'a.spec.mjs'), '');
    fs.writeFileSync(path.join(root, 'sub', 'b.test.mjs'), '');
    fs.mkdirSync(path.join(root, 'fixtures'));
    fs.writeFileSync(path.join(root, 'fixtures', 'c.spec.mjs'), '');
    fs.mkdirSync(path.join(root, 'recorded'));
    fs.writeFileSync(path.join(root, 'recorded', 'd.spec.mjs'), '');
    expect(testFiles(root).sort()).toEqual(['a.spec.mjs', 'sub/b.test.mjs']);
    fs.mkdirSync(path.join(root, 'sub', 'fixtures'));
    fs.writeFileSync(path.join(root, 'sub', 'fixtures', 'e.spec.mjs'), '');
    expect(testFiles(root).sort()).toEqual(['a.spec.mjs', 'sub/b.test.mjs', 'sub/fixtures/e.spec.mjs']);
  } finally { fs.rmSync(root, {recursive: true, force: true}); }

  const runs = path.join(here, 'fixtures', 'runs');
  const mine = path.join(runs, 'budget-churn');
  const stop = path.join(runs, 'budget-churn.stop');
  fs.mkdirSync(runs, {recursive: true});
  fs.rmSync(stop, {force: true});
  const proc = spawnChild(process.execPath, ['-e', CHURN, mine, stop]);
  const errors = [];
  try {
    for (let walk = 0; walk < 100; walk++) {
      try { testFiles(); } catch (e) { errors.push(String(e.message).replace(here, '<test>').slice(0, 120)); }
    }
  } finally {
    fs.writeFileSync(stop, '');
    if (proc.exitCode === null && proc.signalCode === null) await new Promise((r) => proc.once('exit', r));
    fs.rmSync(mine, {recursive: true, force: true});
    fs.rmSync(stop, {force: true});
  }
  expect(errors, `${errors.length} of 100 walks threw`).toEqual([]);
});

test('the sentences about the checks name every alternative of SIZE: the README and the comment of lanes.mjs', () => {
  const norm = (t) => t.replace(/\\s\*/g, '').replace(/\\/g, '').replace(/^\./, '').replace(/\($/, '');
  const alternatives = SIZE.source.split('|').map(norm);
  expect(alternatives).toHaveLength(16);
  const readme = fs.readFileSync(path.join(here, '..', 'README.md'), 'utf8');
  const fromReadme = /`SIZE` pattern \(([^)]*(?:\([^)]*\)[^)]*)*)\) without an entry/.exec(readme);
  expect(fromReadme, 'the README sentence about the SIZE pattern').not.toBeNull();
  const fromLanes = /A wide-only file whose code names a size, a position or a visibility matcher \(([\s\S]*?)\) is listed here/.exec(text('lanes.mjs'));
  expect(fromLanes, 'the lanes.mjs comment about SIZE').not.toBeNull();
  for (const [where, m] of [['the README', fromReadme], ['lanes.mjs', fromLanes]]) {
    const named = new Set([...m[1].matchAll(/`([^`]+)`/g)].map((x) => norm(x[1])));
    expect(alternatives.filter((a) => !named.has(a)), `${where} does not name`).toEqual([]);
  }
});

test('no file under test/ names a listening port: servers take one from the kernel (test/serve.mjs)', () => {
  const bad = [];
  for (const f of fs.readdirSync(here).filter((x) => x.endsWith('.mjs') && x !== 'budget.spec.mjs')) {
    const t = text(f);
    for (const m of t.matchAll(/--port['"],\s*(?:String\()?['"]?(\d+)|SDC_UI_[A-Z]+_PORT|\bport\s*(?:=|\|\|)\s*\d{4}|127\.0\.0\.1:\d{4}/g)) if (m[1] !== '0') bad.push(`${f}: ${m[0]}`);
  }
  expect(bad).toEqual([]);
});

// A line of a file that starts an argv with 'serve' and does not spread SERVE_CAPS (child.mjs), as file:line.
const uncapped = (files) => Object.entries(files).flatMap(([f, t]) => t.split('\n').map((l, i) => [i + 1, l])
  .filter(([, l]) => /\[\s*['"]serve['"]/.test(l) && !l.includes('...SERVE_CAPS')).map(([n]) => `${f}:${n}`));
test('every runbook serve a file of test/ starts is given the caps: its argv spreads SERVE_CAPS of child.mjs', () => {
  expect(uncapped({'a.mjs': "spawnChild(bin, ['serve', '--ui', ...SERVE_CAPS], {})"})).toEqual([]);
  expect(uncapped({'a.mjs': "x\nspawnChild(bin, [ \"serve\", '--ui'], {})"})).toEqual(['a.mjs:2']);
  const files = Object.fromEntries(fs.readdirSync(here).filter((x) => x.endsWith('.mjs') && x !== 'budget.spec.mjs').map((f) => [f, text(f)]));
  expect(Object.values(files).filter((t) => /\[\s*['"]serve['"]/.test(t)).length, 'files that start a daemon').toBeGreaterThan(0);
  expect(uncapped(files), 'argv of a serve without the caps').toEqual([]);
});
