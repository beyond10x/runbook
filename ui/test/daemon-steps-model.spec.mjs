// The steps of a daemon run read from its events (web/runsteps.js) and the page's clock words (web/clock.js), on the
// synthetic run of daemon-live-fixture.mjs: no server, no browser. Story run-page-watches-a-live-run.
import {test, expect} from '@playwright/test';
import {fixture, LIVE, DONE, RUNNING_SINCE} from './daemon-live-fixture.mjs';
import {runSteps, stepEnds, eventWords, runSay, fmtTook, renderStepsTable, renderStepsHead, runStateWord, eventsFeed, eventsPane} from '../web/runsteps.js';
import {clockTime, zoneName} from '../web/clock.js';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const f = fixture();
const of = (run) => f.events.filter((e) => e.run_id === run);
const row = (run) => f.runs.find((r) => r.run_id === run);

test('the steps of a live run: in order, each with role, try, start, end and exit; the last one in flight', () => {
  const m = runSteps(of(LIVE));
  expect(m.steps.map((s) => s.action)).toEqual(['subject.read', 'evidence.deploy', 'evidence.tickets', 'evidence.repo', 'evidence.chat', 'report.write', 'findings.judge']);
  expect(m.steps.map((s) => s.state)).toEqual(['done', 'done', 'done', 'exit', 'done', 'done', 'running']);
  expect(m.steps[3]).toMatchObject({exit: 10, role: 'program', try: 1});
  expect(m.steps[2].end - m.steps[2].at).toBe(4000);
  expect(m.inFlight).toMatchObject({action: 'findings.judge', at: RUNNING_SINCE, end: null});
  expect(m.steps[5]).toMatchObject({calls: 1, tokens: 2000, micro: 4200});
  expect(m).toMatchObject({calls: 1, tokens: 2000, micro: 4200, costKnown: true, ended: null});
});

test('an ended run: no step in flight, the end reason, every model call counted', () => {
  const m = runSteps(of(DONE));
  expect(m.inFlight).toBeNull();
  expect(m.steps.every((s) => s.state !== 'running')).toBe(true);
  expect(m.ended).toEqual({reason: 'investigated'});
  expect(m.calls).toBe(2);
  expect(runSay(row(DONE), m, Date.now())).toMatch(/^Ended: investigated/);
});

test('a step that started twice is two steps; a finish closes the last open start of its action', () => {
  const ev = (kind, record) => ({kind, at: '2026-11-26T18:00:00.000Z', record});
  const m = runSteps([ev('runbook.action-started', {case: 'c', action: 'a', role: 'program', try: 1}), ev('ActionFinished', {action: 'a', exit: 3}),
    ev('runbook.action-started', {case: 'c', action: 'a', role: 'program', try: 2}), ev('runbook.action-started', {case: 'c', action: 'b', role: 'check'}), ev('ActionFinished', {action: 'a', exit: 0})]);
  expect(m.steps.map((s) => [s.action, s.try ?? null, s.state])).toEqual([['a', 1, 'exit'], ['a', 2, 'done'], ['b', null, 'running']]);
});

test('the end of a step, with or without the runtime\'s own line: the step-budget stop ends the step in flight, never the run', () => {
  const ev = (kind, record, at = '2026-11-26T18:00:05.000Z') => ({kind, at, record});
  const start = (action, kase = 'c') => ev('runbook.action-started', {case: kase, action, role: 'person', try: 1}, '2026-11-26T18:00:00.000Z');
  const stop = ev('Stopped', {stop: 'BudgetSpent', next: [], budget: 'MaxSteps'});
  const ended = ev('runbook.step-ended', {case: 'c', seq: 4});
  // a step with no line of its own end (no program ran): the stop alone ends it; with the runtime's line after it too
  for (const evs of [[start('a'), stop], [start('a'), stop, ended], [start('a'), ended]]) {
    const m = runSteps(evs);
    expect(m.steps.map((s) => [s.action, s.state, s.end])).toEqual([['a', 'done', Date.parse('2026-11-26T18:00:05.000Z')]]);
    expect(m.inFlight).toBeNull();
    expect(m.ended).toBeNull();
  }
  // after a finish the stop changes nothing: the exit stays, and the next step still runs once it started
  const m = runSteps([start('a'), ev('ActionFinished', {action: 'a', exit: 2}), stop, ended, start('b')]);
  expect(m.steps.map((s) => [s.action, s.state, s.exit])).toEqual([['a', 'exit', 2], ['b', 'running', null]]);
  // a step-ended line of another case leaves this one in flight; a wait ends no step
  expect(runSteps([start('b'), ev('runbook.step-ended', {case: 'other', seq: 1})]).inFlight).toMatchObject({action: 'b'});
  expect(runSteps([start('b'), ev('Stopped', {stop: 'AwaitingPerson', next: []})]).inFlight).toMatchObject({action: 'b'});
});

