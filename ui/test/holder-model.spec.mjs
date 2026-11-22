// R-01 findings UXF-010, UXF-011, UXF-012 on hand-made cases (no server, no browser): the Now band says
// in words what the absence of a driver means, every open decision says who holds it and what taking
// it does, and a decision over several fields reads as one decision with its fields.
import {test, expect} from '@playwright/test';
import {nowState, renderNow} from '../web/now.js';
import {holderOf, fieldsOf, fieldCounts, deciderPerField, takenText} from '../web/holder.js';
import {computeKpis, renderKpis, personStops} from '../web/kpis.js';
import {principalOf} from '../web/principal.js';
import {useSdcView} from './sdc-view.mjs';
useSdcView();

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

test('UXF-010: a wait, a stop and the end are said in words; with no driver attached the clocks stand at the last event', () => {
  let st = nowState([waiting], noDriver, {decisions: []}, 5000);
  expect(st.idle).toEqual({t: 1000});
  let html = renderNow(st, 5000);
  expect(html).toContain('Waiting on you: nothing runs until this is decided');
  expect(html).toMatch(/no driver attached, last activity [\d: APM]+/);
  // the KPI clock ends at the last event, not at now
  const kp = computeKpis([waiting], {decisions: []}, [], st, 9000);
  expect(kp[0].live).toBe(null);
  expect(kp[0].sub.live).toBe(null);
  // a driver attached (its lock pid runs): the clock runs, nothing said about the driver's absence
  st = nowState([waiting], {records: [], drivers: {T: {file: false, alive: false, lock_alive: true, lock_pid: 7, observed: true}}}, {decisions: []}, 5000);
  expect(st.idle).toBe(null);
  expect(renderNow(st, 5000)).not.toContain('no driver attached');
  const stuck = mk([{event: 'opened'}, {event: 'stuck', next: []}], {times: {1: 2000}});
  const s = renderNow(nowState([stuck], noDriver, null, 5000), 5000);
  expect(s).toMatch(/Stopped: stuck since [\d: APM]+.*no step is admissible/);
  expect(s).toContain('no driver attached');
  const closed = mk([{event: 'opened'}, {event: 'terminable', next: []}, {event: 'terminated', outcome: 'operating'}], {parent: null, depth: 0});
  const cst = nowState([closed], noDriver, null, 5000);
  expect(cst.idle).toBe(null);
  const c = renderNow(cst, 5000);
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

// The run-settings record (sdc.run-settings/1) names who holds the decisions; run-stopped.json
// (sdc.run-stopped/1) a stop made on purpose.
const settings = (kind, model = null) => ({format: 'sdc.run-settings/1', decider: {kind, model, escalate: 'stop', max_decider_calls: 20}});
const stopped = {at: '1970-01-01T00:00:04Z', by: 'operator', reason: 'looking at the context', verdict_at_stop: 'awaiting_authority', case: 'IDEA-1.direction'};

test('the settings record says who holds a decision, in place of "not recorded"', () => {
  const at = (s) => nowState([waiting], noDriver, {decisions: [], settings: s}, 5000).waiting[0].holder;
  expect(at(settings('haiku', 'claude-haiku-5-5'))).toMatchObject({who: 'decider', label: 'Decider claude-haiku-5-5'});
  expect(at(settings('haiku', 'claude-haiku-5-5')).note).toMatch(/run-settings\.jsonl, escalate stop/);
  expect(at(settings('approve'))).toMatchObject({who: 'rule', label: 'Rule (approve)'});
  expect(at(settings('person'))).toMatchObject({who: 'person', label: 'You (operator)'});
  expect(at(settings('model', 'claude-opus-5-5'))).toMatchObject({who: 'decider', label: 'Decider claude-opus-5-5'});
  for (const k of ['model', 'haiku', 'approve', 'person']) expect(at(settings(k)).note).not.toMatch(/not recorded/);
  // a decider holds it: no Decide control, the head names the decider
  const html = renderNow(nowState([waiting], noDriver, {decisions: [], settings: settings('haiku', 'claude-haiku-5-5')}, 5000), 5000);
  expect(html).toContain('Held by the decider: it decides when the run continues');
  expect(html).not.toContain('Decide</button>');
  // the decider's own decision on it still wins (it refused or escalated: the person holds it)
  expect(nowState([waiting], noDriver, {decisions: [reject], settings: settings('haiku')}, 5000).waiting[0].holder.who).toBe('person');
});

test('a stopped run: said in the past tense, no Decide control, the KPI clock ends at the stop', () => {
  const st = nowState([waiting], noDriver, {decisions: [], settings: settings('haiku', 'claude-haiku-5-5'), stopped}, 4000);
  expect(st.mode).toBe('stopped');
  const html = renderNow(st, 4000);
  expect(html).toMatch(/Stopped by operator at [\d: APM]+: looking at the context/);
  expect(html).toContain('It was awaiting authority for context.approve in IDEA-1.direction');
  expect(html).not.toContain('Decide</button>');
  expect(html).not.toContain('Waiting on you');
  // waiting from the verdict (t=1000) to the stop (t=4000), not to now (t=9000)
  const kp = computeKpis([waiting], {decisions: [], settings: settings('haiku', 'claude-haiku-5-5'), stopped}, [], st, 9000);
  expect(kp[0].sub.text).toBe('0:03');
  expect(kp[0].live).toBe(null);
  expect(kp[0].sub.live).toBe(null);
  expect(kp[0].fail).toMatchObject({text: 'should be 0'});
  expect(kp[0].fail.stops.map((s) => [s.c.id, s.actions, s.ms])).toEqual([['IDEA-1.direction', ['context.approve'], 3000]]);
  expect(renderKpis(kp, 9000)).toContain('data-goto="d" data-seq="1"');
  // without the file nothing changes
  expect(nowState([waiting], noDriver, {decisions: []}, 4000).mode).toBe('waiting');
});

test('who a principal is: a person only for a person principal', () => {
  expect(principalOf('person:anna')).toMatchObject({kind: 'person', label: 'anna', person: true});
  expect(principalOf('decider:haiku', {model: 'claude-haiku-5-5'})).toMatchObject({kind: 'decider', label: 'decider claude-haiku-5-5, on the operator’s behalf', person: false});
  // D149: decider:model is the LLM decider whatever its model; decider:haiku its old name (runs before D149)
  expect(principalOf('decider:model', {model: 'claude-opus-5-5'})).toMatchObject({kind: 'decider', label: 'decider claude-opus-5-5, on the operator’s behalf', person: false});
  expect(principalOf('decider:approve')).toMatchObject({kind: 'rule', label: 'rule approve', person: false});
  expect(principalOf('rule:auto-approve')).toMatchObject({kind: 'rule', label: 'rule auto-approve', person: false});
  expect(principalOf('human')).toMatchObject({kind: 'legacy', label: 'recorded as human (principal not recorded)', person: false});
  expect(principalOf('non-interactive').person).toBe(false);
  expect(principalOf(null)).toMatchObject({kind: 'none', person: false});
  // a bare id is the person a driver's --principal named before the person: prefix
  expect(principalOf('operator')).toMatchObject({kind: 'person', legacy: true});
  // autopilot (autonomy A5): the holder is the decider, the head never "waiting on you"
  const auto = {...settings('haiku', 'claude-haiku-5-5'), autonomy: 'A5'};
  const st = nowState([waiting], noDriver, {decisions: [], settings: auto}, 5000);
  expect(st.waiting[0].holder).toMatchObject({who: 'decider', label: 'Decider claude-haiku-5-5', autopilot: true});
  const html = renderNow(st, 5000);
  expect(html).toContain('No decision is pending: the decider takes the next one when the run continues');
  expect(html).not.toMatch(/\bperson\b|Waiting on you|You \(/i);
  // even after the decider escalated
  expect(nowState([waiting], noDriver, {decisions: [{...reject, decision: 'escalate'}], settings: auto}, 5000).waiting[0].holder.who).toBe('decider');
});

test('waiting on a person: only the spans a person held; neutral when a person holds the decisions', () => {
  // two waits: the first taken by the decider (its decision inside the span), the second still open
  const c = mk([{event: 'opened'}, {event: 'awaiting_person', next}, {event: 'action_started', action: 'context.approve'}, {event: 'action_finished', action: 'context.approve', principal: 'decider:haiku'}, {event: 'awaiting_person', next}],
    {times: {1: 1000, 2: 61000, 3: 61000, 4: 70000}});
  const run = (kind) => ({decisions: [], settings: kind ? settings(kind, 'claude-haiku-5-5') : null});
  const stops = personStops([c], run('haiku'), 100000, null);
  expect(stops.map((s) => [s.seq, s.ms, s.open])).toEqual([[4, 30000, true]]);
  const attached = {records: [], drivers: {T: {file: false, alive: false, lock_alive: true, lock_pid: 7, observed: true}}};
  const kp = (kind, live = attached) => computeKpis([c], run(kind), [], nowState([c], live, run(kind), 100000), 100000)[0];
  expect(kp('haiku').fail.stops).toHaveLength(1);
  expect(kp('person').fail).toBe(null);
  expect(kp(null).fail).toBe(null);
  expect(kp('person').sub.text).toBe('0:30');
  // no driver attached: the open wait ends at the last event
  expect(kp('person', noDriver).sub.text).toBe('0:00');
  expect(renderKpis([kp('haiku')], 100000)).toContain('should be 0');
});
