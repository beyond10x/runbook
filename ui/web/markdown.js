// Markdown for the page, written here, with no dependency. The text of a file of a run is hostile input (a confined
// model wrote it), so this module never makes markup of it: it builds elements with createElement and puts the text
// in text nodes. Raw HTML in the text stays text, an image is a link with its alt text (nothing is loaded), a link
// is followed only when it is an http or https address (a new tab, no opener) or a path inside the run (opened by
// the page's own panel); any other address is shown as text, with the address, so nothing is hidden.
// Supported: headings, paragraphs, lists (nested, ordered), fenced code, inline code, strong, emphasis, tables
// (with alignment), block quotes, horizontal rules, links, images (as links), hard line breaks.
//
//   renderMarkdown(text, {dir, open}, host)
//     dir   the directory of the file inside the run ('work/example'): relative links resolve against it
//     open  called with the path inside the run (relative to the run) when a link to a file of the run is clicked

// A document makes at most MAX_NODES elements: a few KB of table rows or emphasis must not make a million nodes.
// Past it the rest of the document is shown as plain text with a line that says so. The count is of the document
// being rendered (renderMarkdown resets it); a table has its own bounds below.
const MAX_NODES = 30000;
const MAX_COLS = 50, MAX_ROWS = 500, MAX_CELLS = 5000; // of one table
let made = 0;
const over = () => made > MAX_NODES;
const el = (tag, cls) => { made++; const e = document.createElement(tag); if (cls) e.className = cls; return e; };
const txt = (s) => document.createTextNode(s);

// the rest of some lines as plain text, with the line that says why
function plain(parent, lines, why) {
  const pre = el('pre', 'md-cut-text');
  pre.textContent = lines.join('\n');
  if (why) { const note = el('p', 'md-cut'); note.textContent = why; parent.append(note); }
  parent.append(pre);
}
// the same for the rest of a document: the line that says so is written once, whatever the nesting
function cut(parent, ctx, lines, why) { plain(parent, lines, ctx.noted ? '' : why); ctx.noted = true; }
const isWs = (ch) => ch.trim() === '';
const isBlank = (ch) => ch === ' ' || ch === '\t';
// the end of `s` once trailing white space is dropped, found by one scan from the end
const trimmedEnd = (s, test = isWs) => { let e = s.length; while (e > 0 && test(s[e - 1])) e--; return e; };