// The runtime's runbook.step-ended against the step-budget stop (story a-budget-stop-in-a-running-run-is-named): the
// event when the run has it, the line otherwise.
const T = (s) => `2026-11-26T18:00:${String(s).padStart(2, '0')}.000Z`;
const at = (kind, record, s) => ({kind, at: T(s), record});
const begin = (action, s) => at('runbook.action-started', {case: 'c', action, role: 'person', try: 1}, s);
const budgetStop = (seq, s) => at('Stopped', {seq, stop: 'BudgetSpent', next: [], budget: 'MaxSteps'}, s);
const stepEnded = (seq, s) => at('runbook.step-ended', {case: 'c', seq}, s);

test('a run with runbook.step-ended: a step ends at that event, and a step-budget stop no such event names ends no step', () => {
  const one = [begin('a', 0), budgetStop(3, 5), stepEnded(3, 6)];
  expect(runSteps(one).steps.map((s) => [s.action, s.state, s.end])).toEqual([['a', 'done', Date.parse(T(6))]]);
  // the second stop has no step-ended after it: a wait or the end of the tree, so b still runs
  const two = [...one, begin('b', 7), budgetStop(9, 8)];
  const m = runSteps(two);
  expect(m.inFlight).toMatchObject({action: 'b', end: null});
  expect(m.ended).toBeNull();
  // in words: the stop the event names is the engine's own line, the event says the step ended, the other stop is said
  const ends = stepEnds(two);
  expect(eventWords(two[1], ends)).toEqual({name: 'Step budget spent', detail: 'the engine call of one step ended', engine: true});
  expect(eventWords(two[2], ends)).toEqual({name: 'Step ended', detail: 'the run goes on', engine: true});
  expect(eventWords(two[4], ends)).toEqual({name: 'Engine stopped', detail: 'step budget spent, no step ended: a wait or the end of the tree', engine: false});
});

test('a run without runbook.step-ended (an older runtime): the step-budget stop ends the step in flight, said as its end', () => {
  const evs = [begin('a', 0), budgetStop(3, 5), begin('b', 7), budgetStop(9, 8)];
  const m = runSteps(evs);
  expect(m.steps.map((s) => [s.action, s.state, s.end])).toEqual([['a', 'done', Date.parse(T(5))], ['b', 'done', Date.parse(T(8))]]);
  expect(m.inFlight).toBeNull();
  const ends = stepEnds(evs);
  expect(ends.present).toBe(false);
  for (const e of [evs[1], evs[3]]) expect(eventWords(e, ends)).toEqual({name: 'Step ended', detail: 'the run goes on', engine: true});
});

test('the cases of a run: each one it opened; a run of more than one names each step\'s case', () => {
  const ev = (kind, record) => ({kind, at: '2026-11-26T18:00:00.000Z', record});
  expect([...runSteps(of(LIVE)).cases]).toEqual([LIVE]);
  const m = runSteps([ev('CaseOpened', {case: 'r'}), ev('ActionFinished', {action: 'draft.open', exit: 0}), ev('CaseOpened', {case: 'r.draft'}),
    ev('runbook.action-started', {case: 'r.draft', action: 'draft.write', role: 'program', try: 1})]);
  expect([...m.cases]).toEqual(['r', 'r.draft']);
  const html = renderStepsTable({state: 'Running'}, m, Date.now(), (c) => (c === 'r.draft' ? 'draft' : null));
  expect(html).toContain('<th scope="col">Case</th>');
  expect([...html.matchAll(/<td class="ds-case">([^<]*)<\/td>/g)].map((x) => x[1])).toEqual(['—', 'draft']);
  expect(renderStepsTable({state: 'Running'}, runSteps(of(LIVE)), Date.now())).not.toContain('Case</th>');
});

