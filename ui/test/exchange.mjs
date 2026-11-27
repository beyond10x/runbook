// The race of the security-products race specs: a child process exchanges two names in a loop while this process reads.
// Not a spec. The exchange is exchange.rs (renameat2 with RENAME_EXCHANGE), built with rustc once per digest of its
// source into ui/target/exchange/<digest>/ and read by every spec and worker after that; a loop of `mv --exchange`
// depended on GNU coreutils 9.5 or later, and an older mv refused the flag in every turn, so no swap happened and each
// case ran to its timeout. Here a missing rustc, a failed build, an exchange the kernel or file system refuses and an
// exchange loop that ends all fail the case at once with one line that names them.
// The race is bounded by what it saw (each state served or refused 100 times), and by a count of reads after which one
// state was still not seen enough: never by a time.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {spawnChild} from './child.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const SOURCE = path.join(here, 'exchange.rs');
const firstLine = (text) => String(text || '').trim().split('\n')[0] || '(no output)';

// The path of the built helper. A worker that finds it built reads it; workers that build at once each build in a
// directory of their own (rustc names its object files after the output's stem, so two builds in one directory
// overwrite each other's) and rename the file into place, so no worker runs a half-written one.
export function exchanger() {
  const digest = crypto.createHash('sha256').update(fs.readFileSync(SOURCE)).digest('hex').slice(0, 16);
  const bin = path.resolve(here, '..', 'target', 'exchange', digest, 'exchange');
  if (fs.existsSync(bin)) return bin;
  const own = path.join(path.dirname(bin), `build-${process.pid}`);
  fs.mkdirSync(own, {recursive: true});
  try {
    const r = spawnSync('rustc', ['--edition', '2021', '-o', path.join(own, 'exchange'), SOURCE], {cwd: here, encoding: 'utf8'});
    if (r.error) throw new Error(`the exchange helper of the race specs cannot be built: rustc did not start (${r.error.code})`);
    if (r.status !== 0) throw new Error(`the exchange helper of the race specs cannot be built: rustc exited ${r.status}: ${firstLine(r.stderr)}`);
    fs.renameSync(path.join(own, 'exchange'), bin);
  } finally { fs.rmSync(own, {recursive: true, force: true}); }
  return bin;
}

// Exchange `a` and `b` in `dir` twice (the tree as it was): throws the helper's line when this host cannot exchange.
export function exchangeTwice(dir, a, b) {
  const r = spawnSync(exchanger(), ['2', a, b], {cwd: dir, encoding: 'utf8'});
  if (r.error || r.status !== 0) throw new Error(firstLine(r.stderr) + (r.error ? ` (${r.error.code})` : ''));
}

// Read with `read()` while a child exchanges `a` and `b` in `dir`, until the plain state was served 100 times and the
// exchanged one refused or absent 100 times, or until `leaked(answer)` holds. Returns the answers by status, the number
// of reads, the leaked answer (or null) and the time the race took.
export async function raceExchange({dir, a, b, read, leaked, each = 100, most = 20000}) {
  const t0 = performance.now();
  const swapper = spawnChild(exchanger(), ['0', a, b], {cwd: dir, stdio: ['ignore', 'ignore', 'pipe']});
  let said = '', ended = null;
  swapper.stderr.on('data', (d) => { said += d; });
  const closed = new Promise((ok) => swapper.once('close', (code, signal) => { ended = firstLine(said) === '(no output)' ? `exit ${code ?? signal}` : firstLine(said); ok(); }));
  const race = {};
  let reads = 0, hit = null;
  const enough = () => (race[200] || 0) >= each && (race[403] || 0) + (race[404] || 0) >= each;
  try {
    while (!hit && !enough()) {
      if (ended != null) throw new Error(`the exchange loop ended after ${reads} reads: ${ended}`);
      if (reads >= most) throw new Error(`after ${reads} reads beside the exchange loop each state was not seen ${each} times: ${JSON.stringify(race)}`);
      for (let i = 0; i < 200 && !hit; i++) {
        reads++;
        const r = read();
        race[r.status] = (race[r.status] || 0) + 1;
        if (leaked(r)) hit = r;
      }
      await new Promise((ok) => setImmediate(ok));
    }
  } finally {
    // the helper's own process group, not only the guard: a guard signalled before it set its handlers would end alone
    // and leave the helper exchanging, holding the pipe that `closed` waits for
    const pid = await swapper.started;
    if (pid) try { process.kill(-pid, 'SIGKILL'); } catch { /* gone */ }
    swapper.kill();
    await closed;
  }
  return {race, reads, hit, ms: Math.round(performance.now() - t0)};
}
