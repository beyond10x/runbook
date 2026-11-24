// The Now band of a run with the runner's own records (runner.json, the action_started events): the server's answer in
// /api/live `liveness` and the page's three texts at the wide size. One server of its own (a port the kernel gives) over a
// runs directory of hand-made runs with invented names (test/runner-fixture.mjs):
//   in-step    the runner is this test process, an action started 125 s ago and no step finished since
//   between    the same runner, the last step finished and Loom ended it on its step budget
//   gone       a child the test started and ended: the pid is not alive
//   reused     the pid of a live process with another start time than the record (a pid the kernel gave twice)
//   closed     the runner is alive and says it closed its record
//   stepcall   an action the runner could not name (the child host: a step is the unit it sees)
//   plain      no runner record: today's answer (unknown)
//   markup     an action with markup in its name
// The liveness test is /proc/<pid>/stat field 22 against the record: no signal, no new permission.
import {test, expect} from '@playwright/test';
import {spawn, execFileSync} from 'node:child_process';
import {once} from 'node:events';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeLoomRun, EXAMPLE_VIEW} from './loom-run-fixture.mjs';
import {writeLiveRun, startTicks} from './runner-fixture.mjs';
import * as Live from '../runnerlive.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'runner-live');
const runs = path.join(root, 'runs');
const outside = path.join(root, 'outside');
let srv, base, sources, gonePid;
const kids = [];
const me = {pid: process.pid, start_ticks: startTicks(process.pid)};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { for (const k of kids) k.kill(); srv?.kill(); });

