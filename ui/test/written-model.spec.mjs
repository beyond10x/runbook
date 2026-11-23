// The listing of what a run wrote, as a model (story products-are-what-the-run-wrote): ui/written.mjs on run directories
// written by test/written-fixture.mjs, no server and no browser. The products are the files the run's records name as
// written or changed by an action; a seeded tree is an input, told by the hash of the seed record and never by a name.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {readSeeded, writtenOf, finishedSteps, cleanRel, MAX_LINES, MAX_PATH, MAX_ACTION} from '../written.mjs';
import {productOf} from '../products.mjs';
import {writeWrittenRun, change, sha, WRITTEN} from './written-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'written-model');
let n = 0;
const fresh = (opts) => {
  const dir = path.join(root, `r${n++}`);
  fs.rmSync(dir, {recursive: true, force: true});
  writeWrittenRun(dir, opts);
  return dir;
};
const read = (dir) => {
  const seeded = readSeeded(dir);
  const file = path.join(dir, 'fs', 'changes.jsonl');
  const changes = fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : [];
  return {seeded, w: seeded.state === 'ok' ? writtenOf({runDir: dir, roots: [path.join(dir, 'work')], seeded, changes, finished: finishedSteps(dir)}) : null};
};
const paths = (w) => w.files.map((f) => f.path);

test.beforeAll(() => fs.rmSync(root, {recursive: true, force: true}));

test('a seeded tree of 200 files, three steps and five written files: five products in three groups, one Inputs line, the last written file first', () => {
  const dir = fresh();
  const {seeded, w} = read(dir);
  expect(seeded.state).toBe('ok');
  expect(w.total).toBe(5);
  expect(w.groups.map((g) => [g.step, g.files.map((f) => f.path)])).toEqual([
    [3, ['work/impl/src/m7.rs', 'work/repro-result.txt']],
    [2, ['work/dissection.md', 'work/impl/tests/repro.rs']],
    [1, ['work/aep/story.md']],
  ]);
  expect(w.groups.map((g) => g.actions)).toEqual([['change.fix'], ['bug.dissect', 'test.reproduce'], []]);
  expect(Object.fromEntries(w.files.map((f) => [f.path, f.mark]))).toEqual({
    'work/impl/src/m7.rs': 'changed', 'work/repro-result.txt': null, 'work/dissection.md': null, 'work/impl/tests/repro.rs': 'added', 'work/aep/story.md': null,
  });
  expect(w.inputs).toEqual({files: 201, trees: [{path: 'idea.yaml', kind: 'file', files: 1}, {path: 'impl', kind: 'dir', files: 100}, {path: 'mainline', kind: 'dir', files: 100}]});
  expect(w.last.path).toBe('work/repro-result.txt');
  expect(w.last.step).toBe(3);
  expect(paths(w).some((p) => p.startsWith('work/mainline/'))).toBe(false);
  expect(w.files.find((f) => f.path === 'work/repro-result.txt')).toMatchObject({result: 'red', format: 'text'});
  expect(w.notShown).toBe(0);
});

test('the product opened first is the last file of the last finished step, never a file of an input tree; a step still running does not count', () => {
  const dir = fresh({rounds: 2});
  const {w} = read(dir);
  expect(w.finished).toBe(2);
  expect(w.last.path).toBe('work/impl/tests/repro.rs');
  const a = productOf({runDir: dir, files: w.files, view: null, ids: [], written: {...w, isInput: () => false}});
  expect(a).toMatchObject({via: 'last-written', path: 'work/impl/tests/repro.rs', missing: false});
  expect(a.note).toContain('step 2');
  // the "newest Markdown" rule is not used with records: the seeded README, written last by mtime, is not named
  fs.utimesSync(path.join(dir, 'work/impl/README.md'), new Date(), new Date());
  expect(productOf({runDir: dir, files: w.files, view: null, ids: [], written: {...w, isInput: () => false}}).path).toBe('work/impl/tests/repro.rs');
  const none = fresh({rounds: 0});
  const r = read(none);
  expect(r.w.last).toBeNull();
  expect(productOf({runDir: none, files: r.w.files, view: null, ids: [], written: {...r.w, isInput: () => false}})).toMatchObject({via: null, path: null});
});

