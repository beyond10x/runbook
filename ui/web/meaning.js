// What a change means, not only which lines moved: planning (AEP artifacts), specification (ESS
// sources), direction and intake artifacts, decisions, code. Pure functions, used by the server
// (to classify each fs change once) and by the page; no dependencies.

import {fileKindOf, artifactClassOf} from './view.js';

// ---- a small YAML reader -------------------------------------------------------------------------
// Block maps and sequences, flow [..] and {..}, quoted and plain scalars, | and > block scalars,
// comments. Enough for the run's YAML and AEP front matter; anything else throws, and the caller
// falls back to the text diff.
export function parseYaml(text) {
  const lines = String(text ?? '').replace(/\r/g, '').split('\n').map((raw) => ({raw, ind: raw.length - raw.trimStart().length, t: stripComment(raw).trim()}));
  let i = 0;
  while (i < lines.length && (!lines[i].t || lines[i].t === '---')) i++;
  const P = {lines, i};
  const nextReal = () => { while (P.i < lines.length && !lines[P.i].t) P.i++; return lines[P.i]; };
  function block(minInd) {
    const l = nextReal();
    if (!l || l.ind < minInd) return null;
    if (l.t === '---' || l.t === '...') return null;
    if (l.t === '-' || l.t.startsWith('- ')) return seq(l.ind);
    if (keyOf(l.t)) return map(l.ind);
    P.i++;
    return value(l.t, l.ind - 1);
  }
  function seq(ind) {
    const out = [];
    for (;;) {
      const l = nextReal();
      if (!l || l.ind !== ind || !(l.t === '-' || l.t.startsWith('- '))) break;
      const rest = l.t === '-' ? '' : l.t.slice(2);
      const pad = rest.length - rest.trimStart().length;
      const content = rest.trim();
      if (!content) { P.i++; out.push(block(ind + 1)); continue; }
      if (content.startsWith('- ') || content === '-' || (keyOf(content) && !/^["'[{]/.test(content))) {
        // the item's content starts on the dash line: read it as a block indented to its column
        const col = ind + 2 + pad;
        lines[P.i] = {raw: ' '.repeat(col) + content, ind: col, t: content};
        out.push(content.startsWith('-') ? seq(col) : map(col));
        continue;
      }
      if (keyOf(content)) {
        const col = ind + 2 + pad;
        lines[P.i] = {raw: ' '.repeat(col) + content, ind: col, t: content};
        out.push(map(col));
        continue;
      }
      P.i++;
      out.push(value(content, ind));
    }
    return out;
  }
  function map(ind) {
    const out = {};
    for (;;) {
      const l = nextReal();
      if (!l || l.ind !== ind || l.t.startsWith('- ') || l.t === '-') break;
      const k = keyOf(l.t);
      if (!k) throw new Error(`line ${P.i + 1}: expected a key`);
      P.i++;
      if (k.rest === '') {
        const n = nextReal();
        if (n && n.ind > ind) out[k.key] = block(ind + 1);
        else if (n && n.ind === ind && (n.t.startsWith('- ') || n.t === '-')) out[k.key] = seq(ind);
        else out[k.key] = null;
      } else out[k.key] = value(k.rest, ind);
    }
    return out;
  }
  function value(s, ind) {
    if (/^[|>][-+]?\d*$/.test(s)) {
      const fold = s[0] === '>', chomp = s.includes('-') ? '-' : s.includes('+') ? '+' : '';
      const body = [];
      let bi = null;
      while (P.i < lines.length) {
        const l = lines[P.i];
        if (l.raw.trim() && l.ind <= ind) break;
        if (l.raw.trim() && bi == null) bi = l.ind;
        body.push(l.raw.trim() ? l.raw.slice(bi) : '');
        P.i++;
      }
      while (chomp !== '+' && body.length && body[body.length - 1] === '') body.pop();
      let out = fold ? body.reduce((acc, x, j) => (j === 0 ? x : x === '' ? acc + '\n' : acc.endsWith('\n') ? acc + x : acc + ' ' + x), '') : body.join('\n');
      if (chomp !== '-') out += '\n';
      return out;
    }
    if (s[0] === '[' || s[0] === '{') {
      let src = s;
      while (!balanced(src) && P.i < lines.length) { src += ' ' + lines[P.i].t; P.i++; }
      const f = flow(src, 0);
      return f.v;
    }
    if (s[0] === '"' || s[0] === "'") {
      let src = s;
      while (!closedQuote(src) && P.i < lines.length) { src += ' ' + lines[P.i].raw.trim(); P.i++; }
      return quoted(src, 0).v;
    }
    let src = s;
    while (P.i < lines.length) {
      const l = lines[P.i];
      if (!l.t || l.ind <= ind || keyOf(l.t) || l.t.startsWith('- ')) break;
      src += ' ' + l.t;
      P.i++;
    }
    return scalar(src);
  }
  const v = block(0);
  if (nextReal() && P.lines[P.i].t !== '---') throw new Error(`line ${P.i + 1}: not read`);
  return v;
}
function stripComment(s) {
  let q = null;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) {
      if (c === '\\' && q === '"') { i++; continue; }
      if (c === q) { if (q === "'" && s[i + 1] === "'") { i++; continue; } q = null; }
      continue;
    }
    if ((c === '"' || c === "'") && (i === 0 || /[\s[{,:-]/.test(s[i - 1]))) { q = c; continue; }
    if (c === '#' && (i === 0 || /\s/.test(s[i - 1]))) return s.slice(0, i);
  }
  return s;
}
function keyOf(t) {
  let m = /^"((?:[^"\\]|\\.)*)"\s*:(?:\s+(.*)|$)/.exec(t) || /^'((?:[^']|'')*)'\s*:(?:\s+(.*)|$)/.exec(t);
  if (m) return {key: m[1], rest: (m[2] || '').trim()};
  if (/^[[{"'|>]/.test(t) || t.startsWith('- ')) return null;
  m = /^([^\s:][^:]*?|[^\s:]*):(?:\s+(.*)|$)/.exec(t);
  if (!m || /[{}[\],]/.test(m[1])) return null;
  return {key: m[1].trim(), rest: (m[2] || '').trim()};
}
function balanced(s) {
  let d = 0, q = null;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) { if (c === '\\' && q === '"') i++; else if (c === q) q = null; continue; }
    if (c === '"' || c === "'") q = c;
    else if (c === '[' || c === '{') d++;
    else if (c === ']' || c === '}') d--;
  }
  return d <= 0;
}
function closedQuote(s) { try { quoted(s, 0); return true; } catch { return false; } }
function quoted(s, i) {
  const q = s[i];
  let out = '';
  for (let j = i + 1; j < s.length; j++) {
    const c = s[j];
    if (q === "'" && c === "'") { if (s[j + 1] === "'") { out += "'"; j++; continue; } return {v: out, i: j + 1}; }
    if (q === '"' && c === '\\') { const n = s[++j]; out += {n: '\n', t: '\t', '"': '"', '\\': '\\', '/': '/', r: '\r', 0: '\0'}[n] ?? n; continue; }
    if (q === '"' && c === '"') return {v: out, i: j + 1};
    out += c;
  }
  throw new Error('unterminated quote');
}
function flow(s, i) {
  const ws = () => { while (i < s.length && /\s/.test(s[i])) i++; };
  ws();
  if (s[i] === '[') {
    const out = [];
    i++;
    for (;;) {
      ws();
      if (s[i] === ']') return {v: out, i: i + 1};
      const r = flow(s, i);
      out.push(r.v);
      i = r.i;
      ws();
      if (s[i] === ',') { i++; continue; }
      if (s[i] === ']') return {v: out, i: i + 1};
      throw new Error('bad flow sequence');
    }
  }
  if (s[i] === '{') {
    const out = {};
    i++;
    for (;;) {
      ws();
      if (s[i] === '}') return {v: out, i: i + 1};
      let k;
      if (s[i] === '"' || s[i] === "'") { const r = quoted(s, i); k = r.v; i = r.i; } else { const m = /^[^:,}]+/.exec(s.slice(i)); if (!m) throw new Error('bad flow key'); k = m[0].trim(); i += m[0].length; }
      ws();
      if (s[i] !== ':') throw new Error('bad flow map');
      i++;
      const r = flow(s, i);
      out[k] = r.v;
      i = r.i;
      ws();
      if (s[i] === ',') { i++; continue; }
      if (s[i] === '}') return {v: out, i: i + 1};
      throw new Error('bad flow map');
    }
  }
  if (s[i] === '"' || s[i] === "'") return quoted(s, i);
  const m = /^[^,\]}]*/.exec(s.slice(i));
  return {v: scalar(m[0].trim()), i: i + m[0].length};
}
function scalar(s) {
  if (s === '' || s === '~' || s === 'null') return null;
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (/^-?\d+$/.test(s) && s.length < 16) return Number(s);
  if (/^-?\d+\.\d+$/.test(s)) return Number(s);
  return s;
}

