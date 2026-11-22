// A test child ends with its parent (child.mjs, guard.mjs). A parent process starts the inspector's server
// through the helper every spec uses (startServer); the parent is ended with SIGKILL, so no teardown runs,
// and the server's pid must be gone within 2 s. The negative starts the server without the guard: it stays
// until the test ends it by its pid. The parent itself is started through the helper too, so a worker ended
// in the middle of a test leaves nothing behind. Each test takes about 1 s.
import {test, expect} from '@playwright/test';
import {spawnChild} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const parentScript = path.join(path.dirname(fileURLToPath(import.meta.url)), 'child-parent.mjs');
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
const goneWithin = async (pid, ms) => { for (const end = Date.now() + ms; Date.now() < end; await new Promise((r) => setTimeout(r, 25))) if (!alive(pid)) return true; return !alive(pid); };

async function startParent(info, mode) {
  const runs = info.outputPath('runs');
  fs.mkdirSync(runs, {recursive: true});
  const parent = spawnChild(process.execPath, [parentScript, runs, ...(mode ? [mode] : [])], {stdio: ['ignore', 'pipe', 'inherit'], endWithParent: mode !== 'plain'});
  const line = await new Promise((resolve, reject) => {
    let out = '';
    parent.stdout.on('data', (d) => { out += d; if (out.includes('\n')) resolve(out.split('\n')[0]); });
    parent.on('exit', (code) => reject(new Error(`parent exited with ${code} before it printed: ${out}`)));
    setTimeout(() => reject(new Error(`parent printed nothing in 20 s: ${out}`)), 20000);
  });
  parent.removeAllListeners('exit');
  return {...JSON.parse(line), parentPid: parent.childPid};
}

test('a server started through the helper ends within 2 s when its parent is ended with SIGKILL', async ({}, info) => {
  test.skip(info.project.name !== 'wide', 'no page: one project is enough');
  const {parentPid, server} = await startParent(info, '');
  expect(alive(server)).toBe(true);
  process.kill(parentPid, 'SIGKILL');
  const gone = await goneWithin(server, 2000);
  if (!gone) process.kill(server, 'SIGKILL');
  expect(gone, `server ${server} still alive 2 s after its parent was killed`).toBe(true);
});

test('without the guard the server stays when its parent is ended with SIGKILL', async ({}, info) => {
  test.skip(info.project.name !== 'wide', 'no page: one project is enough');
  const {parentPid, server} = await startParent(info, 'plain');
  try {
    process.kill(parentPid, 'SIGKILL');
    await new Promise((r) => setTimeout(r, 1000));
    expect(alive(server)).toBe(true);
  } finally {
    process.kill(server, 'SIGKILL');
  }
  expect(await goneWithin(server, 2000)).toBe(true);
});
