// The last error of a writer that used up its runs (decisions.mjs lastError) is read from a file named by the run's own
// records (llm-calls.jsonl `record`, an action_finished `log`). That name is held under the run (a blob under the case's
// blobs/), with no dot-directory part: a name of another run, of a place outside every run, one that leaves the run by ../
// or one in a dot-directory is not read. In-process, hand-made case directories, invented text.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {amendStall} from '../decisions.mjs';
import {registerRun} from '../runfiles.mjs';
import {useSdcView} from './sdc-view.mjs';
import {fixtureRoot} from './security-products-lib.mjs';
useSdcView();

const root = fixtureRoot('lasterror');
const run = path.join(root, 'run');
const other = path.join(root, 'other-run');
const stray = path.join(root, 'not-a-run');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));
test.afterAll(() => fs.rmSync(root, {recursive: true, force: true}));

const bindings = {$artifacts: {findings: 'findings.yaml'}, 'plan.decompose': {argv: ['agent-node', 'plan.decompose'], llm: {model: 'claude-haiku-5-5'}, max_runs: 3}, 'amend.upstream': {argv: ['sdc-tool.sh', 'amend', 'plan']}};
const fin = (log) => ({event: 'action_finished', action: 'plan.decompose', exit: '1', ...(log ? {log} : {})});
const events = (log) => [{event: 'opened'}, ...Array.from({length: 3}, () => [{event: 'action_started', action: 'plan.decompose'}, fin(log)]).flat(),
  {event: 'awaiting_person', next: [{action: 'amend.upstream', command: 'a-runner run', decides: 'decide the open findings'}]}].map((e, i) => ({seq: i, at: '2026-11-21T01:33:53Z', ...e}));

function makeCase({record, log}) {
  fs.rmSync(root, {recursive: true, force: true});
  for (const d of [run, other]) registerRun(d);
  const dir = path.join(run, 'store', 'IDEA-1.todo.plan');
  fs.mkdirSync(path.join(dir, 'work'), {recursive: true});
  fs.writeFileSync(path.join(dir, 'work', 'findings.yaml'), 'findings:\n- id: f1\n  status: resolved\n');
  fs.mkdirSync(path.join(other, 'llm', 'x'), {recursive: true});
  fs.writeFileSync(path.join(other, 'llm', 'x', 'apply.log'), 'LAST-LINE-OF-ANOTHER-RUN\n');
  fs.mkdirSync(path.join(stray, 'llm'), {recursive: true});
  fs.writeFileSync(path.join(stray, 'llm', 'apply.log'), 'LAST-LINE-OUTSIDE-ANY-RUN\n');
  fs.mkdirSync(path.join(run, '.hid'), {recursive: true});
  fs.writeFileSync(path.join(run, '.hid', 'apply.log'), 'LAST-LINE-OF-A-DOT-DIRECTORY\n');
  fs.writeFileSync(path.join(other, 'blob-outside'), 'LAST-LINE-OF-A-BLOB-OUTSIDE\n');
  fs.writeFileSync(path.join(run, 'llm-calls.jsonl'), record ? JSON.stringify({action: 'plan.decompose', case: 'IDEA-1.todo.plan', record, applied_exit: 1}) + '\n' : '');
  return {id: 'IDEA-1.todo.plan', rel: 'IDEA-1.todo.plan', dir, bindings, events: events(log)};
}
const errorOf = (o) => String(amendStall(makeCase(o), run).exhausted[0].error);

test('control: the record of the writer is read', () => {
  const c = makeCase({record: 'llm/own'});
  fs.mkdirSync(path.join(run, 'llm', 'own'), {recursive: true});
  fs.writeFileSync(path.join(run, 'llm', 'own', 'apply.log'), 'OWN-LAST-LINE\n');
  expect(amendStall(c, run).exhausted[0].error).toBe('OWN-LAST-LINE');
});

test('a record that names another run is not read', () => {
  expect(errorOf({record: path.join(other, 'llm', 'x')})).not.toContain('ANOTHER-RUN');
});
test('a record that leaves the run by ../ is not read', () => {
  expect(errorOf({record: '../other-run/llm/x'})).not.toContain('ANOTHER-RUN');
});
test('a record outside every run is not read', () => {
  expect(errorOf({record: path.join(stray, 'llm')})).not.toContain('OUTSIDE-ANY-RUN');
});
test('a record in a dot-directory is not read', () => {
  expect(errorOf({record: '.hid'})).not.toContain('DOT-DIRECTORY');
});
test('a step log that leaves the blobs by ../ is not read', () => {
  expect(errorOf({log: '../../../../other-run/blob-outside'})).not.toContain('BLOB-OUTSIDE');
});
