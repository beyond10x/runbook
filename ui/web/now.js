// What is happening right now: every action in flight in the case tree, who or what executes it and
// since when, the live progress of an agent or decider call, what waits on a person, the latest
// finished steps, and the file changes each one made. Derivation first (pure, tested in
// test/now.spec.mjs), rendering after.
//
// Signals: a case's events.jsonl (`action_started` without a later `action_finished` is in flight;
// the latest verdict event says what it waits on), `times` (the server's wall clock of each event:
// the events' own `at` is the instant passed to the whole `sdc-drive run`, not the step's), the
// server's /api/live (llm/ and decider/ record directories whose response.json is still empty, with
// the progress read from their metaharness event stream; fs/changes.jsonl), llm-calls.jsonl.
import {actorOf, predicateRefs, VERDICTS, evalAt} from './model.js';
import {PHASES, esc} from './lenses.js';
import {highlightLine, langOf, LANG_LABEL} from './highlight.js';
import {classify, CLASS_LABEL} from './meaning.js';
import {holderOf, takenText, fieldsOf, fieldCounts} from './holder.js';
import {autopilot, principalOf} from './principal.js';

export const KIND_LABEL = {llm: 'LLM agent', program: 'Program', review: 'Review program', copy: 'Copy', driver: 'Driver', person: 'Person', decider: 'Decider'};
const at = (e) => { const t = Date.parse(e?.at); return isNaN(t) ? null : t; };
export const timeOf = (c, e) => (e ? c.times?.[e.seq] ?? null : null);

// ---- derivation ---------------------------------------------------------------------------------
export function inFlight(cases) {
  const out = [];
  for (const c of cases) {
    let open = null;
    for (const e of c.events) {
      if (e.event === 'action_started') open = e;
      else if (e.event === 'action_finished' || e.event === 'terminated' || VERDICTS.includes(e.event)) open = null;
    }
    if (open) out.push({c, start: open});
  }
  return out;
}

