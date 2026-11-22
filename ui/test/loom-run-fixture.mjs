// A run directory of `runbook run` for a runbook that is not the SDC one, written by hand with invented
// text, for every spec that needs one. The layout is docs/loom-run-directory.md; each file has the shape its
// writer gives it:
//   status.json     runbook/src/loom_run/run_dir.rs `ended` (sdc.run-status/1; `delivered` is true only for the
//                   outcomes operating, fixed and delivered, so false for this runbook's good end `ready`)
//   outcome.json    runbook/src/loom_run/run_files.rs (runbook.loom-run-outcome/1)
//   store/<root>/   the projection of run_dir.rs `project`: events.jsonl (opened, action_started, action_finished,
//                   terminated; no evaluation, no claim), observed.jsonl, ir.json (the effect of each action) and
//                   bindings.json (person and copy actions only)
//   protocols/, bindings/   the copies the runner makes of the protocol files and of the bindings
//                   (runbook/src/loom_run/bindings_copy.rs copies `bindings/` as it is; format loom.bindings/1)
//   fs/changes.jsonl only for a run started with --watch (docs/fs-changes.md)
// The protocol is `example`, revision 1; its six actions, its two artifacts and its good end `ready`.
import fs from 'node:fs';
import path from 'node:path';

export const PROTOCOL = 'example';
export const REVISION = 1;
export const ACTIONS = ['notes.write', 'notes.check', 'report.write', 'report.check', 'report.approve', 'report.read'];
export const ARTIFACTS = ['notes', 'report'];

export const PROTOCOL_YAML = `format: protocol/1
protocol:
  id: example
  revision: 1
  description: Write notes, check them, write a report from them, check it, approve it.
artifacts:
  notes:
    description: the notes as a person gave them to the writer
  report:
    description: the report written from the notes
evidence_kinds:
  notes_shape:
    description: the notes passed the shape check
  report_shape:
    description: the report passed the shape check
actions:
  notes.write:
    description: a model writes the notes
    effect: write
  notes.check:
    description: check the shape of the notes
    effect: read
    may_produce:
      - evidence: notes_shape
  report.write:
    description: a model writes the report from the notes
    effect: write
  report.check:
    description: check the shape of the report
    effect: read
    may_produce:
      - evidence: report_shape
  report.approve:
    description: a person approves the report
    effect: read
  report.read:
    description: take the approval of the report from the first case
    effect: read
outcomes:
  ready:
    description: the report is approved
    requires:
      decision: report_signed
`;

export const BINDINGS_YAML = `format: loom.bindings/1
protocol: example@1
program_root: bindings
artifacts:
  notes: notes.txt
  report: report.md
actions:
  - action: notes.write
    executor:
      kind: program
      argv: [sdc-node, notes.write, --model, claude-haiku-5-5, --stub, stub-file, notes.txt, notes.txt]
  - action: notes.check
    executor:
      kind: program
      argv: [tools/example-check.sh, check-notes]
    evidence:
      - kind: notes_shape
        subject: notes
        producer: example-check
        from: exit
        results: {"0": valid, "*": invalid}
  - action: report.write
    executor:
      kind: program
      argv: [sdc-node, report.write, --model, claude-haiku-5-5, --stub, stub-file, report.md, report.md]
  - action: report.check
    executor:
      kind: program
      argv: [tools/example-check.sh, check-report]
    evidence:
      - kind: report_shape
        subject: report
        producer: example-check
        from: exit
        results: {"0": valid, "*": invalid}
  - action: report.approve
    executor:
      kind: person
      principal: person:owner
      argv: [tools/example-check.sh, approve, "{principal}"]
  - action: report.read
    executor:
      kind: copy
      evidence: report_approval
      from: example-first@1
`;

const jl = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + (rows.length ? '\n' : '');
const put = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, text); };
const T0 = Date.parse('2026-11-21T10:00:00Z');
export const iso = (sec) => new Date(T0 + sec * 1000).toISOString().replace('.000Z', 'Z');

