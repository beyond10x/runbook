// The in-flight derivation of web/now.js, on hand-made cases: no server, no browser.
import {test, expect} from '@playwright/test';
import {inFlight, nowState, waitOf, recentSteps, executorOf, phaseOf, parseDiff, fmtDur, fmtAgo, changesFor, driverState, renderNow} from '../web/now.js';
import {highlightLine, langOf} from '../web/highlight.js';
import {unifiedDiff} from '../udiff.mjs';
import {useSdcView} from './sdc-view.mjs';
useSdcView();

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const ir = {protocol: {id: 'sdc.story', revision: 1},
  actions: {'change.implement': {may_produce: [{evidence: 'change_built'}]}, 'tests.run': {may_produce: [{evidence: 'tests_result'}]}, 'change.merge': {requires: [{capability: 'owner.approve'}], may_produce: [{evidence: 'merge_record'}]}},
  claims: {'change.built': {true_when: {evidence: {kind: 'change_built', subject: 'impl'}}}, 'tests.pass': {true_when: {evidence: {kind: 'tests_result', subject: 'impl'}}}, 'change.merged': {true_when: {evidence: {kind: 'merge_record', subject: 'impl'}}}}};
const bindings = {'change.implement': {argv: ['/x/tools/llm-node.sh', 'change.implement'], llm: {model: 'claude-haiku-5-5', max_turns: 30}, subject: 'impl'},
  'tests.run': {argv: ['/x/tools/sdc-tool.sh', 'run-tests'], subject: 'impl'}, 'change.merge': {argv: ['/x/tools/sdc-tool.sh', 'merge'], subject: 'impl'}, 'spec.copy': {copy: {kind: 'spec_source', from: 'IDEA-1.spec'}}};
const mk = (id, events, extra = {}) => ({id, rel: id, depth: extra.parent ? 1 : 0, parent: extra.parent || null, ir, bindings, case: {protocol: 'sdc.story', artifacts: {impl: {revision: 'tabc123'}}}, evidence: {}, evaluations: {}, times: {}, events: events.map((e, i) => ({seq: i, at: '2026-11-21T10:00:00Z', ...e})), ...extra});

test('started without finished is in flight; finished, verdict or end closes it', () => {
  const a = mk('A', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}]);
  const b = mk('B', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}, {event: 'action_finished', action: 'tests.run', exit: '0'}]);
  const c = mk('C', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}, {event: 'stuck', next: []}]);
  const d = mk('D', [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true}, {event: 'evaluated', file: 'x'}]);
  expect(inFlight([a, b, c, d]).map((x) => `${x.c.id}:${x.start.action}`)).toEqual(['A:tests.run', 'D:change.implement']);
});

test('several cases in flight at once, each with its executor and start time', () => {
  const a = mk('S1', [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true, argv: bindings['change.implement'].argv}], {times: {1: 1000}});
  const b = mk('S2', [{event: 'opened'}, {event: 'action_started', action: 'tests.run', argv: bindings['tests.run'].argv}], {times: {1: 2000}});
  const st = nowState([a, b], {records: []}, null, 5000);
  expect(st.mode).toBe('running');
  expect(st.running.map((r) => [r.c.id, r.kind, r.since])).toEqual([['S1', 'llm', 1000], ['S2', 'program', 2000]]);
  expect(st.running[0].exec.model).toBe('claude-haiku-5-5');
  expect(st.running[1].exec.detail).toBe('sdc-tool.sh run-tests');
});

test('no server time: the start is approximate, from the run’s --at', () => {
  const a = mk('A', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}]);
  const r = nowState([a], {records: []}).running[0];
  expect(r.since).toBe(null);
  expect(r.approx).toBe(Date.parse('2026-11-21T10:00:00Z'));
});