// AEP planning markdown: YAML front matter between --- lines, then the body.
export function frontMatter(text) {
  const m = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/.exec(String(text ?? '').replace(/\r/g, ''));
  if (!m) return {data: null, body: String(text ?? '')};
  return {data: parseYaml(m[1]) || {}, body: m[2]};
}
// ## sections of a markdown body: {heading: text}
export function sections(body) {
  const out = {};
  let cur = '(top)';
  for (const l of String(body || '').split('\n')) {
    const m = /^#{2,3}\s+(.*)$/.exec(l);
    if (m) { cur = m[1].trim(); out[cur] = ''; continue; }
    out[cur] = (out[cur] || '') + l + '\n';
  }
  for (const k of Object.keys(out)) { out[k] = out[k].trim(); if (k === '(top)' && !out[k]) delete out[k]; }
  return out;
}

// ---- classes -------------------------------------------------------------------------------------
export const CLASSES = ['planning', 'spec', 'direction', 'decision', 'code'];
export const CLASS_LABEL = {planning: 'Planning', spec: 'Specification', direction: 'Direction', decision: 'Decision', code: 'Code'};
const base = (p) => String(p || '').split('/').pop();

// What a path is (class and kind) is the runbook's view (web/view.js, file_kinds.rules: the first rule that
// matches). A path no rule matches, and every path without a view, is code: its label is its path.
export const classify = (p) => fileKindOf(p) || {cls: 'code', kind: 'code'};

