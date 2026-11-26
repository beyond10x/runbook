// The steps of a daemon's run, read from the run's events (the stream the page already reads; docs/daemon-api.md, "The
// step in flight" and "Event streams"), and the words for each event. Story run-page-watches-a-live-run.
//   runbook.action-started  a bound program starts: case, action, role, try. A step begins.
//   ActionFinished          Loom's line when an action ends: action, exit. The last open step of that action ends; an
//                           action that ran no program (open, person) is a step with no start.
//   runbook.model-call      a model call of a step: step, input_tokens, output_tokens, micro_usd, cost_known.
//   Evaluated, Stopped      the engine's own lines around each step: the runtime runs each step as one engine call
//                           that stops on its step budget, so every step is followed by `Stopped` with
//                           `BudgetSpent`/`MaxSteps`. That line ends one step, never the run: it closes the last step
//                           started when that one still runs (a step with no program has no `ActionFinished`). Any
//                           other stop is said for what it is (a wait is a wait).
//   runbook.step-ended      the runtime's own line after such a stop: case, seq. The same end, said twice changes nothing.
//   Terminated              the case ended, with its reason.
// Nothing here reads a digest or an id of a record: a person reads names, numbers and times.
import {esc} from './lenses.js';
import {clockTime, fullTime} from './clock.js';

const ms = (t) => { const v = Date.parse(t); return Number.isNaN(v) ? null : v; };
const num = (v) => (typeof v === 'number' && Number.isFinite(v) ? v : null);
const str = (v) => (typeof v === 'string' && v ? v : null);
const STEP_END = (r) => r?.stop === 'BudgetSpent' && r?.budget === 'MaxSteps';

