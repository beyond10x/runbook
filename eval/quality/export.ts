// Writes the sanitized result files for experiments/quality-0.1.0/ from the scratch results.
//   node export.ts <quality dir> <out dir>
import fs from 'node:fs';
import path from 'node:path';

const [q, out] = process.argv.slice(2);
const u = JSON.parse(fs.readFileSync(path.join(q, 'unblinded.json'), 'utf8'));
const findings = JSON.parse(fs.readFileSync(path.join(q, 'findings-blinded.json'), 'utf8'));
const cost: Record<string, {usd: number; wall_s: number; source: string}> = {
  'trial-0.1.0-strong-1': {usd: 5.432, wall_s: 1075, source: 'experiments/baseline-0.1.0/report.md:43'},
  'trial-0.1.0-strong-2': {usd: 6.194, wall_s: 1247, source: 'experiments/baseline-0.1.0/report.md:44'},
  'trial-0.1.0-strong-3': {usd: 5.503, wall_s: 1430, source: 'experiments/baseline-0.1.0/report.md:45'},
  'trial-0.1.0-mixed-1': {usd: 2.497, wall_s: 1480, source: 'experiments/baseline-0.1.0/report.md:46'},
  'trial-0.1.0-mixed-2': {usd: 3.883, wall_s: 2091, source: 'experiments/baseline-0.1.0/report.md:47'},
  'trial-0.1.0-mixed-3': {usd: 2.549, wall_s: 1489, source: 'experiments/baseline-0.1.0/report.md:48'},
  'free-agent-0.1.0-sonnet-1': {usd: 0.470, wall_s: 163, source: 'run events.jsonl result event (total_cost_usd 0.4701, duration_ms 163052)'},
  'free-agent-0.1.0-sonnet-2': {usd: 0.414, wall_s: 143, source: 'run events.jsonl result event (total_cost_usd 0.4137, duration_ms 143457)'},
  'free-agent-0.1.0-sonnet-3': {usd: 0.478, wall_s: 173, source: 'run events.jsonl result event (total_cost_usd 0.4779, duration_ms 173474)'},
};
const routes: Record<string, string[]> = {
  p1: ['GET /', 'GET /sw.js', 'GET /api/health', 'POST /api/identity/enter-name', 'GET /api/identity/sessions/{id}', 'GET|POST /api/list/items', 'PUT|DELETE /api/list/items/{id}', 'POST /api/list/items/{id}/done', 'POST /api/sync/edits'],
  p2: ['GET /', 'GET /sw.js', 'POST /api/sign-in', 'GET|POST /api/items', 'POST /api/items/{id}/edit', 'POST /api/items/{id}/complete', 'POST /api/changes', 'sync_service: GET /items, POST|PUT /items/{id}'],
  p3: ['GET /', 'GET /sw.js', 'GET|POST /api/tasks', 'POST /api/tasks/{id}/complete'],
  p4: ['GET /list/{secret}', 'GET /?secret=', 'GET /sw.js', 'GET /api/open', 'POST /api/queue', 'POST /api/send', 'GET /api/resolve', 'GET /api/tasks', 'GET /api/changes', 'POST /api/task/add|edit|done|remove', 'GET /api/task/open', 'GET /api/task/done-list'],
  p5: ['GET /', 'GET /sw.js', 'GET /api/tasks', 'POST /api/commands'],
  p6: ['GET /l/{link}', 'GET /sw.js', 'GET /api/open/{link}', 'GET /api/list/{link}', 'POST /api/list/{link}/items', 'POST /api/list/{link}/items/{id}/done', 'POST /api/list/{link}/items/{id}/delete', 'POST /api/list/{link}/changes'],
  p7: ['GET /', 'GET /sw.js', 'GET|POST /api/tasks', 'POST /api/tasks/{id}/complete', 'POST /api/tasks/{id}/reopen'],
  p8: ['list-service: GET /', 'GET /sw.js', 'GET|POST /api/tasks', 'PUT|DELETE /api/tasks/{id}', 'POST /api/tasks/{id}/edit|complete|delete', 'POST /api/push', 'sync-service (second binary): GET /, GET /sw.js, POST /api/push, GET /api/shared'],
  p9: ['GET /', 'GET /lists/{id}', 'GET /sw.js', 'GET|POST /api/lists', 'GET /api/lists/{id}', 'GET /api/lists/{id}/items', 'GET|POST /api/changes', 'GET /api/changes/{id}', 'POST /api/changes/{id}/send', 'POST /api/tasks', 'GET /api/tasks/{id}', 'POST /api/tasks/{id}/tick|remove', 'GET /api/tasks/open|done'],
};
const scrub = (s: string) => s.replace(/\/home\/[^\s"]+/g, '<path>').replace(/[0-9a-f]{32}/g, '<secret>')
  .replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g, '<uuid>').replace(/:\d{4,5}\b/g, ':<port>');
const rows = u.rows.map((r: any) => {
  const st = JSON.parse(fs.readFileSync(path.join(q, 'results', `static-${r.id}.json`), 'utf8'));
  return {
    run: r.run, side: r.side, blinded_id: r.id,
    score: r.score, n: r.n, not_testable: r.not_testable, failed: r.failed,
    own_tests_passed: r.tests.passed,
    cost_usd: cost[r.run].usd, wall_s: cost[r.run].wall_s, cost_source: cost[r.run].source,
    items: Object.fromEntries(Object.entries(r.items).map(([k, v]: any) => [k, {score: v.score, step: scrub(v.step), got: scrub(v.got)}])),
    console_errors_other_than_network: r.console_errors.filter((e: string) => !/ERR_INTERNET_DISCONNECTED|ERR_CONNECTION_REFUSED/.test(e)),
    exploratory_findings: findings.findings[r.id],
    static: {product_loc: st.product_loc, rust_loc: st.rust_loc, separate_web_loc: st.web_loc, test_fns: st.test_fns,
      dependencies: st.dependencies.length, binaries: st.binaries, service_worker: st.service_worker,
      local_storage: st.local_storage, indexed_db: st.indexed_db, routes: routes[r.id]},
  };
}).sort((a: any, b: any) => a.run.localeCompare(b.run));
fs.mkdirSync(out, {recursive: true});
fs.writeFileSync(path.join(out, 'scores.json'), JSON.stringify({
  format: 'sdc.quality-scores/1', rubric: 'rubric.md', n: 23,
  key: u.key, sides: u.sides, products: rows,
  uniform_probe_outcomes: findings.uniform, notes: findings.notes,
}, null, 2) + '\n');
console.log('wrote', rows.length);