// ---- helpers for sentences -------------------------------------------------------------------------
const short = (n) => String(n ?? '').split('.').pop();
const clip = (s, n = 80) => { const t = String(s ?? '').replace(/\s+/g, ' ').trim(); return t.length > n ? t.slice(0, n - 1) + '…' : t; };
const q = (s, n) => `“${clip(s, n)}”`;
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const asArr = (x) => (Array.isArray(x) ? x : x == null ? [] : [x]);
function byKey(list, key = 'name') {
  const m = new Map();
  for (const x of asArr(list)) {
    if (x && typeof x === 'object') { const k = x[key] ?? x.id ?? x.name; if (k != null) m.set(String(k), x); }
    else if (x != null) m.set(String(x), x);
  }
  return m;
}
// added, removed and changed members of two keyed lists
function keyed(a, b, key) {
  const A = byKey(a, key), B = byKey(b, key);
  return {added: [...B.keys()].filter((k) => !A.has(k)).map((k) => [k, B.get(k)]), removed: [...A.keys()].filter((k) => !B.has(k)).map((k) => [k, A.get(k)]),
    changed: [...B.keys()].filter((k) => A.has(k) && !same(A.get(k), B.get(k))).map((k) => [k, A.get(k), B.get(k)])};
}
const it = (text, kind = 'chg') => ({text, kind});

// ---- planning: AEP artifacts -----------------------------------------------------------------------
const REL_WORD = (k) => String(k).replace(/_/g, ' ').replace(/^depends on$/, 'depends on');
function relSet(rel) {
  const out = [];
  for (const r of asArr(rel)) {
    if (r && typeof r === 'object') for (const [k, v] of Object.entries(r)) for (const t of asArr(v)) out.push(`${REL_WORD(k)} ${t}`);
    else if (r != null) out.push(String(r));
  }
  return out;
}
function scopeSet(scope) { return asArr(scope).map((s) => (s && typeof s === 'object' ? s.path || s.glob || JSON.stringify(s) : String(s))); }
function setDiff(a, b) { return {added: b.filter((x) => !a.includes(x)), removed: a.filter((x) => !b.includes(x))}; }

