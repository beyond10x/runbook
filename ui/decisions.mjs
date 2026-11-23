// The decisions inbox: what a person must decide in a case tree, and what was decided. It reads; it
// acts on nothing. A decision is answered through the daemon (`runbook serve`, `runbook decide`).
//
// Pending items come from the run's own records: the `next` list of a case's latest verdict
// event (awaiting_person, awaiting_authority) and the open scope-changing questions of the artifact
// the runbook's view names (decisions.scope_answer). Which action, artifact and fields a
// runbook has is its view (web/view.js, `decisions`); this file knows the runtime's records only.
import {readText, readJson, leavesRun, exists, hasDotPart, within} from './runfiles.mjs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {principalOf} from './web/principal.js';
import {scopeAnswer, stallRule} from './web/view.js';

const VERDICTS = ['awaiting_authority', 'awaiting_person', 'stuck', 'step_budget_spent', 'llm_budget_spent', 'wall_time_spent', 'terminable'];
const sha = (t) => crypto.createHash('sha256').update(t).digest('hex');

function lastVerdict(events) {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i];
    if (e.event === 'evaluated' || e.event === 'revised') continue;
    return VERDICTS.includes(e.event) ? e : null;
  }
  return null;
}
function latestEval(c) {
  for (let i = c.events.length - 1; i >= 0; i--) {
    const e = c.events[i];
    if (e.event === 'evaluated' && c.evaluations[e.file]) return c.evaluations[e.file];
  }
  return null;
}
function refs(p, out = {claims: [], evidence: []}) {
  if (!p || typeof p !== 'object') return out;
  if (Array.isArray(p)) { p.forEach((x) => refs(x, out)); return out; }
  if (p.claim) out.claims.push(p.claim.id);
  if (p.evidence) out.evidence.push(p.evidence.kind);
  for (const k of ['all', 'any', 'not', 'first', 'before']) if (p[k]) refs(p[k], out);
  return out;
}
// A file of the run only: a link that leaves the run (the work directory itself, or the artifact) is no artifact.
// Every read below has the run as its root (`runDir`): a file whose real path is not under the run is read as absent.
function artifactPath(c, name, runDir) {
  const rel = c.bindings?.$artifacts?.[name];
  const work = path.join(c.dir, 'work');
  if (!rel || !exists(work, runDir)) return null;
  const abs = path.resolve(work, rel);
  return abs.startsWith(work + path.sep) && !hasDotPart(path.relative(work, abs)) && !leavesRun(abs, runDir) ? abs : null;
}
// The value of YAML text, or null. The text is given to `yq` on standard input: no program opens a file of a run by its
// name, so what is parsed is the text that was read through the checked open. A parse failure is an absent value and
// `yq`'s own messages (which name a file) go nowhere.
export function yamlOf(text) {
  if (typeof text !== 'string') return null;
  try { return JSON.parse(execFileSync('yq', ['-o=json', '.'], {input: text, encoding: 'utf8', timeout: 10000, stdio: ['pipe', 'pipe', 'ignore']})); } catch { return null; }
}
// the YAML file of a run read once through the checked open (root: the run), then parsed
export const yamlJson = (file, runDir) => yamlOf(readText(file, runDir));
function readArtifact(c, name, runDir) {
  const abs = artifactPath(c, name, runDir);
  if (!abs) return {name, missing: true};
  const text = readText(abs, runDir);
  if (text == null) return {name, path: c.bindings.$artifacts[name], missing: true};
  // the revision the run records for a file: t + 16 hex digits of its sha256
  return {name, path: c.bindings.$artifacts[name], text: text.slice(0, 200000), sha: sha(text), work_revision: `t${sha(text).slice(0, 16)}`, json: /\.ya?ml$/.test(abs) ? yamlOf(text) : null,
    revision: c.case?.artifacts?.[name]?.revision};
}

