// The reader of the runner's records (runnerlive.mjs) over hand-made runs with invented names, one server of its own: what a
// temporary file a killed runner left changes, the size bound of runner.json, the cut of a start line, the pid forms, the
// states of the process and the one place the server asks the kernel.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawn, execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {readText} from '../runfiles.mjs';
import {startServer} from './serve.mjs';
import {writeLoomRun, EXAMPLE_VIEW} from './loom-run-fixture.mjs';
import {writeLiveRun, startTicks} from './runner-fixture.mjs';
import * as Live from '../runnerlive.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'runner-live-guards');
const runs = path.join(root, 'runs');
let srv, base, sources;
const me = {pid: process.pid, start_ticks: startTicks(process.pid)};
const dirOf = (name) => path.join(runs, name);
const source = (name) => sources.find((s) => s.path.endsWith(`/${name}/store`)).index;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); });

const recordText = (extra = {}) => JSON.stringify({format: 'runbook.runner/1', pid: me.pid, start_ticks: me.start_ticks, started_at: 1, closed_at: null, ...extra});
const withRecord = (name, text, opts = {}) => {
  writeLiveRun(dirOf(name), {runner: null, ...opts});
  fs.writeFileSync(path.join(dirOf(name), 'runner.json'), text);
};
// a record of exactly `bytes` bytes
const recordOfSize = (bytes) => {
  const base = recordText({pad: ''});
  return recordText({pad: 'x'.repeat(bytes - base.length)});
};
const startLine = (action, extra = {}) => JSON.stringify({at: Math.floor(Date.now() / 1000) - 5, event: 'action_started', source: 'program_start', role: 'program', case: 'CASE-1', action, try: 1, ...extra}) + '\n';
const withStart = (name, line) => {
  writeLiveRun(dirOf(name), {runner: me});
  fs.appendFileSync(path.join(dirOf(name), 'events.jsonl'), line);
};
const liveOf = async (name) => (await (await fetch(`${base}/api/live?source=${source(name)}`)).json()).liveness;

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  // what a killed runner leaves in the run directory: a temporary file of its writer, under a dot name
  writeLiveRun(dirOf('leftover'), {runner: me});
  fs.writeFileSync(path.join(dirOf('leftover'), '.runner.json.4242.0.tmp'), `MARKER-LEFTOVER-RECORD ${recordText()}`);
  fs.writeFileSync(path.join(dirOf('leftover'), '.outcome.json.4242.1.tmp'), 'MARKER-LEFTOVER-OUTCOME {"outcome":"x"}');
  fs.writeFileSync(path.join(dirOf('leftover'), '.tree-status.json.4242.2.tmp'), 'MARKER-LEFTOVER-TREE');
  withRecord('rec-4096', recordOfSize(4096));
  withRecord('rec-4097', recordOfSize(4097));
  withStart('cut-256', startLine('A'.repeat(256), {case: 'C'.repeat(256), source: 'S'.repeat(256)}));
  withStart('cut-257', startLine('B'.repeat(257), {case: 'C'.repeat(257), source: 'S'.repeat(257)}));
  withStart('cut-astral', startLine('\u{1F600}'.repeat(256) + 'tail'));
  withStart('lone', startLine('a\ud83d'));
  for (const [k, v] of Object.entries({'pid-2e31': '2147483648', 'pid-2e53m1': '9007199254740991', 'pid-2e53': '9007199254740992', 'pid-negzero': '-0'})) {
    withRecord(k, `{"format":"runbook.runner/1","pid":${v},"start_ticks":${me.start_ticks},"started_at":1,"closed_at":null}`);
  }
  writeLiveRun(dirOf('plain-run'), {runner: me});
  writeLoomRun(path.join(runs, 'ended'));
  const view = path.join(root, 'example.view.json');
  fs.writeFileSync(view, JSON.stringify(EXAMPLE_VIEW));
  const s = await startServer(['--view', view, '--runs', runs]);
  srv = s.proc;
  base = s.url;
  sources = await (await fetch(base + '/api/sources')).json();
});

