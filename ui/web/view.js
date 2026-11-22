// The runbook's view (format runbook.view/1, e.g. views/sdc.view.json): what the inspector knows
// about one runbook and does not read from the case store. Members:
//   runbook      the runbook's name
//   protocols    [regular expression]: the protocol ids of the runbook, which runs this view covers (without it: the
//                protocols of its phases)
//   phases       {stages: [name], items: [{id, stage, label, short?, protocol, claims, doc?, has_source?,
//                root_outcome?}], absent?: [{stage, steps, protocol_pattern, note}]}
//   file_kinds   {rules: [{match, flags?, class, kind}], artifacts: {artifact name: class},
//                parsers?: {kind: parser}}   a parser says what a change of a file of that kind means:
//                {type: "keyed-list", ...} or {type: "document", ...}, see ui/README.md
//   decisions    {scope_answer?, stall?}: which decisions a person takes, as the page shows them (it answers none)
// The server serves the view of a run at /api/view?source=N (null without one); the page and the server both
// call setView. A view with a member this file does not know is refused with a message that names it.
// Without a view: phases are one flat list of the protocols found in the store, a file's label is its
// name (class code), no tool has an owner, the decisions shown are the approvals the store itself
// asks for. No dependencies; runs in the page and in node.
export const VIEW_FORMAT = 'runbook.view/1';

const T = {
  str: {t: 'string'},
  bool: {t: 'boolean'},
  any: {t: 'any'},
  obj: (members) => ({t: 'object', members}),
  map: (of) => ({t: 'object', of}),
  arr: (of) => ({t: 'array', of}),
};
T.strs = T.arr(T.str);
const ident = {t: 'string', re: /^[A-Za-z_][A-Za-z0-9_]*$/, what: 'a field name (letters, digits and _)'};
const cond = T.obj({set: T.strs, was_unset: T.strs, changed: T.strs, equals: T.map(T.any)});
const rule = T.obj({when: cond, each: T.bool, except: T.strs, stop: T.bool, text: T.str, variants: T.arr(T.obj({when: cond, text: T.str}))});
const KEYED_LIST = T.obj({
  type: T.str, subject: T.str, list: T.str, as: T.str, id: T.str, word: T.str,
  added: T.arr(T.obj({when: cond, text: T.str})), removed: T.str, changed: T.obj({rules: T.arr(rule), otherwise: T.str}),
});
const DOCUMENT = T.obj({type: T.str, subject: T.str, entries: T.obj({word: T.str, sections: T.str, summary: T.str}), created: T.str});
const RECORD_FIELDS = T.obj({id: ident, item: ident, question: ident, options: ident, default: ident, status: ident, answer: ident, answered_by: ident, answered_at: ident, reason: ident, reasons: ident});

export const VIEW_SCHEMA = T.obj({
  format: T.str,
  runbook: T.str,
  protocols: T.strs,
  phases: T.obj({
    stages: T.strs,
    items: T.arr(T.obj({id: T.str, stage: T.str, label: T.str, short: T.str, protocol: T.str, claims: T.strs, doc: T.str, has_source: T.bool, root_outcome: T.str})),
    absent: T.arr(T.obj({stage: T.str, steps: T.strs, protocol_pattern: T.str, note: T.str})),
  }),
  file_kinds: T.obj({
    rules: T.arr(T.obj({match: T.str, flags: T.str, class: T.str, kind: T.str})),
    artifacts: T.map(T.str),
    parsers: T.map({t: 'typed', by: {'keyed-list': KEYED_LIST, document: DOCUMENT}}),
  }),
  decisions: T.obj({
    scope_answer: T.obj({action: T.str, artifact: T.str, list: T.str, flag: ident, fields: RECORD_FIELDS, statuses: T.obj({open: T.str, accepted: T.str, changed: T.str})}),
    stall: T.obj({action: T.str, artifact: T.str, list: T.str, noun: T.str, status: ident, needs_person: T.str}),
  }),
});

const typeName = (v) => (Array.isArray(v) ? 'array' : v === null ? 'null' : typeof v);
function check(v, s, path, out) {
  if (s.t === 'any') return;
  if (s.t === 'typed') {
    const by = s.by[v?.type];
    if (!by) out.push(`${path}.type: "${v?.type}" is not a type of the inspector (${Object.keys(s.by).join(', ')})`);
    else check(v, by, path, out);
    return;
  }
  if (s.t === 'object') {
    if (typeName(v) !== 'object') { out.push(`${path}: expected an object, found ${typeName(v)}`); return; }
    for (const [k, x] of Object.entries(v)) {
      const at = path ? `${path}.${k}` : k;
      if (s.members) {
        if (!(k in s.members)) out.push(`${at}: view member "${k}" is not known to the inspector (${VIEW_FORMAT})`);
        else check(x, s.members[k], at, out);
      } else if (s.of) check(x, s.of, at, out);
    }
    return;
  }
  if (s.t === 'array') {
    if (!Array.isArray(v)) { out.push(`${path}: expected an array, found ${typeName(v)}`); return; }
    v.forEach((x, i) => check(x, s.of, `${path}[${i}]`, out));
    return;
  }
  if (typeName(v) !== s.t) { out.push(`${path}: expected ${s.t}, found ${typeName(v)}`); return; }
  if (s.re && !s.re.test(v)) out.push(`${path}: "${v}" is not ${s.what}`);
}

