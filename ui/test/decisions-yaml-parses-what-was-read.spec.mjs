// An artifact of a case is parsed from the text that was read through the checked open, never from a second open by name:
// an artifact that flips between an own file and a link to a file outside the run while amendStall is called for 8 s is
// never counted from the file outside the run. In-process, invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawn, spawnSync} from 'node:child_process';
import {amendStall} from '../decisions.mjs';
import {registerRun} from '../runfiles.mjs';
import {useSdcView} from './sdc-view.mjs';
import {fixtureRoot} from './security-products-lib.mjs';
useSdcView();

const root = fixtureRoot('yq-race');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

test('an artifact swapped for a link out of the run after its check is not parsed (bounded: 8 s)', async () => {
  test.setTimeout(30000);
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
  const child = spawn('node', [flip, file, own, outside, '10000'], {stdio: 'ignore'});
  let calls = 0, fromOutside = 0;
  const end = Date.now() + 8000;
  while (Date.now() < end) { const r = amendStall(c, run); calls++; if (r?.needsPerson === 2) fromOutside++; }
  child.kill();
  expect(calls).toBeGreaterThan(0);
  expect(fromOutside).toBe(0);
});

// Deterministic: `yq` is found on PATH, so a wrapper in front of it records every argument list it is started with and then
// runs the real one. No argument is a path of the run: the text goes in on standard input, and the answer is the parse of it.
test('yq is started with no path of a run as an argument, and the text it parses is the text that was read', () => {
  const real = spawnSync('sh', ['-c', 'command -v yq'], {encoding: 'utf8'}).stdout.trim();
  test.skip(!real, 'yq is not on the PATH');
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  const dir = path.join(run, 'store', 'IDEA-1.todo.plan');
  fs.mkdirSync(path.join(dir, 'work'), {recursive: true});
  registerRun(run);
  fs.writeFileSync(path.join(dir, 'work', 'findings.yaml'), 'findings:\n- id: f1\n  status: needs_person\n- id: f2\n  status: needs_person\n');
  const bin = path.join(root, 'bin'), log = path.join(root, 'yq-args.log');
  fs.mkdirSync(bin, {recursive: true});
  fs.writeFileSync(path.join(bin, 'yq'), `#!/bin/sh\nprintf '%s\\n' "$*" >> '${log}'\nexec '${real}' "$@"\n`, {mode: 0o755});
  const bindings = {$artifacts: {findings: 'findings.yaml'}, 'amend.upstream': {argv: ['x']}};
  const events = [{seq: 0, event: 'opened'}, {seq: 1, event: 'awaiting_person', next: [{action: 'amend.upstream', command: 'a', decides: 'd'}]}];
  const c = {id: 'IDEA-1.todo.plan', rel: 'IDEA-1.todo.plan', dir, bindings, events};
  const before = process.env.PATH;
  process.env.PATH = `${bin}${path.delimiter}${before}`;
  let result;
  try { result = amendStall(c, run); } finally { process.env.PATH = before; }
  expect(result.needsPerson).toBe(2);
  const calls = fs.readFileSync(log, 'utf8').split('\n').filter(Boolean);
  expect(calls.length).toBeGreaterThan(0);
  for (const argv of calls) {
    expect(argv).not.toContain(root);
    expect(argv).not.toContain('findings.yaml');
  }
});
