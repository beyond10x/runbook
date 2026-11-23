// The test step's budget is held by the runner (playwright.config.mjs: 60 s a test, 120 s the run, the
// ten slowest printed), and this file holds the config and the specs to it: a spec that sets a longer
// test timeout would take its test out of the runner's reach, and a fixed port would make a test talk
// to whatever else listens there. Pure file checks, no browser.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import gate, {BUDGET} from '../playwright.config.mjs';
import {slowFiles, bothFiles, wideOnlyFiles, WIDE_ONLY, BOTH, SIZE_EXCEPTIONS} from './lanes.mjs';

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
const SIZE = /setViewportSize|viewport\s*:|viewportSize|boundingBox|getBoundingClientRect|scrollWidth|innerWidth|toHaveScreenshot|\.screenshot\(|\.isVisible\(/;

test('every spec file is in exactly one list of test/lanes.mjs: both viewports, wide only, or slow', () => {
  const lists = {both: bothFiles, wide: wideOnlyFiles, slow: slowFiles};
  const bad = [];
  for (const f of specs) {
    const in_ = Object.entries(lists).filter(([, l]) => l.includes(f)).map(([k]) => k);
    if (in_.length !== 1) bad.push(`${f}: in ${in_.length ? in_.join(' and ') : 'no list'}`);
  }
  for (const [k, l] of Object.entries(lists)) for (const f of l) if (!specs.includes(f)) bad.push(`${f}: listed in ${k}, not a spec file under test/`);
  expect(bad).toEqual([]);
  for (const [name, why] of [...Object.entries(WIDE_ONLY), ...Object.entries(BOTH)]) {
    expect(why.trim().split(/\s+/).length, `${name}: the reason in at most five words`).toBeLessThanOrEqual(5);
  }
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
