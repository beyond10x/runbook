// Confirmation pass of the independent review, finding 1 of story inspector-shows-step-products (time of check to time
// of use), with an input the first case did not use. fileAnswer (products.mjs) answers a file only when its real path
// lies under a work/ directory of the run: it checks that on the real path it computed (runRel / runPath), and then
// calls readRegular, which computes the real path AGAIN and opens that one; the descriptor is compared with the
// second computation only (openChecked). A directory of work/ swapped for a link to another directory of the run
// between the first and the second computation is therefore read, although the file is not under work/.
// The swap is an exchange of work/d and a link to store/x (a directory of the run, not under work/); the reader is
// fileAnswer, in-process. Invented text only.
// What the case proves, and how: the swap is made right before each call the reader makes to the file system, from both
// states, and before every set of up to three such calls (interleave.mjs), so every window between two calls of the
// reader is tried, the one between the first and the second computation of the real path among them; no answer serves
// the store text. Where a loop of `mv --exchange` in a child process hit those windows by chance for 10 s, each is hit
// here on purpose in every run. A short race with that child process follows, bounded by what it saw (each state served
// or refused 100 times), not by a time.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileAnswer} from '../products.mjs';
import {places, before, sets} from './interleave.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-2-race');
const STORE_TEXT = 'STORE-TEXT-OF-THE-RUN-RACE-77be';
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));

test('finding 1 again: a directory of work/ swapped for a link to a directory of the run outside work/ between the check and the read does not serve that file', async () => {
  test.setTimeout(30000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  const work = path.join(run, 'work');
  put(path.join(work, 'd', 'f.txt'), 'WORK-TEXT\n');
  put(path.join(run, 'store', 'x', 'f.txt'), `${STORE_TEXT}\n`);
  fs.symlinkSync(path.join(run, 'store', 'x'), path.join(work, 'lnk'));
  const swap = () => { fs.renameSync(path.join(work, 'd'), path.join(work, 'tmp')); fs.renameSync(path.join(work, 'lnk'), path.join(work, 'd')); fs.renameSync(path.join(work, 'tmp'), path.join(work, 'lnk')); };
  const read = () => fileAnswer(run, 'work/d/f.txt');
  const by = {};
  let tried = 0, between = 0;
  for (const start of ['directory', 'link']) {
    if (start === 'link') swap();
    const plain = places(read);
    const real = plain.names.flatMap((name, i) => (name === 'realpathSync.native' ? [i + 1] : []));
    expect(plain.names, 'the reader computes real paths and then opens').toEqual(expect.arrayContaining(['realpathSync.native', 'openSync']));
    expect(plain.value.status, `from the ${start} in place, untouched`).toBe(start === 'directory' ? 200 : 404);
    const n = plain.names.length + 2;
    for (const set of sets(n, 3)) {
      let swaps = 0;
      const r = before(set, () => { swap(); swaps++; }, read);
      // an odd number of swaps leaves the tree swapped: bring it back to the start of this state
      if (swaps % 2) swap();
      tried++;
      by[r.status] = (by[r.status] || 0) + 1;
      expect(r.body.text ?? '', `swaps before calls ${[...set].join(',')} of ${n} (${start} in place first)`).not.toContain(STORE_TEXT);
      if (start === 'directory' && set.size === 1 && [...set][0] > real[0] && [...set][0] <= real.at(-1)) between++;
    }
    if (start === 'link') swap();
  }
  expect(between, 'swaps between the first and the last computation of a real path were tried').toBeGreaterThan(0);
  const swapper = spawn('sh', ['-c', 'while :; do mv --exchange -T d lnk; done'], {cwd: work, detached: true, stdio: 'ignore'});
  const race = {};
  let reads = 0, hit = null;
  try {
    while (!hit && ((race[200] || 0) < 100 || (race[403] || 0) + (race[404] || 0) < 100)) {
      for (let i = 0; i < 200 && !hit; i++) {
        reads++;
        const r = read();
        race[r.status] = (race[r.status] || 0) + 1;
        if (r.status === 200 && r.body.text?.includes(STORE_TEXT)) hit = r;
      }
      await new Promise((ok) => setImmediate(ok));
    }
  } finally { try { process.kill(-swapper.pid); } catch { /* gone */ } }
  console.log(`race 2: ${tried} interleavings by status ${JSON.stringify(by)}; ${reads} reads beside a swapping process by status ${JSON.stringify(race)}, text of store/ served: ${hit != null}`);
  expect(hit, `the text of store/x/f.txt was served as a file of work/ after ${reads} reads beside a swapping process`).toBeNull();
});