// what the record and the events file of a run are when a program of the run (`--confinement local`) wrote them, and a
// process that is not what the record says
const dirOf = (name) => path.join(runs, name);
const raw = (name, text) => { writeLiveRun(dirOf(name), {runner: null}); fs.writeFileSync(path.join(dirOf(name), 'runner.json'), text); };
const record = (name, extra) => raw(name, JSON.stringify({format: 'runbook.runner/1', pid: me.pid, start_ticks: me.start_ticks, started_at: 1, closed_at: null, ...extra}));
const answer = async (name) => {
  const text = await (await fetch(`${base}/api/live?source=${source(name)}`)).text();
  return {text, liveness: JSON.parse(text).liveness};
};
// a child of the test that prints one line when it is ready; `script` is sh
const startSh = async (script) => {
  const k = spawn('sh', ['-c', script], {stdio: ['pipe', 'pipe', 'ignore']});
  kids.push(k);
  let out = '';
  k.stdout.on('data', (d) => { out += d; });
  for (let i = 0; i < 200 && !out.includes('\n'); i++) await new Promise((r) => setTimeout(r, 25));
  return {k, out: out.trim()};
};
const PID_FORMS = {text: '"2"', slash: '"../../etc/passwd"', self: '"self/environ"', rootpath: '"1/root/x"', float: '1.5', array: '[2]', object: '{"a":1}', suffix: '"2abc"', bool: 'true', nul: 'null', huge: '1e30', imprecise: '9007199254740993', exp: '"1e1"', hex: '"0x2"', blank: '" 2 "'};
const PID_INTEGERS = {negative: '-1', zero: '0', one: '1', big: '4294967296'};
const startLine = (action, extra = {}) => JSON.stringify({at: Math.floor(Date.now() / 1000) - 5, event: 'action_started', source: 'program_start', role: 'program', case: 'CASE-1', action, try: 1, ...extra}) + '\n';

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  // a process the test started and ended: its start time is the record's, its pid belongs to nothing now
  const child = spawn('sleep', ['300'], {stdio: 'ignore'});
  await once(child, 'spawn');
  const gone = {pid: child.pid, start_ticks: startTicks(child.pid)};
  gonePid = child.pid;
  child.kill('SIGKILL');
  await once(child, 'exit');
  fs.mkdirSync(outside, {recursive: true});
  for (const [k, v] of Object.entries({...PID_FORMS, ...PID_INTEGERS})) raw(`pid-${k}`, `{"format":"runbook.runner/1","pid":${v},"start_ticks":${me.start_ticks},"started_at":1,"closed_at":null}`);
  const t = me.start_ticks;
  const ticks = {text: JSON.stringify(String(t)), missing: undefined, null: 'null', zero: '0', negative: '-1', bool: 'true', array: `[${t}]`, object: '{}', fraction: String(t + 0.5), exp: JSON.stringify(`${t}e0`), hex: JSON.stringify(`0x${t.toString(16)}`)};
  for (const [k, v] of Object.entries(ticks)) raw(`ticks-${k}`, `{"format":"runbook.runner/1","pid":${me.pid},${v === undefined ? '' : `"start_ticks":${v},`}"started_at":1,"closed_at":null}`);
  raw('fmt-other', JSON.stringify({format: 'runbook.runner/2', pid: me.pid, start_ticks: me.start_ticks, started_at: 1, closed_at: null}));
  raw('fmt-missing', JSON.stringify({pid: me.pid, start_ticks: me.start_ticks, started_at: 1, closed_at: null}));
  raw('fmt-array', JSON.stringify([{format: 'runbook.runner/1', pid: me.pid, start_ticks: me.start_ticks}]));
  // a record that is a good record of a live runner, held outside the run
  const live = JSON.stringify({format: 'runbook.runner/1', pid: me.pid, start_ticks: me.start_ticks, started_at: 1, closed_at: null});
  fs.writeFileSync(path.join(outside, 'live-runner.json'), live);
  writeLiveRun(dirOf('rec-link'), {runner: null});
  fs.symlinkSync(path.join(outside, 'live-runner.json'), path.join(dirOf('rec-link'), 'runner.json'));
  writeLiveRun(dirOf('rec-hardlink'), {runner: null});
  fs.linkSync(path.join(outside, 'live-runner.json'), path.join(dirOf('rec-hardlink'), 'runner.json'));
  writeLiveRun(dirOf('rec-fifo'), {runner: null});
  execFileSync('mkfifo', [path.join(dirOf('rec-fifo'), 'runner.json')]);
  writeLiveRun(dirOf('rec-dir'), {runner: null});
  fs.mkdirSync(path.join(dirOf('rec-dir'), 'runner.json'));
  writeLiveRun(dirOf('rec-1gb'), {runner: null});
  fs.writeFileSync(path.join(dirOf('rec-1gb'), 'runner.json'), live);
  fs.truncateSync(path.join(dirOf('rec-1gb'), 'runner.json'), 1 << 30);
  record('rec-4mb', {pad: 'x'.repeat(4 * 1048576)});
  // events: a link to a file outside that holds a start, a pipe, and one line of 50 MB after the real start
  writeLiveRun(dirOf('ev-link'), {runner: me});
  fs.writeFileSync(path.join(outside, 'events.jsonl'), startLine('OUTSIDE-ACTION', {case: 'OUTSIDE-CASE'}));
  fs.rmSync(path.join(dirOf('ev-link'), 'events.jsonl'));
  fs.symlinkSync(path.join(outside, 'events.jsonl'), path.join(dirOf('ev-link'), 'events.jsonl'));
  writeLiveRun(dirOf('ev-fifo'), {runner: me});
  fs.rmSync(path.join(dirOf('ev-fifo'), 'events.jsonl'));
  execFileSync('mkfifo', [path.join(dirOf('ev-fifo'), 'events.jsonl')]);
  writeLiveRun(dirOf('ev-big'), {runner: me});
  fs.writeFileSync(path.join(dirOf('ev-big'), 'events.jsonl'), startLine('old.action') + (JSON.stringify({at: 1, event: 'decision_asked', decision: 'x'.repeat(200)}) + '\n').repeat(10000) + startLine('new.action'));
  writeLiveRun(dirOf('ev-50mb'), {runner: me});
  const ev = path.join(dirOf('ev-50mb'), 'events.jsonl');
  fs.truncateSync(ev, fs.statSync(ev).size + 50 * 1048576);
  writeLiveRun(dirOf('long-action'), {runner: me});
  fs.appendFileSync(path.join(dirOf('long-action'), 'events.jsonl'), startLine('A'.repeat(300000), {case: 'C'.repeat(300000), source: 'S'.repeat(300000)}));
  writeLiveRun(dirOf('wide-action'), {runner: me});
  fs.appendFileSync(path.join(dirOf('wide-action'), 'events.jsonl'), startLine('\u{1F600}'.repeat(5000)));
  // a process whose name holds spaces, brackets and a closing parenthesis
  const odd = await startSh(`printf '%s' ') S 1 2 3 (] [' > /proc/self/comm; echo ready; read x`);
  expect(odd.out, 'the odd-named child did not start').toBe('ready');
  expect(fs.readFileSync(`/proc/${odd.k.pid}/comm`, 'utf8').trim()).toBe(') S 1 2 3 (] [');
  writeLiveRun(dirOf('odd-name'), {runner: {pid: odd.k.pid, start_ticks: startTicks(odd.k.pid)}});
  // a runner killed by SIGKILL whose parent has not waited for it: the pid is there, a zombie
  const z = await startSh('sleep 300 & echo $!; exec cat');
  const zpid = Number(z.out);
  expect(zpid).toBeGreaterThan(1);
  const zt = startTicks(zpid);
  process.kill(zpid, 'SIGKILL');
  for (let i = 0; i < 100 && !fs.readFileSync(`/proc/${zpid}/stat`, 'utf8').includes(') Z '); i++) await new Promise((r) => setTimeout(r, 25));
  expect(fs.readFileSync(`/proc/${zpid}/stat`, 'utf8'), 'the killed child is not a zombie').toContain(') Z ');
  writeLiveRun(dirOf('zombie'), {runner: {pid: zpid, start_ticks: zt}});
  writeLiveRun(path.join(runs, 'in-step'), {runner: me, tryNumber: 2});
  writeLiveRun(path.join(runs, 'between'), {runner: me, state: 'between'});
  writeLiveRun(path.join(runs, 'gone'), {runner: gone});
  writeLiveRun(path.join(runs, 'reused'), {runner: {pid: me.pid, start_ticks: me.start_ticks + 1}});
  writeLiveRun(path.join(runs, 'closed'), {runner: {...me, closed_at: Math.floor(Date.now() / 1000)}});
  writeLiveRun(path.join(runs, 'stepcall'), {runner: me, source: 'step_call', action: null, caseId: 'CASE-1', tryNumber: null, inStore: false});
  writeLiveRun(path.join(runs, 'markup'), {runner: me, action: '<img src=x onerror=alert(1)>'});
  writeLiveRun(path.join(runs, 'plain'), {runner: null});
  writeLoomRun(path.join(runs, 'ended'));
  const view = path.join(root, 'example.view.json');
  fs.writeFileSync(view, JSON.stringify(EXAMPLE_VIEW));
  const s = await startServer(['--view', view, '--runs', runs]);
  srv = s.proc;
  base = s.url;
  sources = await (await fetch(base + '/api/sources')).json();
});

