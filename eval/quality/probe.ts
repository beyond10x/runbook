// Does context.setOffline also cut a service worker's fetch() in this Playwright/Chromium?
import {chromium} from 'playwright';
import http from 'node:http';
let n = 0;
const html = () => `<!doctype html><h1>hit ${++n}</h1><script>navigator.serviceWorker.register('/sw.js')</script>`;
const sw = `self.addEventListener('install',e=>self.skipWaiting());self.addEventListener('activate',e=>e.waitUntil(clients.claim()));
self.addEventListener('fetch',e=>{e.respondWith(fetch(e.request).then(r=>{const c=r.clone();caches.open('x').then(k=>k.put(e.request,c));return r}).catch(()=>caches.match(e.request)))});`;
const srv = http.createServer((q, r) => {
  if (q.url === '/sw.js') { r.setHeader('content-type', 'text/javascript'); r.end(sw); return; }
  r.setHeader('content-type', 'text/html'); r.end(html());
}).listen(0, '127.0.0.1');
await new Promise(r => srv.on('listening', r));
const port = (srv.address() as any).port;
const b = await chromium.launch();
const ctx = await b.newContext();
const p = await ctx.newPage();
await p.goto(`http://127.0.0.1:${port}/`);
await p.waitForTimeout(1000);
await p.reload(); await p.waitForTimeout(500);
console.log('online, controlled:', await p.evaluate(() => !!navigator.serviceWorker.controller), await p.textContent('h1'));
await ctx.setOffline(true);
try { await p.reload(); console.log('offline reload:', await p.textContent('h1'), 'server hits', n); } catch (e) { console.log('offline reload failed', String(e).slice(0, 100)); }
console.log('fetch from page offline:', await p.evaluate(() => fetch('/x').then(r => 'reached ' + r.status, e => 'blocked')));
await b.close(); srv.close();