function planning(p, before, after, op) {
  const A = before != null ? frontMatter(before) : null, B = after != null ? frontMatter(after) : null;
  const d = (B || A).data || {};
  if ((A && !A.data) || (B && !B.data)) throw new Error('no front matter');
  const kind = d.kind || p.split('/').slice(-2, -1)[0] || 'artifact';
  const name = String(d.id || base(p).replace(/\.md$/, '')).replace(/^[\w-]+:/, '');
  const subject = `${kind} ${name}`;
  if (!A) return {subject, title: d.title, items: [it(`${subject} created${d.title ? `: ${q(d.title, 70)}` : ''}${d.status ? ` (${d.status})` : ''}`, 'add'),
    ...relSet(d.relations).map((r) => it(`edge: ${r}`, 'add')), ...scopeSet(d.scope).map((s) => it(`scope: ${s}`, 'add'))]};
  if (!B) return {subject, title: A.data.title, items: [it(`${subject} removed`, 'del')]};
  const a = A.data, b = B.data, items = [];
  if (a.status !== b.status) items.push(it(`${subject}: ${a.status ?? '—'} → ${b.status ?? '—'}`));
  if (a.title !== b.title) items.push(it(`${subject}: title ${q(a.title, 40)} → ${q(b.title, 40)}`));
  const rel = setDiff(relSet(a.relations), relSet(b.relations));
  for (const r of rel.added) items.push(it(`${subject}: edge added: ${r}`, 'add'));
  for (const r of rel.removed) items.push(it(`${subject}: edge removed: ${r}`, 'del'));
  const sc = setDiff(scopeSet(a.scope), scopeSet(b.scope));
  for (const s of sc.added) items.push(it(`${subject}: scope added: ${s}`, 'add'));
  for (const s of sc.removed) items.push(it(`${subject}: scope removed: ${s}`, 'del'));
  if (!same(a.acceptance, b.acceptance)) items.push(it(`${subject}: acceptance changed`));
  if (a.summary !== b.summary) items.push(it(`${subject}: summary changed`));
  for (const k of Object.keys({...a, ...b})) {
    if (['status', 'title', 'relations', 'scope', 'acceptance', 'summary', 'revision', 'transitions', 'format', 'id', 'kind'].includes(k)) continue;
    if (!same(a[k], b[k])) items.push(it(`${subject}: ${k} changed`));
  }
  const sa = sections(A.body), sb = sections(B.body);
  for (const k of Object.keys({...sa, ...sb})) {
    if (sa[k] === sb[k]) continue;
    const label = /accept/i.test(k) ? 'acceptance' : `section ${q(k, 40)}`;
    items.push(it(`${subject}: ${label} ${sa[k] == null ? 'added' : sb[k] == null ? 'removed' : 'changed'}`, sa[k] == null ? 'add' : sb[k] == null ? 'del' : 'chg'));
  }
  if (!items.length) items.push(it(`${subject} revised${b.revision != null ? ` (revision ${b.revision})` : ''}`));
  return {subject, title: b.title, items};
}