const shortArg = (a) => String(a).replace(/^.*\/(tools|scripts|eval)\//, '').replace(/^\/.*\//, '');
export function executorOf(c, e, record) {
  const b = c.bindings?.[e.action] || {};
  const cp = e.copy || b.copy;
  if (cp) return {kind: 'copy', detail: `${cp.kind || 'record'} from ${cp.from || '?'}`};
  const argv = e.argv || b.argv || [];
  if (!argv.length) return {kind: 'driver', detail: c.ir?.actions?.[e.action]?.case_open ? 'opens child cases' : 'the driver’s own step'};
  const cmd = [argv[0].split('/').pop(), ...argv.slice(1).map(shortArg)].join(' ');
  if (e.llm || b.llm) return {kind: 'llm', model: record?.model || (typeof b.llm === 'object' ? b.llm.model : null) || null, detail: cmd, maxTurns: record?.maxTurns ?? b.llm?.max_turns ?? null};
  const k = actorOf(c, e.action).kind;
  return {kind: k === 'review' ? 'review' : k === 'llm' ? 'llm' : 'program', detail: cmd};
}

const flag = (cmd, name) => (new RegExp(`--${name} (\\S+)`).exec(cmd || '') || [])[1] || null;
export function waitOf(c) {
  let v = null;
  for (let i = c.events.length - 1; i >= 0; i--) {
    const e = c.events[i];
    if (e.event === 'evaluated' || e.event === 'revised') continue;
    if (e.event === 'awaiting_person' || e.event === 'awaiting_authority') v = e;
    break;
  }
  if (!v) return null;
  const asks = (v.next || []).filter((n) => n && n.action).map((n) => {
    const subject = c.bindings?.[n.action]?.subject || null;
    // scope.answer belongs to the interview's scope-changing questions (c.scope, from the server):
    // the ones nobody named has answered, else the ones answered and waiting for the approval
    const sq = n.action === 'scope.answer' && c.scope?.questions?.length ? c.scope.questions : null;
    const questions = sq ? (sq.some((q) => q.awaiting) ? sq.filter((q) => q.awaiting) : sq) : [];
    return {action: n.action, capability: flag(n.command, 'capability'), principal: flag(n.command, 'principal'), handoff: / run --at /.test(n.command || ''),
      subject, revision: subject ? c.case?.artifacts?.[subject]?.revision || null : null, decides: n.decides || '', questions};
  });
  return {c, event: v, verdict: v.event, asks};
}

// What the driver is doing to a case tree, from /api/live `drivers` (driver.json, sdc.driver/1, present
// while a driver holds the tree; the pid in the lock file; whether any case has an observed.jsonl):
// attached (its pid runs), gone (driver.json names a pid that is not running, or no driver.json and
// no live lock while an observed.jsonl exists), unknown (an old run: no signal, nothing to say).
export function driverState(c, live) {
  const d = live?.drivers?.[c.tree];
  if (!d) return {state: 'unknown'};
  if (d.file) {
    return d.alive ? {state: 'attached', pid: d.pid, started: Date.parse(d.started_at) || null, argv: d.argv, via: 'driver.json'}
      : {state: 'gone', pid: d.pid, why: `driver.json names pid ${d.pid}, which is not running`};
  }
  if (d.lock_alive) return {state: 'attached', pid: d.lock_pid, started: null, argv: null, via: 'lock'};
  if (d.observed) return {state: 'gone', pid: null, why: 'no driver.json while observed.jsonl exists'};
  return {state: 'unknown'};
}

// Pair calls in progress with what they serve: an agent call with the in-flight action of the same
// name and, when its request.json names the case, that case (before that, two cases running the
// same action pair by start order), a decider call with the waiting case its request.json or
// decide-request.json names.
function claim(pool, pred, since) {
  const cands = pool.filter(pred);
  if (!cands.length) return null;
  const own = cands.filter((r) => r.caseFrom === 'request.json');
  const after = !own.length && since != null ? cands.filter((r) => r.started == null || r.started >= since - 3000) : [];
  const pick = (own.length ? own : after.length ? after : cands).sort((a, b) => (a.n ?? 0) - (b.n ?? 0))[0];
  pool.splice(pool.indexOf(pick), 1);
  return pick;
}

export function changesFor(live, caseId, action, record, since) {
  return (live?.fs?.changes || []).filter((x) => x.attribution?.case === caseId && x.attribution?.action === action
    && (record && x.attribution.record ? x.attribution.record === record : since == null || Date.parse(x.at) >= since - 2000));
}

export function nowState(cases, live, run, nowMs = Date.now()) {
  const pool = (live?.records || []).slice();
  const drivers = new Map();
  for (const c of cases) if (!drivers.has(c.tree)) drivers.set(c.tree, driverState(c, live));
  const driver = {attached: [...drivers.values()].filter((d) => d.state === 'attached'), gone: [...drivers.values()].filter((d) => d.state === 'gone')};
  const all = inFlight(cases).map(({c, start}) => {
    const since = timeOf(c, start);
    const record = start.llm || c.bindings?.[start.action]?.llm ? claim(pool, (r) => r.kind === 'llm' && r.action === start.action && (!r.case || r.case === c.id), since) : null;
    const exec = executorOf(c, start, record);
    const t0 = since ?? record?.started ?? null;
    return {c, start, action: start.action, exec, kind: exec.kind, since: t0, sinceSrc: since != null ? c.timeSource?.[start.seq] || 'server' : record?.started != null ? 'call' : null, approx: t0 == null ? at(start) : null, record,
      driver: drivers.get(c.tree) || {state: 'unknown'},
      timeout: start.timeout_s ? start.timeout_s * 1000 : null, files: changesFor(live, c.id, start.action, record?.rel, t0)};
  });
  const byStart = (a, b) => (a.since ?? a.approx ?? 0) - (b.since ?? b.approx ?? 0) || a.c.depth - b.c.depth;
  // a step started and never finished, with no driver alive to finish it, is not running
  const running = all.filter((r) => r.driver.state !== 'gone').sort(byStart);
  const gone = all.filter((r) => r.driver.state === 'gone').sort(byStart);
  const waiting = [], stalled = [];
  for (const c of cases) {
    const w = waitOf(c);
    if (!w) continue;
    const since = timeOf(c, w.event);
    // amend.upstream with no finding for a person is not a decision: nothing can run (decisions.mjs amendStall)
    if (c.stall && c.stall.needsPerson === 0 && w.asks.length && w.asks.every((a) => a.action === 'amend.upstream')) { stalled.push({c, stall: c.stall, event: w.event, since}); continue; }
    const asks = w.asks.length ? w.asks : [{action: null, capability: null, principal: null, decides: w.verdict.replace('_', ' ')}];
    for (const a of asks) {
      const record = claim(pool, (r) => r.kind === 'decider' && r.case === c.id && (!a.action || r.action === a.action), since);
      const entry = {c, ...a, verdict: w.verdict, event: w.event, since: since ?? record?.started ?? null, approx: since == null ? at(w.event) : null,
        kind: record ? 'decider' : 'person', record};
      entry.holder = holderOf(entry, run?.decisions, run?.settings);
      waiting.push(entry);
    }
  }
  const root = cases.find((c) => !c.parent) || cases[0];
  const term = root?.events.find((e) => e.event === 'terminated');
  const stuck = cases.filter((c) => { const l = lastVerdictish(c); return l && ['stuck', 'step_budget_spent', 'llm_budget_spent', 'wall_time_spent'].includes(l.event); });
  const terminable = cases.filter((c) => lastVerdictish(c)?.event === 'terminable' && !c.events.some((e) => e.event === 'terminated'));
  const between = cases.filter((c) => {
    const l = c.events[c.events.length - 1];
    return l && !c.events.some((e) => e.event === 'terminated') && !lastVerdictish(c) && !running.some((r) => r.c === c) && !gone.some((r) => r.c === c);
  });
  // a run stopped on purpose (run-stopped.json): said as such, whatever it was doing at the stop
  const stopped = run?.stopped && !isNaN(Date.parse(run.stopped.at)) ? {...run.stopped, t: Date.parse(run.stopped.at)} : null;
  let mode = stopped ? 'stopped' : running.length ? 'running' : gone.length ? 'gone' : waiting.length ? 'waiting' : stalled.length ? 'stalled' : term ? 'closed' : stuck.length ? 'stuck' : terminable.length ? 'terminable' : between.length ? 'between' : 'idle';
  const lastT = Math.max(0, ...cases.map((c) => Math.max(0, ...Object.values(c.times || {}))));
  return {mode, stopped, autopilot: autopilot(run?.settings), running, gone, driver, waiting, stalled, root, term, stuck, terminable, between, recent: recentSteps(cases, run, 4), lastT: lastT || null, now: nowMs,
    steps: cases.reduce((n, c) => n + c.events.filter((e) => e.event === 'action_finished').length, 0)};
}
function lastVerdictish(c) {
  for (let i = c.events.length - 1; i >= 0; i--) {
    const e = c.events[i];
    if (e.event === 'evaluated' || e.event === 'revised') continue;
    return VERDICTS.includes(e.event) || e.event === 'terminated' ? e : null;
  }
  return null;
}

// The latest finished steps of the tree, newest first, with a wall-clock duration where one exists:
// the server's times of `action_started` and `action_finished`, else the LLM call's duration_ms.
export function recentSteps(cases, run, n = 4) {
  const all = [];
  for (const c of cases) {
    let started = null;
    const nth = {};
    for (const e of c.events) {
      if (e.event === 'action_started') { started = e; nth[e.action] = (nth[e.action] || 0) + 1; }
      else if (e.event === 'action_finished') {
        const s = started?.action === e.action ? started : null;
        const t1 = timeOf(c, e), t0 = timeOf(c, s);
        let dur = t1 != null && t0 != null ? t1 - t0 : null;
        let durSrc = dur != null ? (c.timeSource?.[e.seq] === 'driver' && c.timeSource?.[s.seq] === 'driver' ? 'driver observed_at' : 'observed') : null;
        if (dur == null && (s?.llm || c.bindings?.[e.action]?.llm)) {
          const recs = (run?.llm || []).filter(({record}) => record.action === e.action && (record.case == null || record.case === c.id));
          const r = recs[(nth[e.action] || 1) - 1]?.record;
          if (r?.duration_ms != null) { dur = Number(r.duration_ms); durSrc = 'llm-calls.jsonl'; }
        }
        const discarded = c.events.some((x) => x.event === 'discarded' && x.action === e.action && x.seq > (s?.seq ?? -1) && x.seq < e.seq);
        const result = (e.evidence || []).map((id) => { const r = c.evidence?.[id]; return r ? `${r.kind}${r.result != null ? ' = ' + (typeof r.result === 'object' ? JSON.stringify(r.result) : r.result) : ''}` : id; }).join(', ');
        all.push({c, start: s, end: e, t: t1, tApprox: at(e), dur, durSrc, discarded, exit: String(e.exit), result: result || (Object.keys(e.revises || {}).length ? `revised ${Object.keys(e.revises).join(', ')}` : 'no evidence'),
          kind: e.principal ? ({person: 'person', decider: 'decider', rule: 'decider'}[principalOf(e.principal).kind] || 'driver') : executorOf(c, s || {action: e.action}).kind});
        started = null;
      }
    }
  }
  all.sort((a, b) => (b.t ?? b.tApprox ?? 0) - (a.t ?? a.tApprox ?? 0) || b.c.depth - a.c.depth || b.end.seq - a.end.seq);
  return all.slice(0, n);
}

// The SDC phase an action works on: the phase of its protocol whose claims its evidence kinds
// establish (directly, or through a derived claim), else the first open phase of that protocol.
export function phaseOf(c, action) {
  const ir = c.ir || {};
  const proto = c.case?.protocol || ir.protocol?.id;
  const mine = PHASES.filter((p) => p.p === proto);
  if (!mine.length) return null;
  const kinds = new Set((ir.actions?.[action]?.may_produce || []).map((m) => m.evidence));
  const direct = new Set(Object.entries(ir.claims || {}).filter(([, d]) => predicateRefs(d.true_when).evidence.some((x) => kinds.has(x.kind))).map(([id]) => id));
  const derived = new Set(Object.entries(ir.claims || {}).filter(([, d]) => predicateRefs(d.true_when).claims.some((x) => direct.has(x.id))).map(([id]) => id));
  const hit = mine.find((p) => p.claims.some((cl) => direct.has(cl))) || mine.find((p) => p.claims.some((cl) => derived.has(cl)));
  if (hit) return hit.id;
  const ev = evalAt(c, null);
  return (mine.find((p) => p.claims.some((cl) => ir.claims?.[cl] && ev?.claims?.[cl]?.value !== 'true')) || mine[0]).id;
}

// phase id -> what is active in it, for the phase strip
export function activePhases(st) {
  const m = new Map();
  if (st.stopped) return m;
  for (const r of st.running) { const p = phaseOf(r.c, r.action); if (p && !m.has(p)) m.set(p, {kind: r.kind, verb: 'running', action: r.action, case: r.c.id}); }
  for (const w of st.waiting) if (w.kind === 'decider') { const p = phaseOf(w.c, w.action); if (p && !m.has(p)) m.set(p, {kind: 'decider', verb: 'deciding', action: w.action, case: w.c.id}); }
  return m;
}

// action -> {kind, verb, since} for one case's graph
export function graphLive(st, c) {
  const m = new Map();
  // a stopped run: the node says so, its clock stands at the stop (the page's clock is the stop time)
  for (const r of st.running) if (r.c === c) m.set(r.action, {kind: r.kind, verb: st.stopped ? 'stopped' : 'running', since: r.since ?? r.approx});
  for (const w of st.waiting) if (w.c === c && w.action) m.set(w.action, {kind: w.kind, verb: st.stopped ? 'stopped' : w.kind === 'decider' ? 'deciding' : 'waiting', since: w.since ?? w.approx});
  return m;
}

// ---- formatting ---------------------------------------------------------------------------------
export function fmtDur(ms) {
  if (ms == null || isNaN(ms)) return '—';
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return h ? `${h}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}` : `${m}:${String(r).padStart(2, '0')}`;
}
export function fmtSpan(ms) {
  if (ms == null || isNaN(ms)) return '—';
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)} s`;
  return fmtDur(ms);
}
export function fmtAgo(ms) {
  if (ms == null || isNaN(ms)) return '';
  const s = Math.floor(ms / 1000);
  if (s < 5) return 'just now';
  if (s < 60) return `${s} s ago`;
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}
const clock = (t) => (t == null ? '' : new Date(t).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}));
const since = (t, now) => `<span class="tk" data-since="${t}">${fmtDur(now - t)}</span>`;
const ago = (t, now) => `<span data-ago="${t}">${fmtAgo(now - t)}</span>`;
const shortCase = (c) => (c.parent && c.id.startsWith(c.parent + '.') ? c.id.slice(c.parent.length + 1) : c.id);

// ---- the band --------------------------------------------------------------------------------------
const ICON = {
  llm: '<svg viewBox="0 0 16 16"><path d="M8 1.5l1.6 3.9 3.9 1.6-3.9 1.6L8 12.5 6.4 8.6 2.5 7l3.9-1.6z" fill="currentColor"/></svg>',
  decider: '<svg viewBox="0 0 16 16"><path d="M8 2v12M3 5h10M3 5l-2 5h4zM13 5l-2 5h4z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>',
  person: '<svg viewBox="0 0 16 16"><circle cx="8" cy="5.2" r="2.7" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M2.8 14c.6-3 2.7-4.4 5.2-4.4s4.6 1.4 5.2 4.4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  program: '<svg viewBox="0 0 16 16"><path d="M3 4.5l3.5 3.5L3 11.5M8.5 12h5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  review: '<svg viewBox="0 0 16 16"><path d="M2.5 8.5l3.5 3.5 7.5-8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  copy: '<svg viewBox="0 0 16 16"><rect x="5" y="5" width="8.5" height="8.5" rx="1.8" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M11 3.2V3a1.5 1.5 0 0 0-1.5-1.5H4A1.5 1.5 0 0 0 2.5 3v5.5A1.5 1.5 0 0 0 4 10h.2" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>',
  driver: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="2.2" fill="currentColor"/><circle cx="8" cy="8" r="5.6" fill="none" stroke="currentColor" stroke-width="1.4" stroke-dasharray="2.6 2.2"/></svg>',
  done: '<svg viewBox="0 0 16 16"><path d="M3 8.5l3.2 3.2L13 4.8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  stuck: '<svg viewBox="0 0 16 16"><path d="M8 4v5M8 11.6v.4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
  stopped: '<svg viewBox="0 0 16 16"><rect x="4.2" y="4.2" width="7.6" height="7.6" rx="1.4" fill="currentColor"/></svg>',
  idle: '<svg viewBox="0 0 16 16"><path d="M5.5 4v8M10.5 4v8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
};
const orb = (kind, spin) => `<span class="orb k-${kind}${spin ? ' spin' : ''}"><span class="orb-i">${ICON[kind] || ICON.program}</span></span>`;
const tag = (kind, text) => `<span class="ek k-${kind}">${esc(text || KIND_LABEL[kind] || kind)}</span>`;

function progress(rec, maxTurns) {
  if (!rec) return '';
  if (!rec.events) return `<span class="np-s">starting the session</span>`;
  const last = rec.last?.kind === 'tool' ? `<span class="np-last"><span class="np-tool">${esc(rec.last.name)}</span>${rec.last.arg ? ` <span class="np-arg">${esc(rec.last.arg)}</span>` : ''}</span>`
    : rec.last?.kind === 'text' ? `<span class="np-last"><span class="np-say">“${esc(rec.last.text.slice(0, 80))}”</span></span>` : rec.last?.kind === 'thinking' ? `<span class="np-last np-think">thinking…</span>` : '';
  const mt = maxTurns ?? rec.maxTurns;
  return `<span class="np-n"><b>${rec.turns}</b>${mt ? `<i>/${mt}</i>` : ''} turns</span><span class="np-n"><b>${rec.tools}</b> tool calls</span>${rec.denied ? `<span class="np-n np-deny"><b>${rec.denied}</b> denied</span>` : ''}${rec.ended ? '<span class="np-s">session ended · applying</span>' : ''}${last}`;
}
const turnBar = (rec, maxTurns) => { const mt = maxTurns ?? rec?.maxTurns; return rec && mt ? `<div class="nc-bar" title="${rec.turns} of at most ${mt} turns"><i style="width:${Math.min(100, (100 * rec.turns) / mt).toFixed(1)}%"></i></div>` : ''; };

const OP = {create: 'A', modify: 'M', delete: 'D', rename: 'R'};
const BASIS = {'action-window': ['exact', 'attributed exactly: the change fell inside this action’s window and nothing else ran'], 'tool-window': ['inferred', 'inferred from the agent’s event stream: the change fell inside this tool call’s window'], record: ['call', 'in an agent or decider call’s working directory while no action of its case was open and no tool call’s window held it: attributed to the call'], none: ['unattributed', 'nothing was in flight when this changed']};
// The tool call that made a change, as name and input; the input is left out when it only repeats
// the file's own path.
export function toolText(x) {
  const a = x.attribution || {};
  if (!a.tool) return {name: a.basis === 'action-window' ? (a.record ? 'the call' : 'the step') : a.basis === 'record' ? 'the call' : '—', input: ''};
  let input = String(a.tool.input ?? '').replace(/^.*\/(llm|decider)\/\d+-[^/]+\/cwd\//, '');
  if (input && (input === x.path || (x.path && input.endsWith('/' + x.path)) || (x.path && x.path.endsWith(input)))) input = '';
  return {name: a.tool.name, input, seq: a.tool.seq};
}
const toolHtml = (t) => `<span class="ft"><b>${esc(t.name)}</b>${t.input ? ` <span class="fti">${esc(t.input)}</span>` : ''}</span>`;
const lines = (x) => (x.binary ? '<span class="fbin">binary</span>' : x.too_large ? '<span class="fbin">over 1 MiB</span>' : x.lines_added == null && x.op === 'rename' ? '<span class="fbin">moved</span>' : `<span class="fa">+${x.lines_added ?? 0}</span><span class="fd">−${x.lines_removed ?? 0}</span>`);
// A planning, spec, direction or decision change shows what it means (web/meaning.js); code its path.
export const meaningFor = (x) => x.meaning || (classify(x.path).cls === 'code' ? null : {cls: classify(x.path).cls, items: [{text: `${x.op === 'create' ? 'created' : x.op === 'delete' ? 'deleted' : 'changed'} ${x.path}`}]});
export const classBadge = (cls) => `<span class="cb c-${cls}" title="${esc(CLASS_LABEL[cls] || cls)}">${esc(cls === 'spec' ? 'Spec' : CLASS_LABEL[cls] || cls)}</span>`;
function fpath(x, p, cut) {
  const m = meaningFor(x);
  if (m && m.cls !== 'code') return `<span class="fp fmn">${classBadge(m.cls)}<span class="fmt">${esc(m.items[0]?.text || p)}</span>${m.items.length > 1 ? `<span class="fmore">+${m.items.length - 1}</span>` : ''}</span>`;
  return `<span class="fp"><span class="fdir">${esc(cut >= 0 ? p.slice(0, cut + 1) : '')}</span>${esc(cut >= 0 ? p.slice(cut + 1) : p)}</span>`;
}
export function fileRow(x, opts = {}) {
  const p = x.path || '';
  const cut = p.lastIndexOf('/');
  const [bl, bt0] = BASIS[x.attribution?.basis] || BASIS.none;
  const bt = bt0 + (x.attribution?.late_at ? `\ntool attributed late, at ${x.attribution.late_at}` : '') + (x.attribution?.tool ? `\ntool call #${x.attribution.tool.seq}: ${x.attribution.tool.name} ${x.attribution.tool.input ?? ''}` : '');
  const tt = toolText(x);
  const tool = tt.seq != null ? `${tt.name} #${tt.seq}` : tt.name;
  if (opts.two) return `<button class="fr fr2" data-key="f:${x.seq}" data-fs="${x.seq}" title="${esc(`${x.op} ${x.root}/${p}${x.from_path ? ` (from ${x.from_path})` : ''}\n${bt}`)}"><span class="op op-${x.op}">${OP[x.op] || '?'}</span>${fpath(x, p, cut)}${x.binary || x.too_large || (x.lines_added == null && x.op === 'rename') ? lines(x) + '<span></span>' : lines(x)}<span class="fmeta">${toolHtml(tt)}<span class="fb b-${bl}">${bl}</span><span class="fat">${esc(clock(Date.parse(x.at)))}</span>${x.root && !x.root.startsWith('llm/') && !x.root.startsWith('decider/') ? `<span class="froot">${esc(x.root)}</span>` : ''}</span></button>`;
  return `<button class="fr${meaningFor(x)?.cls && meaningFor(x).cls !== 'code' ? ' fr-m' : ''}" data-key="f:${x.seq}" data-fs="${x.seq}" title="${esc(`${x.op} ${x.root}/${p}${x.from_path ? ` (from ${x.from_path})` : ''}\n${bt}`)}"><span class="op op-${x.op}">${OP[x.op] || '?'}</span>${fpath(x, p, cut)}${lines(x)}${opts.tool === false ? '' : toolHtml(tt)}<span class="fb b-${bl}">${bl}</span>${opts.time ? `<span class="fat">${esc(clock(Date.parse(x.at)))}</span>` : ''}</button>`;
}
function files(list, caseRel) {
  if (!list.length) return '';
  const shown = list.slice(-3).reverse();
  const add = list.reduce((n, x) => n + (x.lines_added || 0), 0), del = list.reduce((n, x) => n + (x.lines_removed || 0), 0);
  const paths = new Set(list.map((x) => x.root + '/' + x.path)).size;
  return `<div class="nc-files"><div class="nf-h"><span>${paths} file${paths === 1 ? '' : 's'} changed</span><span class="fa">+${add}</span><span class="fd">−${del}</span>${list.length > 3 ? `<a href="#" data-tab="changes" class="nf-more">all ${list.length} changes</a>` : ''}</div>${shown.map((x) => fileRow(x)).join('')}</div>`;
}

