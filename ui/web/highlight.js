// A small line-oriented syntax highlighter for diffs: Rust, TS/JS, YAML, JSON, Markdown, shell,
// TOML. Each language is a list of sticky regexes tried in order at every position; the first that
// matches gives the token class. Block comments, Markdown fences and multi-line strings carry
// state from one line to the next (`state`, one per side of a diff).
const esc = (s) => s.replace(/[&<>"]/g, (ch) => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'}[ch]));
const kw = (words) => new RegExp(`\\b(?:${words.split(' ').join('|')})\\b`);

const NUM = /-?\b(?:0x[0-9a-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?)(?:[iu](?:8|16|32|64|128|size)|f32|f64)?\b/;
const LANGS = {
  rust: {block: ['/*', '*/'], rules: [
    [/\/\/.*/, 'c'], [/b?r(#*)"[\s\S]*?"\1/, 's'], [/b?"(?:[^"\\]|\\.)*"/, 's'], [/b?'(?:[^'\\]|\\.)'/, 's'], [/'[a-z_]\w*\b/, 'l'],
    [/#!?\[[^\]]*\]/, 'm'], [NUM, 'n'],
    [kw('as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while'), 'k'],
    [/\b[a-z_]\w*!/, 'f'], [/\b[A-Z]\w*\b/, 't'], [/\b[a-z_]\w*(?=\s*(?:::<[^>]*>)?\()/, 'f'], [/[{}()[\];,.:]+/, 'p'], [/[-+*/%=&|<>!^?]+/, 'o']]},
  js: {block: ['/*', '*/'], strOpen: '`', rules: [
    [/\/\/.*/, 'c'], [/"(?:[^"\\]|\\.)*"/, 's'], [/'(?:[^'\\]|\\.)*'/, 's'], [/`(?:[^`\\]|\\.)*`/, 's'], [NUM, 'n'],
    [kw('as async await break case catch class const continue debugger default delete do else enum export extends false finally for from function get if implements import in instanceof interface let new null of private protected public readonly return set static super switch this throw true try type typeof undefined var void while with yield'), 'k'],
    [/\b[A-Z]\w*\b/, 't'], [/[A-Za-z_$][\w$]*(?=\s*\()/, 'f'], [/[{}()[\];,.:]+/, 'p'], [/[-+*/%=&|<>!^?~]+/, 'o']]},
  json: {rules: [[/"(?:[^"\\]|\\.)*"(?=\s*:)/, 'y'], [/"(?:[^"\\]|\\.)*"/, 's'], [NUM, 'n'], [/\b(?:true|false|null)\b/, 'k'], [/[{}[\],:]/, 'p']]},
  yaml: {rules: [
    [/#.*/, 'c'], [/^\s*(?:-\s+)?[\w.\-/"' ]+?(?=:(\s|$))/, 'y'], [/^\s*-(?=\s|$)/, 'p'], [/"(?:[^"\\]|\\.)*"/, 's'], [/'(?:[^']|'')*'/, 's'],
    [/&\w+|\*\w+|!\w+/, 'm'], [/\b(?:true|false|null|yes|no|~)\b(?=\s*(?:#|$))/, 'k'], [/-?\b\d+(?:\.\d+)?\b(?=\s*(?:#|$))/, 'n'], [/^---$|^\.\.\.$/, 'm'], [/[|>][-+]?(?=\s*$)/, 'o'], [/[:{}[\],]/, 'p']]},
  toml: {rules: [
    [/#.*/, 'c'], [/^\s*\[\[?[^\]]+\]\]?/, 't'], [/^\s*[\w.\-"]+(?=\s*=)/, 'y'], [/"""[\s\S]*?"""|"(?:[^"\\]|\\.)*"/, 's'], [/'(?:[^'])*'/, 's'],
    [/\b(?:true|false)\b/, 'k'], [/\d{4}-\d\d-\d\d[T ][\d:.]+Z?/, 'n'], [NUM, 'n'], [/[=[\]{},]/, 'p']]},
  sh: {rules: [
    [/(^|\s)#.*/, 'c'], [/"(?:[^"\\]|\\.)*"/, 's'], [/'[^']*'/, 's'], [/\$\{[^}]*\}|\$\w+|\$[@*#?$!0-9]/, 'v'],
    [kw('if then else elif fi for while until do done case esac in function return local export readonly declare set unset shift exit break continue'), 'k'],
    [/^\s*[\w.\-/]+(?=\s|$)/, 'f'], [/\s--?[\w-]+/, 'm'], [NUM, 'n'], [/[|&;<>()]+/, 'o']]},
  md: {rules: [
    [/^#{1,6}\s.*/, 'h'], [/^\s*(?:[-*+]|\d+\.)\s/, 'p'], [/^>.*/, 'c'], [/`[^`]+`/, 's'], [/\*\*[^*]+\*\*|__[^_]+__/, 'b'],
    [/\*[^*\s][^*]*\*|_[^_\s][^_]*_/, 'i'], [/!?\[[^\]]*\]\([^)]*\)/, 'a'], [/<https?:[^>]+>|https?:\/\/\S+/, 'a'], [/^\s*\|.*\|\s*$/, 'p'], [/^(?:---|\*\*\*)\s*$/, 'm']]},
};
const sticky = new Map();
for (const [k, L] of Object.entries(LANGS)) sticky.set(k, L.rules.map(([re, cls]) => [new RegExp(re.source, 'y' + (re.flags.includes('m') ? 'm' : '')), cls]));

export function langOf(path) {
  const p = (path || '').toLowerCase();
  const base = p.split('/').pop();
  if (/\.rs$/.test(p)) return 'rust';
  if (/\.(m?[jt]sx?|cjs|cts|mts)$/.test(p)) return 'js';
  if (/\.json(l)?$/.test(p)) return 'json';
  if (/\.ya?ml$/.test(p)) return 'yaml';
  if (/\.toml$/.test(p) || base === 'cargo.lock') return 'toml';
  if (/\.(sh|bash|zsh)$/.test(p) || base === '.envrc') return 'sh';
  if (/\.(md|markdown)$/.test(p)) return 'md';
  return null;
}
export const LANG_LABEL = {rust: 'Rust', js: 'TypeScript / JavaScript', json: 'JSON', yaml: 'YAML', toml: 'TOML', sh: 'Shell', md: 'Markdown'};

// One line to HTML. `state` is {block: bool, fence: lang|null} and is updated.
export function highlightLine(line, lang, state = {}) {
  if (!lang) return esc(line);
  if (lang === 'md') {
    const f = /^\s*(```|~~~)\s*([\w+-]*)/.exec(line);
    if (f) { state.fence = state.fence ? null : (langOf('x.' + (f[2] || '')) || 'plain'); return `<span class="hl-m">${esc(line)}</span>`; }
    if (state.fence) return state.fence === 'plain' ? `<span class="hl-s">${esc(line)}</span>` : highlightLine(line, state.fence, state.inner ||= {});
  }
  const L = LANGS[lang];
  const rules = sticky.get(lang);
  let out = '', plain = '', i = 0;
  const flush = () => { if (plain) { out += esc(plain); plain = ''; } };
  const tok = (text, cls) => { flush(); out += `<span class="hl-${cls}">${esc(text)}</span>`; };
  if (state.block && L.block) {
    const end = line.indexOf(L.block[1]);
    if (end < 0) return `<span class="hl-c">${esc(line)}</span>`;
    tok(line.slice(0, end + L.block[1].length), 'c');
    i = end + L.block[1].length;
    state.block = false;
  }
  while (i < line.length) {
    if (L.block && line.startsWith(L.block[0], i)) {
      const end = line.indexOf(L.block[1], i + L.block[0].length);
      if (end < 0) { tok(line.slice(i), 'c'); state.block = true; break; }
      tok(line.slice(i, end + L.block[1].length), 'c');
      i = end + L.block[1].length;
      continue;
    }
    let hit = false;
    for (const [re, cls] of rules) {
      re.lastIndex = i;
      const m = re.exec(line);
      if (m && m[0].length) {
        // a word rule must not start inside a word
        if (/^\w/.test(m[0]) && i > 0 && /\w/.test(line[i - 1]) && cls !== 'c') continue;
        if (cls === 'p' || cls === 'o') { flush(); out += `<span class="hl-${cls}">${esc(m[0])}</span>`; } else tok(m[0], cls);
        i += m[0].length;
        hit = true;
        break;
      }
    }
    if (!hit) { plain += line[i]; i++; }
  }
  flush();
  return out;
}
