// The steps of a daemon's run, read from the run's events (the stream the page already reads; docs/daemon-api.md, "The
// step in flight" and "Event streams"), the words for each event, and the bounded lists the page draws of both (the step
// table, the feed and the Events tab: the latest STEP_ROWS steps and EVENT_ROWS events). Story run-page-watches-a-live-run.
//   runbook.action-started  a bound program starts: case, action, role, try. A step begins. Role `check`: the handoff
//                           check of a binding (runbook/src/engine/announce.rs), which runs before the writer.
//   ActionFinished          Loom's line when an action ends: action and an exit or a refusal. The open steps of that
//                           action that are not checks end; an action that ran no program (open, person) is a step
//                           with no start. A refusal and no exit (a signal, a timeout, no exit status, another refusal;
//                           runbook/src/engine/refusal.rs `failed_step_in`): the step is `ended` with the refusal's
//                           cause, never done.
//   HandoffChecked          Loom's line when a handoff check ends: action, exit. The newest running check of that
//                           action ends with that exit (0: the run waits on a person; another: the writer goes on).
//   runbook.model-call      a model call of a step: step, input_tokens, output_tokens, micro_usd, cost_known.
//   Evaluated, Stopped      the engine's own lines around each step: the runtime runs each step as one engine call
//                           that stops on its step budget, so every step is followed by `Stopped` with
//                           `BudgetSpent`/`MaxSteps`. That line ends one step, never the run. Any other stop is said
//                           for what it is (a wait is a wait).
//   runbook.step-ended      the runtime's own line after the stop it took as one step's end: case, seq. It closes the
//                           last step started of that case when that one still runs (a step with no program has no
//                           `ActionFinished`). In a run that has it, a step-budget stop it does not name is a wait or
//                           the end of the tree and ends no step; in a run without it (an older runtime) the stop
//                           itself closes the step (stepEnds).
//   Terminated              the case ended, with its reason.
// Nothing here reads a digest or an id of a record: a person reads names, numbers and times.
import {esc} from './lenses.js';
import {clockTime, fullTime} from './clock.js';

const ms = (t) => { const v = Date.parse(t); return Number.isNaN(v) ? null : v; };
const num = (v) => (typeof v === 'number' && Number.isFinite(v) ? v : null);
const str = (v) => (typeof v === 'string' && v ? v : null);
const STEP_END = (r) => r?.stop === 'BudgetSpent' && r?.budget === 'MaxSteps';

// The ends of steps a run's events name (story a-budget-stop-in-a-running-run-is-named; docs/daemon-api.md, "Event
// streams"): the runtime records `runbook.step-ended` (case, seq) after the step-budget `Stopped` line it took as the end
// of one step, before the next run-log line. {present, named}: present, the run has such an event (its runtime writes
// them); named, the `Stopped` lines one names (the nearest step-budget stop before it, of its seq). In a run that has
// them, a step-budget stop none names is a wait or the end of the tree, not a step's end; in a run without them (an
// older runtime) the line itself ends the step.
export function stepEnds(events) {
  const list = events || [];
  const named = new Set();
  let present = false;
  list.forEach((e, i) => {
    if (e?.kind !== 'runbook.step-ended') return;
    present = true;
    const seq = num(e.record?.seq);
    for (let j = i - 1; j >= 0 && list[j]?.kind !== 'runbook.step-ended'; j--) {
      const x = list[j];
      if (x?.kind === 'Stopped' && STEP_END(x.record) && (seq == null || num(x.record?.seq) == null || x.record.seq === seq)) { named.add(x); break; }
    }
  });
  return {present, named};
}

