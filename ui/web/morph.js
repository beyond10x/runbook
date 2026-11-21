// A small DOM morph: patch an element's children to match an HTML string, in place.
// Elements with `data-key` are matched by key (moved, not rebuilt); others by position and tag.
// New elements get the class `enter` for one animation; an element whose `data-v` changes gets
// `flash`. Focused inputs, open <details> and scroll positions survive because nothing that
// already exists is replaced.
const tpl = document.createElement('template');

export function patch(el, html) {
  if (el.__html === html) return;
  el.__html = html;
  const isSvg = el instanceof SVGElement;
  let src;
  if (isSvg) {
    tpl.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg">${html}</svg>`;
    src = tpl.content.firstChild;
  } else {
    tpl.innerHTML = html;
    src = tpl.content;
  }
  morphChildren(el, src);
}

function keyOf(n) { return n.nodeType === 1 ? n.getAttribute('data-key') : null; }

function morphChildren(to, from) {
  const oldKids = Array.from(to.childNodes);
  const keyed = new Map();
  for (const k of oldKids) { const key = keyOf(k); if (key != null) keyed.set(key, k); }
  const newKids = Array.from(from.childNodes);
  let i = 0;
  for (const nk of newKids) {
    const key = keyOf(nk);
    let cur = to.childNodes[i];
    if (key != null) {
      const match = keyed.get(key);
      if (match) {
        keyed.delete(key);
        if (match !== cur) to.insertBefore(match, cur || null);
        morphNode(match, nk);
      } else {
        const fresh = nk.cloneNode(true);
        enter(fresh);
        to.insertBefore(fresh, cur || null);
      }
    } else if (cur && keyOf(cur) == null && cur.nodeType === nk.nodeType && cur.nodeName === nk.nodeName) {
      morphNode(cur, nk);
    } else {
      const fresh = nk.cloneNode(true);
      if (fresh.nodeType === 1) enter(fresh);
      to.insertBefore(fresh, cur || null);
    }
    i++;
  }
  while (to.childNodes.length > i) to.removeChild(to.lastChild);
}

function enter(n) {
  if (n.nodeType !== 1 || n.hasAttribute('data-noenter')) return;
  n.classList.add('enter');
  setTimeout(() => n.classList.remove('enter'), 400);
}

function morphNode(a, b) {
  if (a.nodeType === 3 || a.nodeType === 8) { if (a.nodeValue !== b.nodeValue) a.nodeValue = b.nodeValue; return; }
  if (a.nodeType !== 1) return;
  const oldV = a.getAttribute('data-v');
  const hadOpen = a.tagName === 'DETAILS' && a.open;
  for (const attr of Array.from(b.attributes)) {
    if (attr.name === 'class') {
      const keep = ['enter', 'flash'].filter((c) => a.classList.contains(c));
      const want = [attr.value, ...keep].join(' ').trim();
      if (a.getAttribute('class') !== want) a.setAttribute('class', want);
      continue;
    }
    if (a.getAttribute(attr.name) !== attr.value) a.setAttribute(attr.name, attr.value);
  }
  for (const attr of Array.from(a.attributes)) {
    if (!b.hasAttribute(attr.name) && !(attr.name === 'open' && hadOpen) && attr.name !== 'class') a.removeAttribute(attr.name);
  }
  if (!b.hasAttribute('class') && a.hasAttribute('class')) a.removeAttribute('class');
  const newV = b.getAttribute('data-v');
  if (oldV != null && newV != null && oldV !== newV) {
    a.classList.remove('flash');
    void a.getBoundingClientRect();
    a.classList.add('flash');
    setTimeout(() => a.classList.remove('flash'), 1400);
  }
  if (a.tagName === 'INPUT') {
    if (document.activeElement !== a && b.getAttribute('value') != null && a.value !== b.getAttribute('value')) a.value = b.getAttribute('value');
    a.checked = b.hasAttribute('checked');
    return;
  }
  if (a.tagName === 'SELECT') { morphChildren(a, b); const s = b.querySelector('option[selected]'); if (s) a.value = s.value; return; }
  morphChildren(a, b);
}
