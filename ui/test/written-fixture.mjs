// A run directory of `runbook run --watch` that was seeded with two trees of files, written by hand with invented names
// (story products-are-what-the-run-wrote). It is test/loom-run-fixture.mjs plus what the runner and its watcher leave:
//   work/idea.yaml, work/mainline/src/m<N>.rs, work/impl/src/m<N>.rs, work/impl/README.md   the seed: 201 files
//   seeded.json, seeded-files.jsonl   what the runner listed when the seed ended (runbook/src/loom_run/seeded.rs)
//   fs/changes.jsonl                  every file under work/ the watcher saw: the 201 seeded files as `create` of step 1
//                                     (the watcher starts before work/ exists), then five files written by three steps
//   rounds.jsonl                      three lines, one per finished step
// Written by the steps: step 1 work/aep/story.md; step 2 work/dissection.md and work/impl/tests/repro.rs (added to a
// seeded tree); step 3 work/impl/src/m7.rs (a seeded file, changed) and work/repro-result.txt (written last).
// Nothing here is read from, or copied from, a real run.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {writeLoomRun} from './loom-run-fixture.mjs';

export const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const put = (file, data) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, data); };
const jl = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + '\n';
const T0 = Date.parse('2026-11-23T09:00:00Z');
const at = (sec) => new Date(T0 + sec * 1000).toISOString();

export const seedText = (p) => `// invented module of ${p}\npub fn f() -> u32 { ${p.length} }\n`;

export const WRITTEN = [
  {step: 1, path: 'work/aep/story.md', text: '# Invented story\n\nA story of an invented bug.\n', op: 'create', action: null},
  {step: 2, path: 'work/dissection.md', text: '# Dissection\n\nThe invented cause lies in module 7.\n', op: 'create', action: 'bug.dissect'},
  {step: 2, path: 'work/impl/tests/repro.rs', text: '#[test]\nfn reproduces() { assert_eq!(1, 2); }\n', op: 'create', action: 'test.reproduce'},
  {step: 3, path: 'work/impl/src/m7.rs', text: '// invented module of work/impl/src/m7.rs, fixed\npub fn f() -> u32 { 7 }\n', op: 'modify', action: 'change.fix'},
  {step: 3, path: 'work/repro-result.txt', text: 'subject: repro\nresult: red\n', op: 'create', action: 'change.fix'},
];

export function change(seq, p, {op = 'create', text = null, step = 1, action = null, root = 'work', from = null, after = undefined} = {}) {
  return {format: 'sdc.fs-change/1', seq, at: at(seq), root, path: p, op, from_path: from, before: null, after: after === undefined ? (text == null ? null : sha(text)) : after, binary: false, too_large: false,
    lines_added: null, lines_removed: null, diff: null, attribution: {case: null, action, record: null, tool: null, basis: 'step-window', step}};
}

// options: seed (false: no record of the seed, an older run), complete (the `complete` member of seeded.json), rounds (the
// lines of rounds.jsonl; null: no such file), watch (false: no fs/changes.jsonl), extra (more change records, appended)
export function writeWrittenRun(dir, {seed = true, complete = true, rounds = 3, watch = true, extra = [], written = WRITTEN} = {}) {
  writeLoomRun(dir, {outcome: 'ready'});
  const work = path.join(dir, 'work');
  fs.rmSync(work, {recursive: true, force: true});
  const seeded = [];
  const seedFile = (p, text) => { put(path.join(dir, p), text); seeded.push({path: p.replace(/^work\//, ''), sha256: sha(text), full: p, text}); };
  seedFile('work/idea.yaml', 'idea:\n  title: an invented idea\n');
  for (let i = 0; i < 100; i++) {
    seedFile(`work/mainline/src/m${i}.rs`, seedText(`work/mainline/src/m${i}.rs`));
    if (i < 99) seedFile(`work/impl/src/m${i}.rs`, seedText(`work/impl/src/m${i}.rs`));
  }
  seedFile('work/impl/README.md', '# Invented crate\n\nA README of the tree under test.\n');
  if (seeded.length !== 201) throw new Error(`the seed holds ${seeded.length} files`);
  for (const w of written) put(path.join(dir, w.path), w.text);
  if (seed) {
    seeded.sort((a, b) => (a.path < b.path ? -1 : 1));
    put(path.join(dir, 'seeded-files.jsonl'), jl(seeded.map(({path: p, sha256}) => ({path: p, sha256}))));
    put(path.join(dir, 'seeded.json'), JSON.stringify({format: 'runbook.seeded/1', at: 1795000000, complete, files: seeded.length, entries: [
      {path: 'idea.yaml', kind: 'file', files: 1}, {path: 'impl', kind: 'dir', files: 100}, {path: 'mainline', kind: 'dir', files: 100}]}, null, 2) + '\n');
  }
  if (watch) {
    let seq = 0;
    const rows = seeded.map((s) => change(seq++, s.path, {text: s.text, step: 1}));
    for (const w of written) rows.push(change(seq++, w.path.replace(/^work\//, ''), {op: w.op, text: w.text, step: w.step, action: w.action}));
    for (const e of extra) rows.push({...e, seq: seq++});
    put(path.join(dir, 'fs', 'changes.jsonl'), jl(rows));
  }
  if (rounds != null) put(path.join(dir, 'rounds.jsonl'), jl(Array.from({length: rounds}, (_, i) => ({started: at(i), ended: at(i + 1), seconds: 1, verdict: 'budget_spent', case_dir: ''}))));
  return {dir, seeded};
}