test('a view or runbook file that is a seeded file is refused as the product', () => {
  const dir = fresh();
  const {w, seeded} = read(dir);
  const isInput = (p) => seeded.files.has(p) && !w.files.some((f) => f.path === p);
  const a = productOf({runDir: dir, files: w.files, view: {product: 'work/impl/README.md'}, ids: [], written: {...w, isInput}});
  expect(a).toMatchObject({via: 'view', missing: true});
  expect(a.note).toContain('input');
  expect(productOf({runDir: dir, files: w.files, view: {product: 'work/dissection.md'}, ids: [], written: {...w, isInput}})).toMatchObject({via: 'view', path: 'work/dissection.md', missing: false});
});

test('a seeded file that is changed and then restored is an input again, one an action deleted is shown removed, one nobody touched is not listed', () => {
  const seedText = (p) => `// invented module of ${p}\npub fn f() -> u32 { ${p.length} }\n`;
  const same = seedText('work/impl/src/m1.rs');
  const dir = fresh({written: [], extra: [
    change(0, 'impl/src/m1.rs', {op: 'modify', text: 'changed\n', step: 2}),
    change(0, 'impl/src/m1.rs', {op: 'modify', text: same, step: 3}),
    change(0, 'impl/src/m2.rs', {op: 'delete', step: 2}),
    change(0, 'impl/src/m3.rs', {op: 'modify', text: 'changed again\n', step: 2}),
  ]});
  const {w} = read(dir);
  expect(Object.fromEntries(w.files.map((f) => [f.path, f.mark]))).toEqual({'work/impl/src/m2.rs': 'removed', 'work/impl/src/m3.rs': 'changed'});
  expect(w.groups.map((g) => g.step)).toEqual([2]);
  expect(w.last.path).toBe('work/impl/src/m3.rs');
});

test('a rename moves a product: the old path is gone, the new one is listed', () => {
  const dir = fresh({written: [], extra: [
    change(0, 'notes/a.md', {text: 'a\n', step: 2}),
    change(0, 'notes/b.md', {op: 'rename', from: 'notes/a.md', text: 'a\n', step: 3}),
  ]});
  fs.mkdirSync(path.join(dir, 'work/notes'), {recursive: true});
  fs.writeFileSync(path.join(dir, 'work/notes/b.md'), 'a\n');
  const {w} = read(dir);
  expect(paths(w)).toEqual(['work/notes/b.md']);
});

test('a record that names a path outside work/ or outside the run lists nothing for it and prints no path', () => {
  const dir = fresh({extra: [
    change(0, '../../outside-secret.txt', {text: 'x\n', step: 3}),
    change(0, '../store/tree.json', {text: 'x\n', step: 3}),
    change(0, '/etc/hostname', {text: 'x\n', step: 3}),
    change(0, 'a\\b', {text: 'x\n', step: 3}),
    change(0, 'ok/../../up.txt', {text: 'x\n', step: 3}),
    change(0, 'x', {text: 'x\n', step: 3, root: '../outside'}),
    change(0, 'x.txt', {text: 'x\n', step: 3, root: 'store'}),
    change(0, 'x.txt', {text: 'x\n', step: 3, root: 'work/.sdc/llm/001-a/cwd'}),
    change(0, '.cache/token.txt', {text: 'x\n', step: 3}),
    change(0, 'sub/.hidden', {text: 'x\n', step: 3}),
  ]});
  const {w} = read(dir);
  expect(w.total).toBe(5);
  expect(w.refusedRecords).toBe(5);
  const text = JSON.stringify(w);
  for (const bad of ['outside-secret', 'tree.json', 'hostname', 'up.txt', 'token.txt', '.hidden', 'a\\\\b']) expect(text, bad).not.toContain(bad);
});

test('a file a record names that is a link leaving the run is not listed and is counted, and a file that is gone is counted', () => {
  const outside = path.join(root, 'outside-model.txt');
  fs.mkdirSync(root, {recursive: true});
  fs.writeFileSync(outside, 'OUTSIDE-SECRET\n');
  const dir = fresh({extra: [
    change(0, 'leak.txt', {text: 'x\n', step: 3}),
    change(0, 'vanished.txt', {text: 'x\n', step: 3}),
  ]});
  fs.symlinkSync(outside, path.join(dir, 'work/leak.txt'));
  const {w} = read(dir);
  expect(paths(w)).not.toContain('work/leak.txt');
  expect(w.leftOut).toBe(1);
  expect(w.gone).toBe(1);
  expect(JSON.stringify(w)).not.toContain('OUTSIDE-SECRET');
});