test('an open agent call joins its in-flight action and gives the progress', () => {
  const a = mk('S1', [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true}], {times: {1: 10000}});
  const old = {kind: 'llm', rel: 'llm/003-change.implement', n: 3, action: 'change.implement', started: 2000, model: 'm-old', turns: 1, tools: 0};
  const cur = {kind: 'llm', rel: 'llm/007-change.implement', n: 7, action: 'change.implement', started: 10400, model: 'claude-haiku-5-5', turns: 5, tools: 4, maxTurns: 30};
  const other = {kind: 'llm', rel: 'llm/008-spec.author', n: 8, action: 'spec.author', started: 10500};
  const r = nowState([a], {records: [old, cur, other]}).running[0];
  expect(r.record.rel).toBe('llm/007-change.implement');
  expect(r.exec.model).toBe('claude-haiku-5-5');
});

test('two cases running the same agent action pair with their calls in start order', () => {
  const a = mk('S1', [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true}], {times: {1: 1000}});
  const b = mk('S2', [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true}], {times: {1: 5000}});
  const r1 = {kind: 'llm', rel: 'llm/001-change.implement', n: 1, action: 'change.implement', started: 1100};
  const r2 = {kind: 'llm', rel: 'llm/002-change.implement', n: 2, action: 'change.implement', started: 5100};
  const st = nowState([a, b], {records: [r2, r1]});
  expect(st.running.map((r) => `${r.c.id}=${r.record.rel}`)).toEqual(['S1=llm/001-change.implement', 'S2=llm/002-change.implement']);
});

test('a wait names who, for what capability and subject; a decider call in progress takes it', () => {
  const next = [{action: 'change.merge', command: 'sdc-drive --case-dir /s/S1 approve --at "$(date)" --capability owner.approve --principal operator --action change.merge', decides: 'approve impl revision tabc123 (read /s/S1/work/impl)'}];
  const a = mk('S1', [{event: 'opened'}, {event: 'awaiting_authority', next}], {times: {1: 3000}});
  const w = waitOf(a);
  expect(w.asks[0]).toMatchObject({action: 'change.merge', capability: 'owner.approve', principal: 'operator', subject: 'impl', revision: 'tabc123', handoff: false});
  let st = nowState([a], {records: []});
  expect(st.mode).toBe('waiting');
  expect(st.waiting[0]).toMatchObject({kind: 'person', since: 3000});
  st = nowState([a], {records: [{kind: 'decider', rel: 'decider/004-change.merge', case: 'S1', action: 'change.merge', started: 3200, model: 'claude-haiku-5-5'}]});
  expect(st.waiting[0].kind).toBe('decider');
  expect(st.waiting[0].record.rel).toBe('decider/004-change.merge');
  // a decider call for another case does not
  st = nowState([a], {records: [{kind: 'decider', rel: 'decider/005-x', case: 'S9', action: 'change.merge'}]});
  expect(st.waiting[0].kind).toBe('person');
});

test('nothing running: closed, stuck, terminable and between steps are told apart', () => {
  const closed = mk('R', [{event: 'opened'}, {event: 'terminable', next: []}, {event: 'terminated', outcome: 'released'}]);
  expect(nowState([closed], {}).mode).toBe('closed');
  expect(nowState([closed], {}).term.outcome).toBe('released');
  expect(nowState([mk('R', [{event: 'opened'}, {event: 'stuck', next: []}])], {}).mode).toBe('stuck');
  expect(nowState([mk('R', [{event: 'opened'}, {event: 'terminable', next: []}, {event: 'evaluated', file: 'e'}])], {}).mode).toBe('terminable');
  expect(nowState([mk('R', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}, {event: 'action_finished', action: 'tests.run', exit: '0'}])], {}).mode).toBe('between');
});

