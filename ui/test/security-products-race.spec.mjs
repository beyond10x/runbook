// Independent review, invariant 1 of story inspector-shows-step-products, "a path that changes between check and
// open (time of check to time of use)". readRegular (runfiles.mjs) takes the real path of a file, checks that it lies
// under the run, reads its type with a stat, and then opens THAT PATH STRING with O_NOFOLLOW. O_NOFOLLOW refuses a
// link in the last component only; a directory on the way that is swapped for a link to another directory between
// the check and the open is followed. A run is written while the inspector reads it (a program the run bound), so
// the check is a conformance question: after the open, does the opened file still lie under the run?
// The swap is an exchange of a directory of the run and a link to a directory outside the run; the reader is fileAnswer
// (products.mjs), the function behind GET /api/file, called in-process. Invented text only.
// What the case proves, and how: the swap is made right before each call the reader makes to the file system, from both
// states (the directory in place, the link in place), and before every set of up to three such calls (interleave.mjs),
// so every window between two calls of the reader is tried, the one between the check and the open among them; no answer
// holds the outside text. Where a loop of `mv --exchange` in a child process hit those windows by chance for 10 s, each is
// hit here on purpose in every run. A short race with that child process follows, bounded by what it saw (each state
// served or refused 100 times), not by a time: the reader holds against a swap by another process too.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileAnswer} from '../products.mjs';
import {places, before, sets} from './interleave.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-race');
const OUTSIDE = 'OUTSIDE-TEXT-RACE-41ac';
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'one process, one viewport'));

test('inv 1: a directory of the run swapped for a link that leaves the run between the check and the open does not make the file read', async () => {
  test.setTimeout(30000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  const work = path.join(run, 'work');
  put(path.join(work, 'd', 'f.txt'), 'INSIDE-TEXT\n');
  put(path.join(root, 'out', 'f.txt'), `${OUTSIDE}\n`);
  fs.symlinkSync(path.join(root, 'out'), path.join(work, 'lnk'));
  const swap = () => { fs.renameSync(path.join(work, 'd'), path.join(work, 'tmp')); fs.renameSync(path.join(work, 'lnk'), path.join(work, 'd')); fs.renameSync(path.join(work, 'tmp'), path.join(work, 'lnk')); };
  const read = () => fileAnswer(run, 'work/d/f.txt');
  const by = {};
  let tried = 0, refusedAtOpen = null;
  for (const start of ['directory', 'link']) {
    if (start === 'link') swap();
    const plain = places(read);
    if (start === 'directory') expect(plain.names, 'the reader checks a real path and then opens').toEqual(expect.arrayContaining(['realpathSync.native', 'openSync']));
    expect(plain.value.status, `from the ${start} in place, untouched`).toBe(start === 'directory' ? 200 : 403);
    const n = plain.names.length + 2;
    for (const set of sets(n, 3)) {
      let swaps = 0;
      const r = before(set, () => { swap(); swaps++; }, read);
      // an odd number of swaps leaves the tree swapped: bring it back to the start of this state
      if (swaps % 2) swap();
      tried++;
      by[r.status] = (by[r.status] || 0) + 1;
      expect(r.body.text ?? '', `swaps before calls ${[...set].join(',')} of ${n} (${start} in place first)`).not.toContain(OUTSIDE);
      if (start === 'directory' && set.size === 1 && plain.names[[...set][0] - 1] === 'openSync') refusedAtOpen = r;
    }
    if (start === 'link') swap();
  }
  expect(refusedAtOpen, 'the swap right before the open, after the check passed, was tried').not.toBeNull();
  expect(refusedAtOpen.status, 'the file the open reached is not the file the check saw: refused').toBe(403);
  // the same against a process that swaps in a loop, until each state was seen 100 times
  const swapper = spawn('sh', ['-c', 'while :; do mv --exchange -T d lnk; done'], {cwd: work, detached: true, stdio: 'ignore'});
  const race = {};
  let reads = 0, hit = null;
  try {
    while (!hit && ((race[200] || 0) < 100 || (race[403] || 0) + (race[404] || 0) < 100)) {
      for (let i = 0; i < 200 && !hit; i++) {
        reads++;
        const r = read();
        race[r.status] = (race[r.status] || 0) + 1;
        if (r.status === 200 && r.body.text?.includes(OUTSIDE)) hit = r;
      }
      await new Promise((ok) => setImmediate(ok));
    }
  } finally { try { process.kill(-swapper.pid); } catch { /* gone */ } }
  console.log(`race: ${tried} interleavings by status ${JSON.stringify(by)}; ${reads} reads beside a swapping process by status ${JSON.stringify(race)}, outside text read: ${hit != null}`);
  expect(hit, `the text of a file outside the run was served after ${reads} reads beside a swapping process`).toBeNull();
});
