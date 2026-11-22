// A view file applies to a run of `runbook run`. That run's store (docs/loom-run-directory.md) has,
// for each case, events.jsonl whose first event is `opened` with the protocol, and no case.json; the
// protocol of the run is read from there. A case that has both keeps the protocol of case.json.
// Own server on a port the kernel gives (test/serve.mjs); the store is written by hand with invented
// text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'view-loom-store');
const PROTOCOL = 'example@1';
const VIEW = {
  format: 'runbook.view/1',
  runbook: 'example',
  protocols: ['^example@1$'],
  phases: {stages: ['Work'], items: [{id: 'do', stage: 'Work', label: 'Do', protocol: PROTOCOL, claims: ['work.done']}]},
};
const jl = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + '\n';
const opened = (id, revision = 1) => ({at: '2026-11-21T10:00:00Z', event: 'opened', protocol: {id, revision}, seq: 0});
const write = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, text); };
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const loom = path.join(root, 'loom-run', 'store');
  write(path.join(loom, 'CASE-1', 'events.jsonl'), jl([opened(PROTOCOL), {at: '2026-11-21T10:00:01Z', event: 'terminated', outcome: 'done', seq: 1}]));
  write(path.join(loom, 'CASE-1', 'children', 'CASE-2', 'events.jsonl'), jl([opened(PROTOCOL)]));
  const both = path.join(root, 'both-run', 'store');
  write(path.join(both, 'CASE-3', 'case.json'), JSON.stringify({format: 'canon-case/1', id: 'CASE-3', protocol: PROTOCOL, revision: 'c1'}));
  write(path.join(both, 'CASE-3', 'events.jsonl'), jl([opened('other@1')]));
  const viewFile = path.join(root, 'example.view.json');
  write(viewFile, JSON.stringify(VIEW, null, 2));
  ({url: base, proc: srv} = await startServer(['--view', viewFile, loom, both]));
});

test('a run of the runner (events.jsonl with an opened event, no case.json) gets the view that covers its protocol', async ({request}) => {
  expect(await (await request.get(`${base}/api/view-info?source=0`)).json()).toMatchObject({protocols: [PROTOCOL], missing: false, runbook: 'example'});
  expect((await (await request.get(`${base}/api/view?source=0`)).json()).runbook).toBe('example');
  const sources = await (await request.get(`${base}/api/sources`)).json();
  expect(sources[0].view).toMatchObject({protocols: [PROTOCOL], runbook: 'example'});
});

test('a case that has case.json and an opened event keeps the protocol of case.json', async ({request}) => {
  expect(await (await request.get(`${base}/api/view-info?source=1`)).json()).toMatchObject({protocols: [PROTOCOL], missing: false, runbook: 'example'});
});
