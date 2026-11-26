// What a step produced, in the page (story inspector-shows-step-products): the product line under the header and the
// Products tab of the panel: the evidence records of the case, its actions and the files of the run, each opened in
// the same panel. Everything here is built with createElement and text nodes, outside the page's patch(): what a run
// holds (a file, the output of an action, an evidence record) is the text of a confined model and never becomes
// markup of the page. A value the run did not record says so, never an empty field. On the daemon's page the tab lists the
// run's files (the view `products`) and reads one through the view `product` (web/source.js daemonFile); the daemon serves
// no evidence record and no action output, and the tab says so.
import {daemonFile} from './source.js';

// the renderer is loaded when the first Markdown file is opened: the first screen does not wait for it
let markdown = null;
const loadMarkdown = () => import('./markdown.js').then((m) => { markdown = m; P.v++; hooks.refresh(); }, () => { markdown = false; P.v++; hooks.refresh(); });

export const NR = 'not recorded by this run';
const MARKDOWN_LIMIT = 131072; // a longer file is shown as written: a hostile one must not keep the page busy
const P = {view: {kind: 'list'}, cache: new Map(), v: 0};
let hooks = {refresh() {}, open() {}};
export const bindProducts = (h) => { hooks = {...hooks, ...h}; };
export const showProduct = (view) => { P.view = view; };

function h(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null && k !== false) e.append(k instanceof Node ? k : document.createTextNode(String(k)));
  return e;
}
const attr = (e, o) => { for (const [k, v] of Object.entries(o)) e.setAttribute(k, String(v)); return e; };
const btn = (cls, data, onClick, ...kids) => { const b = attr(h('button', cls, ...kids), {type: 'button', ...data}); b.addEventListener('click', onClick); return b; };

const dirOf = (p) => p.split('/').slice(0, -1).join('/');
const fmtBytes = (n) => `${n} byte${n === 1 ? '' : 's'}`;
export function fmtSecs(ms) {
  if (ms == null || isNaN(ms)) return null;
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`;
}
const when = (t) => (t == null ? null : new Date(t).toISOString().replace('T', ' ').replace(/\.\d+Z$/, ' UTC'));

// ---- fetched content -----------------------------------------------------------------------------------
async function fetchJson(url) {
  try {
    const r = await fetch(url);
    let body = null;
    try { body = await r.json(); } catch { /* not JSON */ }
    return {status: r.status, body};
  } catch (e) { return {status: 0, body: {error: String(e?.message || e)}}; }
}
// `url` is the inspector's route, or a function that answers in its shape (the daemon's view `product`, web/source.js)
function need(key, url) {
  const hit = P.cache.get(key);
  if (hit) return hit;
  P.cache.set(key, {loading: true});
  (typeof url === 'function' ? url() : fetchJson(url)).then((r) => { P.cache.set(key, r); P.v++; hooks.refresh(); });
  return P.cache.get(key);
}

// ---- the product line ------------------------------------------------------------------------------------
export function renderProductBar(host, {products, daemon}) {
  const sig = JSON.stringify([products?.product, products?.served, daemon, daemon ? products?.files?.length : null]);
  if (host.__sig === sig) return;
  host.__sig = sig;
  host.replaceChildren();
  host.hidden = !daemon && !products;
  if (host.hidden) return;
  host.append(h('span', 'prod-k', 'Product'));
  if (daemon) {
    // the daemon names no product of a step: the line counts the run's files, and the Products tab lists them
    const n = products?.files?.length;
    host.append(h('span', 'prod-via', n == null ? 'reading the run…' : n ? `${n} ${n === 1 ? 'file' : 'files'} under the run's work/ directory: the Products tab lists ${n === 1 ? 'it' : 'them'}` : 'the run has written no file under work/ yet'));
    return;
  }
  const p = products.product;
  const link = (path, cls = 'prod-link') => { const a = attr(h('a', cls, path), {href: `#file=${encodeURIComponent(path)}`, 'data-path': path}); a.addEventListener('click', (e) => { e.preventDefault(); hooks.open({kind: 'file', path}); }); return a; };
  if (p.path && !p.missing) {
    host.append(link(p.path), h('span', 'prod-via', p.note));
    for (const o of p.others || []) host.append(link(o.path, 'prod-link prod-other'));
  } else if (p.path) host.append(h('span', 'prod-via prod-missing', p.note));
  else host.append(h('span', 'prod-via', p.note));
}

