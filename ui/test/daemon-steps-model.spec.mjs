// The steps of a daemon run read from its events (web/runsteps.js) and the page's clock words (web/clock.js), on the
// synthetic run of daemon-live-fixture.mjs: no server, no browser. Story run-page-watches-a-live-run.
import {test, expect} from '@playwright/test';
import {fixture, LIVE, DONE, RUNNING_SINCE} from './daemon-live-fixture.mjs';
import {runSteps, eventWords, runSay, fmtTook, renderStepsTable} from '../web/runsteps.js';
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
  expect(runSay(row(LIVE), between, RUNNING_SINCE)).toMatch(/^Between steps: 6 done, the last report\.write/);
  expect(runSay({...row(LIVE), state: 'WaitingOnDecision'}, between, RUNNING_SINCE)).toMatch(/^Waiting on a decision/);
  expect(runSay({...row(LIVE), state: 'Cancelled'}, between, RUNNING_SINCE)).toMatch(/^Cancelled/);
  expect(runSay({...row(LIVE), state: 'FailedToStart', failure: 'no route'}, between, RUNNING_SINCE)).toBe('Failed to start: no route');
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
