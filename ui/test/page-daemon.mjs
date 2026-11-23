// The daemon the `security-start-*` specs share: one for each worker process and kind, started on first use and ended
// with the worker (spawnChild ends it with its parent), where every spec file started its own. A worker runs its
// files one after another, so a test counts the runs it adds (`runs after - runs before`) and never assumes an empty
// list. Kinds: `open` (stub runtime, no token, floor linux) and `token` (the same with the token TOKEN).
// A test that needs another floor, a runbook directory or the Loom runtime starts a daemon of its own (daemon-start.spec.mjs).
// The state directory lies under SEC_SCRATCH when it is set, else under node_modules, and is removed when the worker ends.
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import {randomBytes} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
export const TOKEN = `tok-${randomBytes(24).toString('hex')}`;
const daemons = new Map();
let dir = null;

process.on('exit', () => {
  for (const p of daemons.values()) p.proc?.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

function start(kind) {
  return new Promise((resolve, reject) => {
    fs.mkdirSync(path.join(dir, kind, 'empty'), {recursive: true});
    const env = {...process.env, HOME: path.join(dir, kind), RUNBOOK_TOKEN: kind === 'token' ? TOKEN : ''};
    const proc = spawnChild(bin, ['serve', '--ui', '--principal', 'agent:test-daemon', '--listen', '127.0.0.1:0', '--state-dir', path.join(dir, kind, 'state')], {cwd: path.join(dir, kind, 'empty'), env, stdio: ['ignore', 'pipe', 'pipe']});
    let out = '', err = '';
    const timer = setTimeout(() => { proc.kill(); reject(new Error(`no listening line in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { err += d; });
    proc.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], log: () => out + err}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

// the daemon of this kind in this worker
export function pageDaemon(kind) {
  if (!fs.existsSync(bin)) throw new Error(`${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`);
  if (!dir) {
    const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
    fs.mkdirSync(root, {recursive: true});
    dir = fs.mkdtempSync(path.join(root, '.pw-start-shared-'));
  }
  if (!daemons.has(kind)) daemons.set(kind, start(kind).then((d) => { daemons.set(kind, d); return d; }));
  const d = daemons.get(kind);
  return d instanceof Promise ? d : Promise.resolve(d);
}
