// What the runner copied into a run directory of `runbook run`, read for the graph: the protocol files
// (`protocols/*.protocol.yaml`, Canon `protocol/1`) with their artifacts and actions, and the bindings copy
// (`bindings/*.yaml`, `loom.bindings/1`) with each action's executor and the placement of each artifact. The
// runner's own projection (`store/<root>/ir.json`, `bindings.json`) holds only the effect of each action and the
// person and copy actions (docs/loom-run-directory.md), so a graph made from it alone draws actions with no
// artifacts and no program. YAML is read with the reader the inspector already uses for `work/questions.yaml`
// (`yq`, decisions.mjs); nothing here adds a dependency. A part that cannot be read is named in `parts`, and the
// page says "not available for this run" for it.
import fs from 'node:fs';
import path from 'node:path';
import {yamlJson} from './decisions.mjs';

// the registration stem of a protocol id: `.` and `_` written as `-` (runbook/src/loom_run/run_dir.rs `stem`)
const stem = (id) => String(id ?? '').replace(/[._]/g, '-');
const yamlFiles = (dir) => { try { return fs.readdirSync(dir).filter((n) => n.endsWith('.yaml')).sort(); } catch { return []; } };
const sigOf = (dir) => yamlFiles(dir).map((n) => { try { const st = fs.statSync(path.join(dir, n)); return `${n}:${st.size}:${st.mtimeMs}`; } catch { return n; } }).join('|');
const cache = new Map();

// {protocols: [doc], bindings: [doc], unreadable: [file name]} for one run directory, cached on the files' signatures
export function runFacts(runDir) {
  const pdir = path.join(runDir, 'protocols'), bdir = path.join(runDir, 'bindings');
  const sig = `${sigOf(pdir)}#${sigOf(bdir)}`;
  const hit = cache.get(runDir);
  if (hit && hit.sig === sig) return hit.facts;
  const facts = {protocols: [], bindings: [], unreadable: []};
  for (const [dir, list] of [[pdir, facts.protocols], [bdir, facts.bindings]]) {
    for (const n of yamlFiles(dir)) {
      const doc = yamlJson(path.join(dir, n));
      if (doc && typeof doc === 'object' && !Array.isArray(doc)) list.push(doc); else facts.unreadable.push(`${path.basename(dir)}/${n}`);
    }
  }
  cache.set(runDir, {sig, facts});
  return facts;
}

// the binding the page reads (web/model.js actorOf) for one action of a loom.bindings/1 file
function bindingOf(a) {
  const ex = a?.executor || {};
  const first = Array.isArray(a?.evidence) ? a.evidence[0] : null;
  const out = {executor: ex.kind ?? null};
  if (ex.kind === 'person') return {...out, manual: true, ...(ex.principal ? {principal: ex.principal} : {}), ...(Array.isArray(ex.argv) ? {argv: ex.argv} : {})};
  if (ex.kind === 'copy') return {...out, copy: {kind: ex.evidence ?? null, from: ex.from ?? null}};
  if (ex.kind === 'open') return {...out, child: true};
  const argv = Array.isArray(ex.argv) ? ex.argv.map(String) : [];
  const model = argv.length && path.basename(argv[0]) === 'sdc-node';
  return {...out, argv, ...(model ? {llm: true} : {}), ...(first?.producer ? {producer: first.producer} : {}), ...(a?.timeout_seconds ? {timeout_seconds: a.timeout_seconds} : {})};
}

// The graph's inputs for one case of a run of the runner: `ir` (the store's ir.json, or null, with the protocol's
// artifacts and actions added) and `bindings` (the store's bindings.json with the bindings copy's actions and
// `$artifacts`), and `parts`: what could be read. `protocol` is the protocol id of the case, `revision` its revision
// from the `opened` event.
export function applyFacts({protocol, revision, ir, bindings}, facts) {
  const pdoc = facts.protocols.find((d) => d?.protocol?.id === protocol);
  const bdoc = facts.bindings.find((d) => stem(String(d?.protocol ?? '').split('@')[0]) === stem(protocol));
  const bad = facts.unreadable.length ? ` (could not read ${facts.unreadable.join(', ')}: is yq installed?)` : '';
  const parts = {
    loom: true,
    protocol: pdoc ? {state: 'copy'} : {state: 'unavailable', why: `no protocols/ copy of the run directory holds ${protocol || 'the protocol of this case'}${bad}`},
    bindings: bdoc ? {state: 'copy'} : {state: 'unavailable', why: `no bindings/ copy of the run directory is for ${protocol || 'the protocol of this case'}${bad}`},
  };
  let outIr = ir;
  if (pdoc) {
    const actions = {};
    const decl = pdoc.actions && typeof pdoc.actions === 'object' ? pdoc.actions : {};
    for (const [name, d] of Object.entries(decl)) actions[name] = {...(ir?.actions?.[name] || {}), ...(d?.description ? {description: d.description} : {}), ...(d?.effect ? {effect: d.effect} : {}), ...(Array.isArray(d?.may_produce) ? {may_produce: d.may_produce} : {})};
    const artifacts = {};
    for (const [name, d] of Object.entries(pdoc.artifacts && typeof pdoc.artifacts === 'object' ? pdoc.artifacts : {})) artifacts[name] = {description: d?.description ?? null};
    outIr = {...(ir || {}), protocol: {id: protocol, revision: pdoc.protocol.revision ?? revision ?? null, description: pdoc.protocol.description ?? null}, artifacts, actions};
  }
  let outBindings = bindings;
  if (bdoc) {
    outBindings = {...(bindings || {})};
    for (const a of Array.isArray(bdoc.actions) ? bdoc.actions : []) if (a?.action) outBindings[a.action] = {...(bindings?.[a.action] || {}), ...bindingOf(a)};
    outBindings.$artifacts = bdoc.artifacts && typeof bdoc.artifacts === 'object' ? bdoc.artifacts : {};
  }
  return {ir: outIr, bindings: outBindings, parts};
}