const SINCE_SRC = {driver: 'the driver’s observed_at', server: 'the server’s clock when the event line appeared', call: 'the call’s own start'};
// A step whose driver is gone: started, never finished, and nothing alive to finish it.
function goneCard(r, now) {
  const rec = r.record;
  const t = r.since ?? r.approx;
  const call = rec ? (rec.status?.alive ? `<span class="nc-warn">its call (pid ${esc(rec.status.pid)}) still runs</span>` : '') : '';
  return `<div class="nc gone k-stuck" data-key="gone:${esc(r.c.rel)}:${r.start.seq}" data-goto="${esc(r.c.rel)}" data-node="action:${esc(r.action)}" title="${esc(`${r.action} in ${r.c.id}: started, not finished\n${r.driver.why}${rec ? `\nrecord ${rec.rel}` : ''}`)}">
    ${orb('stuck', false)}
    <div class="nc-main">
      <div class="nc-1"><span class="nc-verb bad">Driver gone</span><b class="nc-act">${esc(r.action)}</b><span class="nc-in">in</span><span class="nc-case">${esc(r.c.id)}</span></div>
      <div class="nc-2"><span class="nc-x">Started and never finished. ${esc(r.driver.why)}.</span>${call}</div>
      <div class="nc-3"><span class="nc-why">Nothing runs this step now. Continue the run to have a driver take the tree again.</span></div>
    </div>
    <div class="nc-time">${t != null ? `<span class="tk">${fmtDur(now - t)}</span>` : '<span class="tk">—</span>'}<span class="nc-since">${r.since != null ? `started ${clock(r.since)}` : r.approx != null ? `run since ${clock(r.approx)}` : ''}</span></div>
  </div>`;
}

