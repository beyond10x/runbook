// Two sources with two different views on one write server. One act waits inside its tool (a yq
// that holds until released) while a request for the other source arrives. The act then goes on
// under its own view: its audit record and its approval name the action of its own view, not
// what the other source's view (which has none) would give. Own servers on ports the kernel gives;
// the driver and canon are stubs that exit 0; needs yq on the PATH.
import {test, expect} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeSecondRunbook, SECOND_VIEW} from './second-runbook.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'view-race');
let hasYq = true;
let realYq = '';
try { realYq = execFileSync('which', ['yq'], {encoding: 'utf8'}).trim(); } catch { hasYq = false; }
let srv, base;

const exe = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, text, {mode: 0o755}); };

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide' || !hasYq, 'own server, one viewport; yq is on the PATH'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide' || !hasYq) return;
  fs.rmSync(root, {recursive: true, force: true});
  const storeA = writeSecondRunbook(path.join(root, 'a'));
  const storeB = writeSecondRunbook(path.join(root, 'b'));
  for (const id of fs.readdirSync(storeB)) {
    const f = path.join(storeB, id, 'case.json');
    fs.writeFileSync(f, fs.readFileSync(f, 'utf8').replace('triage.ticket', 'triage.other'));
  }
  const viewA = path.join(root, 'a.view.json');
  fs.writeFileSync(viewA, JSON.stringify({...SECOND_VIEW, tool_owners: {always: ['yq']}}));
  const viewB = path.join(root, 'b.view.json');
  fs.writeFileSync(viewB, JSON.stringify({format: 'runbook.view/1', runbook: 'other', protocols: ['^triage\\.other$'],
    phases: {stages: ['Other'], items: [{id: 'o', stage: 'Other', label: 'Other', protocol: 'triage.other', claims: ['ticket.read']}]}}));
  const bin = path.join(root, 'bin');
  exe(path.join(bin, 'drive'), '#!/bin/sh\nexit 0\n');
  exe(path.join(bin, 'canon'), '#!/bin/sh\nexit 0\n');
  exe(path.join(bin, 'yq'), `#!/bin/sh\nif [ "$1" = -i ]; then touch "${root}/started"; while [ ! -e "${root}/go" ]; do sleep 0.05; done; fi\nexec "${realYq}" "$@"\n`);
  ({url: base, proc: srv} = await startServer(['--view', viewA, '--view', viewB, '--write', '--principal', 'owner-1', storeA, storeB],
    {env: {...process.env, PATH: `${bin}:${process.env.PATH}`, SDC_UI_DRIVE: path.join(bin, 'drive'), SDC_UI_CANON: path.join(bin, 'canon')}}));
});

test('an act that waits in its tool keeps its own view while a request for another source arrives', async ({request}) => {
  const sources = await (await request.get(`${base}/api/sources`)).json();
  expect(sources.map((s) => s.view.runbook)).toEqual(['triage', 'other']);
  const ib = await (await request.get(`${base}/api/inbox?source=0`)).json();
  const a1 = ib.items.find((x) => x.qid === 'a1');
  expect(a1).toMatchObject({kind: 'question', case: 'T-2'});
  const act = request.post(`${base}/api/act?source=0`, {headers: {'x-sdc-ui': '1'}, data: {kind: 'answer', rel: 'T-2', qid: 'a1', sha: a1.sha, answer: 'queue-b', revision: 'r1'}});
  await expect.poll(() => fs.existsSync(path.join(root, 'started'))).toBe(true);
  const other = await request.get(`${base}/api/inbox?source=1`);
  expect(other.status()).toBe(200);
  expect((await (await request.get(`${base}/api/view-info?source=1`)).json()).runbook).toBe('other');
  fs.writeFileSync(path.join(root, 'go'), '');
  const out = await (await act).json();
  expect(out).toMatchObject({ok: true, approved: true, answered_by: 'owner-1'});
  const audit = fs.readFileSync(path.join(root, 'a', 'ui-actions.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
  expect(audit.find((x) => x.action === 'answer')).toMatchObject({target: 'a1', answered_by: 'owner-1 (written by ticket.ask)'});
  expect(audit.find((x) => x.action === 'approve')).toMatchObject({target: 'ticket.ask', capability: 'ticket.ask', exit: 0});
  expect(audit.filter((x) => x.refused)).toEqual([]);
});
