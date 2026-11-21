// The layout contract at phone sizes: generic checks over any page, no project code. Vertical page
// scroll is allowed on a phone; horizontal page scroll is not. Every function takes a Playwright page
// and plain data from the adapter; each returns its offenders, so a count is the metric.
import {visibilityMisses} from './contract.mjs';

// Shared in-page helpers, installed once per document.
async function helpers(page) {
  await page.evaluate(() => {
    if (window.__uxp) return;
    const name = (el) => {
      if (el.id) return `#${el.id}`;
      const cls = typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean).slice(0, 2).join('.') : (el.className?.baseVal || '').trim().split(/\s+/).filter(Boolean).slice(0, 2).join('.');
      const own = `${el.tagName.toLowerCase()}${cls ? '.' + cls : ''}`;
      const up = el.parentElement?.closest('[id]');
      return up && up !== document.body ? `#${up.id} ${own}` : own;
    };
    // rendered: a box, visible, not inside a closed dialog, not a fixed panel parked off the screen
    const shown = (el) => {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) return false;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || Number(cs.opacity) === 0 && !el.matches('input')) return false;
      if (el.closest('dialog:not([open])')) return false;
      for (let a = el; a && a !== document.body; a = a.parentElement) {
        const s = getComputedStyle(a);
        if (s.display === 'none') return false;
        if (s.position === 'fixed') {
          const q = a.getBoundingClientRect();
          if (q.left >= innerWidth - 1 || q.right <= 1 || q.top >= innerHeight - 1 || q.bottom <= 1) return false;
        }
      }
      return true;
    };
    // the element's box in viewport coordinates, cut by every ancestor that clips (overflow not
    // visible); the body's and the root's overflow belong to the viewport, so they do not count here
    const clipped = (el) => {
      const r = el.getBoundingClientRect();
      let l = r.left, t = r.top, rt = r.right, b = r.bottom;
      for (let a = el.parentElement; a && a !== document.body && a !== document.documentElement; a = a.parentElement) {
        const s = getComputedStyle(a);
        if (s.overflowX !== 'visible' || s.overflowY !== 'visible') {
          const q = a.getBoundingClientRect();
          if (s.overflowX !== 'visible') { l = Math.max(l, q.left); rt = Math.min(rt, q.right); }
          if (s.overflowY !== 'visible') { t = Math.max(t, q.top); b = Math.min(b, q.bottom); }
        }
        if (s.position === 'fixed') break;
      }
      return {l, t, r: rt, b};
    };
    const ownText = (el) => [...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join(' ').replace(/\s+/g, ' ').trim();
    // rendered font size: an SVG text is drawn at its CSS size times the SVG's scale
    const fontPx = (el) => {
      const px = parseFloat(getComputedStyle(el).fontSize) || 0;
      if (el instanceof SVGElement && el.getScreenCTM) { const m = el.getScreenCTM(); return m ? px * Math.hypot(m.a, m.b) : px; }
      return px;
    };
    window.__uxp = {name, shown, clipped, ownText, fontPx};
  });
}

// Horizontal page scroll in CSS px (0 when none), and the outermost elements that run past the
// viewport's left or right edge where nothing clips them.
export async function horizontalOverflow(page) {
  await helpers(page);
  return page.evaluate(() => {
    const {name, shown, clipped} = window.__uxp;
    const de = document.scrollingElement || document.documentElement;
    const doc = Math.max(0, de.scrollWidth - innerWidth);
    const off = new Set();
    const wide = [];
    for (const el of document.querySelectorAll('body *')) {
      if (!shown(el)) continue;
      const c = clipped(el);
      if (c.r <= c.l) continue;
      if (c.r > innerWidth + 1 || c.l < -1) {
        off.add(el);
        if (el.parentElement && off.has(el.parentElement)) continue;
        wide.push({sel: name(el), left: Math.round(c.l), right: Math.round(c.r)});
      }
    }
    return {doc, wide};
  });
}