function runCard(r, now) {
  const rec = r.record;
  const execLine = r.kind === 'llm' ? `${tag('llm')}<span class="nc-x"><b>${esc(r.exec.model || 'model not recorded')}</b></span><span class="nc-dim">via ${esc(r.exec.detail.split(' ')[0])}</span>`
    : r.kind === 'copy' ? `${tag('copy')}<span class="nc-x">${esc(r.exec.detail)}</span>`
    : r.kind === 'driver' ? `${tag('driver')}<span class="nc-x">${esc(r.exec.detail)}</span>`
    : `${tag(r.kind)}<span class="nc-x"><code>${esc(r.exec.detail)}</code></span>`;
  const t = r.since ?? r.approx;
  const over = r.timeout && r.since != null && now - r.since > r.timeout;
  const prog = r.kind === 'llm' ? (rec ? progress(rec, r.exec.maxTurns) : `<span class="np-s">no call record yet</span>`) : '';
  return `<div class="nc run k-${r.kind}${over ? ' over' : ''}" data-key="run:${esc(r.c.rel)}:${r.start.seq}" data-goto="${esc(r.c.rel)}" data-node="action:${esc(r.action)}" title="${esc(`${r.action} in ${r.c.id}\n${KIND_LABEL[r.kind]}: ${r.exec.detail}${r.since != null ? `\nstarted ${new Date(r.since).toLocaleString()} (${SINCE_SRC[r.sinceSrc] || r.sinceSrc})` : '\nstart time not observed (the event carries only the run’s --at)'}${r.driver.state === 'attached' ? `\ndriver pid ${r.driver.pid}` : ''}${rec ? `\nrecord ${rec.rel}` : ''}`)}">
    ${orb(r.kind, true)}
    <div class="nc-main">
      <div class="nc-1"><span class="nc-verb">Running</span><b class="nc-act">${esc(r.action)}</b><span class="nc-in">in</span><span class="nc-case">${esc(r.c.id)}</span></div>
      <div class="nc-2">${execLine}</div>
      ${prog ? `<div class="nc-3">${prog}</div>` : ''}
      ${r.kind === 'llm' ? turnBar(rec, r.exec.maxTurns) : ''}
      ${files(r.files, r.c.rel)}
    </div>
    <div class="nc-time">${t != null ? since(t, now) : '<span class="tk">—</span>'}<span class="nc-since">${r.since != null ? `since ${clock(r.since)}` : r.approx != null ? `run since ${clock(r.approx)}` : ''}</span>${over ? '<span class="nc-warn">past its timeout</span>' : rec?.status?.alive === false ? `<span class="nc-warn">call pid ${esc(rec.status.pid)} is gone</span>` : rec?.lastAt && now - rec.lastAt > 90000 ? `<span class="nc-warn">silent ${ago(rec.lastAt, now)}</span>` : ''}</div>
  </div>`;
}