test('just finished: newest first, with observed durations, else the LLM call’s', () => {
  const a = mk('S1', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}, {event: 'action_finished', action: 'tests.run', exit: '0', evidence: ['e0002-tests.run']},
    {event: 'action_started', action: 'change.implement', llm: true}, {event: 'action_finished', action: 'change.implement', exit: '0', evidence: []}], {times: {1: 1000, 2: 3500}});
  a.evidence['e0002-tests.run'] = {kind: 'tests_result', result: 'pass'};
  const run = {llm: [{record: {action: 'change.implement', case: 'S1', duration_ms: 42000}}]};
  const r = recentSteps([a], run, 4);
  expect(r.map((s) => s.end.action)).toEqual(['change.implement', 'tests.run']);
  expect(r[0]).toMatchObject({dur: 42000, durSrc: 'llm-calls.jsonl', kind: 'llm'});
  expect(r[1]).toMatchObject({dur: 2500, durSrc: 'observed', result: 'tests_result = pass'});
  // both ends dated by the driver's observed.jsonl say so
  const b = mk('S2', [{event: 'opened'}, {event: 'action_started', action: 'tests.run'}, {event: 'action_finished', action: 'tests.run', exit: '0'}], {times: {1: 1000, 2: 3500}, timeSource: {1: 'driver', 2: 'driver'}});
  expect(recentSteps([b], null, 1)[0]).toMatchObject({dur: 2500, durSrc: 'driver observed_at'});
});

const drv = (extra = {}) => ({file: false, pid: null, started_at: null, argv: null, alive: false, lock_pid: null, lock_alive: false, observed: false, ...extra});
const started = (id, extra = {}) => mk(id, [{event: 'opened'}, {event: 'action_started', action: 'tests.run', argv: bindings['tests.run'].argv}], {tree: 'S', times: {1: 2000}, ...extra});

test('driver state: driver.json with a live pid, a dead pid, a lock only, observed without a file, an old run', () => {
  const c = started('S');
  const live = (d) => ({drivers: {S: d}});
  expect(driverState(c, live(drv({file: true, pid: 77, started_at: '2026-11-21T10:00:00.000Z', alive: true, argv: ['sdc-drive', 'run']})))).toMatchObject({state: 'attached', pid: 77, started: Date.parse('2026-11-21T10:00:00.000Z'), via: 'driver.json'});
  expect(driverState(c, live(drv({file: true, pid: 77, alive: false})))).toMatchObject({state: 'gone', why: 'driver.json names pid 77, which is not running'});
  expect(driverState(c, live(drv({lock_pid: '5', lock_alive: true, observed: true})))).toMatchObject({state: 'attached', pid: '5', via: 'lock'});
  expect(driverState(c, live(drv({observed: true})))).toMatchObject({state: 'gone'});
  expect(driverState(c, live(drv()))).toEqual({state: 'unknown'});
  expect(driverState(c, {})).toEqual({state: 'unknown'});
  expect(driverState(c, {drivers: {}})).toEqual({state: 'unknown'});
});

test('a started step with a gone driver is not running; with a live or unknown driver it is', () => {
  const a = started('S');
  const gone = nowState([a], {records: [], drivers: {S: drv({file: true, pid: 9, alive: false})}});
  expect(gone.mode).toBe('gone');
  expect(gone.running).toEqual([]);
  expect(gone.gone.map((r) => [r.c.id, r.action, r.driver.state])).toEqual([['S', 'tests.run', 'gone']]);
  expect(nowState([a], {records: [], drivers: {S: drv({observed: true})}}).mode).toBe('gone');
  const up = nowState([a], {records: [], drivers: {S: drv({file: true, pid: 9, alive: true})}});
  expect(up.mode).toBe('running');
  expect(up.driver.attached).toHaveLength(1);
  const old = nowState([a], {records: []});
  expect(old.mode).toBe('running');
  expect(old.driver).toEqual({attached: [], gone: []});
  const html = (st) => renderNow(st, 9000);
  expect(html(gone)).toContain('Driver gone');
  expect(html(gone)).not.toContain('no driver attached');
  expect(html(gone)).toMatch(/a started step has no live driver, last activity [\d: APM]+/);
  expect(html(gone)).not.toContain('>Running<');
  expect(html(up)).toContain('driver attached');
  expect(html(up)).toContain('>Running<');
  const o = html(old);
  expect(o).toContain('>Running<');
  expect(o).not.toContain('now-drv');
});

