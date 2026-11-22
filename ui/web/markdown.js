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

const el = (tag, cls) => { const e = document.createElement(tag); if (cls) e.className = cls; return e; };
const txt = (s) => document.createTextNode(s);

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
      const hard = / {2,}$/.test(buf);
      buf = buf.replace(/ +$/, '');
      if (hard) { flush(); parent.append(el('br')); } else buf += ' ';
      i++; continue;
    }
    buf += c; i++;
  }
  flush();
}

// ---- blocks ------------------------------------------------------------------------------------------
const FENCE = /^ {0,3}(`{3,}|~{3,})[ \t]*([^`]*)$/;
const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const HR = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const QUOTE = /^ {0,3}>/;
const ITEM = /^( *)([-+*]|\d{1,9}[.)])([ \t]+|$)(.*)$/;
const SEP = /^[ \t]*\|?[ \t]*:?-+:?[ \t]*(\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;
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
const startsTable = (lines, i) => i + 1 < lines.length && lines[i].includes('|') && SEP.test(lines[i + 1]) && lines[i + 1].includes('-') && (lines[i + 1].includes('|') || cells(lines[i]).length === 1) && cells(lines[i]).length === cells(lines[i + 1]).length;
const interrupts = (lines, i) => {
  const l = lines[i];
  if (FENCE.test(l) || HR.test(l) || QUOTE.test(l)) return true;
  const h = HEADING.exec(l);
  if (h) return true;
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
    if ((m = HEADING.exec(l))) {
      const h = el(`h${m[1].length}`);
      inline(m[2] ?? '', h, ctx);
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
      const align = cells(lines[i + 1]).map((c) => (c.startsWith(':') && c.endsWith(':') ? 'center' : c.endsWith(':') ? 'right' : c.startsWith(':') ? 'left' : ''));
      i += 2;
      const wrap = el('div', 'md-table'), table = el('table'), thead = el('thead'), tr = el('tr');
      head.forEach((h, k) => { const th = el('th', align[k] ? `al-${align[k]}` : ''); inline(h, th, ctx); tr.append(th); });
      thead.append(tr);
      const tbody = el('tbody');
      while (i < lines.length && !blank(lines[i]) && lines[i].includes('|')) {
        const row = el('tr');
        const cs = cells(lines[i++]);
        head.forEach((_, k) => { const td = el('td', align[k] ? `al-${align[k]}` : ''); inline(cs[k] ?? '', td, ctx); row.append(td); });
        tbody.append(row);
      }
      table.append(thead, tbody);
      wrap.append(table);
      parent.append(wrap);
      continue;
    }
    if (ITEM.test(l) && ITEM.exec(l)[4] !== '' || (ITEM.test(l) && /^[-+*]$/.test(ITEM.exec(l)[2]))) { i = list(lines, i, parent, ctx, depth); continue; }
    const body = [l];
    i++;
    while (i < lines.length && !blank(lines[i]) && !interrupts(lines, i)) body.push(lines[i++]);
    const p = el('p');
    inline(body.map((x, k) => (k < body.length - 1 ? x.replace(/\s+$/, (sp) => (sp.length >= 2 ? '  ' : '')) : x.replace(/\s+$/, ''))).map((x) => x.replace(/^\s+/, '')).join('\n'), p, ctx);
    parent.append(p);
  }
}

export function renderMarkdown(text, {dir = '', open = null} = {}, host) {
  host.replaceChildren();
  const lines = String(text).replace(/\r\n?/g, '\n').split('\n');
  blocks(lines, host, {dir, open, steps: 0});
  return host;
}