// ---- field rows ---------------------------------------------------------------------------------------------
function row(name, value, extra = {}) {
  const r = attr(h('div', 'pf-row'), {'data-field': name, ...extra});
  const v = h('span', 'pf-v');
  if (value instanceof Node) v.append(value);
  else if (value == null || value === '') { v.append(NR); r.classList.add('pf-nr'); } else v.append(String(value));
  r.append(h('span', 'pf-k', name), v);
  return r;
}
const scalar = (v) => (typeof v === 'string' ? v : v === null ? 'null' : String(v));
// a value nested deeper than this is not drawn: the line says where it was cut
const MAX_NEST = 24;
function nested(key, value, path, depth) {
  const here = path ? `${path}.${key}` : key;
  if (value !== null && typeof value === 'object' && depth >= MAX_NEST) return leaf(key, `cut at depth ${MAX_NEST}`, here);
  if (value !== null && typeof value === 'object') {
    const entries = Array.isArray(value) ? value.map((x, i) => [String(i), x]) : Object.entries(value);
    if (!entries.length) return leaf(key, Array.isArray(value) ? '[]' : '{}', here);
    const g = attr(h('div', 'pf-grp'), {'data-path': here, style: `--depth:${depth}`});
    g.append(h('div', 'pf-gk', Array.isArray(value) ? `${key} (${entries.length})` : key));
    for (const [k, x] of entries) g.append(nested(k, x, here, depth + 1));
    return g;
  }
  return leaf(key, scalar(value), here);
}
const leaf = (key, text, here) => { const r = attr(h('div', 'pf-row pf-leaf'), {'data-path': here}); r.append(h('span', 'pf-k', key), h('span', 'pf-v', text === '' ? '""' : text)); return r; };

// ---- details ---------------------------------------------------------------------------------------------------
const back = () => btn('prod-back', {}, () => { P.view = {kind: 'list'}; hooks.refresh(); }, '← all products');

function evidenceDetail(c, id) {
  const r = c?.evidence?.[id];
  const d = attr(h('div', 'prod-detail'), {'data-kind': 'evidence'});
  d.append(back(), h('h3', 'prod-title', 'Evidence record ', h('code', '', id)));
  if (!r) { d.append(h('p', 'prod-note', 'this record is not in the evidence/ of the case')); return d; }
  const rows = h('div', 'pf-rows');
  const seen = new Set();
  for (const k of ['result', 'subject', 'producer', 'observed_at']) { seen.add(k); rows.append(row(k, r[k] != null && typeof r[k] !== 'object' ? scalar(r[k]) : null)); }
  const deep = [];
  for (const [k, v] of Object.entries(r)) {
    if (seen.has(k)) continue;
    if (v !== null && typeof v === 'object') deep.push([k, v]); else rows.append(row(k, scalar(v)));
  }
  d.append(rows);
  if (deep.length) {
    const box = h('div', 'pf-nested');
    for (const [k, v] of deep) box.append(nested(k, v, '', 0));
    d.append(box);
  }
  const pe = c.events.find((e) => e.event === 'action_finished' && (e.evidence || []).includes(id));
  if (pe) d.append(h('p', 'prod-note', `written by the action ${pe.action} (event #${pe.seq}, exit ${pe.exit ?? NR})`));
  return d;
}