// What a record of `kind` would make TRUE, and what reads those claims.
function unlocks(c, kind) {
  const ir = c.ir || {};
  const ev = latestEval(c);
  const direct = Object.entries(ir.claims || {}).filter(([, d]) => refs(d.true_when).evidence.includes(kind)).map(([id]) => id);
  const claims = new Set(direct);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [id, d] of Object.entries(ir.claims || {})) if (!claims.has(id) && refs(d.true_when).claims.some((x) => claims.has(x))) { claims.add(id); grew = true; }
  }
  const touches = (p) => refs(p).claims.some((x) => claims.has(x));
  return {
    claims: [...claims].map((id) => ({id, now: ev?.claims?.[id]?.value || 'unknown'})),
    actions: Object.entries(ir.actions || {}).filter(([, d]) => touches(d.precondition)).map(([id]) => ({id, now: ev?.actions?.[id]?.status || null})),
    outcomes: Object.entries(ir.outcomes || {}).filter(([, d]) => touches(d.requires)).map(([id]) => ({id, now: ev?.outcomes?.[id]?.status || null})),
    obligations: Object.entries(ir.obligations || {}).filter(([, d]) => touches(d.discharged_when)).map(([id]) => ({id, now: (ev?.obligations || []).find((o) => o.id === id)?.status || null})),
  };
}

function pendingApprovals(c) {
  const v = lastVerdict(c.events);
  if (!v || (v.event !== 'awaiting_person' && v.event !== 'awaiting_authority')) return [];
  const ev = latestEval(c);
  const next = Array.isArray(v.next) && v.next.length ? v.next
    : Object.entries(ev?.actions || {}).filter(([a, x]) => x.status === 'approval-required' || (x.status === 'admissible' && c.bindings?.[a]?.manual)).map(([action]) => ({action}));
  return next.filter((n) => n && n.action).map((n) => ({verdict: v, ...n}));
}

// ---- scope-changing questions ----------------------------------------------------------------------------
// A runbook may have an action that a person's answer to a scope-changing question unlocks: the view names
// it (decisions.scope_answer: `action`, the `artifact` and its `list` of records, the record's `flag` and
// `fields`). The page shows the questions and their answers; it writes none.
export const scopeAction = () => scopeAnswer()?.action || null;
// the record of the scope artifact as the page knows it, under the view's own field names
const sf = (cfg, name) => cfg?.fields?.[name] || name;
const openStatus = (cfg) => cfg?.statuses?.open || 'open';
const questionOf = (cfg, q) => ({id: q[sf(cfg, 'id')], item: q[sf(cfg, 'item')], question: q[sf(cfg, 'question')], options: q[sf(cfg, 'options')] || [], default: q[sf(cfg, 'default')] ?? null,
  status: q[sf(cfg, 'status')], answer: q[sf(cfg, 'answer')] ?? null, answered_by: q[sf(cfg, 'answered_by')] ?? null,
  awaiting: q[sf(cfg, 'status')] === openStatus(cfg), scope: !cfg.flag || !!q[cfg.flag]});
// a case waits on the scope action when its latest verdict lists it among `next`
export function waitsOnScope(events) {
  const v = lastVerdict(events);
  const action = scopeAction();
  return !!action && !!v && (v.event === 'awaiting_authority' || v.event === 'awaiting_person') && Array.isArray(v.next) && v.next.some((n) => n?.action === action);
}
// the scope-changing questions of a case's scope artifact; `awaiting` is a question still open
// (an answer with no principal waits for the approval, which names it)
export function scopeQuestions(c, runDir) {
  const cfg = scopeAnswer();
  if (!cfg) return null;
  const file = artifactPath(c, cfg.artifact, runDir);
  const text = file && readText(file, runDir);
  if (text == null) return null;
  const j = yamlOf(text);
  const questions = (j?.[cfg.list] || []).map((q) => questionOf(cfg, q)).filter((q) => q.scope).map(({scope, ...q}) => q);
  return {path: c.bindings.$artifacts[cfg.artifact], sha: sha(text), questions};
}

