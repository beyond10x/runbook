// runs/ is git-ignored: a fresh clone has no recorded run. A test that reads one calls needRun first,
// with its `test`: on a checkout without the run it is skipped, and the reason names the run it is
// missing (not an ENOENT deep inside a fixture or a page that never fills). Without `test` (a
// script) it throws. needTool does the same for a binary that is built, not committed.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {canonPath, drivePath} from '../binaries.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const sdc = path.resolve(here, '../..');
const shown = (p) => (path.relative(sdc, p).startsWith('..') ? p : path.relative(sdc, p));

export function missingRun(dir) {
  if (fs.existsSync(dir)) return null;
  return `missing run directory ${shown(dir)}: runs/ is git-ignored, so a fresh clone does not have it; copy the recorded run there or point the test at one (SDC_UI_FIXTURE_SOURCE for test/fixture-run.mjs)`;
}

export function needRun(dir, who, test) {
  const m = missingRun(dir);
  if (!m) return;
  if (test) test.skip(true, `${who}: ${m}`);
  else throw new Error(`${who}: ${m}`);
}

export function missingTool(file, how) {
  return fs.existsSync(file) ? null : `missing ${shown(file)} (built, not committed): ${how}`;
}

export function needTool(file, how, who, test) {
  const m = missingTool(file, how);
  if (m) test.skip(true, `${who}: ${m}`);
}

// The recorded run test/fixture-run.mjs plays from: SDC_UI_FIXTURE_SOURCE, else (the scenarios that need
// only the cases intake and direction) the committed slim copy test/recorded/trial-todo-1-slim (made by
// test/slim-source.mjs), else runs/trial-todo-1. The gate's lane always takes the slim copy for its
// scenarios, so the step does the same with or without runs/; the slow lane (SDC_UI_LANE=slow, set by
// playwright.slow.config.mjs) takes runs/trial-todo-1 when it is there.
export const SLIM_SCENARIOS = ['person', 'stopped', 'human', 'decider', 'autopilot'];
export const slowLane = () => process.env.SDC_UI_LANE === 'slow';
export function fixtureSource(scenario) {
  if (process.env.SDC_UI_FIXTURE_SOURCE) return path.resolve(process.env.SDC_UI_FIXTURE_SOURCE);
  const full = path.join(sdc, 'runs/trial-todo-1');
  const slim = path.join(here, 'recorded/trial-todo-1-slim');
  if (!SLIM_SCENARIOS.includes(scenario)) return full;
  return slowLane() && fs.existsSync(full) ? full : slim;
}

// The driver and canon binaries test/scope-fixture.mjs runs.
export const DRIVE = drivePath(sdc);
export const CANON = canonPath();
export function needDriver(who, test) {
  needTool(DRIVE, 'cargo build in driver/', who, test);
  needTool(CANON, 'build canon (cargo build in the canon repository)', who, test);
}
