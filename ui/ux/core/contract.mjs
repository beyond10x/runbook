// The layout contract and the screen-only user's eyes: generic checks over any page, no project code.
// Every function takes a Playwright page and plain data from the adapter.
import AxeBuilder from '@axe-core/playwright';

// Hard gate 1. The document never scrolls; an element may scroll only when the adapter allows it
// (a decision in the baseline). Returns the offenders.
export async function scrollOffenders(page, allowed = []) {
  return page.evaluate((allowed) => {
    const out = [];
    const de = document.scrollingElement || document.documentElement;
    if (de.scrollHeight > window.innerHeight + 1 || de.scrollWidth > window.innerWidth + 1) out.push({sel: 'document', h: de.scrollHeight, w: de.scrollWidth});
    const name = (el) => el.id ? `#${el.id}` : `${el.tagName.toLowerCase()}${el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).slice(0, 2).join('.') : ''}`;
    for (const el of document.querySelectorAll('body *')) {
      const cs = getComputedStyle(el);
      const sy = /(auto|scroll)/.test(cs.overflowY) && el.scrollHeight > el.clientHeight + 1;
      const sx = /(auto|scroll)/.test(cs.overflowX) && el.scrollWidth > el.clientWidth + 1;
      if (!sy && !sx) continue;
      if (el.closest('dialog:not([open])') || el.offsetParent === null) continue;
      if (allowed.some((s) => el.matches(s))) continue;
      out.push({sel: name(el), axis: sy && sx ? 'xy' : sy ? 'y' : 'x'});
    }
    return out;
  }, allowed);
}

// Hard gate 2. Each selector's element is fully inside the viewport and not covered at its centre.
export async function visibilityMisses(page, selectors) {
  return page.evaluate((selectors) => {
    const out = [];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (!el) { out.push({sel, why: 'missing'}); continue; }
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) { out.push({sel, why: 'empty'}); continue; }
      if (r.left < -0.5 || r.top < -0.5 || r.right > innerWidth + 0.5 || r.bottom > innerHeight + 0.5) { out.push({sel, why: `outside the viewport (${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.right)},${Math.round(r.bottom)})`}); continue; }
      const top = document.elementFromPoint(r.left + r.width / 2, r.top + Math.min(r.height / 2, 12));
      if (top && top !== el && !el.contains(top)) out.push({sel, why: `covered by ${top.id ? '#' + top.id : top.tagName.toLowerCase()}`});
    }
    return out;
  }, selectors);
}

// Hard gate 5. axe-core serious and critical violations.
export async function axeViolations(page, {tags = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'], exclude = []} = {}) {
  // judge the settled frame (the skill's 0.75 s rule): finite animations run to their end first
  await page.evaluate(() => Promise.race([
    Promise.all(document.getAnimations().filter((a) => a.effect?.getComputedTiming?.().iterations !== Infinity).map((a) => a.finished.catch(() => null))),
    new Promise((r) => setTimeout(r, 750)),
  ]));
  let b = new AxeBuilder({page}).withTags(tags);
  for (const e of exclude) b = b.exclude(e);
  const r = await b.analyze();
  return r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => ({id: v.id, impact: v.impact, help: v.help, nodes: v.nodes.length, target: v.nodes.slice(0, 3).map((n) => n.target.join(' '))}));
}

// Text cut off with no way to read the full value: an ellipsis or a clamp, and no title (or
// aria-label) on the element or an ancestor within four levels that holds the text.
export async function truncatedWithoutValue(page, scope = 'body') {
  return page.evaluate((scope) => {
    const out = [];
    for (const el of document.querySelectorAll(`${scope} *`)) {
      if (el.offsetParent === null || !el.textContent.trim() || el.closest('dialog:not([open])')) continue;
      const cs = getComputedStyle(el);
      const cut = (cs.textOverflow === 'ellipsis' && el.scrollWidth > el.clientWidth + 1) || (cs.webkitLineClamp !== 'none' && cs.webkitLineClamp && el.scrollHeight > el.clientHeight + 1);
      if (!cut) continue;
      const text = el.textContent.trim().replace(/\s+/g, ' ');
      let a = el, ok = false;
      for (let i = 0; i < 5 && a; i++, a = a.parentElement) {
        const t = (a.getAttribute('title') || '') + ' ' + (a.getAttribute('aria-label') || '');
        if (t.replace(/\s+/g, ' ').includes(text.slice(0, Math.min(text.length, 40)))) { ok = true; break; }
      }
      if (!ok) out.push({text: text.slice(0, 80), sel: el.className && typeof el.className === 'string' ? '.' + el.className.split(' ')[0] : el.tagName.toLowerCase()});
    }
    return out;
  }, scope);
}

// Layout shift (CLS entries without recent input), summed from a buffered observer.
export async function startShifts(page) {
  await page.evaluate(() => {
    window.__ux_shift = 0;
    window.__ux_shift_t0 = performance.now();
    new PerformanceObserver((l) => { for (const e of l.getEntries()) if (!e.hadRecentInput) window.__ux_shift += e.value; }).observe({type: 'layout-shift', buffered: false});
  });
}
export async function shiftsPerMinute(page) {
  return page.evaluate(() => { const min = (performance.now() - window.__ux_shift_t0) / 60000; return min > 0 ? window.__ux_shift / min : 0; });
}

// The screen-only user's eyes: a control it may use is found by role and name, fully inside the
// viewport and not covered at its centre. Returns {locator, glance} or null; glance 1 when the
// control is inside the primary region, 2 elsewhere in view.
export async function visibleControl(page, role, name, primary) {
  const all = page.getByRole(role, typeof name === 'string' ? {name, exact: true} : {name});
  const n = await all.count();
  for (let i = 0; i < n; i++) {
    const loc = all.nth(i);
    if (!(await loc.isVisible())) continue;
    const ok = await loc.evaluate((el, primary) => {
      const r = el.getBoundingClientRect();
      if (r.left < 0 || r.top < 0 || r.right > innerWidth || r.bottom > innerHeight || !r.width || !r.height) return null;
      const top = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      if (!top || (top !== el && !el.contains(top))) return null;
      return primary && el.closest(primary) ? 1 : 2;
    }, primary);
    if (ok) return {locator: loc, glance: ok};
  }
  return null;
}
// Text the user can read without acting: inside the viewport, in a visible element.
export async function visibleText(page, scope, text) {
  return page.evaluate(([scope, text]) => {
    const root = document.querySelector(scope);
    if (!root) return false;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    for (let el = walker.currentNode; el; el = walker.nextNode()) {
      if (!el.textContent.includes(text)) continue;
      if ([...el.children].some((c) => c.textContent.includes(text))) continue;
      const r = el.getBoundingClientRect();
      // cut off by its own ellipsis, or running past a clipping ancestor: the user cannot read it
      let cut = getComputedStyle(el).textOverflow === 'ellipsis' && el.scrollWidth > el.clientWidth + 1;
      for (let a = el.parentElement, i = 0; !cut && a && a !== root.parentElement && i < 4; a = a.parentElement, i++) {
        if (getComputedStyle(a).overflowX === 'visible') continue;
        const q = a.getBoundingClientRect();
        if (r.right > q.right + 1 || r.left < q.left - 1) cut = true;
      }
      if (cut) continue;
      if (r.width && r.height && r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth && el.offsetParent !== null) return true;
    }
    return false;
  }, [scope, text]);
}

// Words a person reads in a region (reading load).
export async function wordsIn(page, sel) {
  return page.evaluate((sel) => { const el = document.querySelector(sel); return el ? el.innerText.split(/\s+/).filter(Boolean).length : 0; }, sel);
}
