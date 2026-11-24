// Two sources with two different views on one server. Requests for both arrive at once, many times over:
// each answer is read under the view of its own source (the inbox of the source with a view that names
// the ask, the view-info of the other one), never under the view of the one asked just before. Own
// server on a port the kernel gives; the server only reads.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {writeSecondRunbook, SECOND_VIEW} from './second-runbook.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(FIXTURE_DIR, 'view-race');
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const storeA = writeSecondRunbook(path.join(root, 'a'));
  const storeB = writeSecondRunbook(path.join(root, 'b'));
  for (const id of fs.readdirSync(storeB)) {
    const f = path.join(storeB, id, 'case.json');
    fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace('triage.ticket', 'triage.other'));
  }
  const viewA = path.join(root, 'a.view.json');
  fs.writeFileSync(viewA, JSON.stringify(SECOND_VIEW));
  const viewB = path.join(root, 'b.view.json');
  fs.writeFileSync(viewB, JSON.stringify({format: 'runbook.view/1', runbook: 'other', protocols: ['^triage\\.other$'],
    phases: {stages: ['Other'], items: [{id: 'o', stage: 'Other', label: 'Other', protocol: 'triage.other', claims: ['ticket.read']}]}}));
  ({url: base, proc: srv} = await startServer(['--view', viewA, '--view', viewB, storeA, storeB]));
});

test('concurrent requests for two sources each keep their own view, and nothing the server serves can write', async ({request}) => {
  const sources = await (await request.get(`${base}/api/sources`)).json();
  expect(sources.map((s) => s.view.runbook)).toEqual(['triage', 'other']);
  const rounds = await Promise.all(Array.from({length: 12}, async (_, i) => {
    const [a, b] = await Promise.all([request.get(`${base}/api/view-info?source=${i % 2}`), request.get(`${base}/api/view-info?source=${1 - (i % 2)}`)]);
    return [i % 2, (await a.json()).runbook, 1 - (i % 2), (await b.json()).runbook];
  }));
  for (const [sa, ra, sb, rb] of rounds) {
    expect(ra).toBe(['triage', 'other'][sa]);
    expect(rb).toBe(['triage', 'other'][sb]);
  }
  const ib = await (await request.get(`${base}/api/inbox?source=0`)).json();
  expect(ib.items.find((x) => x.qid === 'a1')).toMatchObject({kind: 'question', case: 'T-2'});
  const post = await request.post(`${base}/api/act?source=0`, {headers: {'x-sdc-ui': '1'}, data: {kind: 'answer', rel: 'T-2', qid: 'a1', answer: 'queue-b'}});
  expect(post.status()).toBe(405);
  expect(fs.existsSync(path.join(root, 'a', 'ui-actions.jsonl'))).toBe(false);
});
