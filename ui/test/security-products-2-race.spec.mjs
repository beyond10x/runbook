// Confirmation pass of the independent review, finding 1 of story inspector-shows-step-products (time of check to time
// of use), with an input the first case did not use. fileAnswer (products.mjs) answers a file only when its real path
// lies under a work/ directory of the run: it checks that on the real path it computed (runRel / runPath), and then
// calls readRegular, which computes the real path AGAIN and opens that one; the descriptor is compared with the
// second computation only (openChecked). A directory of work/ swapped for a link to another directory of the run
// between the first and the second computation is therefore read, although the file is not under work/.
// The swap is `mv --exchange` of work/d and a link to store/x (a directory of the run, not under work/), in a loop,
// by a child process; the reader is fileAnswer, in-process. 25 s at most, or the first answer that holds the store text.
// Invented text only.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileAnswer} from '../products.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-2-race');
const STORE_TEXT = 'STORE-TEXT-OF-THE-RUN-RACE-77be';
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));

test('finding 1 again: a directory of work/ swapped for a link to a directory of the run outside work/ between the check and the read does not serve that file', async () => {
  test.setTimeout(50000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  put(path.join(run, 'work', 'd', 'f.txt'), 'WORK-TEXT\n');
  put(path.join(run, 'store', 'x', 'f.txt'), `${STORE_TEXT}\n`);
  fs.symlinkSync(path.join(run, 'store', 'x'), path.join(run, 'work', 'lnk'));
  const swapper = spawn('sh', ['-c', 'while :; do mv --exchange -T d lnk; done'], {cwd: path.join(run, 'work'), detached: true, stdio: 'ignore'});
  const deadline = Date.now() + 25000;
  let tries = 0, hit = null;
  const by = {};
  try {
    while (Date.now() < deadline && !hit) {
      for (let i = 0; i < 200 && !hit; i++) {
        tries++;
        const r = fileAnswer(run, 'work/d/f.txt');
        by[r.status] = (by[r.status] || 0) + 1;
        if (r.status === 200 && r.body.text?.includes(STORE_TEXT)) hit = r;
      }
      await new Promise((ok) => setImmediate(ok));
    }
  } finally { try { process.kill(-swapper.pid); } catch { /* gone */ } }
  console.log(`race 2: ${tries} reads of work/d/f.txt by status ${JSON.stringify(by)}, text of store/ served: ${hit != null}`);
  expect(hit, `the text of store/x/f.txt was served as a file of work/ after ${tries} reads`).toBeNull();
});
