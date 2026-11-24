// A spec's fixture runs, played at once. A beforeAll that played its runs one after another (`spawnSync` in a loop) paid
// each player's start in turn, and a loaded machine makes a start cost seconds; `playFixtures` starts them all and waits
// for all. The player runs to its end in well under a second and is started through child.mjs without the guard, which a
// player that ends by itself does not need.
//   await playFixtures([['person', dirA], ['stopped', dirB, '1000']]);   // [scenario, run directory, speed]
import {spawnChild} from './child.mjs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function playFixture(scenario, dir, speed) {
  return new Promise((resolve, reject) => {
    const proc = spawnChild(process.execPath, ['test/fixture-run.mjs', scenario, dir, ...(speed ? [String(speed)] : [])], {cwd: ui, stdio: ['ignore', 'ignore', 'pipe'], endWithParent: false});
    let err = '';
    proc.stderr.on('data', (d) => { err += d; });
    proc.on('error', reject);
    proc.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`fixture ${scenario} into ${dir} exited with ${code}: ${err}`))));
  });
}

export const playFixtures = (jobs) => Promise.all(jobs.map(([scenario, dir, speed]) => playFixture(scenario, dir, speed)));
