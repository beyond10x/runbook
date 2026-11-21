// Metric records: one JSON line per value, {scenario, viewport, key, value}, appended by the specs to
// <out>/metrics-<viewport>.jsonl and folded here. Generic: no project code.
import fs from 'node:fs';
import path from 'node:path';

export const median = (xs) => { const s = xs.slice().sort((a, b) => a - b); return s.length ? (s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2) : 0; };
export const p95 = (xs) => { const s = xs.slice().sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.ceil(0.95 * s.length) - 1)] : 0; };
export const max = (xs) => (xs.length ? Math.max(...xs) : 0);

export function record(out, viewport, scenario, key, value) {
  fs.mkdirSync(out, {recursive: true});
  fs.appendFileSync(path.join(out, `metrics-${viewport}.jsonl`), JSON.stringify({scenario, viewport, key, value: Math.round(value * 1000) / 1000}) + '\n');
}
export function reset(out, viewport) { fs.rmSync(path.join(out, `metrics-${viewport}.jsonl`), {force: true}); }

// {scenario: {"key@viewport": value}}; a key recorded twice keeps the worse (larger) value
export function fold(out) {
  const res = {};
  if (!fs.existsSync(out)) return res;
  for (const f of fs.readdirSync(out).filter((n) => /^metrics-.*\.jsonl$/.test(n)).sort()) {
    for (const l of fs.readFileSync(path.join(out, f), 'utf8').split('\n')) {
      if (!l.trim()) continue;
      const r = JSON.parse(l);
      const k = `${r.key}@${r.viewport}`;
      const s = (res[r.scenario] ||= {});
      s[k] = s[k] == null ? r.value : Math.max(s[k], r.value);
    }
  }
  return res;
}