// What is wrong with a view, one line each: a member this inspector does not know (the line names it and
// where it stands), a member of the wrong type, a regular expression that does not compile. [] when none.
export function checkView(v) {
  const out = [];
  if (!v || typeof v !== 'object' || Array.isArray(v)) return ['the view is not an object'];
  if (v.format !== VIEW_FORMAT) return [`format is ${JSON.stringify(v.format)}, not ${VIEW_FORMAT}`];
  check(v, VIEW_SCHEMA, '', out);
  const re = (p, s, flags) => { try { new RegExp(s, flags); } catch (e) { out.push(`${p}: not a regular expression (${e.message})`); } };
  (Array.isArray(v.file_kinds?.rules) ? v.file_kinds.rules : []).forEach((r, i) => typeof r?.match === 'string' && re(`file_kinds.rules[${i}].match`, r.match, r.flags));
  (Array.isArray(v.phases?.absent) ? v.phases.absent : []).forEach((r, i) => typeof r?.protocol_pattern === 'string' && re(`phases.absent[${i}].protocol_pattern`, r.protocol_pattern));
  (Array.isArray(v.protocols) ? v.protocols : []).forEach((r, i) => typeof r === "string" && re(`protocols[${i}]`, r));
  for (const [k, p] of Object.entries(v.file_kinds?.parsers || {})) for (const f of ["sections", "summary"]) if (typeof p?.entries?.[f] === "string") re(`file_kinds.parsers.${k}.entries.${f}`, p.entries[f]);
  return out;
}

// The current view lives in a state object. The page has one (the module's own). The server serves many
// runs at once, so it hands in a scope (bindViewScope): a function that returns the state of the request
// or tick being served (an AsyncLocalStorage store), and an await in the middle of an action cannot see
// another request's view.
const shared = {V: null, rules: []};
let scopeOf = null;
const cur = () => (scopeOf && scopeOf()) || shared;
export const bindViewScope = (fn) => { scopeOf = fn; };
export const newViewState = () => ({V: null, rules: []});

// A view that checkView refuses is not used (the inspector then runs without one); the problems are
// returned so a caller can say them. A view of another format is refused the same way.
export function setView(v) {
  const problems = v == null ? [] : checkView(v);
  const st = cur();
  st.V = v && !problems.length ? v : null;
  st.rules = [];
  for (const r of st.V?.file_kinds?.rules || []) st.rules.push({re: new RegExp(r.match, r.flags || ''), cls: r.class, kind: r.kind});
  return problems;
}
export const getView = () => cur().V;

export const phaseItems = () => (Array.isArray(cur().V?.phases?.items) ? cur().V.phases.items : []);
export const phaseStages = () => {
  const names = Array.isArray(cur().V?.phases?.stages) ? cur().V.phases.stages : [];
  return names.map((name) => [name, phaseItems().filter((p) => p.stage === name).map((p) => p.id)]);
};
export const absentStages = () => (Array.isArray(cur().V?.phases?.absent) ? cur().V.phases.absent : []);

// does a view cover a run whose cases run these protocols? One that lists `protocols` covers a run when one of
// them matches; else a view with phases covers a run when one of the protocols is the protocol of a phase (an
// `absent` stage names a protocol that is not there, so it says nothing); a view with neither covers any run
export function viewCovers(v, protocols) {
  const ps = [...protocols];
  if (Array.isArray(v?.protocols)) return ps.some((p) => v.protocols.some((re) => new RegExp(re).test(p)));
  const items = v?.phases?.items || [];
  if (!items.length) return true;
  return ps.some((p) => items.some((i) => i.protocol === p));
}

// the rule a path matches first: {cls, kind}, or null; kind "$basename" is the file name without its extension
export function fileKindOf(path) {
  const s = String(path || '').replace(/\\/g, '/');
  for (const r of cur().rules) {
    if (r.re.test(s)) return {cls: r.cls, kind: r.kind === '$basename' ? s.split('/').pop().replace(/\.\w+$/, '') : r.kind};
  }
  return null;
}
export const artifactClassOf = (name) => cur().V?.file_kinds?.artifacts?.[name] || null;
export const parserOf = (kind) => cur().V?.file_kinds?.parsers?.[kind] || null;

// the decisions a person takes in this runbook, as the page shows them (null each without a view or a member)
export const scopeAnswer = () => cur().V?.decisions?.scope_answer || null;
export const stallRule = () => cur().V?.decisions?.stall || null;
// the kind of file the questions of a decision are written in (feed: a change of it is a question asked or answered)
export const questionsKind = () => scopeAnswer()?.artifact || null;