const source = (name) => sources.find((s) => s.path.endsWith(`/${name}/store`)).index;
const liveness = async (name) => (await (await fetch(`${base}/api/live?source=${source(name)}`)).json()).liveness;
const clockOf = (page, t) => page.evaluate((x) => new Date(x).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}), t);
const open = async (page, name) => {
  await page.setViewportSize({width: 1600, height: 1000});
  await page.goto(`${base}/?run=${name}`);
  await expect(page.locator('#now .now-st')).toBeVisible({timeout: 30000});
};

test('the server says running with the action, the case, the try and when it started', async () => {
  expect(gonePid).toBeGreaterThan(0);
  const l = await liveness('in-step');
  expect(l.state).toBe('running');
  expect(l.runner).toMatchObject({pid: process.pid, closed_at: null});
  expect(l.step).toMatchObject({action: 'notes.write', case: 'CASE-1', try: 2, source: 'program_start', role: 'program'});
  expect(l.step.at).toBeGreaterThan(Date.now() / 1000 - 140);
  expect(l.step.at).toBeLessThan(Date.now() / 1000 - 100);
});

test('the server says between steps for a live runner whose last action ended, and keeps the last action it started', async () => {
  const l = await liveness('between');
  expect(l.state).toBe('between');
  expect(l.step.action).toBe('notes.write');
});

test('the server says not live for a pid that is gone, a pid with another start time and a record that is closed', async () => {
  for (const name of ['gone', 'reused', 'closed']) {
    const l = await liveness(name);
    expect(l.state, name).toBe('not_live');
    expect(l.step.action, name).toBe('notes.write');
  }
  expect((await liveness('gone')).why).toContain('no process');
  expect((await liveness('closed')).why).toContain('closed');
});

test('a run without the records keeps its answer, and a run with an end record is ended', async () => {
  expect((await liveness('plain')).state).toBe('unknown');
  expect((await liveness('ended')).state).toBe('ended');
});

test('the answer follows the runner: the record of a runner that closes makes the run not live', async () => {
  const dir = path.join(root, 'runs', 'in-step');
  const file = path.join(dir, 'runner.json');
  const rec = JSON.parse(fs.readFileSync(file, 'utf8'));
  try {
    expect((await liveness('in-step')).state).toBe('running');
    fs.writeFileSync(file, JSON.stringify({...rec, closed_at: rec.started_at + 200}));
    await expect.poll(async () => (await liveness('in-step')).state).toBe('not_live');
  } finally {
    fs.writeFileSync(file, JSON.stringify(rec));
  }
  await expect.poll(async () => (await liveness('in-step')).state).toBe('running');
});

