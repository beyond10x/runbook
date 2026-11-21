#!/usr/bin/env node
// The round command (SKILL § 3.2): runs the loop at every viewport (gating and measured-only) with
// screenshots, then writes ux/rounds/<R-nn>/report.md, round.json, contact.html and ../trend.md.
//   node ux/core/round.mjs R-01 [--config ux/playwright.config.mjs] [--no-run]
// Deterministic over its inputs; generic: no project code (the adapter comes in through the config).
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fold} from './metrics.mjs';

const [round, ...rest] = process.argv.slice(2);
if (!/^R-\d\d$/.test(round || '')) { console.error('usage: round.mjs R-nn [--config FILE] [--no-run]'); process.exit(2); }
const opt = (k, d) => { const i = rest.indexOf(k); return i >= 0 ? rest[i + 1] : d; };
const config = path.resolve(opt('--config', 'ux/playwright.config.mjs'));
// the gating viewports: their hard-gate failures are the gate result; the rest is measured only
const gatingVps = (opt('--gating', process.env.UX_GATING || '') || '').split(',').filter(Boolean);
const isGating = (v) => !gatingVps.length || gatingVps.includes(v);
const ux = path.dirname(config);
const dir = path.join(ux, 'rounds', round);
const out = path.join(ux, 'out');
fs.mkdirSync(path.join(dir, 'shots'), {recursive: true});
// every file this writes names paths relative to the repository (an absolute home path never lands
// in a committed round file): the repository root, then the home directory, then any other user's home
const top = spawnSync('git', ['rev-parse', '--show-toplevel'], {cwd: ux, encoding: 'utf8'}).stdout?.trim() || process.cwd();
const home = process.env.HOME || '/nonexistent';
const repoRel = (s) => s.split(top + '/').join('').split(home + '/').join('~/').replace(/\/home\/[^/\s"'<>\\]+/g, '~');
const write = (file, text) => fs.writeFileSync(file, repoRel(text));

let exit = 0;
if (!rest.includes('--no-run')) {
  for (const n of fs.readdirSync(path.join(dir, 'shots'))) if (!/^(review|decision|before)-/.test(n)) fs.rmSync(path.join(dir, 'shots', n));
  const r = spawnSync('slot', ['npx', 'playwright', 'test', '-c', config, '--workers=2', '--reporter=line,json'], {stdio: ['ignore', 'inherit', 'inherit'],
    env: {...process.env, UX_ROUND: round, PLAYWRIGHT_JSON_OUTPUT_NAME: path.join(dir, 'playwright.json')}});
  exit = r.status ?? 1;
}
const metrics = fold(out);
const baseline = JSON.parse(fs.readFileSync(path.join(ux, 'baseline.json'), 'utf8'));
const prevRound = fs.readdirSync(path.join(ux, 'rounds')).filter((n) => /^R-\d\d$/.test(n) && n < round).sort().pop();
const prev = prevRound ? JSON.parse(fs.readFileSync(path.join(ux, 'rounds', prevRound, 'round.json'), 'utf8')) : null;

// gate results from the Playwright JSON report: every failed expectation, by test and viewport
const failures = [];
const pj = path.join(dir, 'playwright.json');
if (fs.existsSync(pj)) {
  const rep = JSON.parse(fs.readFileSync(pj, 'utf8'));
  const visit = (suite, titles) => {
    for (const s of suite.suites || []) visit(s, [...titles, s.title]);
    for (const sp of suite.specs || []) for (const t of sp.tests || []) for (const r of t.results || []) {
      for (const e of r.errors || []) failures.push({test: sp.title, viewport: t.projectName, message: String(e.message || '').replace(/\u001b\[[0-9;]*m/g, '').split('\n').slice(0, 16).join(' ').replace(/\s+/g, ' ').slice(0, 900)});
    }
  };
  for (const s of rep.suites || []) visit(s, []);
}
const viewports = [...new Set(Object.values(metrics).flatMap((k) => Object.keys(k).map((x) => x.split('@')[1])))].sort();
const keys = [...new Set(Object.values(metrics).flatMap((k) => Object.keys(k).map((x) => x.split('@')[0])))].sort();
const round3 = (v) => (v == null ? '—' : Math.round(v * 100) / 100);
for (const f of failures) { f.gating = isGating(f.viewport); f.gate = /hard gate \d/.test(f.message) ? Number(/hard gate (\d)/.exec(f.message)[1]) : null; }
const gateFails = failures.filter((f) => f.gating && f.gate), taskFails = failures.filter((f) => f.gating && !f.gate), measuredFails = failures.filter((f) => !f.gating);
const json = {round, at: new Date().toISOString(), exit, viewports, metrics, failures, baseline: baseline.scenarios, previous: prevRound || null};
write(path.join(dir, 'round.json'), JSON.stringify(json, null, 2) + '\n');

const shots = fs.readdirSync(path.join(dir, 'shots')).filter((n) => n.endsWith('.png')).sort();
let md = `# ${round} report\n\nWritten by \`ux/core/round.mjs\` at ${json.at}. Suite exit ${exit}. Viewports: ${viewports.join(', ')}.\n\n`;
const line = (f) => `- **${f.viewport}** · ${f.test}: ${f.message}`;
md += `## Hard gates at the gating viewports (${gatingVps.join(', ') || 'all'})\n\n${gateFails.length ? gateFails.map(line).join('\n') : 'Every hard gate passed.'}\n\n`;
md += `## Screen-only user tasks at the gating viewports\n\n${taskFails.length ? taskFails.map(line).join('\n') : 'Every task completed.'}\n\n`;
md += `## Measured-only viewports (not gating)\n\n${measuredFails.length ? measuredFails.map(line).join('\n') : 'No failed check.'}\n\n`;
md += `## Metrics (lower is better)\n\nEach cell: this round / baseline / ${prevRound || 'no previous round'}.\n\n`;
for (const [sc, vals] of Object.entries(metrics)) {
  md += `### ${sc}\n\n| metric | ${viewports.join(' | ')} |\n|---|${viewports.map(() => '---').join('|')}|\n`;
  for (const k of keys) {
    if (!viewports.some((v) => vals[`${k}@${v}`] != null)) continue;
    md += `| ${k} | ${viewports.map((v) => { const x = vals[`${k}@${v}`]; const b = baseline.scenarios?.[sc]?.[`${k}@${v}`]?.value; const p = prev?.metrics?.[sc]?.[`${k}@${v}`]; return x == null ? '—' : `${round3(x)} / ${round3(b)} / ${round3(p)}`; }).join(' | ')} |\n`;
  }
  md += '\n';
}
// the worst moments: per gate metric, the scenario and viewport with the largest value
const worst = [];
for (const k of ['contract.scroll', 'contract.hidden', 'contract.axe_serious', 'contract.truncated_hidden', 'layout_shift.per_minute', 'reading_load.max', 'glance_cost.p95', 'screen_only.actions']) {
  let best = null;
  for (const [sc, vals] of Object.entries(metrics)) for (const v of viewports) { const x = vals[`${k}@${v}`]; if (x != null && (!best || x > best.x)) best = {sc, v, x}; }
  if (best && best.x > 0) worst.push(`- ${k}: ${round3(best.x)} in **${best.sc}** at ${best.v}${shots.includes(`${best.v}-${best.sc}-0.png`) ? ` · ![](shots/${best.v}-${best.sc}-0.png)` : ''}`);
}
md += `## Worst moments\n\n${worst.join('\n') || 'none'}\n\n`;
md += `## Missing inputs\n\n- No recordings (rrweb is not set up; the driver bot plays each run, and the screenshots stand in for the contact sheet: \`contact.html\`).\n- No questionnaire, no L4 keyed replay, no L5 probe, no L6 live round, no L8 sessions: skipped for this loop (paid or people).\n\n`;
md += `## Screenshots\n\n${shots.map((s) => `- shots/${s}`).join('\n')}\n`;
write(path.join(dir, 'report.md'), md);
write(path.join(dir, 'contact.html'), `<!doctype html><meta charset="utf-8"><title>${round} contact sheet</title><style>body{background:#0a0b0f;color:#ccc;font:12px system-ui;display:grid;grid-template-columns:repeat(auto-fill,minmax(420px,1fr));gap:12px;padding:12px}figure{margin:0}img{width:100%;border:1px solid #333}</style>${shots.map((s) => `<figure><img src="shots/${s}" loading="lazy"><figcaption>${s}</figcaption></figure>`).join('')}`);
const trend = path.join(ux, 'rounds', 'trend.md');
if (!fs.existsSync(trend)) write(trend, '# Trend\n\n| round | date | suite exit | gate failures | Σ screen_only.actions | Σ screen_only.looks | Σ contract.truncated_hidden | max layout_shift.per_minute |\n|---|---|---|---|---|---|---|---|\n');
const g = ([x]) => isGating(x.split('@')[1]);
const sum = (k) => round3(Object.values(metrics).reduce((n, vals) => n + Object.entries(vals).filter(([x]) => x.startsWith(k + '@')).filter(g).reduce((a, [, v]) => a + v, 0), 0));
const mx = (k) => round3(Math.max(0, ...Object.values(metrics).flatMap((vals) => Object.entries(vals).filter(([x]) => x.startsWith(k + '@')).filter(g).map(([, v]) => v))));
const row = `| ${round} | ${json.at.slice(0, 16)} | ${exit} | ${gateFails.length} gate, ${taskFails.length} task | ${sum('screen_only.actions')} | ${sum('screen_only.looks')} | ${sum('contract.truncated_hidden')} | ${mx('layout_shift.per_minute')} |\n`;
const t = fs.readFileSync(trend, 'utf8').split('\n').filter((l) => !l.startsWith(`| ${round} |`)).join('\n');
write(trend, t.trimEnd() + '\n' + row);
console.log(`${dir}/report.md · ${gateFails.length} hard-gate and ${taskFails.length} task failures at the gating viewports · exit ${exit}`);
