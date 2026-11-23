// The Now band's three states of a run that has the runner's own record (runner.json, action_started events:
// docs/loom-run-directory.md), on hand-made cases and a hand-made /api/live answer: no server, no browser.
//   running   "<action> running since <time>, <n> min"
//   between   "Between steps"
//   not live  "Not live: last started <action>"
// A run without the record keeps the answers it had.
import {test, expect} from '@playwright/test';
import * as Now from '../web/now.js';
import {useSdcView} from './sdc-view.mjs';
const {nowState, nowSay, nowBar, renderNow, renderNowBar} = Now;
const procOf = (live) => Now.procOf(live);
useSdcView();

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const ir = {protocol: {id: 'sdc.story', revision: 1}, actions: {'draft.write': {may_produce: [{evidence: 'draft_text'}]}}, claims: {}};
const bindings = {'draft.write': {argv: ['/x/tools/write-draft.sh'], subject: 'draft'}};
const mk = (events, extra = {}) => ({id: 'C-1', rel: 'C-1', tree: 'T', depth: 0, parent: null, ir, bindings, case: {protocol: 'sdc.story', artifacts: {draft: {revision: 'r1'}}}, evidence: {}, evaluations: {}, times: {}, events: events.map((e, i) => ({seq: i, at: '2026-11-21T10:00:00Z', ...e})), ...extra});

const START = Date.parse('2026-11-21T10:05:00Z');
const step = {action: 'draft.write', case: 'C-1', try: 2, at: START / 1000, source: 'program_start', role: 'program'};
const runner = {pid: 4242, started_at: START / 1000 - 30, closed_at: null};
const live = (state, extra = {}) => ({records: [], liveness: {state, runner, step, ...extra}});
const clock = (t) => new Date(t).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'});

// a case after one step of a run with `--max-steps 1` per step: Loom ends every such step with step_budget_spent
const afterAStep = () => mk([{event: 'opened'}, {event: 'action_started', action: 'draft.write'}, {event: 'action_finished', action: 'draft.write', exit: '0'}, {event: 'step_budget_spent', next: []}]);
const inStep = () => mk([{event: 'opened'}, {event: 'action_started', action: 'draft.write'}]);

test('running: the action, the time it started and the minutes it has run', () => {
  const st = nowState([inStep()], live('running'), null, START + 3 * 60000 + 20000);
  expect(st.mode).toBe('running');
  expect(nowSay(st).head).toBe(`draft.write running since ${clock(START)}, 3 min`);
  expect(nowSay(st).sub).toContain('try 2');
  expect(st.idle).toBe(null);
  expect(nowBar(st).state).toBe(`draft.write running since ${clock(START)}, 3 min`);
});

test('running, with no case event of the step yet: the runner’s record alone says it, as text of a card', () => {
  const st = nowState([mk([{event: 'opened'}])], live('running'), null, START + 61000);
  expect(st.mode).toBe('running');
  expect(nowSay(st).head).toBe(`draft.write running since ${clock(START)}, 1 min`);
  const html = renderNow(st, START + 61000);
  expect(html).toContain('Running');
  expect(html).toContain('draft.write');
  expect(html).toContain('try 2');
});

test('running, a step the runner started without knowing its action: it says so', () => {
  const l = live('running', {step: {action: null, case: null, try: null, at: START / 1000, source: 'step_call', role: null}});
  const st = nowState([mk([{event: 'opened'}])], l, null, START + 5000);
  expect(nowSay(st).head).toBe(`A step running since ${clock(START)}, 0 min`);
  expect(renderNow(st, START + 5000)).toContain('does not know which action');
});

test('between steps: a case that ended a step on its step budget is not stopped while the runner lives', () => {
  const st = nowState([afterAStep()], live('between'), null, START + 9000);
  expect(st.mode).toBe('between');
  expect(st.stuck).toEqual([]);
  expect(nowSay(st).head).toBe('Between steps');
  expect(nowBar(st).state).toBe('Between steps');
  expect(nowBar(st).step).toBe('No action in flight');
  const html = renderNow(st, START + 9000);
  expect(html).not.toMatch(/Stopped/);
  expect(html).toContain('Between steps');
  expect(st.idle).toBe(null);
});

test('the same case without the runner’s record keeps its old answer (stopped on the budget)', () => {
  for (const l of [undefined, {records: []}, {records: [], liveness: {state: 'unknown', why: 'the runner writes no record of a live process'}}]) {
    const st = nowState([afterAStep()], l, null, START);
    expect(st.mode).toBe('stuck');
    expect(st.proc).toBe(null);
    expect(nowSay(st).head).toMatch(/^Stopped: step budget spent/);
  }
});

test('a started step in a run without the record is shown as before', () => {
  const st = nowState([inStep()], {records: [], liveness: {state: 'unknown'}}, null, START);
  expect(st.mode).toBe('running');
  expect(nowSay(st).head).toBe('1 running');
});

test('not live: the runner is gone, the last action it started is named, a started step is not running', () => {
  const st = nowState([inStep()], live('not_live', {why: 'no process of the recorded pid and start time is running'}), null, START + 120000);
  expect(st.mode).toBe('notlive');
  expect(st.running).toEqual([]);
  expect(nowSay(st).head).toBe('Not live: last started draft.write');
  expect(nowBar(st).state).toBe('Not live: last started draft.write');
  expect(nowBar(st).step).toContain('draft.write');
  expect(st.idle).not.toBe(null);
  const html = renderNow(st, START + 120000);
  expect(html).toContain('Not live');
  expect(html).not.toContain('>Running<');
});

test('not live, no action was started in the run', () => {
  const st = nowState([mk([{event: 'opened'}])], {records: [], liveness: {state: 'not_live', why: 'the runner closed its record', runner, step: null}}, null, START);
  expect(st.mode).toBe('notlive');
  expect(nowSay(st).head).toBe('Not live: no action was started');
});

test('a stopped or closed run keeps its own words, whatever the record says', () => {
  const stopped = {at: '2026-11-21T10:06:00Z', by: 'operator', reason: 'enough'};
  expect(nowState([inStep()], live('not_live'), {stopped}, START).mode).toBe('stopped');
  const closed = mk([{event: 'opened'}, {event: 'terminated', outcome: 'done'}]);
  expect(nowState([closed], live('between'), null, START).mode).toBe('closed');
});

test('run content is text: an action or a case with markup in its name is never markup', () => {
  const evil = '<img src=x onerror=alert(1)>';
  const l = live('running', {step: {action: evil, case: evil, try: 1, at: START / 1000, source: 'program_start', role: 'program'}});
  const st = nowState([mk([{event: 'opened'}])], l, null, START + 1000);
  for (const html of [renderNow(st, START + 1000), renderNowBar(st)]) {
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;img');
  }
  const dead = nowState([mk([{event: 'opened'}])], live('not_live', {step: l.liveness.step}), null, START + 1000);
  for (const html of [renderNow(dead, START + 1000), renderNowBar(dead)]) expect(html).not.toContain('<img');
});

test('procOf reads only the three states and fixes the shape of what it passes on', () => {
  expect(procOf(undefined)).toBe(null);
  expect(procOf({liveness: {state: 'ended'}})).toBe(null);
  expect(procOf({liveness: {state: 'history'}})).toBe(null);
  expect(procOf(live('running'))).toMatchObject({state: 'running', pid: 4242, step: {action: 'draft.write', case: 'C-1', try: 2, t: START}});
  expect(procOf({liveness: {state: 'between', runner, step: null}}).step).toBe(null);
});
