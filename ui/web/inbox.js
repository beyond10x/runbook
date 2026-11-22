// The decisions drawer of a page the inspector server serves: every pending person decision of the
// tree, the artifact to review, what deciding changes, and what was decided. It reads and takes no
// act: a decision is answered through the daemon, as the one line DECIDE_AT says. (The page the
// daemon serves has its own drawer, decide.js.)
import {esc, decisionRows, decisionSummary} from './lenses.js';
import {fieldsOf, fieldCounts, deciderPerField, takenText} from './holder.js';
import {autopilot, runSettings} from './principal.js';
import {scopeAnswer} from './view.js';

export const DECIDE_AT = 'This page only reads. A decision is answered through the daemon: on the page of `runbook serve --ui`, or with `runbook decide <run> <request> <answer>` (`runbook runs decisions <run>` lists the open ones).';
const line = (t) => esc(t).replace(/`([^`]+)`/g, '<code>$1</code>');

const val = (v) => `<span class="v v-${esc(v)}">${esc(String(v ?? 'unknown').toUpperCase())}</span>`;
const st = (s) => `<span class="v st-${esc(s)}">${esc(s ?? '—')}</span>`;

// A small Markdown renderer for AEP artifacts: front matter, headings, lists, code, emphasis.
export function md(text) {
  let src = String(text || '');
  let front = '';
  const fm = /^---\n([\s\S]*?)\n---\n?/.exec(src);
  if (fm) { front = `<pre class="front">${esc(fm[1])}</pre>`; src = src.slice(fm[0].length); }
  const inline = (s) => esc(s).replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>').replace(/(^|[^*])\*([^*]+)\*/g, '$1<i>$2</i>');
  const out = [];
  let list = null, code = null, para = [];
  const flush = () => { if (para.length) { out.push(`<p>${inline(para.join(' '))}</p>`); para = []; } if (list) { out.push(`<${list.t}>${list.items.map((i) => `<li>${inline(i)}</li>`).join('')}</${list.t}>`); list = null; } };
  for (const line of src.split('\n')) {
    if (code) { if (/^```/.test(line)) { out.push(`<pre>${esc(code.join('\n'))}</pre>`); code = null; } else code.push(line); continue; }
    if (/^```/.test(line)) { flush(); code = []; continue; }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) { flush(); out.push(`<h${h[1].length + 2}>${inline(h[2])}</h${h[1].length + 2}>`); continue; }
    const li = /^\s*(?:[-*]|(\d+)\.)\s+(.*)$/.exec(line);
    if (li) { if (para.length) { out.push(`<p>${inline(para.join(' '))}</p>`); para = []; } const t = li[1] ? 'ol' : 'ul'; if (!list || list.t !== t) { if (list) flush(); list = {t, items: []}; } list.items.push(li[2]); continue; }
    if (!line.trim()) { flush(); continue; }
    if (list) flush();
    para.push(line.trim());
  }
  flush();
  return front + out.join('');
}

// The fields one approval covers (UXF-012): each with its value, its reason and its state (default
// kept, accepted or changed by whom), and what the decider's reply says about it when it decided.
function contextTable(item, perField) {
  const j = item.artifact?.json || {};
  const rows = Object.entries(j).filter(([, v]) => v && typeof v === 'object' && 'value' in v);
  if (!rows.length) return `<pre class="pred">${esc(item.artifact?.text || '')}</pre>`;
  const state = Object.fromEntries((fieldsOf(j) || []).map((f) => [f.name, f.state]));
  return `<table class="ctx"><thead><tr><th>field</th><th>value and reason</th><th>state</th></tr></thead><tbody>${rows.map(([k, v]) => {
    const dv = perField?.fields?.[k];
    return `<tr data-key="ctx:${esc(item.rel)}:${esc(k)}" class="${dv === 'decider would change this' ? 'dflag' : ''}"><td class="mono">${esc(k)}</td><td><div class="cv">${esc(v.value)}</div><div class="cr">${esc(v.reason || '')}</div></td><td><span class="cs cs-${esc(v.status)}">${esc(state[k] || v.status)}</span>${dv ? `<span class="cdv">${esc(dv)}</span>` : ''}</td></tr>`;
  }).join('')}</tbody></table>`;
}

function unlockBlock(u, produces) {
  if (!u) return '';
  const list = (xs, f) => xs.map((x) => `<span class="ul">${esc(x.id)} ${f(x.now)}</span>`).join('');
  return `<div class="unl"><div class="unl-h">Approving records <code>${esc(produces)}</code> about this revision. Canon then holds:</div>
    <div class="unl-r"><span class="unl-k">claims → TRUE</span>${list(u.claims, val)}</div>
    ${u.actions.length ? `<div class="unl-r"><span class="unl-k">actions that read them</span>${list(u.actions, st)}</div>` : ''}
    ${u.outcomes.length ? `<div class="unl-r"><span class="unl-k">outcomes</span>${list(u.outcomes, st)}</div>` : ''}
    ${u.obligations.length ? `<div class="unl-r"><span class="unl-k">obligations</span>${list(u.obligations, st)}</div>` : ''}</div>`;
}

// the scope-changing questions the scope action's approval belongs to
function scopeBlock(scope) {
  if (!scope?.questions?.length) return '';
  return `<div class="unl" data-key="scopeqs"><div class="unl-h">Approving takes <code>${esc(scopeAnswer()?.action)}</code>: the scope-changing questions of this interview</div>${scope.questions.map((q) => `<div class="unl-r"><span class="unl-k">${esc(q.id)}</span><span>${esc(q.question)} <b>${esc(q.answer ?? '—')}</b> <span class="muted">${esc(q.answered_by || 'unanswered')}</span></span></div>`).join('')}</div>`;
}

function card(item, S) {
  // an open decision of a stopped run is shown greyed
  const off = !!S.inbox?.stopped;
  const stopChip = off ? '<span class="dstop">run stopped</span>' : '';
  if (item.kind === 'approve') {
    const a = item.artifact || {};
    const edited = item.capability !== scopeAnswer()?.action && !!(a.work_revision && item.revision && a.work_revision !== item.revision);
    // who holds it (UXF-011), from the Now band's derivation of the same wait
    const w = (S.nowState?.waiting || []).find((x) => x.c?.id === item.case && x.action === item.action);
    const h = w?.holder;
    const fields = a.json ? fieldsOf(a.json) : null;
    const perField = fields && h?.decider ? deciderPerField(fields, h.decider) : null;
    const body = a.missing ? `<p class="muted">The artifact file is missing.</p>` : a.json && item.subject === 'context' ? contextTable(item, perField) : /\.md$/.test(a.path || '') ? `<div class="mdv">${md(a.text)}</div>` : `<pre class="pred">${esc(a.text)}</pre>`;
    return `<article class="dcard k-approve${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
      <div class="dc-h"><span class="dk">${item.waiting === 'awaiting_authority' ? 'Authority' : 'Approval'}</span><h3>Approve ${esc(item.subject || item.action)}</h3>${stopChip}<span class="muted mono">${esc(item.case)}</span></div>
      ${fields ? `<p class="dc-one"><b>One decision over ${fields.length} fields</b> · ${esc(fieldCounts(fields))} · one approval takes the whole revision</p>` : ''}
      ${h ? `<p class="dc-who"><span class="dc-hk">Held by</span><b>${esc(h.label)}</b>${h.note ? ` · ${esc(h.note)}` : ''}${w?.since != null ? ` · waiting <span class="tk" data-since="${w.since}"></span>` : ''}</p>` : ''}
      <p class="dc-taken">${esc(takenText({capability: item.capability, subject: item.subject, revision: item.revision}))}</p>
      ${perField?.whole ? `<p class="dc-dv">Decider: ${esc(perField.whole)}</p>` : ''}
      <div class="dc-meta"><span class="chip-r">revision <code>${esc(item.revision)}</code></span><span class="chip-r">capability <code>${esc(item.capability)}</code></span>${item.upstream.map((u) => `<span class="chip-r">against ${esc(u.name)} <code>${esc(u.revision)}</code></span>`).join('')}<span class="chip-r">waiting since ${esc((item.since || '').replace('T', ' ').replace('Z', ''))}</span></div>
      ${item.decides ? `<p class="dec">${esc(item.decides.replace(/\(read [^)]*\)/, '').trim())}</p>` : ''}
      <div class="art"><div class="art-h"><code>${esc(a.path || '')}</code><span class="muted">${fields ? 'the fields one approval covers' : 'the revision to approve'}</span></div>${body}</div>
      ${edited ? `<div class="gap" data-key="edited"><b>Changed since the run recorded it.</b> The file is now <code>${esc(a.work_revision)}</code>, the run recorded <code>${esc(item.revision)}</code>. Canon holds <code>${esc(item.subject)}</code> unchecked until the run checks the new revision.</div>` : ''}
      ${scopeBlock(item.scope)}
      ${unlockBlock(item.unlocks, item.produces)}
      ${item.grant ? `<p class="foot">authority.json already holds <code>${esc(item.grant.capability)}: ${esc(item.grant.decision)}</code>; Canon does not bind it to a revision, so an approval is recorded as a separate ${esc(item.produces)} record about <code>${esc(item.revision)}</code>.</p>` : ''}
      </article>`;
  }
  if (item.kind === 'question') {
    const opts = [...item.options];
    if (item.default != null && !opts.includes(item.default)) opts.unshift(item.default);
    const ap = item.approval;
    const qw = ap ? (S.nowState?.waiting || []).find((x) => x.c?.id === item.case && x.action === ap.action) : null;
    return `<article class="dcard k-question${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
      <div class="dc-h"><span class="dk q">Question</span><h3>${esc(item.question)}</h3>${stopChip}</div>
      ${qw?.holder ? `<p class="dc-who"><span class="dc-hk">Held by</span><b>${esc(qw.holder.label)}</b>${qw.holder.note ? ` · ${esc(qw.holder.note)}` : ''}${qw.since != null ? ` · waiting <span class="tk" data-since="${qw.since}"></span>` : ''}</p>` : ''}
      <div class="dc-meta"><span class="chip-r">${esc(item.qid)} · from item ${esc(item.item)}</span><span class="chip-r">scope-changing</span><span class="chip-r"><code>${esc(item.path)}</code></span>${ap ? `<span class="chip-r ap" title="${esc(ap.decides || '')}">waits on <code>${esc(ap.action)}</code> · capability <code>${esc(ap.capability)}</code> · ${ap.waiting_on === 'awaiting_authority' ? 'awaiting authority' : 'awaiting a person'}</span>` : ''}</div>
      <ul class="opts-ro">${opts.map((o) => `<li>${esc(o)}${o === item.default ? ' <span class="def">default</span>' : ''}</li>`).join('')}</ul></article>`;
  }
  return `<article class="dcard k-resume${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
    <div class="dc-h"><span class="dk r">Then</span><h3>${esc(item.action)}</h3>${stopChip}<span class="muted mono">${esc(item.case)}</span></div>
    <p class="dec">${esc(item.decides || item.description || '')}</p></article>`;
}

export function renderDrawer(S) {
  const ib = S.inbox || {items: []};
  const items = ib.items;
  const auto = autopilot(runSettings());
  return `<div class="dr-h" data-key="drh"><div><h2>Decisions</h2><span class="muted">${items.length ? ib.stopped ? `${items.length} open when the run was stopped` : auto ? `${items.length} open, held by the decider` : `${items.length} waiting for you` : auto ? 'no decision is pending' : 'nothing waiting'}</span></div>
      <button class="ib wide" id="drclose">Close</button></div>
    ${ib.stopped ? `<div class="gap stopgap" data-key="stopped" role="status"><b>Run stopped</b> by ${esc(ib.stopped.by || 'not recorded')} at ${esc(String(ib.stopped.at).replace('T', ' ').replace('Z', ' UTC'))}${ib.stopped.reason ? `: ${esc(ib.stopped.reason)}` : ''}. Its open decisions are shown greyed.</div>` : ''}
    <div class="gap" id="decide-at" data-key="decide-at">${line(DECIDE_AT)}</div>
    <div class="dr-list" data-key="drl">${items.length ? items.map((i) => card(i, S)).join('') : `<div class="empty-d"><div class="big">✓</div><p>${auto ? 'No decision is pending. Autopilot: the decider or a rule takes every decision of this run.' : 'No decision is waiting. The run continues on its own until it needs you.'}</p></div>`}
      <section class="dr-hist" data-key="drhist"><div class="sect">Decided <span class="muted">${(ib.history || []).length}, newest first</span></div>${(ib.history || []).length ? decisionSummary(ib.history) + `<div class="steplist">${decisionRows(ib.history.slice(0, 80), 'dh')}</div>` : '<p class="muted">Nothing decided yet.</p>'}</section></div>`;
}
