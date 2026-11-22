// A run directory of `runbook run` of a runbook that is not SDC (protocol id `example@1`), written by hand
// with invented text, for the specs of what a step produced (story inspector-shows-step-products). It is
// test/loom-run-fixture.mjs plus what the runner and the steps leave:
//   work/example/<step>.txt     one file per step: `key: value` lines and a `result:` line
//   work/example/example.md     the runbook's product by the naming rule `work/<runbook>/<runbook>.md`
//   work/example/report.md      a second Markdown file that is not the product
//   work/example/data.json, work/idea.yaml, a hostile Markdown file, a file over the display limit, files that
//   are not text, a named pipe, links that stay in the run and links that leave it, a directory
//   work/.cache/token.txt       a tool's cache: a dot-directory the inspector must never list or serve
//   store/CASE-1/evidence/      evidence records with nested fields
//   store/runlog-case-<hex>.jsonl and store/blobs/<sha256>   Loom's own record of each program action (the
//                               digest of its output, standard output followed by standard error, as Loom
//                               keeps it; docs/loom-run-directory.md)
// Nothing here is read from, or copied from, a real run.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {writeLoomRun, ACTIONS, iso, EXAMPLE_VIEW} from './loom-run-fixture.mjs';

export const ROOT = 'CASE-1';
export const TOKEN_TEXT = 'invented-token-that-no-answer-may-hold-7f3a';
export const SECRET_TEXT = 'OUTSIDE-THE-RUN-SECRET-9c1d';
export const HOSTILE_MD = [
  '# Hostile report',
  '',
  'A paragraph with <script>window.__pwned = 1</script> in it.',
  '',
  '<img src="https://example.invalid/pixel.png" onerror="window.__pwned = 2">',
  '',
  '<div onclick="window.__pwned = 3">a block of raw html</div>',
  '',
  '[js](javascript:window.__pwned=4) [tab](java\tscript:window.__pwned=5) [data](data:text/html;base64,PHNjcmlwdD4=) [mail](mailto:a@example.invalid)',
  '',
  '[upper](JaVaScRiPt:window.__pwned=6) [spaced](  javascript:window.__pwned=7) [proto](//example.invalid/x) [abs](/etc/hostname)',
  '',
  '![the alt text](https://example.invalid/pic.png) and ![bad](javascript:window.__pwned=8)',
  '',
  '[good external](https://example.com/page) and [relative](data.json) and [up](../../../outside.txt)',
  '',
  '```',
  '<script>window.__pwned = 9</script>',
  '```',
  '',
  '`<b onmouseover=window.__pwned=10>`',
  '',
].join('\n');

export const REPORT_MD = [
  '# Example report',
  '',
  'An invented report with **strong**, *emphasis* and `inline code`.',
  '',
  '## What was done',
  '',
  '- the notes were written',
  '- the notes were checked',
  '  - nested item one',
  '  - nested item two',
  '',
  '1. first step',
  '2. second step',
  '',
  '| step | result |',
  '|------|:------:|',
  '| notes | valid |',
  '| report | invalid |',
  '',
  '> a quoted line',
  '',
  '---',
  '',
  '```',
  'echo "a fenced block"',
  '```',
  '',
  'See [the data](data.json) and [the site](https://example.com/docs).',
  '',
].join('\n');

const put = (file, data) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, data); };
const jl = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + '\n';
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const hex = (s) => Buffer.from(s).toString('hex');

export const OUTPUT = {
  'notes.write': Array.from({length: 300}, (_, i) => `writer line ${i + 1} of invented output`).join('\n') + '\nwriter finished\n',
  'notes.check': 'checked notes: 3 lines\n',
  'report.check': 'checked report: the shape is wrong\nerror: no heading named Summary\n',
};
export const OUTPUT_DIGEST = Object.fromEntries(Object.entries(OUTPUT).map(([a, t]) => [a, sha(t)]));
const LOST_DIGEST = sha('this blob is not in the run');