// A wait on the view's stall action (decisions.stall) with no record that needs a person is not a
// decision: the driver says awaiting_person when nothing else can run, for example when a writer used up
// its max_runs. Returns null when the runbook has no such rule or the case does not wait on that action;
// else {needsPerson, exhausted: [{action, runs, max, exit, error}]} with the actions that used up their
// runs and the last error of each, read from the action's last record (its LLM call's apply.log or
// stderr, else the step's log blob).
export const stallAction = () => stallRule()?.action || null;
export function amendStall(c, runDir) {
  const rule = stallRule();
  const v = lastVerdict(c.events || []);
  if (!rule || !v || v.event !== 'awaiting_person' || !(v.next || []).some((n) => n?.action === rule.action)) return null;
  const file = artifactPath(c, rule.artifact, runDir);
  const j = file ? yamlJson(file, runDir) : null;
  const needsPerson = (j?.[rule.list] || []).filter((f) => f?.[rule.status || 'status'] === (rule.needs_person || 'needs_person')).length;
  if (needsPerson) return {needsPerson, exhausted: []};
  const exhausted = [];
  for (const [a, b] of Object.entries(c.bindings || {})) {
    if (!b || typeof b !== 'object' || !b.max_runs) continue;
    const fin = (c.events || []).filter((e) => e.event === 'action_finished' && e.action === a && e.seq < v.seq);
    if (fin.length < b.max_runs || String(fin.at(-1).exit) === '0') continue;
    exhausted.push({action: a, runs: fin.length, max: b.max_runs, exit: String(fin.at(-1).exit), error: lastError(c, a, fin.at(-1), runDir)});
  }
  return {needsPerson: 0, exhausted};
}
function lastLine(text) {
  return (text || '').split('\n').map((l) => l.trim()).filter((l) => l && !/^metaharness: the transcript was written by/.test(l)).at(-1) || null;
}
// Both names come from the run's own records, so both are held: the record directory under the run (with no dot part in
// its name), the step log under the case's blobs/. A name that leaves its root is read as absent.
function lastError(c, action, fin, runDir) {
  if (runDir) {
    const calls = jsonl(path.join(runDir, 'llm-calls.jsonl'), runDir).filter((r) => r.action === action && (!r.case || r.case === c.id));
    const rec = calls.at(-1)?.record;
    if (typeof rec === 'string' && rec) {
      const dir = path.resolve(runDir, rec);
      if (within(dir, path.resolve(runDir)) && !hasDotPart(path.relative(runDir, dir))) {
        for (const f of ['apply.log', 'stderr.txt']) { const l = lastLine(readText(path.join(dir, f), runDir)); if (l) return l; }
      }
    }
  }
  if (typeof fin?.log === 'string' && fin.log) {
    const blobs = path.resolve(c.dir, 'blobs'), file = path.resolve(blobs, fin.log);
    if (within(file, blobs) && !hasDotPart(path.relative(blobs, file))) { const l = lastLine(readText(file, c.dir)); if (l) return l; }
  }
  return null;
}