// options: outcome (the tree's outcome), delivered (what status.json says), root (the case id), protocols and
// bindings (write the copies), watch (null: no fs/ directory; 'empty': an empty record; 'changes': two changes)
export function writeLoomRun(dir, {outcome = 'ready', delivered = false, root = 'CASE-1', protocolId = PROTOCOL, protocols = true, bindings = true, watch = null} = {}) {
  fs.rmSync(dir, {recursive: true, force: true});
  put(path.join(dir, 'settings.json'), JSON.stringify({case: root, root_protocol: `${protocolId}@${REVISION}`, loom: {in_process: true, confinement: 'local'}}, null, 2) + '\n');
  put(path.join(dir, 'run-settings.jsonl'), jl([{format: 'sdc.run-settings/1', at: iso(0), leg: 'loom', adapter: 'claude-cli'}]));
  put(path.join(dir, 'llm-calls.jsonl'), '');
  put(path.join(dir, 'decisions.jsonl'), '');
  const end = 60;
  put(path.join(dir, 'status.json'), JSON.stringify({format: 'sdc.run-status/1', at: iso(end), state: 'closed', outcome, delivered, note: null, closed_by: null}, null, 2) + '\n');
  put(path.join(dir, 'outcome.json'), JSON.stringify({format: 'runbook.loom-run-outcome/1', outcome: 'ended', reason: outcome, loom_exit_code: 0, stderr_tail: '', exit_code: 0, steps: ACTIONS.length, spent_micro_usd: 0, llm_calls: 0}, null, 2) + '\n');
  if (protocols) put(path.join(dir, 'protocols', 'example.protocol.yaml'), PROTOCOL_YAML);
  if (bindings) put(path.join(dir, 'bindings', 'example.yaml'), BINDINGS_YAML);
  put(path.join(dir, 'work', 'notes.txt'), 'three invented lines\nabout nothing real\nfor a fixture\n');
  put(path.join(dir, 'work', 'report.md'), '# Report\n\nAn invented report.\n');
  const store = path.join(dir, 'store', root);
  const events = [{event: 'opened', at: iso(0), protocol: {id: protocolId, revision: REVISION}}];
  ACTIONS.forEach((a, i) => {
    events.push({event: 'action_started', at: iso(10 * (i + 1)), action: a, llm: /\.write$/.test(a), retry_of: []});
    const fin = {event: 'action_finished', at: iso(10 * (i + 1)), action: a, exit: '0', evidence: []};
    if (a === 'report.approve') fin.principal = 'decider:rule';
    events.push(fin);
  });
  events.push({event: 'terminated', at: iso(end), outcome});
  events.forEach((e, i) => { e.seq = i; e.source = 'loom-runlog'; });
  put(path.join(store, 'events.jsonl'), jl(events));
  put(path.join(store, 'observed.jsonl'), jl(events.map((e, i) => ({index: i, event: e.event, observed_at: e.at, format: 'sdc.observed/1'}))));
  const effects = {'notes.write': 'write', 'notes.check': 'read', 'report.write': 'write', 'report.check': 'read', 'report.approve': 'read', 'report.read': 'read'};
  if (protocols) put(path.join(store, 'ir.json'), JSON.stringify({actions: Object.fromEntries(Object.entries(effects).map(([a, e]) => [a, {effect: e}]))}, null, 2) + '\n');
  if (bindings) put(path.join(store, 'bindings.json'), JSON.stringify({'report.approve': {manual: true}, 'report.read': {copy: {from: 'example-first'}}}, null, 2) + '\n');
  if (watch) {
    const change = (seq, file, n) => ({format: 'sdc.fs-change/1', seq, at: iso(10 * seq), root: 'work', path: file, op: 'create', from_path: null, before: null, after: 'a'.repeat(64), binary: false, too_large: false,
      lines_added: n, lines_removed: 0, diff: `--- /dev/null\n+++ b/${file}\n@@ -0,0 +1,${n} @@\n${Array.from({length: n}, (_, k) => `+line ${k + 1}`).join('\n')}\n`,
      attribution: {case: root, action: seq === 1 ? 'notes.write' : 'report.write', record: null, tool: null, basis: 'step-window', step: seq}});
    put(path.join(dir, 'fs', 'changes.jsonl'), watch === 'changes' ? jl([change(1, 'notes.txt', 3), change(2, 'report.md', 3)]) : '');
  }
  return dir;
}

export const EXAMPLE_VIEW = {
  format: 'runbook.view/1',
  runbook: 'example',
  protocols: ['^example$'],
  good_outcomes: ['ready'],
  phases: {stages: ['Work'], items: [{id: 'notes', stage: 'Work', label: 'Notes', protocol: PROTOCOL, claims: ['notes.valid']}, {id: 'report', stage: 'Work', label: 'Report', protocol: PROTOCOL, claims: ['report.valid']}]},
};
export const withoutGoodEnd = ({good_outcomes, ...rest}) => rest;
