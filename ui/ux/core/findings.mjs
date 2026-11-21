// Finding records (SKILL § 5): ux/findings/UXF-nnn.md with a header of `key: value` lines up to the
// first blank line. A closed S1 or S2 record needs a regression asset that exists. Generic.
import fs from 'node:fs';
import path from 'node:path';

export const FIELDS = ['id', 'title', 'class', 'severity', 'metric', 'status', 'found_by', 'evidence_recording', 'evidence_screenshot', 'evidence_lines', 'regression_asset'];
export function readFindings(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).filter((n) => /^UXF-\d+\.md$/.test(n)).sort().map((n) => {
    const text = fs.readFileSync(path.join(dir, n), 'utf8');
    const head = {};
    for (const l of text.split('\n')) { if (!l.trim()) break; const m = /^([a-z_]+):\s*(.*)$/.exec(l); if (m) head[m[1]] = m[2].trim(); }
    return {file: n, ...head};
  });
}
// Problems with the records: missing header fields, a closed S1/S2 without an asset, an asset that
// names a file that does not exist (`path` or `path:line` or `path › test name`).
export function problems(dir, root) {
  const out = [];
  for (const f of readFindings(dir)) {
    for (const k of FIELDS) if (!(k in f)) out.push(`${f.file}: no ${k}`);
    if (/^(closed|fixed)$/.test(f.status || '') && /^S[12]$/.test(f.severity || '')) {
      const a = f.regression_asset || '';
      if (!a || a === '-' || a === 'none') { out.push(`${f.file}: closed ${f.severity} without a regression asset`); continue; }
      const file = a.split(/[:›]/)[0].trim();
      if (!fs.existsSync(path.resolve(root, file))) out.push(`${f.file}: regression asset ${file} does not exist`);
    }
  }
  return out;
}
