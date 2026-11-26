// Helpers of the security-products-* specs (independent review of story inspector-shows-step-products).
// Not a spec. Every run directory is written by hand with invented text through products-fixture.mjs.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const fixtureRoot = (name) => path.join(FIXTURE_DIR, name);
export const put = (file, data) => { fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, data); };

// GET (or another method) with a bound on the wait: a server that hangs answers nothing, and that is a result
export async function ask(base, p, {method = 'GET', headers = {}, timeout = 8000} = {}) {
  const t0 = Date.now();
  try {
    const r = await fetch(base + p, {method, headers, signal: AbortSignal.timeout(timeout)});
    const text = await r.text();
    let json = null;
    try { json = JSON.parse(text); } catch { /* not JSON */ }
    return {status: r.status, headers: r.headers, text, json, ms: Date.now() - t0, hung: false};
  } catch (e) {
    return {status: 0, headers: new Headers(), text: '', json: null, ms: Date.now() - t0, hung: true, error: String(e?.name || e)};
  }
}

// a request with a Host (and Origin) header of our choosing: fetch cannot set Host
export function raw(port, p, {method = 'GET', headers = {}} = {}) {
  return new Promise((ok, no) => {
    const q = http.request({host: '127.0.0.1', port, path: p, method, headers}, (res) => {
      let body = '';
      res.on('data', (d) => { body += d; });
      res.on('end', () => ok({status: res.statusCode, headers: res.headers, body}));
    });
    q.on('error', no);
    q.setTimeout(8000, () => { q.destroy(new Error('no answer in 8 s')); });
    q.end();
  });
}

// The DOM audit of invariant 4 (the content of a run never becomes markup or code of the page): below `scope`, no element
// that loads or runs, no handler or source attribute, no style but the one the page sets, no link that is not http, https
// or the page's own #file=. The inspector's page (security-products-page.spec.mjs) and the daemon's
// (daemon-run-tree-security.spec.mjs) run it.
export const audit = (page, scope = '#panel *, #product *', styles = '^--depth:\\d+$') => page.evaluate(([scope, styles]) => {
  const bad = [];
  const forbidden = /^(img|svg|iframe|object|embed|video|audio|source|picture|math|template|noscript|base|meta|form|input|frame|frameset|applet|portal)$/i;
  for (const el of document.querySelectorAll(scope)) {
    if (forbidden.test(el.tagName)) bad.push(`element ${el.tagName}`);
    if (el.tagName === 'SCRIPT' || el.tagName === 'STYLE' || el.tagName === 'LINK') bad.push(`element ${el.tagName}`);
    for (const a of el.attributes) {
      if (/^on/i.test(a.name)) bad.push(`${el.tagName}[${a.name}]`);
      if (/^(src|srcset|srcdoc|formaction|action|xlink:href|background|poster|data|ping)$/i.test(a.name)) bad.push(`${el.tagName}[${a.name}]`);
      if (a.name === 'style' && !new RegExp(styles).test(a.value)) bad.push(`style=${a.value}`);
      if (a.name === 'class' && /[<>"'=]/.test(a.value)) bad.push(`class=${a.value}`);
    }
    if (el.tagName === 'A') {
      const href = el.getAttribute('href');
      if (href === null) continue;
      if (href.startsWith('#file=')) {
        const p = decodeURIComponent(href.slice(6));
        if (p.startsWith('/') || p.split('/').some((n) => n === '..' || n.startsWith('.'))) bad.push(`file link ${p}`);
        continue;
      }
      let u = null;
      try { u = new URL(href); } catch { bad.push(`href ${href}`); continue; }
      if (u.protocol !== 'http:' && u.protocol !== 'https:') bad.push(`href ${href}`);
      if (el.getAttribute('target') !== '_blank' || el.getAttribute('rel') !== 'noopener noreferrer') bad.push(`external link without target and rel ${href}`);
    }
  }
  return bad;
}, [scope, styles]);

// After a step of a hostile case: the audit is clean, nothing ran (no window.__pwned*), no script node was added, no
// dialog, no page error, no request to an address that is not the server's own. `seen` is what openPage collected.
export async function quiet(expect, page, seen, label, scope, styles) {
  expect(await audit(page, scope, styles), `${label}: DOM`).toEqual([]);
  expect(await page.evaluate(() => Object.keys(window).filter((k) => k.startsWith('__pwned')))).toEqual([]);
  expect(await page.locator('script').count(), `${label}: script nodes`).toBe(seen.scripts);
  expect(seen.dialogs, `${label}: dialogs`).toEqual([]);
  expect(seen.errors, `${label}: page errors`).toEqual([]);
  expect(seen.requests, `${label}: requests`).toEqual([]);
}

export const sourceIndex = async (base, name) => (await ask(base, '/api/sources')).json.findIndex((s) => s.name === name);
export const fileUrl = (src, rel) => `/api/file?source=${src}&path=${encodeURIComponent(rel)}`;
export const VmHWM = (pid) => { try { return Number(/VmHWM:\s+(\d+) kB/.exec(fs.readFileSync(`/proc/${pid}/status`, 'utf8'))[1]) * 1024; } catch { return null; } };
