// Adversary cases on the run-page wave (stories run-page-watches-a-live-run, a-budget-stop-in-a-running-run-is-named,
// the-daemon-serves-a-runs-metrics). Each case drives web/runsteps.js with the events a daemon's run writes
// (docs/daemon-api.md, "Event streams"; Loom's run-log lines as the engine reports them) and asserts what the step list
// must say. Synthetic events only, no recorded run, no page.
//
// The lines the cases rest on, as Loom writes them (b10x_loom_runlog): an `ActionFinished` holds an `exit` or a
// `refusal`, exactly one; a program ended by a signal, a timeout or with no exit status is a refusal
// (runbook/src/engine/refusal.rs reads `ProgramSignalled: ...`). A binding with `handoff_when` (bindings/sdc-plan.yaml)
// runs its check before the writer: the engine records `runbook.action-started` with role `check`
// (runbook/src/engine/announce.rs), Loom writes `HandoffChecked` (action, exit); exit 0 stops the run `AwaitingPerson`,
// any other exit hands on and the writer of the same action runs in the same step.
import {test, expect} from '@playwright/test';
import {runSteps, eventWords, renderStepsTable} from '../web/runsteps.js';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure functions: one project is enough'));

const T = (s) => `2026-11-26T18:00:${String(s).padStart(2, '0')}.000Z`;
const ev = (kind, record, s) => ({kind, at: T(s), record});
const SIGNALLED = 'ProgramSignalled: action note.write: t.sh was ended by a signal: exit 137';

test('a program ended by a signal (ActionFinished with a refusal, no exit) is not a step done', () => {
  const m = runSteps([
    ev('runbook.action-started', {case: 'c', action: 'note.write', role: 'program', try: 1}, 0),
    ev('ActionFinished', {seq: 3, kind: 'ActionFinished', action: 'note.write', refusal: SIGNALLED}, 4),
  ]);
  expect(m.steps).toHaveLength(1);
  expect(m.steps[0].state, 'the step list says "done" and counts it among the steps done').not.toBe('done');
});

test('the event of a program ended by a signal says in words that it did not end by itself', () => {
  const w = eventWords(ev('ActionFinished', {seq: 3, kind: 'ActionFinished', action: 'note.write', refusal: SIGNALLED}, 4));
  expect(`${w.name} · ${w.detail}`).toMatch(/signal|refus|killed|no exit/i);
});

test('a handoff check ends at its HandoffChecked line: the writer that runs after it neither ends it nor gives it its exit', () => {
  const m = runSteps([
    ev('runbook.action-started', {case: 'c', action: 'amend.upstream', role: 'check'}, 0),
    ev('HandoffChecked', {seq: 2, kind: 'HandoffChecked', action: 'amend.upstream', exit: 1}, 1),
    ev('runbook.action-started', {case: 'c', action: 'amend.upstream', role: 'program', try: 1}, 2),
    ev('ActionFinished', {seq: 3, kind: 'ActionFinished', action: 'amend.upstream', exit: 3}, 9),
    ev('Stopped', {seq: 4, kind: 'Stopped', stop: 'BudgetSpent', next: [], budget: 'MaxSteps'}, 9),
    ev('runbook.step-ended', {case: 'c', seq: 4}, 9),
  ]);
  const [check, writer] = m.steps;
  expect([writer.role, writer.state, writer.exit, writer.end]).toEqual(['program', 'exit', 3, Date.parse(T(9))]);
  expect(check.role).toBe('check');
  expect(check.end, 'the check ended at its own line (1 s), not at the writer\'s end (9 s)').toBe(Date.parse(T(1)));
  expect(check.exit, 'the check took the writer\'s exit').not.toBe(3);
});

test('a handoff check whose exit 0 makes the run wait on a person is not shown running while the run waits', () => {
  const m = runSteps([
    ev('runbook.action-started', {case: 'c', action: 'amend.upstream', role: 'check'}, 0),
    ev('HandoffChecked', {seq: 2, kind: 'HandoffChecked', action: 'amend.upstream', exit: 0}, 1),
    ev('Stopped', {seq: 3, kind: 'Stopped', stop: 'AwaitingPerson', next: ['amend.upstream']}, 1),
  ]);
  const html = renderStepsTable({state: 'WaitingOnDecision'}, m, Date.parse(T(50)));
  expect(html, 'a row marked running, its duration ticking, while nothing runs').not.toContain('data-state="running"');
});
