// The decisions drawer: every pending person decision of the tree, the artifact to review, what
// deciding changes, and the act itself. The server checks and performs every act (decisions.mjs).
import {esc, decisionRows, decisionSummary} from './lenses.js';
import {fieldsOf, fieldCounts, deciderPerField, takenText} from './holder.js';
import {autopilot, runSettings} from './principal.js';
import {scopeAnswer} from './view.js';

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
function contextTable(item, S, perField) {
  const j = item.artifact?.json || {};
  const rows = Object.entries(j).filter(([, v]) => v && typeof v === 'object' && 'value' in v);
  if (!rows.length) return `<pre class="pred">${esc(item.artifact?.text || '')}</pre>`;
  const state = Object.fromEntries((fieldsOf(j) || []).map((f) => [f.name, f.state]));
  return `<table class="ctx"><thead><tr><th>field</th><th>value and reason</th><th>state</th><th></th></tr></thead><tbody>${rows.map(([k, v]) => {
    const key = `ctx:${item.rel}:${k}`;
    const editing = S.editing === key;
    const dv = perField?.fields?.[k];
    return `<tr data-key="${esc(key)}" class="${editing ? 'editing' : ''}${dv === 'decider would change this' ? ' dflag' : ''}"><td class="mono">${esc(k)}</td><td>${editing
      ? `<input class="tin" data-form="${esc(key)}:value" value="${esc(S.form[`${key}:value`] ?? v.value)}" maxlength="500"><input class="tin note" data-form="${esc(key)}:note" placeholder="why (optional)" value="${esc(S.form[`${key}:note`] ?? '')}" maxlength="300">`
      : `<div class="cv">${esc(v.value)}</div><div class="cr">${esc(v.reason || '')}</div>`}</td><td><span class="cs cs-${esc(v.status)}">${esc(state[k] || v.status)}</span>${dv ? `<span class="cdv">${esc(dv)}</span>` : ''}</td><td class="ca">${editing
      ? `<button class="btn primary sm" data-act="context" data-item="${esc(item.id)}" data-field="${esc(k)}">Save</button><button class="btn sm" data-cancel="1">Cancel</button>`
      : `<button class="btn ghost sm" data-edit="${esc(key)}" ${S.inbox.write && !S.inbox.stopped ? '' : 'disabled'}>Change</button>`}</td></tr>`;
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
  // an open decision of a stopped run is not offered: shown greyed, every control off
  const off = !!S.inbox?.stopped;
  const can = S.inbox.write && !off;
  const stopChip = off ? '<span class="dstop">run stopped</span>' : '';
  const busy = S.busy === item.id;
  const res = S.results[item.id];
  const resHtml = res ? `<div class="res ${res.ok ? 'ok' : 'bad'}">${esc(res.ok ? res.msg || 'done' : res.error)}${res.output ? `<details><summary>driver output</summary><pre class="pred">${esc(res.output)}</pre></details>` : ''}</div>` : '';
  if (item.kind === 'approve') {
    const a = item.artifact || {};
    const edited = item.capability !== scopeAnswer()?.action && !!(a.work_revision && item.revision && a.work_revision !== item.revision);
    // who holds it (UXF-011), from the Now band's derivation of the same wait
    const w = (S.nowState?.waiting || []).find((x) => x.c?.id === item.case && x.action === item.action);
    const h = w?.holder;
    const fields = a.json ? fieldsOf(a.json) : null;
    const perField = fields && h?.decider ? deciderPerField(fields, h.decider) : null;
    const body = a.missing ? `<p class="muted">The artifact file is missing.</p>` : a.json && item.subject === 'context' ? contextTable(item, S, perField) : /\.md$/.test(a.path || '') ? `<div class="mdv">${md(a.text)}</div>` : `<pre class="pred">${esc(a.text)}</pre>`;
    // the one control, first: it approves the whole revision (UXF-001, UXF-012)
    const control = `<div class="dc-a dc-top">${edited
        ? `<button class="btn primary" data-act="continue" ${can && !S.proc?.running ? '' : 'disabled'}>${S.proc?.running ? 'Run in progress…' : 'Continue run to re-check'}</button><span class="muted">approval waits for the checked revision</span>`
        : `<button class="btn primary" data-act="approve" data-item="${esc(item.id)}" ${can && !busy ? '' : 'disabled'}>${busy ? 'Approving…' : `Approve ${esc(item.revision || '')}`}</button><span class="muted">${fields ? `approves all ${fields.length} fields as one revision · ` : ''}as <b>${esc(S.inbox.principal)}</b> · runs <code>sdc-drive approve</code></span>`}</div>`;
    return `<article class="dcard k-approve${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
      <div class="dc-h"><span class="dk">${item.waiting === 'awaiting_authority' ? 'Authority' : 'Approval'}</span><h3>Approve ${esc(item.subject || item.action)}</h3>${stopChip}<span class="muted mono">${esc(item.case)}</span></div>
      ${fields ? `<p class="dc-one"><b>One decision over ${fields.length} fields</b> · ${esc(fieldCounts(fields))} · one approval takes the whole revision</p>` : ''}
      ${h ? `<p class="dc-who"><span class="dc-hk">Held by</span><b>${esc(h.label)}</b>${h.note ? ` · ${esc(h.note)}` : ''}${w?.since != null ? ` · waiting <span class="tk" data-since="${w.since}"></span>` : ''}</p>` : ''}
      <p class="dc-taken">${esc(takenText({capability: item.capability, subject: item.subject, revision: item.revision}))}</p>
      ${perField?.whole ? `<p class="dc-dv">Decider: ${esc(perField.whole)}</p>` : ''}
      ${control}${resHtml}
      <div class="dc-meta"><span class="chip-r">revision <code>${esc(item.revision)}</code></span><span class="chip-r">capability <code>${esc(item.capability)}</code></span>${item.upstream.map((u) => `<span class="chip-r">against ${esc(u.name)} <code>${esc(u.revision)}</code></span>`).join('')}<span class="chip-r">waiting since ${esc((item.since || '').replace('T', ' ').replace('Z', ''))}</span></div>
      ${item.decides ? `<p class="dec">${esc(item.decides.replace(/\(read [^)]*\)/, '').trim())}</p>` : ''}
      <div class="art"><div class="art-h"><code>${esc(a.path || '')}</code><span class="muted">${fields ? 'the fields this one approval covers' : autopilot(runSettings()) ? 'the revision to approve' : 'the revision you approve'}</span></div>${body}</div>
      ${edited ? `<div class="gap" data-key="edited"><b>Changed since the run recorded it.</b> The file is now <code>${esc(a.work_revision)}</code>, the run recorded <code>${esc(item.revision)}</code>. Canon holds <code>${esc(item.subject)}</code> unchecked until the run checks the new revision: Continue run, then approve that revision here.</div>` : ''}
      ${scopeBlock(item.scope)}
      ${unlockBlock(item.unlocks, item.produces)}
      ${item.grant ? `<p class="foot">authority.json already holds <code>${esc(item.grant.capability)}: ${esc(item.grant.decision)}</code>; Canon does not bind it to a revision, so this approval is recorded as a separate ${esc(item.produces)} record by ${esc(S.inbox.principal)} about <code>${esc(item.revision)}</code>.</p>` : ''}
      </article>`;
  }
  if (item.kind === 'question') {
    const key = `q:${item.rel}:${item.qid}`;
    const pick = S.form[`${key}:pick`] ?? item.default;
    const other = S.form[`${key}:other`] ?? '';
    const opts = [...item.options];
    if (item.default != null && !opts.includes(item.default)) opts.unshift(item.default);
    const ap = item.approval;
    const who = S.inbox.principal;
    const after = !ap ? '' : ap.last ? `, then records the <code>${esc(ap.capability)}</code> approval as ${esc(who)}` : `, <code>${esc(ap.capability)}</code> is approved with the last of ${ap.waiting} questions`;
    const qw = ap ? (S.nowState?.waiting || []).find((x) => x.c?.id === item.case && x.action === ap.action) : null;
    return `<article class="dcard k-question${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
      <div class="dc-h"><span class="dk q">Question</span><h3>${esc(item.question)}</h3>${stopChip}</div>
      ${qw?.holder ? `<p class="dc-who"><span class="dc-hk">Held by</span><b>${esc(qw.holder.label)}</b>${qw.holder.note ? ` · ${esc(qw.holder.note)}` : ''}${qw.since != null ? ` · waiting <span class="tk" data-since="${qw.since}"></span>` : ''}</p>` : ''}
      <div class="dc-meta"><span class="chip-r">${esc(item.qid)} · from item ${esc(item.item)}</span><span class="chip-r">scope-changing</span><span class="chip-r"><code>${esc(item.path)}</code></span>${ap ? `<span class="chip-r ap" title="${esc(ap.decides || '')}">waits on <code>${esc(ap.action)}</code> · capability <code>${esc(ap.capability)}</code> · ${ap.waiting_on === 'awaiting_authority' ? 'awaiting authority' : 'awaiting a person'}</span>` : ''}</div>
      <div class="opts">${opts.map((o, i) => `<label class="opt${pick === o ? ' on' : ''}"><input type="radio" name="${esc(key)}" data-form="${esc(key)}:pick" value="${esc(o)}"${pick === o ? ' checked' : ''}><span class="rd"></span><span>${esc(o)}</span>${o === item.default ? '<span class="def">default</span>' : ''}</label>`).join('')}
        <label class="opt${pick === '__other' ? ' on' : ''}"><input type="radio" name="${esc(key)}" data-form="${esc(key)}:pick" value="__other"${pick === '__other' ? ' checked' : ''}><span class="rd"></span><input class="tin" data-form="${esc(key)}:other" placeholder="another answer" value="${esc(other)}" maxlength="500"></label></div>
      <div class="dc-a"><button class="btn primary" data-act="answer" data-item="${esc(item.id)}" ${can && !busy ? '' : 'disabled'}>${busy ? 'Saving…' : ap?.last ? 'Answer and approve' : 'Answer'}</button><span class="muted">writes answer, status ${pick === item.default ? 'accepted' : 'changed'}, answered_by <b>${esc(who)}</b>${after}</span></div>${resHtml}</article>`;
  }
  return `<article class="dcard k-resume${off ? ' stopped' : ''}" data-key="${esc(item.id)}">
    <div class="dc-h"><span class="dk r">Then</span><h3>${esc(item.action)}</h3>${stopChip}<span class="muted mono">${esc(item.case)}</span></div>
    <p class="dec">${esc(item.decides || item.description || '')}</p>
    <div class="dc-a"><button class="btn primary" data-act="continue" ${can && !S.proc?.running ? '' : 'disabled'}>Continue run</button></div>${resHtml}</article>`;
}

export function renderDrawer(S) {
  const ib = S.inbox || {items: [], write: false};
  const items = ib.items;
  const proc = S.proc;
  const auto = autopilot(runSettings());
  return `<div class="dr-h" data-key="drh"><div><h2>Decisions</h2><span class="muted">${items.length ? ib.stopped ? `${items.length} open when the run was stopped` : auto ? `${items.length} open, held by the decider` : `${items.length} waiting for you` : auto ? 'no decision is pending' : 'nothing waiting'} · acting as <b>${esc(ib.principal || '—')}</b></span></div>
      <button class="ib wide" id="drclose">Close</button></div>
    ${ib.stopped ? `<div class="gap stopgap" data-key="stopped" role="status"><b>Run stopped</b> by ${esc(ib.stopped.by || 'not recorded')} at ${esc(String(ib.stopped.at).replace('T', ' ').replace('Z', ' UTC'))}${ib.stopped.reason ? `: ${esc(ib.stopped.reason)}` : ''}. Its open decisions are shown greyed and cannot be taken while it is stopped.</div>` : ''}
    ${ib.missingText ? `<div class="gap" data-key="missing" role="alert"><b>Driver steps refused.</b> ${esc(ib.missingText)}</div>` : ''}
    ${ib.write ? '' : `<div class="gap" data-key="ro">This store is read-only here. Start the server with <code>--write</code> or <code>--write-only STORE</code> to approve, answer and change from here.</div>`}
    <div class="dr-list" data-key="drl">${items.length ? items.map((i) => card(i, S)).join('') : `<div class="empty-d"><div class="big">✓</div><p>${auto ? 'No decision is pending. Autopilot: the decider or a rule takes every decision of this run.' : 'No decision is waiting. The run continues on its own until it needs you.'}</p></div>`}
      <section class="dr-hist" data-key="drhist"><div class="sect">Decided <span class="muted">${(ib.history || []).length}, newest first</span></div>${(ib.history || []).length ? decisionSummary(ib.history) + `<div class="steplist">${decisionRows(ib.history.slice(0, 80), 'dh')}</div>` : '<p class="muted">Nothing decided yet.</p>'}</section></div>
    <div class="dr-f" data-key="drf"><div class="dr-fa"><button class="btn${S.dirty ? ' primary' : ''}" data-act="continue" ${ib.write && !proc?.running ? '' : 'disabled'}>${proc?.running ? 'Run in progress…' : 'Continue run'}</button><span class="muted">${proc ? `${esc(proc.name)} ${proc.running ? 'running' : `exited ${esc(proc.exit)}`}` : 'starts scripts/run-trial.sh in the background'}</span></div>
      ${proc?.tail?.length ? `<pre class="proc" id="proclog">${esc(proc.tail.slice(-200).join('\n'))}</pre>` : ''}</div>`;
}