test('guard: a temporary file of the runner that a killed runner left under a dot name is neither served nor named by any route', async () => {
  const s = source('leftover');
  const seen = [];
  for (const rel of ['.runner.json.4242.0.tmp', '.outcome.json.4242.1.tmp', '.tree-status.json.4242.2.tmp', 'work/.runner.json.4242.0.tmp']) {
    const r = await fetch(`${base}/api/file?source=${s}&path=${encodeURIComponent(rel)}`);
    const t = await r.text();
    expect(r.status, `${rel}: served`).toBeGreaterThanOrEqual(400);
    expect(t, `${rel}: its text is in the answer`).not.toContain('MARKER-LEFTOVER');
  }
  for (const route of ['run', 'products', 'live', 'tree', 'workfacts', 'meta', 'view-info']) {
    const t = await (await fetch(`${base}/api/${route}?source=${s}`)).text();
    seen.push(t);
  }
  for (const t of seen) {
    expect(t).not.toContain('MARKER-LEFTOVER');
    expect(t).not.toContain('.runner.json.4242');
    expect(t).not.toContain('.outcome.json.4242');
  }
  expect((await liveOf('leftover')).state, 'a leftover temporary file changes the answer').toBe('running');
});

test('guard: a runner.json of exactly 4096 bytes is the record and one of 4097 bytes is not', async () => {
  expect(Live.RECORD_LIMIT).toBe(4096);
  expect((await liveOf('rec-4096')).state).toBe('running');
  expect((await liveOf('rec-4097')).state).toBe('unknown');
});

test('the reason given for a runner.json that is not read is not "the runner writes no record"', async () => {
  const l = await liveOf('rec-4097');
  expect(l.state).toBe('unknown');
  expect(l.why, 'the runner did write a record: the inspector did not read it').not.toContain('writes no record');
});

test('guard: the text of a start line is cut at 256 characters, not at 255 or 257, never inside a character, and a lone surrogate is made well formed', async () => {
  for (const k of ['action', 'case', 'source']) {
    const a = (await liveOf('cut-256')).step[k];
    expect([...a].length, `${k} of 256`).toBe(256);
    const b = (await liveOf('cut-257')).step[k];
    expect([...b].length, `${k} of 257`).toBe(256);
  }
  expect((await liveOf('cut-257')).step.action).toBe('B'.repeat(256));
  const astral = (await liveOf('cut-astral')).step.action;
  expect(astral).toBe('\u{1F600}'.repeat(256));
  const lone = (await liveOf('lone')).step.action;
  expect(lone.isWellFormed()).toBe(true);
  expect(lone).toBe('a�');
});

test('guard: a pid of 2^31, 2^53 - 1, 2^53 and -0 is never a running runner', async () => {
  for (const k of ['pid-2e31', 'pid-2e53m1', 'pid-2e53', 'pid-negzero']) {
    const l = await liveOf(k);
    expect(l.state, k).not.toBe('running');
    expect(l.state, k).not.toBe('between');
    expect(JSON.stringify(l), k).not.toContain('/proc');
  }
});

test('guard: the state of the process decides: Z and X are not live, any other state with the start time of the record is', () => {
  const dir = dirOf('plain-run');
  for (const state of ['Z', 'X']) {
    const l = Live.runnerLiveness(dir, () => ({state, ticks: me.start_ticks}));
    expect(l.state, `state ${state}`).toBe('not_live');
    expect(l.why).toContain('collected');
  }
  for (const state of ['S', 'R', 'D', 'I']) {
    expect(Live.runnerLiveness(dir, () => ({state, ticks: me.start_ticks})).state, `state ${state}`).toBe('running');
  }
  expect(Live.runnerLiveness(dir, () => ({state: 'S', ticks: me.start_ticks + 1})).state).toBe('not_live');
  expect(Live.runnerLiveness(dir, () => ({state: 'S', ticks: null})).state).toBe('not_live');
  expect(Live.runnerLiveness(dir, () => ({gone: true})).state).toBe('not_live');
  expect(Live.runnerLiveness(dir, () => ({error: 'EIO'})).state).toBe('unknown');
});

test('guard: only test code can name the reader of /proc: the server calls runnerLiveness with the run directory only, and runnerlive.mjs reads no environment', () => {
  const server = fs.readFileSync(path.join(ui, 'server.mjs'), 'utf8');
  const calls = server.split('\n').filter((l) => !l.trim().startsWith('//') && l.includes('runnerLiveness'));
  expect(calls.map((l) => l.trim())).toEqual(["import {runnerLiveness} from './runnerlive.mjs';", 'const runner = runnerLiveness(runDir);']);
  const src = fs.readFileSync(path.join(ui, 'runnerlive.mjs'), 'utf8').split('\n').filter((l) => !l.trim().startsWith('//')).join('\n');
  expect(src).not.toMatch(/process\.env|process\.argv|import\.meta|child_process|process\.kill/);
});

