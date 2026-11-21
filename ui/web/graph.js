// The protocol of one case as a layered graph, coloured with one stored canon-decision/1.
// Layout: docs-system's deterministic layered layout (vendor/graph-layout.js, copied from
// docs-system/dist/graph-layout.js in the beyond10x workspace), the engine behind the Canon site's
// b10x-protocol-graph/1 pictures. Columns here add artifacts in front of actions. The layout is
// computed once per protocol structure; a new evaluation only changes classes and labels, which
// the morph applies in place (CSS transitions animate the colours).
import {layeredLayout} from './vendor/graph-layout.js';
import {actorOf, predicateRefs, ACTOR_LABEL, seqOfEvidence} from './model.js';
import {actorLabel} from './principal.js';
import {patch} from './morph.js';
import {fmtDur} from './now.js';

const COLS = ['Artifacts', 'Actions', 'Evidence kinds', 'Claims', 'Derived claims', 'Outcomes · obligations'];
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (ch) => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'}[ch]));

export function buildGraph(c) {
  const ir = c.ir;
  const nodes = [], edges = [];
  const has = new Set();
  const add = (id, kind, name, layer, extra = {}) => { nodes.push({id, kind, name, layer, ...extra}); has.add(id); return id; };
  for (const a of Object.keys(ir.artifacts || {})) add(`artifact:${a}`, 'artifact', a, 0);
  for (const a of Object.keys(ir.actions || {})) add(`action:${a}`, 'action', a, 1, {actor: actorOf(c, a)});
  for (const k of Object.keys(ir.evidence_kinds || {})) add(`evidence:${k}`, 'evidence', k, 2);
  const depth = {};
  const claimDepth = (id, seen = new Set()) => {
    if (depth[id] != null) return depth[id];
    if (seen.has(id)) return 0;
    seen.add(id);
    const refs = predicateRefs(ir.claims[id]?.true_when);
    depth[id] = refs.claims.length ? 1 + Math.max(...refs.claims.map((r) => claimDepth(r.id, seen))) : 0;
    return depth[id];
  };
  Object.keys(ir.claims || {}).forEach((id) => claimDepth(id));
  const derived = Object.values(depth).some((d) => d > 0);
  for (const id of Object.keys(ir.claims || {})) add(`claim:${id}`, 'claim', id, 3 + Math.min(depth[id], 1));
  const last = derived ? 5 : 4;
  for (const id of Object.keys(ir.outcomes || {})) add(`outcome:${id}`, 'outcome', id, last);
  for (const id of Object.keys(ir.obligations || {})) add(`obligation:${id}`, 'obligation', id, last);
  const edge = (from, to, kind) => { if (from !== to && !edges.some((e) => e.from === from && e.to === to && e.kind === kind)) edges.push({from, to, kind}); };
  const input = (id, kind, name) => (has.has(id) ? id : add(id, kind, name, 2));
  for (const [a, d] of Object.entries(ir.actions || {})) {
    for (const m of d.may_produce || []) if (m.evidence) edge(`action:${a}`, input(`evidence:${m.evidence}`, 'evidence', m.evidence), 'produces');
    if (d.case_open) edge(`action:${a}`, input('evidence:case_outcome', 'evidence', 'case_outcome'), 'produces');
    for (const r of predicateRefs(d.precondition).claims) edge(`claim:${r.id}`, `action:${a}`, 'reads');
  }
  for (const [id, d] of Object.entries(ir.claims || {})) {
    const refs = predicateRefs(d.true_when);
    for (const e of refs.evidence) {
      const ev = input(`evidence:${e.kind}`, 'evidence', e.kind);
      edge(ev, `claim:${id}`, 'establishes');
      if (has.has(`artifact:${e.subject}`)) edge(`artifact:${e.subject}`, ev, 'about');
    }
    for (const r of refs.claims) edge(`claim:${r.id}`, `claim:${id}`, 'supports');
    if (refs.children.length) edge(input('evidence:case_outcome', 'evidence', 'case_outcome'), `claim:${id}`, 'establishes');
  }
  for (const [id, d] of Object.entries(ir.outcomes || {})) {
    const refs = predicateRefs(d.requires);
    for (const r of refs.claims) edge(`claim:${r.id}`, `outcome:${id}`, 'requires');
    for (const dec of refs.decisions) edge(input(`decision:${dec}`, 'decision', dec), `outcome:${id}`, 'requires');
  }
  for (const [id, d] of Object.entries(ir.obligations || {})) for (const r of predicateRefs(d.discharged_when).claims) edge(`claim:${r.id}`, `obligation:${id}`, 'discharges');
  for (const inv of Object.values(ir.invalidation || {})) for (const cl of inv.invalidates || []) if (has.has(`artifact:${inv.upstream}`)) edge(`artifact:${inv.upstream}`, `claim:${cl}`, 'invalidates');
  return {nodes, edges, columns: derived ? COLS : COLS.filter((x) => x !== 'Derived claims')};
}