function waitCard(w, now, item) {
  const rec = w.record;
  const t = w.since ?? w.approx;
  const h = w.holder || {who: w.kind, label: w.kind === 'decider' ? 'Decider' : 'a person', note: ''};
  const fields = fieldsOf(item?.artifact?.json);
  const what = [w.capability && `<span class="nc-kv"><i>capability</i><code>${esc(w.capability)}</code></span>`, w.subject && `<span class="nc-kv"><i>subject</i><code>${esc(w.subject)}${w.revision ? ` @ ${esc(String(w.revision).slice(0, 9))}` : ''}</code></span>`].filter(Boolean).join('');
  // who holds the decision (UXF-011): the decider deciding now, else the person, with why
  const who = w.kind === 'decider'
    ? `<span class="nc-hk">Held by</span>${tag('decider', `Decider ${rec.model || ''}`.trim())}<span class="nc-dim">${esc(h.note)}${rec.type ? ` · ${esc(rec.type)}` : ''}</span>`
    : `<span class="nc-hk">Held by</span>${tag(h.who === 'person' ? 'person' : 'decider', h.label)}<span class="nc-dim nc-hn" title="${esc(h.note)}">${esc(h.note)}</span>`;
  const verb = w.kind === 'decider' ? 'Deciding' : h.who !== 'person' ? 'Open decision' : w.verdict === 'awaiting_authority' ? 'Awaiting authority' : 'Awaiting a person';
  // one decision over several fields (UXF-012), and what taking it does
  const one = fields ? `<span class="nc-one"><b>1 decision over ${fields.length} fields</b> · ${esc(fieldCounts(fields))}</span>` : '';
  // a decision no person holds is drawn as the decider's
  const kc = w.kind === 'person' && h.who !== 'person' ? 'decider' : w.kind;
  return `<div class="nc wait k-${kc}" data-key="wait:${esc(w.c.rel)}:${esc(w.action)}:${w.event.seq}" data-goto="${esc(w.c.rel)}"${w.action ? ` data-node="action:${esc(w.action)}"` : ''} title="${esc(`${w.c.id}: ${w.verdict.replace('_', ' ')}\n${w.decides}\nheld by ${h.label}${h.note ? `: ${h.note}` : ''}${rec ? `\nrecord ${rec.rel}` : ''}`)}">
    ${orb(kc, w.kind === 'decider')}
    <div class="nc-main">
      <div class="nc-1"><span class="nc-verb">${verb}</span><b class="nc-act">${esc(w.action || w.verdict.replace('_', ' '))}</b><span class="nc-in">in</span><span class="nc-case">${esc(w.c.id)}</span></div>
      <div class="nc-2 nc-hold">${who}</div>
      <div class="nc-2 nc-what">${one}${what}<span class="nc-taken">${esc(takenText(w))}</span></div>
      ${w.kind === 'decider' ? `<div class="nc-3">${progress(rec)}</div>${turnBar(rec)}` : `<div class="nc-3"><span class="nc-why">${esc(w.handoff ? w.decides.replace(/ in \/\S+/, '') : w.decides.replace(/ \(read \/.*\)$/, ''))}</span>${w.kind === 'person' && h.who === 'person' ? '<button class="btn sm nc-btn" data-open-drawer="1">Decide</button>' : ''}</div>`}
      ${w.questions?.length ? `<div class="nc-qs">${w.questions.slice(0, 3).map((q) => `<div class="nc-q" data-qid="${esc(q.id)}"><b>${esc(q.id)}</b><span>${esc(q.question)}</span>${q.awaiting ? '' : `<i>answered: ${esc(q.answer)} · ${esc(q.answered_by)}</i>`}</div>`).join('')}${w.questions.length > 3 ? `<div class="nc-q more">and ${w.questions.length - 3} more</div>` : ''}</div>` : ''}
    </div>
    <div class="nc-time">${t != null ? since(t, now) : '<span class="tk">—</span>'}<span class="nc-since">${w.since != null ? `${w.kind === 'decider' ? 'waiting' : 'waiting'} since ${clock(w.since)}` : ''}</span></div>
  </div>`;
}

// "No step can run: plan.decompose used its 3 runs (last error: …)"
export function stallText(stall) {
  const x = stall?.exhausted?.[0];
  if (!x) return 'No step can run: the driver waits on amend.upstream, but no finding needs a person';
  return `No step can run: ${x.action} used its ${x.max} runs${x.error ? ` (last error: ${x.error})` : ` (its last run exited ${x.exit}; no error recorded)`}`;
}