// ---- specification: ESS sources -----------------------------------------------------------------
const SPEC_LISTS = [['types', 'type'], ['entities', 'entity'], ['commands', 'command'], ['events', 'event'], ['views', 'view'], ['errors', 'error'], ['queries', 'query'], ['policies', 'policy']];
function fieldsDiff(label, a, b, word, out) {
  const f = keyed(a, b, 'name');
  for (const [k] of f.added) out.push(it(`${label}: ${word} \`${k}\` added`, 'add'));
  for (const [k] of f.removed) out.push(it(`${label}: ${word} \`${k}\` removed`, 'del'));
  for (const [k, x, y] of f.changed) {
    if (x && y && typeof x === 'object' && x.type !== y.type && y.type != null) out.push(it(`${label}: ${word} \`${k}\` type ${x.type ?? '—'} → ${y.type}`));
    else out.push(it(`${label}: ${word} \`${k}\` changed`));
  }
}
function transKey(t) { return t && typeof t === 'object' ? `${t.from ?? '*'} → ${t.to ?? '?'}${t.on || t.command ? ` on ${short(t.on || t.command)}` : ''}` : String(t); }
function specMember(word, name, a, b, out) {
  const label = `${word} ${short(name)}`;
  const keys = new Set([...Object.keys(a || {}), ...Object.keys(b || {})]);
  for (const k of keys) {
    const x = a?.[k], y = b?.[k];
    if (same(x, y) || k === 'name') continue;
    if (k === 'fields' || k === 'input' || k === 'output') fieldsDiff(label, x, y, k === 'fields' ? 'field' : k, out);
    else if (k === 'outcomes') {
      const o = keyed(x, y, 'name');
      for (const [n] of o.added) out.push(it(`${label}: outcome \`${n}\` added`, 'add'));
      for (const [n] of o.removed) out.push(it(`${label}: outcome \`${n}\` removed`, 'del'));
      for (const [n] of o.changed) out.push(it(`${label}: outcome \`${n}\` changed`));
    } else if (k === 'lifecycle') {
      const la = x || {}, lb = y || {};
      const st = setDiff(asArr(la.states).map(String), asArr(lb.states).map(String));
      for (const s of st.added) out.push(it(`${label}: state \`${s}\` added`, 'add'));
      for (const s of st.removed) out.push(it(`${label}: state \`${s}\` removed`, 'del'));
      const tr = setDiff(asArr(la.transitions).map(transKey), asArr(lb.transitions).map(transKey));
      for (const s of tr.added) out.push(it(`${label}: transition ${s} added`, 'add'));
      for (const s of tr.removed) out.push(it(`${label}: transition ${s} removed`, 'del'));
      if (la.initial !== lb.initial) out.push(it(`${label}: initial state ${la.initial ?? '—'} → ${lb.initial ?? '—'}`));
      if (!same(la.terminal, lb.terminal)) out.push(it(`${label}: terminal states ${asArr(la.terminal).join(', ') || '—'} → ${asArr(lb.terminal).join(', ') || '—'}`));
    } else if (x == null) out.push(it(`${label}: ${k} added`, 'add'));
    else if (y == null) out.push(it(`${label}: ${k} removed`, 'del'));
    else if (typeof y !== 'object' && String(y).length < 40 && String(x).length < 40) out.push(it(`${label}: ${k} ${x} → ${y}`));
    else out.push(it(`${label}: ${k} changed`));
  }
}
function spec(p, before, after) {
  const a = before != null ? parseYaml(before) : null, b = after != null ? parseYaml(after) : null;
  if ((before != null && (a == null || typeof a !== 'object')) || (after != null && (b == null || typeof b !== 'object'))) throw new Error('not a YAML map');
  const d = b || a;
  const what = d.domain ? `domain ${d.domain}` : d.system ? `system ${d.system}` : d.scenario || d.name ? `scenario ${d.scenario || d.name}` : base(p).replace(/\.ya?ml$/, '');
  if (!a || !b) {
    const counts = SPEC_LISTS.map(([k, w]) => (asArr(d[k]).length ? `${asArr(d[k]).length} ${w === 'entity' ? (asArr(d[k]).length > 1 ? 'entities' : 'entity') : w + (asArr(d[k]).length > 1 ? 's' : '')}` : null)).filter(Boolean);
    return {subject: what, items: [it(`${what} ${a ? 'removed' : 'created'}${!a && counts.length ? `: ${counts.join(', ')}` : ''}`, a ? 'del' : 'add'),
      ...(!a ? SPEC_LISTS.flatMap(([k, w]) => asArr(d[k]).map((x) => it(`${w} ${short(x?.name ?? x)} added`, 'add'))) : [])]};
  }
  const items = [];
  for (const [k, w] of SPEC_LISTS) {
    if (same(a[k], b[k])) continue;
    const m = keyed(a[k], b[k], 'name');
    for (const [n] of m.added) items.push(it(`${w} ${short(n)} added`, 'add'));
    for (const [n] of m.removed) items.push(it(`${w} ${short(n)} removed`, 'del'));
    for (const [n, x, y] of m.changed) specMember(w, n, x, y, items);
  }
  if (!same(a.domains, b.domains)) {
    const s = setDiff(asArr(a.domains).map(String), asArr(b.domains).map(String));
    for (const x of s.added) items.push(it(`domain ${x} added to the system`, 'add'));
    for (const x of s.removed) items.push(it(`domain ${x} removed from the system`, 'del'));
  }
  if (!same(a.components, b.components)) {
    const m = keyed(a.components, b.components, 'component');
    for (const [n] of m.added) items.push(it(`component ${n} added`, 'add'));
    for (const [n] of m.removed) items.push(it(`component ${n} removed`, 'del'));
    for (const [n, x, y] of m.changed) for (const k of Object.keys({...x, ...y})) if (!same(x[k], y[k])) items.push(it(`component ${n}: ${k} changed`));
  }
  const done = new Set([...SPEC_LISTS.map(([k]) => k), 'domains', 'components']);
  for (const k of Object.keys({...a, ...b})) {
    if (done.has(k) || same(a[k], b[k])) continue;
    const x = a[k], y = b[k];
    items.push(it(typeof y !== 'object' && y != null && x != null && String(x).length < 30 && String(y).length < 30 ? `${what}: ${k} ${x} → ${y}` : `${what}: ${k} ${x == null ? 'added' : y == null ? 'removed' : 'changed'}`));
  }
  if (!items.length) items.push(it(`${what}: formatting only`));
  return {subject: what, items};
}

