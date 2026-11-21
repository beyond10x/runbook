// R-01 findings UXF-010, UXF-011, UXF-012 on hand-made cases (no server, no browser): the Now band says
// in words what the absence of a driver means, every open decision says who holds it and what taking
// it does, and a decision over several fields reads as one decision with its fields.
import {test, expect} from '@playwright/test';
import {nowState, renderNow} from '../web/now.js';
import {holderOf, fieldsOf, fieldCounts, deciderPerField, takenText} from '../web/holder.js';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const ir = {protocol: {id: 'sdc.direction', revision: 1}, actions: {'context.approve': {requires: [{capability: 'owner.approve'}], may_produce: [{evidence: 'context_approval'}]}}, claims: {}};
const bindings = {'context.approve': {argv: ['true'], subject: 'context', produces: 'context_approval'}};
const mk = (events, extra = {}) => ({id: 'IDEA-1.direction', rel: 'd', tree: 'T', depth: 1, parent: 'IDEA-1', ir, bindings, case: {protocol: 'sdc.direction', artifacts: {context: {revision: 't2a77fb422aa60348'}}},
  evidence: {}, evaluations: {}, times: {1: 1000}, events: events.map((e, i) => ({seq: i, at: '2026-11-21T01:23:55Z', ...e})), ...extra});
const next = [{action: 'context.approve', command: 'sdc-drive --case-dir /s approve --at "$(date)" --capability owner.approve --principal operator', decides: 'approve context revision t2a77fb422aa60348 (read /s/work/context.yaml)'}];
const waiting = mk([{event: 'opened'}, {event: 'awaiting_authority', next}]);
const noDriver = {records: [], drivers: {T: {file: false, alive: false, lock_alive: false, observed: true}}};
const reject = {at: '2026-11-21T01:26:40Z', principal: 'decider:haiku', case: 'IDEA-1.direction', what: 'context.approve', revision: 't2a77fb422aa60348', decision: 'reject', model: 'claude-haiku-5-5',
  reasons: ["The context's deployment is 'on the user's own machine', but requirement r4 fixes the deployment as a hosted web server.", 'The deployment amendment only settles offline use.']};
const context = {
  audience: {value: 'Two people', status: 'default', reason: 'The idea says so.'},
  platform: {value: 'Web browsers', status: 'accepted', reason: 'operator, 2026-11-21, as proposed'},
  deployment: {value: "on the user's own machine", status: 'changed', reason: 'operator, 2026-11-21, works offline'},
  licence: {value: 'Proprietary', status: 'default', reason: 'No input names a licence.'},
};

test('UXF-010: a wait, a stop and the end are said in words, never as a missing driver', () => {
  const html = renderNow(nowState([waiting], noDriver, {decisions: []}, 5000), 5000);
  expect(html).toContain('Waiting on you: nothing runs until this is decided');
  expect(html).not.toContain('no driver attached');
  const stuck = mk([{event: 'opened'}, {event: 'stuck', next: []}], {times: {1: 2000}});
  const s = renderNow(nowState([stuck], noDriver, null, 5000), 5000);
  expect(s).toMatch(/Stopped: stuck since [\d:]+.*no step is admissible/);
  expect(s).not.toContain('no driver attached');
  const closed = mk([{event: 'opened'}, {event: 'terminable', next: []}, {event: 'terminated', outcome: 'operating'}], {parent: null, depth: 0});
  const c = renderNow(nowState([closed], noDriver, null, 5000), 5000);
  expect(c).toContain('Closed');
  expect(c).not.toContain('no driver attached');
});

test('UXF-011: every open decision says who holds it, how long it waits and what taking it does', () => {
  let st = nowState([waiting], noDriver, {decisions: []}, 5000);
  expect(st.waiting[0].holder).toMatchObject({who: 'person', label: 'You (operator)'});
  expect(st.waiting[0].holder.note).toMatch(/decider setting is not recorded/);
  let html = renderNow(st, 5000);
  expect(html).toContain('Held by');
  expect(html).toContain('You (operator)');
  expect(html).toContain('Approving records owner.approve for context @ t2a77fb42');
  // the decider refused it: the person holds it, and the card says why
  st = nowState([waiting], noDriver, {decisions: [reject]}, 5000);
  expect(st.waiting[0].holder.note).toMatch(/^the decider \(claude-haiku-5-5\) refused it: The context's deployment/);
  // a decider call in progress holds it, with its progress
  st = nowState([waiting], {...noDriver, records: [{kind: 'decider', rel: 'decider/002-context.approve', case: 'IDEA-1.direction', action: 'context.approve', started: 1200, model: 'claude-haiku-5-5', turns: 3}]}, {decisions: [reject]}, 5000);
  expect(st.waiting[0].holder).toMatchObject({who: 'decider', label: 'Decider claude-haiku-5-5'});
  html = renderNow(st, 5000);
  expect(html).toContain('Decider claude-haiku-5-5');
  expect(html).not.toContain('Waiting on you');
  // a rule decider
  expect(holderOf({c: waiting, action: 'x.approve'}, [{principal: 'decider:approve', case: 'other', what: 'y', decision: 'approve'}])).toMatchObject({who: 'rule', label: 'Rule (approve)'});
  expect(takenText({capability: 'owner.approve'})).toMatch(/^Approving records owner.approve/);
});

test('UXF-012: one decision over several fields lists them, with each field’s state and who set it', () => {
  const f = fieldsOf(context);
  expect(f.map((x) => [x.name, x.state])).toEqual([['audience', 'default kept'], ['platform', 'accepted by operator'], ['deployment', 'changed by operator'], ['licence', 'default kept']]);
  expect(fieldCounts(f)).toBe('2 default, 1 accepted, 1 changed');
  expect(fieldsOf({questions: [1]})).toBe(null);
  const d = deciderPerField(f, reject);
  expect(d.whole).toBe(null);
  expect(d.fields).toMatchObject({deployment: 'decider would change this', audience: 'decider kept this'});
  expect(deciderPerField(f, {decision: 'approve', reasons: ['Consistent with the requirements.']}).whole).toBe('approved as a whole');
  // the Now band's waiting card names one decision and its fields
  const st = nowState([waiting], noDriver, {decisions: []}, 5000);
  const html = renderNow(st, 5000, [{kind: 'approve', case: 'IDEA-1.direction', action: 'context.approve', artifact: {json: context}}]);
  expect(html).toContain('1 decision over 4 fields');
  expect(html).toContain('2 default, 1 accepted, 1 changed');
});