function restCard(st, now) {
  const root = st.root;
  if (st.mode === 'stopped') {
    const s = st.stopped;
    const was = wasLines(st);
    return `<div class="nc idle k-stopped" data-key="rest:stopped" data-goto="${esc(root.rel)}" title="${esc(`run-stopped.json: stopped by ${s.by || 'not recorded'} at ${s.at}\nverdict at the stop: ${s.verdict_at_stop || 'not recorded'}${s.case ? ` in ${s.case}` : ''}`)}">${orb('stopped')}<div class="nc-main">
      <div class="nc-1"><span class="nc-verb">Stopped</span><span class="nc-in">by</span><b class="nc-act">${esc(s.by || 'not recorded')}</b><span class="nc-in">at</span><span class="nc-case">${esc(clock(s.t))}</span></div>
      <div class="nc-2"><span class="nc-x">${esc(s.reason || 'No reason recorded.')}</span></div>
      ${was.map((w) => `<div class="nc-3 nc-was"${w.rel ? ` data-goto="${esc(w.rel)}" data-seq="${w.seq}"` : ''}><span class="nc-why">${esc(w.text[0].toUpperCase() + w.text.slice(1))}${w.ms != null ? ` (${esc(fmtDur(w.ms))} until the stop)` : ''}</span></div>`).join('')}
      <div class="nc-3"><span class="nc-dim">Nothing runs and nothing is decided until the run is resumed.</span></div></div>
      <div class="nc-time"><span class="tk">${esc(clock(s.t))}</span><span class="nc-since">stopped</span></div></div>`;
  }
  if (st.mode === 'stalled') {
    return st.stalled.map(({c, stall, since: t}) => `<div class="nc idle k-stuck" data-key="rest:stalled:${esc(c.rel)}" data-goto="${esc(c.rel)}" title="${esc(`the driver recorded awaiting_person with next amend.upstream; findings.yaml has no finding with status needs_person`)}">${orb('stuck')}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Stopped</span><b class="nc-act">${esc(c.id)}</b><span class="nc-out bad">no step can run</span></div>
      <div class="nc-2 nc-what"><span class="nc-x">${esc(stallText(stall))}</span></div>
      <div class="nc-3"><span class="nc-why">Not a decision: the driver says it waits on amend.upstream, but no finding has status needs_person.</span></div></div>${t ? `<div class="nc-time">${since(t, now)}<span class="nc-since">since ${clock(t)}</span></div>` : ''}</div>`).join('');
  }
  const last = st.lastT;
  if (st.mode === 'closed') {
    return `<div class="nc idle k-done" data-key="rest:closed" data-goto="${esc(root.rel)}">${orb('done')}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Closed</span><b class="nc-act">${esc(root.id)}</b><span class="nc-in">ended</span><span class="nc-out">${esc(st.term.outcome)}</span></div>
      <div class="nc-2"><span class="nc-x">Nothing running and nothing waiting.</span><span class="nc-dim">${st.steps} steps${last ? ` · last change ${ago(last, now)}` : ''}</span></div></div></div>`;
  }
  if (st.mode === 'stuck') {
    return st.stuck.map((c) => { const e = lastVerdictish(c); return `<div class="nc idle k-stuck" data-key="rest:stuck:${esc(c.rel)}" data-goto="${esc(c.rel)}">${orb('stuck')}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Stopped</span><b class="nc-act">${esc(c.id)}</b><span class="nc-out bad">${esc(e.event.replace(/_/g, ' '))}</span></div>
      <div class="nc-2"><span class="nc-x">Nothing running. ${e.event === 'stuck' ? 'No admissible action moves this case and nobody is asked to decide.' : 'A budget of the run is spent.'}</span></div></div>${timeOf(c, e) ? `<div class="nc-time">${since(timeOf(c, e), now)}<span class="nc-since">since ${clock(timeOf(c, e))}</span></div>` : ''}</div>`; }).join('');
  }
  if (st.mode === 'terminable') {
    return st.terminable.map((c) => { const ev = evalAt(c, null); const outs = Object.entries(ev?.outcomes || {}).filter(([, o]) => o.status === 'legitimate').map(([k]) => k); return `<div class="nc idle k-done" data-key="rest:term:${esc(c.rel)}" data-goto="${esc(c.rel)}">${orb('done')}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Terminable</span><b class="nc-act">${esc(c.id)}</b>${outs.length ? `<span class="nc-in">may end</span><span class="nc-out">${esc(outs.join(' or '))}</span>` : ''}</div>
      <div class="nc-2"><span class="nc-x">Nothing running. The case can close; the driver closes it on its next run.</span></div></div></div>`; }).join('');
  }
  if (st.mode === 'between') {
    const c = st.between[st.between.length - 1];
    const t = Math.max(0, ...Object.values(c.times || {})) || null;
    return `<div class="nc idle k-driver" data-key="rest:between" data-goto="${esc(c.rel)}">${orb('driver', true)}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Between steps</span><b class="nc-act">${esc(c.id)}</b></div>
      <div class="nc-2"><span class="nc-x">No action in flight. The driver is evaluating or choosing the next step${st.between.length > 1 ? ` (${st.between.length} open cases)` : ''}.</span></div></div>${t ? `<div class="nc-time">${since(t, now)}<span class="nc-since">since the last event</span></div>` : ''}</div>`;
  }
  return `<div class="nc idle k-idle" data-key="rest:idle">${orb('idle')}<div class="nc-main"><div class="nc-1"><span class="nc-verb">Idle</span></div><div class="nc-2"><span class="nc-x">Nothing running.</span></div></div></div>`;
}

function trail(st, now) {
  if (!st.recent.length) return '';
  return `<div class="trail" data-key="trail"><div class="tr-h">Just finished</div>${st.recent.map((s) => {
    const ok = s.exit === '0' && !s.discarded;
    const t = s.t ?? s.tApprox;
    return `<div class="tr${ok ? '' : s.discarded ? ' bad' : ' nz'}" data-key="tr:${esc(s.c.rel)}:${s.end.seq}" data-goto="${esc(s.c.rel)}" data-seq="${s.end.seq}" title="${esc(`${s.end.action} in ${s.c.id} · exit ${s.exit}${s.discarded ? ' · result discarded' : ''}\n${s.result}\nduration: ${s.dur != null ? fmtSpan(s.dur) + ` (${s.durSrc})` : 'not recorded (no wall clock per step)'}`)}">
      <span class="tr-k k-${s.kind}"></span><b class="tr-a">${esc(s.end.action)}</b><span class="tr-c">${esc(shortCase(s.c))}</span><span class="tr-r">${esc(s.discarded ? 'discarded' : s.result)}</span><span class="tr-d">${s.dur != null ? fmtSpan(s.dur) : '—'}</span><span class="tr-t">${s.t != null ? ago(s.t, now) : esc(clock(t))}</span></div>`;
  }).join('')}</div>`;
}

// Whether a driver is attached, where the run says so (driver.json, the lock, observed.jsonl); an old run says nothing.
function driverChip(st) {
  const d = st.driver;
  if (d?.attached.length) {
    const x = d.attached[0];
    return `<span class="now-drv on" title="${esc(`${x.via}${x.argv ? `\n${x.argv.join(' ')}` : ''}`)}">driver attached<i>pid ${esc(x.pid)}${x.started ? ` · since ${clock(x.started)}` : ''}</i></span>`;
  }
  // the absence of a driver is never a status of its own (UXF-010): only a started step that nothing
  // finishes says "Driver gone"; a wait, a stop and the end are said in words by the band's head
  if (d?.gone.length && st.mode === 'gone') return `<span class="now-drv off" title="${esc(d.gone[0].why)}">Driver gone</span>`;
  return '';
}

// Why a stuck case moves no more: its open obligations at the stop, else the plain rule.
function stuckReason(c) {
  const ev = evalAt(c, null);
  const open = Object.entries(ev?.obligations || {}).filter(([, o]) => o?.status === 'open').map(([k]) => k);
  return open.length ? `open: ${open.slice(0, 3).join(', ')}${open.length > 3 ? ` and ${open.length - 3} more` : ''}` : 'nobody is asked to decide';
}