// The cause of a program's end that Loom wrote as a refusal (`ActionFinished` with `refusal` and no `exit`), in the
// step list's words: the texts runbook/src/engine/refusal.rs reads (`PROGRAM_REFUSALS`, `TIMED_OUT`, `NO_EXIT`), any
// other refusal by its lead word (`RunsSpent: ...` → `refused: runs spent`), else `refused`.
const REFUSED = [[/^ProgramSignalled:/, 'ended by a signal'], [/^timed out$/, 'ended: timed out'], [/^ended with no exit$/, 'ended: no exit status'], [/^ProgramError:/, 'ended: an error exit']];
export function refusalCause(text) {
  const t = String(text);
  for (const [re, word] of REFUSED) if (re.test(t)) return word;
  const lead = /^([A-Z][A-Za-z]+):/.exec(t);
  return lead ? `refused: ${lead[1].replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase()}` : 'refused';
}
// a check's exit in words: 0 stops the run for a person, any other hands on to the writer of the same action
const checkWords = (exit) => (exit === 0 ? 'the run waits on a person' : 'the writer goes on');

// {steps, cases, inFlight, calls, tokens, tokensKnown, micro, costKnown, ended, stop, failed} of one run's events, in
// their order. A step's state: running, done, exit (another exit than 0) or ended (a refusal and no exit; `cause` says
// it). tokens and micro: the sum of the counts the calls report; tokensKnown, costKnown: every call reported one.
// cases: the ids of the cases the run opened (`CaseOpened`). One walk over the events with an index per action and per
// case, so a run of thousands of steps costs one pass.
export function runSteps(events) {
  const steps = [];
  // a step-budget stop ends a step only in a run whose runtime names no step's end (stepEnds)
  const ends = stepEnds(events);
  const m = {steps, cases: new Set(), inFlight: null, calls: 0, tokens: 0, tokensKnown: true, micro: 0, costKnown: true, ended: null, stop: null, failed: null};
  // the running steps of an action: programs (ended by ActionFinished) and checks (ended by HandoffChecked) apart
  const running = {program: new Map(), check: new Map()};
  const open = (kind, action) => { const l = (running[kind].get(action) || []).filter((s) => s.state === 'running'); running[kind].set(action, l); return l; };
  // the newest step of an action (a model call's), of a case, and of no case
  const lastOfAction = new Map(), lastOfCase = new Map();
  let lastNoCase = null;
  const add = (s) => {
    steps.push(s);
    lastOfAction.set(s.action, s);
    if (s.case == null) lastNoCase = s; else lastOfCase.set(s.case, s);
    if (s.state === 'running') open(s.role === 'check' ? 'check' : 'program', s.action).push(s);
  };
  const step = (o) => ({n: steps.length + 1, case: null, role: null, try: null, at: null, end: null, exit: null, cause: null, state: 'running', calls: 0, tokens: 0, tokensKnown: true, micro: 0, costKnown: true, ...o});
  // the end of a step: the last step started (of the case named, when one is) ends here if it still runs
  const stepEnd = (kase, at) => {
    const own = kase == null ? steps[steps.length - 1] : lastOfCase.get(kase);
    const s = kase == null ? own : [own, lastNoCase].filter(Boolean).sort((a, b) => b.n - a.n)[0];
    if (s?.state === 'running') Object.assign(s, {end: at, state: 'done'});
  };
  for (const e of events || []) {
    const r = e?.record && typeof e.record === 'object' ? e.record : {};
    const at = ms(e?.at);
    if (e?.kind === 'runbook.action-started') {
      add(step({action: str(r.action) ?? 'a step', case: str(r.case), role: str(r.role), try: num(r.try), at}));
    } else if (e?.kind === 'ActionFinished') {
      const action = str(r.action) ?? 'a step';
      const exit = num(r.exit);
      const cause = exit == null && str(r.refusal) ? refusalCause(r.refusal) : null;
      const state = cause ? 'ended' : exit == null || exit === 0 ? 'done' : 'exit';
      const list = open('program', action);
      if (!list.length) add(step({action, end: at, exit, cause, state}));
      for (const s of list) Object.assign(s, {end: at, exit, cause, state});
      running.program.set(action, []);
    } else if (e?.kind === 'HandoffChecked') {
      const action = str(r.action) ?? 'a step';
      const exit = num(r.exit);
      const list = open('check', action);
      const s = list[list.length - 1];
      if (s) Object.assign(s, {end: at, exit, state: 'done'});
      else add(step({action, role: 'check', end: at, exit, state: 'done'}));
    } else if (e?.kind === 'runbook.model-call') {
      // a token count or a cost the call does not report is not known: it adds nothing and says so (tokensKnown, costKnown)
      const counted = num(r.input_tokens) != null && num(r.output_tokens) != null;
      const tokens = (num(r.input_tokens) ?? 0) + (num(r.output_tokens) ?? 0);
      const costKnown = r.cost_known !== false && num(r.micro_usd) != null;
      const micro = costKnown ? r.micro_usd : 0;
      m.calls++; m.tokens += tokens; m.micro += micro;
      if (!counted) m.tokensKnown = false;
      if (!costKnown) m.costKnown = false;
      const s = lastOfAction.get(r.step);
      if (s) { s.calls++; s.tokens += tokens; s.micro += micro; if (!counted) s.tokensKnown = false; if (!costKnown) s.costKnown = false; }
    } else if (e?.kind === 'CaseOpened') { if (str(r.case)) m.cases.add(r.case); }
    else if (e?.kind === 'Terminated') m.ended = {reason: str(r.reason)};
    else if (e?.kind === 'Stopped') { if (STEP_END(r)) { if (!ends.present) stepEnd(null, at); } else m.stop = r; }
    else if (e?.kind === 'runbook.step-ended') stepEnd(str(r.case), at);
    else if (e?.kind === 'runbook.step-failed') m.failed = r;
  }
  m.inFlight = null;
  for (let i = steps.length - 1; i >= 0 && !m.inFlight; i--) if (steps[i].state === 'running') m.inFlight = steps[i];
  return m;
}

