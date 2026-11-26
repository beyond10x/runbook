// A daemon of the Loom runtime for the specs of story daemon-page-shows-a-run-tree-and-products
// (daemon-run-tree.spec.mjs, daemon-run-tree-security.spec.mjs). Not a spec. The runbook home is a copy of the
// engine tests' fixture (runbook/tests/fixtures/engine-runbook) made three levels deep: the root `engine-demo` opens
// `section` (protocol engine-section), which opens the drafting case `draft` (a writer program, a check, and one person
// step). The person step is answered as the daemon's principal. Each daemon listens on 127.0.0.1:0 with a state
// directory and a working directory of its own; no model call is made.
import {spawnChild, SERVE_CAPS} from './child.mjs';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

export const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const bin = process.env.SDC_RUNBOOK_BIN || path.resolve(ui, '../runbook/target/debug/runbook');
const fixture = path.resolve(ui, '../runbook/tests/fixtures/engine-runbook');
export const PRINCIPAL = 'agent:test-daemon';

const SECTION_PROTOCOL = `format: protocol/1
protocol:
  id: engine.section
  revision: 1
  description: >-
    A section of the engine tests' runbook: it opens one drafting case and passes once that case passed.
artifacts:
  idea: {}
evidence_kinds:
  case_outcome: {}
  draft_opened: {}
claims:
  draft.opened:
    true_when:
      evidence: {kind: draft_opened, subject: idea, result: opened}
  draft.passed:
    true_when:
      children: {in: [passed]}
actions:
  draft.open:
    effect: write
    case_open: {protocol: engine.draft, subject: idea}
    may_produce:
      - evidence: draft_opened
outcomes:
  passed:
    requires:
      all:
        - {claim: draft.opened}
        - {claim: draft.passed}
`;
const SECTION_BINDINGS = `format: loom.bindings/1
protocol: engine-section@1
program_root: bindings
artifacts:
  idea: idea.yaml
actions:
  - action: draft.open
    executor:
      kind: open
      name: draft
      bindings: engine-draft.yaml
    evidence:
      - kind: draft_opened
        subject: idea
        producer: loom
        from: effect
        results: {performed: opened}
`;

function replaceIn(file, from, to) {
  const text = fs.readFileSync(file, 'utf8');
  if (!text.includes(from)) throw new Error(`${file} does not hold ${from}`);
  fs.writeFileSync(file, text.replace(from, to));
}

// the three-level runbook home under `dir`
export function deepHome(dir) {
  const home = path.join(dir, 'home');
  fs.cpSync(fixture, home, {recursive: true});
  fs.writeFileSync(path.join(home, 'protocol', 'engine-section.protocol.yaml'), SECTION_PROTOCOL);
  fs.writeFileSync(path.join(home, 'bindings', 'engine-section.yaml'), SECTION_BINDINGS);
  replaceIn(path.join(home, 'protocol', 'engine-demo.protocol.yaml'), 'case_open: {protocol: engine.draft, subject: idea}', 'case_open: {protocol: engine.section, subject: idea}');
  replaceIn(path.join(home, 'bindings', 'engine-demo.yaml'), '      name: draft\n      bindings: engine-draft.yaml', '      name: section\n      bindings: engine-section.yaml');
  replaceIn(path.join(home, 'bindings', 'engine-draft.yaml'), 'principal: reviewer', `principal: ${PRINCIPAL}`);
  return home;
}

// `runbook serve --ui --runtime loom` over the three-level home; resolves {proc, base, state}
export function startDaemon(dir) {
  const home = deepHome(dir);
  fs.mkdirSync(path.join(dir, 'empty'), {recursive: true});
  const state = path.join(dir, 'state');
  return new Promise((resolve, reject) => {
    const proc = spawnChild(bin, ['serve', ...SERVE_CAPS, '--ui', '--principal', PRINCIPAL, '--listen', '127.0.0.1:0', '--state-dir', state, '--runtime', 'loom', '--runbook-home', home, '--min-confinement', 'local'],
      {cwd: path.join(dir, 'empty'), env: {...process.env, HOME: dir, RUNBOOK_TOKEN: ''}, stdio: ['ignore', 'pipe', 'pipe']});
    let out = '', err = '';
    const timer = setTimeout(() => { proc.kill(); reject(new Error(`no listening line in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { err += d; });
    proc.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) { clearTimeout(timer); proc.removeAllListeners('exit'); resolve({proc, base: m[1], state}); }
    });
    proc.on('exit', (code) => { clearTimeout(timer); reject(new Error(`runbook exited with ${code}: ${err}${out}`)); });
  });
}

export const post = (d, name, body) => fetch(`${d.base}/daemon/commands/${name}`, {method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify(body)}).then((r) => r.json());
export const rows = (d, view, q = '') => fetch(`${d.base}/daemon/views/${view}${q}`).then((r) => r.json()).then((j) => j.rows);
export const workspace = (d, run) => path.join(d.state, 'workspaces', run);

// submit a run of the three-level runbook; resolves its id
export async function submit(d, intake) {
  const r = await post(d, 'submit-run', {runbook_id: 'engine-demo', intake, confinement: 'local'});
  const id = r.published?.[0]?.payload?.run_id;
  if (!id) throw new Error(`no run: ${JSON.stringify(r)}`);
  return id;
}

// the open review request of a run (the person step of `draft`), or null
export async function review(d, run) {
  const open = await rows(d, 'open-decisions', `?run_id=${encodeURIComponent(run)}`);
  return open.find((x) => (x.options || []).some((o) => String(o.answer || '').includes('result'))) ?? null;
}

// answer every open request of the run, the review with `performed` and any other with `approve`, until it ended
export async function settle(d, run, poll) {
  const answered = new Set();
  await poll(async () => {
    const [r] = await rows(d, 'run', `?run_id=${encodeURIComponent(run)}`);
    if (['Ended', 'Cancelled', 'FailedToStart'].includes(r.state)) return r.state;
    for (const x of await rows(d, 'open-decisions', `?run_id=${encodeURIComponent(run)}`)) {
      if (answered.has(x.request_id)) continue;
      answered.add(x.request_id);
      const isReview = (x.options || []).some((o) => String(o.answer || '').includes('result'));
      await post(d, 'answer-decision', {run_id: run, request_id: x.request_id, answer: isReview ? 'performed' : 'approve', answered_by: 'spec'});
    }
    return r.state;
  });
}