// ---- links -------------------------------------------------------------------------------------------
// {kind: 'external', href} | {kind: 'file', path} | {kind: 'none'}
export function linkOf(dest, dir = '') {
  const raw = String(dest ?? '').trim();
  // browsers drop tabs, newlines and control characters inside a scheme (java<TAB>script:), so the test does too
  const bare = raw.replace(/[\u0000- \u007f]/g, '');
  if (!bare) return {kind: 'none'};
  const scheme = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(bare);
  if (scheme) {
    if (!/^https?$/i.test(scheme[1])) return {kind: 'none'};
    try {
      const u = new URL(bare);
      if (u.protocol !== 'http:' && u.protocol !== 'https:') return {kind: 'none'};
      return {kind: 'external', href: bare};
    } catch { return {kind: 'none'}; }
  }
  if (bare.startsWith('//') || bare.startsWith('/') || bare.startsWith('\\')) return {kind: 'none'};
  if (raw !== bare && /[\u0000-\u001f\u007f]/.test(raw)) return {kind: 'none'};
  let p = raw.replace(/[?#].*$/, '');
  try { p = decodeURIComponent(p); } catch { return {kind: 'none'}; }
  if (/[\u0000-\u001f\u007f\\]/.test(p)) return {kind: 'none'};
  const parts = String(dir).split('/').filter(Boolean);
  for (const n of p.split('/')) {
    if (n === '' || n === '.') continue;
    if (n === '..') { if (!parts.length) return {kind: 'none'}; parts.pop(); continue; }
    parts.push(n);
  }
  if (!p || !parts.length || parts.some((n) => n.startsWith('.'))) return {kind: 'none'};
  return {kind: 'file', path: parts.join('/')};
}

// the node for a link or an image: an anchor, or text that says where it pointed
function linkNode(label, dest, ctx, image) {
  const l = linkOf(dest, ctx.dir);
  if (l.kind === 'none') {
    const t = String(dest ?? '').trim();
    const f = document.createDocumentFragment();
    if (label.hasChildNodes()) f.append(label);
    f.append(txt(`${label.hasChildNodes() ? ' ' : ''}(${t})`));
    return f;
  }
  const a = el('a', image ? 'md-image' : '');
  a.append(label.hasChildNodes() ? label : txt(String(dest)));
  if (l.kind === 'external') {
    a.setAttribute('href', l.href);
    a.setAttribute('target', '_blank');
    a.setAttribute('rel', 'noopener noreferrer');
  } else {
    a.setAttribute('href', `#file=${encodeURIComponent(l.path)}`);
    a.addEventListener('click', (e) => { e.preventDefault(); ctx.open?.(l.path); });
  }
  return a;
}

// ---- inline ------------------------------------------------------------------------------------------
// how far ahead a bracket, a parenthesis, a code span or an emphasis looks for its end: a hostile file of a
// million brackets must not make the page do a million scans of a million characters
const SPAN = 4000;
// and every step of such a look-ahead is counted for the whole file: past the budget a bracket, a star or a
// backtick that has no end in sight is text. A real file never gets near it.
const BUDGET = 3000000;
const PUNCT = /[!-/:-@[-`{-~]/;
const isWord = (c) => c != null && /[A-Za-z0-9]/.test(c);

// the index of the `]` that closes the `[` at `i`, or -1
function closeBracket(s, i, ctx) {
  let depth = 0;
  for (let k = i; k < Math.min(s.length, i + SPAN); k++) { if (++ctx.steps > BUDGET) return -1;
    const c = s[k];
    if (c === '\\') { k++; continue; }
    if (c === '`') { const m = /^`+/.exec(s.slice(k))[0]; const e = s.substring(0, i + SPAN).indexOf(m, k + m.length); if (e > 0) k = e + m.length - 1; continue; }
    if (c === '[') depth++;
    else if (c === ']' && --depth === 0) return k;
  }
  return -1;
}
// the index of the `)` that closes the `(` at `i`, or -1
function closeParen(s, i, ctx) {
  let depth = 0;
  for (let k = i; k < Math.min(s.length, i + SPAN); k++) { if (++ctx.steps > BUDGET) return -1;
    const c = s[k];
    if (c === '\\') { k++; continue; }
    if (c === '(') depth++;
    else if (c === ')' && --depth === 0) return k;
  }
  return -1;
}
const destOf = (s) => {
  let d = s.trim();
  const t = /^(.*?)[ \t]+"[^"]*"$/s.exec(d);
  if (t && !/[\t]/.test(t[1].trim())) d = t[1].trim();
  if (d.startsWith('<') && d.endsWith('>')) d = d.slice(1, -1);
  return d.replace(/\\([!-/:-@[-`{-~])/g, '$1');
};

export function inline(s, parent, ctx, inLink = false) {
  let buf = '';
  const flush = () => { if (buf) { parent.append(txt(buf)); buf = ''; } };
  let i = 0;
  while (i < s.length) {
    if (over()) { buf += s.slice(i); i = s.length; break; }
    const c = s[i];
    if (c === '\\' && i + 1 < s.length) {
      if (s[i + 1] === '\n') { flush(); parent.append(el('br')); i += 2; continue; }
      if (PUNCT.test(s[i + 1])) { buf += s[i + 1]; i += 2; continue; }
    }
    if (c === '`') {
      const run = /^`+/.exec(s.slice(i))[0];
      let end = -1;
      for (let k = i + run.length; k < Math.min(s.length, i + SPAN); k++) { if (++ctx.steps > BUDGET) break;
        if (s[k] !== '`') continue;
        const r = /^`+/.exec(s.slice(k))[0];
        if (r.length === run.length) { end = k; break; }
        k += r.length - 1;
      }
      if (end > 0) {
        flush();
        let code = s.slice(i + run.length, end).replace(/\n/g, ' ');
        if (code.length > 2 && code.startsWith(' ') && code.endsWith(' ') && code.trim()) code = code.slice(1, -1);
        const e = el('code');
        e.textContent = code;
        parent.append(e);
        i = end + run.length;
        continue;
      }
      buf += run; i += run.length; continue;
    }
    if ((c === '[' || (c === '!' && s[i + 1] === '[')) && !(inLink && c === '[')) {
      const image = c === '!';
      const open = image ? i + 1 : i;
      const close = closeBracket(s, open, ctx);
      if (close > 0 && s[close + 1] === '(') {
        const end = closeParen(s, close + 1, ctx);
        if (end > 0) {
          flush();
          const label = document.createDocumentFragment();
          inline(s.slice(open + 1, close), label, ctx, true);
          parent.append(linkNode(label, destOf(s.slice(close + 2, end)), ctx, image));
          i = end + 1;
          continue;
        }
      }
      buf += c; i++; continue;
    }
    if (c === '*' || c === '_') {
      const double = s[i + 1] === c;
      const mark = double ? c + c : c;
      const before = s[i - 1], after = s[i + mark.length];
      const canOpen = after != null && !/\s/.test(after) && (c === '*' || !isWord(before));
      if (canOpen) {
        let end = -1;
        for (let k = i + mark.length; k < Math.min(s.length, i + SPAN); k++) { if (++ctx.steps > BUDGET) break;
          if (s[k] === '\\') { k++; continue; }
          if (s[k] === '`') { const m = /^`+/.exec(s.slice(k))[0]; const e = s.substring(0, i + SPAN).indexOf(m, k + m.length); if (e > 0) k = e + m.length - 1; continue; }
          if (s.startsWith(mark, k) && !/\s/.test(s[k - 1] ?? ' ') && (c === '*' || !isWord(s[k + mark.length])) && k > i + mark.length - 1 + 0) {
            if (!double && s[k + 1] === c) { k++; continue; }
            end = k; break;
          }
        }
        if (end > i + mark.length - 1) {
          flush();
          const e = el(double ? 'strong' : 'em');
          inline(s.slice(i + mark.length, end), e, ctx, inLink);
          parent.append(e);
          i = end + mark.length;
          continue;
        }
      }
      buf += mark; i += mark.length; continue;
    }
    if (c === '\n') {
      // two spaces before a line break make a break; a single break is a space
      const keep = trimmedEnd(buf, (ch) => ch === ' ');
      const hard = buf.length - keep >= 2;
      buf = buf.slice(0, keep);
      if (hard) { flush(); parent.append(el('br')); } else buf += ' ';
      i++; continue;
    }
    buf += c; i++;
  }
  flush();
}

// ---- blocks ------------------------------------------------------------------------------------------
const FENCE = /^ {0,3}(`{3,}|~{3,})[^`]*$/;
const HR = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const QUOTE = /^ {0,3}>/;
const ITEM = /^( *)([-+*]|\d{1,9}[.)])([ \t]+|$)(.*)$/;
const blank = (l) => l.trim() === '';
const indentOf = (l) => /^ */.exec(l.replace(/\t/g, '    '))[0].length;