test('the products are not cut by depth; a bound on the lines stays and says how many are not shown', () => {
  const deep = Array.from({length: 14}, (_, i) => `d${i}`).join('/');
  const many = Array.from({length: MAX_LINES + 100}, (_, i) => change(0, `bulk/f${String(i).padStart(4, '0')}.txt`, {text: `${i}\n`, step: 3}));
  const dir = fresh({written: [], extra: [change(0, `${deep}/leaf.txt`, {text: 'leaf\n', step: 2}), ...many]});
  fs.mkdirSync(path.join(dir, 'work', deep), {recursive: true});
  fs.writeFileSync(path.join(dir, 'work', deep, 'leaf.txt'), 'leaf\n');
  fs.mkdirSync(path.join(dir, 'work/bulk'), {recursive: true});
  for (let i = 0; i < MAX_LINES + 100; i++) fs.writeFileSync(path.join(dir, `work/bulk/f${String(i).padStart(4, '0')}.txt`), `${i}\n`);
  const {w} = read(dir);
  expect(w.total).toBe(MAX_LINES + 101);
  expect(w.shown).toBe(MAX_LINES);
  expect(w.notShown).toBe(101);
  expect(w.files.length).toBe(MAX_LINES);
  // the deep file is in step 2, behind the 600 of step 3: it is one of those not shown, said by number
  expect(paths(w)).not.toContain(`work/${deep}/leaf.txt`);
  const dir2 = fresh({written: [], extra: [change(0, `${deep}/leaf.txt`, {text: 'leaf\n', step: 2})]});
  fs.mkdirSync(path.join(dir2, 'work', deep), {recursive: true});
  fs.writeFileSync(path.join(dir2, 'work', deep, 'leaf.txt'), 'leaf\n');
  expect(paths(read(dir2).w)).toEqual([`work/${deep}/leaf.txt`]);
});

test('a seed record that is incomplete, wrong or absent gives no records: the caller lists as before and says why', () => {
  expect(readSeeded(fresh({seed: false}))).toMatchObject({state: 'absent'});
  expect(readSeeded(fresh({complete: false}))).toMatchObject({state: 'unusable'});
  const d1 = fresh();
  fs.writeFileSync(path.join(d1, 'seeded-files.jsonl'), fs.readFileSync(path.join(d1, 'seeded-files.jsonl'), 'utf8').split('\n').slice(1).join('\n'));
  expect(readSeeded(d1).why).toContain('do not agree');
  const d2 = fresh();
  fs.writeFileSync(path.join(d2, 'seeded.json'), JSON.stringify({format: 'runbook.seeded/9'}));
  expect(readSeeded(d2).state).toBe('unusable');
  const d3 = fresh();
  fs.appendFileSync(path.join(d3, 'seeded-files.jsonl'), `${JSON.stringify({path: '../../etc/passwd', sha256: sha('x')})}\n`);
  expect(readSeeded(d3).state).toBe('unusable');
  const d4 = fresh();
  fs.rmSync(path.join(d4, 'seeded-files.jsonl'));
  expect(readSeeded(d4).state).toBe('unusable');
  const d5 = fresh();
  const rec = JSON.parse(fs.readFileSync(path.join(d5, 'seeded.json'), 'utf8'));
  rec.entries.push({path: '../escape', kind: 'dir', files: 1});
  fs.writeFileSync(path.join(d5, 'seeded.json'), JSON.stringify(rec));
  expect(readSeeded(d5).state).toBe('unusable');
});

test('a record that is a link out of the run is not read', () => {
  const dir = fresh();
  const outside = path.join(root, 'outside-seeded.json');
  fs.copyFileSync(path.join(dir, 'seeded.json'), outside);
  fs.rmSync(path.join(dir, 'seeded.json'));
  fs.symlinkSync(outside, path.join(dir, 'seeded.json'));
  expect(readSeeded(dir).state).toBe('unusable');
});

test('cleanRel accepts one plain relative path only', () => {
  expect(cleanRel('a/b.txt')).toEqual({rel: 'a/b.txt'});
  expect(cleanRel('./a//b')).toEqual({rel: 'a/b'});
  for (const bad of ['', '/a', 'a/../b', '..', 'a\\b', 'a\0b', null, 5, {}]) expect(cleanRel(bad), String(bad)).toBeNull();
  expect(cleanRel('a/.git/x')).toEqual({dot: true});
  expect(WRITTEN.length).toBe(5);
});

test('cleanRel refuses a path longer than MAX_PATH and keeps one of exactly that length', () => {
  expect(cleanRel('a'.repeat(MAX_PATH))).toEqual({rel: 'a'.repeat(MAX_PATH)});
  expect(cleanRel('a'.repeat(MAX_PATH + 1))).toBeNull();
});