// The model calls of the steps whose events name none, from the row of the daemon's view run-metrics (ui/daemon/metrics.js
// draws the row; docs/daemon-api.md, "The metrics of a run"): a run on no route books its calls in `runbook.cost-booked`,
// which the steps here do not read. A step of the view is one action of one case in one role over all its tries; its
// calls go to the newest step of `m` with that action (and case and role where the step names them) when none of those
// has a call of its own, and to the run's counts. A number the row leaves out is not known: it adds nothing and says so,
// never 0. No row: `m` as it is.
const count = (v) => (Number.isSafeInteger(v) && v >= 0 ? v : null);
export function withViewCalls(m, row) {
  const byAction = new Map();
  for (const s of m.steps) { if (!byAction.has(s.action)) byAction.set(s.action, []); byAction.get(s.action).push(s); }
  for (const v of Array.isArray(row?.steps) ? row.steps : []) {
    const c = v?.calls;
    const calls = count(c?.model_calls);
    if (!calls) continue;
    const mine = (byAction.get(v.action) || []).filter((s) => (s.case == null || s.case === v.case_id) && (s.role == null || s.role === v.role));
    if (!mine.length || mine.some((s) => s.calls)) continue;
    const input = count(c.input_tokens), output = count(c.output_tokens);
    const tokensKnown = input != null && output != null && count(c.calls_without_tokens) === 0;
    const tokens = (input ?? 0) + (output ?? 0);
    const micro = count(c.cost_micro_usd) ?? 0;
    const costKnown = count(c.cost_micro_usd) != null && count(c.unknown_cost_calls) === 0;
    Object.assign(mine[mine.length - 1], {calls, tokens, tokensKnown, micro, costKnown});
    m.calls += calls; m.tokens += tokens; m.micro += micro;
    if (!tokensKnown) m.tokensKnown = false;
    if (!costKnown) m.costKnown = false;
  }
  return m;
}

