// fixture-run.mjs holds a call after its last event only when the caller passes the file as its fourth argument
// (autopilot.spec.mjs does). A variable of the shell that runs the tests changes nothing: a player started the way
// every other spec starts one (test/fixtures.mjs) ends by itself although SDC_FIXTURE_HOLD names a file nobody writes.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {spawnChild} from './child.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const dir = path.join(FIXTURE_DIR, 'fixture-hold');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));
test.afterAll(() => fs.rmSync(dir, {recursive: true, force: true}));

test('a player of the decider scenario ends by itself although SDC_FIXTURE_HOLD names a file nobody writes', async () => {
  fs.mkdirSync(dir, {recursive: true});
  const never = path.join(dir, 'never-written.hold');
  const proc = spawnChild(process.execPath, ['test/fixture-run.mjs', 'decider', path.join(dir, 'ambient'), '50'],{cwd: ui, stdio: ['ignore', 'ignore', 'pipe'], env: {...process.env, SDC_FIXTURE_HOLD: never}});
  let err = '';
  proc.stderr.on('data', (d) => { err += d; });
  try {
    const ended = new Promise((resolve) => proc.once('exit', (code) => resolve(code)));
    const outcome = await Promise.race([ended, new Promise((r) => setTimeout(() => r('still running after 20 s'), 20000))]);
    expect(outcome, `the player of the decider scenario: ${err}`).toBe(0);
  } finally {
    proc.kill();
  }
});
