// Derivations over the stored data. Nothing here re-implements Canon: every status shown comes
// from a stored canon-decision/1 (evaluations/NNNN.json); this module only finds which one applies
// at an event, joins evidence ids to records and events, and counts.
import {principalOf, autopilot, runSettings} from './principal.js';

export const VERDICTS = ['awaiting_authority', 'awaiting_person', 'stuck', 'step_budget_spent', 'llm_budget_spent', 'wall_time_spent', 'terminable'];

// ---- who acts: until Canon has an `author` field, the binding tells ----------------------------
const LLM_ARGV = /(^|[\/\s])stub-|adapter|metaharness|claude|\{profile\}|eval\/levels\//;
export function actorOf(c, action) {
  const b = c.bindings?.[action];
  const decl = c.ir?.actions?.[action];
  if (!b) return {kind: 'unbound', why: 'no binding in this case\'s bindings.json'};
  if (b.llm) return {kind: 'llm', why: 'binding has `llm` (agent node)'};
  if (b.manual) return {kind: 'person', why: 'binding is `manual`: only `sdc-drive approve --action` takes it'};
  if (b.child) return {kind: 'driver', why: 'binding opens child cases (sdc-drive, TEMPORARY Loom copy)'};
  if (b.copy) return {kind: 'driver', why: `binding copies \`${b.copy.kind}\` from \`${b.copy.from}\``};
  const argv = (b.argv || []).join(' ');
  if (/llm/i.test(b.producer || '') || LLM_ARGV.test(argv)) return {kind: 'llm', why: `argv \`${(b.argv || []).slice(1).join(' ')}\` is an LLM executor (stub or adapter)`};
  const indep = c.case?.independence || {};
  if (b.producer && indep[b.producer]) return {kind: 'review', why: `producer \`${b.producer}\` is listed independent (${indep[b.producer].join(', ')}): a judgement`};
  if (decl && /\bLLM\b/.test(decl.description || '')) return {kind: 'llm', why: 'IR description names an LLM'};
  return {kind: 'program', why: `program \`${(b.argv || []).slice(1).join(' ') || '(driver)'}\`; result from its exit code`};
}
// Who a principal is (web/principal.js): person, decider, rule, legacy, agent; null when none.
export const principalKind = (p) => { const k = principalOf(p).kind; return k === 'none' ? null : k; };
export const ACTOR_LABEL = {llm: 'LLM', person: 'person', decider: 'decider', rule: 'rule', legacy: 'recorded as human', agent: 'agent', none: 'not recorded', driver: 'driver', program: 'program', review: 'review', unbound: 'unbound'};

// ---- events, evaluations, time ------------------------------------------------------------------
export function seqOfEvidence(id) { const m = /^e(\d+)-/.exec(id || ''); return m ? Number(m[1]) : null; }

export function evalEventAt(c, seq) {
  let best = null;
  for (const e of c.events) {
    if (seq != null && e.seq > seq) break;
    if (e.event === 'evaluated' && c.evaluations[e.file]) best = e;
  }
  return best;
}
export function evalAt(c, seq) { const e = evalEventAt(c, seq); return e ? c.evaluations[e.file] : null; }
export function lastSeq(c) { return c.events.length ? c.events[c.events.length - 1].seq : 0; }

export function caseState(c) {
  const ev = c.events;
  if (!ev.length) return {state: 'empty', label: 'no events'};
  const term = ev.find((e) => e.event === 'terminated');
  if (term) return {state: 'terminated', label: `ended ${term.outcome}`, outcome: term.outcome};
  for (let i = ev.length - 1; i >= 0; i--) {
    const e = ev[i];
    if (e.event === 'evaluated' || e.event === 'revised') continue;
    if (VERDICTS.includes(e.event)) {
      const label = {awaiting_authority: 'awaiting authority', awaiting_person: autopilot(runSettings()) ? 'awaiting a decision' : 'awaiting a person', stuck: 'stuck', terminable: 'terminable', step_budget_spent: 'step budget spent', llm_budget_spent: 'LLM budget spent', wall_time_spent: 'wall time spent'}[e.event];
      return {state: e.event, label, next: e.next, at: e.at};
    }
    if (e.event === 'action_started') return {state: 'running', label: `running ${e.action}`, action: e.action};
    break;
  }
  return {state: 'open', label: 'open'};
}

