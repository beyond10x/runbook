// A unified diff of two texts (3 lines of context), for a change record whose recorder left `diff`
// null, and for the test fixture. Line-based LCS over the region between the common prefix and
// suffix; large regions fall back to "all removed, all added".
export function unifiedDiff(before, after, pathA = 'a', pathB = 'b', context = 3) {
  const a = before == null ? [] : splitLines(before);
  const b = after == null ? [] : splitLines(after);
  const ops = lineOps(a, b);
  if (!ops.some((o) => o.t !== ' ')) return '';
  const out = [`--- ${before == null ? '/dev/null' : 'a/' + pathA}`, `+++ ${after == null ? '/dev/null' : 'b/' + pathB}`];
  let i = 0;
  while (i < ops.length) {
    while (i < ops.length && ops[i].t === ' ') i++;
    if (i >= ops.length) break;
    let start = Math.max(0, i - context);
    let end = i;
    for (;;) {
      while (end < ops.length && ops[end].t !== ' ') end++;
      let k = end;
      while (k < ops.length && ops[k].t === ' ' && k - end < 2 * context) k++;
      if (k < ops.length && ops[k].t !== ' ') { end = k; continue; }
      end = Math.min(ops.length, end + context);
      break;
    }
    const h = ops.slice(start, end);
    const aStart = h.find((o) => o.a != null)?.a, bStart = h.find((o) => o.b != null)?.b;
    const aLen = h.filter((o) => o.t !== '+').length, bLen = h.filter((o) => o.t !== '-').length;
    out.push(`@@ -${aLen ? aStart + 1 : (aStart ?? 0)},${aLen} +${bLen ? bStart + 1 : (bStart ?? 0)},${bLen} @@`);
    for (const o of h) out.push(o.t + o.line);
    i = end;
  }
  return out.join('\n') + '\n';
}

export function diffCounts(diff) {
  let add = 0, del = 0;
  for (const l of (diff || '').split('\n')) {
    if (l.startsWith('+++') || l.startsWith('---')) continue;
    if (l[0] === '+') add++; else if (l[0] === '-') del++;
  }
  return {add, del};
}

function splitLines(t) { const l = t.split('\n'); if (l[l.length - 1] === '') l.pop(); return l; }

function lineOps(a, b) {
  let p = 0;
  while (p < a.length && p < b.length && a[p] === b[p]) p++;
  let s = 0;
  while (s < a.length - p && s < b.length - p && a[a.length - 1 - s] === b[b.length - 1 - s]) s++;
  const ops = [];
  for (let i = 0; i < p; i++) ops.push({t: ' ', line: a[i], a: i, b: i});
  const am = a.slice(p, a.length - s), bm = b.slice(p, b.length - s);
  if (am.length * bm.length > 4e6) {
    am.forEach((line, i) => ops.push({t: '-', line, a: p + i}));
    bm.forEach((line, i) => ops.push({t: '+', line, b: p + i}));
  } else {
    const n = am.length, m = bm.length;
    const L = Array.from({length: n + 1}, () => new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = am[i] === bm[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
    let i = 0, j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && am[i] === bm[j]) { ops.push({t: ' ', line: am[i], a: p + i, b: p + j}); i++; j++; }
      else if (j < m && (i >= n || L[i][j + 1] >= L[i + 1][j])) { ops.push({t: '+', line: bm[j], b: p + j}); j++; }
      else { ops.push({t: '-', line: am[i], a: p + i}); i++; }
    }
  }
  for (let k = 0; k < s; k++) ops.push({t: ' ', line: a[a.length - s + k], a: a.length - s + k, b: b.length - s + k});
  return ops;
}