test('the observed start of a step dates it, and says where the time came from', () => {
  const a = started('S', {times: {1: 5000}, timeSource: {1: 'driver'}});
  const r = nowState([a], {records: []}).running[0];
  expect(r).toMatchObject({since: 5000, sinceSrc: 'driver'});
  expect(renderNow(nowState([a], {records: []}), 9000)).toContain('the driver’s observed_at');
  const b = started('S', {times: {1: 5000}});
  expect(nowState([b], {records: []}).running[0].sinceSrc).toBe('server');
});

test('an agent call joins its case by request.json.case when present, else by action and start order', () => {
  const llm = (id, extra) => mk(id, [{event: 'opened'}, {event: 'action_started', action: 'change.implement', llm: true}], {times: {1: 1000}, ...extra});
  const a = llm('S1'), b = llm('S2');
  // S1's call started later than S2's, and names its case: the case wins over the start order
  const r1 = {kind: 'llm', rel: 'llm/001-change.implement', n: 1, action: 'change.implement', started: 1100, case: 'S2', caseFrom: 'request.json'};
  const r2 = {kind: 'llm', rel: 'llm/002-change.implement', n: 2, action: 'change.implement', started: 1200, case: 'S1', caseFrom: 'request.json'};
  const st = nowState([a, b], {records: [r1, r2]});
  expect(st.running.map((r) => `${r.c.id}=${r.record.rel}`).sort()).toEqual(['S1=llm/002-change.implement', 'S2=llm/001-change.implement']);
  // a call of another case is not taken
  expect(nowState([a], {records: [{...r1}]}).running[0].record).toBe(null);
  // without a case in request.json the old pairing by start order still holds
  const o1 = {kind: 'llm', rel: 'llm/001-change.implement', n: 1, action: 'change.implement', started: 1100};
  const o2 = {kind: 'llm', rel: 'llm/002-change.implement', n: 2, action: 'change.implement', started: 1200};
  expect(nowState([a, b], {records: [o2, o1]}).running.map((r) => r.record.rel)).toEqual(['llm/001-change.implement', 'llm/002-change.implement']);
});

test('a wait on scope.answer names the question it belongs to', () => {
  const next = [{action: 'scope.answer', command: 'sdc-drive --case-dir /s/S1 approve --at "$(date)" --capability scope.answer --principal operator --action scope.answer', decides: 'approve questions revision tq1 (read /s/S1/work/questions.yaml)'}];
  const scope = {questions: [{id: 'q2', question: 'Who may cancel an invoice?', status: 'open', answer: null, answered_by: null, awaiting: true},
    {id: 'q4', question: 'Which currency?', status: 'changed', answer: 'EUR', answered_by: null, awaiting: false}]};
  const a = mk('S1', [{event: 'opened'}, {event: 'awaiting_authority', next}], {scope, times: {1: 3000}});
  const w = nowState([a], {records: []}).waiting[0];
  expect(w).toMatchObject({action: 'scope.answer', capability: 'scope.answer'});
  expect(w.questions.map((q) => q.id)).toEqual(['q2']);
  const html = renderNow(nowState([a], {records: []}), 9000);
  expect(html).toContain('Who may cancel an invoice?');
  expect(html).not.toContain('Which currency?');
  // every question answered, the approval alone waits: the answered ones are shown
  scope.questions[0] = {...scope.questions[0], status: 'accepted', answer: 'x', awaiting: false};
  expect(waitOf(a).asks[0].questions.map((q) => q.id)).toEqual(['q2', 'q4']);
  // an old case (no c.scope from the server) shows no question
  expect(waitOf(mk('S1', [{event: 'opened'}, {event: 'awaiting_authority', next}])).asks[0].questions).toEqual([]);
});

test('executor kinds: copy, driver, program, LLM', () => {
  const c = mk('S1', []);
  expect(executorOf(c, {action: 'spec.copy', copy: {kind: 'spec_source', from: 'IDEA-1.spec'}}).kind).toBe('copy');
  expect(executorOf(c, {action: 'x.open', argv: []}).kind).toBe('driver');
  expect(executorOf(c, {action: 'tests.run', argv: bindings['tests.run'].argv})).toMatchObject({kind: 'program', detail: 'sdc-tool.sh run-tests'});
  expect(executorOf(c, {action: 'change.implement', llm: true, argv: bindings['change.implement'].argv}, {model: 'opus'}).model).toBe('opus');
});