export function inbox(ctx) {
  const items = [];
  const seenFiles = new Set();
  for (const c of ctx.cases) {
    const asked = new Set();
    for (const n of pendingApprovals(c)) {
      const decl = c.ir?.actions?.[n.action];
      const b = c.bindings?.[n.action] || {};
      const cap = decl?.requires?.[0]?.capability;
      const base = {case: c.id, rel: c.rel, action: n.action, decides: n.decides || null, waiting: n.verdict.event, since: n.verdict.at, seq: n.verdict.seq, description: decl?.description};
      if (cap) {
        const subject = b.subject || null;
        const revision = subject ? c.case?.artifacts?.[subject]?.revision : null;
        let scope = null;
        if (cap === scopeAction()) {
          // One act for the person: each question still unanswered carries the approval, and the
          // answer that leaves none unanswered records it.
          scope = scopeQuestions(c, ctx.runDir);
          const waiting = (scope?.questions || []).filter((q) => q.awaiting);
          for (const q of waiting) {
            asked.add(q.id);
            items.push({id: `question:${c.rel}:${q.id}`, kind: 'question', case: c.id, rel: c.rel, qid: q.id, question: q.question, options: q.options, default: q.default,
              item: q.item, sha: scope.sha, path: scope.path, revision, answer: q.answer,
              approval: {action: n.action, capability: cap, decides: n.decides || null, since: n.verdict.at, seq: n.verdict.seq, last: waiting.length === 1, waiting: waiting.length, waiting_on: n.verdict.event}});
          }
          if (waiting.length) continue;
        }
        items.push({...base, id: `approve:${c.rel}:${n.action}`, kind: 'approve', capability: cap, manual: !!b.manual, subject, revision, artifact: subject ? readArtifact(c, subject, ctx.runDir) : null,
          upstream: (b.upstream || []).map((u) => ({name: u, revision: c.case?.artifacts?.[u]?.revision})),
          unlocks: b.produces ? unlocks(c, b.produces) : null, produces: b.produces || null, scope,
          grant: [readJson(path.join(c.dir, 'authority.json'), null, ctx.runDir)].flat().find((g) => g && g.capability === cap) || null});
      } else {
        // not a decision: the stall action with no record for a person (the Now band says why nothing runs)
        if (n.action === stallAction() && amendStall(c, ctx.runDir)?.needsPerson === 0) continue;
        items.push({...base, id: `resume:${c.rel}:${n.action}`, kind: 'resume'});
      }
    }
    const sc = scopeAnswer();
    const qfile = sc ? artifactPath(c, sc.artifact, ctx.runDir) : null;
    if (qfile && !seenFiles.has(qfile) && exists(qfile, ctx.runDir)) {
      seenFiles.add(qfile);
      const text = readText(qfile, ctx.runDir);
      const j = yamlOf(text);
      for (const raw of j?.[sc.list] || []) {
        const q = questionOf(sc, raw);
        if (!q.awaiting || !q.scope || asked.has(q.id)) continue;
        items.push({id: `question:${c.rel}:${q.id}`, kind: 'question', case: c.id, rel: c.rel, qid: q.id, question: q.question, options: q.options, default: q.default,
          item: q.item, sha: sha(text), path: c.bindings.$artifacts[sc.artifact], revision: c.case?.artifacts?.[sc.artifact]?.revision, approval: null});
      }
    }
  }
  return items;
}