test('page: running says the action, since when and the minutes', async ({page}) => {
  await open(page, 'in-step');
  const l = await liveness('in-step');
  const since = await clockOf(page, l.step.at * 1000);
  const head = page.locator('#now .now-st');
  await expect(head).toHaveText(new RegExp(`^notes\\.write running since ${since.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}, [23] min$`));
  await expect(page.locator('#now .now-s')).toHaveClass(/m-running/);
  await expect(page.locator('#now .now-sub')).toContainText('CASE-1');
  await expect(page.locator('#now .now-sub')).toContainText('try 2');
  await expect(page.locator('#now')).not.toContainText('Stopped');
});

test('page: between steps is not a stop, though the case ended its step on the step budget', async ({page}) => {
  await open(page, 'between');
  await expect(page.locator('#now .now-st')).toHaveText('Between steps');
  await expect(page.locator('#now .now-s')).toHaveClass(/m-between/);
  await expect(page.locator('#now')).not.toContainText('Stopped');
  await expect(page.locator('#now')).not.toContainText('budget');
  await expect(page.locator('#now')).toContainText('no action is in flight');
});

test('page: not live names the last action it started, and no step is shown running', async ({page}) => {
  await open(page, 'gone');
  await expect(page.locator('#now .now-st')).toHaveText('Not live: last started notes.write');
  await expect(page.locator('#now .now-s')).toHaveClass(/m-notlive/);
  await expect(page.locator('#now .nc .nc-verb')).not.toHaveText('Running');
  await expect(page.locator('#now')).not.toContainText(/\d+ running/);
});

test('page: a runner that cannot name its action says a step runs and that it does not know the action', async ({page}) => {
  await open(page, 'stepcall');
  await expect(page.locator('#now .now-st')).toHaveText(/^A step running since \d\d:\d\d:\d\d( [AP]M)?, [23] min$/);
  await expect(page.locator('#now')).toContainText('does not know which action');
});

test('page: a run without the records says what it said before', async ({page}) => {
  await open(page, 'plain');
  await expect(page.locator('#now .now-st')).toHaveText('1 running');
  await expect(page.locator('#now')).not.toContainText('Not live');
});