test('guard: a runner.json whose size changes between the size check of the read and the read is still read for at most RECORD_LIMIT bytes', async () => {
  const dir = path.join(root, 'race');
  fs.mkdirSync(dir, {recursive: true});
  const file = path.join(dir, 'runner.json');
  fs.writeFileSync(file, recordText());
  // a program of the run that makes the file small and 16 MiB (sparse) in turn, as fast as it can
  const toggler = spawn(process.execPath, ['-e', `const fs = require('node:fs'); const fd = fs.openSync(${JSON.stringify(file)}, 'r+'); console.log('ready'); const end = Date.now() + 20000; while (Date.now() < end) { fs.ftruncateSync(fd, 100); fs.ftruncateSync(fd, 16 * 1048576); }`], {stdio: ['ignore', 'pipe', 'ignore']});
  try {
    await new Promise((resolve) => toggler.stdout.once('data', resolve));
    let over = 0, longest = 0, taken = 0, most = 0;
    const realRead = fs.readSync;
    fs.readSync = (...args) => { const n = realRead(...args); taken += n; return n; };
    try {
      for (let i = 0; i < 1500; i++) {
        taken = 0;
        const t = readText(file, dir, Live.RECORD_LIMIT);
        if (t != null && t.length > Live.RECORD_LIMIT) { over++; longest = Math.max(longest, t.length); }
        most = Math.max(most, taken);
      }
    } finally {
      fs.readSync = realRead;
    }
    expect({over, longest}, 'reads that returned more than RECORD_LIMIT bytes').toEqual({over: 0, longest: 0});
    expect(most, 'the most bytes one read took from the file').toBeLessThanOrEqual(Live.RECORD_LIMIT + 1);
  } finally {
    toggler.kill();
  }
});

test('guard: a file that is longer than its limit at the time of the read is answered as too large, whatever size the check saw, and no more than the limit plus one byte is taken from it', () => {
  const dir = path.join(root, 'grown');
  fs.mkdirSync(dir, {recursive: true});
  const file = path.join(dir, 'runner.json');
  fs.writeFileSync(file, 'x'.repeat(2 * 1048576));
  const realStat = fs.fstatSync, realRead = fs.readSync;
  let taken = 0;
  // the size of the check is the one of a file that was small a moment ago
  fs.fstatSync = (...args) => { const st = realStat(...args); return Object.assign(Object.create(Object.getPrototypeOf(st)), st, {size: 100}); };
  fs.readSync = (...args) => { const n = realRead(...args); taken += n; return n; };
  try {
    expect(readText(file, dir, Live.RECORD_LIMIT)).toBeNull();
    expect(taken, 'bytes taken from the file').toBeLessThanOrEqual(Live.RECORD_LIMIT + 1);
    taken = 0;
    expect(readText(file, dir, 1000)).toBeNull();
    expect(taken, 'bytes taken from the file with a limit of 1000').toBeLessThanOrEqual(1001);
  } finally {
    fs.fstatSync = realStat;
    fs.readSync = realRead;
  }
});

// runs `body` with every byte `fs.readSync` returns counted, and `fs.fstatSync` answering `size` when it is given
const counting = (size, body) => {
  const realStat = fs.fstatSync, realRead = fs.readSync;
  const seen = {taken: 0};
  if (size != null) fs.fstatSync = (...args) => { const st = realStat(...args); return Object.assign(Object.create(Object.getPrototypeOf(st)), st, {size}); };
  fs.readSync = (...args) => { const n = realRead(...args); seen.taken += n; return n; };
  try { return body(seen); } finally { fs.fstatSync = realStat; fs.readSync = realRead; }
};

test('guard: a file of exactly its limit is read, a file of one byte more is not, and neither takes more than the limit plus one byte', () => {
  const dir = path.join(root, 'edge');
  fs.mkdirSync(dir, {recursive: true});
  for (const limit of [1, 2, 1000, 1048576, 1048577]) {
    for (const [extra, expected] of [[0, true], [1, false]]) {
      const file = path.join(dir, `f-${limit}-${extra}.txt`);
      fs.writeFileSync(file, 'x'.repeat(limit + extra));
      counting(null, (seen) => {
        const t = readText(file, dir, limit);
        expect(t != null, `limit ${limit} with ${extra} byte more`).toBe(expected);
        if (t != null) expect(t.length).toBe(limit);
        expect(seen.taken, `bytes taken, limit ${limit} with ${extra} byte more`).toBeLessThanOrEqual(limit + 1);
      });
    }
  }
});

