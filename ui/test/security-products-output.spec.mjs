// Independent review, invariant 8 (and 2, 3 for the output route) of story inspector-shows-step-products: the page
// reads Loom's run log under store/ to find an action's output blob. That read goes through the run-path check; a
// digest from a hand-written log is never a path component unless it is 64 lowercase hex digits; the blob is read as
// a regular file only; a link that leaves the run, a pipe and a log that is a link are refused without a hang.
// Own server, run written by hand (products-fixture.mjs), invented text.
import {test, expect} from '@playwright/test';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT, TOKEN_TEXT, SECRET_TEXT} from './products-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-output');
const hex = (s) => Buffer.from(s).toString('hex');
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
const D = {link: sha('link'), fifo: sha('fifo'), toDot: sha('to-dot'), toOutside: sha('to-outside'), toLog: sha('to-log')};
let srv, base;

const rewrite = (file, f) => {
  const rows = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
  fs.writeFileSync(file, f(rows).map((r) => JSON.stringify(r)).join('\n') + '\n');
};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const outside = path.join(root, 'outside');
  put(path.join(outside, 'secret.txt'), `${SECRET_TEXT}\n`);
  // run h: a run log and events written by hand with hostile digests, one per action
  const h = path.join(root, 'runs', 'h');
  writeProductsRun(h);
  const store = path.join(h, 'store');
  const blobs = path.join(store, 'blobs');
  fs.symlinkSync(path.join(outside, 'secret.txt'), path.join(blobs, D.toOutside));
  fs.symlinkSync(path.join(h, 'work', '.cache', 'token.txt'), path.join(blobs, D.toDot));
  const mk = spawnSync('mkfifo', [path.join(blobs, D.fifo)]);
  if (mk.status !== 0) throw new Error(`mkfifo: ${mk.stderr}`);
  const evs = path.join(store, ROOT, 'events.jsonl');
  // the case store's own `log` member: a path, a path of 64 characters, a pipe, a link outside the run
  const logs = {'notes.write': `../../../outside/secret.txt`, 'notes.check': `${'../'.repeat(21)}x`, 'report.check': D.toOutside};
  fs.mkdirSync(path.join(store, ROOT, 'blobs'));
  fs.symlinkSync(path.join(outside, 'secret.txt'), path.join(store, ROOT, 'blobs', D.toOutside)); // the case store's own blobs/
  rewrite(evs, (rows) => rows.map((r) => (r.event === 'action_finished' && logs[r.action] ? {...r, log: logs[r.action]} : r)));
  // Loom's run log: the digest of the actions that name none in the events
  const rl = path.join(store, `runlog-case-${hex(ROOT)}.jsonl`);
  const digests = {'report.approve': `../../outside/secret.txt`, 'report.read': D.toDot, 'report.write': D.fifo};
  rewrite(rl, (rows) => rows.map((r) => (r.kind === 'ActionFinished' && digests[r.action] ? {...r, blob_digest: digests[r.action]} : r)));
  // run b: the blobs directory of the run is a link to a directory outside it that holds the same blobs
  const b = path.join(root, 'runs', 'b');
  writeProductsRun(b);
  fs.cpSync(path.join(b, 'store', 'blobs'), path.join(outside, 'blobs'), {recursive: true});
  fs.rmSync(path.join(b, 'store', 'blobs'), {recursive: true});
  fs.symlinkSync(path.join(outside, 'blobs'), path.join(b, 'store', 'blobs'));
  // run l: the run log of the case is a link to a file outside the run
  const l = path.join(root, 'runs', 'l');
  writeProductsRun(l);
  const log = path.join(l, 'store', `runlog-case-${hex(ROOT)}.jsonl`);
  fs.copyFileSync(log, path.join(outside, 'runlog.jsonl'));
  fs.rmSync(log);
  fs.symlinkSync(path.join(outside, 'runlog.jsonl'), log);
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

async function outputs(name) {
  const src = await sourceIndex(base, name);
  const tree = (await ask(base, `/api/tree?source=${src}`)).json;
  const c = tree.cases.find((x) => x.id === ROOT);
  const out = {};
  for (const e of c.events.filter((x) => x.event === 'action_finished')) out[e.action] = await ask(base, `/api/output?source=${src}&case=${encodeURIComponent(c.rel)}&seq=${e.seq}`, {timeout: 5000});
  return out;
}

test('inv 8: a digest that is not 64 hex digits is never a path: `..`, separators and look-alike lengths are answered, not read', async () => {
  const o = await outputs('h');
  for (const [action, r] of Object.entries(o)) {
    expect(r.hung, action).toBe(false);
    expect(r.text, action).not.toContain(SECRET_TEXT);
  }
  expect(o['notes.write'].json).toMatchObject({recorded: false});
  expect(o['notes.write'].json.why).toMatch(/not a SHA-256/);
  expect(o['notes.check'].json.why).toMatch(/not a SHA-256/);
  expect(o['report.approve'].json.why).toMatch(/not a SHA-256/);
});

test('inv 8 and 2: a blob that is a pipe, or a link to a file outside the run, is refused at once', async () => {
  const o = await outputs('h');
  expect(o['report.write'].hung).toBe(false);
  expect(o['report.write'].json).toMatchObject({recorded: false});
  expect(o['report.write'].json.why).toMatch(/not a regular file inside the run/);
  expect(o['report.check'].json).toMatchObject({recorded: false});
  expect(o['report.check'].text).not.toContain(SECRET_TEXT);
});

test('inv 8 and 3: a blob that is a link into a dot-directory of the run is not served, as a file under it is not', async () => {
  const o = await outputs('h');
  expect(o['report.read'].text, 'the blob read through a plain-named link into work/.cache').not.toContain(TOKEN_TEXT);
  expect(o['report.read'].json.recorded).toBe(false);
});

test('inv 8: a blobs directory that is a link leaving the run, and a run log that is one, give no output', async () => {
  const b = await outputs('b');
  for (const [action, r] of Object.entries(b)) { expect(r.hung, action).toBe(false); expect(r.json.recorded, `b ${action}`).toBe(false); }
  const l = await outputs('l');
  for (const [action, r] of Object.entries(l)) { expect(r.hung, action).toBe(false); expect(r.json.recorded, `l ${action}`).toBe(false); }
  const own = await outputs('h');
  expect(own['notes.check'].text).not.toContain(SECRET_TEXT);
});