const cells = (line) => {
  let t = line.trim();
  if (t.startsWith('|')) t = t.slice(1);
  if (t.endsWith('|') && !t.endsWith('\\|')) t = t.slice(0, -1);
  const out = [];
  let cur = '';
  for (let k = 0; k < t.length; k++) {
    if (t[k] === '\\' && t[k + 1] === '|') { cur += '|'; k++; } else if (t[k] === '|') { out.push(cur.trim()); cur = ''; } else cur += t[k];
  }
  out.push(cur.trim());
  return out;
};
// An ATX heading `# text`, with an optional closing run of `#`, as {level, text}, or null. Found by scans along
// the line, never by a pattern that backtracks over white space: a line of 131072 spaces costs one pass.
function headingOf(l) {
  let k = 0;
  while (k < 3 && l[k] === ' ') k++;
  let n = 0;
  while (l[k + n] === '#') n++;
  if (n < 1 || n > 6) return null;
  k += n;
  if (k < l.length && !isBlank(l[k])) return null;
  while (k < l.length && isBlank(l[k])) k++;
  const end = trimmedEnd(l, isBlank);
  if (end <= k) return {level: n, text: ''};
  let e = end;
  while (e > k && l[e - 1] === '#') e--;
  if (e === k) return {level: n, text: ''};
  if (e < end && isBlank(l[e - 1])) return {level: n, text: l.slice(k, trimmedEnd(l.slice(0, e), isBlank))};
  return {level: n, text: l.slice(k, end)};
}
// a table's delimiter row: cells of `---`, `:--`, `--:` or `:-:` between pipes
function isSep(l) {
  let t = l.trim();
  if (t.startsWith('|')) t = t.slice(1);
  if (t.endsWith('|')) t = t.slice(0, -1);
  if (t === '') return false;
  return t.split('|').every((c) => /^:?-+:?$/.test(c.trim()));
}
const startsTable = (lines, i) => i + 1 < lines.length && lines[i].includes('|') && isSep(lines[i + 1]) && lines[i + 1].includes('-') && (lines[i + 1].includes('|') || cells(lines[i]).length === 1) && cells(lines[i]).length === cells(lines[i + 1]).length;
const interrupts = (lines, i) => {
  const l = lines[i];
  if (FENCE.test(l) || HR.test(l) || QUOTE.test(l)) return true;
  if (headingOf(l)) return true;
  const m = ITEM.exec(l);
  if (m && m[4] !== '' && (/^[-+*]$/.test(m[2]) || /^1[.)]$/.test(m[2]))) return true;
  return startsTable(lines, i);
};