// Tap targets smaller than min x min CSS px, over the whole page (it scrolls). A link inside a line
// of text is exempt (WCAG 2.5.8 inline); an invisible input inside its label counts as the label.
export async function smallTargets(page, selector, min = 44) {
  await helpers(page);
  return page.evaluate(([selector, min]) => {
    const {name, shown} = window.__uxp;
    const out = [];
    const seen = new Set();
    for (const el of document.querySelectorAll(selector)) {
      if (seen.has(el)) continue;
      seen.add(el);
      if (el.matches('input') && Number(getComputedStyle(el).opacity) === 0 && el.closest('label')) continue;
      if (!shown(el)) continue;
      if (el.matches('[disabled]')) continue;
      const cs = getComputedStyle(el);
      if (el.matches('a') && cs.display === 'inline' && [...(el.parentElement?.childNodes || [])].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
      const r = el.getBoundingClientRect();
      if (r.width >= min - 0.5 && r.height >= min - 0.5) continue;
      out.push({sel: name(el), w: Math.round(r.width), h: Math.round(r.height), text: (el.innerText || el.getAttribute('aria-label') || '').trim().replace(/\s+/g, ' ').slice(0, 40)});
    }
    return out;
  }, [selector, min]);
}

// Text drawn smaller than its floor: a label (up to 3 words) under min px, a line of body text (4 or
// more words) under body px. Over the whole page; distinct by element and size.
export async function smallText(page, {min = 12, body = 14} = {}) {
  await helpers(page);
  return page.evaluate(([min, body]) => {
    const {name, shown, ownText, fontPx} = window.__uxp;
    const out = [];
    for (const el of document.querySelectorAll('body *')) {
      const t = ownText(el);
      if (!t || !/[A-Za-z0-9]/.test(t)) continue;
      if (!shown(el)) continue;
      const px = Math.round(fontPx(el) * 10) / 10;
      const words = t.split(' ').length;
      const floor = words >= 4 ? body : min;
      if (px + 0.05 < floor) out.push({sel: name(el), px, kind: words >= 4 ? 'body' : 'label', text: t.slice(0, 40)});
    }
    return out;
  }, [min, body]);
}

// Controls in the viewport whose centre another element covers (a column over a card, a bar over a
// button). Only controls whose centre is on the screen.
export async function coveredControls(page, selector) {
  await helpers(page);
  return page.evaluate((selector) => {
    const {name, shown, clipped} = window.__uxp;
    const out = [];
    for (const el of document.querySelectorAll(selector)) {
      if (!shown(el)) continue;
      if (el.matches('input') && Number(getComputedStyle(el).opacity) === 0) continue;
      // the part its own scrolling boxes show; a control scrolled out of its box is not covered
      const c = clipped(el);
      if (c.r - c.l < 1 || c.b - c.t < 1) continue;
      const x = (c.l + c.r) / 2, y = (c.t + c.b) / 2;
      if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) continue;
      const top = document.elementFromPoint(x, y);
      if (!top || top === el || el.contains(top) || top.contains(el) && top.matches('label')) continue;
      out.push({sel: name(el), by: name(top), text: (el.innerText || el.getAttribute('aria-label') || '').trim().replace(/\s+/g, ' ').slice(0, 40)});
    }
    return out;
  }, selector);
}

// The always-visible elements, each an ordered list of selectors (the first one rendered stands for
// it), checked at the top of the page, half-way down and at the bottom.
export async function alwaysVisibleAcross(page, items) {
  const misses = [];
  const height = await page.evaluate(() => (document.scrollingElement || document.documentElement).scrollHeight - innerHeight);
  const stops = height > 1 ? [0, Math.round(height / 2), height] : [0];
  for (const y of stops) {
    await page.evaluate((y) => window.scrollTo(0, y), y);
    await page.waitForTimeout(60);
    for (const [key, sels] of Object.entries(items)) {
      const sel = await page.evaluate((sels) => sels.find((s) => { const el = document.querySelector(s); return el && el.getClientRects().length && getComputedStyle(el).display !== 'none'; }) || sels[0], sels);
      for (const m of await visibilityMisses(page, [sel])) misses.push({key, sel, at: y, why: m.why});
    }
  }
  await page.evaluate(() => window.scrollTo(0, 0));
  return misses;
}

// Reachable within one scroll screen: the element's bottom, in page coordinates, at most two viewport
// heights from the top. Returns {docTop, docBottom, reach} (reach 0 when it is, 1 when it is not).
export async function withinOneScroll(page, selector) {
  return page.evaluate((selector) => {
    const el = document.querySelector(selector);
    if (!el || !el.getClientRects().length) return {docTop: null, docBottom: null, reach: 1};
    const r = el.getBoundingClientRect();
    const top = r.top + scrollY, bottom = r.bottom + scrollY;
    return {docTop: Math.round(top), docBottom: Math.round(bottom), reach: bottom <= 2 * innerHeight ? 0 : 1};
  }, selector);
}

// An open dialog as a full-screen sheet: it covers the viewport (within 2 px) and its close control
// is fully in view. Returns the problems (empty when it is a sheet).
export async function sheetProblems(page, dialog, close) {
  return page.evaluate(([dialog, close]) => {
    const d = document.querySelector(dialog);
    if (!d) return ['no open dialog'];
    const r = d.getBoundingClientRect();
    const out = [];
    if (r.left > 2 || r.top > 2 || r.right < innerWidth - 2 || r.bottom < innerHeight - 2) out.push(`not full-screen (${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.right)},${Math.round(r.bottom)})`);
    const c = [...d.querySelectorAll(close)].find((x) => x.getClientRects().length);
    if (!c) out.push('no close control');
    else {
      const q = c.getBoundingClientRect();
      if (q.left < 0 || q.top < 0 || q.right > innerWidth || q.bottom > innerHeight) out.push('close control out of view');
      else if (q.width < 44 || q.height < 44) out.push(`close control ${Math.round(q.width)}x${Math.round(q.height)}`);
    }
    return out;
  }, [dialog, close]);
}
