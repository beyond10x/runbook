// Writes scores.json (blinded ids) from results/; with --unblind also reads key.json and the
// run costs and writes the per-side table. No product source text, no path outside the results.
//   node summary.ts <quality dir> [--unblind]
import fs from 'node:fs';
import path from 'node:path';

const q = process.argv[2];
const unblind = process.argv.includes('--unblind');
const R = path.join(q, 'results');
const ids = ['p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7', 'p8', 'p9'];
const ITEMS = ['B1', 'B2', 'B3', 'U1', 'F1', 'F2', 'F3', 'F4', 'F5', 'P1', 'S1', 'S2', 'S3', 'O1', 'O2', 'O3', 'O4', 'O5', 'O6', 'O7', 'R1', 'R2', 'R3'];
const scrub = (s: string) => s.split(q).join('<quality>').replace(/\/home\/[^\/\s]+/g, '~');
const rows: any[] = [];
for (const id of ids) {
  const p = path.join(R, `${id}.json`);
  if (!fs.existsSync(p)) { console.log(id, 'not scored yet'); continue; }
  const r = JSON.parse(fs.readFileSync(p, 'utf8'));
  const st = JSON.parse(fs.readFileSync(path.join(R, `static-${id}.json`), 'utf8'));
  const ex = fs.existsSync(path.join(R, `explore-${id}.json`)) ? JSON.parse(fs.readFileSync(path.join(R, `explore-${id}.json`), 'utf8')) : null;
  for (const k of ITEMS) { r.items[k].got = scrub(r.items[k].got); r.items[k].step = scrub(r.items[k].step); }
  rows.push({id, score: r.score, n: r.n, not_testable: r.not_testable, failed: r.failed, tests: r.tests, items: r.items,
    static: {product_loc: st.product_loc, test_fns: st.test_fns, dependencies: st.dependencies.length, binaries: st.binaries,
      service_worker: st.service_worker, local_storage: st.local_storage, indexed_db: st.indexed_db},
    console_errors: r.console_errors.map(scrub), explored: !!ex});
}
fs.writeFileSync(path.join(q, 'scores.json'), JSON.stringify(rows, null, 2) + '\n');
for (const r of rows) console.log(r.id, `${r.score}/${r.n}`, 'nt', r.not_testable, 'failed', r.failed.join(','), 'tests', r.tests.passed, 'loc', r.static.product_loc);

if (unblind) {
  const key = JSON.parse(fs.readFileSync(path.join(q, 'key.json'), 'utf8'));
  const side = (run: string) => run.startsWith('free-agent') ? 'free agent' : run.includes('strong') ? 'SDC strong' : 'SDC mixed';
  const by: Record<string, any[]> = {};
  for (const r of rows) { r.run = key[r.id]; r.side = side(r.run); (by[r.side] ??= []).push(r); }
  const med = (xs: number[]) => { const s = [...xs].sort((a, b) => a - b); return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2; };
  const out: any = {};
  for (const [s, rs] of Object.entries(by)) {
    const sc = rs.map(r => r.score), loc = rs.map(r => r.static.product_loc), t = rs.map(r => r.tests.passed);
    out[s] = {runs: rs.map(r => `${r.run}=${r.id}`), score: {median: med(sc), min: Math.min(...sc), max: Math.max(...sc)},
      loc: {median: med(loc), min: Math.min(...loc), max: Math.max(...loc)}, tests: {median: med(t), min: Math.min(...t), max: Math.max(...t)},
      per_item_passes: Object.fromEntries(ITEMS.map(k => [k, rs.filter(r => r.items[k].score === 'pass').length]))};
  }
  fs.writeFileSync(path.join(q, 'unblinded.json'), JSON.stringify({key, sides: out, rows}, null, 2) + '\n');
  console.log(JSON.stringify(out, null, 1));
}