function list(lines, i, parent, ctx, depth) {
  const first = ITEM.exec(lines[i]);
  const ordered = /^\d/.test(first[2]);
  const base = first[1].length;
  const root = el(ordered ? 'ol' : 'ul');
  if (ordered && Number(first[2].replace(/\D/g, '')) !== 1) root.setAttribute('start', String(Number(first[2].replace(/\D/g, ''))));
  while (i < lines.length) {
    if (over()) break; // the items count against the node bound; blocks() shows the rest as plain text
    const m = ITEM.exec(lines[i]);
    if (!m || m[1].length !== base && !(m[1].length <= base + 1 && m[1].length >= base) || /^\d/.test(m[2]) !== ordered) break;
    const content = m[1].length + m[2].length + (m[3].length > 4 ? 1 : Math.max(1, m[3].length));
    const sub = [m[4]];
    let loose = false;
    i++;
    while (i < lines.length) {
      const l = lines[i];
      if (blank(l)) {
        let k = i;
        while (k < lines.length && blank(lines[k])) k++;
        if (k < lines.length && indentOf(lines[k]) > base) { for (; i < k; i++) sub.push(''); loose = true; continue; }
        break;
      }
      if (indentOf(l) > base) { sub.push(l.replace(/\t/g, '    ').slice(Math.min(indentOf(l), content))); i++; continue; }
      if (ITEM.test(l) || interrupts(lines, i) || blank(sub[sub.length - 1])) break;
      sub.push(l.trim());
      i++;
    }
    const li = el('li');
    const holder = document.createDocumentFragment();
    blocks(sub, holder, ctx, depth + 1);
    if (!loose) for (const p of Array.from(holder.childNodes)) if (p.nodeName === 'P') p.replaceWith(...Array.from(p.childNodes));
    li.append(holder);
    root.append(li);
    while (i < lines.length && blank(lines[i]) && i + 1 < lines.length && ITEM.test(lines[i + 1]) && indentOf(lines[i + 1]) === base) i++;
  }
  parent.append(root);
  return i;
}