// A duration in words: `0.1 s`, `4.0 s`, `2 min 12 s`, `1 h 2 min`; whole seconds under a minute when `whole`.
export function fmtTook(t, whole = false) {
  if (t == null || Number.isNaN(t)) return '—';
  const v = Math.max(0, t);
  if (v < 60000) return whole ? `${Math.floor(v / 1000)} s` : `${(Math.max(v, 50) / 1000).toFixed(1)} s`;
  const s = Math.floor(v / 1000);
  if (s < 3600) return `${Math.floor(s / 60)} min ${s % 60} s`;
  return `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}

// a count in the browser's digits (`1,200`) and micro USD in dollars (`$0.0042`): the step list's words, also of the
// daemon's Metrics pane (ui/daemon/metrics.js)
export const fmtCount = (v) => Number(v).toLocaleString();
const n = fmtCount;
export const usd =(micro) => `$${(micro / 1e6).toFixed(micro > 0 && micro < 10000 ? 4 : 2)}`;
const STOP = {AwaitingPerson: ['Waiting', 'on a person'], AwaitingAuthority: ['Waiting', 'on an authority'], Stuck: ['Stuck', 'no step can be taken'], Cancelled: ['Cancelled', '']};
const OWN = {'runbook.cancel-requested': 'Cancel requested', 'runbook.start-failed': 'Start failed', 'runbook.run-orphaned': 'Run orphaned', 'runbook.answer-undeliverable': 'Answer not delivered',
  'runbook.runtime-failed': 'Runtime failed', 'runbook.step-failed': 'Step failed', 'runbook.waiting-for-route': 'Waiting for the route'};
// a kind the page does not know, in words: `stub.answered` → `Stub answered`, `SomethingNew` → `Something new`
const humane = (kind) => {
  const w = String(kind || 'event').replace(/^runbook\./, '').replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/[._-]+/g, ' ').trim().toLowerCase();
  return w ? w[0].toUpperCase() + w.slice(1) : 'Event';
};
// the short plain members of a record nobody named here: no digest, no id of a record, no nested value
const DIGEST = /^[0-9a-f]{32,}$|^[0-9a-f-]{36}$/i;
const plain = (r) => Object.entries(r).filter(([k, v]) => !['seq', 'prev', 'at', 'kind', 'run', 'run_id'].includes(k) && (typeof v === 'number' || (typeof v === 'string' && v.length <= 80 && !DIGEST.test(v))))
  .slice(0, 4).map(([k, v]) => `${k.replace(/_/g, ' ')} ${v}`).join(' · ');
const join = (...xs) => xs.filter((x) => x != null && x !== '' && x !== false).join(' · ');

// {name, detail, engine}: an event in words. `engine`: one of the engine's own lines around each step. `ends`: stepEnds of
// the run's events: in a run whose runtime names each step's end, the step-budget stop it names is the engine's own line
// and one it does not name is a wait or the end of the tree; without `ends`, or in a run with no such event, the stop
// is the step's end.
export function eventWords(e, ends = null) {
  const r = e?.record && typeof e.record === 'object' ? e.record : {};
  switch (e?.kind) {
    case 'runbook.submitted': return {name: 'Run submitted', detail: join(str(r.runbook_id), str(r.confinement) && `${r.confinement} confinement`, str(r.model_route) && `route ${r.model_route}`), engine: false};
    case 'runbook.pinned': return {name: 'Runbook pinned', detail: join(str(r.runbook), num(r.revision) != null && `revision ${r.revision}`), engine: false};
    case 'CaseOpened': return {name: 'Case opened', detail: str(r.protocol) ?? '', engine: false};
    case 'runbook.action-started': return {name: 'Step started', detail: join(str(r.action), str(r.role), num(r.try) != null && `try ${r.try}`), engine: false};
    case 'ActionFinished':
      if (num(r.exit) == null && str(r.refusal)) return {name: 'Step refused', detail: join(str(r.action), refusalCause(r.refusal)), engine: false};
      return {name: 'Step finished', detail: join(str(r.action), num(r.exit) != null && `exit ${r.exit}`), engine: false};
    case 'HandoffChecked': return {name: 'Check finished', detail: join(str(r.action), num(r.exit) != null && `exit ${r.exit}`, num(r.exit) != null && checkWords(r.exit)), engine: false};
    case 'Evaluated': return {name: 'Case evaluated', detail: num(r.case_revision) != null ? `case revision ${r.case_revision}` : '', engine: true};
    case 'Stopped': {
      if (STEP_END(r) && ends?.present) return ends.named.has(e) ? {name: 'Step budget spent', detail: 'the engine call of one step ended', engine: true} : {name: 'Engine stopped', detail: 'step budget spent, no step ended: a wait or the end of the tree', engine: false};
      if (STEP_END(r)) return {name: 'Step ended', detail: 'the run goes on', engine: true};
      if (r.stop === 'BudgetSpent') return {name: 'Budget spent', detail: str(r.budget) ?? 'not named', engine: false};
      const [name, detail] = STOP[r.stop] ?? [str(r.stop) ? humane(r.stop) : 'Stopped', ''];
      return {name, detail, engine: false};
    }
    case 'runbook.step-ended': return {name: 'Step ended', detail: 'the run goes on', engine: true};
    case 'runbook.waiting-on-key': return {name: 'Waiting for its key', detail: join(str(r.serial_key), str(r.behind) && `behind run ${r.behind.slice(-8)}`), engine: false};
    case 'Terminated': return {name: 'Case ended', detail: str(r.reason) ?? '', engine: false};
    case 'runbook.model-call': return {name: 'Model call', detail: join(str(r.step), num(r.input_tokens) != null && num(r.output_tokens) != null ? `${n(r.input_tokens)} in, ${n(r.output_tokens)} out` : 'tokens not known', r.cost_known === false ? 'cost not known' : num(r.micro_usd) != null && usd(r.micro_usd)), engine: false};
    default:
      if (OWN[e?.kind]) return {name: OWN[e.kind], detail: str(r.reason) ?? str(r.failure) ?? join(str(r.route), str(r.phase)), engine: false};
      return {name: humane(e?.kind), detail: plain(r), engine: false};
  }
}

// What the run does now, in words: {head, since} (since: the start of the step in flight, for a duration that ticks).
const STATE_SAY = {Submitted: 'Submitted: not started yet', WaitingOnDecision: 'Waiting on a decision: nothing runs until it is answered', WaitingForRoute: 'Waiting for the model route', Cancelled: 'Cancelled'};
// A step that is over in words: `done`, `ended with exit 3`, the cause of a refusal (`ended by a signal`), or what a
// check's exit does (`checked: the writer goes on`).
const overWords = (s) => (s.state === 'exit' ? `ended with exit ${s.exit}` : s.state === 'ended' ? s.cause ?? 'ended' : s.role === 'check' && s.exit != null ? `checked: ${checkWords(s.exit)}` : 'done');
export function runSayParts(run, m, now, {clock = (t) => clockTime(t, now)} = {}) {
  const state = run?.state;
  const done = m.steps.filter((s) => s.state !== 'running');
  const last = done[done.length - 1];
  const lastWords = last ? `, the last ${last.action} ${overWords(last)}` : '';
  // the steps that are over: done (an exit of their own) apart from ended (a refusal and no exit)
  const cut = done.filter((s) => s.state === 'ended').length;
  const own = done.length - cut;
  const tally = (steps) => [own > 0 && `${own}${steps ? ` ${own === 1 ? 'step' : 'steps'}` : ''} done`, cut > 0 && `${cut} ended`].filter(Boolean).join(', ');
  if (state === 'Running' || (state === 'Submitted' && m.steps.length)) {
    const s = m.inFlight;
    if (s) return {head: `Running step ${s.n}: ${join(s.action, s.role, s.try != null && `try ${s.try}`).replaceAll(' · ', ', ')}${s.at != null ? `, since ${clock(s.at)}` : ''}`, since: s.at, running: true};
    if (done.length) return {head: `Between steps: ${tally(false)}${lastWords}`, since: null};
    return {head: 'Running: no step started yet', since: null};
  }
  if (state === 'Ended') {
    const why = run.outcome ?? m.ended?.reason ?? 'no outcome named';
    const fail = str(m.failed?.reason);
    return {head: `Ended: ${why}${fail ? ` (${fail})` : ''}`, since: null};
  }
  if (state === 'FailedToStart') return {head: `Failed to start: ${run.failure ?? 'no reason named'}`, since: null};
  if (state === 'WaitingOnKey') return {head: `Waiting for its serial key${str(run.serial_key) ? ` ${run.serial_key}` : ''}: nothing of it starts before the runs ahead of it end`, since: null};
  if (state === 'WaitingForRoute') return {head: `Waiting for the model route${run.route_phase ? ` (${run.route_phase})` : ''}`, since: null};
  return {head: `${STATE_SAY[state] ?? state ?? 'State not read'}${done.length ? `: ${tally(true)}${lastWords}` : ''}`, since: null};
}
export function runSay(run, m, now, opts) {
  const p = runSayParts(run, m, now, opts);
  return p.since != null ? `${p.head}, for ${fmtTook(now - p.since, true)}` : p.head;
}

const LIVE_STATES = ['Submitted', 'WaitingOnKey', 'Running', 'WaitingOnDecision', 'WaitingForRoute'];
const stateCls = (run, m) => (run?.state === 'Running' && m.inFlight ? 'running' : run?.state === 'Ended' ? 'ended' : ['Cancelled', 'FailedToStart'].includes(run?.state) ? 'bad' : ['WaitingOnDecision', 'WaitingOnKey'].includes(run?.state) ? 'waiting' : 'between');

// The tokens and the cost of calls in words: a count no call reported is "not known", never 0, and a sum that leaves
// calls out says so.
const tokenWords = (x) => (x.tokensKnown !== false ? `${n(x.tokens)} tokens` : x.tokens ? `${n(x.tokens)} tokens and calls with no token count` : 'tokens not known');
const costWords = (x) => (x.costKnown !== false ? usd(x.micro) : x.micro ? `${usd(x.micro)} and calls of unknown cost` : 'cost not known');

// The counts the run's events hold (and the view's row, withViewCalls).
function counts(m) {
  let done = 0, exits = 0, cut = 0;
  for (const s of m.steps) { if (s.state === 'done' || s.state === 'exit') done++; if (s.state === 'exit') exits++; if (s.state === 'ended') cut++; }
  return [`${done} ${done === 1 ? 'step' : 'steps'} done`, exits && `${exits} ended with an exit other than 0`, cut && `${cut} ended with no exit`,
    `${m.calls} model ${m.calls === 1 ? 'call' : 'calls'}`, m.calls && tokenWords(m), m.calls && costWords(m)].filter(Boolean);
}

// The head of the centre: what runs now (#dnow) and the counts.
export function renderStepsHead(run, m, now) {
  const p = runSayParts(run, m, now);
  const took = p.since != null ? `, for <span data-took-since="${p.since}">${esc(fmtTook(now - p.since, true))}</span>` : '';
  return `<p id="dnow" class="ds-now ds-${stateCls(run, m)}" data-key="dnow"><i aria-hidden="true"></i><span>${esc(p.head)}${took}</span></p>`
    + `<p class="ds-counts" data-key="dcounts">${counts(m).map((c) => `<span>${esc(c)}</span>`).join('')}</p>`;
}

// The steps in order, one row each: the latest STEP_ROWS of them and the step in flight when it started before those,
// with a line that says how many earlier steps are left out (the counts above are of every step). `caseName(id)`: the
// name of a case of the tree (a column only when steps name more than one case). A step still open in a run that is
// over is not running: it is "not finished".
export const STEP_ROWS = 200;
export function renderStepsTable(run, m, now, caseName = () => null) {
  if (!m.steps.length) return `<p class="ds-empty muted" data-key="dempty">${run && LIVE_STATES.includes(run.state) ? 'No step has started yet. The steps show here as the run takes them.' : 'This run took no step.'}</p>`;
  const live = run && LIVE_STATES.includes(run.state);
  // a run of more than one case names each step's case (— where its events name none: a step with no program)
  const named = new Set(m.cases ?? []);
  for (const s of m.steps) if (s.case) named.add(s.case);
  const many = named.size > 1;
  const latest = m.steps.slice(-STEP_ROWS);
  const shown = m.inFlight && m.inFlight.n <= m.steps.length - latest.length ? [m.inFlight, ...latest] : latest;
  const left = m.steps.length - shown.length;
  const note = left > 0 ? `<p class="ds-left muted" data-key="dleft">${fmtCount(left)} earlier ${left === 1 ? 'step' : 'steps'} left out: the list shows the latest ${STEP_ROWS}${shown.length > latest.length ? ' and the step in flight' : ''}.</p>` : '';
  const rows = shown.map((s) => {
    const state = s.state === 'running' && !live ? 'unfinished' : s.state;
    const word = state === 'unfinished' ? 'not finished' : state === 'running' ? 'running' : state === 'exit' ? `exit ${s.exit}` : overWords(s);
    const took = state === 'running' && s.at != null ? `<span data-took-since="${s.at}">${esc(fmtTook(now - s.at, true))}</span>` : s.at != null && s.end != null ? esc(fmtTook(s.end - s.at)) : '—';
    const calls = s.calls ? ` <span class="ds-calls" title="${esc([tokenWords(s), (s.micro || s.costKnown === false) && costWords(s)].filter(Boolean).join(', '))}">${s.calls} model ${s.calls === 1 ? 'call' : 'calls'}</span>` : '';
    return `<tr data-key="st:${s.n}" data-state="${state}"><td class="ds-n">${s.n}</td><td class="ds-act-c"><span class="ds-act">${esc(s.action)}</span>${calls}</td>`
      + (many ? `<td class="ds-case">${esc(caseName(s.case) ?? s.case ?? '—')}</td>` : '')
      + `<td class="ds-role">${esc(s.role ?? '—')}</td><td class="ds-st">${esc(word)}</td>`
      + `<td class="ds-at"${s.at != null ? ` title="${esc(fullTime(s.at))}"` : ''}>${s.at != null ? esc(clockTime(s.at, now)) : '—'}</td><td class="ds-took">${took}</td><td class="ds-try">${s.try ?? '—'}</td></tr>`;
  }).join('');
  return `${note}<table class="ds-table" data-key="dtable"><caption class="sr">Steps of the run, in the order they started</caption><thead><tr><th scope="col" class="ds-n">#</th><th scope="col">Step</th>${many ? '<th scope="col">Case</th>' : ''}<th scope="col">Role</th><th scope="col">State</th><th scope="col">Started</th><th scope="col">Took</th><th scope="col">Try</th></tr></thead><tbody>${rows}</tbody></table>`;
}