const W = {0: 136, 1: 200, 2: 184, 3: 188, 4: 178, 5: 176};
const H = 50;

export function statusOf(node, ev) {
  if (!ev) return null;
  if (node.kind === 'claim') return ev.claims?.[node.name]?.value || 'unknown';
  if (node.kind === 'action') return ev.actions?.[node.name]?.status || null;
  if (node.kind === 'outcome') return ev.outcomes?.[node.name]?.status || null;
  if (node.kind === 'obligation') return (ev.obligations || []).find((o) => o.id === node.name)?.status || null;
  return null;
}

const cache = new Map();
function layoutFor(c) {
  const key = `${c.rel}|${JSON.stringify(c.ir?.protocol)}|${Object.keys(c.ir?.actions || {}).length}|${Object.keys(c.bindings || {}).join(',')}`;
  if (cache.has(key)) return cache.get(key);
  const g = buildGraph(c);
  const layers = [...new Set(g.nodes.map((n) => n.layer))].sort((a, b) => a - b);
  const idx = new Map(layers.map((l, i) => [l, i]));
  const lay = layeredLayout(g.nodes.map((n) => ({id: n.id, layer: idx.get(n.layer), width: W[n.layer] || 190, height: H})),
    g.edges.map((e) => ({from: e.from, to: e.to, constraint: e.kind !== 'reads' && e.kind !== 'invalidates'})), {layerGap: 48, nodeGap: 14});
  const out = {g, lay, head: 40, width: lay.width + 32, height: lay.height + 40 + 24};
  cache.set(key, out);
  return out;
}

function shape(kind, w, h) {
  const r = 8;
  switch (kind) {
    case 'artifact': return `<path class="body" d="M0,6 Q0,0 6,0 H${w - 14} L${w},14 V${h - 6} Q${w},${h} ${w - 6},${h} H6 Q0,${h} 0,${h - 6} Z"/><path class="fold" d="M${w - 14},0 V10 Q${w - 14},14 ${w - 10},14 H${w}"/>`;
    case 'claim': return `<rect class="body" width="${w}" height="${h}" rx="${h / 2}"/>`;
    case 'outcome': case 'decision': { const k = 10; return `<path class="body" d="M${k},0 H${w - k} L${w},${k} V${h - k} L${w - k},${h} H${k} L0,${h - k} V${k} Z"/>`; }
    case 'obligation': return `<rect class="body" width="${w}" height="${h}" rx="4"/><rect class="flag" x="0" y="0" width="4" height="${h}" rx="2"/>`;
    case 'evidence': return `<rect class="sheet" x="5" y="5" width="${w}" height="${h}" rx="7"/><rect class="body" width="${w}" height="${h}" rx="7"/>`;
    default: return `<rect class="body" width="${w}" height="${h}" rx="${r}"/>`;
  }
}

