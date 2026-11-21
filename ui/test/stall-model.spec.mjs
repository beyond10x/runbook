// UXF-014: a wait on amend.upstream with no finding that needs a person is not a decision. The server
// finds the writer that used up its runs and its last error (decisions.mjs amendStall, on a hand-made
// case directory), and the Now band says "No step can run: … used its 3 runs (last error: …)" instead
// of asking anyone to decide. No server, no browser.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {amendStall, inbox} from '../decisions.mjs';
import {nowState, renderNow} from '../web/now.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'stall');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const ERR = 'story:access is draft; a story may move to: proposed, archived';
const bindings = {$artifacts: {findings: 'findings.yaml'}, 'plan.decompose': {argv: ['llm-node.sh', 'plan.decompose'], llm: {model: 'claude-haiku-5-5'}, max_runs: 3}, 'amend.upstream': {argv: ['sdc-tool.sh', 'amend', 'plan']}};
const run = (n, exit) => [{event: 'action_started', action: 'plan.decompose'}, {event: 'action_finished', action: 'plan.decompose', exit, log: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'}];
const events = (fails) => [{event: 'opened'}, ...Array.from({length: fails}, (_, i) => run(i, '1')).flat(),
  {event: 'awaiting_person', next: [{action: 'amend.upstream', command: 'sdc-drive run', decides: 'decide the open findings'}]}].map((e, i) => ({seq: i, at: '2026-11-21T01:33:53Z', ...e}));

function makeCase(findingStatus, fails) {
  fs.rmSync(root, {recursive: true, force: true});
  const dir = path.join(root, 'store', 'IDEA-1.todo.plan');
  fs.mkdirSync(path.join(dir, 'work'), {recursive: true});
  fs.writeFileSync(path.join(dir, 'work', 'findings.yaml'), `findings:\n- id: f1\n  status: ${findingStatus}\n`);
  const rec = path.join(root, 'llm', '018-plan.decompose');
  fs.mkdirSync(rec, {recursive: true});
  fs.writeFileSync(path.join(rec, 'apply.log'), `created story:access (draft) at aep/story/access.md\n${ERR}\n`);
  fs.writeFileSync(path.join(rec, 'stderr.txt'), 'metaharness: the transcript was written by Claude Code 2.1.330 and this adapter is pinned\n');
  fs.writeFileSync(path.join(root, 'llm-calls.jsonl'), JSON.stringify({action: 'plan.decompose', case: 'IDEA-1.todo.plan', record: 'llm/018-plan.decompose', applied_exit: 1}) + '\n');
  return {id: 'IDEA-1.todo.plan', rel: 'IDEA-1.todo.plan', dir, bindings, events: events(fails)};
}

test('UXF-014: amend.upstream with no finding for a person names the writer that used its runs', () => {
  let c = makeCase('resolved', 3);
  const stall = amendStall(c, root);
  expect(stall).toEqual({needsPerson: 0, exhausted: [{action: 'plan.decompose', runs: 3, max: 3, exit: '1', error: ERR}]});
  // not in the Decisions drawer
  expect(inbox({cases: [{...c, case: {}, ir: {actions: {}}}], runDir: root}).filter((i) => i.action === 'amend.upstream')).toEqual([]);
  // the Now band: no step can run, no Decide control
  const st = nowState([{...c, tree: 'T', depth: 1, parent: null, ir: {actions: {}, claims: {}}, case: {artifacts: {}}, evidence: {}, evaluations: {}, times: {}, stall}], {records: []}, {decisions: []}, 5000);
  expect(st.mode).toBe('stalled');
  expect(st.waiting).toEqual([]);
  const html = renderNow(st, 5000);
  expect(html).toContain(`No step can run: plan.decompose used its 3 runs (last error: ${ERR})`);
  expect(html).not.toContain('Decide</button>');
  expect(html).not.toContain('Waiting on you');
  // a finding that needs a person keeps it a decision
  c = makeCase('needs_person', 3);
  expect(amendStall(c, root)).toEqual({needsPerson: 1, exhausted: []});
  // fewer runs than max_runs: nothing is named as used up
  c = makeCase('resolved', 2);
  expect(amendStall(c, root)).toEqual({needsPerson: 0, exhausted: []});
});