// What a stopped run was doing at the stop, in the past tense: the file's verdict and case first,
// the waits and steps the store still shows open after them.
const PAST = {awaiting_person: 'waiting on a person', awaiting_authority: 'awaiting authority', stuck: 'stuck', terminable: 'terminable', terminated: 'closed', step_budget_spent: 'out of steps', llm_budget_spent: 'out of LLM calls', wall_time_spent: 'out of time'};
export function wasLines(st) {
  const s = st.stopped || {};
  const out = [];
  const past = (v) => (st.autopilot && v === 'awaiting_person' ? 'waiting on a decision' : PAST[v] || String(v).replace(/_/g, ' '));
  for (const w of st.waiting) out.push({text: `was ${past(w.verdict)}${w.action ? ` for ${w.action}` : ''} in ${w.c.id}${w.since != null ? ` since ${clock(w.since)}` : ''}`, rel: w.c.rel, seq: w.event.seq, ms: w.since != null ? s.t - w.since : null});
  for (const r of [...st.running, ...st.gone]) out.push({text: `was running ${r.action} in ${r.c.id}${r.since != null ? ` since ${clock(r.since)}` : ''}`, rel: r.c.rel, seq: r.start.seq, ms: r.since != null ? s.t - r.since : null});
  for (const x of st.stalled) out.push({text: `could run no step in ${x.c.id}: ${stallText(x.stall).replace(/^No step can run: /, '')}`, rel: x.c.rel, seq: x.event.seq, ms: null});
  if (!out.length && s.verdict_at_stop) out.push({text: `was ${past(s.verdict_at_stop)}${s.case ? ` in ${s.case}` : ''}`, rel: null, seq: null, ms: null});
  // the file's own case first
  return out.sort((a, b) => (b.text.includes(` in ${s.case}`) ? 1 : 0) - (a.text.includes(` in ${s.case}`) ? 1 : 0));
}
function wasText(st) {
  const w = wasLines(st);
  return w.length ? `It ${w[0].text}${w.length > 1 ? ` (and ${w.length - 1} more)` : ''}` : 'Nothing was running or waiting';
}

// The band's head: what is true now, in words.
export function nowSay(st) {
  const n = st.waiting.length;
  const mine = st.waiting.filter((w) => w.kind !== 'decider' && w.holder?.who !== 'rule' && w.holder?.who !== 'decider').length;
  if (st.mode === 'stopped') {
    const s = st.stopped;
    return {head: `Stopped by ${s.by || 'someone (not recorded)'} at ${clock(s.t)}${s.reason ? `: ${s.reason}` : ''}`, sub: wasText(st)};
  }
  if (st.mode === 'waiting' && st.autopilot) {
    // autopilot: never "waiting on you"
    if (st.waiting.some((w) => w.kind === 'decider')) return {head: 'The decider is deciding', sub: n > 1 ? `${n} decisions open` : ''};
    return {head: 'No decision is pending: the decider takes the next one when the run continues', sub: ''};
  }
  if (st.mode === 'waiting') {
    if (mine) return {head: `Waiting on you: nothing runs until ${mine > 1 ? `these ${mine} are` : 'this is'} decided`, sub: n > mine ? `${n - mine} held by a decider` : ''};
    if (st.waiting.some((w) => w.kind === 'decider')) return {head: 'Decider deciding: the run goes on when it decides', sub: n > 1 ? `${n} waiting` : ''};
    return {head: `Held by the ${st.waiting[0].holder?.who === 'rule' ? 'rule decider' : 'decider'}: it decides when the run continues`, sub: n > 1 ? `${n} waiting` : ''};
  }
  if (st.mode === 'stuck') {
    const c = st.stuck[0];
    const e = lastVerdictish(c);
    const t = timeOf(c, e) ?? at(e);
    const what = e.event === 'stuck' ? 'stuck' : e.event.replace(/_/g, ' ');
    return {head: `Stopped: ${what}${t != null ? ` since ${clock(t)}` : ''}, no step is admissible (${e.event === 'stuck' ? stuckReason(c) : 'a budget of the run is spent'})`, sub: ''};
  }
  if (st.mode === 'stalled') return {head: stallText(st.stalled[0].stall), sub: st.stalled.length > 1 ? `${st.stalled.length} cases` : ''};
  if (st.mode === 'closed') return {head: `Closed${st.term?.outcome ? `: ended ${st.term.outcome}` : ''}`, sub: ''};
  const head = {running: `${st.running.length} running`, gone: 'Nothing running', terminable: 'Nothing running', between: 'Between steps', idle: 'Nothing running'}[st.mode];
  const sub = st.mode === 'gone' ? `a started step has no live driver${n ? ` · ${n} waiting` : ''}` : st.mode === 'running' && n ? `${n} waiting${mine && !st.autopilot ? ` on you` : ''}` : '';
  return {head, sub};
}

export function renderNow(st, now, items = []) {
  const {head, sub} = nowSay(st);
  const itemOf = (w) => (items || []).find((i) => i.case === w.c.id && (i.action === w.action || i.approval?.action === w.action));
  const cards = st.mode === 'stopped' ? restCard(st, now) : st.running.map((r) => runCard(r, now)).join('') + st.gone.map((r) => goneCard(r, now)).join('') + st.waiting.map((w) => waitCard(w, now, itemOf(w))).join('') + (st.running.length || st.gone.length || st.waiting.length ? '' : restCard(st, now));
  return `<div class="now-h" data-key="h"><span class="now-l">Now</span><span class="now-s m-${st.mode}"><i></i>${esc(head)}</span>${sub ? `<span class="now-sub">${esc(sub)}</span>` : ''}${driverChip(st)}</div>
    <div class="now-cards n${st.mode === 'stopped' ? 1 : Math.min(3, st.running.length + st.gone.length + st.waiting.length || 1)}" data-key="cards">${cards}</div>${trail(st, now)}`;
}

// ---- file changes of the run ----------------------------------------------------------------------
export function renderChanges(live, cases, onlyCase, now = Date.now()) {
  if (!live?.fs?.present) return `<div class="sect">File changes</div><p class="muted">No <code>fs/changes.jsonl</code> in the run directory yet. The recorder (<code>sdc-watch</code>, docs/fs-changes.md) writes one line per effective change, with the command that made it.</p>`;
  const list = live.fs.changes.filter((x) => !onlyCase || x.attribution?.case === onlyCase);
  if (!list.length) return `<div class="sect">File changes</div><p class="muted">No change recorded${onlyCase ? ` in ${esc(onlyCase)}` : ''} yet.</p>`;
  const groups = [];
  for (const x of list) {
    const a = x.attribution || {};
    const key = `${a.case || '—'}|${a.action || '—'}|${a.record || ''}`;
    const g = groups[groups.length - 1];
    if (g && g.key === key) g.items.push(x); else groups.push({key, a, items: [x]});
  }
  const running = new Set(inFlight(cases).map(({c, start}) => `${c.id}|${start.action}`));
  const ts = list.map((x) => Date.parse(x.at)).filter((t) => !isNaN(t));
  const t0 = Math.min(...ts), t1 = Math.max(...ts, t0 + 1);
  const add = list.reduce((n, x) => n + (x.lines_added || 0), 0), del = list.reduce((n, x) => n + (x.lines_removed || 0), 0);
  const nfiles = new Set(list.map((x) => x.root + '/' + x.path)).size;
  let h = `<div class="sect">File changes <span class="muted">${list.length} changes · ${nfiles} files · <span class="fa">+${add}</span> <span class="fd">−${del}</span></span></div>`;
  h += `<div class="chg-strip" data-key="strip">${list.map((x) => { const t = Date.parse(x.at); return `<button class="cs-t op-${x.op} b-${(BASIS[x.attribution?.basis] || BASIS.none)[0]}" data-key="st:${x.seq}" data-fs="${x.seq}" style="left:${(((t - t0) / (t1 - t0)) * 100).toFixed(2)}%" title="${esc(`${clock(t)} ${x.op} ${x.path} +${x.lines_added ?? 0} −${x.lines_removed ?? 0}\n${x.attribution?.action || ''} ${x.attribution?.case || ''}`)}"></button>`; }).join('')}</div><div class="chg-axis"><span>${esc(clock(t0))}</span><span>${esc(clock(t1))}</span></div>`;
  h += `<div class="chg-groups">`;
  for (const g of groups.reverse()) {
    const live = running.has(`${g.a.case}|${g.a.action}`);
    const gadd = g.items.reduce((n, x) => n + (x.lines_added || 0), 0), gdel = g.items.reduce((n, x) => n + (x.lines_removed || 0), 0);
    const c = cases.find((x) => x.id === g.a.case);
    const kind = g.a.record?.startsWith('decider/') ? 'decider' : g.a.record ? 'llm' : c && g.a.action ? executorOf(c, {action: g.a.action}).kind : 'driver';
    h += `<div class="cg${live ? ' live' : ''}" data-key="g:${esc(g.key)}:${g.items[0].seq}"><div class="cg-h">${tag(kind)}<b class="mono">${esc(g.a.action || 'no action in flight')}</b><span class="muted mono">${esc(g.a.case || '')}</span>${live ? '<span class="cg-live">live</span>' : ''}<span class="cg-n"><span class="fa">+${gadd}</span> <span class="fd">−${gdel}</span></span><span class="cg-t">${esc(clock(Date.parse(g.items[0].at)))}</span></div>
      ${g.items.slice().reverse().map((x) => fileRow(x, {two: true})).join('')}</div>`;
  }
  return h + `</div><p class="foot">From <code>fs/changes.jsonl</code> (sdc.fs-change/1). <b>exact</b>: inside one driver action’s window. <b>inferred</b>: matched to one agent tool call by the timestamps of its event stream. Click a file for its diff.</p>`;
}