// nesting (a list in a list, a quote in a quote) deeper than this is shown as the text it is
const MAX_DEPTH = 12;
function blocks(lines, parent, ctx, depth = 0) {
  if (depth > MAX_DEPTH) { const p = el('p'); p.textContent = lines.join('\n'); parent.append(p); return; }
  let i = 0;
  while (i < lines.length) {
    const l = lines[i];
    if (blank(l)) { i++; continue; }
    if (over()) { cut(parent, ctx, lines.slice(i), 'The rest of this document is shown as plain text: it makes more than ' + MAX_NODES + ' elements.'); return; }
    let m;
    if ((m = FENCE.exec(l))) {
      const mark = m[1];
      const body = [];
      i++;
      while (i < lines.length && !(new RegExp(`^ {0,3}${mark[0]}{${mark.length},}[ \\t]*$`).test(lines[i]))) body.push(lines[i++]);
      i++;
      const pre = el('pre'), code = el('code');
      code.textContent = body.join('\n');
      pre.append(code);
      parent.append(pre);
      continue;
    }
    if (HR.test(l)) { parent.append(el('hr')); i++; continue; }
    const hd = headingOf(l);
    if (hd) {
      const h = el(`h${hd.level}`);
      inline(hd.text, h, ctx);
      parent.append(h);
      i++;
      continue;
    }
    if (QUOTE.test(l)) {
      const body = [];
      while (i < lines.length && QUOTE.test(lines[i])) body.push(lines[i++].replace(/^ {0,3}> ?/, ''));
      const q = el('blockquote');
      blocks(body, q, ctx, depth + 1);
      parent.append(q);
      continue;
    }
    if (startsTable(lines, i)) {
      const head = cells(lines[i]);
      let end = i + 2;
      while (end < lines.length && !blank(lines[end]) && lines[end].includes('|')) end++;
      if (head.length > MAX_COLS) {
        plain(parent, lines.slice(i, end), `This table is shown as plain text: it has ${head.length} columns and the page shows at most ${MAX_COLS}.`);
        i = end;
        continue;
      }
      const align = cells(lines[i + 1]).map((c) => (c.startsWith(':') && c.endsWith(':') ? 'center' : c.endsWith(':') ? 'right' : c.startsWith(':') ? 'left' : ''));
      const wrap = el('div', 'md-table'), table = el('table'), thead = el('thead'), tr = el('tr');
      head.forEach((h, k) => { const th = el('th', align[k] ? `al-${align[k]}` : ''); inline(h, th, ctx); tr.append(th); });
      thead.append(tr);
      const tbody = el('tbody');
      let r = i + 2, rows = 0, count = head.length;
      while (r < end && rows < MAX_ROWS && count + head.length <= MAX_CELLS && !over()) {
        const row = el('tr');
        const cs = cells(lines[r++]);
        head.forEach((_, k) => { const td = el('td', align[k] ? `al-${align[k]}` : ''); inline(cs[k] ?? '', td, ctx); row.append(td); });
        tbody.append(row);
        rows++; count += head.length;
      }
      table.append(thead, tbody);
      wrap.append(table);
      parent.append(wrap);
      if (r < end) plain(parent, lines.slice(r, end), `The rest of this table is shown as plain text: the page shows at most ${MAX_ROWS} rows and ${MAX_CELLS} cells of a table.`);
      i = end;
      continue;
    }
    if (ITEM.test(l) && ITEM.exec(l)[4] !== '' || (ITEM.test(l) && /^[-+*]$/.test(ITEM.exec(l)[2]))) { i = list(lines, i, parent, ctx, depth); continue; }
    const body = [l];
    i++;
    while (i < lines.length && !blank(lines[i]) && !interrupts(lines, i)) body.push(lines[i++]);
    const p = el('p');
    inline(body.map((x, k) => { const e = trimmedEnd(x); return k < body.length - 1 && x.length - e >= 2 ? `${x.slice(0, e)}  ` : x.slice(0, e); }).map((x) => x.replace(/^\s+/, '')).join('\n'), p, ctx);
    parent.append(p);
  }
}

export function renderMarkdown(text, {dir = '', open = null} = {}, host) {
  host.replaceChildren();
  made = 0;
  const lines = String(text).replace(/\r\n?/g, '\n').split('\n');
  blocks(lines, host, {dir, open, steps: 0});
  return host;
}