export function renderGraph(host, c, ev, opts) {
  const {g, lay, head, width, height} = layoutFor(c);
  let svg = host.querySelector('svg.pgraph');
  if (!svg || svg.dataset.case !== c.rel) {
    host.innerHTML = `<svg class="pgraph" xmlns="http://www.w3.org/2000/svg"></svg>`;
    svg = host.querySelector('svg');
    svg.dataset.case = c.rel;
  }
  svg.setAttribute('viewBox', `-16 0 ${width} ${height}`);
  svg.setAttribute('width', Math.round(width * opts.zoom));
  svg.setAttribute('height', Math.round(height * opts.zoom));
  const sel = opts.selected;
  const near = new Set();
  if (sel) { near.add(sel); for (const e of g.edges) { if (e.from === sel) near.add(e.to); if (e.to === sel) near.add(e.from); } }
  const parts = [];
  parts.push(`<defs data-key="defs"><linearGradient id="gEdge" x1="0" x2="1"><stop offset="0" stop-color="currentColor" stop-opacity=".25"/><stop offset="1" stop-color="currentColor" stop-opacity=".8"/></linearGradient>${['produces', 'establishes', 'supports', 'requires', 'discharges', 'reads', 'about', 'invalidates'].map((k) => `<marker id="ah-${k}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path class="ah ah-${k}" d="M0,1 L9,5 L0,9 z"/></marker>`).join('')}</defs>`);
  lay.columns.forEach((col, i) => parts.push(`<g data-key="col${i}" class="col"><rect class="lane" x="${col.x - 10}" y="28" width="${col.width + 20}" height="${height - 36}" rx="12"/><text class="colhead" x="${col.x + 2}" y="20">${esc((g.columns[i] || '').toUpperCase())}</text></g>`));
  parts.push(`<g data-key="body" transform="translate(0,${head})"><g class="edges">`);
  for (const le of lay.edges) {
    const e = g.edges[le.index];
    const on = sel && (e.from === sel || e.to === sel);
    if (!opts.showReads && (e.kind === 'reads' || e.kind === 'invalidates') && !on) continue;
    parts.push(`<path data-key="e:${esc(e.from)}>${esc(e.to)}:${e.kind}" class="edge e-${e.kind}${on ? ' on' : ''}${sel && !on ? ' dim' : ''}" d="${le.path}" marker-end="url(#ah-${e.kind})"><title>${esc(`${e.from} ${e.kind} ${e.to}`)}</title></path>`);
  }
  parts.push(`</g><g class="nodes">`);
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  for (const p of lay.nodes) {
    const n = byId.get(p.id);
    const st = statusOf(n, ev);
    const w = p.width, h = p.height;
    let sub = '', count = 0;
    if (n.kind === 'claim') sub = (st || 'unknown').toUpperCase();
    else if (n.kind === 'action') { const req = c.ir.actions[n.name].requires || []; sub = `${st || '—'}${req.length ? ` · ${req.map((r) => r.capability).join(',')}` : ''}`; }
    else if (n.kind === 'evidence') { count = Object.values(c.evidence || {}).filter((r) => r.kind === n.name && (opts.atSeq == null || (seqOfEvidence(r.id) ?? 0) <= opts.atSeq)).length; sub = `${count} record${count === 1 ? '' : 's'}`; }
    else if (n.kind === 'artifact') { const r = (opts.revisions || {})[n.name]?.revision; sub = r && r !== 'none' ? r : 'not written'; }
    else if (n.kind === 'decision') sub = 'explicit decision';
    else sub = st || '';
    const frontier = n.kind === 'action' && (st === 'admissible' || st === 'approval-required');
    const lv = n.kind === 'action' && opts.atSeq == null ? opts.live?.get(n.name) : null;
    const cls = ['node', lv ? `live lv-${lv.kind} lv-${lv.verb}` : '', `k-${n.kind}`, st ? `s-${st}` : '', n.actor ? `a-${n.actor.kind}` : '', frontier ? 'frontier' : '', n.kind === 'evidence' && count ? 'has' : '', p.id === sel ? 'sel' : '', sel && !near.has(p.id) ? 'dim' : ''].filter(Boolean).join(' ');
    const tx = n.kind === 'claim' ? 26 : n.kind === 'action' ? 14 : 12;
    const fit = (s, px, cw) => { const m = Math.floor(px / cw); return s.length > m ? s.slice(0, Math.max(1, m - 1)) + '…' : s; };
    const chipW = n.actor ? actorLabel(n.actor.kind, ACTOR_LABEL).length * 6.2 + 12 : 0;
    const label = fit(n.name, w - tx - 10, 7.4);
    sub = fit(sub, w - tx - (chipW ? chipW + 14 : 10), 6.1);
    let inner = (lv ? `<rect class="livering" x="-5" y="-5" width="${w + 10}" height="${h + 10}" rx="13"/>` : '') + (frontier && !lv ? `<rect class="glow" x="-3" y="-3" width="${w + 6}" height="${h + 6}" rx="11"/>` : '') + shape(n.kind, w, h);
    if (n.kind === 'claim') inner += `<circle class="dot" cx="14" cy="${h / 2}" r="5"/>`;
    if (n.kind === 'action') inner += `<rect class="stripe" x="0" y="8" width="3" height="${h - 16}" rx="1.5"/>`;
    const subEl = lv ? `<text class="nsub lvsub" x="${tx}" y="38">${esc(lv.verb)} <tspan${lv.since != null ? ` data-since="${lv.since}"` : ''}>${lv.since != null ? fmtDur(opts.now - lv.since) : ''}</tspan></text>` : `<text class="nsub" x="${tx}" y="38">${esc(sub)}</text>`;
    inner += `<text class="nname" x="${tx}" y="21">${esc(label)}</text>${subEl}`;
    if (n.actor) {
      const t = actorLabel(n.actor.kind, ACTOR_LABEL);
      const bw = chipW;
      inner += `<g class="chip c-${n.actor.kind}"><rect x="${w - bw - 8}" y="27" width="${bw}" height="16" rx="8"/><text x="${w - bw / 2 - 8}" y="38.5" text-anchor="middle">${esc(t)}</text></g>`;
    }
    inner += `<title>${esc(`${n.kind} ${n.name}${st ? ': ' + st : ''}${n.actor ? '\n' + n.actor.why : ''}`)}</title>`;
    parts.push(`<g data-key="n:${esc(p.id)}" data-id="${esc(p.id)}" data-v="${esc(st || sub)}" class="${cls}" transform="translate(${p.x},${p.y})" tabindex="0">${inner}</g>`);
  }
  parts.push(`</g></g>`);
  patch(svg, parts.join(''));
  return {width, height};
}
