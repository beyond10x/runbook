// Independent review, invariant 1 of story inspector-shows-step-products, "a path that changes between check and
// open (time of check to time of use)". readRegular (runfiles.mjs) takes the real path of a file, checks that it lies
// under the run, reads its type with a stat, and then opens THAT PATH STRING with O_NOFOLLOW. O_NOFOLLOW refuses a
// link in the last component only; a directory on the way that is swapped for a link to another directory between
// the check and the open is followed. A run is written while the inspector reads it (a program the run bound), so
// the check is a conformance question: after the open, does the opened file still lie under the run?
// The swap here is `mv --exchange` of a directory of the run and a link to a directory outside the run, in a loop,
// by a child process; the reader is fileAnswer (products.mjs), the function behind GET /api/file, called in-process.
// Time-bounded: 10 s, or the first answer that holds the outside text. Invented text only.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileAnswer} from '../products.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-race');
const OUTSIDE = 'OUTSIDE-TEXT-RACE-41ac';
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));

test('inv 1: a directory of the run swapped for a link that leaves the run between the check and the open does not make the file read', async () => {
  test.setTimeout(30000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  put(path.join(run, 'work', 'd', 'f.txt'), 'INSIDE-TEXT\n');
  put(path.join(root, 'out', 'f.txt'), `${OUTSIDE}\n`);
  fs.symlinkSync(path.join(root, 'out'), path.join(run, 'work', 'lnk'));
  const swapper = spawn('sh', ['-c', 'while :; do mv --exchange -T d lnk; done'], {cwd: path.join(run, 'work'), detached: true, stdio: 'ignore'});
  const deadline = Date.now() + 10000;
  let tries = 0, hit = null;
  const by = {};
  try {
    while (Date.now() < deadline && !hit) {
      for (let i = 0; i < 200 && !hit; i++) {
        tries++;
        const r = fileAnswer(run, 'work/d/f.txt');
        by[r.status] = (by[r.status] || 0) + 1;
        if (r.status === 200 && r.body.text?.includes(OUTSIDE)) hit = r;
      }
      await new Promise((ok) => setImmediate(ok));
    }
  } finally { try { process.kill(-swapper.pid); } catch { /* gone */ } }
  console.log(`race: ${tries} reads of work/d/f.txt by status ${JSON.stringify(by)}, outside text read: ${hit != null}`);
  expect(hit, `the text of a file outside the run was served after ${tries} reads`).toBeNull();
});
