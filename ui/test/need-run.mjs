// runs/ is git-ignored: a fresh clone has no recorded run. A test that reads one calls needRun first
// and fails with the run directory it is missing, not with an ENOENT deep inside a fixture or a page
// that never fills.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const sdc = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export function missingRun(dir) {
  if (fs.existsSync(dir)) return null;
  const shown = path.relative(sdc, dir).startsWith('..') ? dir : path.relative(sdc, dir);
  return `missing run directory ${shown}: runs/ is git-ignored, so a fresh clone does not have it; copy the recorded run there or point the test at one (SDC_UI_FIXTURE_SOURCE for test/fixture-run.mjs)`;
}

export function needRun(dir, who) {
  const m = missingRun(dir);
  if (m) throw new Error(`${who}: ${m}`);
}