// The phone's sticky bar: the run's state in a word and the step in flight (or how the run stands).
const STATE_WORD = {Submitted: 'Submitted', Running: 'Running', WaitingOnDecision: 'Waiting on a decision', WaitingForRoute: 'Waiting for the route', WaitingOnKey: 'Waiting for its key', Ended: 'Ended', Cancelled: 'Cancelled', FailedToStart: 'Failed to start'};
export const runStateWord = (run) => STATE_WORD[run?.state] ?? run?.state ?? '';
export function renderStepsBar(run, m, now) {
  const s = m.inFlight && run?.state === 'Running' ? m.inFlight : null;
  const step = s ? `${esc(s.action)}${s.at != null ? ` · <span data-took-since="${s.at}">${esc(fmtTook(now - s.at, true))}</span>` : ''}` : esc(runSayParts(run, m, now).head.replace(/^[^:]*: ?/, '') || runStateWord(run));
  const cls = {running: 'm-running', ended: 'm-closed', bad: 'm-stuck', waiting: 'm-waiting', between: 'm-between'}[stateCls(run, m)];
  return `<span class="nb-state ${cls}" data-key="nbs"><i></i>${esc(runStateWord(run))}</span><span class="nb-step" data-key="nbp">${step}</span>`;
}

// One event of a run as a row: its name and what it says in words, its place in the run with the kind the runtime
// wrote, and its time on the page's clock (the date and time in full on hover).
export function eventRow(e, now = Date.now(), cls = 'fd-row', ends = null) {
  const w = eventWords(e, ends);
  const position = Number(e.position), sequence = Number(e.sequence), at = ms(e.at);
  return `<div class="${cls}${w.engine ? ' fd-engine' : ''}" data-key="ev:${position}" data-run="${esc(e.run_id)}" data-sequence="${sequence}">`
    + `<div class="fd-1"><span class="fd-s">${esc(w.name)}</span></div>`
    + (w.detail ? `<div class="fd-m">${esc(w.detail)}</div>` : '')
    + `<div class="fd-2"><span class="fd-c">event ${sequence} · ${esc(e.kind)}</span><span class="fd-t" title="${esc(fullTime(at))}">${esc(clockTime(at, now))}</span></div></div>`;
}