test('every event in words: a name of words, the record\'s meaning, no digest; the engine\'s own lines are marked', () => {
  const words = of(LIVE).map(eventWords);
  for (const w of words) {
    expect(w.name).toMatch(/^[A-Z][a-z]+( [a-z]+)*$/);
    expect(w.detail).not.toMatch(/[0-9a-f]{32}|\{"/);
  }
  const by = (kind) => eventWords(of(LIVE).find((e) => e.kind === kind));
  expect(by('runbook.action-started')).toEqual({name: 'Step started', detail: 'subject.read · program · try 1', engine: false});
  expect(by('ActionFinished')).toEqual({name: 'Step finished', detail: 'subject.read · exit 0', engine: false});
  // the engine runs each step as one call that stops on its step budget: that line ends one step, never the run
  expect(by('Stopped')).toEqual({name: 'Step ended', detail: 'the run goes on', engine: true});
  expect(eventWords({kind: 'runbook.step-ended', record: {case: 'c', seq: 9}})).toEqual({name: 'Step ended', detail: 'the run goes on', engine: true});
  expect(by('Evaluated').engine).toBe(true);
  expect(by('runbook.model-call').detail).toBe('report.write · 1,200 in, 800 out · $0.0042');
  // any other stop is said for what it is: a wait is a wait
  expect(eventWords({kind: 'Stopped', record: {stop: 'AwaitingPerson', next: []}})).toEqual({name: 'Waiting', detail: 'on a person', engine: false});
  expect(eventWords({kind: 'Stopped', record: {stop: 'AwaitingAuthority', next: []}})).toEqual({name: 'Waiting', detail: 'on an authority', engine: false});
  expect(eventWords({kind: 'Stopped', record: {stop: 'Stuck', next: []}})).toEqual({name: 'Stuck', detail: 'no step can be taken', engine: false});
  expect(eventWords({kind: 'Stopped', record: {stop: 'BudgetSpent', budget: 'WallSeconds'}})).toEqual({name: 'Budget spent', detail: 'WallSeconds', engine: false});
  for (const r of [{stop: 'BudgetSpent', budget: 'MaxSteps'}, {stop: 'AwaitingPerson'}, {stop: 'Stuck'}]) expect(eventWords({kind: 'Stopped', record: r}).name).not.toBe('Stopped');
  expect(eventWords({kind: 'SomethingNew', record: {x: 1}}).name).toBe('Something new');
});

test('what runs now, in words', () => {
  const live = runSteps(of(LIVE));
  expect(runSay(row(LIVE), live, RUNNING_SINCE + 132000, {clock: () => '19:53:17 CET'})).toBe('Running step 7: findings.judge, program, try 1, since 19:53:17 CET, for 2 min 12 s');
  const between = runSteps(of(LIVE).slice(0, -2));
  expect(runSay(row(LIVE), between, RUNNING_SINCE)).toMatch(/^Between steps: 5 done, 1 exited non-zero, the last report\.write/);
  expect(runSay({...row(LIVE), state: 'WaitingOnDecision'}, between, RUNNING_SINCE)).toMatch(/^Waiting on a decision/);
  expect(runSay({...row(LIVE), state: 'Cancelled'}, between, RUNNING_SINCE)).toMatch(/^Cancelled/);
  expect(runSay({...row(LIVE), state: 'FailedToStart', failure: 'no route'}, between, RUNNING_SINCE)).toBe('Failed to start: no route');
  // a run held behind another run of its serial key: nothing of it started
  const none = runSteps([]);
  expect(runSay({...row(LIVE), state: 'WaitingOnKey', serial_key: 'deploy'}, none, RUNNING_SINCE)).toBe('Waiting for its serial key deploy: nothing of it starts before the runs ahead of it end');
  expect(runStateWord({state: 'WaitingOnKey'})).toBe('Waiting for its key');
  expect(renderStepsTable({state: 'WaitingOnKey'}, none, RUNNING_SINCE)).toContain('No step has started yet');
  expect(eventWords({kind: 'runbook.waiting-on-key', record: {serial_key: 'deploy', behind: LIVE}})).toEqual({name: 'Waiting for its key', detail: 'deploy · behind run 0000a1b2', engine: false});
});

test('durations in words; clock times 24 h with the zone\'s name, a date in ISO form when not today', () => {
  expect([fmtTook(60), fmtTook(4000), fmtTook(9500), fmtTook(61000), fmtTook(132000), fmtTook(3725000)]).toEqual(['0.1 s', '4.0 s', '9.5 s', '1 min 1 s', '2 min 12 s', '1 h 2 min']);
  const t = Date.parse('2026-11-26T18:54:50Z');
  expect(clockTime(t, t + 1000, 'Europe/Berlin')).toBe('19:54:50 CET');
  expect(clockTime(t, t + 86400000, 'Europe/Berlin')).toBe('2026-11-26 19:54:50 CET');
  expect(clockTime(Date.parse('2026-07-01T06:05:04Z'), Date.parse('2026-07-01T07:00:00Z'), 'Europe/Berlin')).toBe('08:05:04 CEST');
  expect(clockTime(t, t, 'America/New_York')).toBe('13:54:50 EST');
  expect(clockTime(t, t, 'UTC')).toBe('18:54:50 UTC');
  expect(zoneName(new Date(t), 'Asia/Tokyo')).toBe('GMT+9');
});

// A program the runtime ended (Loom's `ActionFinished` with a `refusal` and no `exit`; runbook/src/engine/refusal.rs
// `failed_step_in`): the step ended with the refusal's cause, never "done", and the counts say it apart.
const finished = (record, s) => at('ActionFinished', {kind: 'ActionFinished', action: 'note.write', ...record}, s);
const program = (action, s, role = 'program') => at('runbook.action-started', {case: 'c', action, role, ...(role === 'program' ? {try: 1} : {})}, s);
const REFUSALS = [
  ['ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137', 'ended by a signal'],
  ['timed out', 'ended: timed out'],
  ['ended with no exit', 'ended: no exit status'],
  ['ProgramError: action note.write: exit 3 is listed as an error', 'ended: an error exit'],
  ['RunsSpent: action note.write: no run left', 'refused: runs spent'],
  ['something Loom has not said before', 'refused'],
];

test('a program ended with a refusal and no exit: the step is ended with its cause, not done, in the list, the counts and the words', () => {
  for (const [refusal, word] of REFUSALS) {
    const m = runSteps([program('note.write', 0), finished({refusal}, 4)]);
    expect(m.steps.map((s) => [s.state, s.exit, s.cause, s.end]), refusal).toEqual([['ended', null, word, Date.parse(T(4))]]);
    expect(m.inFlight).toBeNull();
    const table = renderStepsTable({state: 'Running'}, m, Date.parse(T(9)));
    expect(table).toContain('data-state="ended"');
    expect(table).toContain(`<td class="ds-st">${word}</td>`);
    const head = renderStepsHead({state: 'Running'}, m, Date.parse(T(9)));
    expect(head, 'an ended step is not a step done').not.toMatch(/\d+ done/);
    expect(head).toContain('<span>1 ended with no exit</span>');
    expect(runSay({state: 'Running'}, m, Date.parse(T(9)))).toBe(`Between steps: 1 ended with no exit, the last note.write ${word}`);
    const w = eventWords(finished({refusal}, 4));
    expect(w.name).toBe('Step refused');
    expect(w.detail).toBe(`note.write · ${word}`);
    expect(w.detail).not.toContain('ProgramSignalled');
  }
  // an exit wins: the line holds exactly one of the two
  expect(runSteps([program('note.write', 0), finished({exit: 0}, 4)]).steps[0].state).toBe('done');
  expect(runSteps([program('note.write', 0), finished({exit: 2}, 4)]).steps[0].state).toBe('exit');
  // a refusal of a step with no start of its own is a step too, ended
  expect(runSteps([finished({refusal: 'timed out'}, 4)]).steps.map((s) => [s.action, s.state, s.cause])).toEqual([['note.write', 'ended', 'ended: timed out']]);
  // a step-budget stop after it leaves it ended
  expect(runSteps([program('note.write', 0), finished({refusal: 'timed out'}, 4), budgetStop(3, 5)]).steps[0].state).toBe('ended');
});

// The steps that are over, counted apart: an exit of 0 is done, any other exit is "exited non-zero", a refusal with no
// exit "ended with no exit"; no step is in two counts. The head's counts and the words of what runs now say the same.
test('the counts of the steps that are over: done, exited non-zero and ended with no exit, each step in one of them', () => {
  const over = (ends) => ends.flatMap((end, i) => [program(`s${i}`, 2 * i), finished({action: `s${i}`, ...end}, 2 * i + 1)]);
  const m = runSteps(over([{exit: 0}, {exit: 3}, {exit: 0}, {refusal: 'timed out'}, {exit: 0}, {exit: 10}, {exit: 0}]));
  expect(m.steps.map((s) => s.state)).toEqual(['done', 'exit', 'done', 'ended', 'done', 'exit', 'done']);
  const now = Date.parse(T(30));
  const head = renderStepsHead({state: 'Running'}, m, now);
  expect(head).toContain('<span>4 done, 2 exited non-zero, 1 ended with no exit</span>');
  expect(runSay({state: 'Running'}, m, now)).toBe('Between steps: 4 done, 2 exited non-zero, 1 ended with no exit, the last s6 done');
  expect(runSay({state: 'Cancelled'}, m, now)).toBe('Cancelled: 4 done, 2 exited non-zero, 1 ended with no exit, the last s6 done');
  // one of each: the singular
  const one = runSteps(over([{exit: 0}, {exit: 2}]));
  expect(renderStepsHead({state: 'Running'}, one, now)).toContain('<span>1 done, 1 exited non-zero</span>');
  expect(runSay({state: 'Running'}, one, now)).toBe('Between steps: 1 done, 1 exited non-zero, the last s1 ended with exit 2');
  // no step over yet: nothing done, said as a count
  expect(renderStepsHead({state: 'Running'}, runSteps([program('s0', 0)]), now)).toContain('<span>0 done</span>');
});

// A handoff check (role `check`, runbook/src/engine/announce.rs `describe`) ends at Loom's `HandoffChecked` with its own
// exit; `ActionFinished` is the writer's and ends no check (runbook/src/run_metrics.rs reads the two the same way).
const checked = (exit, s, action = 'amend.upstream') => at('HandoffChecked', {kind: 'HandoffChecked', action, exit}, s);

test('a handoff check ends at its own HandoffChecked line with its exit; ActionFinished ends only a program', () => {
  const m = runSteps([program('amend.upstream', 0, 'check'), checked(1, 1), program('amend.upstream', 2), finished({action: 'amend.upstream', exit: 0}, 9)]);
  expect(m.steps.map((s) => [s.role, s.state, s.exit, s.end])).toEqual([['check', 'done', 1, Date.parse(T(1))], ['program', 'done', 0, Date.parse(T(9))]]);
  // with no HandoffChecked line yet, the writer's end leaves the check running (the writer had no start: a step of no role)
  const open = runSteps([program('amend.upstream', 0, 'check'), finished({action: 'amend.upstream', exit: 0}, 3)]);
  expect(open.steps.map((s) => [s.role, s.state])).toEqual([['check', 'running'], [null, 'done']]);
  // the newest running check of that action ends, not an older one of another action
  const two = runSteps([program('a', 0, 'check'), program('b', 1, 'check'), checked(0, 2, 'a')]);
  expect(two.steps.map((s) => [s.action, s.state, s.exit])).toEqual([['a', 'done', 0], ['b', 'running', null]]);
  // a check with no start of its own (an older runtime) is a check step, ended
  expect(runSteps([checked(0, 2)]).steps.map((s) => [s.role, s.state, s.exit])).toEqual([['check', 'done', 0]]);
  // in words: what the check's exit does
  expect(eventWords(checked(0, 1))).toEqual({name: 'Check finished', detail: 'amend.upstream · exit 0 · the run waits on a person', engine: false});
  expect(eventWords(checked(1, 1))).toEqual({name: 'Check finished', detail: 'amend.upstream · exit 1 · the writer goes on', engine: false});
  const waits = runSteps([program('amend.upstream', 0, 'check'), checked(0, 1), at('Stopped', {stop: 'AwaitingPerson', next: ['amend.upstream']}, 1)]);
  const table = renderStepsTable({state: 'WaitingOnDecision'}, waits, Date.parse(T(50)));
  expect(table).toContain('<td class="ds-st">checked: the run waits on a person</td>');
  expect(table).not.toContain('data-took-since');
  expect(renderStepsTable({state: 'Running'}, m, Date.parse(T(50)))).toContain('<td class="ds-st">checked: the writer goes on</td>');
});

// The bound of one render of the run below: about 50 ms measured on this host (2026-11-26; 1.9 s before the lists were
// bounded), five times that as the margin for a loaded host. The case prints its time.
const RENDER_MS = 250;
// A run of 5000 steps (30,000 events) renders a bounded page: the latest 200 steps and the step in flight, the latest
// 200 events in the feed and in the Events tab, each list saying how many it left out; one render's model and markup
// stay under a time measured on this host with a wide margin.
function longRun(n, {firstRuns = false} = {}) {
  const ev = [];
  let t = Date.parse('2026-11-26T10:00:00Z');
  const push = (kind, record) => ev.push({position: ev.length + 1, sequence: ev.length + 1, run_id: 'r', kind, at: new Date(t++).toISOString(), record});
  if (firstRuns) push('runbook.action-started', {case: 'c', action: 'watch.all', role: 'program', try: 1});
  for (let i = 0; i < n; i++) {
    const a = `step.${i % 40}`;
    push('Evaluated', {case_revision: 1});
    push('runbook.action-started', {case: 'c', action: a, role: 'program', try: 1});
    push('ActionFinished', {action: a, exit: 0});
    push('Stopped', {seq: i, stop: 'BudgetSpent', next: [], budget: 'MaxSteps'});
    push('runbook.model-call', {step: a, input_tokens: 1, output_tokens: 1, cost_known: true, micro_usd: 1});
    push('runbook.step-ended', {case: 'c', seq: i});
  }
  return {ev, now: t};
}
const rowsOf = (html, re) => (html.match(re) || []).length;

test('a run of 5000 steps: the step table, the feed and the Events tab each draw a bounded list and say what they left out', () => {
  const {ev, now} = longRun(5000);
  const m = runSteps(ev);
  expect(m.steps).toHaveLength(5000);
  const table = renderStepsTable({state: 'Running'}, m, now);
  expect(rowsOf(table, /<tr data-key="st:/g)).toBe(200);
  expect(table).toMatch(/4,?800 earlier steps left out/);
  expect(table).toContain('data-key="st:5000"');
  expect(table).not.toContain('data-key="st:4800"');
  expect(renderStepsHead({state: 'Running'}, m, now), 'the counts are of every step').toMatch(/<span>5,?000 done<\/span>/);
  // the step in flight is always drawn, also when it started before every step in the window
  const old = longRun(5000, {firstRuns: true});
  const mo = runSteps(old.ev);
  expect(mo.inFlight).toMatchObject({n: 1, action: 'watch.all'});
  const tableOld = renderStepsTable({state: 'Running'}, mo, old.now);
  expect(rowsOf(tableOld, /<tr data-key="st:/g)).toBe(201);
  expect(tableOld).toContain('data-key="st:1" data-state="running"');
  // the feed (newest first, engine lines folded or shown) and the Events tab (oldest first)
  for (const all of [false, true]) {
    const feed = eventsFeed(ev, 'r', {all, now});
    expect(rowsOf(feed, /class="fd-row[ "]/g), `feed all=${all}`).toBe(200);
    expect(feed).toMatch(/\d[\d,]* earlier events left out/);
    // the newest event drawn: the runtime's step-ended line when all are shown, the model call before it when folded
    expect(feed).toContain(`data-key="ev:${all ? ev.length : ev.length - 1}"`);
  }
  const pane = eventsPane(ev, now);
  expect(rowsOf(pane, /class="ev-row /g)).toBe(200);
  expect(pane).toMatch(/29,?800 earlier events left out/);
  expect(pane).toContain(`data-key="ev:${ev.length}"`);
  expect(pane).not.toContain(`data-key="ev:${ev.length - 200}"`);
  // one render: the model of the run, the head, the table, the feed and the Events tab
  const once = () => {
    const t0 = performance.now();
    const mm = runSteps(ev);
    const html = renderStepsHead({state: 'Running'}, mm, now) + renderStepsTable({state: 'Running'}, mm, now) + eventsFeed(ev, 'r', {now}) + eventsPane(ev, now);
    return {ms: performance.now() - t0, bytes: html.length};
  };
  once();
  const best = [once(), once(), once()].sort((a, b) => a.ms - b.ms)[0];
  console.log(`a run of 5000 steps: one render ${best.ms.toFixed(1)} ms, ${best.bytes} bytes of markup`);
  expect(best.bytes).toBeLessThan(400000);
  expect(best.ms).toBeLessThan(RENDER_MS);
});
