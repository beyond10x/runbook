// runs/ is git-ignored: a fresh clone has no recorded run. A test that reads one calls needRun first,
// with its `test`: on a checkout without the run it is skipped, and the reason names the run it is
// missing (not an ENOENT deep inside a fixture or a page that never fills). Without `test` (a
// script) it throws. needTool does the same for a binary that is built, not committed.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

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

// The recorded run test/fixture-run.mjs plays from: SDC_UI_FIXTURE_SOURCE, else runs/trial-todo-1,
// else (for the scenarios that need only the cases intake and direction) its committed slim copy
// test/recorded/trial-todo-1-slim (made by test/slim-source.mjs).
export const SLIM_SCENARIOS = ['person', 'stopped', 'human', 'decider', 'autopilot'];
export function fixtureSource(scenario) {
  if (process.env.SDC_UI_FIXTURE_SOURCE) return path.resolve(process.env.SDC_UI_FIXTURE_SOURCE);
  const full = path.join(sdc, 'runs/trial-todo-1');
  return !fs.existsSync(full) && SLIM_SCENARIOS.includes(scenario) ? path.join(here, 'recorded/trial-todo-1-slim') : full;
}

// The driver and canon binaries test/scope-fixture.mjs runs.
export const DRIVE = path.join(sdc, 'driver/target/debug/sdc-drive');
export const CANON = path.join(process.env.HOME || '/nonexistent', 'beyond10x/canon/target/debug/canon');
export function needDriver(who, test) {
  needTool(DRIVE, 'cargo build in driver/', who, test);
  needTool(CANON, 'build canon (cargo build in the canon repository)', who, test);
}
