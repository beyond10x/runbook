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

export const sourceIndex = async (base, name) => (await ask(base, '/api/sources')).json.findIndex((s) => s.name === name);
export const fileUrl = (src, rel) => `/api/file?source=${src}&path=${encodeURIComponent(rel)}`;
export const VmHWM = (pid) => { try { return Number(/VmHWM:\s+(\d+) kB/.exec(fs.readFileSync(`/proc/${pid}/status`, 'utf8'))[1]) * 1024; } catch { return null; } };
