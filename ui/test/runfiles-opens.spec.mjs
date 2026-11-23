// Every read of a file of a run goes through the one checked open of runfiles.mjs (story inspector-shows-step-products,
// fix round 3): the descriptor is opened once, its real path lies under the run, the file is regular, has one name and is
// within its bound. Three reads of server.mjs opened a file themselves: the head of a case's events.jsonl (the protocol
// of a case, read when it has no case.json), fs/changes.jsonl read from the offset the page has seen (/api/live), and one
// change of it read by its position (/api/fschange). For each of the three: a second name of a file outside the run and
// a link that leaves the run are not read, and an ordinary file is read as before. Own server, runs written by hand
// (loom-run-fixture.mjs), invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeLoomRun} from './loom-run-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex} from './security-products-lib.mjs';

const root = fixtureRoot('runfiles-opens');
const SECRET = 'OUTSIDE-SECRET-9c1d';
const outside = path.join(root, 'outside');
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });

const change = (file) => JSON.stringify({format: 'sdc.fs-change/1', seq: 1, at: '2026-01-01T00:00:10Z', root: 'work', path: file, op: 'create', from_path: null, before: null, after: 'a'.repeat(64), binary: false, too_large: false,
  lines_added: 1, lines_removed: 0, diff: '--- /dev/null\n+++ b/x\n@@ -0,0 +1,1 @@\n+line\n', attribution: {case: 'CASE-1', action: 'notes.write', record: null, tool: null, basis: 'step-window', step: 1}}) + '\n';
// two paths of the same length: the line of the one is as long as the line of the other
const OWN = `work/${'x'.repeat(40)}`;
const OTHER = `${SECRET}${'y'.repeat(40 - SECRET.length)}`;

const events = (dir) => path.join(dir, 'store', 'CASE-1', 'events.jsonl');
const changes = (dir) => path.join(dir, 'fs', 'changes.jsonl');
const replace = (file, how, target) => { fs.rmSync(file, {force: true}); (how === 'hard' ? fs.linkSync : fs.symlinkSync)(target, file); };

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  put(path.join(outside, 'events.jsonl'), JSON.stringify({event: 'opened', at: '2026-01-01T00:00:00Z', protocol: {id: 'leaked-protocol', revision: 1}, seq: 0, source: 'loom-runlog'}) + '\n');
  put(path.join(outside, 'changes.jsonl'), change(`work/${SECRET}.txt`));
  put(path.join(outside, 'same-size.jsonl'), change(OTHER));
  writeLoomRun(path.join(runs, 'ordinary'), {watch: 'changes'});
  for (const how of ['hard', 'link']) {
    const head = writeLoomRun(path.join(runs, `head-${how}`));
    replace(events(head), how, path.join(outside, 'events.jsonl'));
    const off = writeLoomRun(path.join(runs, `offset-${how}`), {watch: 'empty'});
    replace(changes(off), how, path.join(outside, 'changes.jsonl'));
    const pos = writeLoomRun(path.join(runs, `position-${how}`), {watch: 'empty'});
    put(changes(pos), change(OWN));
  }
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
});

const src = (name) => sourceIndex(base, name);

test('control: an ordinary events.jsonl names the protocol of its case, an ordinary changes.jsonl is listed and one change is read by its position', async () => {
  const i = await src('ordinary');
  const info = await ask(base, `/api/view-info?source=${i}`);
  expect(info.json.protocols).toEqual(['example']);
  const live = await ask(base, `/api/live?source=${i}`);
  expect(live.json.fs.present).toBe(true);
  expect(live.json.fs.changes.map((c) => c.path)).toEqual(['notes.txt', 'report.md']);
  const one = await ask(base, `/api/fschange?source=${i}&seq=1`);
  expect(one.status).toBe(200);
  expect(one.json.path).toBe('notes.txt');
});

test('control: a changes.jsonl that is one name of an own file is read by position', async () => {
  const i = await src('position-hard');
  const live = await ask(base, `/api/live?source=${i}`);
  expect(live.json.fs.changes.map((c) => c.path)).toEqual([OWN]);
  const one = await ask(base, `/api/fschange?source=${i}&seq=1`);
  expect(one.status).toBe(200);
  expect(one.json.path).toBe(OWN);
});

for (const how of ['hard', 'link']) {
  const what = how === 'hard' ? 'a second name of a file outside the run' : 'a link that leaves the run';

  test(`the head of events.jsonl that is ${what} does not name the protocol of the case`, async () => {
    const i = await src(`head-${how}`);
    const info = await ask(base, `/api/view-info?source=${i}`);
    expect(info.status).toBe(200);
    expect(JSON.stringify(info.json)).not.toContain('leaked-protocol');
  });

  test(`a changes.jsonl that is ${what} is not read from the offset`, async () => {
    const i = await src(`offset-${how}`);
    const live = await ask(base, `/api/live?source=${i}`);
    expect(live.status).toBe(200);
    expect(live.text).not.toContain(SECRET);
  });

  test(`a changes.jsonl that becomes ${what} after it was listed is not read by position`, async () => {
    const i = await src(`position-${how}`);
    const listed = await ask(base, `/api/live?source=${i}`);
    expect(listed.json.fs.changes.map((c) => c.path)).toEqual([OWN]);
    replace(changes(path.join(root, 'runs', `position-${how}`)), how, path.join(outside, 'same-size.jsonl'));
    const one = await ask(base, `/api/fschange?source=${i}&seq=1`);
    expect(one.text).not.toContain(SECRET);
    expect(one.status).toBe(404);
  });
}