// the records of a list that was folded are not read again: the state is kept with the list, as fsChanges keeps the list
const counting = (list) => {
  const reads = {n: 0};
  const proxy = new Proxy(list, {get(t, k, r) { if (typeof k === 'string' && /^\d+$/.test(k)) reads.n++; return Reflect.get(t, k, r); }});
  return {proxy, reads};
};
const answerOf = (dir, seeded, changes, extra = {}) => writtenOf({runDir: dir, roots: [path.join(dir, 'work')], seeded, changes, finished: finishedSteps(dir), ...extra});
const listOf = (dir) => fs.readFileSync(path.join(dir, 'fs', 'changes.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));

test('a second answer for a list that did not change reads no record; a list that grew reads the new records only', () => {
  const N = 2000;
  const many = Array.from({length: N}, (_, i) => change(0, `bulk/f${i}.txt`, {text: `${i}\n`, step: 3 + i}));
  const dir = fresh({written: [], extra: many});
  fs.mkdirSync(path.join(dir, 'work/bulk'), {recursive: true});
  for (let i = N - 20; i < N; i++) fs.writeFileSync(path.join(dir, `work/bulk/f${i}.txt`), `${i}\n`);
  const seeded = readSeeded(dir);
  const list = listOf(dir);
  const {proxy, reads} = counting(list);
  const first = answerOf(dir, seeded, proxy);
  expect(reads.n).toBe(list.length);
  expect(first.total).toBe(N);
  reads.n = 0;
  const second = answerOf(dir, seeded, proxy);
  expect(reads.n).toBe(0);
  expect(second.files).toEqual(first.files);
  list.push(change(0, 'bulk/late.txt', {text: 'late\n', step: 9999}));
  fs.writeFileSync(path.join(dir, 'work/bulk/late.txt'), 'late\n');
  const third = answerOf(dir, seeded, proxy);
  expect(reads.n).toBe(1);
  expect(third.files[0].path).toBe('work/bulk/late.txt');
  expect(third.total).toBe(N + 1);
});

test('a list whose revision moved is folded again: a late attribution of a record that was read changes its step', () => {
  const dir = fresh({written: [], extra: [change(0, 'late.txt', {text: 'x\n', step: 2, action: 'a.one'})]});
  fs.writeFileSync(path.join(dir, 'work/late.txt'), 'x\n');
  const seeded = readSeeded(dir);
  const list = listOf(dir);
  expect(answerOf(dir, seeded, list, {rev: 0}).groups.map((g) => [g.step, g.actions])).toEqual([[2, ['a.one']]]);
  list.at(-1).attribution = {...list.at(-1).attribution, step: 3, action: 'a.two'};
  expect(answerOf(dir, seeded, list, {rev: 0}).groups.map((g) => g.step)).toEqual([2]);
  expect(answerOf(dir, seeded, list, {rev: 1}).groups.map((g) => [g.step, g.actions])).toEqual([[3, ['a.two']]]);
});

test('a file that is gone or became a link out of the run after the state was kept is described again, not answered from the state', () => {
  const dir = fresh({written: [], extra: [change(0, 'a.txt', {text: 'a\n', step: 2}), change(0, 'b.txt', {text: 'b\n', step: 2})]});
  for (const f of ['a.txt', 'b.txt']) fs.writeFileSync(path.join(dir, 'work', f), `${f[0]}\n`);
  const seeded = readSeeded(dir);
  const list = listOf(dir);
  expect(paths(answerOf(dir, seeded, list))).toEqual(['work/a.txt', 'work/b.txt']);
  fs.rmSync(path.join(dir, 'work/a.txt'));
  fs.rmSync(path.join(dir, 'work/b.txt'));
  fs.symlinkSync(root, path.join(dir, 'work/b.txt'));
  const w = answerOf(dir, seeded, list);
  expect([paths(w), w.gone, w.leftOut]).toEqual([[], 1, 1]);
});

test('an action name over MAX_ACTION is cut and counted once per record, one of exactly that length is kept', () => {
  const dir = fresh({written: [], extra: [change(0, 'a.txt', {text: 'a\n', step: 2, action: 'x'.repeat(MAX_ACTION)}), change(0, 'b.txt', {text: 'b\n', step: 3, action: 'y'.repeat(MAX_ACTION + 1)})]});
  for (const f of ['a.txt', 'b.txt']) fs.writeFileSync(path.join(dir, 'work', f), `${f[0]}\n`);
  const w = answerOf(dir, readSeeded(dir), listOf(dir));
  expect(w.cutStrings).toBe(1);
  expect(w.groups.map((g) => g.actions.map((a) => a.length))).toEqual([[MAX_ACTION], [MAX_ACTION]]);
});
