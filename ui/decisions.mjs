// The decisions inbox: what a person must decide in a case tree, and the four acts that decide it.
//
// Pending items come from the driver's own records: the `next` list of a case's latest verdict
// event (awaiting_person, awaiting_authority) and the open scope-changing questions of the artifact
// the runbook's view names (decisions.scope_answer). Which action, artifact, fields and command a
// runbook has is its view (web/view.js, `decisions`); this file knows the runtime's records only.
// Acts are whitelisted; each builds an argv (never a shell string), checks that the revision the
// page showed is still the current one, and appends one line to <run>/ui-actions.jsonl.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFile, execFileSync, spawn} from 'node:child_process';
import {principalOf} from './web/principal.js';
import {toolOwners, scopeAnswer, stallRule, contextRule, continueRule, runbookName} from './web/view.js';

const VERDICTS = ['awaiting_authority', 'awaiting_person', 'stuck', 'step_budget_spent', 'llm_budget_spent', 'wall_time_spent', 'terminable'];
const sha = (t) => crypto.createHash('sha256').update(t).digest('hex');
const now = () => new Date().toISOString().replace(/\.\d+Z$/, 'Z');
const readText = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };
const readJson = (p) => { try { return JSON.parse(fs.readFileSync(p, 'utf8')); } catch { return null; } };

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
function workDir(c) { try { return fs.realpathSync(path.join(c.dir, 'work')); } catch { return null; } }
function artifactPath(c, name) {
  const rel = c.bindings?.$artifacts?.[name];
  const work = workDir(c);
  if (!rel || !work) return null;
  const abs = path.resolve(work, rel);
  return abs.startsWith(work + path.sep) ? abs : null;
}
function yamlJson(file) {
  try { return JSON.parse(execFileSync('yq', ['-o=json', '.', file], {encoding: 'utf8', timeout: 10000})); } catch { return null; }
}
function readArtifact(c, name) {
  const abs = artifactPath(c, name);
  if (!abs) return {name, missing: true};
  const text = readText(abs);
  if (text == null) return {name, path: c.bindings.$artifacts[name], missing: true};
  // the driver's revision of a file: t + 16 hex digits of its sha256 (driver/src/main.rs revision_of)
  return {name, path: c.bindings.$artifacts[name], text: text.slice(0, 200000), sha: sha(text), work_revision: `t${sha(text).slice(0, 16)}`, json: /\.ya?ml$/.test(abs) ? yamlJson(abs) : null,
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

// ---- scope-changing questions and the approval that now goes with answering them -----------------------
// A runbook may have an action that a person's answer to a scope-changing question unlocks: the view names
// it (decisions.scope_answer: `action`, whose capability is named like it, the `check` action that records
// the changed artifact, the `artifact` and its `list` of records, the record's `flag` and `fields`). A
// named principal takes the capability with the driver's `approve --capability C --principal P --action C`,
// and the program refuses an answer whose answerer is empty or one of `refused_answerers`.
//
// The order that works (observed with the driver on a stub store): the person's answer goes into the
// artifact with its status and WITHOUT the answerer; the `check` action then records the new revision
// (shape valid, settled "needs a scope answer"), which keeps the action admissible; the approval runs
// the driver's tool, which writes the answerer = the principal on every scope-changing answer that has
// none. Writing the principal's name first would make the check answer "settled", and Canon then no
// longer asks for the action at all (its precondition is "not once settled"), so the capability would
// never be exercised.
export const scopeAction = () => scopeAnswer()?.action || null;
export const scopeCheck = () => scopeAnswer()?.check || null;
export const principalOk = (p) => typeof p === 'string' && /^[A-Za-z0-9._-]+$/.test(p) && !(scopeAnswer()?.refused_answerers || []).includes(p);
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
export function scopeQuestions(c) {
  const cfg = scopeAnswer();
  if (!cfg) return null;
  const file = artifactPath(c, cfg.artifact);
  const text = file && readText(file);
  if (text == null) return null;
  const j = yamlJson(file);
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
  const file = artifactPath(c, rule.artifact);
  const j = file ? yamlJson(file) : null;
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
function lastError(c, action, fin, runDir) {
  if (runDir) {
    const calls = jsonl(path.join(runDir, 'llm-calls.jsonl')).filter((r) => r.action === action && (!r.case || r.case === c.id));
    const rec = calls.at(-1)?.record;
    if (rec) {
      const dir = path.isAbsolute(rec) ? rec : path.join(runDir, rec);
      for (const f of ['apply.log', 'stderr.txt']) { const l = lastLine(readText(path.join(dir, f))); if (l) return l; }
    }
  }
  if (fin?.log) { const l = lastLine(readText(path.join(c.dir, 'blobs', fin.log))); if (l) return l; }
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
          scope = scopeQuestions(c);
          const waiting = (scope?.questions || []).filter((q) => q.awaiting);
          for (const q of waiting) {
            asked.add(q.id);
            items.push({id: `question:${c.rel}:${q.id}`, kind: 'question', case: c.id, rel: c.rel, qid: q.id, question: q.question, options: q.options, default: q.default,
              item: q.item, sha: scope.sha, path: scope.path, revision, answer: q.answer,
              approval: {action: n.action, capability: cap, decides: n.decides || null, since: n.verdict.at, seq: n.verdict.seq, last: waiting.length === 1, waiting: waiting.length, waiting_on: n.verdict.event}});
          }
          if (waiting.length) continue;
        }
        items.push({...base, id: `approve:${c.rel}:${n.action}`, kind: 'approve', capability: cap, manual: !!b.manual, subject, revision, artifact: subject ? readArtifact(c, subject) : null,
          upstream: (b.upstream || []).map((u) => ({name: u, revision: c.case?.artifacts?.[u]?.revision})),
          unlocks: b.produces ? unlocks(c, b.produces) : null, produces: b.produces || null, scope,
          grant: (readJson(path.join(c.dir, 'authority.json')) || []).find((g) => g.capability === cap) || null});
      } else {
        // not a decision: the stall action with no record for a person (the Now band says why nothing runs)
        if (n.action === stallAction() && amendStall(c, ctx.runDir)?.needsPerson === 0) continue;
        items.push({...base, id: `resume:${c.rel}:${n.action}`, kind: 'resume'});
      }
    }
    const sc = scopeAnswer();
    const qfile = sc ? artifactPath(c, sc.artifact) : null;
    if (qfile && !seenFiles.has(qfile) && fs.existsSync(qfile)) {
      seenFiles.add(qfile);
      const text = readText(qfile);
      const j = yamlJson(qfile);
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
export function jsonl(file) {
  const out = [];
  for (const l of (readText(file) || '').split('\n')) { if (!l.trim()) continue; try { out.push(JSON.parse(l)); } catch { /* a line mid-write */ } }
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
  for (const r of jsonl(path.join(ctx.runDir, 'decisions.jsonl'))) out.push({src: 'decisions.jsonl', ...decisionRecord(r)});
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
    const qfile = sc ? artifactPath(c, sc.artifact) : null;
    if (!qfile || seenFiles.has(qfile) || !fs.existsSync(qfile)) continue;
    seenFiles.add(qfile);
    for (const q of yamlJson(qfile)?.[sc.list] || []) {
      if (!q[sf(sc, 'answered_by')]) continue;
      out.push({src: c.bindings.$artifacts[sc.artifact], at: q[sf(sc, 'answered_at')] || null, principal: q[sf(sc, 'answered_by')], case: c.id, what: q[sf(sc, 'id')], question: q[sf(sc, 'question')],
        decision: q[sf(sc, 'answer')], reasons: asList(q[sf(sc, 'reason')] ?? q[sf(sc, 'reasons')]), status: q[sf(sc, 'status')], scope: !!(sc.flag && q[sc.flag])});
    }
  }
  // the page's own acts; an approve that the driver recorded joins its event row (it adds the revision)
  const secs = (t) => Date.parse(t || '') / 1000;
  for (const r of jsonl(path.join(ctx.runDir, 'ui-actions.jsonl'))) {
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

// ---- acts ------------------------------------------------------------------------------------------
function audit(ctx, rec) {
  try { fs.appendFileSync(path.join(ctx.runDir, 'ui-actions.jsonl'), JSON.stringify({at: now(), principal: ctx.principal, ...rec}) + '\n'); } catch { /* the run dir may be read-only */ }
}
function run(argv, opts = {}) {
  return new Promise((resolve) => {
    execFile(argv[0], argv.slice(1), {timeout: 180000, maxBuffer: 8 << 20, ...opts}, (err, stdout, stderr) => {
      resolve({exit: err ? (typeof err.code === 'number' ? err.code : 1) : 0, stdout: String(stdout), stderr: String(stderr)});
    });
  });
}
const caseOf = (ctx, rel) => ctx.cases.find((c) => c.rel === rel);
const str = (v, max = 500) => typeof v === 'string' && v.trim() && v.length <= max && !/[\0\r]/.test(v);

// ---- preflight: the programs a driver step needs, on the server's own PATH ----------------------------
// A check whose tool cannot run must not be started from here: the driver would record it (a missing
// yq once turned questions.check into an `invalid` record of a real run). So every act that starts the
// driver or a tool first looks for what it needs, and refuses, starting nothing, when one is missing.
export function onPath(p, env = process.env) {
  if (p.includes('/')) { try { fs.accessSync(p, fs.constants.X_OK); return fs.statSync(p).isFile(); } catch { return false; } }
  for (const d of (env.PATH || '').split(':').filter(Boolean)) {
    try { const f = path.join(d, p); fs.accessSync(f, fs.constants.X_OK); if (fs.statSync(f).isFile()) return true; } catch { /* next */ }
  }
  return false;
}
// The programs the runbook's tools need come from its view (web/view.js, tool_owners): `always` for every act,
// `driver` for an act that starts the driver, `continue` for the whole run, and per tool (the program a
// binding runs, matched on its first argument) the programs of each subcommand. Without a view only the
// driver and canon binaries are looked for.
// kind: approve (with the action it may take), answer, context, check (a driver step of `action`) or
// continue (the view's decisions.continue command: the whole trial). Returns the missing programs, [] when all are there.
export function missingPrograms(ctx, {kind, c = null, action = null}, env = process.env) {
  const owners = toolOwners();
  const need = new Set(owners?.always || []);
  if (kind !== 'answer' && kind !== 'context') {
    for (const p of [...(owners?.driver || []), ctx.drive, ctx.canon]) if (p) need.add(p);
    if (kind === 'continue') for (const p of owners?.continue || []) need.add(p);
    const argv = action && c?.bindings?.[action]?.argv;
    if (Array.isArray(argv)) for (const t of owners?.tools || []) if (new RegExp(t.argv0).test(argv[0] || '')) for (const sc of t.subcommands || []) if (new RegExp(sc.match).test(argv[1] || '')) (sc.needs || []).forEach((p) => need.add(p));
  }
  return [...need].filter((p) => !onPath(p, env));
}
export const missingText = (missing) => `cannot start: ${missing.map((p) => (p.includes('/') ? `${path.basename(p)} (${p})` : p)).join(', ')} ${missing.length > 1 ? 'are' : 'is'} not on the server's PATH or not built. Nothing was run, so no check is recorded; restart the server with them on its PATH.`;

export async function act(ctx, b, emit) {
  const refuse = (error, extra = {}) => { audit(ctx, {action: b.kind, case: b.rel, target: b.action || b.qid || b.field, refused: error, ...extra}); return {ok: false, error}; };
  {
    const c0 = b.kind === 'continue' ? null : caseOf(ctx, b.rel);
    // an approval may take its manual action; an answer with scope.answer pending runs questions.check
    // and the approval in the same act
    const scope = b.kind === 'answer' && c0 && pendingApprovals(c0).some((n) => n.action === scopeAction());
    const kind = scope ? 'check' : b.kind;
    const action = b.kind === 'approve' ? b.action : scope ? scopeCheck() : null;
    const missing = missingPrograms(ctx, {kind, c: c0, action});
    if (missing.length) return refuse(missingText(missing), {missing});
  }
  if (b.kind === 'continue') return continueRun(ctx, b, emit, refuse);
  const c = caseOf(ctx, b.rel);
  if (!c) return refuse('no such case');

  if (b.kind === 'approve') return approve(ctx, c, b, refuse, emit);

  if (b.kind === 'answer') {
    const sc = scopeAnswer();
    if (!sc) return refuse('this runbook has no decision a person answers here: its view has no decisions.scope_answer');
    if (!principalOk(ctx.principal)) return refuse(`the server's principal "${ctx.principal}" cannot answer: the program refuses an empty or non-interactive answerer`);
    const file = artifactPath(c, sc.artifact);
    const text = file && readText(file);
    if (text == null) return refuse(`no ${sc.artifact} artifact in this case`);
    if (sha(text) !== b.sha) return refuse(`stale view: ${path.basename(file)} changed since the page showed it; reload and answer again`);
    const q = (yamlJson(file)?.[sc.list] || []).find((x) => x[sf(sc, 'id')] === b.qid);
    if (!q) return refuse(`no question ${b.qid}`);
    const rec = questionOf(sc, q);
    if (!rec.scope) return refuse(`question ${b.qid} does not change the scope: the agent answers it`);
    // with the scope action pending, a scope-changing answer that names no principal is still the person's to change
    const scopePending = pendingApprovals(c).some((n) => n.action === scopeAction());
    const unattributed = !rec.answered_by || (sc.refused_answerers || []).includes(rec.answered_by);
    if (!rec.awaiting && !(scopePending && unattributed)) return refuse(`question ${b.qid} is already ${rec.status}`);
    if (!str(b.answer)) return refuse('the answer must be 1 to 500 characters on one line');
    const status = b.answer === rec.default ? sc.statuses?.accepted || 'accepted' : sc.statuses?.changed || 'changed';
    // Pending scope action: the answer waits without the answerer, and the approval's tool names the
    // principal (see the note on scopeAction). Otherwise the answer is attributed at once.
    const f = (n) => sf(sc, n);
    const by = scopePending ? `del(.${f('answered_by')})` : `.${f('answered_by')} = strenv(RB_BY)`;
    const argv = ['yq', '-i', `(.${sc.list}[] | select(.${f('id')} == strenv(RB_QID))) |= (.${f('answer')} = strenv(RB_ANSWER) | .${f('status')} = strenv(RB_STATUS) | ${by})`, file];
    const r = await run(argv, {env: {...process.env, RB_QID: b.qid, RB_ANSWER: b.answer, RB_STATUS: status, RB_BY: ctx.principal}});
    audit(ctx, {action: 'answer', case: c.id, target: b.qid, file: path.relative(ctx.storePath, file), answer: b.answer, status, answered_by: scopePending ? `${ctx.principal} (written by ${scopeAction()})` : ctx.principal, sha_before: b.sha, revision: b.revision, argv, exit: r.exit});
    ctx.refresh();
    emit('act', {kind: 'answer', case: c.id, qid: b.qid, exit: r.exit});
    if (r.exit !== 0) return {ok: false, exit: r.exit, status, error: r.stderr.slice(-500)};
    if (!scopePending) return {ok: true, exit: 0, status, answered_by: ctx.principal, approved: false, remaining: 0};
    // the last unanswered scope-changing question: the same act records the scope action's approval
    const remaining = (scopeQuestions({dir: c.dir, bindings: c.bindings, events: c.events})?.questions || []).filter((x) => x.awaiting).length;
    if (remaining) return {ok: true, exit: 0, status, answered_by: ctx.principal, approved: false, remaining};
    const a = await approve(ctx, c, {action: scopeAction(), revision: b.revision}, refuse, emit);
    return {...a, status, answered_by: ctx.principal, approved: !!a.ok, remaining: 0, error: a.ok ? null : `answer written as ${ctx.principal}; ${a.error}`};
  }

  if (b.kind === 'context') {
    const cx = contextRule();
    if (!cx) return refuse('this runbook has no context a person changes here: its view has no decisions.context');
    const file = artifactPath(c, cx.artifact);
    const text = file && readText(file);
    if (text == null) return refuse(`no ${cx.artifact} artifact in this case`);
    if (sha(text) !== b.sha) return refuse(`stale view: ${path.basename(file)} changed since the page showed it; reload and change again`);
    const j = yamlJson(file) || {};
    if (!Object.prototype.hasOwnProperty.call(j, b.field) || !new RegExp(cx.field_pattern || '^[a-z_]+$').test(b.field || '')) return refuse(`no ${cx.artifact} field ${b.field}`);
    if (!str(b.value)) return refuse('the value must be 1 to 500 characters on one line');
    if (b.note != null && b.note !== '' && !str(b.note, 300)) return refuse('the note must be at most 300 characters on one line');
    const reason = `${ctx.principal}, ${now().slice(0, 10)}${b.note ? `, ${b.note}` : ''}`;
    const cf = (n) => cx.fields?.[n] || n;
    const argv = ['yq', '-i', `.[strenv(RB_FIELD)].${cf('value')} = strenv(RB_VALUE) | .[strenv(RB_FIELD)].${cf('status')} = strenv(RB_CHANGED) | .[strenv(RB_FIELD)].${cf('reason')} = strenv(RB_REASON)`, file];
    const r = await run(argv, {env: {...process.env, RB_FIELD: b.field, RB_VALUE: b.value, RB_CHANGED: cx.changed || 'changed', RB_REASON: reason}});
    audit(ctx, {action: 'context', case: c.id, target: b.field, file: path.relative(ctx.storePath, file), value: b.value, reason, sha_before: b.sha, revision: b.revision, argv, exit: r.exit});
    ctx.refresh();
    emit('act', {kind: 'context', case: c.id, field: b.field, exit: r.exit});
    return {ok: r.exit === 0, exit: r.exit, reason, error: r.exit === 0 ? null : r.stderr.slice(-500)};
  }
  return refuse(`unknown act ${b.kind}`);
}

// Records an approval the way the driver does: its `approve --capability C --principal P
// [--action A]`, logged to ui-actions.jsonl. For a manual action the driver also takes the action.
async function approve(ctx, c, b, refuse, emit) {
  const pending = pendingApprovals(c).find((n) => n.action === b.action);
  if (!pending) return refuse(`${b.action} is not pending in ${c.id}`);
  const decl = c.ir.actions[b.action];
  const bind = c.bindings[b.action] || {};
  const cap = decl?.requires?.[0]?.capability;
  if (!cap) return refuse(`${b.action} requires no capability`);
  if (cap === scopeAction()) {
    if (!principalOk(ctx.principal)) return refuse(`the server's principal "${ctx.principal}" cannot take scope.answer: the program refuses an empty or non-interactive answerer`);
    // the driver's scope-answer tool gives every scope-changing question still open its default: not for the person to do unseen
    const left = (scopeQuestions(c)?.questions || []).filter((q) => q.awaiting).map((q) => q.id);
    if (left.length) return refuse(`answer ${left.join(', ')} first: approving scope.answer would accept their defaults as ${ctx.principal}`);
  }
  const caseNow = readJson(path.join(c.dir, 'case.json'));
  const revNow = bind.subject ? caseNow?.artifacts?.[bind.subject]?.revision : null;
  if (bind.subject && revNow !== b.revision) return refuse(`stale view: ${bind.subject} is now ${revNow}, the page showed ${b.revision}; reload and review again`, {revision: b.revision, current: revNow});
  // scope.answer is the action that revises the questions: the person's answers sit in the file before it runs
  if (bind.subject && b.sha && cap !== scopeAction()) {
    const f = artifactPath(c, bind.subject);
    const t = f && readText(f);
    if (t != null && sha(t) !== b.sha) return refuse(`stale view: ${bind.$artifacts?.[bind.subject] || bind.subject} changed on disk since the page showed it; reload and review again`, {revision: b.revision});
    if (t != null && `t${sha(t).slice(0, 16)}` !== revNow) return refuse(`${bind.subject} was changed after the run recorded ${revNow}: continue the run so it checks the new revision, then approve that one`, {revision: b.revision, current: `t${sha(t).slice(0, 16)}`});
  }
  if (cap === scopeAction()) {
    // the person's answers moved the questions to a new revision: Canon needs it checked (shape valid,
    // settled needs_scope_answer) before scope.answer is admissible again
    const checkArgv = [ctx.drive, '--canon', ctx.canon, '--case-dir', c.dir, 'step', '--at', now(), '--action', scopeCheck()];
    const k = await run(checkArgv);
    const kerr = k.exit === 0 ? null : (k.stderr.trim().split('\n').at(-1) || '').slice(0, 400);
    audit(ctx, {action: 'check', case: c.id, target: scopeCheck(), argv: checkArgv, exit: k.exit, ...(kerr ? {error: kerr} : {})});
    ctx.refresh();
    if (k.exit !== 0) return {ok: false, exit: k.exit, argv: checkArgv, output: (k.stdout + k.stderr).slice(-6000), error: `${path.basename(ctx.drive)} step ${scopeCheck()} exited ${k.exit}: ${kerr}`};
  }
  const at = now();
  const argv = [ctx.drive, '--canon', ctx.canon, '--case-dir', c.dir, 'approve', '--at', at, '--capability', cap, '--principal', ctx.principal, ...(bind.manual ? ['--action', b.action] : [])];
  const r = await run(argv);
  const errLine = r.exit === 0 ? null : (r.stderr.trim().split('\n').at(-1) || '').slice(0, 400);
  audit(ctx, {action: 'approve', case: c.id, target: b.action, capability: cap, subject: bind.subject, revision: b.revision, argv, exit: r.exit, ...(errLine ? {error: errLine} : {})});
  ctx.refresh();
  emit('act', {kind: 'approve', case: c.id, action: b.action, exit: r.exit});
  return {ok: r.exit === 0, exit: r.exit, argv, output: (r.stdout + r.stderr).slice(-6000), error: r.exit === 0 ? null : `${path.basename(ctx.drive)} approve exited ${r.exit}: ${errLine}`};
}

// ---- continue the run: the view's command <run>, output streamed --------------------------------
const procs = new Map();
export function procState(index) {
  const p = procs.get(index);
  if (!p) return null;
  return {name: p.name, running: p.exit == null, exit: p.exit, started: p.started, log: p.log, tail: p.lines.slice(-400)};
}
function continueRun(ctx, b, emit, refuse) {
  const rule = continueRule();
  if (!rule) return refuse('this runbook has no command to continue a run from here (its view has no decisions.continue): continue it from a shell');
  const rb = runbookName();
  const runs = path.join(ctx.root, rule.runs_dir);
  let runDir;
  try { runDir = fs.realpathSync(ctx.runDir); } catch { return refuse("no run directory"); }
  let runsReal;
  try { runsReal = fs.realpathSync(runs); } catch { runsReal = null; }
  if (path.dirname(runDir) !== runsReal) return refuse(`this store is not a run under ${rb ? rb + "/" : ""}${rule.runs_dir}/; continue it from a shell`);
  const name = path.basename(runDir);
  if (!/^[A-Za-z0-9._-]+$/.test(name)) return refuse("run name has unexpected characters");
  const cur = procs.get(ctx.index);
  if (cur && cur.exit == null) return refuse(`${path.basename(rule.script)} ${cur.name} is already running`);
  const script = path.resolve(ctx.root, rule.script);
  if (!script.startsWith(ctx.root + path.sep)) return refuse(`${rule.script} is outside the repository`);
  if (!fs.existsSync(script)) return refuse(`${rule.script} is missing`);
  const log = path.join(runDir, `ui-continue-${now().replace(/[:]/g, '')}.log`);
  const fd = fs.openSync(log, 'a');
  const argv = [script, name];
  const child = spawn(script, [name], {cwd: ctx.root, stdio: ['ignore', fd, fd], detached: true, env: {...process.env, ...Object.fromEntries(Object.entries(rule.env || {}).map(([k, v]) => [k, v.replace('{principal}', ctx.principal)]))}});
  fs.closeSync(fd);
  child.unref();
  const p = {name, started: now(), exit: null, log: path.relative(ctx.root, log), lines: [], pos: 0, pid: child.pid};
  procs.set(ctx.index, p);
  const pump = () => {
    try {
      const st = fs.statSync(log);
      if (st.size > p.pos) {
        const buf = Buffer.alloc(st.size - p.pos);
        const h = fs.openSync(log, 'r');
        fs.readSync(h, buf, 0, buf.length, p.pos);
        fs.closeSync(h);
        p.pos = st.size;
        const lines = buf.toString('utf8').split('\n').filter((l) => l.length);
        p.lines.push(...lines);
        if (lines.length) emit('proc', {name, lines});
      }
    } catch { /* not yet */ }
  };
  const timer = setInterval(pump, 500);
  child.on('exit', (code) => { setTimeout(() => { pump(); clearInterval(timer); p.exit = code ?? 1; emit('procend', {name, exit: p.exit}); audit(ctx, {action: 'continue-finished', target: name, exit: p.exit}); }, 600); });
  audit(ctx, {action: 'continue', target: name, argv, pid: child.pid, log: p.log});
  emit('proc', {name, lines: [`$ ${rule.script} ${name}   (pid ${child.pid}, output in ${p.log})`]});
  return {ok: true, pid: child.pid, log: p.log, argv};
}
