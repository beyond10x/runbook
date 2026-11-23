// readRegular with `at` (the position a window starts at: the incremental read of fs/changes.jsonl and the read of one change
// by its position) and the read of the head of a case's events.jsonl, both without a server. `at` is clamped to the size of the
// file, the window is at most `limit` bytes from there, and the checks of the one checked open apply to every window: a second
// name, a link that leaves the run and a descriptor that is not the file that was checked are not read.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {readRegular, registerRun} from '../runfiles.mjs';
import {openedProtocol, HEAD_BYTES} from '../runfacts.mjs';
import {fixtureRoot, put} from './security-products-lib.mjs';

const root = fixtureRoot('runfiles-at');
const run = path.join(root, 'run');
const MIB = 1048576;
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'no server: one project is enough'));
test.beforeAll(({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  registerRun(run);
  put(path.join(run, 'ten.txt'), '0123456789');
  put(path.join(root, 'outside.txt'), 'outside the run\n');
  const big = path.join(run, 'big.bin');
  fs.mkdirSync(run, {recursive: true});
  fs.writeFileSync(big, '');
  fs.truncateSync(big, 40 * MIB);
});
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

const read = (name, o) => readRegular(path.join(run, name), {root: run, ...o});
const text = (r) => r.bytes.toString('utf8');

test('a window starts at `at` and is at most `limit` bytes', () => {
  expect(text(read('ten.txt', {at: 3, limit: 4}))).toBe('3456');
  expect(text(read('ten.txt', {at: 0, limit: 100}))).toBe('0123456789');
  expect(text(read('ten.txt', {at: 7, limit: 100}))).toBe('789');
  const w = read('ten.txt', {at: 3, limit: 4});
  expect(w).toMatchObject({state: 'ok', size: 10, from: 3, to: 7});
});

test('`at` at or past the end gives an empty window of the file, not an error', () => {
  for (const at of [10, 11, 1000, Number.MAX_SAFE_INTEGER]) {
    const r = read('ten.txt', {at, limit: 4});
    expect(r, `at ${at}`).toMatchObject({state: 'ok', size: 10});
    expect(r.bytes.length, `at ${at}`).toBe(0);
  }
});

test('a negative, fractional or non-numeric `at` reads nothing', () => {
  for (const at of [-1, -1000, 0.5, 2.5, NaN]) {
    const r = read('ten.txt', {at, limit: 4});
    expect(r.state, `at ${at}`).not.toBe('ok');
    expect(r.bytes, `at ${at}`).toBeUndefined();
  }
});

test('a file over 32 MiB is read at most `limit` bytes from `at`, never to its end', () => {
  const a = read('big.bin', {at: 0, limit: 32 * MIB});
  expect(a).toMatchObject({state: 'ok', size: 40 * MIB, from: 0, to: 32 * MIB});
  expect(a.bytes.length).toBe(32 * MIB);
  const b = read('big.bin', {at: 32 * MIB, limit: 32 * MIB});
  expect(b).toMatchObject({state: 'ok', from: 32 * MIB, to: 40 * MIB});
  expect(b.bytes.length).toBe(8 * MIB);
  const c = read('big.bin', {at: 5 * MIB, limit: 1000});
  expect(c.bytes.length).toBe(1000);
});

test('a window of a second name of a file outside the run, of a link that leaves the run, and of a descriptor that is another file is not read', () => {
  fs.linkSync(path.join(root, 'outside.txt'), path.join(run, 'second-name.txt'));
  fs.symlinkSync(path.join(root, 'outside.txt'), path.join(run, 'link-out.txt'));
  for (const name of ['second-name.txt', 'link-out.txt']) {
    const r = read(name, {at: 0, limit: 100});
    expect(r.state, name).not.toBe('ok');
    expect(r.bytes, name).toBeUndefined();
  }
  const other = read('ten.txt', {at: 0, limit: 100, nameOf: () => path.join(run, 'big.bin')});
  expect(other.state).not.toBe('ok');
  expect(other.bytes).toBeUndefined();
  const blind = read('ten.txt', {at: 0, limit: 100, nameOf: () => null});
  expect(blind).toMatchObject({state: 'refused', why: 'unverified'});
});

test('the head of events.jsonl names the protocol of the case; a second name of a file outside the run and a link that leaves the run name none', () => {
  const opened = (id) => JSON.stringify({event: 'opened', protocol: {id}, seq: 0}) + '\n';
  const caseDir = (name) => path.join(run, 'cases', name);
  put(path.join(caseDir('own'), 'events.jsonl'), opened('own-protocol'));
  expect(openedProtocol(caseDir('own'))).toBe('own-protocol');
  put(path.join(root, 'outside-events.jsonl'), opened('outside-protocol'));
  fs.mkdirSync(caseDir('hard'), {recursive: true});
  fs.linkSync(path.join(root, 'outside-events.jsonl'), path.join(caseDir('hard'), 'events.jsonl'));
  expect(openedProtocol(caseDir('hard'))).toBeNull();
  fs.mkdirSync(caseDir('link'), {recursive: true});
  fs.symlinkSync(path.join(root, 'outside-events.jsonl'), path.join(caseDir('link'), 'events.jsonl'));
  expect(openedProtocol(caseDir('link'))).toBeNull();
});

test('only the first 64 KiB of events.jsonl is read, and a line cut by that bound is not parsed', () => {
  const dir = path.join(run, 'cases', 'long');
  const filler = JSON.stringify({event: 'noise', pad: 'x'.repeat(1000)}) + '\n';
  const late = JSON.stringify({event: 'opened', protocol: {id: 'late-protocol'}}) + '\n';
  put(path.join(dir, 'events.jsonl'), filler.repeat(Math.ceil(HEAD_BYTES / filler.length) + 1) + late);
  expect(openedProtocol(dir)).toBeNull();
});
