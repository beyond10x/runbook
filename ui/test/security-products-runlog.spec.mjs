// Independent review, invariants 2 and 6 for the one new read that is not a regular-file read of products.mjs
// `readRegular`: outputOf reads Loom's run log of the case with `readText` (runfiles.mjs), which checks the run path
// and then calls fs.readFileSync on it. Two conformance checks of that read, against the story's own rule ("serves a
// regular file only", a read bounded by a limit):
//   a named pipe in place of the run log must not hang the server;
//   a very large run log must not be read whole into memory by a request.
// Own servers (one per case, the pipe case blocks the server's thread), run written by hand, invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT} from './products-fixture.mjs';
import {fixtureRoot, ask, VmHWM} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-runlog');
const hex = (s) => Buffer.from(s).toString('hex');
const logOf = (run) => path.join(root, run, 'r', 'store', `runlog-case-${hex(ROOT)}.jsonl`);
const MB = 1024 * 1024;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.beforeAll(({}, info) => { if (info.project.name === 'wide') fs.rmSync(root, {recursive: true, force: true}); });

async function seqOfCheck(base) {
  const c = (await ask(base, '/api/tree?source=0')).json.cases.find((x) => x.id === ROOT);
  return {rel: c.rel, seq: c.events.find((e) => e.event === 'action_finished' && e.action === 'notes.check').seq};
}

test('inv 2: a named pipe in place of the run log of a case does not hang the server', async () => {
  test.setTimeout(40000);
  writeProductsRun(path.join(root, 'fifo', 'r'));
  fs.rmSync(logOf('fifo'));
  const mk = spawnSync('mkfifo', [logOf('fifo')]);
  expect(mk.status).toBe(0);
  const s = await startServer(['--no-view', '--runs', path.join(root, 'fifo')]);
  let hung = false, status = null, nextHung = false;
  try {
    const {rel, seq} = await seqOfCheck(s.url);
    const r = await ask(s.url, `/api/output?source=0&case=${encodeURIComponent(rel)}&seq=${seq}`, {timeout: 4000});
    hung = r.hung;
    status = r.status;
    // the next request, to a route that reads nothing of the log, must be answered too
    const next = await ask(s.url, '/api/meta', {timeout: 3000});
    nextHung = next.hung;
  } finally {
    // let a blocked open() return so that the server can end: a writer opens the pipe and closes it
    try { fs.closeSync(fs.openSync(logOf('fifo'), fs.constants.O_RDWR | fs.constants.O_NONBLOCK)); } catch { /* nothing blocked */ }
    s.proc.kill();
  }
  expect(hung, 'the request for the output of an action whose run log is a named pipe').toBe(false);
  expect(status).toBe(200);
  expect(nextHung, 'the server answers /api/meta after the request for the output').toBe(false);
});

test('inv 6: a run log of 150 MB is not read whole into the memory of the server by a request for one output', async () => {
  test.setTimeout(50000);
  writeProductsRun(path.join(root, 'huge', 'r'));
  const f = logOf('huge');
  const head = fs.readFileSync(f, 'utf8');
  const fd = fs.openSync(f, 'w');
  fs.writeSync(fd, head);
  fs.ftruncateSync(fd, 150 * MB); // sparse: the padding is zero bytes, a line the reader skips
  fs.closeSync(fd);
  const s = await startServer(['--no-view', '--runs', path.join(root, 'huge')]);
  try {
    const pid = await s.proc.started;
    const {rel, seq} = await seqOfCheck(s.url);
    const before = VmHWM(pid);
    const r = await ask(s.url, `/api/output?source=0&case=${encodeURIComponent(rel)}&seq=${seq}`, {timeout: 30000});
    expect(r.hung).toBe(false);
    const after = VmHWM(pid);
    expect(r.status).toBe(200);
    if (before != null && after != null) expect(after - before, `peak resident memory of the server grew by ${Math.round((after - before) / MB)} MB for one request`).toBeLessThan(64 * MB);
  } finally { s.proc.kill(); }
});
