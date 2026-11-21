// Playwright global setup: the metric files start empty on every run.
import fs from 'node:fs';
import path from 'node:path';

export default function setup() {
  const out = path.resolve(process.env.UX_OUT || path.join(path.dirname(new URL(import.meta.url).pathname), '..', 'out'));
  if (!fs.existsSync(out)) return;
  for (const f of fs.readdirSync(out)) if (/^(metrics|phone-facts)-.*\.jsonl$/.test(f)) fs.rmSync(path.join(out, f));
}