export function describeEvent(e) {
  switch (e.event) {
    case 'opened': return `opened, protocol ${e.protocol?.id} r${e.protocol?.revision}`;
    case 'revised': return `revised ${Object.entries(e.artifacts || {}).map(([a, r]) => `${a} → ${r}`).join(', ')} (case ${e.case_revision})`;
    case 'evaluated': return `Canon evaluated → ${e.file}`;
    case 'action_started': return `started ${e.action}${e.llm ? ' (LLM)' : ''}${e.copy ? ' (copy)' : ''}${e.retry_of?.length ? ` retry of ${e.retry_of.join(',')}` : ''}`;
    case 'action_finished': return `finished ${e.action}: exit ${e.exit}${e.principal ? ` by ${principalOf(e.principal).label}` : ''}${e.evidence?.length ? ` → ${e.evidence.join(', ')}` : ', no evidence'}`;
    case 'discarded': return `DISCARDED ${e.action}: ${e.reason}`;
    case 'child_opened': return `opened child ${e.child_case} (${e.protocol})`;
    case 'child_closed': return `child ${e.child_case} ended ${e.outcome}`;
    case 'authority_recorded': return `${principalOf(e.principal).label} ${e.decision} ${e.capability}`;
    case 'terminated': return `terminated: ${e.outcome}`;
    default: return e.event.replace(/_/g, ' ') + (e.next?.length ? ` (next: ${e.next.map((n) => n?.action || n).join(', ')})` : '');
  }
}
export const EVENT_CLASS = {opened: 'ev-open', revised: 'ev-rev', evaluated: 'ev-eval', action_started: 'ev-act', action_finished: 'ev-act', discarded: 'ev-bad', child_opened: 'ev-child', child_closed: 'ev-child', authority_recorded: 'ev-auth', awaiting_authority: 'ev-wait', awaiting_person: 'ev-wait', stuck: 'ev-bad', terminable: 'ev-end', terminated: 'ev-end'};

// Which event produced a record, and which revision moved its subject past it.
export function producerEvent(c, evidenceId) {
  return c.events.find((e) => e.event === 'action_finished' && (e.evidence || []).includes(evidenceId))
    || c.events.find((e) => (e.event === 'child_opened') && e.evidence === evidenceId);
}
export function supersedingRevision(c, record) {
  const from = seqOfEvidence(record.id) ?? -1;
  for (const e of c.events) {
    if (e.event !== 'revised' || e.seq <= from) continue;
    const r = e.artifacts?.[record.subject];
    if (r && r !== record.subject_revision) {
      const started = [...c.events].reverse().find((x) => x.seq < e.seq && x.event === 'action_started');
      const finished = c.events.find((x) => x.seq > e.seq && x.event === 'action_finished');
      return {event: e, to: r, by: started && finished && finished.action === started.action ? started.action : started?.action || null};
    }
  }
  return null;
}

// ---- predicates ---------------------------------------------------------------------------------
export function predicateRefs(p, out = {claims: [], evidence: [], decisions: [], children: []}) {
  if (!p || typeof p !== 'object') return out;
  if (Array.isArray(p)) { p.forEach((x) => predicateRefs(x, out)); return out; }
  if (p.claim) out.claims.push(p.claim);
  if (p.evidence) out.evidence.push(p.evidence);
  if (p.decision) out.decisions.push(p.decision);
  if (p.children) out.children.push(p.children);
  for (const k of ['all', 'any', 'not', 'first', 'before']) if (p[k]) predicateRefs(p[k], out);
  return out;
}

export function predicateText(p, claims, depth = 0) {
  const pad = '  '.repeat(depth);
  if (!p) return pad + '(none)';
  if (p.all) return p.all.length ? `${pad}all of\n${p.all.map((x) => predicateText(x, claims, depth + 1)).join('\n')}` : `${pad}always`;
  if (p.any) return `${pad}any of\n${p.any.map((x) => predicateText(x, claims, depth + 1)).join('\n')}`;
  if (p.not) return `${pad}not\n${predicateText(p.not, claims, depth + 1)}`;
  if (p.claim) { const v = claims?.[p.claim.id]?.value; return `${pad}${p.claim.id} is ${p.claim.is}${v ? `   [now ${v}${v === p.claim.is ? ' ✓' : ' ✗'}]` : ''}`; }
  if (p.evidence) return `${pad}evidence ${p.evidence.kind}${p.evidence.result != null ? ` = ${p.evidence.result}` : ''} about ${p.evidence.subject} at its current revision${p.evidence.independent_on ? `, independent on ${p.evidence.independent_on.join(',')}` : ''}`;
  if (p.decision) return `${pad}decision ${p.decision}`;
  if (p.children) return `${pad}every child case ended ${p.children.in.join(' or ')}`;
  return pad + JSON.stringify(p);
}