// {steps, cases, inFlight, calls, tokens, micro, costKnown, ended, stop, failed} of one run's events, in their order.
// cases: the ids of the cases the run opened (`CaseOpened`).
export function runSteps(events) {
  const steps = [];
  const m = {steps, cases: new Set(), inFlight: null, calls: 0, tokens: 0, micro: 0, costKnown: true, ended: null, stop: null, failed: null};
  const open = (action) => steps.filter((s) => s.state === 'running' && s.action === action);
  // the end of a step: the last step started (of the case named, when one is) ends here if it still runs
  const stepEnd = (kase, at) => {
    const s = [...steps].reverse().find((x) => kase == null || x.case == null || x.case === kase);
    if (s?.state === 'running') Object.assign(s, {end: at, state: 'done'});
  };
  for (const e of events || []) {
    const r = e?.record && typeof e.record === 'object' ? e.record : {};
    const at = ms(e?.at);
    if (e?.kind === 'runbook.action-started') {
      steps.push({n: steps.length + 1, action: str(r.action) ?? 'a step', case: str(r.case), role: str(r.role), try: num(r.try), at, end: null, exit: null, state: 'running', calls: 0, tokens: 0, micro: 0});
    } else if (e?.kind === 'ActionFinished') {
      const action = str(r.action) ?? 'a step';
      const exit = num(r.exit);
      const state = exit == null || exit === 0 ? 'done' : 'exit';
      const ends = open(action);
      if (!ends.length) steps.push({n: steps.length + 1, action, case: null, role: null, try: null, at: null, end: at, exit, state, calls: 0, tokens: 0, micro: 0});
      for (const s of ends) Object.assign(s, {end: at, exit, state});
    } else if (e?.kind === 'runbook.model-call') {
      const tokens = (num(r.input_tokens) ?? 0) + (num(r.output_tokens) ?? 0);
      const micro = r.cost_known === false ? 0 : num(r.micro_usd) ?? 0;
      m.calls++; m.tokens += tokens; m.micro += micro;
      if (r.cost_known === false) m.costKnown = false;
      const s = [...steps].reverse().find((x) => x.action === r.step);
      if (s) { s.calls++; s.tokens += tokens; s.micro += micro; }
    } else if (e?.kind === 'CaseOpened') { if (str(r.case)) m.cases.add(r.case); }
    else if (e?.kind === 'Terminated') m.ended = {reason: str(r.reason)};
    else if (e?.kind === 'Stopped') { if (STEP_END(r)) stepEnd(null, at); else m.stop = r; }
    else if (e?.kind === 'runbook.step-ended') stepEnd(str(r.case), at);
    else if (e?.kind === 'runbook.step-failed') m.failed = r;
  }
  m.inFlight = [...steps].reverse().find((s) => s.state === 'running') ?? null;
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

const n = (v) => Number(v).toLocaleString();
export const usd = (micro) => `$${(micro / 1e6).toFixed(micro > 0 && micro < 10000 ? 4 : 2)}`;
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

// {name, detail, engine}: an event in words. `engine`: one of the engine's own lines around each step.
export function eventWords(e) {
  const r = e?.record && typeof e.record === 'object' ? e.record : {};
  switch (e?.kind) {
    case 'runbook.submitted': return {name: 'Run submitted', detail: join(str(r.runbook_id), str(r.confinement) && `${r.confinement} confinement`, str(r.model_route) && `route ${r.model_route}`), engine: false};
    case 'runbook.pinned': return {name: 'Runbook pinned', detail: join(str(r.runbook), num(r.revision) != null && `revision ${r.revision}`), engine: false};
    case 'CaseOpened': return {name: 'Case opened', detail: str(r.protocol) ?? '', engine: false};
    case 'runbook.action-started': return {name: 'Step started', detail: join(str(r.action), str(r.role), num(r.try) != null && `try ${r.try}`), engine: false};
    case 'ActionFinished': return {name: 'Step finished', detail: join(str(r.action), num(r.exit) != null && `exit ${r.exit}`), engine: false};
    case 'Evaluated': return {name: 'Case evaluated', detail: num(r.case_revision) != null ? `case revision ${r.case_revision}` : '', engine: true};
    case 'Stopped': {
      if (STEP_END(r)) return {name: 'Step ended', detail: 'the run goes on', engine: true};
      if (r.stop === 'BudgetSpent') return {name: 'Budget spent', detail: str(r.budget) ?? 'not named', engine: false};
      const [name, detail] = STOP[r.stop] ?? [str(r.stop) ? humane(r.stop) : 'Stopped', ''];
      return {name, detail, engine: false};
    }
    case 'runbook.step-ended': return {name: 'Step ended', detail: 'the run goes on', engine: true};
    case 'runbook.waiting-on-key': return {name: 'Waiting for its key', detail: join(str(r.serial_key), str(r.behind) && `behind run ${r.behind.slice(-8)}`), engine: false};
    case 'Terminated': return {name: 'Case ended', detail: str(r.reason) ?? '', engine: false};
    case 'runbook.model-call': return {name: 'Model call', detail: join(str(r.step), `${n(num(r.input_tokens) ?? 0)} in, ${n(num(r.output_tokens) ?? 0)} out`, r.cost_known === false ? 'cost not known' : num(r.micro_usd) != null && usd(r.micro_usd)), engine: false};
    default:
      if (OWN[e?.kind]) return {name: OWN[e.kind], detail: str(r.reason) ?? str(r.failure) ?? join(str(r.route), str(r.phase)), engine: false};
      return {name: humane(e?.kind), detail: plain(r), engine: false};
  }
}

// What the run does now, in words: {head, since} (since: the start of the step in flight, for a duration that ticks).
const STATE_SAY = {Submitted: 'Submitted: not started yet', WaitingOnDecision: 'Waiting on a decision: nothing runs until it is answered', WaitingForRoute: 'Waiting for the model route', Cancelled: 'Cancelled'};
export function runSayParts(run, m, now, {clock = (t) => clockTime(t, now)} = {}) {
  const state = run?.state;
  const done = m.steps.filter((s) => s.state !== 'running');
  const last = done[done.length - 1];
  const lastWords = last ? `, the last ${last.action} ${last.state === 'exit' ? `ended with exit ${last.exit}` : 'done'}` : '';
  if (state === 'Running' || (state === 'Submitted' && m.steps.length)) {
    const s = m.inFlight;
    if (s) return {head: `Running step ${s.n}: ${join(s.action, s.role, s.try != null && `try ${s.try}`).replaceAll(' · ', ', ')}${s.at != null ? `, since ${clock(s.at)}` : ''}`, since: s.at, running: true};
    if (done.length) return {head: `Between steps: ${done.length} done${lastWords}`, since: null};
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
  return {head: `${STATE_SAY[state] ?? state ?? 'State not read'}${done.length ? `: ${done.length} ${done.length === 1 ? 'step' : 'steps'} done${lastWords}` : ''}`, since: null};
}
export function runSay(run, m, now, opts) {
  const p = runSayParts(run, m, now, opts);
  return p.since != null ? `${p.head}, for ${fmtTook(now - p.since, true)}` : p.head;
}

const LIVE_STATES = ['Submitted', 'WaitingOnKey', 'Running', 'WaitingOnDecision', 'WaitingForRoute'];
const stateCls = (run, m) => (run?.state === 'Running' && m.inFlight ? 'running' : run?.state === 'Ended' ? 'ended' : ['Cancelled', 'FailedToStart'].includes(run?.state) ? 'bad' : ['WaitingOnDecision', 'WaitingOnKey'].includes(run?.state) ? 'waiting' : 'between');

// The counts the run's events hold.
function counts(m) {
  const done = m.steps.filter((s) => s.state === 'done' || s.state === 'exit').length;
  const exits = m.steps.filter((s) => s.state === 'exit').length;
  return [`${done} ${done === 1 ? 'step' : 'steps'} done`, exits && `${exits} ended with an exit other than 0`,
    `${m.calls} model ${m.calls === 1 ? 'call' : 'calls'}`, m.calls && `${n(m.tokens)} tokens`, m.calls && (m.costKnown ? usd(m.micro) : `${usd(m.micro)} and calls of unknown cost`)].filter(Boolean);
}

// The head of the centre: what runs now (#dnow) and the counts.
export function renderStepsHead(run, m, now) {
  const p = runSayParts(run, m, now);
  const took = p.since != null ? `, for <span data-took-since="${p.since}">${esc(fmtTook(now - p.since, true))}</span>` : '';
  return `<p id="dnow" class="ds-now ds-${stateCls(run, m)}" data-key="dnow"><i aria-hidden="true"></i><span>${esc(p.head)}${took}</span></p>`
    + `<p class="ds-counts" data-key="dcounts">${counts(m).map((c) => `<span>${esc(c)}</span>`).join('')}</p>`;
}

// The steps in order, one row each. `caseName(id)`: the name of a case of the tree (a column only when steps name more
// than one case). A step still open in a run that is over is not running: it is "not finished".
export function renderStepsTable(run, m, now, caseName = () => null) {
  if (!m.steps.length) return `<p class="ds-empty muted" data-key="dempty">${run && LIVE_STATES.includes(run.state) ? 'No step has started yet. The steps show here as the run takes them.' : 'This run took no step.'}</p>`;
  const live = run && LIVE_STATES.includes(run.state);
  // a run of more than one case names each step's case (— where its events name none: a step with no program)
  const many = new Set([...(m.cases ?? []), ...m.steps.map((s) => s.case).filter(Boolean)]).size > 1;
  const rows = m.steps.map((s) => {
    const state = s.state === 'running' && !live ? 'unfinished' : s.state;
    const word = {running: 'running', done: 'done', exit: `exit ${s.exit}`, unfinished: 'not finished'}[state];
    const took = state === 'running' && s.at != null ? `<span data-took-since="${s.at}">${esc(fmtTook(now - s.at, true))}</span>` : s.at != null && s.end != null ? esc(fmtTook(s.end - s.at)) : '—';
    const calls = s.calls ? ` <span class="ds-calls" title="${esc(`${n(s.tokens)} tokens${s.micro ? `, ${usd(s.micro)}` : ''}`)}">${s.calls} model ${s.calls === 1 ? 'call' : 'calls'}</span>` : '';
    return `<tr data-key="st:${s.n}" data-state="${state}"><td class="ds-n">${s.n}</td><td class="ds-act-c"><span class="ds-act">${esc(s.action)}</span>${calls}</td>`
      + (many ? `<td class="ds-case">${esc(caseName(s.case) ?? s.case ?? '—')}</td>` : '')
      + `<td class="ds-role">${esc(s.role ?? '—')}</td><td class="ds-st">${esc(word)}</td>`
      + `<td class="ds-at"${s.at != null ? ` title="${esc(fullTime(s.at))}"` : ''}>${s.at != null ? esc(clockTime(s.at, now)) : '—'}</td><td class="ds-took">${took}</td><td class="ds-try">${s.try ?? '—'}</td></tr>`;
  }).join('');
  return `<table class="ds-table" data-key="dtable"><caption class="sr">Steps of the run, in the order they started</caption><thead><tr><th scope="col" class="ds-n">#</th><th scope="col">Step</th>${many ? '<th scope="col">Case</th>' : ''}<th scope="col">Role</th><th scope="col">State</th><th scope="col">Started</th><th scope="col">Took</th><th scope="col">Try</th></tr></thead><tbody>${rows}</tbody></table>`;
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
