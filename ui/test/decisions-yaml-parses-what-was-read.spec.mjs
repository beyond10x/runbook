// An artifact of a case is parsed from the text that was read through the checked open, never from a second open by name:
// an artifact that flips between an own file and a link to a file outside the run while amendStall is called for 15 s is
// never counted from the file outside the run. In-process, invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {amendStall} from '../decisions.mjs';
import {registerRun} from '../runfiles.mjs';
import {useSdcView} from './sdc-view.mjs';
import {fixtureRoot} from './security-products-lib.mjs';
useSdcView();

const root = fixtureRoot('yq-race');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

test('an artifact swapped for a link out of the run after its check is not parsed', async () => {
  test.setTimeout(55000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  const dir = path.join(run, 'store', 'IDEA-1.todo.plan');
  fs.mkdirSync(path.join(dir, 'work'), {recursive: true});
  registerRun(run);
  const file = path.join(dir, 'work', 'findings.yaml'), own = path.join(root, 'own.yaml'), outside = path.join(root, 'outside.yaml');
  fs.writeFileSync(own, 'findings:\n- id: f1\n  status: resolved\n');
  fs.writeFileSync(outside, 'findings:\n- id: f1\n  status: needs_person\n- id: f2\n  status: needs_person\n');
  fs.copyFileSync(own, file);
  const flip = path.join(root, 'flip.mjs');
  fs.writeFileSync(flip, `import fs from 'node:fs';
const [f, own, outside, ms] = process.argv.slice(2);
const end = Date.now() + Number(ms);
let i = 0;
while (Date.now() < end) { try { fs.rmSync(f, {force: true}); if (i++ % 2) fs.symlinkSync(outside, f); else fs.copyFileSync(own, f); } catch {} }
`);
  const bindings = {$artifacts: {findings: 'findings.yaml'}, 'amend.upstream': {argv: ['x']}};
  const events = [{seq: 0, event: 'opened'}, {seq: 1, event: 'awaiting_person', next: [{action: 'amend.upstream', command: 'a', decides: 'd'}]}];
  const c = {id: 'IDEA-1.todo.plan', rel: 'IDEA-1.todo.plan', dir, bindings, events};
  const child = spawn('node', [flip, file, own, outside, '20000'], {stdio: 'ignore'});
  let calls = 0, fromOutside = 0;
  const end = Date.now() + 15000;
  while (Date.now() < end) { const r = amendStall(c, run); calls++; if (r?.needsPerson === 2) fromOutside++; }
  child.kill();
  expect(calls).toBeGreaterThan(0);
  expect(fromOutside).toBe(0);
});