// ---- direction and intake --------------------------------------------------------------------------
function idList(p, before, after, key, word, describe) {
  const a = before != null ? parseYaml(before) || {} : {}, b = after != null ? parseYaml(after) || {} : {};
  const la = Array.isArray(a) ? a : a[key], lb = Array.isArray(b) ? b : b[key];
  const m = keyed(la, lb, 'id');
  const items = [];
  for (const [k, x] of m.added) items.push(it(describe.added(k, x), 'add'));
  for (const [k] of m.removed) items.push(it(`${word} ${k} removed`, 'del'));
  for (const [k, x, y] of m.changed) items.push(...describe.changed(k, x, y).map((t) => it(t)));
  return items;
}
function direction(p, kind, before, after) {
  if (kind === 'requirements') return {subject: 'requirements', items: idList(p, before, after, 'requirements', 'requirement', {
    added: (k, x) => `requirement ${k} added: ${q(x.text, 90)}`,
    changed: (k, x, y) => [x.text !== y.text ? `requirement ${k} reworded: ${q(y.text, 80)}` : null, ...Object.keys({...x, ...y}).filter((f) => f !== 'text' && !same(x[f], y[f])).map((f) => `requirement ${k}: ${f.replace(/_/g, ' ')} changed`)].filter(Boolean)})};
  if (kind === 'questions') return {subject: 'questions', items: idList(p, before, after, 'questions', 'question', {
    added: (k, x) => (x.answer != null ? `question ${k} answered: ${q(x.answer, 60)}${x.answered_by ? ` (by ${x.answered_by})` : ''}` : `question ${k} asked: ${q(x.question, 90)}`),
    changed: (k, x, y) => {
      if (x.answer == null && y.answer != null) return [`question ${k} answered: ${q(y.answer, 70)}${y.answered_by ? ` (by ${y.answered_by})` : ''}`];
      const out = [];
      if (x.answer !== y.answer) out.push(`question ${k}: answer changed to ${q(y.answer, 60)}`);
      if (x.status !== y.status) out.push(`question ${k}: ${x.status ?? '—'} → ${y.status ?? '—'}${y.status === 'accepted' && y.answer != null ? ` (${q(y.answer, 50)})` : ''}`);
      if (x.question !== y.question) out.push(`question ${k} reworded`);
      return out.length ? out : [`question ${k} changed`];
    }})};
  if (kind === 'findings') return {subject: 'findings', items: idList(p, before, after, 'findings', 'finding', {
    added: (k, x) => `finding ${k} raised${x.raised_by ? ` by ${x.raised_by}` : ''}: ${q(x.text || x.summary || x.finding, 70)}`,
    changed: (k, x, y) => [x.status !== y.status ? `finding ${k}: ${x.status ?? '—'} → ${y.status ?? '—'}${y.decided_by ? ` (by ${y.decided_by})` : ''}` : `finding ${k} changed`]})};
  if (kind === 'dissection') return {subject: 'dissection', items: idList(p, before, after, 'items', 'item', {
    added: (k, x) => `dissection ${k} (${x.type || 'item'}): ${q(x.note || x.span, 70)}`,
    changed: (k) => [`dissection ${k} changed`]})};
  if (kind === 'context') {
    const a = before != null ? parseYaml(before) || {} : {}, b = after != null ? parseYaml(after) || {} : {};
    const items = [];
    const val = (x) => (x && typeof x === 'object' && 'value' in x ? x.value : x);
    for (const k of Object.keys({...a, ...b})) {
      const x = a[k], y = b[k];
      if (same(x, y)) continue;
      if (x === undefined) items.push(it(`context ${k} set: ${q(val(y), 60)}${y?.status ? ` (${y.status})` : ''}`, 'add'));
      else if (y === undefined) items.push(it(`context ${k} removed`, 'del'));
      else if (!same(val(x), val(y))) items.push(it(`context ${k}: ${q(val(x), 36)} → ${q(val(y), 36)}${y?.status ? ` (${y.status})` : ''}`));
      else if (x?.status !== y?.status) items.push(it(`context ${k}: ${x?.status ?? '—'} → ${y?.status ?? '—'}`));
      else items.push(it(`context ${k}: reason changed`));
    }
    return {subject: 'context', items};
  }
  if (kind === 'roadmap' || kind === 'vision') {
    const A = before != null ? frontMatter(before) : null, B = after != null ? frontMatter(after) : null;
    const epics = (x) => {
      if (!x) return [];
      const s = sections(x.body);
      const fromBody = Object.keys(s).filter((h) => /^[\w.-]+\s*:/.test(h)).map((h) => h.split(':')[0].trim());
      if (fromBody.length) return fromBody;
      const m = /epics?:\s*(.*)$/i.exec(x.data?.summary || '');
      return m ? m[1].split(/[,\s]+/).filter(Boolean) : [];
    };
    const title = (B || A).data?.title || kind;
    if (!A) return {subject: kind, items: [it(kind === 'roadmap' ? `roadmap written: epics ${epics(B).join(', ') || '—'}` : `vision written: ${q(title, 70)}`, 'add')]};
    if (!B) return {subject: kind, items: [it(`${kind} removed`, 'del')]};
    const items = [];
    if (kind === 'roadmap') {
      const ea = epics(A), eb = epics(B), s = setDiff(ea, eb);
      for (const x of s.added) items.push(it(`roadmap: epic ${x} added`, 'add'));
      for (const x of s.removed) items.push(it(`roadmap: epic ${x} removed`, 'del'));
      if (!s.added.length && !s.removed.length && !same(ea, eb)) items.push(it(`roadmap: epic order changed (${ea.join(', ')} → ${eb.join(', ')})`));
    }
    if (A.data?.status !== B.data?.status) items.push(it(`${kind}: ${A.data?.status ?? '—'} → ${B.data?.status ?? '—'}`));
    const sa = sections(A.body), sb = sections(B.body);
    for (const k of Object.keys({...sa, ...sb})) if (sa[k] !== sb[k] && !(kind === 'roadmap' && /^[\w.-]+\s*:/.test(k))) items.push(it(`${kind}: section ${q(k, 40)} ${sa[k] == null ? 'added' : sb[k] == null ? 'removed' : 'changed'}`, sa[k] == null ? 'add' : sb[k] == null ? 'del' : 'chg'));
    if (!items.length) items.push(it(`${kind} revised`));
    return {subject: kind, items};
  }
  // idea and anything else in this class: top-level keys
  const a = before != null ? parseYaml(before) || {} : {}, b = after != null ? parseYaml(after) || {} : {};
  if (typeof a !== 'object' || typeof b !== 'object') throw new Error('not a YAML map');
  const items = Object.keys({...a, ...b}).filter((k) => !same(a[k], b[k])).map((k) => it(`${kind}: ${k} ${a[k] === undefined ? 'set' : b[k] === undefined ? 'removed' : 'changed'}${typeof b[k] === 'string' ? `: ${q(b[k], 60)}` : ''}`, a[k] === undefined ? 'add' : b[k] === undefined ? 'del' : 'chg'));
  return {subject: kind, items: items.length ? items : [it(`${kind} revised`)]};
}