// ---- claim history over the stored evaluations --------------------------------------------------
export function claimHistory(c, claim) {
  const out = [];
  for (const e of c.events) if (e.event === 'evaluated' && c.evaluations[e.file]) {
    const v = c.evaluations[e.file].claims?.[claim]?.value;
    if (!out.length || out[out.length - 1].value !== v) out.push({seq: e.seq, at: e.at, value: v});
  }
  return out;
}

// Records excluded by Canon in an evaluation, with reason.
export function excludedIn(ev) {
  const out = [];
  for (const [claim, x] of Object.entries(ev?.explanation?.claims || {})) for (const b of x.because || []) if (b.status === 'excluded') out.push({claim, ...b});
  return out;
}

// ---- metrics ------------------------------------------------------------------------------------
export function caseMetrics(c, meta) {
  const ir = c.ir || {};
  const actions = Object.keys(ir.actions || {});
  const actors = actions.map((a) => actorOf(c, a).kind);
  const ev = c.events;
  const finished = ev.filter((e) => e.event === 'action_finished');
  const kindOf = (a) => actorOf(c, a).kind;
  const ats = ev.map((e) => e.at).filter((a) => a && a !== '-').map((a) => Date.parse(a)).filter((x) => !isNaN(x));
  const last = evalAt(c, null);
  const pid = ir.protocol?.id, rev = ir.protocol?.revision;
  const check = meta?.check?.[`${pid}@${rev}`];
  const revisedArtifacts = ev.filter((e) => e.event === 'revised').reduce((n, e) => n + Object.keys(e.artifacts || {}).length, 0);
  let invalidations = 0;
  const seen = new Set();
  for (const e of ev) if (e.event === 'evaluated' && c.evaluations[e.file]) for (const x of excludedIn(c.evaluations[e.file])) {
    if (x.reason === 'revision_mismatch' && !seen.has(x.evidence)) { seen.add(x.evidence); invalidations++; }
  }
  return {
    states: check ? check.states : null, stateSource: check ? `canon check, ${pid} r${rev}` : `no canon check line for ${pid} r${rev}`,
    findings: check ? check.findings : null,
    artifacts: Object.keys(ir.artifacts || {}).length, evidenceKinds: Object.keys(ir.evidence_kinds || {}).length,
    claims: Object.keys(ir.claims || {}).length, obligations: Object.keys(ir.obligations || {}).length,
    actions: actions.length, outcomes: Object.keys(ir.outcomes || {}).length,
    llmActions: actors.filter((k) => k === 'llm').length, personActions: actors.filter((k) => k === 'person').length,
    reviewActions: actors.filter((k) => k === 'review').length,
    steps: finished.length,
    llmSteps: finished.filter((e) => kindOf(e.action) === 'llm').length,
    personSteps: finished.filter((e) => principalKind(e.principal) === 'person').length, deciderSteps: finished.filter((e) => ['decider', 'rule'].includes(principalKind(e.principal))).length,
    failedSteps: finished.filter((e) => e.exit !== '0' && e.exit !== 0).length,
    approvals: ev.filter((e) => e.event === 'authority_recorded').length,
    waits: ev.filter((e) => e.event === 'awaiting_authority' || e.event === 'awaiting_person').length,
    evaluations: ev.filter((e) => e.event === 'evaluated').length,
    revisions: revisedArtifacts,
    invalidations,
    discarded: ev.filter((e) => e.event === 'discarded').length,
    evidence: Object.keys(c.evidence || {}).length,
    durationMin: ats.length ? (Math.max(...ats) - Math.min(...ats)) / 60000 : null,
    open: last ? (last.obligations || []).filter((o) => o.status === 'open').length : null,
  };
}