test('the phase of an action comes from the claims its evidence establishes', () => {
  const c = mk('S1', []);
  expect(phaseOf(c, 'change.implement')).toBe('implement');
  expect(phaseOf(c, 'tests.run')).toBe('test');
  expect(phaseOf(c, 'change.merge')).toBe('merge');
});

test('file changes join their action by record, else by time', () => {
  const live = {fs: {changes: [
    {seq: 1, at: '2026-11-21T10:00:01Z', attribution: {case: 'S1', action: 'change.implement', record: 'llm/001-change.implement'}},
    {seq: 2, at: '2026-11-21T10:05:01Z', attribution: {case: 'S1', action: 'change.implement', record: 'llm/002-change.implement'}},
    {seq: 3, at: '2026-11-21T10:05:02Z', attribution: {case: 'S1', action: 'tests.run', record: null}}]}};
  expect(changesFor(live, 'S1', 'change.implement', 'llm/002-change.implement', null).map((x) => x.seq)).toEqual([2]);
  expect(changesFor(live, 'S1', 'tests.run', null, Date.parse('2026-11-21T10:05:00Z')).map((x) => x.seq)).toEqual([3]);
});

test('unified diff, parsed back with line numbers', () => {
  const d = unifiedDiff('a\nb\nc\nd\n', 'a\nB\nc\nd\ne\n', 'f.txt', 'f.txt');
  expect(d).toContain('@@ -1,4 +1,5 @@');
  const rows = parseDiff(d);
  expect(rows.filter((r) => r.t === '-').map((r) => [r.a, r.text])).toEqual([[2, 'b']]);
  expect(rows.filter((r) => r.t === '+').map((r) => [r.b, r.text])).toEqual([[2, 'B'], [5, 'e']]);
  expect(unifiedDiff(null, 'x\n', 'n', 'n')).toContain('@@ -0,0 +1,1 @@');
});

test('highlighting: languages by path, tokens by class', () => {
  expect(['a.rs', 'b.ts', 'c.mjs', 'd.yaml', 'e.json', 'f.md', 'g.sh', 'Cargo.toml', 'Cargo.lock'].map(langOf)).toEqual(['rust', 'js', 'js', 'yaml', 'json', 'md', 'sh', 'toml', 'toml']);
  expect(highlightLine('pub fn add(x: u32) -> u32 { x + 1 } // sum', 'rust')).toMatch(/hl-k">pub<.*hl-k">fn<.*hl-f">add<.*hl-n">1<.*hl-c">\/\/ sum/);
  expect(highlightLine('  name: "todo" # the list', 'yaml')).toMatch(/hl-y">  name<.*hl-s">&quot;todo&quot;<.*hl-c"># the list/);
  expect(highlightLine('[package]', 'toml')).toContain('hl-t');
  expect(highlightLine('echo "$HOME" && ls', 'sh')).toMatch(/hl-f">echo<.*hl-s">/);
  const s = {};
  expect(highlightLine('/* open', 'js', s)).toContain('hl-c');
  expect(highlightLine('still inside */ const x = 1', 'js', s)).toMatch(/hl-c">still inside \*\/<.*hl-k">const/);
  expect(highlightLine('<b>', 'rust')).toBe('<span class="hl-o">&lt;</span>b<span class="hl-o">&gt;</span>');
});

test('formatting of elapsed and relative times', () => {
  expect(fmtDur(7000)).toBe('0:07');
  expect(fmtDur(161000)).toBe('2:41');
  expect(fmtDur(3723000)).toBe('1:02:03');
  expect(fmtAgo(2000)).toBe('just now');
  expect(fmtAgo(42000)).toBe('42 s ago');
  expect(fmtAgo(180000)).toBe('3 min ago');
});