// ---- decisions written down: ADR-like documents ---------------------------------------------------
function adr(p, before, after) {
  const doc = (t) => { if (t == null) return null; const f = frontMatter(t); const h = /^#\s+(.*)$/m.exec(f.body); const st = f.data?.status || (/^##\s*Status\s*\n+\s*(\w+)/mi.exec(f.body) || [])[1] || null; return {title: f.data?.title || (h ? h[1].trim() : base(p)), status: st, body: f.body}; };
  const a = doc(before), b = doc(after);
  const t = (b || a).title;
  if (!a) return {subject: `decision ${t}`, items: [it(`decision recorded: ${q(t, 80)}${b.status ? ` (${b.status})` : ''}`, 'add')]};
  if (!b) return {subject: `decision ${t}`, items: [it(`decision withdrawn: ${q(t, 80)}`, 'del')]};
  if (a.status !== b.status) return {subject: `decision ${t}`, items: [it(`decision ${q(t, 60)}: ${a.status ?? '—'} → ${b.status ?? '—'}`)]};
  return {subject: `decision ${t}`, items: [it(`decision ${q(t, 70)} revised`)]};
}

// The meaning of one fs change record (docs/fs-changes.md), given its before and after contents
// (null when absent). Never throws: when the file cannot be parsed the class stays and the
// sentence says so; the text diff is one click away either way.
export function meaningOf(change, before, after) {
  const p = change.path || '';
  const {cls, kind} = classify(p);
  const op = change.op || (before == null ? 'create' : after == null ? 'delete' : 'modify');
  if (cls === 'code') return {cls, kind, parsed: false, items: [it(`${op === 'create' ? 'created' : op === 'delete' ? 'deleted' : op === 'rename' ? 'moved' : 'changed'} ${p}`, op === 'create' ? 'add' : op === 'delete' ? 'del' : 'chg')]};
  if (change.binary || change.too_large || (before == null && after == null && op !== 'rename')) return {cls, kind, parsed: false, items: [it(`${base(p)} ${{create: 'created', delete: 'deleted', rename: 'moved'}[op] || 'changed'}: content not recorded (${change.binary ? 'binary' : change.too_large ? 'over 1 MiB' : 'no blob'})`)]};
  try {
    const r = cls === 'planning' ? planning(p, before, after, op) : cls === 'spec' ? spec(p, before, after) : cls === 'decision' ? adr(p, before, after) : direction(p, kind, before, after);
    if (!r.items.length) r.items.push(it(`${base(p)}: no change in meaning`));
    return {cls, kind, parsed: true, subject: r.subject, title: r.title || null, items: r.items};
  } catch (e) {
    return {cls, kind, parsed: false, error: String(e.message || e), items: [it(`${base(p)} ${op === 'create' ? 'created' : op === 'delete' ? 'deleted' : 'changed'} (not parsed: ${clip(e.message || e, 60)})`)]};
  }
}

// ---- without fs/changes.jsonl: what the case store still says ------------------------------------
// Artifact names of the runbook (file_kinds.artifacts of its view) and their class.
export const artifactClass = (name) => artifactClassOf(name) || 'code';
// Items from the event log (revisions by action) and from the final AEP front matter (transitions
// with their time and actor). The content of each revision is not recorded there.
export function storeItems(cases, files = []) {
  const out = [];
  for (const c of cases) {
    const revs = {};
    for (const e of c.events || []) {
      if (e.event !== 'action_finished' || !e.revises || !Object.keys(e.revises).length) continue;
      for (const [a, r] of Object.entries(e.revises)) {
        const first = !revs[a];
        revs[a] = r;
        out.push({id: `rev:${c.rel}:${e.seq}:${a}`, cls: artifactClass(a), text: `${a} ${first ? 'written' : 'revised'}`, more: [], note: 'what changed inside is not recorded: this run has no fs/changes.jsonl',
          case: c.id, rel: c.rel, action: e.action, principal: e.principal || null, seq: e.seq, t: c.times?.[e.seq] ?? Date.parse(e.at), revision: r});
      }
    }
  }
  for (const f of files) {
    let fm;
    try { fm = frontMatter(f.text); } catch { continue; }
    const d = fm.data;
    if (!d) continue;
    const {cls} = classify(f.path);
    const name = `${d.kind || 'artifact'} ${String(d.id || base(f.path)).replace(/^[\w-]+:/, '')}`;
    for (const t of asArr(d.transitions)) {
      if (!t || typeof t !== 'object') continue;
      out.push({id: `tr:${f.path}:${t.at}:${t.to}`, cls, text: `${name}: ${t.from ?? '—'} → ${t.to ?? '—'}`, more: d.title ? [d.title] : [], note: `from the transitions in ${f.path}`,
        case: null, action: null, principal: t.actor || null, t: Date.parse(t.at), path: f.path});
    }
  }
  return out;
}
