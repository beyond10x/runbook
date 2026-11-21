// The runbook's view (format runbook.view/1, e.g. views/sdc.view.json): what the inspector knows
// about one runbook and does not read from the case store. Three tables:
//   phases       {stages: [name], items: [{id, stage, label, short?, protocol, claims, doc?, has_source?,
//                root_outcome?}], absent?: [{stage, steps, protocol_pattern, note}]}
//   file_kinds   {rules: [{match, flags?, class, kind}], artifacts: {artifact name: class}}
//   tool_owners  {always, driver, continue: [program], tools: [{argv0, subcommands: [{match, needs}]}]}
// The server serves it at /api/view (null without one); the page and the server both call setView.
// Without a view: phases are one flat list of the protocols found in the store, a file's label is its
// name (class code), and no tool has an owner. No dependencies; runs in the page and in node.
export const VIEW_FORMAT = 'runbook.view/1';

let V = null;
let rules = [];

// A view of another format or a malformed one is ignored (the inspector then runs without one).
export function setView(v) {
  V = v && typeof v === 'object' && v.format === VIEW_FORMAT ? v : null;
  rules = [];
  for (const r of V?.file_kinds?.rules || []) {
    try { rules.push({re: new RegExp(r.match, r.flags || ''), cls: r.class, kind: r.kind}); } catch { /* a bad pattern matches nothing */ }
  }
}
export const getView = () => V;

export const phaseItems = () => (Array.isArray(V?.phases?.items) ? V.phases.items : []);
export const phaseStages = () => {
  const names = Array.isArray(V?.phases?.stages) ? V.phases.stages : [];
  return names.map((name) => [name, phaseItems().filter((p) => p.stage === name).map((p) => p.id)]);
};
export const absentStages = () => (Array.isArray(V?.phases?.absent) ? V.phases.absent : []);

// the rule a path matches first: {cls, kind}, or null; kind "$basename" is the file name without its extension
export function fileKindOf(path) {
  const s = String(path || '').replace(/\\/g, '/');
  for (const r of rules) {
    if (r.re.test(s)) return {cls: r.cls, kind: r.kind === '$basename' ? s.split('/').pop().replace(/\.\w+$/, '') : r.kind};
  }
  return null;
}
export const artifactClassOf = (name) => V?.file_kinds?.artifacts?.[name] || null;

export const toolOwners = () => V?.tool_owners || null;
