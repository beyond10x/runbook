#!/usr/bin/env node
// Writes the committed source of the fixture runs (test/recorded/trial-todo-1-slim) from the recorded
// run runs/trial-todo-1, so the scenarios person, stopped, human, decider and autopilot of
// test/fixture-run.mjs run on a checkout without runs/ (git-ignored).
//
//   node test/slim-source.mjs [<recorded-run-dir>]
//
// Kept: the case tree IDEA-1 with the cases intake and direction (IDEA-1.todolist is dropped), the
// work files the scenarios and the drawer read, the decider record 001-roadmap.approve and its line
// of decisions.jsonl. Nothing an LLM wrote or was given is kept: the record's events keep only their
// kind, time, tool name and tool path; its prompt, transcripts, cwd and answer are replaced, and the
// decision's reasons are written here. Absolute paths become @SRC@ (the run directory, replaced by
// fixture-run.mjs with its target) or ~/ (the home directory).
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const SRC = path.resolve(process.argv[2] || path.join(here, '../../runs/trial-todo-1'));
const OUT = path.join(here, 'recorded/trial-todo-1-slim');
if (!fs.existsSync(path.join(SRC, 'store'))) { console.error(`slim-source.mjs: missing run directory ${SRC}`); process.exit(2); }
const HOME = process.env.HOME;
const user = new RegExp(`\\b${path.basename(HOME).replace(/[^\w-]/g, '')}\\b`, 'g');
// the operator is named by role, never by name
const clean = (t) => t.split(SRC).join('@SRC@').split(HOME + '/').join('~/').replace(/\/home\/[^/\s"'<>\\]+\//g, '~/').replace(user, 'operator');
const TEXT = /\.(json|jsonl|ya?ml|md|txt)$/;

fs.rmSync(OUT, {recursive: true, force: true});
const skip = new Set(['children/IDEA-1.todolist', 'work/impl', 'work/mainline', 'work/published', 'work/release', 'work/spec', 'work/.sdc']);
function copy(from, to, rel = '') {
  for (const n of fs.readdirSync(from)) {
    const r = rel ? `${rel}/${n}` : n;
    if (skip.has(r) || n.endsWith('.lock')) continue;
    const f = path.join(from, n), t = path.join(to, n);
    const st = fs.lstatSync(f);
    if (st.isDirectory()) { fs.mkdirSync(t, {recursive: true}); copy(f, t, r); }
    else if (st.isFile()) fs.writeFileSync(t, TEXT.test(n) ? clean(fs.readFileSync(f, 'utf8')) : fs.readFileSync(f));
    else if (st.isSymbolicLink()) fs.symlinkSync(clean(fs.readlinkSync(f)), t);
  }
}
fs.mkdirSync(path.join(OUT, 'store/IDEA-1'), {recursive: true});
copy(path.join(SRC, 'store/IDEA-1'), path.join(OUT, 'store/IDEA-1'));

// the decider record: structure only
const name = 'decider/001-roadmap.approve';
const from = path.join(SRC, name), to = path.join(OUT, name);
fs.mkdirSync(to, {recursive: true});
for (const f of ['request.json', 'argv.json', 'decide-request.json', 'frame.json']) if (fs.existsSync(path.join(from, f))) fs.writeFileSync(path.join(to, f), clean(fs.readFileSync(path.join(from, f), 'utf8')));
fs.writeFileSync(path.join(to, 'prompt.md'), '(the prompt is not kept in the committed fixture)\n');
const keep = ['format', 'seq', 'run', 'at', 'event', 'adapter', 'model', 'name', 'tool_use_id', 'usage', 'process', 'events', 'reason', 'run_id'];
const events = fs.readFileSync(path.join(from, 'events.jsonl'), 'utf8').trim().split('\n').map((l) => {
  const e = JSON.parse(l);
  const o = {};
  for (const k of keep) if (e[k] !== undefined) o[k] = e[k];
  if (e.input?.file_path) o.input = {file_path: clean(e.input.file_path)};
  else if (e.input?.pattern) o.input = {pattern: e.input.pattern};
  return JSON.stringify(o);
});
fs.writeFileSync(path.join(to, 'events.jsonl'), events.join('\n') + '\n');
const resp = JSON.parse(fs.readFileSync(path.join(from, 'response.json'), 'utf8'));
fs.writeFileSync(path.join(to, 'response.json'), JSON.stringify({adapter: resp.adapter, model: resp.model, exit: resp.exit, duration_ms: resp.duration_ms, wall_ms: resp.wall_ms, cost_usd: resp.cost_usd, num_turns: resp.num_turns, usage: resp.usage, is_error: resp.is_error, subtype: resp.subtype, result: '(the answer is not kept in the committed fixture)'}, null, 2) + '\n');
const line = fs.readFileSync(path.join(SRC, 'decisions.jsonl'), 'utf8').split('\n').find((l) => l.includes('001-roadmap.approve'));
if (line) {
  const d = JSON.parse(clean(line));
  d.reasons = ['The roadmap names its epic but not its scope or order of delivery.', 'Rewrite it with the epic, its scope and its order, traced to r1, r2 and r3.'];
  fs.writeFileSync(path.join(OUT, 'decisions.jsonl'), JSON.stringify(d) + '\n');
}
console.log(`slim-source: ${path.relative(process.cwd(), OUT)}`);