// The most event rows a list of a run's events draws (the feed, the Events tab): the latest ones, and a line saying how
// many earlier ones are left out. The counts in the head are of every event.
export const EVENT_ROWS = 200;
const leftOut = (n) => (n > 0 ? `<div class="fd-left muted" data-key="evleft">${n.toLocaleString()} earlier ${n === 1 ? 'event' : 'events'} left out: the list shows the latest ${EVENT_ROWS}.</div>` : '');

// The run's own events as the feed, newest first, in words (`list`: the events in their order, `id`: the run). The
// engine's own lines around each step (an evaluation before it, the step-budget stop after it and, from a runtime that
// names a step's end, runbook.step-ended) are folded unless `all`; the head says how many and has the control that
// shows them.
export function eventsFeed(list, id, {all = false, now = Date.now()} = {}) {
  const ends = stepEnds(list);
  const engine = list.map((e) => eventWords(e, ends).engine);
  const folded = all ? 0 : engine.filter(Boolean).length;
  const count = list.length - folded;
  const rows = [];
  for (let i = list.length - 1; i >= 0 && rows.length < EVENT_ROWS; i--) if (all || !engine[i]) rows.push(eventRow(list[i], now, 'fd-row', ends));
  const fold = all ? (engine.some(Boolean) ? '<button type="button" class="fd-fold" data-dfold="0">Fold engine lines</button>' : '')
    : folded ? `<button type="button" class="fd-fold" data-dfold="1">Show all ${list.length}</button>` : '';
  return `<div class="fd-h" data-key="fdh"><span class="fd-title">Run events</span><span class="muted">${count}${folded ? ` of ${list.length}` : ''}, newest first${folded ? ` · ${folded} engine lines folded` : ''}</span>${fold}</div>`
    + `<div class="fd-list" data-key="fdl" tabindex="0" role="region" aria-label="Run events" data-run="${esc(id)}">${rows.join('')}${leftOut(count - rows.length)}</div>`;
}

// The Events tab: every event of the run, oldest first, the engine's lines too; the latest EVENT_ROWS of them drawn.
export function eventsPane(list, now = Date.now()) {
  const ends = stepEnds(list);
  const from = Math.max(0, list.length - EVENT_ROWS);
  const rows = list.slice(from).map((e) => eventRow(e, now, 'ev-row fd-row', ends)).join('');
  return `<div class="tabpane" data-key="pane:events"><div class="sect">Run events <span class="muted">${list.length}, oldest first</span></div><div class="ev-list">${leftOut(from)}${rows}</div></div>`;
}

