// The test step's budget is held by the runner (playwright.config.mjs: 60 s a test, 120 s the run, the
// ten slowest printed), and this file holds the config and the specs to it: a spec that sets a longer
// test timeout would take its test out of the runner's reach, and a fixed port would make a test talk
// to whatever else listens there. Pure file checks, no browser.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import gate, {BUDGET} from '../playwright.config.mjs';
import {slowFiles} from './lanes.mjs';

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

test('no file under test/ names a listening port: servers take one from the kernel (test/serve.mjs)', () => {
  const bad = [];
  for (const f of fs.readdirSync(here).filter((x) => x.endsWith('.mjs') && x !== 'budget.spec.mjs')) {
    const t = text(f);
    for (const m of t.matchAll(/--port['"],\s*(?:String\()?['"]?(\d+)|SDC_UI_[A-Z]+_PORT|\bport\s*(?:=|\|\|)\s*\d{4}|127\.0\.0\.1:\d{4}/g)) if (m[1] !== '0') bad.push(`${f}: ${m[0]}`);
  }
  expect(bad).toEqual([]);
});