test('guard: an empty file is read as empty and a limit of zero reads no byte of a file that holds one', () => {
  const dir = path.join(root, 'empty');
  fs.mkdirSync(dir, {recursive: true});
  fs.writeFileSync(path.join(dir, 'empty.txt'), '');
  fs.writeFileSync(path.join(dir, 'one.txt'), 'x');
  expect(readText(path.join(dir, 'empty.txt'), dir, 10)).toBe('');
  expect(readText(path.join(dir, 'empty.txt'), dir, 0)).toBe('');
  counting(null, (seen) => {
    expect(readText(path.join(dir, 'one.txt'), dir, 0)).toBeNull();
    expect(seen.taken).toBeLessThanOrEqual(1);
  });
});

test('guard: a character of three bytes that a chunk of the read ends inside is not torn', () => {
  const dir = path.join(root, 'torn');
  fs.mkdirSync(dir, {recursive: true});
  const text = '€'.repeat(700000);
  const file = path.join(dir, 'f.txt');
  fs.writeFileSync(file, text);
  // the size of the check is small, so the first chunk is 101 bytes (33 characters and two bytes of the 34th) and the rest are 1 MiB
  counting(100, () => { expect(readText(file, dir, 4 * 1048576)).toBe(text); });
});

test('guard: a file that keeps growing while it is read is taken for at most the limit plus one byte per read', async () => {
  const dir = path.join(root, 'growing');
  fs.mkdirSync(dir, {recursive: true});
  const file = path.join(dir, 'f.txt');
  fs.writeFileSync(file, 'x'.repeat(100));
  const limit = 5000;
  const grower = spawn(process.execPath, ['-e', `const fs = require('node:fs'); const fd = fs.openSync(${JSON.stringify(file)}, 'a'); console.log('ready'); const chunk = Buffer.alloc(65536, 120); const end = Date.now() + 15000; while (Date.now() < end) fs.writeSync(fd, chunk);`], {stdio: ['ignore', 'pipe', 'ignore']});
  try {
    await new Promise((resolve) => grower.stdout.once('data', resolve));
    let over = 0, most = 0;
    for (let i = 0; i < 300; i++) {
      counting(null, (seen) => {
        const t = readText(file, dir, limit);
        if (t != null && t.length > limit) over++;
        most = Math.max(most, seen.taken);
      });
    }
    expect({over}, 'reads that answered more than the limit').toEqual({over: 0});
    expect(most, 'the most bytes one read took').toBeLessThanOrEqual(limit + 1);
  } finally {
    grower.kill();
  }
});

test('guard: a named pipe and a link to a device inside the run are refused without a read and without waiting', () => {
  const dir = path.join(root, 'kinds');
  fs.mkdirSync(dir, {recursive: true});
  const stamp = `${process.pid}-${Date.now()}`;
  const fifo = path.join(dir, `p-${stamp}`);
  execFileSync('mkfifo', [fifo]);
  const dev = path.join(dir, `zero-${stamp}`);
  fs.symlinkSync('/dev/zero', dev);
  counting(null, (seen) => {
    const t0 = Date.now();
    expect(readText(fifo, dir, 1000)).toBeNull();
    expect(readText(dev, dir, 1000)).toBeNull();
    expect(Date.now() - t0).toBeLessThan(2000);
    expect(seen.taken).toBe(0);
  });
});

test('guard: readStat takes a pid only as a number whose text is digits', () => {
  const pid = process.pid;
  for (const form of [` ${pid}`, `${pid}\n`, `0x${pid.toString(16)}`, `${pid}e0`, `${pid}.0`, `+${pid}`]) {
    expect(Live.readStat(form), `readStat(${JSON.stringify(form)}) reads /proc/${pid}/stat`).toEqual({gone: true});
  }
  expect(Live.readStat(pid).gone, 'the number of a live process is read').toBeUndefined();
  expect(Live.readStat(String(pid)).gone, 'the digits of a live process are read').toBeUndefined();
});
