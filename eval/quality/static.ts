// Static facts of one blinded product, by program: lines of product code (non-blank, outside
// tests/ and outside #[cfg(test)] modules), #[test] count, dependencies, browser storage and
// service worker use, route literals. Prints JSON.
//   node static.ts <product dir>
import fs from 'node:fs';
import path from 'node:path';

const dir = process.argv[2];
function walk(d: string): string[] {
  return fs.readdirSync(d, {withFileTypes: true}).flatMap(e => {
    const p = path.join(d, e.name);
    if (e.isDirectory()) return ['target', 'data', '.home'].includes(e.name) ? [] : walk(p);
    return [p];
  });
}
// strips `#[cfg(test)] mod x { ... }` blocks by brace counting (string/char literals are rare in
// test module headers; good enough for a line count)
function stripTestModules(src: string): {code: string; testLines: number} {
  const lines = src.split('\n');
  const keep: string[] = [];
  let i = 0, testLines = 0;
  while (i < lines.length) {
    if (/^\s*#\[cfg\(test\)\]/.test(lines[i]) && i + 1 < lines.length && /^\s*(pub\s+)?mod\s+\w+\s*\{/.test(lines[i + 1])) {
      let depth = 0, j = i + 1, started = false;
      for (; j < lines.length; j++) {
        for (const ch of lines[j]) { if (ch === '{') { depth++; started = true; } else if (ch === '}') depth--; }
        if (started && depth <= 0) break;
      }
      testLines += j - i + 1;
      i = j + 1;
      continue;
    }
    keep.push(lines[i]);
    i++;
  }
  return {code: keep.join('\n'), testLines};
}
const files = walk(dir).filter(f => !/Cargo\.lock$/.test(f));
const prod: Record<string, number> = {};
let testsLoc = 0, testFns = 0, rustLoc = 0, webLoc = 0;
let all = '';
const routes = new Set<string>();
for (const f of files) {
  const rel = path.relative(dir, f);
  const src = fs.readFileSync(f, 'utf8');
  testFns += (src.match(/#\[test\]/g) || []).length;
  if (rel.startsWith('tests/')) { testsLoc += src.split('\n').filter(l => l.trim()).length; continue; }
  if (!/\.(rs|html|js|css)$/.test(rel)) continue;
  const {code, testLines} = rel.endsWith('.rs') ? stripTestModules(src) : {code: src, testLines: 0};
  testsLoc += testLines;
  const n = code.split('\n').filter(l => l.trim()).length;
  prod[rel] = n;
  if (rel.endsWith('.rs')) rustLoc += n; else webLoc += n;
  all += code + '\n';
  for (const m of code.matchAll(/"(\/(?:api\/)?[a-z][a-z0-9_\-\/.{}<>:]*)"/gi)) routes.add(m[1]);
  for (const m of code.matchAll(/\(\s*"(GET|POST|PUT|DELETE)"\s*,\s*\[([^\]]*)\]/g)) routes.add(`${m[1]} [${m[2].replace(/\s+/g, '')}]`);
}
const cargo = fs.readFileSync(path.join(dir, 'Cargo.toml'), 'utf8');
const depSection = (cargo.split(/^\[dependencies\]/m)[1] ?? '').split(/^\[/m)[0];
const deps = depSection.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'));
const bins = [...cargo.matchAll(/\[\[bin\]\][^\[]*?name\s*=\s*"([^"]+)"/g)].map(m => m[1]);
console.log(JSON.stringify({
  product_loc: rustLoc + webLoc, rust_loc: rustLoc, web_loc: webLoc, files: prod,
  test_loc: testsLoc, test_fns: testFns,
  dependencies: deps, binaries: bins.length ? bins : ['(package name)'],
  service_worker: /serviceWorker\.register/.test(all),
  local_storage: /localStorage/.test(all),
  indexed_db: /indexedDB/.test(all),
  route_literals: [...routes].filter(r => !/\.(rs|dat|txt|tsv)$/.test(r)).sort(),
}));