function actionDetail(c, seq, src) {
  const e = c?.events?.find((x) => x.seq === seq && x.event === 'action_finished');
  const d = attr(h('div', 'prod-detail'), {'data-kind': 'action'});
  d.append(back(), h('h3', 'prod-title', 'Action ', h('code', '', e ? e.action : `#${seq}`)));
  if (!e) { d.append(h('p', 'prod-note', 'this action finish is not in the events of the case')); return d; }
  const started = [...c.events].reverse().find((x) => x.event === 'action_started' && x.action === e.action && x.seq < e.seq);
  const rows = h('div', 'pf-rows');
  const ex = e.exit == null || e.exit === '' ? null : String(e.exit);
  rows.append(row('exit', ex, ex != null && ex !== '0' ? {'data-bad': 'true'} : {}));
  const fromRunner = e.source === 'loom-runlog';
  let t0 = null, t1 = null;
  if (!fromRunner) {
    const ok = (t) => (t && t !== '-' && !isNaN(Date.parse(t)) ? t : null);
    t0 = ok(started?.at); t1 = ok(e.at);
  }
  rows.append(row('duration', t0 && t1 ? fmtSecs(Date.parse(t1) - Date.parse(t0)) : null), row('started', t0), row('ended', t1));
  if (e.principal) rows.append(row('principal', e.principal));
  if (e.evidence?.length) rows.append(row('evidence', e.evidence.join(', ')));
  d.append(rows);
  // the step the action ran in: the runner dates a step as a whole (observed.jsonl), never one action
  const a = c.times?.[started?.seq], b = c.times?.[e.seq];
  if (a != null && b != null && c.timeSource?.[started.seq] === 'driver' && c.timeSource?.[e.seq] === 'driver') {
    d.append(h('p', 'prod-note prod-span', `the step this action ran in: ${when(a)} to ${when(b)} (${fmtSecs(b - a)}), as observed.jsonl dates the step; the run does not date the action itself`));
  }
  // the output: one stream, as the run keeps it
  const out = attr(h('div', 'pf-row pf-out'), {'data-field': 'output'});
  const ov = h('span', 'pf-v');
  const o = need(`out:${src}:${c.rel}:${seq}`, `/api/output?source=${src}&case=${encodeURIComponent(c.rel)}&seq=${seq}`);
  const note = h('span', 'prod-note');
  if (o.loading) ov.append('reading the output…');
  else if (o.body?.recorded) {
    const t = o.body.text;
    if (t == null) ov.append(`${NR}: the output is not UTF-8 text (${fmtBytes(o.body.bytes)})`);
    else {
      ov.append(h('pre', 'prod-text', t === '' ? '(empty output)' : t));
      note.append(`${o.body.stream}${o.body.truncated ? `; the last ${o.body.shown} of ${fmtBytes(o.body.bytes)} are shown` : `; ${fmtBytes(o.body.bytes)}`}`);
    }
  } else { out.classList.add('pf-nr'); ov.append(`${NR} (${o.body?.why ?? o.body?.error ?? `answer ${o.status}`})`); }
  if (note.hasChildNodes()) ov.append(note);
  out.append(h('span', 'pf-k', 'output'), ov);
  d.append(out);
  const rest = h('div', 'pf-rows');
  for (const s of ['stdout', 'stderr']) rest.append(row(s, `${NR} as a stream of its own: the run keeps one stream of output`, {}));
  d.append(rest);
  return d;
}

function fileDetail(path, files, src, run) {
  const d = attr(h('div', 'prod-detail'), {'data-kind': 'file'});
  d.append(back(), h('div', 'prod-path', path));
  const f = files?.find((x) => x.path === path);
  const key = `file:${run ?? src}:${path}:${f?.mtime ?? ''}:${f?.size ?? ''}`;
  const r = need(key, run != null ? () => daemonFile(run, path) : `/api/file?source=${src}&path=${encodeURIComponent(path)}`);
  if (r.loading) { d.append(h('p', 'prod-note', 'reading the file…')); return d; }
  const b = r.body || {};
  if (r.status !== 200) {
    d.append(h('p', 'prod-note prod-refused', `${b.error ?? `answer ${r.status}`}${b.type ? ` (${b.type})` : ''}`));
    return d;
  }
  d.append(h('div', 'prod-meta', `${fmtBytes(b.size)} · ${b.kind === 'text' ? b.format : 'not text'}${b.mtime ? ` · written ${when(b.mtime)}` : ''}`));
  if (b.kind !== 'text') { d.append(h('p', 'prod-note', `${fmtBytes(b.size)}, ${b.type}: shown by its size and type only`)); return d; }
  if (b.truncated) d.append(h('p', 'prod-note', `showing the first ${b.shown} of ${b.bytes} bytes`));
  let md = null;
  if (b.format === 'markdown') {
    if (markdown === null) { loadMarkdown(); markdown = undefined; }
    if (markdown === undefined) { d.append(h('p', 'prod-note', 'reading the file…')); return d; }
    if (markdown === false) d.append(h('p', 'prod-note', 'the Markdown renderer could not be loaded: shown as written'));
    else if (b.text.length > MARKDOWN_LIMIT) d.append(h('p', 'prod-note', `this file is longer than ${MARKDOWN_LIMIT} characters: shown as written, not rendered`));
    else {
      md = h('div', 'md');
      try { markdown.renderMarkdown(b.text, {dir: dirOf(path), open: (p) => hooks.open({kind: 'file', path: p})}, md); } catch (e) {
        md = null;
        d.append(h('p', 'prod-note', `this file could not be rendered as Markdown (${e?.name || 'error'}): shown as written`));
      }
    }
  }
  if (md) d.append(md);
  else {
    let text = b.text;
    if (b.format === 'json' && !b.truncated) { try { text = JSON.stringify(JSON.parse(b.text), null, 2); } catch { /* shown as written */ } }
    d.append(h('pre', 'prod-text', text));
  }
  return d;
}