test('page: run content is text, never markup', async ({page}) => {
  const errors = [];
  page.on('dialog', (d) => { errors.push(d.message()); d.dismiss(); });
  await open(page, 'markup');
  await expect(page.locator('#now .now-st')).toContainText('<img src=x onerror=alert(1)> running since');
  await expect(page.locator('#now img')).toHaveCount(0);
  await expect(page.locator('#nowbar img')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('the inspector sends no signal: the runner of the fixture is alive after every request', async () => {
  for (const name of ['in-step', 'between', 'gone', 'reused', 'closed']) await liveness(name);
  expect(fs.existsSync(`/proc/${process.pid}`)).toBe(true);
  for (const m of ['runnerlive.mjs', 'server.mjs']) expect(fs.readFileSync(path.join(ui, m), 'utf8')).not.toMatch(/process\.kill\(|\.kill\(/);
});

test('a pid that is text, a path, an array, a float or too big is no runner: the run is "unknown", and no pid of any form is running', async () => {
  for (const k of Object.keys(PID_FORMS)) {
    const {liveness} = await answer(`pid-${k}`);
    expect(liveness.state, `pid form ${k}`).toBe('unknown');
  }
  for (const k of Object.keys(PID_INTEGERS)) {
    const {liveness} = await answer(`pid-${k}`);
    expect(['not_live', 'running', 'between'].includes(liveness.state), `pid ${k}`).toBe(true);
    if (k !== 'one' && k !== 'big') expect(liveness.state, `pid ${k}`).toBe('not_live');
  }
  // pid 1 has its own start time: the record of this process's ticks on it is no live runner
  expect((await answer('pid-one')).liveness.state).toBe('not_live');
});

test('start ticks that are text, missing, null, 0, negative, a fraction, an array or a boolean make a live pid not_live with a reason', async () => {
  for (const k of ['text', 'missing', 'null', 'zero', 'negative', 'bool', 'array', 'object', 'fraction', 'exp', 'hex']) {
    const {liveness} = await answer(`ticks-${k}`);
    expect(liveness.state, `start_ticks ${k}`).toBe('not_live');
    expect(liveness.why, `start_ticks ${k}`).toMatch(/\S/);
  }
});

test('a record of another format, of no format or an array in place of the object is no record: the run is "unknown"', async () => {
  for (const k of ['fmt-other', 'fmt-missing', 'fmt-array']) expect((await answer(k)).liveness.state, k).toBe('unknown');
});

test('a runner.json that is a link out, a hard link, a pipe, a directory or 1 GB is not read, and the server answers at once', async () => {
  for (const k of ['rec-link', 'rec-hardlink', 'rec-fifo', 'rec-dir', 'rec-1gb']) {
    const t0 = Date.now();
    const {liveness} = await answer(k);
    expect(liveness.state, k).toBe('unknown');
    expect(Date.now() - t0, `${k} answered in ms`).toBeLessThan(8000);
  }
});

test('an events.jsonl that is a link out or a pipe gives no step, and no byte of the file outside reaches the answer', async () => {
  for (const k of ['ev-link', 'ev-fifo']) {
    const {text, liveness} = await answer(k);
    expect(liveness.state, k).toBe('between');
    expect(liveness.step, k).toBeNull();
    expect(text, k).not.toContain('OUTSIDE-');
  }
});

test('a line of 50 MB at the end of events.jsonl is bounded: the answer comes at once and the server lives on', async () => {
  const t0 = Date.now();
  const {liveness} = await answer('ev-50mb');
  expect(['running', 'between']).toContain(liveness.state);
  expect(Date.now() - t0).toBeLessThan(8000);
  expect((await answer('ev-50mb')).liveness.runner.pid).toBe(process.pid);
});

test('of an events.jsonl of 2 MB the last start is the one that is read', async () => {
  const {liveness} = await answer('ev-big');
  expect(liveness.state).toBe('running');
  expect(liveness.step.action).toBe('new.action');
});

test('a process whose name holds spaces, brackets and a closing parenthesis is read right, and its name is in no answer', async () => {
  const {text, liveness} = await answer('odd-name');
  expect(liveness.state).toBe('running');
  expect(text).not.toContain(') S 1 2 3');
  expect(text).not.toContain('(]');
});

test('the inspector reads /proc only as /proc/<digits>/stat', () => {
  const src = fs.readFileSync(path.join(ui, 'runnerlive.mjs'), 'utf8');
  const uses = src.split('\n').filter((l) => !l.trim().startsWith('//') && l.includes('/proc'));
  expect(uses.map((l) => l.trim())).toEqual(["try { stat = fs.readFileSync(`/proc/${n}/stat`, 'utf8'); } catch (e) { return e?.code === 'ENOENT' || e?.code === 'ESRCH' ? {gone: true} : {error: /^[A-Z0-9_]{1,24}$/.test(e?.code) ? e.code : 'error'}; }"]);
});

test('a runner that was killed and not yet waited for by its parent (a zombie: the pid and the start time are still there) is not live', async () => {
  const {liveness} = await answer('zombie');
  expect(liveness.state, 'a zombie has no code to run').toBe('not_live');
});

test('a runner.json of 4 MB, a thousand times the record, is not read as the record', async () => {
  const {liveness} = await answer('rec-4mb');
  expect(liveness.state, 'a record is about 150 bytes; the read is bounded to what fits a record').toBe('unknown');
});

test('the action, the case and the source of a start line are cut to a named length in the answer, and a cut never splits a character', async () => {
  const {text, liveness} = await answer('long-action');
  for (const k of ['action', 'case', 'source']) {
    expect(liveness.step[k].length, `${k}: the answer carries the 300000 characters of the line`).toBeLessThanOrEqual(Live.TEXT_CUT);
    expect(liveness.step[k].length, `${k}: the start of the text is kept`).toBeGreaterThan(0);
  }
  expect(text.length).toBeLessThan(4096);
  const wide = (await answer('wide-action')).liveness.step.action;
  expect(wide.isWellFormed(), 'a cut inside a pair of surrogates').toBe(true);
  expect(wide).toMatch(/^(\u{1F600})+$/u);
  expect([...wide].length).toBeLessThanOrEqual(Live.TEXT_CUT);
});

test('a kernel that does not answer for the recorded process is not told as a process that is gone, and a start time of 0 or less is no start time', async () => {
  const denied = Live.runnerLiveness(dirOf('in-step'), () => ({error: 'EACCES'}));
  expect(denied.state).toBe('unknown');
  expect(denied.why).toContain('EACCES');
  expect(denied.why).not.toContain('no process');
  expect(denied.step.action).toBe('notes.write');
  expect(Live.runnerLiveness(dirOf('in-step'), () => ({gone: true})).why).toContain('no process');
  for (const k of ['zero', 'negative']) {
    const l = Live.runnerLiveness(dirOf(`ticks-${k}`), () => ({ticks: Number(k === 'zero' ? 0 : -1), state: 'S'}));
    expect(l.state, `start_ticks ${k} matches a process of start time ${k}`).toBe('not_live');
  }
});