// options.product: 'view' (the view names work/example/example.md), 'missing' (the view names a file the run has not
// written), 'convention' (no key; work/example/example.md exists), 'newest' (no key, no example.md)
export function writeProductsRun(dir, {product = 'convention'} = {}) {
  writeLoomRun(dir, {outcome: 'ready'});
  const work = path.join(dir, 'work');
  put(path.join(work, 'example', 'notes.txt'), 'subject: notes\nlines: 3\nresult: valid\nnote: an invented note\n');
  put(path.join(work, 'example', 'check.txt'), 'subject: report\nlines: 9\nnote: no result line in this one\n');
  put(path.join(work, 'example', 'report.md'), REPORT_MD);
  if (product !== 'newest') put(path.join(work, 'example', 'example.md'), '# Example\n\nThe invented product of the example runbook.\n');
  put(path.join(work, 'example', 'data.json'), JSON.stringify({run: 'example', counts: {notes: 3, report: 9}, tags: ['a', 'b']}));
  put(path.join(work, 'example', 'hostile.md'), HOSTILE_MD);
  put(path.join(work, 'idea.yaml'), 'idea:\n  title: an invented idea\n  items:\n    - one\n    - two\n');
  put(path.join(work, 'example', 'big.txt'), 'x'.repeat(1024 * 1024 + 10));
  put(path.join(work, 'example', 'blob.bin'), Buffer.from([0, 1, 2, 255, 254, 0, 7]));
  put(path.join(work, 'example', 'latin1.txt'), Buffer.from([0x63, 0x61, 0x66, 0xe9, 0x0a]));
  put(path.join(work, '.cache', 'token.txt'), `${TOKEN_TEXT}\n`);
  put(path.join(work, 'example', '.hidden.txt'), `${TOKEN_TEXT}\n`);
  const outside = path.join(path.dirname(path.dirname(dir)), `outside-${path.basename(dir)}`, 'secret.txt'); // beside the runs folder, not in the run
  put(outside, `${SECRET_TEXT}\n`);
  fs.mkdirSync(path.join(work, 'example', 'sub'), {recursive: true});
  put(path.join(work, 'example', 'sub', 'inner.txt'), 'inner text\n');
  fs.symlinkSync(outside, path.join(work, 'example', 'leak.txt'));
  fs.symlinkSync(path.join(work, 'example', 'report.md'), path.join(work, 'example', 'alias.txt'));
  fs.symlinkSync(path.join(work, '.cache', 'token.txt'), path.join(work, 'example', 'viatoken.txt'));
  fs.symlinkSync('/dev/null', path.join(work, 'example', 'null.dev'));
  const mk = spawnSync('mkfifo', [path.join(work, 'example', 'pipe.fifo')]);
  if (mk.status !== 0) throw new Error(`mkfifo: ${mk.stderr}`);
  // modification times: the report is the newest Markdown file, the product older
  const at = (f, sec) => { const t = new Date(Date.parse('2026-11-21T10:00:00Z') + sec * 1000); fs.utimesSync(path.join(work, f), t, t); };
  at('report.md', 4000);
  at('example/hostile.md', 5100);
  if (fs.existsSync(path.join(work, 'example', 'example.md'))) at('example/example.md', 5000);
  at('example/report.md', 5200);

  // the evidence records, with nested fields, and the events that name them
  const store = path.join(dir, 'store', ROOT);
  const rec = (id, o) => put(path.join(store, 'evidence', `${id}.json`), JSON.stringify({format: 'canon-evidence/1', id, ...o}, null, 2) + '\n');
  rec('e0003-notes.check', {kind: 'notes_shape', result: 'valid', subject: 'notes', subject_revision: 'r1', producer: 'example-check', action: 'notes.check', observed_at: iso(24),
    details: {lines: 3, checks: [{name: 'shape', ok: true}, {name: 'length', ok: true, limit: {max: 80, unit: 'characters'}}], tags: ['a', 'b']}});
  rec('e0007-report.check', {kind: 'report_shape', result: 'invalid', subject: 'report', subject_revision: 'r2', producer: 'example-check', action: 'report.check', observed_at: iso(44),
    details: {lines: 9, checks: [{name: 'shape', ok: false, reason: 'no heading named Summary'}], nothing: null}});
  rec('e0009-bare', {kind: 'notes_shape'});
  const evidence = {'notes.check': ['e0003-notes.check'], 'report.check': ['e0007-report.check']};
  const events = [{event: 'opened', at: iso(0), protocol: {id: 'example', revision: 1}}];
  ACTIONS.forEach((a, i) => {
    const code = a === 'report.check' ? '1' : '0';
    events.push({event: 'action_started', at: iso(10 * (i + 1)), action: a, llm: /\.write$/.test(a), retry_of: []});
    const fin = {event: 'action_finished', at: iso(10 * (i + 1)), action: a, exit: code, evidence: evidence[a] || []};
    if (a === 'report.approve') fin.principal = 'decider:rule';
    events.push(fin);
  });
  events.push({event: 'terminated', at: iso(60), outcome: 'ready'});
  events.forEach((e, i) => { e.seq = i; e.source = 'loom-runlog'; });
  put(path.join(store, 'events.jsonl'), jl(events));
  // observed.jsonl: the runner dates a step as a whole: an action's two lines are the start and the end of its step
  put(path.join(store, 'observed.jsonl'), jl(events.map((e, i) => {
    const step = Math.floor((i - 1) / 2);
    const sec = e.event === 'action_started' ? 10 * (step + 1) : e.event === 'action_finished' ? 10 * (step + 1) + 4 : e.event === 'terminated' ? 60 : 0;
    return {index: i, event: e.event, observed_at: iso(sec), format: 'sdc.observed/1'};
  })));

  // Loom's run log of the case and the blobs it names
  const rlog = [{seq: 0, kind: 'CaseOpened', at: iso(0), protocol: 'example@1'}];
  ACTIONS.forEach((a, i) => {
    const line = {seq: i + 1, kind: 'ActionFinished', at: iso(10 * (i + 1)), action: a, exit: a === 'report.check' ? 1 : 0, evidence: evidence[a] || [], revisions: []};
    if (OUTPUT_DIGEST[a]) line.blob_digest = OUTPUT_DIGEST[a];
    if (a === 'report.write') line.blob_digest = LOST_DIGEST;
    rlog.push(line);
  });
  put(path.join(dir, 'store', `runlog-case-${hex(ROOT)}.jsonl`), jl(rlog));
  for (const [a, t] of Object.entries(OUTPUT)) put(path.join(dir, 'store', 'blobs', OUTPUT_DIGEST[a]), t);
  return dir;
}

export const viewWith = (product) => ({...EXAMPLE_VIEW, ...(product === undefined ? {} : {product})});