// ---- the products of a run: the files its records name as written by an action (story products-are-what-the-run-wrote) ----
// Groups by step, newest first, then one closed "Inputs" line for what the run was seeded with. Every name, step, action and
// count below is a run's own text: set with createElement and text nodes, never as markup.
const MARKS = {changed: 'changed', added: 'added', removed: 'removed'};
const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
function recordLists(products) {
  const out = [];
  out.push(h('h4', '', `Products (${products.total})`));
  const body = h('div', 'prod-files prod-groups');
  for (const g of products.groups || []) {
    const sec = attr(h('section', 'prod-group'), {'data-step': g.step == null ? 'unknown' : g.step});
    const names = (g.actions || []).join(', ');
    sec.append(h('h5', 'prod-gh', g.step == null ? 'A step the record does not name' : `Step ${g.step}`, names ? h('span', 'prod-sub', names) : null));
    for (const f of g.files) {
      const mark = MARKS[f.mark] ? attr(h('span', `prod-mark prod-mark-${f.mark}`, MARKS[f.mark]), {'data-mark': f.mark}) : null;
      const sub = f.size == null ? 'no longer in the run' : f.artifact ? `${fmtBytes(f.size)} · artifact ${f.artifact}` : fmtBytes(f.size);
      const b = btn('prod-file', {'data-path': f.path, ...(f.mark ? {'data-mark': f.mark} : {})}, () => { P.view = {kind: 'file', path: f.path}; hooks.refresh(); },
        h('span', 'prod-fp', f.path), mark, h('span', 'prod-sub', sub), f.result ? h('span', 'prod-result', f.result) : null);
      if (f.mark === 'removed') b.disabled = true;
      sec.append(b);
    }
    body.append(sec);
  }
  if (!(products.groups || []).length) body.append(h('p', 'prod-note', 'no action has written a file yet'));
  if (products.notShown) body.append(h('p', 'prod-note prod-cut', `${plural(products.notShown, 'product is', 'products are')} not shown: the page lists at most ${products.shown} of ${products.total} (the oldest steps come last)`));
  if (products.leftOut) body.append(h('p', 'prod-note', `${plural(products.leftOut, 'file is', 'files are')} not listed: a file with more than one link, or a link the inspector does not follow, is neither listed nor served`));
  if (products.gone) body.append(h('p', 'prod-note', `${plural(products.gone, 'file', 'files')} a record names ${products.gone === 1 ? 'is' : 'are'} no longer in the run`));
  if (products.refusedRecords) body.append(h('p', 'prod-note', `${plural(products.refusedRecords, 'change record names', 'change records name')} a path outside the run's work/ directory: not listed, not opened`));
  if (products.cutStrings) body.append(h('p', 'prod-note', `${plural(products.cutStrings, 'action name is', 'action names are')} cut to ${products.maxAction} characters`));
  if (products.linkCut) body.append(h('p', 'prod-note prod-cut', `${plural(products.linkCut, 'name passes', 'names pass')} more than ${products.maxLinks} links: not listed, the inspector resolves no more than that for one name`));
  if (products.recordsStopped) body.append(h('p', 'prod-note prod-cut', `the list of file changes was read to record ${products.recordsStopped.read} only: ${products.recordsStopped.why}`));
  out.push(body);
  const inputs = products.inputs || {trees: [], files: 0};
  const det = attr(h('details', 'prod-inputs'), {'data-files': inputs.files});
  det.append(h('summary', 'prod-inputs-s', `Inputs · ${plural(inputs.trees.length, 'entry', 'entries')} · ${plural(inputs.files, 'file', 'files')}`));
  det.append(h('p', 'prod-note', 'what the run was seeded with: not products, not listed file by file; a file here that an action changed or added is listed above'));
  for (const t of inputs.trees) det.append(attr(h('div', 'prod-input', h('span', 'prod-fp', t.path), h('span', 'prod-sub', `${t.kind === 'dir' ? 'tree' : t.kind} · ${plural(t.files, 'file', 'files')}`)), {'data-path': t.path}));
  out.push(det);
  return out;
}