// ---- one diff -------------------------------------------------------------------------------------
export function parseDiff(text) {
  const out = [];
  let a = 0, b = 0;
  for (const l of (text || '').split('\n')) {
    if (l.startsWith('--- ') || l.startsWith('+++ ')) continue;
    const m = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)/.exec(l);
    if (m) { a = Number(m[1]); b = Number(m[2]); out.push({t: 'h', text: l, ctx: m[3].trim()}); continue; }
    if (l.startsWith('\\')) { out.push({t: 'meta', text: l}); continue; }
    if (l[0] === '+') out.push({t: '+', text: l.slice(1), b: b++});
    else if (l[0] === '-') out.push({t: '-', text: l.slice(1), a: a++});
    else if (l[0] === ' ' || (l === '' && out.length)) { out.push({t: ' ', text: l.slice(1), a: a++, b: b++}); }
  }
  while (out.length && out[out.length - 1].t === ' ' && out[out.length - 1].text === '' && out[out.length - 1].a == null) out.pop();
  return out;
}

export function renderDiff(x, pos, total) {
  const lang = langOf(x.path);
  const a = x.attribution || {};
  const [bl, bt] = BASIS[a.basis] || BASIS.none;
  const p = x.path || '';
  const cut = p.lastIndexOf('/');
  const head = `<div class="dh"><div class="dh-1"><span class="op op-${x.op}">${OP[x.op] || '?'}</span><span class="dh-p"><span class="fdir">${esc(x.root ? x.root + '/' : '')}${esc(cut >= 0 ? p.slice(0, cut + 1) : '')}</span><b>${esc(cut >= 0 ? p.slice(cut + 1) : p)}</b></span>${x.from_path ? `<span class="muted">from ${esc(x.from_path)}</span>` : ''}<span class="fa">+${x.lines_added ?? 0}</span><span class="fd">−${x.lines_removed ?? 0}</span>
      <span class="dh-nav"><button class="ib" data-fsnav="-1" title="previous change (←)"${pos <= 0 ? ' disabled' : ''}>‹</button><span class="dh-pos">${pos + 1} / ${total}</span><button class="ib" data-fsnav="1" title="next change (→)"${pos >= total - 1 ? ' disabled' : ''}>›</button><button class="ib wide" data-fsclose="1">Close</button></span></div>
    <div class="dh-2"><span class="dh-kv"><i>action</i><b class="mono">${esc(a.action || '—')}</b></span><span class="dh-kv"><i>case</i><span class="mono">${esc(a.case || '—')}</span></span><span class="dh-kv"><i>by</i><span class="mono">${a.tool ? `${esc(a.tool.name)} #${a.tool.seq}${a.tool.input ? ` <span class="muted">${esc(String(a.tool.input).slice(0, 80))}</span>` : ''}` : a.record ? esc(a.record) : 'the step'}</span></span><span class="dh-kv" title="${esc(bt)}"><i>basis</i><span class="fb b-${bl}">${bl}</span> <span class="muted">${esc(a.basis || 'none')}</span></span><span class="dh-kv"><i>at</i><span class="mono">${esc(new Date(Date.parse(x.at)).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}))}</span></span>${lang ? `<span class="dh-lang">${esc(LANG_LABEL[lang])}</span>` : ''}</div></div>`;
  let body;
  if (x.binary) body = `<p class="d-note">Binary file: no text diff.</p>`;
  else if (x.too_large) body = `<p class="d-note">Too large to diff (the recorder kept the blobs <code>${esc((x.before || '').slice(0, 12))}</code> → <code>${esc((x.after || '').slice(0, 12))}</code>).</p>`;
  else if (!x.diff) body = `<p class="d-note">No diff in the record${x.op === 'delete' ? ': the file was deleted' : ''}.</p>`;
  else {
    const rows = parseDiff(x.diff);
    const sa = {}, sb = {};
    body = `<div class="dt">${rows.map((r) => {
      if (r.t === 'h') return `<div class="dl hunk"><span class="ln"></span><span class="ln"></span><span class="sg"></span><span class="code">${esc(r.text.replace(/ @@.*$/, ' @@'))}${r.ctx ? ` <span class="hctx">${esc(r.ctx)}</span>` : ''}</span></div>`;
      if (r.t === 'meta') return `<div class="dl meta"><span class="ln"></span><span class="ln"></span><span class="sg"></span><span class="code">${esc(r.text)}</span></div>`;
      if (r.t === ' ') highlightLine(r.text, lang, sa); // a context line moves both sides' state
      const code = highlightLine(r.text, lang, r.t === '-' ? sa : sb);
      return `<div class="dl ${r.t === '+' ? 'add' : r.t === '-' ? 'del' : 'ctx'}"><span class="ln">${r.a ?? ''}</span><span class="ln">${r.b ?? ''}</span><span class="sg">${r.t === ' ' ? '' : r.t === '-' ? '−' : '+'}</span><span class="code">${code || ' '}</span></div>`;
    }).join('')}</div>`;
    if (x.diff_source) body += `<p class="d-note">${esc(x.diff_source)}</p>`;
  }
  const m = meaningFor(x);
  const mean = m && m.cls !== 'code' ? `<div class="dmean c-${m.cls}" data-key="dmean">${classBadge(m.cls)}<ul>${m.items.map((i) => `<li class="mi-${i.kind || 'chg'}">${esc(i.text)}</li>`).join('')}</ul>${m.parsed === false && m.error ? `<span class="muted">not parsed (${esc(m.error)}): the text diff below is the record</span>` : ''}</div>` : '';
  return head + mean + `<div class="db">${body}</div>`;
}
