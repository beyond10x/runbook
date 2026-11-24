// An artifact of a case is parsed from the text that was read through the checked open, never from a second open by name:
// an artifact that flips between an own file and a link to a file outside the run while amendStall reads it is never
// counted from the file outside the run. In-process, invented text.
// What the first case proves, and how: the flip of a child process (remove the file, then make it a link out of the run or
// copy the own file back, in turn) is made here right before each call amendStall makes to the file system or to `yq`,
// from every point of that cycle, and before every pair of such calls (interleave.mjs), with the read cache empty and,
// for one flip, filled by a read of the own file; every window between two calls is tried, the one between the check and
// the open and the one between the read and the parse among them. Where the child process hit those windows by chance for
// 8 s, each is hit here on purpose in every run; and a case where the read reached the outside file must be among them.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {amendStall} from '../decisions.mjs';
import {registerRun, cacheClear} from '../runfiles.mjs';
import {places, before, sets} from './interleave.mjs';
import {useSdcView} from './sdc-view.mjs';
import {fixtureRoot} from './security-products-lib.mjs';
useSdcView();

const root = fixtureRoot('yq-race');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

test('an artifact swapped for a link out of the run after its check is not parsed, at every call of the reader', async () => {
  test.setTimeout(30000);
  fs.rmSync(root, {recursive: true, force: true});
  const run = path.join(root, 'run');
  const dir = path.join(run, 'store', 'IDEA-1.todo.plan');
  fs.mkdirSync(path.join(dir, 'work'), {recursive: true});
  registerRun(run);
  const file = path.join(dir, 'work', 'findings.yaml'), own = path.join(root, 'own.yaml'), outside = path.join(root, 'outside.yaml');
  fs.writeFileSync(own, 'findings:\n- id: f1\n  status: resolved\n');
  fs.writeFileSync(outside, 'findings:\n- id: f1\n  status: needs_person\n- id: f2\n  status: needs_person\n');
  const bindings = {$artifacts: {findings: 'findings.yaml'}, 'amend.upstream': {argv: ['x']}};
  const events = [{seq: 0, event: 'opened'}, {seq: 1, event: 'awaiting_person', next: [{action: 'amend.upstream', command: 'a', decides: 'd'}]}];
  const c = {id: 'IDEA-1.todo.plan', rel: 'IDEA-1.todo.plan', dir, bindings, events};
  // the cycle of the flip: the own file, gone, a link out of the run, gone, the own file again
  const CYCLE = ['own', 'gone', 'link', 'gone'];
  let at = 0;
  const put = (i) => {
    at = i % CYCLE.length;
    fs.rmSync(file, {force: true});
    if (CYCLE[at] === 'own') fs.copyFileSync(own, file);
    if (CYCLE[at] === 'link') fs.symlinkSync(outside, file);
  };
  const flip = () => put(at + 1);
  const read = () => amendStall(c, run);
  put(0);
  cacheClear();
  const plain = places(read);
  expect(plain.value?.needsPerson, 'the own file, untouched').toBe(0);
  expect(plain.names, 'the reader checks, opens and parses').toEqual(expect.arrayContaining(['realpathSync.native', 'openSync', 'execFileSync']));
  put(2);
  expect(read()?.needsPerson, 'the link out of the run, untouched: no record of it is counted').toBe(0);
  const n = plain.names.length + 2;
  let tried = 0, fromOutside = 0, outsideOpened = 0;
  for (let start = 0; start < CYCLE.length; start++) {
    for (const set of sets(n, 2)) {
      for (const warm of set.size === 1 ? [false, true] : [false]) {
        put(0);
        cacheClear();
        if (warm) read();
        put(start);
        let linkOpened = false;
        const r = before(set, flip, read, (name) => { if (name === 'openSync' && CYCLE[at] === 'link') linkOpened = true; });
        tried++;
        if (linkOpened) outsideOpened++;
        if (r?.needsPerson === 2) fromOutside++;
      }
    }
  }
  console.log(`yq race: ${tried} interleavings of ${n} places from ${CYCLE.length} points of the flip, ${outsideOpened} that opened while the link out of the run was in place, counted from outside: ${fromOutside}`);
  expect(outsideOpened, 'interleavings that open the artifact while it is the link out of the run were tried').toBeGreaterThan(0);
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