// ---- the panel -------------------------------------------------------------------------------------------------
function lists(c, products, src) {
  const root = h('div', 'prod-lists');
  const evs = Object.keys(c?.evidence || {});
  root.append(h('h4', '', `Evidence records (${evs.length})`));
  const el = h('div', 'prod-evs');
  for (const id of evs) {
    const r = c.evidence[id];
    el.append(btn('prod-ev', {'data-id': id}, () => { P.view = {kind: 'evidence', id}; hooks.refresh(); }, h('span', 'prod-fp', id), h('span', 'prod-sub', [r.result, r.subject].filter((x) => x != null && typeof x !== 'object').join(' · ') || NR)));
  }
  if (!evs.length) el.append(h('p', 'prod-note', 'no evidence record yet'));
  root.append(el);
  const acts = (c?.events || []).filter((e) => e.event === 'action_finished');
  root.append(h('h4', '', `Actions (${acts.length})`));
  const al = h('div', 'prod-acts');
  for (const e of acts) {
    const bad = e.exit != null && String(e.exit) !== '0';
    al.append(btn('prod-act', {'data-aseq': e.seq, ...(bad ? {'data-bad': 'true'} : {})}, () => { P.view = {kind: 'action', seq: e.seq}; hooks.refresh(); }, h('span', 'prod-fp', e.action), h('span', 'prod-sub', `#${e.seq} · exit ${e.exit ?? NR}`)));
  }
  if (!acts.length) al.append(h('p', 'prod-note', 'no action has finished yet'));
  root.append(al);
  if (products?.basis === 'records') { root.append(...recordLists(products)); return root; }
  root.append(fileList(products));
  return root;
}

// The daemon's page: the run's files, which its cases share; no evidence record and no action output
function daemonLists(products) {
  const root = h('div', 'prod-lists');
  root.append(h('p', 'prod-note prod-daemon', 'the daemon serves the files under the run\'s work/ directory, which its cases share; it serves no evidence records and no action output of a case'));
  root.append(fileList(products));
  return root;
}

function fileList(products) {
  const root = document.createDocumentFragment();
  const files = products?.files || [];
  root.append(h('h4', '', `Files (${files.length})`));
  const fl = h('div', 'prod-files');
  if (products?.basisNote) fl.append(h('p', 'prod-note prod-basis', products.basisNote));
  for (const f of files) {
    fl.append(btn('prod-file', {'data-path': f.path}, () => { P.view = {kind: 'file', path: f.path}; hooks.refresh(); },
      h('span', 'prod-fp', f.path), h('span', 'prod-sub', f.artifact ? `${fmtBytes(f.size)} · artifact ${f.artifact}` : fmtBytes(f.size)), f.result ? h('span', 'prod-result', f.result) : null));
  }
  if (!files.length) fl.append(h('p', 'prod-note', 'the run has written no file under work/ yet'));
  const hid = (products?.hiddenDirectories || 0) + (products?.hiddenFiles || 0);
  if (hid) fl.append(h('p', 'prod-hidden', `${hid} ${hid === 1 ? 'entry whose name starts with a dot is' : 'entries whose name starts with a dot are'} not listed (a tool's cache or login is neither listed nor served)`));
  if (products?.leftOut) fl.append(h('p', 'prod-note', `${products.leftOut} ${products.leftOut === 1 ? 'file is' : 'files are'} not listed: a file with more than one link, or a link the inspector does not follow, is neither listed nor served`));
  if (products?.truncated) fl.append(h('p', 'prod-note', `the list is cut: ${{files: 'the run holds more files than the page lists', directories: 'the run holds more directories than the page walks', entries: 'the run holds more entries than the page walks', depth: 'a directory lies deeper than the page walks'}[products.cut] || 'the run holds more than the page lists'}`));
  root.append(fl);
  return root;
}

export function renderProducts(host, {c, products, src, daemon, run}) {
  const sig = JSON.stringify([P.view, P.v, src, c?.rel, Object.keys(c?.evidence || {}), c?.events?.length, products?.files?.map((f) => `${f.path}:${f.size}:${f.mtime}:${f.result}:${f.mark}:${f.step}`), products?.basis, products?.basisNote, products?.notShown, products?.gone, products?.refusedRecords, products?.cutStrings, products?.linkCut, products?.recordsStopped?.cause, products?.recordsStopped?.read, products?.inputs, products?.hiddenDirectories, products?.hiddenFiles, products?.leftOut, products?.cut, daemon, products?.served, run]);
  if (host.firstChild?.__sig === sig) return;
  const root = h('div', 'prod');
  root.__sig = sig;
  if (!products) root.append(h('p', 'prod-note', 'reading the run…'));
  else if (daemon && P.view.kind === 'file') root.append(fileDetail(P.view.path, products.files, src, run));
  else if (daemon) root.append(daemonLists(products));
  else if (P.view.kind === 'evidence') root.append(evidenceDetail(c, P.view.id));
  else if (P.view.kind === 'action') root.append(actionDetail(c, P.view.seq, src));
  else if (P.view.kind === 'file') root.append(fileDetail(P.view.path, products.files, src));
  else root.append(lists(c, products, src));
  host.replaceChildren(root);
}