// ---- history: every decision already taken, by whom ------------------------------------------------
// A principal `decider:<model>` or `decider:<rule>` is an agent deciding in a person's place
// (runs/<run>/decisions.jsonl); any other principal is a person.
// who a principal is (web/principal.js): person, decider, rule, legacy ("human" without an id), agent; null when none
export const whoKind = (p) => { const k = principalOf(p).kind; return k === 'none' ? null : k; };
export function jsonl(file, root) {
  const out = [];
  for (const l of (readText(file, root) || '').split('\n')) { if (!l.trim()) continue; try { out.push(JSON.parse(l)); } catch { /* a line mid-write */ } }
  return out;
}
const asList = (x) => (Array.isArray(x) ? x : x == null || x === '' ? [] : [x]);
// runs/<run>/decisions.jsonl: {time, case, type, action, question, capability, outcome, revision,
// decision, answer, principal, reasons, cites, model, adapter, tokens, cost_usd, duration_ms, valid,
// invalid_reason, dry, record}
export function decisionRecord(r) {
  const qid = r.qid || r.question_id || (typeof r.question === 'string' && /^[\w.-]{1,24}$/.test(r.question) ? r.question : null);
  const decision = r.answer != null && r.decision != null && r.answer !== r.decision ? `${r.decision}: ${r.answer}` : r.answer ?? r.decision ?? r.verdict ?? null;
  return {at: r.at || r.time || r.decided_at || null, principal: r.principal || r.answered_by || null, case: r.case || null, type: r.type || null,
    what: r.action || qid || r.outcome || r.capability || r.type || null, question: r.question && r.question !== qid ? r.question : null,
    decision, reasons: asList(r.reasons ?? r.reason), cites: asList(r.cites), revision: r.revision || null,
    model: r.model || null, cost: r.cost_usd ?? r.cost ?? null, ms: r.duration_ms ?? null,
    invalid: r.valid === false ? r.invalid_reason || 'invalid' : null, dry: !!r.dry, record: r.record || null};
}
export function history(ctx) {
  const out = [];
  for (const r of jsonl(path.join(ctx.runDir, 'decisions.jsonl'), ctx.runDir)) out.push({src: 'decisions.jsonl', ...decisionRecord(r)});
  const seenFiles = new Set();
  const steps = [];
  for (const c of ctx.cases) {
    for (const e of c.events) {
      if (e.event === 'authority_recorded') out.push({src: 'events.jsonl', at: e.at, principal: e.principal, case: c.id, what: `grant ${e.capability}`, decision: e.decision, reasons: [], seq: e.seq});
      else if (e.event === 'action_finished' && e.principal) {
        const kinds = (e.evidence || []).map((id) => c.evidence?.[id]?.kind || id);
        const row = {src: 'events.jsonl', at: e.at, principal: e.principal, case: c.id, what: e.action, decision: String(e.exit) === '0' ? `recorded ${kinds.join(', ') || 'no evidence'}` : `exit ${e.exit}`, reasons: [], seq: e.seq};
        steps.push(row);
        out.push(row);
      }
    }
    const sc = scopeAnswer();
    const qfile = sc ? artifactPath(c, sc.artifact, ctx.runDir) : null;
    if (!qfile || seenFiles.has(qfile) || !exists(qfile, ctx.runDir)) continue;
    seenFiles.add(qfile);
    for (const q of yamlJson(qfile, ctx.runDir)?.[sc.list] || []) {
      if (!q[sf(sc, 'answered_by')]) continue;
      out.push({src: c.bindings.$artifacts[sc.artifact], at: q[sf(sc, 'answered_at')] || null, principal: q[sf(sc, 'answered_by')], case: c.id, what: q[sf(sc, 'id')], question: q[sf(sc, 'question')],
        decision: q[sf(sc, 'answer')], reasons: asList(q[sf(sc, 'reason')] ?? q[sf(sc, 'reasons')]), status: q[sf(sc, 'status')], scope: !!(sc.flag && q[sc.flag])});
    }
  }
  // acts an earlier version of this page recorded (the page takes none now); an approve joins its event row
  const secs = (t) => Date.parse(t || '') / 1000;
  for (const r of jsonl(path.join(ctx.runDir, 'ui-actions.jsonl'), ctx.runDir)) {
    if (r.action === 'continue-finished' || r.action === 'check') continue;
    const cs = r.case ? path.basename(r.case) : null;
    if (r.action === 'approve' && r.exit === 0) {
      const ev = steps.find((s) => s.case === cs && s.what === r.target && Math.abs(secs(s.at) - secs(r.at)) <= 30);
      if (ev) { ev.revision = r.revision; ev.src = 'events.jsonl · ui-actions.jsonl'; continue; }
    }
    const verb = {approve: 'approve', answer: 'answer', context: 'change', continue: 'continue run'}[r.action] || r.action;
    out.push({src: 'ui-actions.jsonl', at: r.at, principal: r.principal, case: cs, what: [verb, r.target].filter(Boolean).join(' '),
      decision: r.refused ? 'refused' : r.action === 'answer' ? r.answer : r.action === 'context' ? r.value : r.exit === 0 ? 'done' : r.exit == null ? 'started' : `exit ${r.exit}`,
      reasons: [r.refused, r.reason, r.error].filter(Boolean), revision: r.revision || null, refused: !!r.refused});
  }
  for (const h of out) h.who = whoKind(h.principal) || 'none';
  out.sort((a, b) => (b.at || '').localeCompare(a.at || ''));
  return out.slice(0, 400);
}
